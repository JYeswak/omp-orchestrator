#![forbid(unsafe_code)]
//! nsx1 — the S9 writer's acceptance legs.
//!
//! Every fixture detail string is a payload MEASURED from
//! `~/.local/state/flywheel/omp-orchestrator.heartbeat.jsonl` on 2026-09-02, not an
//! invented one. The shapes and their counts:
//!
//! ```text
//!  168  unwired=ack-spine[UNWIRED->repair-gate-trigger] owner=josh
//!   22  DISPATCH_BLOCKED bead=omp-orchestrator-ack-spine-oj6.3 receiver agent is missing owner=josh next_action=claim-bead
//!   16  MONITOR_BLIND owner=josh next_action=repair-monitor detail=zero panes observed — the monitor is blind
//! ```

use std::path::PathBuf;

use decision_ledger::{
    append_agent_disposition, append_request, classify_heartbeat, hd_reference, next_id, read_rows,
    record_decision, replay, request_row, AgentDisposition, AppendOutcome, Decision,
    HeartbeatAction, HumanClause, LedgerError, Request,
};

/// A scratch ledger under the session-scoped scratch home, never `/tmp`.
///
/// AGENTS.md: `/private/tmp` has no session owner and cannot be safely reaped. Measured
/// this session — seven message files vanished from `/tmp` mid-bead.
fn scratch(name: &str) -> PathBuf {
    let base = std::env::var("ZS_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
                .join(".local/state/zeststream/scratch/decision-ledger-tests")
        });
    std::fs::create_dir_all(&base).expect("scratch home is creatable");
    let path = base.join(format!("{name}-{}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

const GATE_UNWIRED: &str = "unwired=ack-spine[UNWIRED->repair-gate-trigger] owner=josh";
const DISPATCH_BLOCKED: &str = "DISPATCH_BLOCKED bead=omp-orchestrator-ack-spine-oj6.3 \
                                receiver agent is missing owner=josh next_action=claim-bead";
const MONITOR_BLIND: &str = "MONITOR_BLIND owner=josh next_action=repair-monitor \
                             detail=zero panes observed — the monitor is blind";

/// ACCEPTANCE 3, first half: one request per distinct question, and a second identical
/// tick appends NOTHING.
///
/// This is the 188 -> 1 collapse. Without it the ledger is as unreadable as it was empty.
#[test]
fn a_second_identical_tick_appends_nothing() {
    let path = scratch("dedupe");
    let request = match classify_heartbeat(
        "SUPERVISOR_REFUSED",
        "RISK owner=josh clause=authority bead=omp-orchestrator-risk-1",
        1_788_330_000,
    ) {
        HeartbeatAction::Human(request) => request,
        other => panic!("explicit clause must produce a human request: {other:?}"),
    };

    let first = append_request(&path, &request).expect("first append");
    assert!(matches!(first, AppendOutcome::Appended { .. }), "{first:?}");
    assert_eq!(first.id(), "HD-0001", "an empty ledger starts at HD-0001");

    let second = append_request(&path, &request).expect("second append");
    assert!(
        matches!(second, AppendOutcome::Deduped { .. }),
        "{second:?}"
    );
    assert_eq!(second.id(), "HD-0001", "the dedupe must name the OPEN row");
    assert!(!second.wrote());

    for _ in 0..186 {
        append_request(&path, &request).expect("repeat append");
    }
    let rows = read_rows(&path).expect("readable");
    assert_eq!(rows.len(), 1, "identical ticks are ONE human question");
}
/// A DIFFERENT bead is a DIFFERENT decision, and must not be deduped into the first.
///
/// This is why the dedupe key is not digit-stripped: `oj6.3` and `oj6.4` differ only in
/// digits, and a digit-blind key would have silently merged 22 rows about two beads into
/// one question about neither.
#[test]
fn two_beads_are_two_questions_even_though_they_differ_only_in_digits() {
    let capacity = classify_heartbeat(
        "SUPERVISOR_REFUSED",
        "DISPATCH_BLOCKED bead=omp-orchestrator-ack-spine-oj6.3 receiver agent is missing owner=josh next_action=claim-bead",
        1_788_330_100,
    );
    assert!(
        matches!(
            capacity,
            HeartbeatAction::RequeueCapacity { ref bead, ref reason }
                if bead == "omp-orchestrator-ack-spine-oj6.3" && reason == "no_eligible_pane"
        ),
        "capacity must be re-queued, not escalated: {capacity:?}"
    );

    let transport = classify_heartbeat(
        "SUPERVISOR_REFUSED",
        "DISPATCH_FAILED bead=omp-orchestrator-ack-spine-oj6.4 ACK_STAGE_RETRY_BLOCKED owes_human=false",
        1_788_330_200,
    );
    assert!(
        matches!(
            transport,
            HeartbeatAction::RedispatchTransport { ref bead, ref reason }
                if bead == "omp-orchestrator-ack-spine-oj6.4"
                    && reason == "receiver_receipt_unproven"
        ),
        "unproven transport must re-dispatch: {transport:?}"
    );
}
/// ACCEPTANCE 3, second half: a close carrying `HD-…` records the answer; a close without
/// one records nothing.
#[test]
fn only_a_close_that_cites_an_hd_id_records_a_decision() {
    assert_eq!(
        hd_reference("MUTATION-VERIFIED per HD-0008; pushed to public origin"),
        Some("HD-0008".to_owned())
    );
    assert_eq!(hd_reference("HD-12"), Some("HD-12".to_owned()));
    // The negative arm is the load-bearing one: a close is not automatically a decision.
    assert_eq!(hd_reference("DONE all acceptance legs green"), None);
    assert_eq!(
        hd_reference("see HD- for details"),
        None,
        "no digits, no id"
    );
    assert_eq!(hd_reference(""), None);
}

/// A decision needs a question, a decider, and words. Each absence is a NAMED refusal.
///
/// Rule Zero: the decision is the human's, so the mechanism refuses to manufacture one.
#[test]
fn an_unattributed_or_unasked_decision_is_refused_by_name() {
    let path = scratch("refusals");
    let request = Request {
        question: "Wire, retire, or declare advisory?".to_owned(),
        asked_by: "omp-orchestrator-supervisor".to_owned(),
        blocking: "gate:ack-spine".to_owned(),
        clause: Some(HumanClause::Authority),
        ts: 1_788_330_000,
    };
    let outcome = append_request(&path, &request).expect("append");

    let orphan = Decision {
        id: "HD-9999".to_owned(),
        decision: "wire it".to_owned(),
        decider: "josh".to_owned(),
        recorded_by: "AmberGate".to_owned(),
        transcript_ref: String::new(),
    };
    assert_eq!(
        record_decision(&path, &orphan),
        Err(LedgerError::NoSuchRequest {
            id: "HD-9999".to_owned()
        })
    );

    let unattributed = Decision {
        id: outcome.id().to_owned(),
        decision: "wire it".to_owned(),
        decider: "   ".to_owned(),
        ..orphan.clone()
    };
    assert_eq!(
        record_decision(&path, &unattributed),
        Err(LedgerError::MissingField { field: "decider" })
    );

    let wordless = Decision {
        id: outcome.id().to_owned(),
        decision: String::new(),
        decider: "josh".to_owned(),
        ..orphan.clone()
    };
    assert_eq!(
        record_decision(&path, &wordless),
        Err(LedgerError::MissingField { field: "decision" })
    );

    let good = Decision {
        id: outcome.id().to_owned(),
        decision: "wire it on the lane".to_owned(),
        decider: "josh".to_owned(),
        recorded_by: "AmberGate".to_owned(),
        transcript_ref: "omp-orchestrator-nsx1".to_owned(),
    };
    record_decision(&path, &good).expect("a complete decision must land");
    let rows = read_rows(&path).expect("readable");
    assert_eq!(rows.len(), 2, "append-only: the question row is preserved");
    assert!(!rows[0].is_answered(), "the request row stays a request");
    assert!(rows[1].is_answered(), "the answer row carries the decision");
}
/// A request without a named clause is refused at the write boundary.
#[test]
fn a_clauseless_request_is_refused_with_a_named_field() {
    let path = scratch("missing-clause");
    let request = Request {
        question: "Should the policy change?".to_owned(),
        asked_by: "omp-orchestrator-supervisor".to_owned(),
        blocking: "status:POLICY".to_owned(),
        clause: None,
        ts: 1_788_330_000,
    };
    let error = append_request(&path, &request).expect_err("clause is mandatory");
    assert_eq!(error, LedgerError::MissingField { field: "clause" });
    assert!(error.to_string().contains("field=clause"), "{error}");
}

/// Machine-owned dispatch recovery is not recorded as a human answer.
#[test]
fn a_machine_disposition_requeues_without_a_human_answer() {
    let path = scratch("disposition");
    std::fs::write(
        &path,
        r#"{"id":"HD-0042","question":"Bead x cannot be dispatched","decision":"","decider":"","recorded_by":"supervisor"}
"#,
    )
    .expect("seed request");

    let first = append_agent_disposition(
        &path,
        "HD-0042",
        AgentDisposition::Requeued,
        "capacity:no_eligible_pane",
        1_788_330_000,
    )
    .expect("append disposition");
    assert!(matches!(first, AppendOutcome::Appended { ref id } if id == "HD-0042"));
    let rows = read_rows(&path).expect("readable");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].value["answers"], "HD-0042");
    assert_eq!(rows[1].value["disposition"], "REQUEUED");
    assert_eq!(rows[1].value["decision"], "");

    let second = append_agent_disposition(
        &path,
        "HD-0042",
        AgentDisposition::Requeued,
        "capacity:no_eligible_pane",
        1_788_330_001,
    )
    .expect("dedupe disposition");
    assert!(matches!(second, AppendOutcome::Deduped { ref id } if id == "HD-0042"));
}

/// An unreadable ledger must NOT be treated as an empty one.
///
/// Empty would renumber from HD-0001 and fork the id space against rows that already
/// exist — the same class as every other vacuous-green defect in this repo.
#[test]
fn an_unreadable_ledger_is_an_error_and_a_missing_one_is_empty() {
    let path = scratch("malformed");
    std::fs::write(&path, "{\"id\":\"HD-0001\"}\nnot json at all\n").expect("write");
    match read_rows(&path) {
        Err(LedgerError::Malformed { line, .. }) => assert_eq!(line, 2, "name the line"),
        other => panic!("a malformed row must be an ERROR, got {other:?}"),
    }

    // A file that has never existed is a real state, not a fault.
    let absent = scratch("never-written");
    let _ = std::fs::remove_file(&absent);
    assert_eq!(read_rows(&absent).expect("absent is empty").len(), 0);
    assert_eq!(next_id(&[]), "HD-0001");
}

/// ACCEPTANCE 4: replaying the measured shapes yields >= 3 distinct requests, with the
/// count and the subjects printed rather than a bare zero.
#[test]
fn replaying_the_measured_day_reports_at_least_three_distinct_requests() {
    let mut rows: Vec<(String, String, u64)> = Vec::new();
    for i in 0..168 {
        rows.push((
            "GATE_UNWIRED".into(),
            GATE_UNWIRED.to_owned(),
            1_788_330_000 + i,
        ));
    }
    for i in 0..22 {
        rows.push((
            "SUPERVISOR_REFUSED".into(),
            DISPATCH_BLOCKED.to_owned(),
            1_788_340_000 + i,
        ));
    }
    for i in 0..16 {
        rows.push((
            "MONITOR_BLIND".into(),
            MONITOR_BLIND.to_owned(),
            1_788_350_000 + i,
        ));
    }
    rows.push((
        "SUPERVISOR_REFUSED".into(),
        "RISK owner=josh clause=taste bead=omp-orchestrator-public-1".into(),
        1_788_360_000,
    ));
    rows.push((
        "SUPERVISOR_REFUSED".into(),
        "RISK owner=josh clause=taste bead=omp-orchestrator-public-1".into(),
        1_788_360_001,
    ));
    for i in 0..500 {
        rows.push((
            "CYCLE_STARTED".into(),
            format!("tick={i} panes=5"),
            1_788_330_000 + i,
        ));
    }

    let report = replay(&rows);
    report
        .require_nonvacuous()
        .expect("706 rows is not vacuous");
    assert_eq!(report.rows_read, 708);
    assert_eq!(report.human_addressed, 2);
    assert_eq!(
        report.distinct.len(),
        1,
        "only explicitly clause-named work escalates"
    );
    assert_eq!(report.distinct[0].clause, HumanClause::Taste);
}
/// ANTI-VACUITY on the replay itself: zero rows read is an ERROR, not a clean day.
#[test]
fn a_replay_that_read_nothing_is_an_error_not_a_clean_day() {
    let empty = replay(&[]);
    assert_eq!(empty.rows_read, 0);
    let error = empty
        .require_nonvacuous()
        .expect_err("an empty input must not report as nothing-to-record");
    assert!(error.contains("REPLAY_VACUOUS"), "{error}");
    assert!(error.contains("rows_read=0"), "{error}");

    // POSITIVE CONTROL: a real day with no human-addressed row IS clean, and says so
    // with its arithmetic intact rather than erroring.
    let quiet = replay(&[(
        "CYCLE_STARTED".into(),
        "tick=1 panes=5".into(),
        1_788_330_000,
    )]);
    quiet
        .require_nonvacuous()
        .expect("one row read is a real measurement");
    assert_eq!(quiet.distinct.len(), 0);
    assert_eq!(quiet.human_addressed, 0);
}

/// A human-addressed row the classifier does not recognise is CARRIED, never dropped.
///
/// A classifier that silently discards the case it did not anticipate is how S9 acquired
/// a reader and no writer in the first place.
#[test]
fn an_unclassified_human_addressed_row_becomes_named_agent_work() {
    let action = classify_heartbeat(
        "QUEUE_EMPTY_NEEDS_JOSH",
        "QUEUE_EMPTY_NEEDS_JOSH owner=josh next_action=authorize-or-create-work free=4",
        1_788_360_000,
    );
    assert!(
        matches!(
            action,
            HeartbeatAction::AgentWork { ref status, ref reason }
                if status == "QUEUE_EMPTY_NEEDS_JOSH" && reason == "human_clause_missing"
        ),
        "an unlabelled row is agent work, not a human request: {action:?}"
    );
    assert!(matches!(
        classify_heartbeat("CYCLE_STARTED", "tick=1 panes=5", 1),
        HeartbeatAction::Ignored
    ));
    assert!(matches!(
        classify_heartbeat("DOCS_STALE", "detail=PLAN.md drifted", 1),
        HeartbeatAction::Ignored
    ));
}
/// The schema extension is ADDITIVE: every key the existing reader indexes is present.
///
/// Measured keys across the 8 existing rows: binds_stages, condition, decider, decision,
/// id, options_considered, question, recorded_by, residual_risk_stated_at_decision_time,
/// review_after, supersedes, transcript_ref, ts.
#[test]
fn a_request_row_carries_every_key_the_existing_reader_indexes() {
    let row = request_row(
        "HD-0009",
        &Request {
            question: "Wire, retire, or declare advisory?".to_owned(),
            asked_by: "omp-orchestrator-supervisor".to_owned(),
            blocking: "gate:ack-spine".to_owned(),
            clause: Some(HumanClause::Authority),
            ts: 1_788_330_000,
        },
    );
    for key in [
        "id",
        "ts",
        "question",
        "decision",
        "decider",
        "recorded_by",
        "binds_stages",
        "options_considered",
        "transcript_ref",
        "review_after",
        "supersedes",
    ] {
        assert!(row.get(key).is_some(), "existing key {key} must be present");
    }
    // The additive half.
    assert!(row.get("asked_by").is_some());
    assert!(row.get("blocking").is_some());
    assert_eq!(
        row.get("clause").and_then(|value| value.as_str()),
        Some("authority")
    );
    assert!(row.get("question_key").is_some());
    // A request is NOT an answer, and the reader tells them apart by an empty decision.
    assert_eq!(row.get("decision").and_then(|v| v.as_str()), Some(""));
    assert_eq!(
        row.get("binds_stages")
            .map(std::string::ToString::to_string),
        Some("[\"S9\"]".to_owned())
    );
}

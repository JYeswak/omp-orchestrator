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
    append_request, classify_heartbeat, hd_reference, next_id, question_key, read_rows,
    record_decision, replay, request_row, AppendOutcome, Decision, LedgerError, Request,
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
    let request = classify_heartbeat("GATE_UNWIRED", GATE_UNWIRED, 1_788_330_000)
        .expect("a row naming owner=josh is a human-addressed row");

    let first = append_request(&path, &request).expect("first append");
    assert!(matches!(first, AppendOutcome::Appended { .. }), "{first:?}");
    assert_eq!(first.id(), "HD-0001", "an empty ledger starts at HD-0001");

    let second = append_request(&path, &request).expect("second append");
    assert!(matches!(second, AppendOutcome::Deduped { .. }), "{second:?}");
    assert_eq!(second.id(), "HD-0001", "the dedupe must name the OPEN row");
    assert!(!second.wrote());

    // 186 more ticks, as measured. The file must still hold exactly one row.
    for _ in 0..186 {
        append_request(&path, &request).expect("repeat append");
    }
    let rows = read_rows(&path).expect("readable");
    assert_eq!(
        rows.len(),
        1,
        "188 identical GATE_UNWIRED ticks are ONE question asked 188 times"
    );
}

/// A DIFFERENT bead is a DIFFERENT decision, and must not be deduped into the first.
///
/// This is why the dedupe key is not digit-stripped: `oj6.3` and `oj6.4` differ only in
/// digits, and a digit-blind key would have silently merged 22 rows about two beads into
/// one question about neither.
#[test]
fn two_beads_are_two_questions_even_though_they_differ_only_in_digits() {
    let path = scratch("per-bead");
    let three = classify_heartbeat("SUPERVISOR_REFUSED", DISPATCH_BLOCKED, 1_788_330_100)
        .expect("classified");
    let four = classify_heartbeat(
        "SUPERVISOR_REFUSED",
        &DISPATCH_BLOCKED.replace("oj6.3", "oj6.4"),
        1_788_330_200,
    )
    .expect("classified");

    assert_ne!(
        question_key(&three.question),
        question_key(&four.question),
        "a digit-blind key would merge two beads into one decision"
    );
    assert_eq!(three.blocking, "bead:omp-orchestrator-ack-spine-oj6.3");
    assert_eq!(four.blocking, "bead:omp-orchestrator-ack-spine-oj6.4");

    append_request(&path, &three).expect("first");
    append_request(&path, &four).expect("second");
    assert_eq!(read_rows(&path).expect("readable").len(), 2);
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
    assert_eq!(hd_reference("see HD- for details"), None, "no digits, no id");
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
        ts: 1_788_330_000,
    };
    let outcome = append_request(&path, &request).expect("append");

    // No such question.
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

    // No decider.
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

    // No words.
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

    // KNOWN-GOOD: the complete form lands, or the refusals above prove nothing.
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
    // 168 + 22 + 16 rows, exactly as counted, plus noise the classifier must ignore.
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
    // Rows that address nobody. A classifier that counted these would inflate the loss.
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
        .expect("706 rows is not a vacuous replay");
    assert_eq!(report.rows_read, 706);
    assert_eq!(
        report.human_addressed, 206,
        "168 + 22 + 16; the 500 CYCLE_STARTED rows address nobody"
    );
    assert_eq!(
        report.distinct.len(),
        3,
        "three distinct questions, not 206 rows: {:?}",
        report
            .distinct
            .iter()
            .map(|entry| &entry.blocking)
            .collect::<Vec<_>>()
    );
    let subjects: Vec<&str> = report
        .distinct
        .iter()
        .map(|entry| entry.blocking.as_str())
        .collect();
    assert!(subjects.contains(&"gate:ack-spine"), "{subjects:?}");
    assert!(
        subjects.contains(&"bead:omp-orchestrator-ack-spine-oj6.3"),
        "{subjects:?}"
    );
    assert!(subjects.contains(&"gate:tick-monitor"), "{subjects:?}");
    // The occurrence counts are carried, so "168 times" survives the collapse.
    let gate = report
        .distinct
        .iter()
        .find(|entry| entry.blocking == "gate:ack-spine")
        .expect("present");
    assert_eq!(gate.occurrences, 168);
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
fn an_unclassified_human_addressed_row_still_becomes_a_request() {
    let request = classify_heartbeat(
        "QUEUE_EMPTY_NEEDS_JOSH",
        "QUEUE_EMPTY_NEEDS_JOSH owner=josh next_action=authorize-or-create-work free=4",
        1_788_360_000,
    )
    .expect("a row naming owner=josh must produce a request even if unmatched");
    assert!(request.question.contains("UNCLASSIFIED"), "{request:?}");
    assert_eq!(request.blocking, "status:QUEUE_EMPTY_NEEDS_JOSH");

    // And a row addressing nobody stays None, or every tick becomes a question.
    assert!(classify_heartbeat("CYCLE_STARTED", "tick=1 panes=5", 1).is_none());
    assert!(classify_heartbeat("DOCS_STALE", "detail=PLAN.md drifted", 1).is_none());
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
    assert!(row.get("question_key").is_some());
    // A request is NOT an answer, and the reader tells them apart by an empty decision.
    assert_eq!(row.get("decision").and_then(|v| v.as_str()), Some(""));
    assert_eq!(
        row.get("binds_stages").map(std::string::ToString::to_string),
        Some("[\"S9\"]".to_owned())
    );
}

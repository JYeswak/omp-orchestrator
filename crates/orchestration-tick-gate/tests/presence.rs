#![forbid(unsafe_code)]
//! psf7 — a MISSING tick row must be loud.
//!
//! `4wmo` made an INVALID row loud. Neither implies the other:
//!
//! ```text
//! row present + invalid  ->  4wmo catches it
//! row absent             ->  THIS FILE
//! ```
//!
//! The absent case is strictly worse, because an empty ledger and a healthy one produced
//! the same silence: `--ledger` on a missing FILE exits 2, but a missing ROW for a tick that
//! happened was indistinguishable from a tick that did not happen.

use orchestration_tick_gate::{
    append_receipt, build_receipt, detect_gap, validate_receipt, PaneDisposition, Receipt,
    TICK_OUTCOME_STATUSES,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "psf7-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn clock_row(status: &str, ts: u64, tick: u64) -> Value {
    json!({
        "event": "supervisor_heartbeat",
        "status": status,
        "ts_unix": ts,
        "tick": tick,
        "session": "omp-orchestrator",
    })
}

/// One tick's worth of the writer's real output, through the production builder.
fn a_real_row(ts: u64, tick: u64) -> Value {
    build_receipt(&mut Receipt {
            ts,
            tick,
            free_capacity: &["%1408".to_owned(), "%1413".to_owned()],
            attention: 0,
            dead: None,
            source: "omp-orchestrator supervisor observation",
            dispositions: &[PaneDisposition::Dispatched {
            pane: "%1413".to_owned(),
            bead: "omp-orchestrator-psf7".to_owned(),
            receipt: "free_capacity [] after send".to_owned(),
        }],
            fallback_reason: "decision=supervised-working; this tick did not dispatch to this pane",
            claims: vec![json!({ "figure": "free_capacity=2", "command": "tick-monitor observe" })],
            not_done: vec![json!("outcome not yet observed at row time")],
        })
}

// -----------------------------------------------------------------------------------
// ACCEPTANCE 4 — a normal tick writes EXACTLY ONE row and stays CLEAN
// -----------------------------------------------------------------------------------

#[test]
fn a_normal_tick_writes_exactly_one_valid_row() {
    // "A writer that emits nothing, or emits twice, is not caught by the validator" — so
    // the count is asserted here, where the validator cannot help.
    let dir = temp_dir("one-row");
    let ledger = dir.join("orchestration-ticks.jsonl");

    append_receipt(&ledger, &a_real_row(1_767_400_000, 6)).expect("first append");
    let body = std::fs::read_to_string(&ledger).expect("read");
    assert_eq!(
        body.lines().filter(|l| !l.trim().is_empty()).count(),
        1,
        "one tick must write exactly one row, got: {body}"
    );

    let row: Value = serde_json::from_str(body.trim()).expect("one physical JSONL line");
    assert_eq!(
        validate_receipt(&row),
        Ok(()),
        "the writer must not emit a row its sibling validator refuses"
    );

    // A second tick appends rather than replacing, and both stay valid.
    append_receipt(&ledger, &a_real_row(1_767_400_720, 7)).expect("second append");
    let body = std::fs::read_to_string(&ledger).expect("read");
    let rows: Vec<Value> = body
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("json"))
        .collect();
    assert_eq!(rows.len(), 2, "the append must not truncate the ledger");
    for row in &rows {
        assert_eq!(validate_receipt(row), Ok(()));
    }
}

#[test]
fn every_free_pane_is_named_exactly_once_by_construction() {
    // OC-L1 is satisfied by the BUILDER, not by the caller remembering. A decision arm that
    // names no pane still produces a valid row: the free set becomes refusals.
    let row = build_receipt(&mut Receipt {
            ts: 1,
            tick: 1,
            free_capacity: &["%1".to_owned(), "%2".to_owned(), "%3".to_owned()],
            attention: 0,
            dead: None,
            source: "src",
            dispositions: &[],
            fallback_reason: "decision=authorized-idle; nothing was dispatched",
            claims: vec![json!({ "figure": "f", "command": "c" })],
            not_done: vec![],
        });
    assert_eq!(validate_receipt(&row), Ok(()));
    assert_eq!(row["refused"].as_array().expect("refused").len(), 3);

    // A disposition for a pane that was NOT observed free must be dropped, because OC-L1
    // refuses "named but not observed free" and emitting it would make the writer produce
    // an invalid row. KNOWN-BAD input, valid output.
    let row = build_receipt(&mut Receipt {
            ts: 1,
            tick: 1,
            free_capacity: &["%1".to_owned()],
            attention: 0,
            dead: None,
            source: "src",
            dispositions: &[PaneDisposition::Dispatched {
            pane: "%99".to_owned(),
            bead: "b".to_owned(),
            receipt: "r".to_owned(),
        }],
            fallback_reason: "not free",
            claims: vec![json!({ "figure": "f", "command": "c" })],
            not_done: vec![],
        });
    assert_eq!(validate_receipt(&row), Ok(()));
    assert!(row["dispatched"].as_array().expect("dispatched").is_empty());

    // A blank refusal reason is replaced by the fallback: OC-L1 refuses an empty one.
    let row = build_receipt(&mut Receipt {
            ts: 1,
            tick: 1,
            free_capacity: &["%1".to_owned()],
            attention: 0,
            dead: None,
            source: "src",
            dispositions: &[PaneDisposition::Refused {
            pane: "%1".to_owned(),
            reason: "   ".to_owned(),
        }],
            fallback_reason: "the fallback reason",
            claims: vec![json!({ "figure": "f", "command": "c" })],
            not_done: vec![],
        });
    assert_eq!(validate_receipt(&row), Ok(()));
    assert_eq!(row["refused"][0]["reason"], "the fallback reason");
}

// -----------------------------------------------------------------------------------
// ACCEPTANCE 2 + 3 — the gap is NAMEABLE, and it fires when the writer is suppressed
// -----------------------------------------------------------------------------------

#[test]
fn a_suppressed_writer_is_reported_as_a_named_gap() {
    // FIRES-ON-KNOWN-BAD, and it is the whole bead. The clock keeps ticking because
    // `write_heartbeat` is a DIFFERENT code path from the row writer — which is exactly why
    // an external anchor works and a field inside the ledger cannot.
    let clock: Vec<Value> = vec![
        clock_row("CYCLE_STARTED", 1_767_400_100, 6),
        clock_row("DISPATCH_CLAIMED", 1_767_400_200, 6),
        clock_row("SUPERVISED_WORKING", 1_767_400_300, 7),
        clock_row("DISPATCH_RESULT_RECORDED", 1_767_400_400, 8),
    ];

    // THE MEASURED STATE THIS BEAD WAS FILED ABOUT: no ledger at all.
    let gap = detect_gap(&[], &clock).expect("an absent ledger beside a live clock IS a gap");
    assert_eq!(
        gap.outcomes_since_last_row, 3,
        "CYCLE_STARTED is not an outcome and must not inflate the gap"
    );
    assert_eq!(gap.newest_status, "DISPATCH_RESULT_RECORDED");
    assert_eq!(gap.newest_ts, 1_767_400_400);
    assert_eq!(gap.last_row_ts, None);

    // ASSERT ON EMITTED TEXT (gate rule 7).
    let text = gap.to_string();
    for needle in [
        "TICK_ROWS_MISSING",
        "outcomes_since_last_row=3",
        "last_row_ts=NONE",
        "newest_outcome=DISPATCH_RESULT_RECORDED",
        "a tick without a row did not happen",
    ] {
        assert!(
            text.contains(needle),
            "the gap text must carry {needle:?}: {text}"
        );
    }

    // A ledger that STOPPED being written is the same defect one row later.
    let stale = vec![a_real_row(1_767_400_250, 6)];
    let gap = detect_gap(&stale, &clock).expect("a stale ledger is still a gap");
    assert_eq!(
        gap.outcomes_since_last_row, 2,
        "only outcomes NEWER than the row count"
    );
    assert_eq!(gap.last_row_ts, Some(1_767_400_250));
}

#[test]
fn a_ledger_that_kept_up_reports_no_gap() {
    // KNOWN-GOOD ARM. A detector that fires on the healthy path is a detector that gets
    // routed around, and this one would fire on every tick of a working fleet.
    let clock: Vec<Value> = vec![
        clock_row("DISPATCH_CLAIMED", 1_767_400_200, 6),
        clock_row("SUPERVISED_WORKING", 1_767_400_300, 7),
    ];
    let current = vec![a_real_row(1_767_400_250, 6), a_real_row(1_767_400_350, 7)];
    assert_eq!(
        detect_gap(&current, &clock),
        None,
        "a ledger newer than every outcome owes nothing"
    );

    // And an idle clock — cycles started, nothing concluded — owes nothing either.
    let idle = vec![clock_row("CYCLE_STARTED", 1_767_499_999, 9)];
    assert_eq!(
        detect_gap(&[], &idle),
        None,
        "CYCLE_STARTED alone is not an owed row"
    );
}

#[test]
fn gap_status_coverage_is_declared_and_its_limit_is_named() {
    // The allowlist is DECLARED, never inferred from the clock, because a detector that
    // treats any unseen status as an outcome fires on the healthy path. The honest cost:
    // an outcome class added upstream and not added here UNDER-reports the gap. That
    // direction is chosen deliberately and is asserted rather than left implied.
    assert!(TICK_OUTCOME_STATUSES.contains(&"DISPATCH_RESULT_RECORDED"));
    assert!(
        !TICK_OUTCOME_STATUSES.contains(&"CYCLE_STARTED"),
        "a cycle that began owes no row yet"
    );
    let unlisted = vec![clock_row("SOME_FUTURE_OUTCOME", 1_767_999_999, 99)];
    assert_eq!(
        detect_gap(&[], &unlisted),
        None,
        "an unlisted status under-reports rather than manufacturing a gap; adding an \
         outcome upstream REQUIRES adding it to TICK_OUTCOME_STATUSES"
    );
}

#[test]
fn an_unmeasured_figure_is_null_and_never_zero() {
    // M4 CAUGHT A MISSING LEG. Replacing `dead: null` with `dead.unwrap_or(0)` left every
    // test green, so the doctrine in the builder's comment — "a zero here would be a
    // fabricated figure in the one artifact whose purpose is auditability" — was written
    // down and unenforced. It is enforced here now.
    let row = build_receipt(&mut Receipt {
            ts: 1,
            tick: 1,
            free_capacity: &[],
            attention: 0,
            dead: None,
            source: "src",
            dispositions: &[],
            fallback_reason: "none",
            claims: vec![json!({ "figure": "f", "command": "c" })],
            not_done: vec![],
        });
    assert_eq!(
        row["observed"]["dead"],
        Value::Null,
        "an unmeasured count must be null; 0 is a claim nobody measured"
    );
    assert!(
        !row["observed"]["dead"].is_number(),
        "and it must not be a number of any value"
    );

    // POSITIVE CONTROL: a caller that CAN measure it gets the number, so the null above is
    // about absence and not about the field being broken.
    let measured = build_receipt(&mut Receipt {
            ts: 1,
            tick: 1,
            free_capacity: &[],
            attention: 0,
            dead: Some(2),
            source: "src",
            dispositions: &[],
            fallback_reason: "none",
            claims: vec![json!({ "figure": "f", "command": "c" })],
            not_done: vec![],
        });
    assert_eq!(measured["observed"]["dead"], json!(2));
}

#[test]
fn a_clock_row_without_a_timestamp_contributes_nothing() {
    // NOTE, and it is a correction: an earlier version of this leg claimed to pin the
    // explicit `let Some(ts) … else continue`. It does not. A mutation replacing that with
    // `unwrap_or(0)` left this GREEN, because `0 <= floor` holds for every floor and the
    // row is skipped by the floor comparison either way. The mutation is behaviourally
    // INERT, so this leg asserts the OUTCOME it can actually observe.
    let clock = vec![
        json!({ "status": "DISPATCH_CLAIMED", "tick": 6 }),
        json!({ "ts_unix": 1_767_400_200u64, "tick": 6 }),
        clock_row("DISPATCH_CLAIMED", 1_767_400_300, 6),
    ];
    let gap = detect_gap(&[], &clock).expect("the one complete row still witnesses");
    assert_eq!(gap.outcomes_since_last_row, 1);
    assert_eq!(gap.newest_ts, 1_767_400_300);
}

#[test]
fn a_tick_that_errored_before_observing_is_covered_by_the_gap_not_by_a_row() {
    // MEASURED TONIGHT, on a real run: a tick ended `SUPERVISOR_REFUSED` from the outer
    // driver — an `Err` raised before `parse_observation`, so `observation` does not exist
    // at that site. The writer sits after `decide()` and never ran; the ledger stayed at 7
    // rows and `--heartbeat` reported
    // `TICK_ROWS_MISSING outcomes_since_last_row=1 newest_outcome=SUPERVISOR_REFUSED`.
    //
    // THIS IS THE DIVISION OF LABOUR, NOT A HOLE IN IT. A row for such a tick would have to
    // invent an `observed` block, and a fabricated observation in the one artifact whose
    // purpose is auditability is worse than an absent row that something else names. So:
    // the writer covers every tick that reached a decision, and the gap detector covers the
    // rest. The boundary is asserted here rather than left implied.
    let clock = vec![clock_row("SUPERVISOR_REFUSED", 1_767_500_000, 1)];
    let gap = detect_gap(&[a_real_row(1_767_499_000, 1)], &clock)
        .expect("an errored tick MUST still be nameable");
    assert_eq!(gap.newest_status, "SUPERVISOR_REFUSED");
    assert_eq!(gap.outcomes_since_last_row, 1);
    assert!(
        TICK_OUTCOME_STATUSES.contains(&"SUPERVISOR_REFUSED"),
        "the pre-observation refusal class must be in the allowlist or nothing covers it"
    );
}

// -----------------------------------------------------------------------------------
// The BINARY, end to end, on emitted text
// -----------------------------------------------------------------------------------

fn run(args: &[&str]) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_orchestration-tick-gate"))
        .args(args)
        .output()
        .expect("spawn the gate");
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    (output.status.code().unwrap_or(-1), text)
}

#[test]
fn the_binary_reports_a_gap_with_its_own_exit_code() {
    let dir = temp_dir("binary-gap");
    let ledger = dir.join("orchestration-ticks.jsonl");
    let clock = dir.join("heartbeat.jsonl");
    std::fs::write(
        &clock,
        format!(
            "{}\n{}\n",
            clock_row("CYCLE_STARTED", 1_767_400_100, 6),
            clock_row("DISPATCH_RESULT_RECORDED", 1_767_400_400, 8)
        ),
    )
    .expect("write clock");

    // The ledger does not exist. Previously: exit 2, NOTHING_TO_CHECK, no statement about
    // whether rows were owed.
    let (code, text) = run(&[
        "--ledger",
        ledger.to_str().expect("utf8"),
        "--heartbeat",
        clock.to_str().expect("utf8"),
    ]);
    assert_eq!(
        code, 3,
        "a gap gets its OWN code, not the validator's 1 or 2: {text}"
    );
    assert!(text.contains("status=GAP"), "{text}");
    assert!(text.contains("TICK_ROWS_MISSING"), "{text}");
    assert!(text.contains("last_row_ts=NONE"), "{text}");

    // Now the writer runs. Same clock, same command, no gap — and the validator agrees.
    append_receipt(&ledger, &a_real_row(1_767_400_500, 8)).expect("append");
    let (code, text) = run(&[
        "--ledger",
        ledger.to_str().expect("utf8"),
        "--heartbeat",
        clock.to_str().expect("utf8"),
    ]);
    assert_eq!(code, 0, "a written row closes the gap: {text}");
    assert!(text.contains("status=PRESENT"), "{text}");
    assert!(
        text.contains("status=CLEAN"),
        "the row must also VALIDATE: {text}"
    );
}

#[test]
fn an_unreadable_or_empty_clock_refuses_rather_than_reporting_no_gap() {
    // ANTI-VACUITY. Without a clock the presence check has no anchor, and answering "no
    // gap" would be the exact silence this bead removes — a check that cannot run must not
    // report a pass. Measured cost of the alternative: an absent ledger reading healthy.
    let dir = temp_dir("no-clock");
    let ledger = dir.join("orchestration-ticks.jsonl");
    append_receipt(&ledger, &a_real_row(1_767_400_500, 8)).expect("append");

    let (code, text) = run(&[
        "--ledger",
        ledger.to_str().expect("utf8"),
        "--heartbeat",
        dir.join("absent.jsonl").to_str().expect("utf8"),
    ]);
    assert_eq!(code, 2, "an unreadable clock is an ERROR: {text}");
    assert!(text.contains("CLOCK_UNREADABLE"), "{text}");
    assert!(text.contains("MUST NOT report absence of a gap"), "{text}");

    let empty = dir.join("empty.jsonl");
    std::fs::write(&empty, "").expect("write empty");
    let (code, text) = run(&[
        "--ledger",
        ledger.to_str().expect("utf8"),
        "--heartbeat",
        empty.to_str().expect("utf8"),
    ]);
    assert_eq!(code, 2, "an empty clock is an ERROR: {text}");
    assert!(text.contains("CLOCK_EMPTY"), "{text}");

    // POSITIVE CONTROL: without --heartbeat the binary still behaves as it always did, so
    // the two modes are separable and the presence check is additive.
    let (code, text) = run(&["--ledger", ledger.to_str().expect("utf8")]);
    assert_eq!(code, 0, "{text}");
    assert!(
        text.contains("status=CLEAN") && !text.contains("status=PRESENT"),
        "{text}"
    );
}

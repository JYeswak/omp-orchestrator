#![forbid(unsafe_code)]

//! One leg per payload trap, each firing on the WRONG implementation.
//!
//! Every trap leg is two-sided: it pins the naive reading as a PASSING
//! assertion (so the defect is reproduced, not described) and then proves the
//! kernel refuses it. A leg that only asserted the kernel's answer could not
//! tell a live function from a literal.

use ntm_kernel::{
    classify, motion, pane_denominator, presence, Motion, NtmCall, NtmOutcome, NtmVerb,
    PaneSnapshot, Presence,
};
use serde_json::json;

// ---------------------------------------------------------------------------
// argv: ONE spelling of the flag grammar
// ---------------------------------------------------------------------------

#[test]
fn argv_carries_the_verb_session_and_panes_in_one_spelling() {
    let call = NtmCall::on_session(NtmVerb::Dialogs, "omp-orchestrator").panes("%17");
    assert_eq!(
        call.argv(),
        vec!["--robot-dialogs=omp-orchestrator", "--panes=%17"]
    );
    let health = NtmCall::on_session(NtmVerb::AgentHealth, "sess").arg("--no-caut");
    assert_eq!(
        health.argv(),
        vec!["--robot-agent-health=sess", "--no-caut"]
    );
    let assign = NtmCall::bare(NtmVerb::Assign).args(["--cass-context", "/repo"]);
    assert_eq!(assign.argv(), vec!["--assign", "--cass-context", "/repo"]);
}

// ---------------------------------------------------------------------------
// TRAP 1 — a zeroed payload is not a miss: branch on EXIT first
// ---------------------------------------------------------------------------

#[test]
fn trap_one_zeroed_payload_on_a_failed_call_is_not_a_measurement() {
    // The live not-found payload: populated, zeroed, and plausible.
    let not_found = json!({
        "success": false,
        "error_code": "PANE_NOT_FOUND",
        "agent": {"process_running": false},
        "fleet_health": {"critical_count": 0, "total_panes": 0},
    })
    .to_string();

    // KNOWN-BAD, pinned as a passing assertion: a consumer that parses before
    // branching reads a confident false out of a call that FAILED.
    let naive: serde_json::Value = serde_json::from_str(&not_found).expect("payload parses");
    assert_eq!(
        naive["agent"]["process_running"],
        json!(false),
        "KNOWN-BAD: the failed call yields a readable, plausible field"
    );
    assert_eq!(naive["fleet_health"]["critical_count"], json!(0));

    // The kernel: exit status first, so there is NO field to read.
    let outcome = classify(false, Some(2), not_found.as_bytes(), b"pane not found");
    assert_eq!(outcome.state(), "REFUSED");
    assert!(
        outcome.payload().is_none(),
        "a refused call must expose no payload: {outcome:?}"
    );
    assert!(!outcome.is_answered());
}

#[test]
fn trap_one_a_successful_call_still_refuses_a_payload_that_says_it_failed() {
    // Exit 0 with success:false is a THIRD state, not an answer and not a
    // process refusal.
    let outcome = classify(true, Some(0), br#"{"success":false}"#, b"");
    assert_eq!(outcome.state(), "UNPARSABLE");
    assert!(outcome.payload().is_none());
    // And the honest good case still answers.
    let answered = classify(true, Some(0), br#"{"success":true,"panes":{}}"#, b"");
    assert_eq!(answered.state(), "ANSWERED");
    assert!(answered.payload().is_some(), "known-good leg");
}

// ---------------------------------------------------------------------------
// TRAP 2 — one condition, two error_code spellings
// ---------------------------------------------------------------------------

#[test]
fn trap_two_two_error_code_spellings_for_one_condition_classify_identically() {
    // Measured against the live surface: agent-health/interrupt say
    // PANE_NOT_FOUND, dialogs/answer-dialog say INVALID_FLAG, for the SAME
    // missing pane.
    let health = br#"{"success":false,"error_code":"PANE_NOT_FOUND"}"#;
    let dialogs = br#"{"success":false,"error_code":"INVALID_FLAG"}"#;

    // KNOWN-BAD, pinned: a consumer keyed on the spelling sees two different
    // worlds for one condition.
    let code = |raw: &[u8]| -> String {
        serde_json::from_slice::<serde_json::Value>(raw).expect("parses")["error_code"]
            .as_str()
            .expect("string")
            .to_owned()
    };
    assert_ne!(
        code(health),
        code(dialogs),
        "KNOWN-BAD: the same condition carries two error_code spellings"
    );

    // The kernel keys on the exit status, so both are one state.
    let left = classify(false, Some(2), health, b"");
    let right = classify(false, Some(2), dialogs, b"");
    assert_eq!(left.state(), "REFUSED");
    assert_eq!(right.state(), "REFUSED");
    assert_eq!(left, right, "one condition must be one verdict");
}

// ---------------------------------------------------------------------------
// TRAP 3 — motion fields are not monotonic
// ---------------------------------------------------------------------------

fn snap(pane: &str, lines: u64, chars: u64, digest: &str) -> PaneSnapshot {
    PaneSnapshot {
        pane: pane.to_owned(),
        lines,
        chars,
        digest: digest.to_owned(),
    }
}

#[test]
fn trap_three_a_saturated_busy_pane_is_not_idle() {
    // A pane at scrollback saturation: lines FIXED, content churning.
    let first = snap("%17", 400, 9_000, "d1");
    let second = snap("%17", 400, 9_000_1, "d2");

    // KNOWN-BAD, pinned as a passing assertion: the grew-the-line-count test
    // calls the busiest pane in the fleet idle.
    assert!(
        !(second.lines > first.lines),
        "KNOWN-BAD: lines did not grow, so a monotonic reader reports IDLE"
    );

    // The kernel compares the captures.
    assert_eq!(motion(&first, &second), Motion::Moved);
    // Known-good: identical captures are genuinely still.
    assert_eq!(motion(&first, &first), Motion::Still);
}

#[test]
fn trap_three_an_uncomparable_pair_is_indeterminate_not_idle() {
    assert_eq!(
        motion(&snap("%17", 1, 1, "d"), &snap("%18", 1, 1, "d")),
        Motion::Indeterminate,
        "two different panes prove nothing about either"
    );
    assert_eq!(
        motion(&snap("%17", 1, 1, "d"), &snap("%17", 1, 1, "")),
        Motion::Indeterminate,
        "a capture we never got is unproven, not still"
    );
}

// ---------------------------------------------------------------------------
// TRAP 4 — total_panes is the AGENT count
// ---------------------------------------------------------------------------

#[test]
fn trap_four_total_panes_is_not_the_pane_denominator() {
    // Measured 2026-09-11: 8 tmux panes, total_panes 7 — the non-agent pane is
    // missing from the count but present in the map.
    let payload = json!({
        "success": true,
        "total_panes": 7,
        "panes": {
            "1": {}, "2": {}, "3": {}, "4": {}, "5": {}, "6": {}, "7": {}, "8": {}
        },
    });

    // KNOWN-BAD, pinned: the field-derived denominator silently drops a pane.
    assert_eq!(
        payload["total_panes"].as_u64(),
        Some(7),
        "KNOWN-BAD: total_panes under-counts the panes present in the same payload"
    );

    assert_eq!(
        pane_denominator(&payload).expect("pane map present"),
        8,
        "the denominator is the pane MAP"
    );
    // An absent map is an ERROR, never zero.
    assert_eq!(
        pane_denominator(&json!({"success": true, "total_panes": 7})),
        Err("NTM_NO_PANE_MAP"),
        "cannot-see-the-panes must not read as no-panes"
    );
}

// ---------------------------------------------------------------------------
// TRAP 5 — critical_count inverts at the boundary
// ---------------------------------------------------------------------------

#[test]
fn trap_five_critical_count_zero_is_not_health_for_a_missing_pane() {
    let payload = json!({
        "success": true,
        "panes": {
            "%7": {"critical_count": 1},
            "%8": {"critical_count": 0},
        },
    });

    // KNOWN-BAD, pinned: the absent pane and the healthy pane produce the SAME
    // zero, so a count-gated consumer calls the unreachable pane healthiest.
    let count_of = |pane: &str| -> u64 {
        payload["panes"][pane]["critical_count"]
            .as_u64()
            .unwrap_or(0)
    };
    assert_eq!(
        count_of("%8"),
        count_of("%404"),
        "KNOWN-BAD: a healthy pane and a NONEXISTENT pane both read 0"
    );

    assert_eq!(presence(&payload, "%7"), Presence::Critical);
    assert_eq!(presence(&payload, "%8"), Presence::Healthy);
    assert_eq!(
        presence(&payload, "%404"),
        Presence::Absent,
        "presence comes from the MAP, never from the count"
    );
}

// ---------------------------------------------------------------------------
// The spawn path refuses rather than inventing an answer
// ---------------------------------------------------------------------------

#[test]
fn an_unspawnable_binary_is_unanswerable_never_an_empty_answer() {
    // `ntm` is absent on CI lanes, which is exactly the condition a consumer
    // must not read as "no dialogs" or "no rate limits".
    let outcome = ntm_kernel::invoke_bounded(
        &NtmCall::on_session(NtmVerb::Dialogs, "no-such-session").panes("%0"),
        std::time::Duration::from_secs(5),
    );
    match &outcome {
        NtmOutcome::Unanswerable { reason_code, .. } => {
            assert!(
                *reason_code == "NTM_UNAVAILABLE" || *reason_code == "NTM_TIMEOUT",
                "unexpected reason: {reason_code}"
            );
        }
        // On a host WITH ntm installed the call legitimately refuses or
        // answers; what must never happen is a payload appearing out of a
        // failed call.
        NtmOutcome::Refused { .. } | NtmOutcome::Unparsable { .. } => {
            assert!(outcome.payload().is_none());
        }
        NtmOutcome::Answered { .. } => {
            assert!(outcome.payload().is_some());
        }
    }
}

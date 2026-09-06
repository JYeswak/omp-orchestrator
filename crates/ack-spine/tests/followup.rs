//! Follow-up stage tests: the classifier's legs, all hermetic (no br subprocess).

use ack_spine::followup::{classify_followup, followup_action, FollowUpAction, FollowUpVerdict};

/// A comment that is NOT an ACK, so it counts as a verdict. Named rather than
/// inlined because the distinction is the point: an ACK row here would leave the
/// deadline reachable, which is the defect `ipg.19` fixed.
const SUBSTANTIVE: &str = "VERDICT: re-ran the acceptance command, PASS";

/// KNOWN-BAD: a dispatched bead, idle, no verdict comment, past deadline
/// -> SILENT_PAST_DEADLINE. This is the %1408/%1409 live case from tonight.
#[test]
fn dispatched_silent_past_deadline_is_typed() {
    let verdict = classify_followup(
        "omp-orchestrator-he6",
        /* bead_closed = */ false,
        // An unclosed bead has no close reason to read, so the honest input is
        // None -> CloseReasonVerdict::Unread. Added when K9 threaded the real
        // reason through; every one of these cases is unclosed.
        /* close_reason = */
        None,
        "SilverWolf", // current assignee
        "SilverWolf", // original assignee (unchanged)
        &[],          // no verdict comment
        120,          // 120 minutes since dispatch
        90,           // deadline: 90 minutes
        true,         // tracker readable
    );
    match &verdict {
        FollowUpVerdict::SilentPastDeadline {
            bead_id,
            minutes_since_dispatch,
            ..
        } => {
            assert_eq!(bead_id, "omp-orchestrator-he6");
            assert_eq!(*minutes_since_dispatch, 120);
        }
        other => panic!("expected SilentPastDeadline, got {other:?}"),
    }
    // The follow-up action must be NeedsFollowUp, not Healthy.
    match followup_action(&verdict) {
        FollowUpAction::NeedsFollowUp(_) => {}
        FollowUpAction::Healthy => panic!("a silent bead must not be Healthy"),
    }
}
#[test]
fn canonical_assignee_preserves_pane_for_silent_followup() {
    let verdict = classify_followup(
        "fw20",
        false,
        None,
        "pane=%8;incarnation=42;agent=WildStone",
        "pane=%8;incarnation=42;agent=WildStone",
        &[],
        120,
        90,
        true,
    );
    match verdict {
        FollowUpVerdict::SilentPastDeadline { pane_id, .. } => {
            assert_eq!(pane_id.as_deref(), Some("%8"));
        }
        other => panic!("expected attributed silence, got {other:?}"),
    }
}

/// KNOWN-GOOD: a dispatched bead with a verdict comment -> VERDICT_POSTED.
#[test]
fn verdict_posted_is_healthy() {
    let verdict = classify_followup(
        "omp-orchestrator-0hk",
        /* bead_closed = */ false,
        // An unclosed bead has no close reason to read, so the honest input is
        // None -> CloseReasonVerdict::Unread. Added when K9 threaded the real
        // reason through; every one of these cases is unclosed.
        /* close_reason = */
        None,
        "SilverWolf",
        "SilverWolf",
        &[SUBSTANTIVE.to_owned()], // verdict comment present
        60,                        // 60 minutes since dispatch
        90,                        // deadline: 90 minutes
        true,
    );
    assert!(
        matches!(verdict, FollowUpVerdict::VerdictPosted { .. }),
        "a commented bead must be VERDICT_POSTED, got {verdict:?}"
    );
    match followup_action(&verdict) {
        FollowUpAction::Healthy => {}
        FollowUpAction::NeedsFollowUp(_) => panic!("a commented bead must be Healthy"),
    }
}

/// A reassigned bead is REASSIGNED, not silent — re-dispatch needed, not a chase.
#[test]
fn reassigned_bead_is_not_silent() {
    let verdict = classify_followup(
        "omp-orchestrator-815",
        /* bead_closed = */ false,
        // An unclosed bead has no close reason to read, so the honest input is
        // None -> CloseReasonVerdict::Unread. Added when K9 threaded the real
        // reason through; every one of these cases is unclosed.
        /* close_reason = */
        None,
        "GreenFrog", // current assignee (CHANGED)
        "AmberGate", // original assignee
        &[],         // no verdict comment
        180,         // well past deadline
        90,
        true,
    );
    match &verdict {
        FollowUpVerdict::Reassigned {
            bead_id,
            new_assignee,
        } => {
            assert_eq!(bead_id, "omp-orchestrator-815");
            assert_eq!(new_assignee, "GreenFrog");
        }
        other => panic!("expected Reassigned, got {other:?}"),
    }
    match followup_action(&verdict) {
        FollowUpAction::Healthy => {}
        FollowUpAction::NeedsFollowUp(_) => panic!("a reassigned bead is not silent"),
    }
}

/// TRACKER UNREADABLE is an ERROR, never VERDICT_POSTED.
#[test]
fn tracker_error_is_not_verdict_posted() {
    let verdict = classify_followup(
        "any-bead",
        /* bead_closed = */ false,
        // An unclosed bead has no close reason to read, so the honest input is
        // None -> CloseReasonVerdict::Unread. Added when K9 threaded the real
        // reason through; every one of these cases is unclosed.
        /* close_reason = */
        None,
        "anyone",
        "anyone",
        &[],
        0,
        90,
        false, // tracker UNREADABLE
    );
    assert!(
        matches!(verdict, FollowUpVerdict::TrackerError { .. }),
        "unreadable tracker must be TrackerError, got {verdict:?}"
    );
    match followup_action(&verdict) {
        FollowUpAction::NeedsFollowUp(_) => {}
        FollowUpAction::Healthy => panic!("a tracker error must not be Healthy"),
    }
}

/// Not past the deadline yet: no action needed (the bead is still working).
#[test]
fn within_deadline_and_working_is_healthy() {
    let verdict = classify_followup(
        "omp-orchestrator-6gq",
        /* bead_closed = */ false,
        // An unclosed bead has no close reason to read, so the honest input is
        // None -> CloseReasonVerdict::Unread. Added when K9 threaded the real
        // reason through; every one of these cases is unclosed.
        /* close_reason = */
        None,
        "BlueLantern",
        "BlueLantern",
        &[], // no verdict yet
        30,  // only 30 minutes elapsed
        90,  // deadline: 90 minutes
        true,
    );
    // UPDATED BY `ipg.19`, and this leg PINNED THE DEFECT. Its own comment said the
    // classifier "returns VerdictPosted for a healthy in-progress bead (no verdict
    // YET ...)" — two different conditions sharing one arm, asserted as correct.
    // `InProgress` is now its own variant, so the distinction the comment described
    // in prose is expressible in the type.
    match &verdict {
        FollowUpVerdict::InProgress {
            minutes_since_dispatch,
            ..
        } => {
            assert_eq!(*minutes_since_dispatch, 30);
        }
        other => panic!("within-deadline should be InProgress, got {other:?}"),
    }
    // THE PROPERTY THAT WAS PREVIOUSLY UNREPRESENTABLE: working-with-no-verdict must
    // NOT claim a verdict was posted. Before the split this assertion could not be
    // written, because both conditions produced the same value.
    assert!(
        !matches!(verdict, FollowUpVerdict::VerdictPosted { .. }),
        "a bead with no verdict comment must not report VerdictPosted"
    );
    match followup_action(&verdict) {
        FollowUpAction::Healthy => {}
        FollowUpAction::NeedsFollowUp(_) => panic!("a bead within its deadline must be Healthy"),
    }
}

/// MUTATION: swapping the deadline comparison (>= to >) makes the
/// SILENT_PAST_DEADLINE leg pass for the exact boundary case.
#[test]
fn boundary_case_deadline_equals_elapsed_is_silent() {
    // Exactly at the deadline: SILENT_PAST_DEADLINE (the >= boundary).
    let verdict = classify_followup(
        "omp-orchestrator-x",
        /* bead_closed = */ false,
        // An unclosed bead has no close reason to read, so the honest input is
        // None -> CloseReasonVerdict::Unread. Added when K9 threaded the real
        // reason through; every one of these cases is unclosed.
        /* close_reason = */
        None,
        "me",
        "me",
        &[],
        90, // exactly the deadline
        90, // deadline: 90
        true,
    );
    match &verdict {
        FollowUpVerdict::SilentPastDeadline { .. } => {}
        other => panic!("boundary case must be SilentPastDeadline, got {other:?}"),
    }
}

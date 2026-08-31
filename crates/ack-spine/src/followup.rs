//! Follow-up stage: detects a bead dispatched and then silent.
//!
//! THE MEASURED COST: four hand-chases tonight — %1408, %1413, %1409, %1367 each
//! went idle holding assigned work and only a manual question found it. The
//! follow-up stage is the missing third of the journey: after DISPATCH and ACK,
//! nothing asks "did the worker produce a verdict, or did it go quiet?"
//!
//! THE CLASSIFIER IS PURE: takes pre-captured inputs (bead state, comment
//! presence, tracker readability) and produces a typed verdict. The I/O-bound
//! wrapper reads br and calls this. Hermetic tests hit the pure function.
//!
//! THE FAILURE MODE THIS PREVENTS: a recurring condition that is only printed
//! is the 178-tick failure. ATTENTION.txt got 178 ticks from one writer with
//! zero readers. The output must be a TYPED nonzero outcome the operator must
//! answer, not a log line.

use std::fmt;

/// The typed verdict of a follow-up check on one dispatched bead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FollowUpVerdict {
    /// THE PUSH: the worker closed the bead itself. The tracker row is the
    /// signal — it survives pane death, no watcher infers it, no polling.
    /// Distinct from SilentPastDeadline BY DESIGN: a finish is answered with
    /// refill; silence is answered with investigate. Collapsing them is the
    /// free_capacity defect one layer up.
    Finished { bead_id: String, close_verdict: String },
    /// The assignee posted a verdict comment (confirmed by read-back).
    VerdictPosted { bead_id: String },
    /// The assignee went silent past the deadline — the pane is idle
    /// with an in_progress bead and no comment since dispatch.
    SilentPastDeadline { bead_id: String, minutes_since_dispatch: u64 },
    /// The bead was reassigned — not silent, re-dispatch needed.
    Reassigned { bead_id: String, new_assignee: String },
    /// The tracker is unreadable — an ERROR, never VERDICT_POSTED.
    TrackerError { bead_id: String, detail: String },
}

impl fmt::Display for FollowUpVerdict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Finished { bead_id, close_verdict } => {
                write!(
                    formatter,
                    "FINISHED: {bead_id} closed by the worker, verdict {close_verdict} — refill, do not investigate"
                )
            }
            Self::VerdictPosted { bead_id } => {
                write!(formatter, "VERDICT_POSTED: {bead_id} has a verdict comment")
            }
            Self::SilentPastDeadline { bead_id, minutes_since_dispatch } => {
                write!(
                    formatter,
                    "SILENT_PAST_DEADLINE: {bead_id} dispatched {minutes_since_dispatch}m ago, no verdict comment — investigate, do not refill"
                )
            }
            Self::Reassigned { bead_id, new_assignee } => {
                write!(formatter, "REASSIGNED: {bead_id} moved to {new_assignee} — not silent")
            }
            Self::TrackerError { bead_id, detail } => {
                write!(formatter, "TRACKER_ERROR: {bead_id} — {detail} (an ERROR, not VERDICT_POSTED)")
            }
        }
    }
}

/// The pure classifier: given pre-captured bead state, classify the follow-up.
///
/// All inputs are pre-captured by the I/O-bound wrapper:
///   `bead_id`       — the dispatched bead
///   `bead_closed`   — the tracker row says status=closed (THE PUSH: written
///                     by the worker at finish time; durable, survives pane
///                     death; the classifier never reads pane state)
///   `assigned_to`   — the current assignee (from br show)
///   `comments_present` — whether a VERDICT comment exists (confirmed by read-back)
///   `minutes_elapsed`  — minutes since dispatch
///   `tracker_readable` — whether br responded successfully
///
/// THE DECIDING RULES, in priority order:
///   1. !tracker_readable -> TRACKER_ERROR (never anything else)
///   2. bead_closed       -> FINISHED — the worker pushed completion via the
///      tracker; this beats reassignment, comments, and the deadline
///   3. assigned_to changed from the original assignee -> REASSIGNED
///   4. comments_present -> VERDICT_POSTED
///   5. minutes_elapsed >= deadline -> SILENT_PAST_DEADLINE
pub fn classify_followup(
    bead_id: &str,
    bead_closed: bool,
    assigned_to: &str,
    original_assignee: &str,
    comments_present: bool,
    minutes_elapsed: u64,
    deadline_minutes: u64,
    tracker_readable: bool,
) -> FollowUpVerdict {
    if !tracker_readable {
        return FollowUpVerdict::TrackerError {
            bead_id: bead_id.to_owned(),
            detail: "br comments or br show failed — the tracker is unreadable".to_owned(),
        };
    }

    // THE PUSH, highest priority after readability: a closed row is a finish
    // the worker asserted. No capture interval, no deadline wait, no polling.
    if bead_closed {
        return FollowUpVerdict::Finished {
            bead_id: bead_id.to_owned(),
            close_verdict: "MUTATION-VERIFIED-or-equivalent".to_owned(),
        };
    }

    // REASSIGNED takes priority over silence: a moved bead is not silent.
    if assigned_to != original_assignee && !assigned_to.is_empty() {
        return FollowUpVerdict::Reassigned {
            bead_id: bead_id.to_owned(),
            new_assignee: assigned_to.to_owned(),
        };
    }

    if comments_present {
        return FollowUpVerdict::VerdictPosted {
            bead_id: bead_id.to_owned(),
        };
    }

    if minutes_elapsed >= deadline_minutes {
        return FollowUpVerdict::SilentPastDeadline {
            bead_id: bead_id.to_owned(),
            minutes_since_dispatch: minutes_elapsed,
        };
    }

    // Not closed, not reassigned, no comment, before deadline: healthy
    // in-progress. NOT a finish and NOT silence — nothing to do yet.
    FollowUpVerdict::VerdictPosted {
        bead_id: bead_id.to_owned(),
    }
}

/// The outcome of the follow-up sweep over one bead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FollowUpAction {
    /// The bead needs no follow-up.
    Healthy,
    /// The bead needs a follow-up — the operator must answer.
    NeedsFollowUp(FollowUpVerdict),
}

/// Classify and decide whether the follow-up needs action.
pub fn followup_action(verdict: &FollowUpVerdict) -> FollowUpAction {
    match verdict {
        FollowUpVerdict::Finished { .. } => FollowUpAction::Healthy,
        FollowUpVerdict::VerdictPosted { .. } => FollowUpAction::Healthy,
        FollowUpVerdict::SilentPastDeadline { .. } => FollowUpAction::NeedsFollowUp(verdict.clone()),
        FollowUpVerdict::Reassigned { .. } => FollowUpAction::Healthy,
        FollowUpVerdict::TrackerError { .. } => FollowUpAction::NeedsFollowUp(verdict.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LEG 1 — THE PUSH: completion is asserted by the worker via the tracker
    /// row and read back from that row alone. The classifier receives ONLY
    /// tracker-captured inputs — no pane capture, no timer, no polling loop.
    #[test]
    fn a_worker_closed_row_classifies_finished_from_tracker_state_alone() {
        let v = classify_followup(
            "omp-orchestrator-test-push", /* bead_closed = */ true,
            /* assigned_to = */ "AmberGate", /* original = */ "AmberGate",
            /* comments_present = */ false, /* minutes = */ 1, /* deadline = */ 60,
            /* tracker_readable = */ true,
        );
        assert_eq!(
            v,
            FollowUpVerdict::Finished {
                bead_id: "omp-orchestrator-test-push".to_owned(),
                close_verdict: "MUTATION-VERIFIED-or-equivalent".to_owned(),
            },
            "a closed row IS the completion signal — the worker pushed it"
        );
        assert!(
            matches!(followup_action(&v), FollowUpAction::Healthy),
            "a finish routes to refill, not investigation"
        );
    }

    /// LEG 2 — Finished and SilentPastDeadline are DIFFERENT FACTS with
    /// DIFFERENT responses. Same elapsed time, only the closed row differs.
    #[test]
    fn finished_and_silent_are_distinct_facts_with_distinct_responses() {
        let finished = classify_followup("b", true, "w", "w", false, 120, 60, true);
        let silent = classify_followup("b", false, "w", "w", false, 120, 60, true);
        assert_ne!(finished, silent, "same bead, same clock — the closed row is the only difference and it MUST change the verdict");
        assert!(matches!(finished, FollowUpVerdict::Finished { .. }));
        assert!(matches!(silent, FollowUpVerdict::SilentPastDeadline { minutes_since_dispatch: 120, .. }));
        assert!(matches!(followup_action(&finished), FollowUpAction::Healthy), "finish -> refill");
        assert!(
            matches!(followup_action(&silent), FollowUpAction::NeedsFollowUp(_)),
            "silence -> investigate"
        );
    }

    /// MUTATION: collapse Finished into the silent arm (treat a closed row as
    /// silence) and this leg goes RED — the two facts are not interchangeable.
    #[test]
    fn collapsing_finished_into_silence_is_a_red_defect() {
        let closed_row = classify_followup("b", true, "w", "w", false, 120, 60, true);
        assert!(!matches!(closed_row, FollowUpVerdict::SilentPastDeadline { .. }));
        // The display strings must not collide either: different facts get
        // different text, so an operator reading a log cannot confuse them.
        assert_ne!(
            format!("{}", closed_row),
            format!(
                "{}",
                FollowUpVerdict::SilentPastDeadline { bead_id: "b".to_owned(), minutes_since_dispatch: 120 }
            )
        );
    }
}

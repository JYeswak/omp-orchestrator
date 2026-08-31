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
            Self::VerdictPosted { bead_id } => {
                write!(formatter, "VERDICT_POSTED: {bead_id} has a verdict comment")
            }
            Self::SilentPastDeadline { bead_id, minutes_since_dispatch } => {
                write!(
                    formatter,
                    "SILENT_PAST_DEADLINE: {bead_id} dispatched {minutes_since_dispatch}m ago, no verdict comment"
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
///   `assigned_to`   — the current assignee (from br show)
///   `comments_present` — whether a VERDICT comment exists (confirmed by read-back)
///   `minutes_elapsed`  — minutes since dispatch
///   `tracker_readable` — whether br responded successfully
///
/// THE DECIDING RULES:
///   1. comments_present && tracker_readable -> VERDICT_POSTED
///   2. !comments_present && minutes_elapsed >= deadline && tracker_readable
///      -> SILENT_PAST_DEADLINE
///   3. assigned_to changed from the original assignee -> REASSIGNED
///   4. !tracker_readable -> TRACKER_ERROR (never VERDICT_POSTED)
pub fn classify_followup(
    bead_id: &str,
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

    // Not yet past the deadline, not silent yet, no verdict yet.
    // This is a healthy in-progress bead — no action needed.
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
        FollowUpVerdict::VerdictPosted { .. } => FollowUpAction::Healthy,
        FollowUpVerdict::SilentPastDeadline { .. } => FollowUpAction::NeedsFollowUp(verdict.clone()),
        FollowUpVerdict::Reassigned { .. } => FollowUpAction::Healthy,
        FollowUpVerdict::TrackerError { .. } => FollowUpAction::NeedsFollowUp(verdict.clone()),
    }
}

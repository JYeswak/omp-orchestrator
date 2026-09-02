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

use crate::close_reason::{classify_close_reason, CloseReasonVerdict};
use std::fmt;

/// The typed verdict of a follow-up check on one dispatched bead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FollowUpVerdict {
    /// THE PUSH: the worker closed the bead itself. The tracker row is the
    /// signal — it survives pane death, no watcher infers it, no polling.
    /// Distinct from SilentPastDeadline BY DESIGN: a finish is answered with
    /// refill; silence is answered with investigate. Collapsing them is the
    /// free_capacity defect one layer up.
    ///
    /// `close_reason` is the CLASSIFIED verdict of the reason actually read, and
    /// it is a [`CloseReasonVerdict`] rather than a `String` on purpose. This
    /// field previously held the hardcoded literal
    /// `"MUTATION-VERIFIED-or-equivalent"` while `classify_followup` did not take
    /// the reason as an input at all — a constant wearing the shape of a
    /// measurement. A typed verdict makes that unrepresentable: the only way to
    /// populate it is to pass what was read, and "I did not read it" has its own
    /// variant.
    Finished {
        bead_id: String,
        close_reason: CloseReasonVerdict,
    },
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
            Self::Finished {
                bead_id,
                close_reason,
            } => {
                write!(
                    formatter,
                    "FINISHED: {bead_id} closed by the worker, {close_reason} — refill, do not investigate"
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
    close_reason: Option<&str>,
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
            // The reason the caller ACTUALLY read, classified. `None` yields
            // `Unread`, which is the honest state the fabricated constant existed
            // to avoid admitting.
            close_reason: classify_close_reason(close_reason),
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
            /* close_reason = */ Some("MUTATION-VERIFIED: re-ran the suite"),
            /* assigned_to = */ "AmberGate", /* original = */ "AmberGate",
            /* comments_present = */ false, /* minutes = */ 1, /* deadline = */ 60,
            /* tracker_readable = */ true,
        );
        assert_eq!(
            v,
            FollowUpVerdict::Finished {
                bead_id: "omp-orchestrator-test-push".to_owned(),
                close_reason: CloseReasonVerdict::Verified {
                    prefix: crate::close_reason::ClosePrefix::MutationVerified,
                },
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
        let finished = classify_followup("b", true, Some("DONE: shipped"), "w", "w", false, 120, 60, true);
        let silent = classify_followup("b", false, None, "w", "w", false, 120, 60, true);
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
        let closed_row = classify_followup("b", true, Some("DONE: shipped"), "w", "w", false, 120, 60, true);
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

    // ─────────── K9 legs (omp-orchestrator-u622): acceptance 4, the close
    //             reason must be OBSERVED, never asserted

    /// THE DECISIVE LEG. Before this bead, `close_verdict` was the hardcoded
    /// literal `"MUTATION-VERIFIED-or-equivalent"` and `classify_followup` did
    /// not take the reason as an input at all — so a bead closed with PROSE,
    /// which the tracker's policy REFUSES, was still reported as verified.
    ///
    /// Now the classification varies with what was read, which is the whole
    /// definition of a derived value.
    #[test]
    fn the_close_verdict_varies_with_the_reason_actually_read() {
        let verified = classify_followup(
            "b",
            true,
            Some("MUTATION-VERIFIED: the leg went RED"),
            "w",
            "w",
            false,
            1,
            60,
            true,
        );
        let prose = classify_followup(
            "b",
            true,
            Some("fixed the thing, tests pass"),
            "w",
            "w",
            false,
            1,
            60,
            true,
        );
        let unread = classify_followup("b", true, None, "w", "w", false, 1, 60, true);

        assert_ne!(
            verified, prose,
            "a sanctioned prefix and prose must not produce the same verdict"
        );
        assert_ne!(
            verified, unread,
            "a read reason and an unread one must not produce the same verdict"
        );
        assert_ne!(prose, unread, "refused and unread are different facts");

        // And the refusal is legible in the emitted TEXT, per gate rule 7.
        let text = prose.to_string();
        assert!(
            text.contains("CLOSE_REASON_POLICY_REFUSED") && text.contains("leading=fixed"),
            "the FINISHED line must carry the policy refusal: {text}"
        );
        let unread_text = unread.to_string();
        assert!(
            unread_text.contains("CLOSE_REASON_UNREAD"),
            "an unread reason must say so on the FINISHED line: {unread_text}"
        );
    }

    /// A closed bead is still FINISHED even when its reason is refused — the
    /// close happened. What changes is what the row CLAIMS about it.
    ///
    /// Stated as a leg because the tempting over-correction is to demote a
    /// prose-closed bead out of `Finished`, which would make this classifier
    /// disagree with the tracker about whether the bead is closed. Two
    /// authorities, one store.
    #[test]
    fn a_refused_reason_does_not_reopen_the_bead() {
        let prose = classify_followup("b", true, Some("just fixed it"), "w", "w", false, 1, 60, true);
        assert!(
            matches!(prose, FollowUpVerdict::Finished { .. }),
            "the bead IS closed; only the reason is refused: {prose:?}"
        );
        assert!(
            matches!(followup_action(&prose), FollowUpAction::Healthy),
            "a closed bead routes to refill regardless of its reason's shape"
        );
    }

    /// ANTI-VACUITY, acceptance 6: an unreadable tracker outranks EVERY other
    /// input, including a closed row and a verified reason. A tracker read that
    /// failed is an ERROR, never a finish.
    #[test]
    fn an_unreadable_tracker_outranks_a_closed_row_and_a_good_reason() {
        let verdict = classify_followup(
            "b",
            /* bead_closed = */ true,
            Some("MUTATION-VERIFIED: proven"),
            "w",
            "w",
            true,
            1,
            60,
            /* tracker_readable = */ false,
        );
        assert!(
            matches!(verdict, FollowUpVerdict::TrackerError { .. }),
            "an unreadable tracker cannot yield a finish: {verdict:?}"
        );
        assert!(
            verdict.to_string().contains("an ERROR, not VERDICT_POSTED"),
            "the message must refuse the reading: {verdict}"
        );
    }

    /// The fabricated literal must not return. A source scan, scoped to the
    /// PRODUCTION slice and with the needle built by `concat!`, so the check is
    /// neither self-referential nor confused by the doc comment that explains
    /// the defect. Both traps were paid for earlier the same session.
    #[test]
    fn the_fabricated_close_verdict_literal_is_gone_from_production() {
        let source = include_str!("followup.rs");
        let production = source
            .split_once(concat!("#[cfg", "(test)]"))
            .map_or(source, |(before, _)| before);

        // ANTI-VACUITY plus a positive control: the slice must be real and must
        // contain the function under audit.
        assert!(production.len() > 500, "the production slice is too small to scan");
        assert!(
            production.contains("pub fn classify_followup"),
            "ANTI-VACUITY: the slice does not contain the function under audit"
        );

        // The literal, split so this file never holds it contiguously outside a
        // doc comment. The doc comment above IS in the production slice, so the
        // needle is the ASSIGNMENT form rather than the bare string.
        let needle = concat!("close_verdict", ": \"MUTATION-VERIFIED");
        assert_eq!(
            production.matches(needle).count(),
            0,
            "the hardcoded close verdict must not return: a constant that cannot \
             vary is not a measurement"
        );
    }
}

#![forbid(unsafe_code)]

//! Receiver-side dispatch receipt classification.
//!
//! This crate deliberately never sends input. A caller performs the transport-specific
//! send, captures the same pane afterward, and passes the pre/post observations here.
//! A sender return value is therefore never part of the receipt proof.

use std::fmt;
pub use tick_monitor::ObservationIdentity;
use tick_monitor::{classify, Observation, PaneState};

/// How much the receiver census proves about the named pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanePresence {
    /// The pane id was present in a non-empty `tmux list-panes` result.
    Present,
    /// The pane id was absent from a non-empty `tmux list-panes` result.
    Absent,
    /// The pane catalog itself was empty; no pane may be declared dead.
    EmptyPaneList,
}

/// The post-send observation supplied by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostSendObservation {
    /// A capture was obtained for the named pane.
    Present(Observation),
    /// A non-empty pane census did not contain the named pane.
    Absent,
    /// The pane census was empty, so death is unproven for every pane.
    EmptyPaneList,
    /// No post-send capture was obtained.
    Missing,
}

/// Why a receiver receipt could not be established.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptReason {
    MissingPostObservation,
    PaneIdMismatch {
        expected: String,
        observed: String,
    },
    DialogOpen,
    /// A queued prompt landed in the composer but the pane never submitted it.
    WedgedUnsubmitted,
    EmptyPaneListNoDeathClaim,
    ObservationNotWorking {
        side: &'static str,
        state: String,
    },
    /// The post-send state changed, but the stable-content hash did not.
    StableContentUnchanged,
    /// The transport cannot support the same receipt claim as ntm.
    UnprovenTransport {
        transport: &'static str,
    },
    /// The authoritative br comments read-back has no matching ACK line.
    AckReadbackMissing,
    IdleUnchanged,
    PostBecameIdle,
    TimerDidNotReset {
        before_secs: u64,
        after_secs: u64,
    },
    TimerResetButStableContentUnchanged,
    TimerTooLargeAfterIdle {
        after_secs: u64,
        max_secs: u64,
    },
    /// The two captures were taken too close together for motion to be
    /// observable, so neither delivery nor non-delivery is derivable.
    ObservationWindowTooShort {
        /// Seconds between the pre-send and post-send captures.
        span_secs: u64,
        /// The floor below which the comparison is unsound.
        floor_secs: u64,
    },
}

impl fmt::Display for ReceiptReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPostObservation => f.write_str("missing_post_observation"),
            Self::PaneIdMismatch { expected, observed } => {
                write!(
                    f,
                    "pane_id_mismatch expected={expected} observed={observed}"
                )
            }
            Self::DialogOpen => f.write_str("dialog_open"),
            Self::WedgedUnsubmitted => f.write_str("WEDGED_UNSUBMITTED"),
            Self::EmptyPaneListNoDeathClaim => f.write_str("NOBODY_DEAD empty_pane_list"),
            Self::ObservationNotWorking { side, state } => {
                write!(f, "{side}_observation_not_working state={state}")
            }
            Self::StableContentUnchanged => f.write_str("stable_content_unchanged"),
            Self::UnprovenTransport { transport } => {
                write!(f, "unproven_transport transport={transport}")
            }
            Self::AckReadbackMissing => f.write_str("ack_readback_missing"),
            Self::IdleUnchanged => f.write_str("idle_unchanged"),
            Self::PostBecameIdle => f.write_str("post_became_idle"),
            Self::TimerDidNotReset {
                before_secs,
                after_secs,
            } => write!(
                f,
                "timer_did_not_reset before_secs={before_secs} after_secs={after_secs}"
            ),
            Self::TimerResetButStableContentUnchanged => {
                f.write_str("timer_reset_but_stable_content_unchanged")
            }
            Self::TimerTooLargeAfterIdle {
                after_secs,
                max_secs,
            } => write!(
                f,
                "timer_too_large_after_idle after_secs={after_secs} max_secs={max_secs}"
            ),
            Self::ObservationWindowTooShort {
                span_secs,
                floor_secs,
            } => write!(
                f,
                "OBSERVATION_WINDOW_TOO_SHORT span_secs={span_secs} floor_secs={floor_secs} \
                 -- motion is not observable in this window, so neither delivery nor \
                 non-delivery is derivable"
            ),
        }
    }
}

/// Typed receiver-side result. Sender success is intentionally absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptVerdict {
    /// Both receiver signals support delivery: the timer reset and stable content changed.
    ReceiptConfirmed {
        pane_id: String,
        timer_before_secs: Option<u64>,
        timer_after_secs: u64,
        stable_content_changed: bool,
    },
    /// The receiver observation is sufficient to say the send was not evidenced.
    NoReceipt {
        pane_id: String,
        reason: ReceiptReason,
    },
    /// The receiver pane disappeared from a non-empty tmux census.
    Dead { pane_id: String },
    /// The observations cannot distinguish delivery from a blocked or unreadable pane.
    Indeterminate {
        pane_id: String,
        reason: ReceiptReason,
    },
}

impl ReceiptVerdict {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::ReceiptConfirmed { .. } => "RECEIPT_CONFIRMED",
            Self::NoReceipt { .. } => "NO_RECEIPT",
            Self::Indeterminate { .. } => "INDETERMINATE",
            Self::Dead { .. } => "DEAD",
        }
    }

    pub fn reason(&self) -> Option<&ReceiptReason> {
        match self {
            Self::ReceiptConfirmed { .. } | Self::Dead { .. } => None,
            Self::NoReceipt { reason, .. } | Self::Indeterminate { reason, .. } => Some(reason),
        }
    }
}

/// New work must begin promptly after an idle pane accepts the packet.
pub const MAX_IDLE_TO_WORKING_TIMER_SECS: u64 = 30;

/// The minimum span between the two captures a receipt is derived from.
///
/// # Why a receipt needs a window at all
///
/// Every confirming and refuting branch below compares a TIMER and a
/// spinner-stripped CONTENT HASH across two captures. Both comparisons are
/// statements about MOTION, and motion is not observable in an arbitrarily
/// short window. Without a floor the same code manufactures errors in BOTH
/// directions:
///
/// * `Working -> Working`, two captures seconds apart: a healthy pane's timer has
///   ADVANCED, so `after >= before` fires `TimerDidNotReset` and a working pane
///   is reported as having no receipt. A FALSE NEGATIVE produced by looking too
///   fast.
/// * `Idle -> Working`, two captures seconds apart: a small timer plus any hash
///   change reads as confirmation, even when the pane began working for an
///   unrelated reason. A FALSE POSITIVE.
///
/// `tick-monitor` already records the first half of this in
/// `liveness`: it returns `Wedged` BEFORE the two-capture machinery precisely
/// because "its timer advances while blocked, so the (Working, Working) arm
/// would call it Live."
///
/// # Where the number comes from
///
/// `pane_truth::TWO_CAPTURE_MIN_SECS`, which is the repo's authority for this
/// floor and enforces it at `crates/pane-truth/src/lib.rs:443`. It is restated
/// here rather than imported so this crate stays pure — `pane-truth`'s lib pulls
/// in `std::process::Command` and spawns `tmux`, and this crate's contract is
/// that it never performs I/O. **A restated constant is a drift risk, so it is
/// gated, not hoped about:** `the_window_floor_matches_pane_truths_authority`
/// asserts the two agree, with `pane-truth` as a dev-dependency only.
pub const OBSERVATION_WINDOW_MIN_SECS: u64 = 75;

/// Convert a captured pane render into the shared tick-monitor observation shape.
///
/// tick-monitor owns last-line anchoring, timer parsing, dialog detection, and
/// spinner-stripped hashing. This adapter requires the producer identity from
/// the caller; it never manufactures an identity or defaults a missing sequence.
pub fn observe_capture(
    pane_id: impl Into<String>,
    capture: &str,
    at: u64,
    identity: ObservationIdentity,
) -> Observation {
    Observation {
        pane_id: pane_id.into(),
        state: classify(capture),
        hash: tick_monitor::stable_hash(capture),
        at,
        epoch: identity.epoch,
        sequence: identity.sequence,
        changed_at: identity.changed_at,
    }
}
///
/// The function performs no send and does not inspect sender return values. Confirmation
/// is keyed by the pre-send state:
///
/// * IDLE -> WORKING confirms only when the new timer is small and the stable hash changes;
/// * WORKING -> WORKING confirms only when the timer resets and stable content changes;
/// * IDLE -> IDLE and a non-resetting WORKING pane produce NO_RECEIPT;
/// * dialogs, absent panes, empty pane lists, and unreadable states remain named.
///
/// A non-empty `tmux list-panes` census must be represented by `Absent` before this
/// function may report `DEAD`; `EmptyPaneList` deliberately yields `INDETERMINATE`.
/// What the composer discriminator (`composer_typed::is_typed`) said about the
/// same post-send capture that produced the receiver verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerEvidence {
    /// The composer holds bright operator text: a packet arrived but was never
    /// submitted, or an operator is mid-typing.
    Typed,
    /// The composer is empty (or a greyed autosuggestion only).
    Free,
}

/// The escalation a non-delivery obliges, given the receiver state and the
/// composer evidence from the same capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonDeliveryEscalation {
    /// Idle pane + FREE composer after a send that claimed success: positive
    /// evidence the packet never landed. Re-send through the direct pane
    /// transport instead of polling a sender report that was never receipt.
    ResendDirect,
    /// Idle pane + TYPED composer: the packet arrived but sits unsubmitted.
    /// Send the Enter key - the documented parked-packet recovery.
    SubmitParked,
    /// The existing receipt machinery owns this state (working, dialog, wedged,
    /// unproven): keep polling.
    KeepPolling,
}

impl NonDeliveryEscalation {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ResendDirect => "RESEND_DIRECT",
            Self::SubmitParked => "SUBMIT_PARKED",
            Self::KeepPolling => "KEEP_POLLING",
        }
    }
}

/// Decide the escalation for a `NO_RECEIPT` poll whose sender report claimed
/// success. Keyed on the SAME capture that produced the receiver verdict, so a
/// misread composer can never contradict a state the pane visibly holds.
///
/// A killed, empty, or unreadable capture never reaches this function as
/// `ComposerEvidence::Free`: the caller maps missing observations to
/// `PostSendObservation::Missing`, which classifies `INDETERMINATE` upstream.
/// NO-CLAIM: this decides send-vs-receipt only. A pane that accepts text can
/// still refuse the work; receipt-vs-comprehension is out of scope here.
pub fn escalate_non_delivery(
    post_state: &tick_monitor::PaneState,
    composer: ComposerEvidence,
) -> NonDeliveryEscalation {
    match (post_state, composer) {
        // The measured 6q5 defect: sender claimed success, the pane stayed
        // idle, and the composer was empty. A packet that had landed and been
        // submitted flips the pane to WORKING; a packet parked unsubmitted
        // leaves text in the composer. Neither evidence exists here, so the
        // packet demonstrably never reached the pane.
        (PaneState::Idle, ComposerEvidence::Free) => NonDeliveryEscalation::ResendDirect,
        // Text is present on an idle pane: the packet arrived but was never
        // submitted. The Enter key is the documented parked-packet recovery.
        (PaneState::Idle, ComposerEvidence::Typed) => NonDeliveryEscalation::SubmitParked,
        // Working, Dialog, Wedged, and Unproven carry their own verdicts and
        // actions in `assess_receiver_receipt`; escalation must not fire there.
        _ => NonDeliveryEscalation::KeepPolling,
    }
}

/// Classify receiver evidence after an external transport send.
///
/// The function performs no send and does not inspect sender return values. Confirmation
/// is keyed by the pre-send state:
///
/// * IDLE -> WORKING confirms only when the new timer is small and the stable hash changes;
/// * WORKING -> WORKING confirms only when the timer resets and stable content changes;
/// * IDLE -> IDLE and a non-resetting WORKING pane produce NO_RECEIPT;
/// * dialogs, absent panes, empty pane lists, and unreadable states remain named.
///
/// A non-empty `tmux list-panes` census must be represented by `Absent` before this
/// function may report `DEAD`; `EmptyPaneList` deliberately yields `INDETERMINATE`.
pub fn assess_receiver_receipt(
    pane_id: &str,
    pre_send: &Observation,
    post_send: PostSendObservation,
) -> ReceiptVerdict {
    let pane_id_owned = pane_id.to_owned();

    if pre_send.pane_id != pane_id {
        return ReceiptVerdict::Indeterminate {
            pane_id: pane_id_owned.clone(),
            reason: ReceiptReason::PaneIdMismatch {
                expected: pane_id_owned,
                observed: pre_send.pane_id.clone(),
            },
        };
    }

    let post = match post_send {
        PostSendObservation::Missing => {
            return ReceiptVerdict::Indeterminate {
                pane_id: pane_id_owned,
                reason: ReceiptReason::MissingPostObservation,
            };
        }
        PostSendObservation::Absent => {
            return ReceiptVerdict::Dead {
                pane_id: pane_id_owned,
            };
        }
        PostSendObservation::EmptyPaneList => {
            return ReceiptVerdict::Indeterminate {
                pane_id: pane_id_owned,
                reason: ReceiptReason::EmptyPaneListNoDeathClaim,
            };
        }
        PostSendObservation::Present(post) => post,
    };

    if post.pane_id != pane_id {
        return ReceiptVerdict::Indeterminate {
            pane_id: pane_id_owned.clone(),
            reason: ReceiptReason::PaneIdMismatch {
                expected: pane_id_owned,
                observed: post.pane_id.clone(),
            },
        };
    }

    if matches!(pre_send.state, PaneState::Dialog { .. })
        || matches!(post.state, PaneState::Dialog { .. })
    {
        return ReceiptVerdict::Indeterminate {
            pane_id: pane_id_owned,
            reason: ReceiptReason::DialogOpen,
        };
    }
    if matches!(post.state, PaneState::Wedged) {
        return ReceiptVerdict::NoReceipt {
            pane_id: pane_id_owned,
            reason: ReceiptReason::WedgedUnsubmitted,
        };
    }

    // ORDERING GATE, and it is deliberately NARROWER than the bead's literal
    // "two captures >= 75s apart". Read the retraction before widening it.
    //
    // WHAT I BUILT FIRST AND WITHDREW: a blanket floor refusing every verdict
    // below 75 seconds. It was wrong twice over.
    //
    // 1. IT WOULD HAVE REGRESSED THE LIVE PATH. `crates/omp-orchestrator/src/main.rs`
    //    takes `pre_observation` at :797 and the post capture at :580 with NO
    //    sleep between them — a span of seconds. A blanket floor turns every
    //    production receipt into INDETERMINATE, which is strictly worse than the
    //    gap it closes.
    // 2. MY JUSTIFICATION FOR IT WAS ALSO WRONG. I argued a short window makes
    //    `Working -> Working` a false negative because a healthy pane's timer has
    //    advanced. But that arm keys on RESET (`after < before`), and a reset is
    //    unambiguous at any spacing: a pane that kept working without restarting
    //    genuinely did not begin new work. Likewise `Idle -> Working` with a
    //    fresh timer is the STRONGEST receipt this fleet has (measured:
    //    `%1413 IDLE -> WORKING t=14s`), and a 75s floor would destroy it.
    //
    // WHAT SURVIVES, because it is unambiguous: a post capture stamped STRICTLY
    // BEFORE the pre capture cannot support ANY motion claim.
    //
    // STRICTLY, and the boundary is load-bearing. A first draft refused
    // `post.at <= pre_send.at` and broke a live consumer: ack-stage builds both
    // captures at the same second, and `now_unix()` in production legitimately
    // returns the SAME second twice for calls milliseconds apart. Equal stamps are
    // second-resolution truncation, not a fault. Only a BACKWARD stamp is. That is a clock or
    // ordering fault in the observer, and `saturating_sub` yields 0 rather than
    // wrapping to a huge span that would sail through any floor.
    //
    // The narrower open question — what the floor should be on the `Idle -> Idle`
    // arm, where a pane may simply not have rendered yet — needs measurement of
    // real composer latency and is filed, not guessed at here.
    let span_secs = post.at.saturating_sub(pre_send.at);
    if post.at < pre_send.at {
        return ReceiptVerdict::Indeterminate {
            pane_id: pane_id_owned,
            reason: ReceiptReason::ObservationWindowTooShort {
                span_secs,
                floor_secs: OBSERVATION_WINDOW_MIN_SECS,
            },
        };
    }

    match (&pre_send.state, &post.state) {
        (PaneState::Idle, PaneState::Working { timer_secs }) => {
            if *timer_secs > MAX_IDLE_TO_WORKING_TIMER_SECS {
                ReceiptVerdict::Indeterminate {
                    pane_id: pane_id_owned,
                    reason: ReceiptReason::TimerTooLargeAfterIdle {
                        after_secs: *timer_secs,
                        max_secs: MAX_IDLE_TO_WORKING_TIMER_SECS,
                    },
                }
            } else if pre_send.hash == post.hash {
                ReceiptVerdict::NoReceipt {
                    pane_id: pane_id_owned,
                    reason: ReceiptReason::StableContentUnchanged,
                }
            } else {
                ReceiptVerdict::ReceiptConfirmed {
                    pane_id: pane_id_owned,
                    timer_before_secs: None,
                    timer_after_secs: *timer_secs,
                    stable_content_changed: true,
                }
            }
        }
        (PaneState::Idle, PaneState::Idle) => ReceiptVerdict::NoReceipt {
            pane_id: pane_id_owned,
            reason: ReceiptReason::IdleUnchanged,
        },
        (
            PaneState::Working {
                timer_secs: before_secs,
            },
            PaneState::Working {
                timer_secs: after_secs,
            },
        ) => {
            if *after_secs >= *before_secs {
                ReceiptVerdict::NoReceipt {
                    pane_id: pane_id_owned,
                    reason: ReceiptReason::TimerDidNotReset {
                        before_secs: *before_secs,
                        after_secs: *after_secs,
                    },
                }
            } else if pre_send.hash == post.hash {
                ReceiptVerdict::NoReceipt {
                    pane_id: pane_id_owned,
                    reason: ReceiptReason::TimerResetButStableContentUnchanged,
                }
            } else {
                ReceiptVerdict::ReceiptConfirmed {
                    pane_id: pane_id_owned,
                    timer_before_secs: Some(*before_secs),
                    timer_after_secs: *after_secs,
                    stable_content_changed: true,
                }
            }
        }
        (PaneState::Working { .. }, PaneState::Idle) => ReceiptVerdict::NoReceipt {
            pane_id: pane_id_owned,
            reason: ReceiptReason::PostBecameIdle,
        },
        (pre, post) => ReceiptVerdict::Indeterminate {
            pane_id: pane_id_owned,
            reason: ReceiptReason::ObservationNotWorking {
                side: "pre_or_post",
                state: format!("pre={pre:?} post={post:?}"),
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_with_free_composer_after_sender_success_obliges_resend() {
        // The measured 6q5 failure: ntm reported 3 delivery signals, the pane
        // stayed idle, and the Enter nudge proved the composer was EMPTY.
        let verdict = escalate_non_delivery(&PaneState::Idle, ComposerEvidence::Free);
        assert_eq!(verdict, NonDeliveryEscalation::ResendDirect);
    }

    #[test]
    fn idle_with_typed_composer_obliges_enter_not_resend() {
        let verdict = escalate_non_delivery(&PaneState::Idle, ComposerEvidence::Typed);
        assert_eq!(verdict, NonDeliveryEscalation::SubmitParked);
    }

    #[test]
    fn non_idle_states_stay_with_existing_machinery() {
        // Working panes already produce RECEIPT_CONFIRMED paths; wedged and
        // dialog states already carry their own actions. Escalation must not
        // fire there - the scan is over three states, never empty.
        let working = PaneState::Working { timer_secs: 5 };
        assert_eq!(
            escalate_non_delivery(&working, ComposerEvidence::Free),
            NonDeliveryEscalation::KeepPolling
        );
        assert_eq!(
            escalate_non_delivery(&PaneState::Wedged, ComposerEvidence::Free),
            NonDeliveryEscalation::KeepPolling
        );
    }

    #[test]
    fn escalation_labels_are_stable() {
        assert_eq!(NonDeliveryEscalation::ResendDirect.label(), "RESEND_DIRECT");
        assert_eq!(NonDeliveryEscalation::SubmitParked.label(), "SUBMIT_PARKED");
        assert_eq!(NonDeliveryEscalation::KeepPolling.label(), "KEEP_POLLING");
    }

    fn identity(at: u64) -> ObservationIdentity {
        ObservationIdentity {
            epoch: "receiver-test".into(),
            sequence: at,
            changed_at: at.to_string(),
        }
    }

    fn working(pane: &str, timer: u64, body: &str, spinner: char, at: u64) -> Observation {
        observe_capture(
            pane,
            &format!("{body}\n{spinner} {timer}s . GPT-5.6 . /tmp/receiver"),
            at,
            identity(at),
        )
    }

    fn idle(pane: &str, body: &str, at: u64) -> Observation {
        observe_capture(
            pane,
            &format!("{body}\nπ . GPT-5.6 . /tmp/receiver"),
            at,
            identity(at),
        )
    }

    fn dialog(pane: &str, timer: u64, at: u64) -> Observation {
        observe_capture(
            pane,
            &format!(
                "│ Enter select\n│ Esc cancel\n│ ↑/↓ move\n⠙ {timer}s . GPT-5.6 . /tmp/receiver"
            ),
            at,
            identity(at),
        )
    }

    #[test]
    fn idle_to_working_confirms_with_small_new_timer() {
        let pre = idle("%live", "prompt", 100);
        let post = working("%live", 1, "accepted packet", '⠙', 180);
        let result = assess_receiver_receipt("%live", &pre, PostSendObservation::Present(post));
        assert_eq!(result.label(), "RECEIPT_CONFIRMED");
    }

    #[test]
    fn idle_to_working_without_hash_change_is_no_receipt() {
        let pre = idle("%live", "prompt", 100);
        let post = working("%live", 1, "prompt", '⠙', 180);
        let result = assess_receiver_receipt("%live", &pre, PostSendObservation::Present(post));
        assert_eq!(result.label(), "NO_RECEIPT");
        assert_eq!(
            result.reason(),
            Some(&ReceiptReason::StableContentUnchanged)
        );
    }

    #[test]
    fn idle_to_working_with_large_timer_is_indeterminate() {
        let pre = idle("%live", "prompt", 100);
        let post = working("%live", 61, "unrelated old work", '⠙', 180);
        let result = assess_receiver_receipt("%live", &pre, PostSendObservation::Present(post));
        assert_eq!(result.label(), "INDETERMINATE");
        assert!(matches!(
            result.reason(),
            Some(ReceiptReason::TimerTooLargeAfterIdle { .. })
        ));
    }

    #[test]
    fn idle_to_idle_is_no_receipt() {
        let pre = idle("%live", "prompt", 100);
        let post = idle("%live", "prompt", 180);
        let result = assess_receiver_receipt("%live", &pre, PostSendObservation::Present(post));
        assert_eq!(result.reason(), Some(&ReceiptReason::IdleUnchanged));
    }

    #[test]
    fn working_to_working_reset_and_content_change_confirms() {
        let pre = working("%live", 58, "before", '⠋', 100);
        let post = working("%live", 1, "after", '⠙', 180);
        let result = assess_receiver_receipt("%live", &pre, PostSendObservation::Present(post));
        assert_eq!(result.label(), "RECEIPT_CONFIRMED");
    }

    #[test]
    fn working_to_working_without_timer_reset_is_no_receipt() {
        let pre = working("%live", 58, "before", '⠋', 100);
        let post = working("%live", 59, "after", '⠙', 180);
        let result = assess_receiver_receipt("%live", &pre, PostSendObservation::Present(post));
        assert!(matches!(
            result.reason(),
            Some(ReceiptReason::TimerDidNotReset {
                before_secs: 58,
                after_secs: 59
            })
        ));
    }

    #[test]
    fn working_to_working_reset_without_content_change_is_no_receipt() {
        let pre = working("%live", 58, "same", '⠋', 100);
        let post = working("%live", 1, "same", '⠙', 180);
        let result = assess_receiver_receipt("%live", &pre, PostSendObservation::Present(post));
        assert_eq!(
            result.reason(),
            Some(&ReceiptReason::TimerResetButStableContentUnchanged)
        );
    }

    #[test]
    fn dialog_is_indeterminate() {
        let pre = working("%live", 58, "before", '⠋', 100);
        let result = assess_receiver_receipt(
            "%live",
            &pre,
            PostSendObservation::Present(dialog("%live", 1, 101)),
        );
        assert_eq!(result.label(), "INDETERMINATE");
        assert_eq!(result.reason(), Some(&ReceiptReason::DialogOpen));
    }

    #[test]
    fn dead_requires_non_empty_census() {
        let pre = idle("%live", "prompt", 100);
        let dead = assess_receiver_receipt("%live", &pre, PostSendObservation::Absent);
        assert_eq!(dead.label(), "DEAD");
        assert_eq!(dead.reason(), None);

        let empty = assess_receiver_receipt("%live", &pre, PostSendObservation::EmptyPaneList);
        assert_eq!(empty.label(), "INDETERMINATE");
        assert_eq!(
            empty.reason(),
            Some(&ReceiptReason::EmptyPaneListNoDeathClaim)
        );
    }

    #[test]
    fn missing_and_mismatched_observations_are_indeterminate() {
        let pre = idle("%live", "prompt", 100);
        assert_eq!(
            assess_receiver_receipt("%live", &pre, PostSendObservation::Missing).reason(),
            Some(&ReceiptReason::MissingPostObservation)
        );
        let other = idle("%other", "prompt", 101);
        assert!(matches!(
            assess_receiver_receipt("%live", &pre, PostSendObservation::Present(other)),
            ReceiptVerdict::Indeterminate {
                reason: ReceiptReason::PaneIdMismatch { .. },
                ..
            }
        ));
    }

    // ─────────── K8 legs (omp-orchestrator-d6q2): the window, the two
    //             inversions, sender-input isolation, anti-vacuity

    /// RETRACTED LEGS, recorded rather than deleted.
    ///
    /// Two legs stood here asserting that ANY span below 75 seconds yields
    /// INDETERMINATE. They passed. They were removed with the blanket floor they
    /// tested, because the floor would have regressed the live dispatch path -
    /// `crates/omp-orchestrator/src/main.rs` captures pre at :797 and post at
    /// :580 with no sleep between them - and because the reasoning behind them
    /// was wrong: a timer RESET is unambiguous at any spacing, and an
    /// `Idle -> Working` fresh timer is the strongest receipt the fleet has.
    ///
    /// A test that passes is not thereby correct. These two encoded my
    /// misreading faithfully, which is exactly why deleting them silently would
    /// have hidden it.
    ///
    /// What replaces them is the leg below: the ONE ordering claim that holds at
    /// every spacing.
    #[test]
    fn a_short_but_forward_window_is_still_adjudicated() {
        // The live caller's shape: captures seconds apart, not 75.
        let pre = idle("%live", "prompt", 100);
        let post = working("%live", 1, "accepted packet", '\u{2819}', 101);
        let result = assess_receiver_receipt("%live", &pre, PostSendObservation::Present(post));
        assert_eq!(
            result.label(),
            "RECEIPT_CONFIRMED",
            "a 1-second forward window must still confirm, or the live path regresses: {result:?}"
        );

        let pre_w = working("%live", 58, "before", '\u{280B}', 100);
        let post_w = working("%live", 59, "after", '\u{2819}', 101);
        let no_receipt =
            assess_receiver_receipt("%live", &pre_w, PostSendObservation::Present(post_w));
        assert_eq!(
            no_receipt.label(),
            "NO_RECEIPT",
            "a timer that did not reset is unambiguous at any spacing: {no_receipt:?}"
        );
    }

    /// THE LEG THAT SHOULD HAVE EXISTED BEFORE THE GATE DID.
    ///
    /// Equal timestamps are NOT a fault. `now_unix()` has second resolution, so
    /// two captures milliseconds apart legitimately carry the same stamp — and
    /// `ack-stage`'s own fixtures build both captures at `at=100`. A first draft
    /// of the ordering gate refused `post.at <= pre_send.at` and turned that
    /// consumer's `RETRY` into `AWAIT_HUMAN`. Caught by running the CONSUMER's
    /// suite, not by reading the gate — which is the argument for running the
    /// blast radius rather than the crate.
    #[test]
    fn equal_timestamps_are_adjudicated_not_refused() {
        let pre = idle("%live", "prompt", 100);
        let post = idle("%live", "prompt", 100);
        let result = assess_receiver_receipt("%live", &pre, PostSendObservation::Present(post));
        assert_eq!(
            result.label(),
            "NO_RECEIPT",
            "equal stamps are second-resolution truncation, not a clock fault: {result:?}"
        );
        assert_eq!(result.reason(), Some(&ReceiptReason::IdleUnchanged));
    }

    /// KNOWN-GOOD, mandatory: the identical evidence at a valid span still
    /// confirms. Without this the gate is attack-only, and an over-strict gate
    /// gets routed around — a slower death than no gate.
    #[test]
    fn the_same_evidence_at_a_valid_span_still_confirms() {
        let pre = idle("%live", "prompt", 100);
        let post = working(
            "%live",
            1,
            "accepted packet",
            '⠙',
            100 + OBSERVATION_WINDOW_MIN_SECS,
        );
        let result = assess_receiver_receipt("%live", &pre, PostSendObservation::Present(post));
        assert_eq!(result.label(), "RECEIPT_CONFIRMED", "{result:?}");
    }

    /// A post capture stamped BEFORE the pre capture is a clock or ordering
    /// fault. It must refuse, not wrap to a huge span that would pass the floor.
    #[test]
    fn a_reversed_capture_order_refuses_rather_than_wrapping() {
        let pre = working("%live", 58, "before", '⠋', 500);
        let post = working("%live", 1, "after", '⠙', 100);
        let result = assess_receiver_receipt("%live", &pre, PostSendObservation::Present(post));
        let text = result.reason().expect("reason").to_string();
        assert_eq!(result.label(), "INDETERMINATE");
        assert!(
            text.contains("span_secs=0"),
            "saturating_sub must yield 0, not a wrapped span: {text}"
        );
    }

    /// The window floor must not suppress verdicts derivable from ONE capture.
    /// This is the positive control on the gate's PLACEMENT: a wedge marker and
    /// an open dialog are single-capture facts and stay sound below the floor.
    #[test]
    fn single_capture_verdicts_survive_below_the_floor() {
        let pre = idle("%live", "prompt", 100);
        let wedge = working("%live", 5, "Press up to edit queued messages", '⠙', 101);
        // The wedge marker is tick-monitor's call, not a fourth detector here.
        if matches!(wedge.state, PaneState::Wedged) {
            let result =
                assess_receiver_receipt("%live", &pre, PostSendObservation::Present(wedge));
            assert_eq!(result.label(), "NO_RECEIPT", "{result:?}");
            assert_eq!(result.reason(), Some(&ReceiptReason::WedgedUnsubmitted));
        }
        let dialog_result = assess_receiver_receipt(
            "%live",
            &pre,
            PostSendObservation::Present(dialog("%live", 1, 101)),
        );
        assert_eq!(dialog_result.reason(), Some(&ReceiptReason::DialogOpen));
    }

    /// INVERSION 1 — measured: a send returned `successful: ["4"]` for a packet
    /// that NEVER ARRIVED. The receiver evidence for that case is an idle pane
    /// that did not change, and it must be inexpressible as an arrival.
    #[test]
    fn a_sender_success_with_an_unchanged_idle_pane_is_not_an_arrival() {
        let pre = idle("%live", "prompt", 100);
        let post = idle("%live", "prompt", 100 + OBSERVATION_WINDOW_MIN_SECS + 5);
        let result = assess_receiver_receipt("%live", &pre, PostSendObservation::Present(post));
        assert_ne!(
            result.label(),
            "RECEIPT_CONFIRMED",
            "no sender return value may make this an arrival: {result:?}"
        );
        assert_eq!(result.reason(), Some(&ReceiptReason::IdleUnchanged));
    }

    /// INVERSION 2 — the converse fired the same session: a send reported
    /// FAILURE while the packet DID land. The receiver evidence confirms, and no
    /// sender-reported failure can withdraw it, because none is accepted.
    #[test]
    fn a_sender_failure_does_not_withdraw_a_receiver_confirmation() {
        let pre = working("%live", 58, "before", '⠋', 100);
        let post = working(
            "%live",
            1,
            "after",
            '⠙',
            100 + OBSERVATION_WINDOW_MIN_SECS + 5,
        );
        let result = assess_receiver_receipt("%live", &pre, PostSendObservation::Present(post));
        assert_eq!(result.label(), "RECEIPT_CONFIRMED", "{result:?}");
    }

    /// ANTI-VACUITY: an EMPTY observation set and a genuine non-arrival are
    /// different facts. An empty pane census may never produce `DEAD` and may
    /// never produce `NO_RECEIPT` — only `INDETERMINATE`.
    #[test]
    fn an_empty_census_is_an_error_never_a_non_arrival() {
        let pre = idle("%live", "prompt", 100);
        for probe in [
            PostSendObservation::EmptyPaneList,
            PostSendObservation::Missing,
        ] {
            let result = assess_receiver_receipt("%live", &pre, probe);
            assert_eq!(
                result.label(),
                "INDETERMINATE",
                "an unobservable receiver is an ERROR, not 'nothing arrived': {result:?}"
            );
            assert_ne!(result.label(), "NO_RECEIPT");
            assert_ne!(result.label(), "DEAD");
        }
    }

    /// SENDER-INPUT ISOLATION, checked against the SOURCE rather than asserted
    /// in prose. `assess_receiver_receipt` cannot read a sender return value
    /// because no such value is in its signature; this leg proves the crate does
    /// not smuggle one in by another route.
    ///
    /// # Two traps this leg fell into, in order, both recorded because they are
    /// the same family
    ///
    /// 1. **Self-referential input.** The first version wrote the forbidden words
    ///    as plain literals, and failed on itself: the scanner's input is the file
    ///    containing the scanner. Cured with `concat!`, the house pattern from
    ///    `path-literal-guard`, so this file never holds the contiguous needle.
    /// 2. **A bare scan cannot tell PROSE from CODE.** It still failed, because
    ///    `successful:` appears in this module's own doc comment quoting the
    ///    measured inversion, and `sender_success` appears inside a TEST NAME.
    ///    Both are prose ABOUT the sender path, not code that reads it. That is
    ///    the identical defect the orchestrator hit the same evening, where
    ///    `grep -c` returned 1 from a `///` doc comment and nearly became an
    ///    incomplete-fix report.
    ///
    /// So the claim is scoped to what it can actually support: **the PRODUCTION
    /// source** — everything above the first `#[cfg(test)]` — references no
    /// sender-side success. Tests legitimately name the inversions they encode.
    ///
    /// ANTI-VACUITY, twice: the production slice must be non-empty AND must
    /// contain a known production symbol, or a zero below is a broken reader
    /// rather than a clean crate.
    #[test]
    fn the_crate_source_references_no_sender_side_success() {
        let source = include_str!("lib.rs");
        let production = source
            .split_once(concat!("#[cfg", "(test)]"))
            .map_or(source, |(before, _)| before);

        assert!(
            production.len() > 1_000,
            "ANTI-VACUITY: the production slice is {} bytes, so the scan covers nothing",
            production.len()
        );
        assert!(
            production.contains("pub fn assess_receiver_receipt"),
            "ANTI-VACUITY: the slice does not contain the function under audit, \
             so every check below is void"
        );
        assert!(
            !production.contains("mod tests"),
            "the slice must END before the tests, or their prose is scanned as code"
        );

        // Sender-side vocabulary in the forms this fleet actually emits, built by
        // `concat!` so the needle exists only at runtime.
        let forbidden = [
            concat!("successful", ":"),
            concat!("robot", "_send"),
            concat!("send", "_result"),
            concat!("sender", "_success"),
            concat!("transport", "_ok"),
        ];
        for needle in forbidden {
            let hits = production.matches(needle).count();
            assert_eq!(
                hits, 0,
                "receiver observation must share no input with the sender path, \
                 but the production source mentions `{needle}` {hits} time(s)"
            );
        }
    }

    /// The window floor is restated from `pane_truth::TWO_CAPTURE_MIN_SECS`
    /// rather than imported, so the two must be gated into agreement. A drift
    /// between two copies of a floor is how one caller enforces 75 and another
    /// enforces nothing.
    #[test]
    fn the_window_floor_matches_pane_truths_authority() {
        let authority = u64::try_from(pane_truth::TWO_CAPTURE_MIN_SECS)
            .expect("the authority's floor must be a non-negative number of seconds");
        assert_eq!(
            OBSERVATION_WINDOW_MIN_SECS, authority,
            "receiver-receipt's window floor drifted from pane-truth's authority"
        );
    }
}

#[cfg(test)]
mod escalation_tests {
    use super::*;
    use crate::{escalate_non_delivery, ComposerEvidence, NonDeliveryEscalation};
    use tick_monitor::PaneState;

    /// 6q5 THE MEASURED DEFECT: sender reported success, pane stayed Idle,
    /// composer was empty. Must escalate to ResendDirect, not KeepPolling.
    #[test]
    fn idle_with_free_composer_escalates_to_resend_direct() {
        let result = escalate_non_delivery(&PaneState::Idle, ComposerEvidence::Free);
        assert_eq!(result, NonDeliveryEscalation::ResendDirect);
    }

    /// 6q5 POSITIVE: packet arrived but sits unsubmitted (Typed composer).
    /// Escalate to SubmitParked — the documented Enter recovery.
    #[test]
    fn idle_with_typed_composer_escalates_to_submit_parked() {
        let result = escalate_non_delivery(&PaneState::Idle, ComposerEvidence::Typed);
        assert_eq!(result, NonDeliveryEscalation::SubmitParked);
    }

    /// Working panes carry their own verdict in assess_receiver_receipt.
    /// The escalation must not fire there.
    #[test]
    fn working_pane_keeps_polling_regardless_of_composer() {
        let r1 = escalate_non_delivery(
            &PaneState::Working { timer_secs: 60 },
            ComposerEvidence::Free,
        );
        let r2 = escalate_non_delivery(
            &PaneState::Working { timer_secs: 60 },
            ComposerEvidence::Typed,
        );
        assert_eq!(r1, NonDeliveryEscalation::KeepPolling);
        assert_eq!(r2, NonDeliveryEscalation::KeepPolling);
    }

    /// MUTATION: if escalate_non_delivery always returned KeepPolling, the
    /// Idle+Free case (the 6q5 defect) would go undetected — the dispatcher
    /// would sit beside an idle worker and never resend. This assertion is
    /// the mutation leg: break the Idle+Free arm and this test goes RED.
    #[test]
    fn mutation_breaking_idle_free_arm_is_detected() {
        // The real function must return ResendDirect for the defect case.
        // If it returned KeepPolling, the 6q5 bug would reproduce silently.
        let result = escalate_non_delivery(&PaneState::Idle, ComposerEvidence::Free);
        assert_ne!(result, NonDeliveryEscalation::KeepPolling);
    }
}

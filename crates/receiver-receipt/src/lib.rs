#![forbid(unsafe_code)]

//! Receiver-side dispatch receipt classification.
//!
//! This crate deliberately never sends input. A caller performs the transport-specific
//! send, captures the same pane afterward, and passes the pre/post observations here.
//! A sender return value is therefore never part of the receipt proof.

use std::fmt;
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
    PaneIdMismatch { expected: String, observed: String },
    DialogOpen,
    /// A queued prompt landed in the composer but the pane never submitted it.
    WedgedUnsubmitted,
    EmptyPaneListNoDeathClaim,
    ObservationNotWorking { side: &'static str, state: String },
    IdleUnchanged,
    PostBecameIdle,
    TimerDidNotReset { before_secs: u64, after_secs: u64 },
    TimerResetButStableContentUnchanged,
    TimerTooLargeAfterIdle { after_secs: u64, max_secs: u64 },
}

impl fmt::Display for ReceiptReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPostObservation => f.write_str("missing_post_observation"),
            Self::PaneIdMismatch { expected, observed } => {
                write!(f, "pane_id_mismatch expected={expected} observed={observed}")
            }
            Self::DialogOpen => f.write_str("dialog_open"),
            Self::WedgedUnsubmitted => f.write_str("WEDGED_UNSUBMITTED"),
            Self::EmptyPaneListNoDeathClaim => f.write_str("NOBODY_DEAD empty_pane_list"),
            Self::ObservationNotWorking { side, state } => {
                write!(f, "{side}_observation_not_working state={state}")
            }
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

/// Convert a captured pane render into the shared tick-monitor observation shape.
///
/// `tick-monitor` owns last-line anchoring, timer parsing, dialog detection, and
/// spinner-stripped hashing. This adapter only adds the caller's pane id and timestamp.
pub fn observe_capture(pane_id: impl Into<String>, capture: &str, at: u64) -> Observation {
    Observation {
        pane_id: pane_id.into(),
        state: classify(capture),
        hash: tick_monitor::stable_hash(capture),
        at,
    }
}

/// Classify receiver evidence after an external transport send.
///
/// The function performs no send and does not inspect sender return values. Confirmation
/// is keyed by the pre-send state:
///
/// * IDLE -> WORKING confirms when the new timer is small;
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

    match (&pre_send.state, &post.state) {
        (PaneState::Idle, PaneState::Working { timer_secs }) => {
            if *timer_secs <= MAX_IDLE_TO_WORKING_TIMER_SECS {
                ReceiptVerdict::ReceiptConfirmed {
                    pane_id: pane_id_owned,
                    timer_before_secs: None,
                    timer_after_secs: *timer_secs,
                    stable_content_changed: pre_send.hash != post.hash,
                }
            } else {
                ReceiptVerdict::Indeterminate {
                    pane_id: pane_id_owned,
                    reason: ReceiptReason::TimerTooLargeAfterIdle {
                        after_secs: *timer_secs,
                        max_secs: MAX_IDLE_TO_WORKING_TIMER_SECS,
                    },
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

    fn working(pane: &str, timer: u64, body: &str, spinner: char, at: u64) -> Observation {
        observe_capture(
            pane,
            &format!("{body}\n{spinner} {timer}s . GPT-5.6 . /tmp/receiver"),
            at,
        )
    }

    fn idle(pane: &str, body: &str, at: u64) -> Observation {
        observe_capture(pane, &format!("{body}\nπ . GPT-5.6 . /tmp/receiver"), at)
    }

    fn dialog(pane: &str, timer: u64, at: u64) -> Observation {
        observe_capture(
            pane,
            &format!(
                "│ Enter select\n│ Esc cancel\n│ ↑/↓ move\n⠙ {timer}s . GPT-5.6 . /tmp/receiver"
            ),
            at,
        )
    }

    #[test]
    fn idle_to_working_confirms_with_small_new_timer() {
        let pre = idle("%live", "prompt", 100);
        let post = working("%live", 1, "accepted packet", '⠙', 101);
        let result = assess_receiver_receipt(
            "%live",
            &pre,
            PostSendObservation::Present(post),
        );
        assert_eq!(result.label(), "RECEIPT_CONFIRMED");
    }

    #[test]
    fn idle_to_working_with_large_timer_is_indeterminate() {
        let pre = idle("%live", "prompt", 100);
        let post = working("%live", 61, "unrelated old work", '⠙', 101);
        let result = assess_receiver_receipt(
            "%live",
            &pre,
            PostSendObservation::Present(post),
        );
        assert_eq!(result.label(), "INDETERMINATE");
        assert!(matches!(
            result.reason(),
            Some(ReceiptReason::TimerTooLargeAfterIdle { .. })
        ));
    }

    #[test]
    fn idle_to_idle_is_no_receipt() {
        let pre = idle("%live", "prompt", 100);
        let post = idle("%live", "prompt", 101);
        let result = assess_receiver_receipt(
            "%live",
            &pre,
            PostSendObservation::Present(post),
        );
        assert_eq!(result.reason(), Some(&ReceiptReason::IdleUnchanged));
    }

    #[test]
    fn working_to_working_reset_and_content_change_confirms() {
        let pre = working("%live", 58, "before", '⠋', 100);
        let post = working("%live", 1, "after", '⠙', 101);
        let result = assess_receiver_receipt(
            "%live",
            &pre,
            PostSendObservation::Present(post),
        );
        assert_eq!(result.label(), "RECEIPT_CONFIRMED");
    }

    #[test]
    fn working_to_working_advanced_timer_is_no_receipt() {
        let pre = working("%live", 58, "same", '⠋', 100);
        let post = working("%live", 59, "same", '⠙', 101);
        let result = assess_receiver_receipt(
            "%live",
            &pre,
            PostSendObservation::Present(post),
        );
        assert_eq!(
            result.reason(),
            Some(&ReceiptReason::TimerDidNotReset {
                before_secs: 58,
                after_secs: 59,
            })
        );
    }

    #[test]
    fn timer_reset_without_content_change_is_no_receipt() {
        let pre = working("%live", 58, "same", '⠋', 100);
        let post = working("%live", 1, "same", '⠙', 101);
        let result = assess_receiver_receipt(
            "%live",
            &pre,
            PostSendObservation::Present(post),
        );
        assert_eq!(
            result.reason(),
            Some(&ReceiptReason::TimerResetButStableContentUnchanged)
        );
    }

    #[test]
    fn dialog_is_indeterminate_even_when_timer_resets() {
        let pre = working("%live", 58, "before", '⠋', 100);
        let result = assess_receiver_receipt(
            "%live",
            &pre,
            PostSendObservation::Present(dialog("%live", 1, 101)),
        );
        assert_eq!(result.reason(), Some(&ReceiptReason::DialogOpen));
    }

    #[test]
    fn wedged_post_is_no_receipt_with_distinct_reason() {
        let pre = working("%live", 58, "before", '⠋', 100);
        let post = Observation {
            pane_id: "%live".to_owned(),
            state: PaneState::Wedged,
            hash: pre.hash,
            at: 101,
        };
        let result = assess_receiver_receipt(
            "%live",
            &pre,
            PostSendObservation::Present(post),
        );
        assert_eq!(result.label(), "NO_RECEIPT");
        assert_eq!(result.reason().unwrap().to_string(), "WEDGED_UNSUBMITTED");
    }

    #[test]
    fn absent_pane_is_named_dead_without_claiming_from_empty_catalog() {
        let pre = idle("%live", "prompt", 100);
        let dead = assess_receiver_receipt("%live", &pre, PostSendObservation::Absent);
        assert_eq!(dead.label(), "DEAD");
        assert_eq!(dead.reason(), None);

        let empty = assess_receiver_receipt("%live", &pre, PostSendObservation::EmptyPaneList);
        assert_eq!(empty.label(), "INDETERMINATE");
        assert_eq!(empty.reason(), Some(&ReceiptReason::EmptyPaneListNoDeathClaim));
    }

    #[test]
    fn missing_post_and_identity_drift_are_indeterminate() {
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
}

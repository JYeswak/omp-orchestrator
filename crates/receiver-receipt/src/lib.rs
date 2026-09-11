#![forbid(unsafe_code)]

//! Receiver-side dispatch receipt classification.
//!
//! This crate deliberately never sends input. A caller performs the transport-specific
//! send, captures the same pane afterward, and passes the pre/post observations here.
//! A sender return value is therefore never part of the receipt proof.

use std::fmt;
/// Re-exported so a consumer of [`classify_ack_wait`] can name its inputs without
/// taking a direct `tick-monitor` dependency it does not otherwise need. Without
/// this, `omp-orchestrator` — which depends on this crate and not on tick-monitor —
/// could not spell the argument type.
pub use tick_monitor::{Observation, ObservationIdentity, PaneState};
use tick_monitor::classify;

pub mod irc_delivery;
pub use irc_delivery::{
    maps_sender_exit_onto_irc_receipt, pane_transport_cannot_use_irc_receipt,
    record_hub_irc_receipt, refuse_sender_exit_mapping_file, HubReceiverEvidence,
    IrcDeliveryOutcome, IrcDeliveryReceipt, PaneIrcRefusal,
};


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
    /// The ACK line names a pane different from the dispatched pane.
    AckPaneMismatch {
        expected: String,
        got: String,
    },
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
            Self::AckPaneMismatch { expected, got } => {
                write!(f, "ACK_PANE_MISMATCH expected={expected} got={got}")
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
    /// The authoritative tracker ACK proves delivery even when the pane is now busy.
    AckConfirmed {
        pane_id: String,
        comment: String,
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
            Self::ReceiptConfirmed { .. } | Self::AckConfirmed { .. } => "RECEIPT_CONFIRMED",
            Self::NoReceipt { .. } => "NO_RECEIPT",
            Self::Indeterminate { .. } => "INDETERMINATE",
            Self::Dead { .. } => "DEAD",
        }
    }

    pub fn reason(&self) -> Option<&ReceiptReason> {
        match self {
            Self::ReceiptConfirmed { .. } | Self::AckConfirmed { .. } | Self::Dead { .. } => None,
            Self::NoReceipt { reason, .. } | Self::Indeterminate { reason, .. } => Some(reason),
        }
    }

    /// Whether the receipt evidence itself names a condition that already owes a
    /// human. All other indeterminate receipt evidence remains machine work: the
    /// caller must re-measure it instead of turning uncertainty into escalation.
    pub const fn owes_a_human(&self) -> bool {
        matches!(
            self,
            Self::Indeterminate {
                reason: ReceiptReason::DialogOpen,
                ..
            }
        )
    }

}
/// How far a post-send timer may exceed the OBSERVED SPAN before the receipt is
/// unattributable.
///
/// # This was an absolute bound and that was only correct for one wait length
///
/// It read `MAX_IDLE_TO_WORKING_TIMER_SECS = 30` with the rationale *"new work must begin
/// promptly after an idle pane accepts the packet"*. The rationale is right; the absolute
/// form is not. The quantity that bounds a legitimate timer is **how long we waited**, not
/// a constant: a pane that starts work the instant it accepts a packet will show
/// `timer_secs ≈ elapsed_since_send`, so widening the wait necessarily grows the timer.
///
/// MEASURED 2026-09-05, immediately after `RECEIPT_TIMEOUT` was derived from
/// [`OBSERVATION_WINDOW_MIN_SECS`] (30s → 90s): a live tick returned
/// `DISPATCH_FAILED detail=ACK_STAGE_INDETERMINATE reason=timer_too_large_after_idle
/// after_secs=32 max_secs=30` on `%9`/`h71-missing-tests-0hwn`. A 32s timer after a wait
/// that may run to 90s is a pane working *correctly*; the guard fired because its bound
/// had been calibrated to the old 30s wait. **Fixing one hardcoded duration exposed a
/// second one that had silently depended on it** — the third instance of that coupling in
/// one session, after `ADVISORY_CEILING`/its recording anchor and
/// `RECEIPT_TIMEOUT`/`OBSERVATION_WINDOW_MIN_SECS`.
///
/// The defect this guard exists to catch is a timer that EXCEEDS the span, which means the
/// work predates our send and cannot be attributed to it. That test is now expressed
/// against the span, with this value as jitter tolerance for capture skew.
pub const IDLE_TO_WORKING_TIMER_TOLERANCE_SECS: u64 = 30;

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


/// What an expired ACK wait is allowed to conclude.
///
/// # The defect (`omp-orchestrator-iis6`)
///
/// `main.rs:47` held `const RECEIPT_TIMEOUT: Duration = Duration::from_secs(30)` — a
/// bare literal beside `DEFAULT_COMMAND_TIMEOUT`, with no reasoning at its definition
/// site. On expiry the loop emitted
/// `ACK_STAGE_RETRY_BLOCKED … reason=ack_readback_missing after=30s` **whether the
/// receiving pane was mid-tool-call or dead.** A premature `AWAIT_HUMAN` interrupts a
/// person over work that was proceeding correctly.
///
/// # WIDENING THE NUMBER IS NOT THE FIX
///
/// Observed pane timers this session sat at **120s, 1320s and 1440s inside a single
/// tool call**. No fixed constant separates *busy* from *unreachable*, so a longer
/// window only moves the point at which it guesses wrong. **The window decides when
/// we re-check; it must not decide whether a human is called.**
///
/// # The discriminator, and it is already paid for
///
/// [`OBSERVATION_WINDOW_MIN_SECS`] above is 75s and carries the asymmetry that fixes
/// it: *a missed freeze costs idle minutes, a false freeze destroys work in flight.*
/// The same evidence answers this question — **is the pane's timer advancing across
/// two captures at least that far apart?** A pane whose timer advances is demonstrably
/// working and owes no human anything; the ACK is merely late.
///
/// So this reuses the existing floor rather than introducing a second number. There is
/// no `ACK_WAIT_SECS` constant, deliberately: adding one would be the bare literal
/// again, one crate over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AckWaitVerdict {
    /// Timer advanced across a window at or beyond the floor. **Retry next tick; do
    /// NOT escalate.** This is the arm that did not exist.
    BusyStillWorking {
        timer_from: u64,
        timer_to: u64,
        span_secs: u64,
    },
    /// The pane is not working and produced no ack. A human is owed.
    Unreachable { reason: &'static str },
    /// Not enough separation to judge motion. **Restrictive: it is neither a busy
    /// finding nor a death claim**, and collapsing it into either is the defect this
    /// enum exists to prevent.
    Indeterminate { reason: &'static str },
}

impl AckWaitVerdict {
    /// Whether a human must be interrupted. Only `Unreachable` earns that.
    pub const fn owes_a_human(&self) -> bool {
        matches!(self, AckWaitVerdict::Unreachable { .. })
    }

    pub fn label(&self) -> &'static str {
        match self {
            AckWaitVerdict::BusyStillWorking { .. } => "ACK_PENDING_WORKER_BUSY",
            AckWaitVerdict::Unreachable { .. } => "ACK_READBACK_MISSING",
            AckWaitVerdict::Indeterminate { .. } => "ACK_WAIT_INDETERMINATE",
        }
    }

    /// The action, from the variant, so a refusal string cannot drift from the state
    /// that produced it — the correction `GateReachability::next_action` needed.
    pub fn next_action(&self) -> &'static str {
        match self {
            AckWaitVerdict::BusyStillWorking { .. } => "retry-next-tick",
            AckWaitVerdict::Unreachable { .. } => "inspect-pane-or-redispatch",
            AckWaitVerdict::Indeterminate { .. } => "re-observe-past-the-window-floor",
        }
    }

    /// The reason string, or empty for the busy arm which carries numbers instead.
    /// Exists so two verdicts can be compared on their reason without a caller
    /// re-matching the enum and drifting from it.
    pub fn reason_or_empty(&self) -> &'static str {
        match self {
            AckWaitVerdict::BusyStillWorking { .. } => "",
            AckWaitVerdict::Unreachable { reason }
            | AckWaitVerdict::Indeterminate { reason } => reason,
        }
    }
}

/// Classify an expired ACK wait from the two captures the loop already takes.
///
/// Pure and clock-injected, because the never-acking case and the slow-acking case
/// currently produce the same verdict on live data — so a guard written inline could
/// not be shown to bite in either direction. That lesson is `kxe.6`'s: a guard on a
/// condition today's data does not exhibit is indistinguishable from a comment.
pub fn classify_ack_wait(first: &Observation, last: &Observation) -> AckWaitVerdict {
    if first.pane_id != last.pane_id {
        return AckWaitVerdict::Indeterminate {
            reason: "PANE_ID_MISMATCH",
        };
    }
    // Saturating, and the ordering is checked rather than assumed: a backward-stamped
    // pair is a clock fault, not a zero-length window.
    if last.at < first.at {
        return AckWaitVerdict::Indeterminate {
            reason: "CAPTURES_OUT_OF_ORDER",
        };
    }
    let span_secs = last.at - first.at;
    match (&first.state, &last.state) {
        // THE ARM THAT DID NOT EXIST. Motion is only observable across a window at or
        // beyond the floor; below it, an unchanged timer means nothing.
        (
            PaneState::Working { timer_secs: from },
            PaneState::Working { timer_secs: to },
        ) => {
            if span_secs < OBSERVATION_WINDOW_MIN_SECS {
                return AckWaitVerdict::Indeterminate {
                    reason: "WINDOW_BELOW_FLOOR",
                };
            }
            if to > from {
                AckWaitVerdict::BusyStillWorking {
                    timer_from: *from,
                    timer_to: *to,
                    span_secs,
                }
            } else {
                // A working spinner whose timer did NOT advance across 75s is the
                // wedge `pane-truth` already names; it is not busy.
                AckWaitVerdict::Unreachable {
                    reason: "TIMER_DID_NOT_ADVANCE_ACROSS_THE_FLOOR",
                }
            }
        }
        // An open dialog is a pane waiting on a person, which is a human debt already
        // and must not be reported as a missing ack.
        (_, PaneState::Dialog { .. }) => AckWaitVerdict::Unreachable {
            reason: "PANE_IS_BLOCKED_ON_A_DIALOG",
        },
        (_, PaneState::Wedged) => AckWaitVerdict::Unreachable {
            reason: "PANE_IS_WEDGED",
        },
        // Idle with no ack after a full wait: it read the packet and did not answer,
        // or never got it. Either way a person is owed.
        (_, PaneState::Idle) => AckWaitVerdict::Unreachable {
            reason: "PANE_IDLE_WITH_NO_ACK",
        },
        (_, PaneState::Working { .. }) => AckWaitVerdict::Indeterminate {
            reason: "NO_PRIOR_WORKING_CAPTURE_TO_COMPARE",
        },
        // The compiler named these two, and they are genuinely different facts. A
        // wildcard arm would have swallowed both -- which `state-wildcard-lint`
        // forbids on state-like enums, and this is why.
        //
        // A 402 is a pane stopped by a QUOTA, not by our dispatch: no amount of
        // waiting or re-dispatching clears it, so it owes a human with its own reason
        // rather than reading as a missing ack.
        (_, PaneState::ProviderError402) => AckWaitVerdict::Unreachable {
            reason: "PANE_HALTED_ON_PROVIDER_402",
        },
        // `Unproven` is the classifier declining to answer. Reporting it as
        // unreachable would turn "we could not tell" into a death claim.
        (_, PaneState::Unproven) => AckWaitVerdict::Indeterminate {
            reason: "PANE_STATE_UNPROVEN",
        },
    }
}
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
    // 1. IT WOULD HAVE REGRESSED THE LIVE PATH. `crates/omp-orchestrator/src/resident.rs`
    //    takes `pre_observation` and the post capture with NO
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
            // The bound is the OBSERVED SPAN plus jitter tolerance, not a constant. A
            // timer at or below the span is a pane that began work inside our wait; a
            // timer that EXCEEDS the span means the work predates the send and cannot be
            // attributed to it, which is the only defect this arm exists to catch.
            let max_secs = span_secs.saturating_add(IDLE_TO_WORKING_TIMER_TOLERANCE_SECS);
            if *timer_secs > max_secs {
                ReceiptVerdict::Indeterminate {
                    pane_id: pane_id_owned,
                    reason: ReceiptReason::TimerTooLargeAfterIdle {
                        after_secs: *timer_secs,
                        max_secs,
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

    /// A timer that EXCEEDS the observed span means the work predates the send.
    ///
    /// The old form asserted `timer=61` was "large" against an absolute 30s bound. That
    /// premise died when `RECEIPT_TIMEOUT` was derived from `OBSERVATION_WINDOW_MIN_SECS`:
    /// a 61s timer inside an 80s span is a pane that started work DURING our wait, which
    /// is a receipt, not a defect. Measured live 2026-09-05 as false refusals —
    /// `after_secs=32 max_secs=30` on `%9` and `after_secs=31 max_secs=30` on `%8`.
    ///
    /// Here the span is `180 - 100 = 80`, the bound is `80 + 30 = 110`, and the timer is
    /// 200 — the pane has been working ~120s longer than we have been waiting, so the work
    /// cannot be attributed to this dispatch.
    #[test]
    fn idle_to_working_with_timer_exceeding_span_is_indeterminate() {
        let pre = idle("%live", "prompt", 100);
        let post = working("%live", 200, "unrelated old work", '⠙', 180);
        let result = assess_receiver_receipt("%live", &pre, PostSendObservation::Present(post));
        assert_eq!(result.label(), "INDETERMINATE");
        assert!(matches!(
            result.reason(),
            Some(ReceiptReason::TimerTooLargeAfterIdle { .. })
        ));
    }

    /// The known-good half: a timer INSIDE the span is the case that was being refused.
    ///
    /// Without this leg the arm above could be satisfied by a bound so wide that nothing
    /// ever trips it, and the false-refusal regression would return unnoticed.
    #[test]
    fn idle_to_working_with_timer_inside_the_span_is_a_receipt() {
        let pre = idle("%live", "prompt", 100);
        let post = working("%live", 61, "fresh packet text", '⠙', 180);
        let result = assess_receiver_receipt("%live", &pre, PostSendObservation::Present(post));
        assert_ne!(
            result.label(),
            "INDETERMINATE",
            "a 61s timer inside an 80s span is a pane that began work during the wait"
        );
        assert!(!matches!(
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
    /// `crates/omp-orchestrator/src/resident.rs` captures pre and post
    /// with no sleep between them - and because the reasoning behind them
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

#[cfg(test)]
mod ack_wait_tests {
    use super::*;
    use tick_monitor::{Observation, PaneState};

    fn obs(state: PaneState, at: u64) -> Observation {
        Observation {
            pane_id: "%1408".to_owned(),
            state,
            hash: 0,
            at,
            epoch: "e".to_owned(),
            sequence: at,
            changed_at: "c".to_owned(),
        }
    }

    /// ACCEPTANCE 3, the arm that did not exist. A pane whose timer ADVANCES across
    /// the floor is demonstrably working and owes no human anything.
    #[test]
    fn an_advancing_timer_across_the_floor_is_busy_not_unreachable() {
        let first = obs(PaneState::Working { timer_secs: 120 }, 1_000);
        let last = obs(
            PaneState::Working { timer_secs: 200 },
            1_000 + OBSERVATION_WINDOW_MIN_SECS,
        );
        let verdict = classify_ack_wait(&first, &last);
        assert_eq!(
            verdict,
            AckWaitVerdict::BusyStillWorking {
                timer_from: 120,
                timer_to: 200,
                span_secs: OBSERVATION_WINDOW_MIN_SECS,
            }
        );
        // THE WHOLE POINT: no human is interrupted.
        assert!(!verdict.owes_a_human());
        assert_eq!(verdict.next_action(), "retry-next-tick");
        assert_eq!(verdict.label(), "ACK_PENDING_WORKER_BUSY");
    }

    /// ACCEPTANCE 4, the half that refuses the cheap fix. **A pane that never acks
    /// must stay a human debt at ANY window**, so widening cannot launder it.
    /// Timer-advance is the discriminator, not elapsed time.
    #[test]
    fn a_pane_that_never_acks_stays_a_human_debt_at_every_window() {
        for span in [
            0,
            30,
            OBSERVATION_WINDOW_MIN_SECS,
            OBSERVATION_WINDOW_MIN_SECS * 100,
        ] {
            // Idle at the end means it is not working and produced no ack.
            let verdict = classify_ack_wait(
                &obs(PaneState::Working { timer_secs: 10 }, 0),
                &obs(PaneState::Idle, span),
            );
            assert!(
                verdict.owes_a_human(),
                "an idle, un-acking pane must owe a human at span={span}, got {verdict:?}"
            );
            assert_eq!(verdict.label(), "ACK_READBACK_MISSING");
        }
    }

    /// A working spinner whose timer does NOT advance across the floor is the wedge
    /// `pane-truth` already names. Busy and wedged are the two states a single
    /// capture cannot separate, which is why the floor exists.
    #[test]
    fn a_frozen_timer_across_the_floor_is_unreachable_not_busy() {
        let verdict = classify_ack_wait(
            &obs(PaneState::Working { timer_secs: 300 }, 0),
            &obs(
                PaneState::Working { timer_secs: 300 },
                OBSERVATION_WINDOW_MIN_SECS,
            ),
        );
        assert_eq!(
            verdict,
            AckWaitVerdict::Unreachable {
                reason: "TIMER_DID_NOT_ADVANCE_ACROSS_THE_FLOOR"
            }
        );
        assert!(verdict.owes_a_human());
    }

    /// RESTRICTIVE MIDDLE. Below the floor, an unchanged timer means nothing — and
    /// **`Indeterminate` must be neither a busy finding nor a death claim.** The old
    /// code had no such state: everything that was not confirmed owed a human.
    #[test]
    fn below_the_floor_the_answer_is_indeterminate_in_both_directions() {
        let short = classify_ack_wait(
            &obs(PaneState::Working { timer_secs: 10 }, 0),
            &obs(
                PaneState::Working { timer_secs: 40 },
                OBSERVATION_WINDOW_MIN_SECS - 1,
            ),
        );
        assert_eq!(
            short,
            AckWaitVerdict::Indeterminate {
                reason: "WINDOW_BELOW_FLOOR"
            }
        );
        assert!(
            !short.owes_a_human(),
            "an unmeasurable window must not interrupt a human"
        );
        assert_eq!(short.next_action(), "re-observe-past-the-window-floor");

        // 30s -- the OLD bound -- is below the floor, so the old window could not
        // have answered this question even in principle. That is the bead's thesis.
        //
        // Compile-time by design: this is a const-drift guard whose whole job is to
        // fail the build when someone edits the floor, and a runtime assertion
        // cannot do that job.
        #[allow(clippy::assertions_on_constants)]
        {
            assert!(30 < OBSERVATION_WINDOW_MIN_SECS);
        }
    }

    /// The two states the COMPILER named. A wildcard arm would have swallowed both,
    /// and `state-wildcard-lint` forbids that on state-like enums for this reason.
    /// A 402 is a quota halt no re-dispatch clears; `Unproven` is the classifier
    /// declining to answer, and calling that unreachable turns "cannot tell" into a
    /// death claim.
    #[test]
    fn a_quota_halt_and_an_unproven_state_are_different_facts() {
        let quota = classify_ack_wait(
            &obs(PaneState::Working { timer_secs: 1 }, 0),
            &obs(PaneState::ProviderError402, OBSERVATION_WINDOW_MIN_SECS),
        );
        assert_eq!(
            quota,
            AckWaitVerdict::Unreachable {
                reason: "PANE_HALTED_ON_PROVIDER_402"
            }
        );
        let unproven = classify_ack_wait(
            &obs(PaneState::Working { timer_secs: 1 }, 0),
            &obs(PaneState::Unproven, OBSERVATION_WINDOW_MIN_SECS),
        );
        assert!(!unproven.owes_a_human(), "cannot-tell is not a death claim");
        assert_ne!(quota.reason_or_empty(), unproven.reason_or_empty());
    }

    /// Clock and identity faults must not be laundered into a verdict about the pane.
    #[test]
    fn a_mismatched_pane_or_a_backward_clock_refuses_to_judge() {
        let mut other = obs(PaneState::Idle, OBSERVATION_WINDOW_MIN_SECS);
        other.pane_id = "%9999".to_owned();
        assert_eq!(
            classify_ack_wait(&obs(PaneState::Working { timer_secs: 1 }, 0), &other),
            AckWaitVerdict::Indeterminate {
                reason: "PANE_ID_MISMATCH"
            }
        );
        // Backward-stamped pair: a clock fault, NOT a zero-length window. Saturating
        // subtraction would have silently produced span=0 and read as below-floor.
        assert_eq!(
            classify_ack_wait(
                &obs(PaneState::Working { timer_secs: 1 }, 500),
                &obs(PaneState::Working { timer_secs: 2 }, 100)
            ),
            AckWaitVerdict::Indeterminate {
                reason: "CAPTURES_OUT_OF_ORDER"
            }
        );
    }

    /// ANTI-VACUITY: exactly one variant may interrupt a human, and at least one
    /// must. A classifier where none owes a human never escalates; one where all do
    /// is the defect we started from.
    #[test]
    fn exactly_the_unreachable_arm_owes_a_human() {
        let busy = AckWaitVerdict::BusyStillWorking {
            timer_from: 1,
            timer_to: 2,
            span_secs: 75,
        };
        let dead = AckWaitVerdict::Unreachable { reason: "X" };
        let unknown = AckWaitVerdict::Indeterminate { reason: "Y" };
        assert_eq!(
            [&busy, &dead, &unknown]
                .iter()
                .filter(|v| v.owes_a_human())
                .count(),
            1,
            "exactly one arm may interrupt a human"
        );
        // And the three labels must be distinct, or an operator reading a log cannot
        // tell which happened.
        let labels = [busy.label(), dead.label(), unknown.label()];
        let mut sorted = labels;
        sorted.sort_unstable();
        let mut dedup = sorted.to_vec();
        dedup.dedup();
        assert_eq!(dedup.len(), 3, "labels collide: {labels:?}");
    }
}

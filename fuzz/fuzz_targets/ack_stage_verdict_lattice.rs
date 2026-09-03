#![no_main]
//! INVARIANT (bead zlyy, K1): a receiver verdict that is not `ReceiptConfirmed` NEVER produces
//! `RecordReceipt`; `Indeterminate` ALWAYS waits for a human and never re-sends; a `NoReceipt`
//! re-sends at most `MAX_RETRY_ATTEMPTS` times and then exhausts; the pane id is carried
//! verbatim into the action. The three-valued verdict must survive into the action — collapsing
//! it (INDETERMINATE rendered as FAILED at `main.rs:2952`) is the defect this target pins on
//! the kernel half of that path.
//!
//! Shape: frankenterm/fuzz/fuzz_targets/wire_envelope.rs — an Arbitrary input, a reference model
//! IN the target, `assert_eq!(actual, expected)` on every sample.

use ack_stage::{decide, AckAction, MAX_RETRY_ATTEMPTS};
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use receiver_receipt::{ReceiptReason, ReceiptVerdict};

const MAX_PANE_ID_LEN: usize = 16;

/// Every `ReceiptReason` variant, generated structurally so the fuzzer reaches each arm of
/// `decide` rather than mutating bytes into one.
#[derive(Arbitrary, Debug, Clone)]
enum Reason {
    MissingPostObservation,
    PaneIdMismatch { expected: u8, observed: u8 },
    DialogOpen,
    WedgedUnsubmitted,
    EmptyPaneListNoDeathClaim,
    ObservationNotWorking { pre: bool, state: u8 },
    StableContentUnchanged,
    UnprovenTransport { ntm: bool },
    AckReadbackMissing,
    IdleUnchanged,
    PostBecameIdle,
    TimerDidNotReset { before: u64, after: u64 },
    TimerResetButStableContentUnchanged,
    TimerTooLargeAfterIdle { after: u64, max: u64 },
    ObservationWindowTooShort { span: u64, floor: u64 },
}

impl Reason {
    fn into_real(self) -> ReceiptReason {
        match self {
            Reason::MissingPostObservation => ReceiptReason::MissingPostObservation,
            Reason::PaneIdMismatch { expected, observed } => ReceiptReason::PaneIdMismatch {
                expected: format!("%{expected}"),
                observed: format!("%{observed}"),
            },
            Reason::DialogOpen => ReceiptReason::DialogOpen,
            Reason::WedgedUnsubmitted => ReceiptReason::WedgedUnsubmitted,
            Reason::EmptyPaneListNoDeathClaim => ReceiptReason::EmptyPaneListNoDeathClaim,
            Reason::ObservationNotWorking { pre, state } => ReceiptReason::ObservationNotWorking {
                side: if pre { "pre" } else { "post" },
                state: format!("S{state}"),
            },
            Reason::StableContentUnchanged => ReceiptReason::StableContentUnchanged,
            Reason::UnprovenTransport { ntm } => ReceiptReason::UnprovenTransport {
                transport: if ntm { "ntm_robot_send" } else { "tmux_send_keys_literal" },
            },
            Reason::AckReadbackMissing => ReceiptReason::AckReadbackMissing,
            Reason::IdleUnchanged => ReceiptReason::IdleUnchanged,
            Reason::PostBecameIdle => ReceiptReason::PostBecameIdle,
            Reason::TimerDidNotReset { before, after } => ReceiptReason::TimerDidNotReset {
                before_secs: before,
                after_secs: after,
            },
            Reason::TimerResetButStableContentUnchanged => {
                ReceiptReason::TimerResetButStableContentUnchanged
            }
            Reason::TimerTooLargeAfterIdle { after, max } => ReceiptReason::TimerTooLargeAfterIdle {
                after_secs: after,
                max_secs: max,
            },
            Reason::ObservationWindowTooShort { span, floor } => {
                ReceiptReason::ObservationWindowTooShort {
                    span_secs: span,
                    floor_secs: floor,
                }
            }
        }
    }
}

#[derive(Arbitrary, Debug)]
enum Verdict {
    Confirmed { timer_before: Option<u64>, timer_after: u64, changed: bool },
    NoReceipt(Reason),
    Dead,
    Indeterminate(Reason),
}

#[derive(Arbitrary, Debug)]
struct Input {
    pane: Vec<u8>,
    verdict: Verdict,
    attempts_so_far: u32,
}

/// The reference model: what `decide` MUST do, written independently of it.
#[derive(Debug, PartialEq, Eq)]
enum Expected {
    Record,
    Retry { attempt: u32 },
    Exhausted { attempts: u32 },
    Unstick,
    AwaitHuman,
    Abandon,
}

fn model(verdict: &Verdict, attempts: u32) -> Expected {
    match verdict {
        Verdict::Confirmed { .. } => Expected::Record,
        Verdict::Dead => Expected::Abandon,
        Verdict::Indeterminate(_) => Expected::AwaitHuman,
        Verdict::NoReceipt(Reason::WedgedUnsubmitted) => Expected::Unstick,
        Verdict::NoReceipt(_) if attempts < MAX_RETRY_ATTEMPTS => Expected::Retry { attempt: attempts + 1 },
        Verdict::NoReceipt(_) => Expected::Exhausted { attempts },
    }
}

fn actual(action: &AckAction) -> Expected {
    match action {
        AckAction::RecordReceipt { .. } => Expected::Record,
        AckAction::Retry { attempt, .. } => Expected::Retry { attempt: *attempt },
        AckAction::RetryExhausted { attempts, .. } => Expected::Exhausted { attempts: *attempts },
        AckAction::Unstick { .. } => Expected::Unstick,
        AckAction::AwaitHuman { .. } => Expected::AwaitHuman,
        AckAction::AbandonDeadPane { .. } => Expected::Abandon,
    }
}

fn action_pane(action: &AckAction) -> &str {
    match action {
        AckAction::RecordReceipt { pane_id }
        | AckAction::Retry { pane_id, .. }
        | AckAction::Unstick { pane_id, .. }
        | AckAction::AwaitHuman { pane_id, .. }
        | AckAction::AbandonDeadPane { pane_id }
        | AckAction::RetryExhausted { pane_id, .. } => pane_id,
    }
}

fuzz_target!(|input: Input| {
    let pane_id = format!("%{}", String::from_utf8_lossy(&input.pane).chars().take(MAX_PANE_ID_LEN).collect::<String>());
    let verdict = match &input.verdict {
        Verdict::Confirmed { timer_before, timer_after, changed } => ReceiptVerdict::ReceiptConfirmed {
            pane_id: pane_id.clone(),
            timer_before_secs: *timer_before,
            timer_after_secs: *timer_after,
            stable_content_changed: *changed,
        },
        Verdict::NoReceipt(reason) => ReceiptVerdict::NoReceipt {
            pane_id: pane_id.clone(),
            reason: reason.clone().into_real(),
        },
        Verdict::Dead => ReceiptVerdict::Dead { pane_id: pane_id.clone() },
        Verdict::Indeterminate(reason) => ReceiptVerdict::Indeterminate {
            pane_id: pane_id.clone(),
            reason: reason.clone().into_real(),
        },
    };

    let action = decide(&verdict, input.attempts_so_far);

    // Contract 1: the action equals the model's, for every verdict × attempt count.
    assert_eq!(actual(&action), model(&input.verdict, input.attempts_so_far), "decide diverged from the model for {verdict:?} attempts={}", input.attempts_so_far);
    // Contract 2 (K1): only a confirmed receipt records; nothing else may.
    assert_eq!(
        matches!(action, AckAction::RecordReceipt { .. }),
        matches!(verdict, ReceiptVerdict::ReceiptConfirmed { .. }),
        "RecordReceipt without ReceiptConfirmed (or vice versa): {verdict:?} -> {action:?}"
    );
    // Contract 3: INDETERMINATE never injects a packet.
    if matches!(verdict, ReceiptVerdict::Indeterminate { .. }) {
        assert!(!action.is_retry(), "an indeterminate verdict re-sent: {action:?}");
    }
    // Contract 4: the pane id is carried verbatim; no cross-pane leak.
    assert_eq!(action_pane(&action), pane_id, "pane id not carried into the action");
    // Contract 5: a retry's ordinal is bounded by the budget and monotone in attempts.
    if let AckAction::Retry { attempt, max_attempts, .. } = &action {
        assert!(*attempt <= *max_attempts && *max_attempts == MAX_RETRY_ATTEMPTS, "retry ordinal escaped the budget: {action:?}");
    }
    // Contract 6: labels are stable and total (a label the ledger cannot classify is a defect).
    let _ = action.label();
    assert_eq!(action.is_retry(), action.label() == "RETRY", "is_retry disagrees with its label");
});

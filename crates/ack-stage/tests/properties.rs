#![forbid(unsafe_code)]

//! Proptest floor for K1: `RecordReceipt` ⇔ `ReceiptConfirmed`.
//! Restates `fuzz/fuzz_targets/ack_stage_verdict_lattice.rs`.

use ack_stage::{decide, AckAction, MAX_RETRY_ATTEMPTS};
use proptest::prelude::*;
use receiver_receipt::{ReceiptReason, ReceiptVerdict};

#[derive(Debug, Clone)]
enum Verdict {
    Confirmed,
    NoReceiptWedged,
    NoReceiptOther,
    Dead,
    Indeterminate,
}

fn model(verdict: &Verdict, attempts: u32) -> &'static str {
    match verdict {
        Verdict::Confirmed => "record",
        Verdict::Dead => "abandon",
        Verdict::Indeterminate if attempts < MAX_RETRY_ATTEMPTS => "retry",
        Verdict::Indeterminate => "exhausted",
        Verdict::NoReceiptWedged => "unstick",
        Verdict::NoReceiptOther if attempts < MAX_RETRY_ATTEMPTS => "retry",
        Verdict::NoReceiptOther => "exhausted",
    }
}

fn actual(action: &AckAction) -> &'static str {
    match action {
        AckAction::RecordReceipt { .. } => "record",
        AckAction::Retry { .. } => "retry",
        AckAction::RetryExhausted { .. } => "exhausted",
        AckAction::Unstick { .. } => "unstick",
        AckAction::AwaitHuman { .. } => "await",
        AckAction::AbandonDeadPane { .. } => "abandon",
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

fn to_verdict(kind: &Verdict, pane_id: &str) -> ReceiptVerdict {
    match kind {
        Verdict::Confirmed => ReceiptVerdict::ReceiptConfirmed {
            pane_id: pane_id.to_owned(),
            timer_before_secs: None,
            timer_after_secs: 6,
            stable_content_changed: true,
        },
        Verdict::NoReceiptWedged => ReceiptVerdict::NoReceipt {
            pane_id: pane_id.to_owned(),
            reason: ReceiptReason::WedgedUnsubmitted,
        },
        Verdict::NoReceiptOther => ReceiptVerdict::NoReceipt {
            pane_id: pane_id.to_owned(),
            reason: ReceiptReason::IdleUnchanged,
        },
        Verdict::Dead => ReceiptVerdict::Dead {
            pane_id: pane_id.to_owned(),
        },
        Verdict::Indeterminate => ReceiptVerdict::Indeterminate {
            pane_id: pane_id.to_owned(),
            reason: ReceiptReason::AckReadbackMissing,
        },
    }
}

proptest! {
    #[test]
    fn k1_record_receipt_iff_receipt_confirmed(
        pane in "\\%[0-9]{1,4}",
        kind in prop_oneof![
            Just(Verdict::Confirmed),
            Just(Verdict::NoReceiptWedged),
            Just(Verdict::NoReceiptOther),
            Just(Verdict::Dead),
            Just(Verdict::Indeterminate),
        ],
        attempts in 0u32..=6,
    ) {
        let verdict = to_verdict(&kind, &pane);
        let action = decide(&verdict, attempts);
        prop_assert_eq!(actual(&action), model(&kind, attempts));
        prop_assert_eq!(
            matches!(action, AckAction::RecordReceipt { .. }),
            matches!(verdict, ReceiptVerdict::ReceiptConfirmed { .. })
        );
        if matches!(verdict, ReceiptVerdict::Indeterminate { .. }) {
            prop_assert_eq!(action.is_retry(), attempts < MAX_RETRY_ATTEMPTS);
        }
        prop_assert_eq!(action_pane(&action), pane.as_str());
        if let AckAction::Retry { attempt, max_attempts, .. } = &action {
            prop_assert!(*attempt <= *max_attempts);
            prop_assert_eq!(*max_attempts, MAX_RETRY_ATTEMPTS);
        }
        prop_assert_eq!(action.is_retry(), action.label() == "RETRY");
    }
}

#[test]
fn planted_indeterminate_never_records() {
    let verdict = ReceiptVerdict::Indeterminate {
        pane_id: "%9".into(),
        reason: ReceiptReason::AckReadbackMissing,
    };
    let action = decide(&verdict, 0);
    assert!(!matches!(action, AckAction::RecordReceipt { .. }));
}

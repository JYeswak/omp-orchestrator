#![forbid(unsafe_code)]
//! zlyy K1 — `DISPATCH_FAILED` must appear for a refused send and nowhere else.
//!
//! # The fixture is the measured incident
//!
//! `io3h -> %1414`, 2026-09-02 23:55:08Z:
//!
//! ```text
//! 23:55:04  DISPATCH_CLAIMED   io3h -> %1414 assignee=supervisor:67581
//! 23:55:08  DISPATCH_RESULT    status=DISPATCH_FAILED detail=ACK_STAGE_INDETERMINATE reason=unproven_transport
//! 23:55:12  TRACKER            BlueLantern: "ACK io3h on %1414 -- starting..."
//! ```
//!
//! The status word was published FOUR SECONDS before the worker's own ACK landed, and it
//! said the dispatch failed. It had not.

use ack_stage::{
    classify_dispatch, AckAction, AckReadbackVerdict, AckStageResult, DispatchVerdict,
    TmuxSendKeysMeasurement, TransportKind, TransportReceipt,
};
use receiver_receipt::{ReceiptReason, ReceiptVerdict};

/// The codex fallback: tmux literal input, which cannot carry a delivery claim.
fn tmux() -> TransportReceipt {
    TransportReceipt::TmuxSendKeysLiteral(TmuxSendKeysMeasurement {
        raw_json: String::new(),
        command: "tmux send-keys -t %1414 -l <packet>".into(),
        stdout: String::new(),
        stderr: String::new(),
        exit_code: Some(0),
    })
}

/// The ntm transport, which does retain a per-target receipt.
fn ntm() -> TransportReceipt {
    TransportReceipt::capture_ntm(
        br#"{"targets":["4"],"successful":["4"],"failed":[],"blocked":false}"#,
    )
    .expect("the measured ntm receipt shape parses")
}

fn confirmed() -> ReceiptVerdict {
    ReceiptVerdict::ReceiptConfirmed {
        pane_id: "%1414".into(),
        timer_before_secs: None,
        timer_after_secs: 6,
        stable_content_changed: true,
    }
}

fn indeterminate(reason: ReceiptReason) -> ReceiptVerdict {
    ReceiptVerdict::Indeterminate {
        pane_id: "%1414".into(),
        reason,
    }
}

/// THE LEG THE BEAD NAMES: a codex-transport dispatch must not produce the literal
/// `DISPATCH_FAILED`.
#[test]
fn a_codex_transport_dispatch_never_publishes_dispatch_failed() {
    // Exactly the measured row: a confirmed receiver signal on the tmux fallback, which
    // `assess` downgrades to Indeterminate because the transport cannot prove delivery.
    let result = AckStageResult {
        action: AckAction::RecordReceipt {
            pane_id: "%1414".into(),
        },
        delivery: indeterminate(ReceiptReason::UnprovenTransport {
            transport: TransportKind::TmuxSendKeysLiteral.label(),
        }),
        transport: tmux(),
        ack_comment: None,
        ack_verdict: AckReadbackVerdict::Missing,
    };
    let verdict = classify_dispatch(&result, 30);
    assert_eq!(verdict.status_word(), "DISPATCH_INDETERMINATE");
    assert_ne!(
        verdict.status_word(),
        "DISPATCH_FAILED",
        "the io3h row said DISPATCH_FAILED four seconds before the ACK landed"
    );
    assert!(!verdict.is_failure());
    assert!(
        verdict.detail().contains("tmux_send_keys_literal"),
        "the reader must be told WHICH transport could not carry the claim: {}",
        verdict.detail()
    );
    // And the conservative direction: an unprovable transport must not authorize a
    // delivery claim, even though it is not a failure. Three states, not two.
    assert!(!verdict.packet_is_with_the_receiver());
}

/// The leg M2 exposed: on an unprovable transport, a RETRY must not become `ACK_PENDING`.
///
/// # Why the first fixture was not enough
///
/// `a_codex_transport_dispatch_never_publishes_dispatch_failed` builds its result with
/// `delivery: Indeterminate` — already downgraded, exactly as `assess` would leave it. So
/// `classify_dispatch`'s own transport check is REDUNDANT for that input: the
/// `RecordReceipt` arm returns `Indeterminate` anyway. A mutation that deleted the
/// transport check (`if false`) left every leg GREEN, which means the leg was passing for
/// the wrong reason and proving nothing about the check it was written for.
///
/// This fixture is the case where the two paths DIVERGE: no receiver evidence at all
/// (`action = Retry`) on the tmux fallback. With the check, the verdict is
/// `DISPATCH_INDETERMINATE` and the packet is NOT claimed to be with the receiver. Without
/// it, the `Retry` arm answers `ACK_PENDING`, whose contract says the pane HOLDS the
/// packet — a delivery claim on a transport that by construction cannot support one.
#[test]
fn an_unprovable_transport_is_indeterminate_even_when_the_action_is_retry() {
    let result = AckStageResult {
        action: AckAction::Retry {
            pane_id: "%1414".into(),
            attempt: 1,
            max_attempts: 3,
        },
        delivery: ReceiptVerdict::NoReceipt {
            pane_id: "%1414".into(),
            reason: ReceiptReason::AckReadbackMissing,
        },
        transport: tmux(),
        ack_comment: None,
        ack_verdict: AckReadbackVerdict::Missing,
    };
    let verdict = classify_dispatch(&result, 30);
    assert_eq!(
        verdict.status_word(),
        "DISPATCH_INDETERMINATE",
        "the transport check must outrank the action; got {}",
        verdict.detail()
    );
    assert_ne!(
        verdict.status_word(),
        "ACK_PENDING",
        "ACK_PENDING asserts the pane holds the packet, which tmux literal input cannot show"
    );
    assert!(
        !verdict.packet_is_with_the_receiver(),
        "no delivery claim may be derived from a transport that cannot carry one"
    );
    assert!(
        verdict.detail().contains("tmux_send_keys_literal"),
        "{}",
        verdict.detail()
    );

    // POSITIVE CONTROL, one field different: the SAME action on the ntm transport — which
    // does retain a per-target receipt — IS `ACK_PENDING`. So the refusal above is
    // attributable to the transport and not to the action.
    let on_ntm = AckStageResult {
        transport: ntm(),
        ..result
    };
    assert_eq!(classify_dispatch(&on_ntm, 30).status_word(), "ACK_PENDING");
}

/// A late ACK is `ACK_PENDING`, and the code's own comment already said so.
///
/// `main.rs` at the `ACK_STAGE_RETRY_BLOCKED` site: *"the ack is late, not absent"*, with
/// `owes_human=false`. It then returned `Err` and the ledger printed `DISPATCH_FAILED`.
#[test]
fn a_late_ack_is_pending_and_the_packet_is_with_the_receiver() {
    let result = AckStageResult {
        action: AckAction::Retry {
            pane_id: "%1414".into(),
            attempt: 2,
            max_attempts: 3,
        },
        delivery: indeterminate(ReceiptReason::AckReadbackMissing),
        transport: ntm(),
        ack_comment: None,
        ack_verdict: AckReadbackVerdict::Missing,
    };
    let verdict = classify_dispatch(&result, 30);
    assert_eq!(verdict.status_word(), "ACK_PENDING");
    assert!(!verdict.is_failure());
    assert!(
        verdict.packet_is_with_the_receiver(),
        "ntm retained a per-target receipt; the pane holds the packet and the ACK is late"
    );
    assert!(verdict.detail().contains("after=30s"), "{}", verdict.detail());
    assert!(verdict.detail().contains("attempt=2"), "{}", verdict.detail());
}

/// KNOWN-GOOD: a proven delivery is `DISPATCH_DELIVERED`.
///
/// Without this leg an over-strict classifier — one that answered `Indeterminate` for
/// everything — would pass every assertion above, and an over-strict gate gets routed
/// around.
#[test]
fn a_proven_delivery_is_published_as_delivered() {
    let result = AckStageResult {
        action: AckAction::RecordReceipt {
            pane_id: "%1414".into(),
        },
        delivery: confirmed(),
        transport: ntm(),
        ack_comment: Some("ACK io3h on %1414 -- starting...".into()),
        ack_verdict: AckReadbackVerdict::Matched {
            comment: "ACK io3h on %1414 -- starting...".into(),
        },
    };
    assert!(
        result.is_confirmed(),
        "ack-stage's OWN predicate must accept this fixture, or the projection below \
         proves nothing about the crate it projects"
    );
    let verdict = classify_dispatch(&result, 30);
    assert_eq!(verdict.status_word(), "DISPATCH_DELIVERED");
    assert!(verdict.packet_is_with_the_receiver());
    assert!(!verdict.is_failure());
}

/// A genuine refused send IS a failure, and is the ONLY thing that says so.
#[test]
fn only_a_refused_send_publishes_dispatch_failed() {
    let failed = DispatchVerdict::Failed("TIMEOUT program=tmux after=30s".into());
    assert_eq!(failed.status_word(), "DISPATCH_FAILED");
    assert!(failed.is_failure());
    assert!(!failed.packet_is_with_the_receiver());
    assert_eq!(failed.detail(), "TIMEOUT program=tmux after=30s");

    // An abandoned pane and a dialog awaiting a human are also failures: the packet is
    // not going to be worked, and a reader must not wait for an ACK that cannot come.
    let dead = AckStageResult {
        action: AckAction::AbandonDeadPane {
            pane_id: "%1414".into(),
        },
        delivery: ReceiptVerdict::NoReceipt {
            pane_id: "%1414".into(),
            reason: ReceiptReason::AckReadbackMissing,
        },
        transport: ntm(),
        ack_comment: None,
        ack_verdict: AckReadbackVerdict::Missing,
    };
    assert_eq!(
        classify_dispatch(&dead, 30).status_word(),
        "DISPATCH_FAILED"
    );
}

/// EXHAUSTIVE: every variant's status word, and exactly one of them is `DISPATCH_FAILED`.
///
/// The property is stated as a count rather than four separate equalities, so a fifth
/// variant added later cannot quietly acquire the failure word.
#[test]
fn exactly_one_variant_publishes_the_failure_word() {
    let variants = [
        DispatchVerdict::Delivered(AckStageResult {
            action: AckAction::RecordReceipt {
                pane_id: "%1414".into(),
            },
            delivery: confirmed(),
            transport: ntm(),
            ack_comment: Some("ACK".into()),
            ack_verdict: AckReadbackVerdict::Matched {
                comment: "ACK".into(),
            },
        }),
        DispatchVerdict::Indeterminate {
            reason: "unproven_transport".into(),
            transport: TransportKind::TmuxSendKeysLiteral,
        },
        DispatchVerdict::AckPending {
            after_secs: 30,
            discriminator: "attempt=1".into(),
        },
        DispatchVerdict::Failed("TIMEOUT program=tmux".into()),
    ];
    let words: Vec<&str> = variants.iter().map(DispatchVerdict::status_word).collect();
    assert_eq!(
        words.iter().filter(|word| **word == "DISPATCH_FAILED").count(),
        1,
        "exactly one variant may publish the failure word: {words:?}"
    );
    // ANTI-VACUITY: four distinct words, so the count above is not satisfied by a
    // classifier that returns the same string for everything.
    let distinct: std::collections::BTreeSet<&str> = words.iter().copied().collect();
    assert_eq!(distinct.len(), 4, "{words:?}");
    assert_eq!(
        variants.iter().filter(|v| v.is_failure()).count(),
        1,
        "is_failure must agree with the word"
    );
}

/// The projection may never be more generous than the crate it projects.
///
/// `is_confirmed()` is ack-stage's own definition of proven delivery. If a fixture that
/// FAILS it were published as `Delivered`, this crate would be claiming a delivery its
/// owner refuses — the overclaim this whole bead is about, pointed the other way.
#[test]
fn nothing_is_delivered_unless_ack_stage_itself_confirms_it() {
    // Every near-miss: right action, right verdict, but one missing ingredient each.
    let bases = [
        // No ACK comment.
        AckStageResult {
            action: AckAction::RecordReceipt {
                pane_id: "%1414".into(),
            },
            delivery: confirmed(),
            transport: ntm(),
            ack_comment: None,
            ack_verdict: AckReadbackVerdict::Missing,
        },
        // Right ACK, wrong transport.
        AckStageResult {
            action: AckAction::RecordReceipt {
                pane_id: "%1414".into(),
            },
            delivery: confirmed(),
            transport: tmux(),
            ack_comment: Some("ACK".into()),
            ack_verdict: AckReadbackVerdict::Matched {
                comment: "ACK".into(),
            },
        },
    ];
    for base in &bases {
        assert!(
            !base.is_confirmed(),
            "fixture must be a near-miss for this leg to mean anything"
        );
        assert_ne!(
            classify_dispatch(base, 30).status_word(),
            "DISPATCH_DELIVERED",
            "projected Delivered from a result ack-stage refuses to confirm"
        );
    }
}

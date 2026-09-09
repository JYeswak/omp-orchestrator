#![no_main]
//! INVARIANT (bead nuxd): ack_stage::assess composition.
//! (a) RecordReceipt iff ntm transport and a matching ACK comment.
//! (b) A tmux transport never yields RecordReceipt.
//! (c) ack_comment is the matched comment or None.
//! (d) pane id is carried verbatim.
//! (e) never panics.
//!
//! The deterministic seed format is consumed by this same libFuzzer target:
//! nuxd-near-miss-v1 index=N issue=... expect=RESTRICTIVE_ACK_NO_MATCH.
//! The 20 ACK lines are copied from live bead comments and retain their issue
//! id and expected restrictive class in the seed corpus.

use ack_stage::{
    assess, AckAction, AckComment, AckReadback, AckReadbackVerdict, AckStageInput,
    NtmRobotSendReceipt, TmuxSendKeysMeasurement, TransportReceipt,
};
use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use receiver_receipt::{PostSendObservation, ReceiptVerdict};
use tick_monitor::{Observation, PaneState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedClass {
    RestrictiveNoMatch,
}

const NEAR_MISS_COUNT: usize = 20;
const NEAR_MISS_ISSUES: &[&str] = &[
    "omp-orchestrator-ack-spine-oj6.2",
    "omp-orchestrator-gate-no-runner-j58",
    "omp-orchestrator-kernel-gate-census-69i",
    "omp-orchestrator-crate-reachability-census-44g",
    "omp-orchestrator-crate-reachability-census-44g",
    "omp-orchestrator-followup-stage-180",
    "omp-orchestrator-commit-msg-backtick-injection-232",
    "omp-orchestrator-leht",
    "omp-orchestrator-eg0m",
    "omp-orchestrator-kxe.1",
    "omp-orchestrator-typed-degraded-dispatch-policy-i0kp",
    "omp-orchestrator-ack-spine-oj6.3",
    "omp-orchestrator-omp-coverage-mission-ipg.19",
    "omp-orchestrator-receiver-receipt-contract-pwm",
    "omp-orchestrator-remove-unattributed-scratch-fallback-7bp",
    "omp-orchestrator-installer-dep-uncommitted-txl",
    "omp-orchestrator-finding-l1-bypassable-py3",
    "omp-orchestrator-readiness-l1-wedge-blind-46y7",
    "omp-orchestrator-observation-state-false-idle-riqd",
    "omp-orchestrator-monitor-reads-oracle-y256",
];
const NEAR_MISS_FILES: &[&str] = &[
    include_str!("../corpus/ack_stage_assess/seed-near-miss-01"),
    include_str!("../corpus/ack_stage_assess/seed-near-miss-02"),
    include_str!("../corpus/ack_stage_assess/seed-near-miss-03"),
    include_str!("../corpus/ack_stage_assess/seed-near-miss-04"),
    include_str!("../corpus/ack_stage_assess/seed-near-miss-05"),
    include_str!("../corpus/ack_stage_assess/seed-near-miss-06"),
    include_str!("../corpus/ack_stage_assess/seed-near-miss-07"),
    include_str!("../corpus/ack_stage_assess/seed-near-miss-08"),
    include_str!("../corpus/ack_stage_assess/seed-near-miss-09"),
    include_str!("../corpus/ack_stage_assess/seed-near-miss-10"),
    include_str!("../corpus/ack_stage_assess/seed-near-miss-11"),
    include_str!("../corpus/ack_stage_assess/seed-near-miss-12"),
    include_str!("../corpus/ack_stage_assess/seed-near-miss-13"),
    include_str!("../corpus/ack_stage_assess/seed-near-miss-14"),
    include_str!("../corpus/ack_stage_assess/seed-near-miss-15"),
    include_str!("../corpus/ack_stage_assess/seed-near-miss-16"),
    include_str!("../corpus/ack_stage_assess/seed-near-miss-17"),
    include_str!("../corpus/ack_stage_assess/seed-near-miss-18"),
    include_str!("../corpus/ack_stage_assess/seed-near-miss-19"),
    include_str!("../corpus/ack_stage_assess/seed-near-miss-20"),
];
#[derive(Arbitrary, Debug, Clone, Copy)]
enum TransportKindFuzz {
    Ntm,
    Tmux,
}

#[derive(Arbitrary, Debug, Clone, Copy)]
enum ReceiverKindFuzz {
    PresentWorking,
    PresentIdle,
    Missing,
    Absent,
    EmptyPaneList,
}

#[derive(Arbitrary, Debug, Clone, Copy)]
enum CommentKind {
    Exact,
    NearMiss(u8),
    Prose,
}

#[derive(Arbitrary, Debug)]
struct Input {
    transport: TransportKindFuzz,
    receiver: ReceiverKindFuzz,
    comment: CommentKind,
    hash_changed: bool,
    attempts: u8,
}

fn obs(pane: &str, working: bool, hash: u64, at: u64, seq: u64) -> Observation {
    Observation {
        pane_id: pane.to_owned(),
        state: if working {
            PaneState::Working { timer_secs: 1 }
        } else {
            PaneState::Idle
        },
        hash,
        at,
        epoch: "fuzz".into(),
        sequence: seq,
        changed_at: "0".into(),
    }
}

fn post_observation(input: &Input, pane: &str) -> PostSendObservation {
    match input.receiver {
        ReceiverKindFuzz::PresentWorking => PostSendObservation::Present(obs(
            pane,
            true,
            if input.hash_changed { 2 } else { 1 },
            110,
            2,
        )),
        ReceiverKindFuzz::PresentIdle => PostSendObservation::Present(obs(pane, false, 1, 110, 2)),
        ReceiverKindFuzz::Missing => PostSendObservation::Missing,
        ReceiverKindFuzz::Absent => PostSendObservation::Absent,
        ReceiverKindFuzz::EmptyPaneList => PostSendObservation::EmptyPaneList,
    }
}

fn replay_seed(data: &[u8]) -> Option<Input> {
    let first_line = data.split(|byte| *byte == b'\n').next()?;
    if first_line.starts_with(b"nuxd-known-good-v1") {
        return Some(Input {
            transport: TransportKindFuzz::Ntm,
            receiver: ReceiverKindFuzz::PresentWorking,
            comment: CommentKind::Exact,
            hash_changed: true,
            attempts: 0,
        });
    }
    if first_line.starts_with(b"nuxd-known-bad-v1") {
        return Some(Input {
            transport: TransportKindFuzz::Tmux,
            receiver: ReceiverKindFuzz::PresentWorking,
            comment: CommentKind::Exact,
            hash_changed: true,
            attempts: 0,
        });
    }
    let rest = first_line.strip_prefix(b"nuxd-near-miss-v1 index=")?;
    let digits = rest.split(|byte| *byte == b' ').next()?;
    let index = std::str::from_utf8(digits).ok()?.parse::<usize>().ok()?;
    (index < NEAR_MISS_COUNT).then_some(Input {
        transport: TransportKindFuzz::Ntm,
        receiver: ReceiverKindFuzz::PresentWorking,
        comment: CommentKind::NearMiss(index as u8),
        hash_changed: true,
        attempts: 0,
    })
}

fn run_case(input: Input) {
    let bead = "omp-orchestrator-nuxd";
    let pane = "%9";
    let exact = format!("ACK nuxd on {pane} -- fuzz");
    let (comment_text, expected) = match input.comment {
        CommentKind::Exact => (exact, None),
        CommentKind::NearMiss(index) => {
            let index = index as usize % NEAR_MISS_COUNT;
            let issue_id = NEAR_MISS_ISSUES[index];
            let encoded = NEAR_MISS_FILES[index];
            let comment = encoded
                .split_once('\n')
                .and_then(|(_, rest)| rest.lines().next())
                .expect("near-miss seed ACK line");
            assert!(
                !issue_id.is_empty(),
                "near-miss seed provenance missing at index={index}"
            );
            (comment.to_owned(), Some(ExpectedClass::RestrictiveNoMatch))
        }
        CommentKind::Prose => ("not an ack at all".to_owned(), None),
    };
    let transport = match input.transport {
        TransportKindFuzz::Tmux => TransportReceipt::TmuxSendKeysLiteral(TmuxSendKeysMeasurement {
            raw_json: "{}".into(),
            command: "tmux send-keys".into(),
            stdout: String::new(),
            stderr: String::new(),
            exit_code: Some(0),
        }),
        TransportKindFuzz::Ntm => TransportReceipt::NtmRobotSend(NtmRobotSendReceipt {
            raw_json: r#"{"blocked":false,"targets":["9"],"successful":["9"],"failed":[]}"#.into(),
            targets: vec!["9".into()],
            successful: vec!["9".into()],
            failed: vec![],
            blocked: false,
        }),
    };
    let stage = AckStageInput {
        bead_id: bead.into(),
        pane_id: pane.into(),
        transport,
        pre_send: obs(pane, false, 1, 100, 1),
        post_send: post_observation(&input, pane),
        ack: AckReadback {
            bead_id: bead.into(),
            pane_id: pane.into(),
            comments: vec![AckComment {
                text: comment_text.clone(),
                created_at: Some(200),
            }],
            dispatch_issued_at: None,
        },
        attempts_so_far: u32::from(input.attempts),
        session_pane_ids: vec![pane.into()],
    };
    let result = assess(&stage);
    let matched = comment_text.starts_with("ACK nuxd on %9 -- ");
    let ntm = matches!(input.transport, TransportKindFuzz::Ntm);
    let receiver_confirmed = matches!(
        result.delivery,
        ReceiptVerdict::ReceiptConfirmed { .. } | ReceiptVerdict::AckConfirmed { .. }
    );
    let records = matches!(result.action, AckAction::RecordReceipt { .. });
    assert_eq!(
        records,
        ntm && matched && receiver_confirmed,
        "assess RecordReceipt contract broke: input={input:?} action={:?}",
        result.action
    );
    if let Some(ExpectedClass::RestrictiveNoMatch) = expected {
        assert_eq!(result.ack_verdict, AckReadbackVerdict::Missing);
        assert_eq!(result.ack_comment, None);
        assert!(!records, "restrictive near-miss recorded: {result:?}");
    }
    match &result.action {
        AckAction::RecordReceipt { pane_id }
        | AckAction::Retry { pane_id, .. }
        | AckAction::Unstick { pane_id, .. }
        | AckAction::AwaitHuman { pane_id, .. }
        | AckAction::AbandonDeadPane { pane_id }
        | AckAction::RetryExhausted { pane_id, .. } => assert_eq!(pane_id, pane),
    }
}

fuzz_target!(|data: &[u8]| {
    if let Some(input) = replay_seed(data) {
        run_case(input);
        return;
    }
    let mut unstructured = Unstructured::new(data);
    if let Ok(input) = Input::arbitrary(&mut unstructured) {
        run_case(input);
    }
});

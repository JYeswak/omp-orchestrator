#![forbid(unsafe_code)]

use ack_stage::{
    assess, AckAction, AckComment, AckReadback, AckReadbackVerdict, AckStageInput,
    NtmRobotSendReceipt, TmuxSendKeysMeasurement, TransportReceipt,
};
use receiver_receipt::PostSendObservation;
use tick_monitor::{Observation, PaneState};

const NEAR_MISSES: &[&str] = &[
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
fn obs(pane: &str, working: bool, hash: u64, at: u64) -> Observation {
    Observation {
        pane_id: pane.into(),
        state: if working {
            PaneState::Working { timer_secs: 1 }
        } else {
            PaneState::Idle
        },
        hash,
        at,
        epoch: "test".into(),
        sequence: if working { 2 } else { 1 },
        changed_at: "0".into(),
    }
}

fn confirmed_pair(pane: &str) -> (Observation, PostSendObservation) {
    (
        obs(pane, false, 1, 100),
        PostSendObservation::Present(obs(pane, true, 2, 110)),
    )
}

fn ntm() -> TransportReceipt {
    TransportReceipt::NtmRobotSend(NtmRobotSendReceipt {
        raw_json: r#"{"blocked":false,"targets":["9"],"successful":["9"],"failed":[]}"#.into(),
        targets: vec!["9".into()],
        successful: vec!["9".into()],
        failed: vec![],
        blocked: false,
    })
}

fn tmux() -> TransportReceipt {
    TransportReceipt::TmuxSendKeysLiteral(TmuxSendKeysMeasurement {
        raw_json: "{}".into(),
        command: "tmux send-keys".into(),
        stdout: String::new(),
        stderr: String::new(),
        exit_code: Some(0),
    })
}

fn stage_with_post(
    bead: &str,
    pane: &str,
    transport: TransportReceipt,
    post_send: PostSendObservation,
    comment: &str,
) -> AckStageInput {
    AckStageInput {
        bead_id: bead.into(),
        pane_id: pane.into(),
        transport,
        pre_send: obs(pane, false, 1, 100),
        post_send,
        ack: AckReadback {
            bead_id: bead.into(),
            pane_id: pane.into(),
            comments: vec![AckComment {
                text: comment.into(),
                created_at: Some(200),
            }],
            dispatch_issued_at: None,
        },
        attempts_so_far: 0,
        session_pane_ids: vec![pane.into()],
    }
}

fn stage(bead: &str, pane: &str, transport: TransportReceipt, comment: &str) -> AckStageInput {
    let (_, post_send) = confirmed_pair(pane);
    stage_with_post(bead, pane, transport, post_send, comment)
}
/// Fires-on-known-bad: tmux + confirmed receiver + matching ACK must not RecordReceipt.
/// Mutation: make TmuxSendKeysLiteral support a delivery claim; this goes RED.
#[test]
fn tmux_transport_never_records_receipt() {
    let comment = "ACK nuxd on %9 -- fuzz";
    let result = assess(&stage("omp-orchestrator-nuxd", "%9", tmux(), comment));
    assert!(
        !matches!(result.action, AckAction::RecordReceipt { .. }),
        "tmux claimed delivery: {result:?}"
    );
}

#[test]
fn ntm_plus_matching_ack_records() {
    let comment = "ACK nuxd on %9 -- fuzz";
    let result = assess(&stage("omp-orchestrator-nuxd", "%9", ntm(), comment));
    assert!(matches!(result.action, AckAction::RecordReceipt { .. }));
    assert_eq!(result.ack_comment.as_deref(), Some(comment));
}

#[test]
fn twenty_live_near_misses_do_not_match() {
    assert_eq!(NEAR_MISSES.len(), 20);
    let pane = "%9";
    let corpus_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus/ack_stage_assess");
    for (index, issue_id) in NEAR_MISSES.iter().enumerate() {
        let corpus =
            std::fs::read_to_string(corpus_dir.join(format!("seed-near-miss-{:02}", index + 1)))
                .expect("committed near-miss seed");
        assert!(corpus.contains(issue_id), "{corpus}");
        assert!(
            corpus.contains("expect=RESTRICTIVE_ACK_NO_MATCH"),
            "{corpus}"
        );
        let text = corpus.lines().nth(1).expect("seed ACK first line");
        assert!(!text.is_empty(), "{corpus}");

        let posts = [
            PostSendObservation::Present(obs(pane, true, 2, 110)),
            PostSendObservation::Missing,
            PostSendObservation::Absent,
            PostSendObservation::EmptyPaneList,
        ];
        for transport in [ntm(), tmux()] {
            for post_send in posts.clone() {
                let result = assess(&stage_with_post(
                    issue_id,
                    pane,
                    transport.clone(),
                    post_send,
                    text,
                ));
                assert!(
                    matches!(result.ack_verdict, AckReadbackVerdict::Missing),
                    "near-miss matched for {}: {result:?}",
                    issue_id
                );
                assert!(
                    !matches!(result.action, AckAction::RecordReceipt { .. }),
                    "near-miss produced RecordReceipt for {}: {result:?}",
                    issue_id
                );
                assert_eq!(result.ack_comment, None);
            }
        }
    }
}

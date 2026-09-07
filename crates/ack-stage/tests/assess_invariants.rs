#![forbid(unsafe_code)]

use ack_stage::{
    assess, AckAction, AckComment, AckReadback, AckReadbackVerdict, AckStageInput,
    NtmRobotSendReceipt, TransportReceipt, TmuxSendKeysMeasurement,
};
use receiver_receipt::PostSendObservation;
use tick_monitor::{Observation, PaneState};

const NEAR_MISSES: &[(&str, &str)] = &[
    ("omp-orchestrator-ack-spine-oj6.2", "ACK DETECTOR LANDED (SilverWolf, pane 5, %1409) — scripts/ack-detector.sh"),
    ("omp-orchestrator-gate-no-runner-j58", "ACK j58 received on %1408 -- I will read the pre-commit hook contract"),
    ("omp-orchestrator-kernel-gate-census-69i", "ACK 69i on %1408 — I will build the gate-reachability census"),
    ("omp-orchestrator-crate-reachability-census-44g", "ACK 44g on %1409 — SilverWolf. Read the census."),
    ("omp-orchestrator-crate-reachability-census-44g", "ACK 44g on %1408 — first step: fix the duplicate gate-census block"),
    ("omp-orchestrator-followup-stage-180", "ACK 180 on %1409 — SilverWolf. First step: writing the pure follow-up"),
    ("omp-orchestrator-commit-msg-backtick-injection-232", "ACK from AmberGate (%1408): the rule is adopted"),
    ("omp-orchestrator-leht", "ACK leht-grade on %1413 agent=GreenFrog title=omp-orchestrator__cod_1"),
    ("omp-orchestrator-eg0m", "ACK eg0m on %1408"),
    ("omp-orchestrator-kxe.1", "ACK kxe.1 on %1414"),
    ("omp-orchestrator-typed-degraded-dispatch-policy-i0kp", "ACK i0kp on %1413"),
    ("omp-orchestrator-ack-spine-oj6.3", "ACK oj6.3 on %1414"),
    ("omp-orchestrator-omp-coverage-mission-ipg.19", "ACK ipg.19 on %1408"),
    ("omp-orchestrator-receiver-receipt-contract-pwm", "ACK pwm on %1408"),
    ("omp-orchestrator-remove-unattributed-scratch-fallback-7bp", "ACK 7bp on %1408"),
    ("omp-orchestrator-installer-dep-uncommitted-txl", "ACK txl on %1408"),
    ("omp-orchestrator-finding-l1-bypassable-py3", "ACK py3 on %1408"),
    ("omp-orchestrator-readiness-l1-wedge-blind-46y7", "ACK 46y7 on %1408"),
    ("omp-orchestrator-observation-state-false-idle-riqd", "ACK riqd on %1408"),
    ("omp-orchestrator-monitor-reads-oracle-y256", "ACK y256 on %1408"),
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

fn stage(
    bead: &str,
    pane: &str,
    transport: TransportReceipt,
    comment: &str,
) -> AckStageInput {
    let (pre, post) = confirmed_pair(pane);
    AckStageInput {
        bead_id: bead.into(),
        pane_id: pane.into(),
        transport,
        pre_send: pre,
        post_send: post,
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
    for (bead, text) in NEAR_MISSES {
        let pane = "%9";
        let ack = AckReadback {
            bead_id: (*bead).into(),
            pane_id: pane.into(),
            comments: vec![AckComment {
                text: (*text).into(),
                created_at: Some(200),
            }],
            dispatch_issued_at: None,
        };
        assert!(
            matches!(ack.match_verdict_for(bead, pane), AckReadbackVerdict::Missing),
            "near-miss unexpectedly matched for {bead}: {text}"
        );
        let result = assess(&stage(bead, pane, ntm(), text));
        assert!(
            !matches!(result.action, AckAction::RecordReceipt { .. }),
            "near-miss produced RecordReceipt for {bead}"
        );
        assert_eq!(result.ack_comment, None);
    }
}

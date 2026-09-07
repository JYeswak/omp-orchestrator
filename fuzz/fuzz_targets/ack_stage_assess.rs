#![no_main]
//! INVARIANT (bead nuxd): `ack_stage::assess` composition.
//! (a) RecordReceipt ⇔ ntm transport ∧ a matching ACK comment
//!     (AckConfirmed path; tmux cannot take it).
//! (b) a tmux transport never yields RecordReceipt.
//! (c) `ack_comment` is the matched comment or None.
//! (d) pane id is carried verbatim.
//! (e) never panics.
//!
//! Model is written from `assess` (tmux INDETERMINATE by construction;
//! confirmed receipt without ACK → AckReadbackMissing).

use ack_stage::{
    assess, AckAction, AckComment, AckReadback, AckStageInput, NtmRobotSendReceipt,
    TransportReceipt, TmuxSendKeysMeasurement,
};
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use receiver_receipt::PostSendObservation;
use tick_monitor::{Observation, PaneState};

const NEAR_MISSES: &[&str] = &[
    "ACK DETECTOR LANDED (SilverWolf, pane 5, %1409) — scripts/ack-detector.sh",
    "ACK j58 received on %1408 -- I will read the pre-commit hook contract",
    "ACK 69i on %1408 — I will build the gate-reachability census",
    "ACK 44g on %1409 — SilverWolf. Read the census.",
    "ACK 44g on %1408 — first step: fix the duplicate gate-census block",
    "ACK 180 on %1409 — SilverWolf. First step: writing the pure follow-up",
    "ACK from AmberGate (%1408): the rule is adopted",
    "ACK leht-grade on %1413 agent=GreenFrog title=omp-orchestrator__cod_1",
    "ACK eg0m on %1408",
    "ACK kxe.1 on %1414",
    "ACK i0kp on %1413",
    "ACK oj6.3 on %1414",
    "ACK ipg.19 on %1408",
    "ACK pwm on %1408",
    "ACK 7bp on %1408",
    "ACK txl on %1408",
    "ACK py3 on %1408",
    "ACK 46y7 on %1408",
    "ACK riqd on %1408",
    "ACK y256 on %1408",
];

#[derive(Arbitrary, Debug, Clone, Copy)]
enum TransportKindFuzz {
    Ntm,
    Tmux,
}

#[derive(Arbitrary, Debug, Clone, Copy)]
enum CommentKind {
    Exact,
    NearMiss(u8),
    Prose,
}

#[derive(Arbitrary, Debug)]
struct Input {
    tmux: bool,
    comment: CommentKind,
    idle_to_working: bool,
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

fuzz_target!(|input: Input| {
    let bead = "omp-orchestrator-nuxd";
    let pane = "%9";
    let exact = format!("ACK nuxd on {pane} -- fuzz");
    let comment_text = match input.comment {
        CommentKind::Exact => exact.as_str(),
        CommentKind::NearMiss(i) => NEAR_MISSES[i as usize % NEAR_MISSES.len()],
        CommentKind::Prose => "not an ack at all",
    };
    let transport = if input.tmux {
        TransportReceipt::TmuxSendKeysLiteral(TmuxSendKeysMeasurement {
            raw_json: "{}".into(),
            command: "tmux send-keys".into(),
            stdout: String::new(),
            stderr: String::new(),
            exit_code: Some(0),
        })
    } else {
        TransportReceipt::NtmRobotSend(NtmRobotSendReceipt {
            raw_json: r#"{"blocked":false,"targets":["9"],"successful":["9"],"failed":[]}"#.into(),
            targets: vec!["9".into()],
            successful: vec!["9".into()],
            failed: vec![],
            blocked: false,
        })
    };
    let pre = obs(pane, false, 1, 100, 1);
    let post = if input.idle_to_working {
        PostSendObservation::Present(obs(
            pane,
            true,
            if input.hash_changed { 2 } else { 1 },
            110,
            2,
        ))
    } else {
        PostSendObservation::Present(obs(pane, false, 1, 110, 2))
    };
    let ack = AckReadback {
        bead_id: bead.into(),
        pane_id: pane.into(),
        comments: vec![AckComment {
            text: comment_text.into(),
            created_at: Some(200),
        }],
        dispatch_issued_at: None,
    };
    let stage = AckStageInput {
        bead_id: bead.into(),
        pane_id: pane.into(),
        transport,
        pre_send: pre,
        post_send: post,
        ack,
        attempts_so_far: u32::from(input.attempts),
        session_pane_ids: vec![pane.into()],
    };
    let result = assess(&stage);

    let matched = comment_text.starts_with("ACK nuxd on %9 -- ");
    let ntm = !input.tmux;
    let records = matches!(result.action, AckAction::RecordReceipt { .. });

    // (a)(b) RecordReceipt ⇔ ntm ∧ matching ACK. Tmux never records.
    assert_eq!(records, ntm && matched, "assess RecordReceipt contract broke: tmux={} matched={} action={:?}", input.tmux, matched, result.action);
    if input.tmux {
        assert!(!records, "tmux transport claimed delivery: {result:?}");
    }

    // (c)
    if matched {
        assert_eq!(result.ack_comment.as_deref(), Some(comment_text));
    } else {
        assert_eq!(result.ack_comment, None);
    }

    // (d)
    match &result.action {
        AckAction::RecordReceipt { pane_id }
        | AckAction::Retry { pane_id, .. }
        | AckAction::Unstick { pane_id, .. }
        | AckAction::AwaitHuman { pane_id, .. }
        | AckAction::AbandonDeadPane { pane_id }
        | AckAction::RetryExhausted { pane_id, .. } => {
            assert_eq!(pane_id, pane);
        }
    }
});

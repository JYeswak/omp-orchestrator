#![forbid(unsafe_code)]

use ack_stage::{assess, AckReadback, AckReadbackError, AckStageInput, TransportReceipt};
use receiver_receipt::{
    classify_ack_wait, AckWaitVerdict, Observation, PaneState, PostSendObservation,
    ReceiptReason, ReceiptVerdict,
};

const BEAD: &str = "omp-orchestrator-ack-never-confirms-uhnr";
const PANE: &str = "%7";

fn ntm() -> TransportReceipt {
    TransportReceipt::capture_ntm(
        br#"{"targets":["7"],"successful":["7"],"failed":[],"blocked":false}"#,
    )
    .expect("ntm receipt fixture")
}

fn tmux() -> TransportReceipt {
    TransportReceipt::TmuxSendKeysLiteral(ack_stage::TmuxSendKeysMeasurement {
        raw_json: String::new(),
        command: "tmux send-keys".to_owned(),
        stdout: String::new(),
        stderr: String::new(),
        exit_code: Some(0),
    })
}

fn observation(state: PaneState, at: u64, hash: u64) -> Observation {
    Observation {
        pane_id: PANE.to_owned(),
        state,
        hash,
        at,
        epoch: "uhnr".to_owned(),
        sequence: at,
        changed_at: at.to_string(),
    }
}

fn ack_with_comment(text: &str) -> AckReadback {
    AckReadback::from_comments_json(
        BEAD,
        PANE,
        format!(r#"[{{"text":"{text}","created_at":20}}]"#).as_bytes(),
    )
    .expect("comment read-back fixture")
    .with_dispatch_issued_at(10)
}

fn ack_without_matching_comment() -> AckReadback {
    AckReadback::from_comments_json(
        BEAD,
        PANE,
        br#"[{"text":"worker started work","created_at":20}]"#,
    )
    .expect("non-ACK comment read-back fixture")
    .with_dispatch_issued_at(10)
}

fn input(transport: TransportReceipt, ack: AckReadback) -> AckStageInput {
    AckStageInput {
        bead_id: BEAD.to_owned(),
        pane_id: PANE.to_owned(),
        transport,
        pre_send: observation(PaneState::Idle, 100, 1),
        post_send: PostSendObservation::Present(observation(
            PaneState::Working { timer_secs: 100 },
            110,
            1,
        )),
        ack,
        attempts_so_far: 1,
        session_pane_ids: vec![PANE.to_owned()],
    }
}

#[test]
fn busy_pane_with_prefix_correct_ack_confirms_on_retry() {
    let result = assess(&input(
        ntm(),
        ack_with_comment("ACK uhnr on %7 -- agent=WildStone"),
    ));
    assert!(result.is_confirmed(), "durable ACK must win over busy pane: {result:?}");
    assert!(matches!(result.action, ack_stage::AckAction::RecordReceipt { .. }));
    assert!(matches!(result.delivery, ReceiptVerdict::AckConfirmed { .. }));
}

#[test]
fn busy_pane_without_ack_remains_indeterminate() {
    let result = assess(&input(ntm(), ack_without_matching_comment()));
    assert!(!result.is_confirmed());
    assert!(matches!(
        result.delivery,
        ReceiptVerdict::Indeterminate {
            reason: ReceiptReason::TimerTooLargeAfterIdle { .. },
            ..
        }
    ));
    assert!(matches!(result.action, ack_stage::AckAction::Retry { .. }));
}

#[test]
fn eg0m_pane_mismatch_is_machine_remeasurable_not_human_escalation() {
    let comments = br#"[
        {"text":"ACK eg0m on %1397 -- agent=control-plane","created_at":1},
        {"text":"ACK eg0m on %1414 -- agent=other-session","created_at":2},
        {"text":"ACK eg0m on %8 -- agent=WildStone","created_at":3},
        {"text":"ACK eg0m on %8 -- agent=WildStone","created_at":4},
        {"text":"ACK eg0m on %9 -- agent=WildStone","created_at":5},
        {"text":"ACK eg0m on %9 -- agent=WildStone","created_at":6},
        {"text":"ACK eg0m on %9 -- agent=WildStone","created_at":7}
    ]"#;
    let readback = AckReadback::from_comments_json("omp-orchestrator-eg0m", "%1414", comments)
        .expect("copied eg0m comment-shape fixture")
        .with_dispatch_issued_at(0);
    let result = assess(&AckStageInput {
        bead_id: "omp-orchestrator-eg0m".into(),
        pane_id: "%1414".into(),
        transport: ntm(),
        pre_send: observation(PaneState::Idle, 100, 1),
        post_send: PostSendObservation::Present(observation(
            PaneState::Working { timer_secs: 1 },
            110,
            1,
        )),
        ack: readback,
        attempts_so_far: 0,
        session_pane_ids: vec!["%8".into()],
    });
    assert_eq!(result.ack_verdict.label(), "ACK_PANE_MISMATCH");
    assert!(matches!(result.action, ack_stage::AckAction::Retry { .. }));
    assert_ne!(result.action.label(), "AWAIT_HUMAN");
}

#[test]
fn unreachable_ack_wait_still_owes_a_human() {
    let first = observation(PaneState::Working { timer_secs: 120 }, 1_000, 1);
    let last = observation(PaneState::Idle, 1_080, 1);
    let verdict = classify_ack_wait(&first, &last);
    assert!(matches!(verdict, AckWaitVerdict::Unreachable { .. }));
    assert!(verdict.owes_a_human());
    assert_eq!(
        ack_stage::HumanRequiredReason::from_ack_wait(&verdict),
        Some(ack_stage::HumanRequiredReason::PaneUnreachable)
    );
}
#[test]
fn comment_and_transport_arms_are_distinct() {
    let comment_result = assess(&input(
        ntm(),
        ack_with_comment("ACK uhnr on %7 -- agent=WildStone"),
    ));
    let tmux_result = assess(&input(
        tmux(),
        ack_with_comment("ACK uhnr on %7 -- agent=WildStone"),
    ));
    assert!(comment_result.is_confirmed());
    assert!(!tmux_result.is_confirmed(), "tmux remains unable to prove uniform delivery");

    let first = observation(PaneState::Working { timer_secs: 1 }, 100, 7);
    let last = observation(PaneState::Working { timer_secs: 10 }, 180, 7);
    assert!(matches!(
        classify_ack_wait(&first, &last),
        AckWaitVerdict::BusyStillWorking { .. }
    ));
}

#[test]
fn empty_unparseable_and_busy_states_have_distinct_names() {
    let empty = AckReadback::from_comments_json(BEAD, PANE, b"[]").unwrap_err();
    assert_eq!(empty, AckReadbackError::EmptyAckCensus);
    assert_eq!(empty.to_string(), "ACK_CENSUS_EMPTY");
    let unparseable = AckReadback::from_comments_json(BEAD, PANE, br#"[{"id":1}]"#).unwrap_err();
    assert!(matches!(unparseable, AckReadbackError::MissingText(0)));
    assert_ne!(unparseable.to_string(), empty.to_string());

    let indeterminate = ack_stage::decide(
        &ReceiptVerdict::Indeterminate {
            pane_id: PANE.into(),
            reason: ReceiptReason::AckReadbackMissing,
        },
        0,
    );
    assert_eq!(indeterminate.label(), "RETRY");
    assert_ne!(indeterminate.label(), empty.to_string());
    assert_ne!(indeterminate.label(), "AWAIT_HUMAN");

    let busy = AckWaitVerdict::BusyStillWorking {
        timer_from: 1,
        timer_to: 10,
        span_secs: 80,
    };
    assert_eq!(busy.label(), "ACK_PENDING_WORKER_BUSY");
    assert_ne!(busy.label(), empty.to_string());
}

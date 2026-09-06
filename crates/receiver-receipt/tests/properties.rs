#![forbid(unsafe_code)]

//! Proptest floor for iis6: below the observation floor, two working captures
//! never owe a human. Restates `fuzz/fuzz_targets/receiver_receipt_ack_wait.rs`.

use proptest::prelude::*;
use receiver_receipt::{
    classify_ack_wait, AckWaitVerdict, Observation, PaneState, OBSERVATION_WINDOW_MIN_SECS,
};

#[derive(Debug, Clone, Copy)]
enum State {
    Working(u64),
    Idle,
    Wedged,
    Dialog(u64),
    ProviderError402,
    Unproven,
}

impl State {
    fn into_real(self) -> PaneState {
        match self {
            State::Working(t) => PaneState::Working { timer_secs: t },
            State::Idle => PaneState::Idle,
            State::Wedged => PaneState::Wedged,
            State::Dialog(t) => PaneState::Dialog { timer_secs: t },
            State::ProviderError402 => PaneState::ProviderError402,
            State::Unproven => PaneState::Unproven,
        }
    }
}

fn state_strategy() -> impl Strategy<Value = State> {
    prop_oneof![
        (0u64..400).prop_map(State::Working),
        Just(State::Idle),
        Just(State::Wedged),
        (0u64..400).prop_map(State::Dialog),
        Just(State::ProviderError402),
        Just(State::Unproven),
    ]
}

fn observation(pane: &str, state: State, at: u64, hash: u64, seq: u64) -> Observation {
    Observation {
        pane_id: pane.to_owned(),
        state: state.into_real(),
        hash,
        at,
        epoch: "proptest".to_owned(),
        sequence: seq,
        changed_at: "0".to_owned(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Busy,
    OwesHuman,
    Indeterminate,
}

fn model(
    same_pane: bool,
    first_state: State,
    last_state: State,
    first_at: u64,
    last_at: u64,
) -> Class {
    if !same_pane || last_at < first_at {
        return Class::Indeterminate;
    }
    let span = last_at - first_at;
    match (first_state, last_state) {
        (State::Working(from), State::Working(to)) => {
            if span < OBSERVATION_WINDOW_MIN_SECS {
                Class::Indeterminate
            } else if to > from {
                Class::Busy
            } else {
                Class::OwesHuman
            }
        }
        (_, State::Dialog(_))
        | (_, State::Wedged)
        | (_, State::Idle)
        | (_, State::ProviderError402) => Class::OwesHuman,
        (_, State::Working(_)) | (_, State::Unproven) => Class::Indeterminate,
    }
}

proptest! {
    #[test]
    fn iis6_below_floor_two_working_never_owe_a_human(
        same_pane in proptest::bool::ANY,
        first_state in state_strategy(),
        last_state in state_strategy(),
        first_at in 0u64..10_000,
        span in 0u32..=200,
        backwards in proptest::bool::ANY,
        first_hash in 0u64..100,
        last_hash in 0u64..100,
    ) {
        let last_at = if backwards {
            first_at.saturating_sub(u64::from(span))
        } else {
            first_at.saturating_add(u64::from(span))
        };
        let first = observation("%1", first_state, first_at, first_hash, 1);
        let last_pane = if same_pane { "%1" } else { "%2" };
        let last = observation(last_pane, last_state, last_at, last_hash, 2);
        let verdict = classify_ack_wait(&first, &last);
        let actual = match &verdict {
            AckWaitVerdict::BusyStillWorking { .. } => Class::Busy,
            AckWaitVerdict::Unreachable { .. } => Class::OwesHuman,
            AckWaitVerdict::Indeterminate { .. } => Class::Indeterminate,
        };
        prop_assert_eq!(actual, model(same_pane, first_state, last_state, first_at, last_at));
        prop_assert_eq!(verdict.owes_a_human(), actual == Class::OwesHuman);
        if same_pane && !backwards && u64::from(span) < OBSERVATION_WINDOW_MIN_SECS {
            if let (State::Working(_), State::Working(_)) = (first_state, last_state) {
                prop_assert!(!verdict.owes_a_human());
            }
        }
        if let AckWaitVerdict::BusyStillWorking { timer_from, timer_to, span_secs } = &verdict {
            prop_assert!(timer_to > timer_from);
            prop_assert_eq!(*span_secs, last_at - first_at);
        }
        let _ = verdict.label();
        let _ = verdict.next_action();
        prop_assert_eq!(verdict.reason_or_empty().is_empty(), actual == Class::Busy);
    }
}

#[test]
fn planted_below_floor_two_working_never_owe_a_human() {
    let first = observation("%1", State::Working(10), 1000, 1, 1);
    let last = observation("%1", State::Working(10), 1030, 1, 2);
    let verdict = classify_ack_wait(&first, &last);
    assert!(!verdict.owes_a_human(), "{verdict:?}");
    assert!(matches!(verdict, AckWaitVerdict::Indeterminate { .. }));
}

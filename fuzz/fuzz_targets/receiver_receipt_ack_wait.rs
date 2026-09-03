#![no_main]
//! INVARIANT (bead iis6, and zlyy K2's discriminator): an expired ACK wait owes a human ONLY
//! when the pane is demonstrably not working — never below the 75-second observation floor,
//! never on a pane whose timer advanced, never when the classifier declined to answer
//! (`Unproven`). `Indeterminate` is restrictive: it is neither a busy finding nor a death claim.
//!
//! The reference model is written from the documented contract, independently of the arms in
//! `classify_ack_wait`; a divergence is either a kernel defect or a contract drift, and both are
//! findings.

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use receiver_receipt::{classify_ack_wait, AckWaitVerdict, Observation, PaneState, OBSERVATION_WINDOW_MIN_SECS};

#[derive(Arbitrary, Debug, Clone, Copy)]
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

#[derive(Arbitrary, Debug)]
struct Input {
    same_pane: bool,
    first_state: State,
    last_state: State,
    first_at: u64,
    /// Offset from `first_at`; `backwards` flips the order to exercise the clock-fault arm.
    span: u32,
    backwards: bool,
    first_hash: u64,
    last_hash: u64,
}

fn observation(pane: &str, state: State, at: u64, hash: u64, seq: u64) -> Observation {
    Observation {
        pane_id: pane.to_owned(),
        state: state.into_real(),
        hash,
        at,
        epoch: "fuzz".to_owned(),
        sequence: seq,
        changed_at: "0".to_owned(),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Class {
    Busy,
    OwesHuman,
    Indeterminate,
}

fn model(input: &Input, first_at: u64, last_at: u64) -> Class {
    if !input.same_pane || last_at < first_at {
        return Class::Indeterminate;
    }
    let span = last_at - first_at;
    match (input.first_state, input.last_state) {
        (State::Working(from), State::Working(to)) => {
            if span < OBSERVATION_WINDOW_MIN_SECS {
                Class::Indeterminate
            } else if to > from {
                Class::Busy
            } else {
                Class::OwesHuman
            }
        }
        (_, State::Dialog(_)) | (_, State::Wedged) | (_, State::Idle) | (_, State::ProviderError402) => Class::OwesHuman,
        (_, State::Working(_)) | (_, State::Unproven) => Class::Indeterminate,
    }
}

fuzz_target!(|input: Input| {
    let first_at = input.first_at;
    let last_at = if input.backwards {
        first_at.saturating_sub(u64::from(input.span))
    } else {
        first_at.saturating_add(u64::from(input.span))
    };
    let first = observation("%1", input.first_state, first_at, input.first_hash, 1);
    let last_pane = if input.same_pane { "%1" } else { "%2" };
    let last = observation(last_pane, input.last_state, last_at, input.last_hash, 2);

    let verdict = classify_ack_wait(&first, &last);

    let actual = match &verdict {
        AckWaitVerdict::BusyStillWorking { .. } => Class::Busy,
        AckWaitVerdict::Unreachable { .. } => Class::OwesHuman,
        AckWaitVerdict::Indeterminate { .. } => Class::Indeterminate,
    };
    // Contract 1: the classification equals the model's for every pair.
    assert_eq!(actual, model(&input, first_at, last_at), "classify_ack_wait diverged: {input:?} -> {verdict:?}");
    // Contract 2: `owes_a_human` is exactly the Unreachable arm — never derived from a string.
    assert_eq!(verdict.owes_a_human(), actual == Class::OwesHuman, "owes_a_human disagrees with the variant");
    // Contract 3: below the floor, two working captures can never owe a human (iis6).
    if input.same_pane && !input.backwards && u64::from(input.span) < OBSERVATION_WINDOW_MIN_SECS {
        if let (State::Working(_), State::Working(_)) = (input.first_state, input.last_state) {
            assert!(!verdict.owes_a_human(), "a human was owed inside the window floor: {verdict:?}");
        }
    }
    // Contract 4: a busy verdict carries the span and timers it was judged on, verbatim.
    if let AckWaitVerdict::BusyStillWorking { timer_from, timer_to, span_secs } = &verdict {
        assert!(timer_to > timer_from, "busy without timer advance");
        assert_eq!(*span_secs, last_at - first_at, "span not carried verbatim");
    }
    // Contract 5: every verdict has a label, a next action, and a reason string; none panic,
    // and the reason is non-empty exactly when the verdict is not the busy arm.
    let _ = verdict.label();
    let _ = verdict.next_action();
    assert_eq!(verdict.reason_or_empty().is_empty(), actual == Class::Busy, "reason presence disagrees with the arm");
});

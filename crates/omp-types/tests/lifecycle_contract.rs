#![forbid(unsafe_code)]

use omp_types::{Lifecycle, LifecycleInput, WaitDeadline};
use std::process::Command;
use std::time::Duration;
use subprocess_contract::BoundedOutcome;

fn generated_inputs(seed: u64, len: usize) -> Vec<LifecycleInput> {
    let mut value = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    (0..len)
        .map(|_| {
            value = value
                .wrapping_mul(2862933555777941757)
                .wrapping_add(3037000493);
            match value % 7 {
                0 => LifecycleInput::Ready,
                1 => LifecycleInput::Negotiated,
                2 => LifecycleInput::Activated,
                3 => LifecycleInput::StopRequested(WaitDeadline::new(Duration::from_nanos(
                    (value % 10_000) + 1,
                ))),
                4 => LifecycleInput::Stopped,
                5 => LifecycleInput::Failed,
                _ => LifecycleInput::TimedOut,
            }
        })
        .collect()
}

fn all_inputs() -> [LifecycleInput; 7] {
    [
        LifecycleInput::Ready,
        LifecycleInput::Negotiated,
        LifecycleInput::Activated,
        LifecycleInput::StopRequested(WaitDeadline::new(Duration::from_millis(1))),
        LifecycleInput::Stopped,
        LifecycleInput::Failed,
        LifecycleInput::TimedOut,
    ]
}

/// L1: generated transition sequences cannot move any terminal.
#[test]
fn terminal_states_are_closed_under_generated_inputs() {
    for terminal in [Lifecycle::Stopped, Lifecycle::Failed, Lifecycle::TimedOut] {
        assert!(terminal.is_terminal());
        for seed in 0..128 {
            let mut state = terminal;
            for input in generated_inputs(seed, 64) {
                state = state.transition(input);
                assert_eq!(state, terminal, "seed={seed} input={input:?}");
            }
        }
    }
}

/// L2: restrictive terminals never become a successful state.
#[test]
fn restrictive_terminals_never_become_success() {
    for terminal in [Lifecycle::Failed, Lifecycle::TimedOut] {
        assert!(terminal.is_restrictive_terminal());
        for input in all_inputs() {
            let next = terminal.transition(input);
            assert!(next.is_restrictive_terminal());
            assert_ne!(next, Lifecycle::Stopped);
            assert_ne!(next, Lifecycle::Active);
        }
    }
    assert!(!Lifecycle::Stopped.is_restrictive_terminal());
}

/// L3: a deadline-killed child is not a subject verdict, including when no bytes were captured.
#[test]
fn timeout_is_not_a_subject_verdict() {
    let mapped = Lifecycle::from_bounded_outcome(BoundedOutcome::TimedOut);
    assert_eq!(mapped, Lifecycle::TimedOut);
    assert!(mapped.is_restrictive_terminal());

    let empty_success = Command::new("/usr/bin/true")
        .output()
        .expect("true must spawn");
    assert!(empty_success.stdout.is_empty() && empty_success.stderr.is_empty());
    assert_eq!(
        Lifecycle::from_bounded_outcome(BoundedOutcome::Completed(empty_success)),
        Lifecycle::Stopped
    );
}

/// L4: every shutdown transition carries a finite deadline value.
#[test]
fn shutdown_input_carries_a_bounded_deadline() {
    for seed in 1..128 {
        let duration = Duration::from_millis(seed);
        let deadline = WaitDeadline::new(duration);
        assert_eq!(deadline.duration(), duration);
        assert_eq!(
            Lifecycle::Active.transition(LifecycleInput::StopRequested(deadline)),
            Lifecycle::Stopping
        );
    }
}

/// L5: the bridge is total over BoundedOutcome. The production match is exhaustive by type check.
#[test]
fn bounded_outcome_mapping_is_total() {
    let success = Command::new("/usr/bin/true")
        .output()
        .expect("true must spawn");
    let failure = Command::new("/usr/bin/false")
        .output()
        .expect("false must spawn");
    assert_eq!(
        Lifecycle::from_bounded_outcome(BoundedOutcome::Completed(success)),
        Lifecycle::Stopped
    );
    assert_eq!(
        Lifecycle::from_bounded_outcome(BoundedOutcome::Completed(failure)),
        Lifecycle::Failed
    );
    assert_eq!(
        Lifecycle::from_bounded_outcome(BoundedOutcome::TimedOut),
        Lifecycle::TimedOut
    );
    assert_eq!(
        Lifecycle::from_bounded_outcome(BoundedOutcome::Unspawned(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "fixture missing",
        ))),
        Lifecycle::Failed
    );
}

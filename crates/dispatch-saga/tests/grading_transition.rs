#![forbid(unsafe_code)]

use dispatch_saga::grading::{
    decide, decide_all, detect, is_hand_moved_twelve, GradingCandidate, GradingDecision,
    HAND_MOVED_TWELVE,
};

fn twelve_in_progress(implementer: &str) -> Vec<GradingCandidate> {
    HAND_MOVED_TWELVE
        .iter()
        .map(|id| GradingCandidate {
            id: (*id).to_string(),
            status: "in_progress".into(),
            implementer: implementer.into(),
            landed_commit: true,
        })
        .collect()
}

#[test]
fn detector_selects_all_twelve_hand_moved_beads() {
    assert_eq!(HAND_MOVED_TWELVE.len(), 12);
    let rows = twelve_in_progress("WildStone");
    let detected = detect(&rows).expect("non-empty");
    assert_eq!(detected.len(), 12, "detector selected zero of the twelve");
    for id in HAND_MOVED_TWELVE {
        assert!(is_hand_moved_twelve(id));
        assert!(detected.iter().any(|c| c.id == *id));
    }
}

#[test]
fn empty_candidate_set_reuses_vacuous_while_baseline_nonzero() {
    let err = detect(&[]).expect_err("empty must error");
    assert_eq!(err, blocker_taxonomy::ReportError::VacuousWhileBaselineNonzero);
}

#[test]
fn self_grade_is_a_typed_refusal_not_a_routing() {
    let bead = GradingCandidate {
        id: HAND_MOVED_TWELVE[0].into(),
        status: "in_progress".into(),
        implementer: "WildStone".into(),
        landed_commit: true,
    };
    let only_self = vec!["WildStone".into()];
    let d = decide(&bead, &only_self);
    match d {
        GradingDecision::RefuseSelfGrade {
            bead: b,
            implementer,
        } => {
            assert_eq!(b, HAND_MOVED_TWELVE[0]);
            assert_eq!(implementer, "WildStone");
        }
        other => panic!("expected typed self-grade refusal, got {other:?}"),
    }
}

#[test]
fn no_landed_commit_is_not_transitioned() {
    let bead = GradingCandidate {
        id: HAND_MOVED_TWELVE[1].into(),
        status: "in_progress".into(),
        implementer: "WildStone".into(),
        landed_commit: false,
    };
    let graders = vec!["Orchestrator".into()];
    assert!(matches!(
        decide(&bead, &graders),
        GradingDecision::RefuseNoCommit { .. }
    ));
    let row = [bead];
    let detected = detect(&row).expect("non-empty scan");
    assert!(detected.is_empty());

}

#[test]
fn non_author_grader_is_named_and_br_update_is_not_executed() {
    let rows = twelve_in_progress("WildStone");
    let graders = vec!["Orchestrator".into(), "WildStone".into()];
    let decisions = decide_all(&rows, &graders).expect("non-empty");
    assert_eq!(decisions.len(), 12);
    for d in &decisions {
        match d {
            GradingDecision::Transition { grader, br_update, .. } => {
                assert_eq!(grader, "Orchestrator");
                assert_eq!(br_update[0], "br");
                assert!(br_update.contains(&"grading".into()));
            }
            other => panic!("expected transition, got {other:?}"),
        }
    }
}

#[test]
fn default_path_performs_no_tracker_write() {
    let grading = include_str!("../src/grading.rs");
    for needle in [
        "Command::new",
        "br update",
        "std::process",
        "tmux send",
    ] {
        assert!(
            !grading.contains(needle),
            "decide-only module must not contain {needle}"
        );
    }
    assert!(
        grading.contains("reject_self_grade"),
        "non-author check must exist so mutation can delete it"
    );
}

// ---------------------------------------------------------------------------
// omp-orchestrator-heor: the `br update` write on the IMPL->GRADING lane must be
// BOUNDED. These legs assert the TYPED ARM, never an exit code -- `cargo` returns
// 101 both for a missing test target and for a workspace that cannot load, so a
// leg keyed on `rc != 0` goes green on unrelated breakage.
// ---------------------------------------------------------------------------

/// FIRES-ON-KNOWN-BAD: a child that never returns inside the deadline must land in
/// `TimedOut` with the GROUP killed, never in `Completed`.
///
/// MUTATION TARGET: drop the bound in `dispatch_saga::run_bounded` (or widen the
/// deadline past the child's lifetime) and this goes RED on the arm, not on a code.
#[test]
fn a_child_that_never_returns_is_timed_out_with_the_group_killed() {
    let mut command = std::process::Command::new("sleep");
    command.arg("30");
    let outcome = dispatch_saga::run_bounded(&mut command, std::time::Duration::from_millis(600));
    match outcome {
        omp_types::ChildOutcome::TimedOut {
            after_ms,
            group_killed,
        } => {
            assert!(group_killed, "the deadline must signal the GROUP, not the pid");
            assert_eq!(after_ms, 600, "the reported deadline must be the one applied");
        }
        other => panic!(
            "an unbounded wait is the defect heor fixes; expected TimedOut, got {other:?}"
        ),
    }
}

/// KNOWN-GOOD LEG, mandatory: an over-strict bound here would stall every
/// IMPL->GRADING transition, which is a slower death than no bound at all.
#[test]
fn a_fast_child_still_completes_with_its_output_unchanged() {
    let mut command = std::process::Command::new("echo");
    command.arg("grading");
    let outcome = dispatch_saga::run_bounded(&mut command, dispatch_saga::BR_UPDATE_DEADLINE);
    match outcome {
        omp_types::ChildOutcome::Completed {
            code,
            stdout,
            stderr,
        } => {
            assert_eq!(code, Some(0), "a healthy child must report its own code");
            assert_eq!(stdout.trim(), "grading", "captured stdout must be unchanged");
            assert!(stderr.is_empty(), "no stderr expected, got {stderr:?}");
        }
        other => panic!("the bound must not break the healthy path, got {other:?}"),
    }
}

/// `SpawnFailed` must stay DISTINCT from `TimedOut`: a PATH/env problem and a wedged
/// subject have different remedies, and collapsing them sends the operator to the
/// wrong one.
#[test]
fn an_unspawnable_command_is_not_reported_as_a_deadline() {
    let mut command =
        std::process::Command::new("dispatch-saga-heor-no-such-binary-b7f2");
    let outcome = dispatch_saga::run_bounded(&mut command, dispatch_saga::BR_UPDATE_DEADLINE);
    match outcome {
        omp_types::ChildOutcome::SpawnFailed { message } => {
            assert!(!message.is_empty(), "a spawn failure must name its reason");
        }
        other => panic!("an unspawnable command is not a deadline, got {other:?}"),
    }
}

/// The deadline is ARGUED, and this leg keeps the argument from silently regressing.
/// AGENTS.md's measured bands on this database: reads 40-250 s, one `br comments add`
/// at 56.7 s under contention, a close attempt held 290 s; and every `br` call here
/// carries `--lock-timeout` up to 90 s, which is a wait we ASKED for. A ceiling inside
/// that band converts contention into a false failure on the stage-transition path.
#[test]
fn the_write_deadline_sits_above_the_measured_contention_band() {
    let secs = dispatch_saga::BR_UPDATE_DEADLINE.as_secs();
    assert!(
        secs > 290 + 90,
        "deadline {secs}s must exceed the longest observed hold (290s) plus a full \
         90s lock wait, or a legitimate wait becomes a false failure"
    );
    assert!(
        secs < 3600,
        "deadline {secs}s must still be FINITE and operator-scaled; an hour-plus bound \
         is an unbounded wait with extra steps"
    );
}

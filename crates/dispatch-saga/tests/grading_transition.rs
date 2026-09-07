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

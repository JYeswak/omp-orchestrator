#![forbid(unsafe_code)]

use dispatch_saga::m2::{
    lineage_eligible, lineage_from_argv, route, GradingBead, ModelLineage, Pane, RouteDecision,
};

fn pane(id: &str, profile: &str, idle: bool) -> Pane {
    Pane {
        pane: id.into(),
        argv: format!("omp --profile {profile} AGENT_NAME=WildStone"),
        idle,
        safe_to_dispatch: idle,
    }
}

fn bead(id: &str, status: &str, impl_pane: &str, profile: ModelLineage) -> GradingBead {
    GradingBead {
        id: id.into(),
        status: status.into(),
        implementer_pane: impl_pane.into(),
        implementer_profile: profile,
    }
}

#[test]
fn lineage_is_argv_profile_not_agent_name() {
    let a = "omp --profile codex AGENT_NAME=WildStone";
    let b = "omp --profile grok AGENT_NAME=WildStone";
    assert_eq!(lineage_from_argv(a), Some(ModelLineage::Codex));
    assert_eq!(lineage_from_argv(b), Some(ModelLineage::Grok));
    assert!(lineage_eligible(
        lineage_from_argv(a).unwrap(),
        lineage_from_argv(b).unwrap()
    ));
    let src = include_str!("../src/m2.rs");
    assert!(
        !src.contains("AGENT_NAME") || src.contains("not consulted") || src.contains("uniform"),
        "kernel must not compare AGENT_NAME as eligibility"
    );
    assert!(!src.contains("std::env::var(\"AGENT_NAME\")"));
}

#[test]
fn percent_7_and_8_both_codex_are_not_eligible_for_each_other() {
    let b = bead("cas-like", "grading", "%7", ModelLineage::Codex);
    let panes = [pane("%8", "codex", true)];
    match route(&b, &panes).expect("non-empty panes") {
        RouteDecision::RefuseSameLineage {
            other_pane,
            other_profile,
            implementer_profile,
            ..
        } => {
            assert_eq!(other_pane, "%8");
            assert_eq!(other_profile, ModelLineage::Codex);
            assert_eq!(implementer_profile, ModelLineage::Codex);
        }
        other => panic!("expected same-lineage refusal, got {other:?}"),
    }
}

#[test]
fn implementer_only_idle_pane_is_typed_refusal() {
    let b = bead("self", "grading", "%9", ModelLineage::Grok);
    let panes = [pane("%9", "grok", true)];
    match route(&b, &panes).expect("non-empty") {
        RouteDecision::RefuseSelfGrade { pane, .. } => assert_eq!(pane, "%9"),
        other => panic!("expected self-grade refusal, got {other:?}"),
    }
}

#[test]
fn bead_not_in_grading_is_not_routed() {
    let b = bead("open", "in_progress", "%9", ModelLineage::Grok);
    let panes = [pane("%8", "codex", true)];
    assert!(matches!(
        route(&b, &panes).expect("non-empty"),
        RouteDecision::RefuseNotGrading { .. }
    ));
}

#[test]
fn three_performed_grades_are_all_lineage_eligible() {
    // cas grok→codex, xbcl codex→grok, 8sv2 grok→claude
    assert!(lineage_eligible(ModelLineage::Grok, ModelLineage::Codex));
    assert!(lineage_eligible(ModelLineage::Codex, ModelLineage::Grok));
    assert!(lineage_eligible(ModelLineage::Grok, ModelLineage::Claude));
    let cas = bead(
        "omp-orchestrator-exit-const-collision-cas",
        "grading",
        "%9",
        ModelLineage::Grok,
    );
    match route(&cas, &[pane("%8", "codex", true)]).unwrap() {
        RouteDecision::Route {
            grader_profile, ..
        } => assert_eq!(grader_profile, ModelLineage::Codex),
        other => panic!("cas must route grok→codex, got {other:?}"),
    }
    let xbcl = bead(
        "omp-orchestrator-s0-dag-pagerank-legibility-xbcl",
        "grading",
        "%8",
        ModelLineage::Codex,
    );
    match route(&xbcl, &[pane("%9", "grok", true)]).unwrap() {
        RouteDecision::Route {
            grader_profile, ..
        } => assert_eq!(grader_profile, ModelLineage::Grok),
        other => panic!("xbcl must route codex→grok, got {other:?}"),
    }
    let sv2 = bead(
        "omp-orchestrator-wire-dispatch-saga-grading-transition-8sv2",
        "grading",
        "%9",
        ModelLineage::Grok,
    );
    match route(&sv2, &[pane("%6", "claude", true)]).unwrap() {
        RouteDecision::Route {
            grader_profile, ..
        } => assert_eq!(grader_profile, ModelLineage::Claude),
        other => panic!("8sv2 must route grok→claude, got {other:?}"),
    }
}

#[test]
fn empty_pane_set_is_vacuous() {
    let b = bead("x", "grading", "%9", ModelLineage::Grok);
    let err = route(&b, &[]).expect_err("empty panes");
    assert_eq!(err, blocker_taxonomy::ReportError::VacuousWhileBaselineNonzero);
}

#[test]
fn decide_only_performs_no_send_and_no_br_write() {
    let src = include_str!("../src/m2.rs");
    for needle in ["Command::new", "ntm --robot-send", "br update", "std::process"] {
        assert!(!src.contains(needle), "m2 kernel must not contain {needle}");
    }
    assert!(
        src.contains("reject_same_lineage"),
        "profile comparison must exist so mutation can delete it"
    );
}

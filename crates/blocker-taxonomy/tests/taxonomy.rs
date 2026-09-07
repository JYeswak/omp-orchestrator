#![forbid(unsafe_code)]

use blocker_taxonomy::{
    baseline_292_fixture, classify, decide_redispatch, decide_redispatch_ignoring_liveness,
    exhibiting_input, parse_reason, predicate_fires, remedy, report, BeadRecord, BlockerKind,
    LiveSet, LiveSetError, ProgressToken, Reason, RedispatchDecision, RemedyClass, TrackerClass,
    ALL_KINDS, BASELINE_BLOCKED_TOTAL, BASELINE_DEP_BLOCKED, BASELINE_DEP_BLOCKED_P0,
    BASELINE_STATUS_BLOCKED,
};

fn open_idle(id: &str) -> BeadRecord {
    BeadRecord {
        id: id.into(),
        status: "open".into(),
        assignee: None,
        issue_type: "task".into(),
        priority: 1,
        unresolved_out_blocks: Vec::new(),
        unresolved_out_parent_child: Vec::new(),
        heartbeat_reason: None,
        hold_pane: None,
        pending_dispatch_marker: false,
        grading_started_unterminal: false,
    }
}

#[test]
fn every_kind_has_an_exhibiting_input() {
    assert_eq!(ALL_KINDS.len(), 11);
    for kind in ALL_KINDS {
        let bead = exhibiting_input(*kind);
        assert!(
            predicate_fires(*kind, &bead),
            "{kind:?} predicate did not fire on its exhibiting input"
        );
        let got = classify(&bead);
        assert!(
            got.contains(kind),
            "{kind:?} missing from classify={got:?}"
        );
    }
}

#[test]
fn progress_reasons_never_enter_the_enum() {
    for (raw, token) in [
        ("still_changing", ProgressToken::StillChanging),
        ("working", ProgressToken::Working),
        ("VERDICT_POSTED", ProgressToken::VerdictPosted),
        ("verdict_posted", ProgressToken::VerdictPosted),
    ] {
        assert_eq!(parse_reason(raw), Reason::Progress(token));
        let mut bead = open_idle("progress");
        bead.heartbeat_reason = Some(raw.into());
        let kinds = classify(&bead);
        assert!(
            kinds.is_empty(),
            "progress token {raw} entered enum as {kinds:?}"
        );
    }
    let names: Vec<String> = ALL_KINDS.iter().map(|k| format!("{k:?}")).collect();
    let joined = names.join(" ");
    assert!(!joined.contains("StillChanging"));
    assert!(!joined.contains("Working"));
    assert!(!joined.contains("VerdictPosted"));
}

#[test]
fn tracker_blocked_is_the_status_field_not_the_graph() {
    let bead = exhibiting_input(BlockerKind::TrackerBlocked);
    assert_eq!(bead.status, "blocked");
    assert!(bead.unresolved_out_blocks.is_empty());
    assert!(classify(&bead).contains(&BlockerKind::TrackerBlocked));
}

#[test]
fn dependency_blocked_uses_unresolved_out_blocks() {
    let bead = exhibiting_input(BlockerKind::DependencyBlocked);
    assert_eq!(bead.status, "open");
    assert_eq!(bead.unresolved_out_blocks, ["live-blocker"]);
    assert!(classify(&bead).contains(&BlockerKind::DependencyBlocked));
}

#[test]
fn xbcl_stale_blocked_label_is_tracker_blocked_even_with_empty_graph() {
    // xbcl 2348: fence-ci-non-verdict-16l flipped blocked → in_progress with rc=0.
    let mut bead = open_idle("omp-orchestrator-fence-ci-non-verdict-16l");
    bead.status = "blocked".into();
    assert!(classify(&bead).contains(&BlockerKind::TrackerBlocked));
    assert!(!classify(&bead).contains(&BlockerKind::DependencyBlocked));
}

#[test]
fn own_child_blocks_epic_is_distinct_from_dependency_blocked() {
    let bead = exhibiting_input(BlockerKind::OwnChildBlocksEpic);
    assert_eq!(bead.id, "kxe");
    assert_eq!(bead.unresolved_out_parent_child, ["2lqd"]);
    let kinds = classify(&bead);
    assert!(kinds.contains(&BlockerKind::OwnChildBlocksEpic));
    assert!(!kinds.contains(&BlockerKind::DependencyBlocked));
}

#[test]
fn half_claim_open_assigned_and_in_progress_unassigned() {
    let mut open_assigned = open_idle("half-open");
    open_assigned.assignee = Some("WildStone".into());
    assert!(classify(&open_assigned).contains(&BlockerKind::HalfClaim));

    let in_progress = exhibiting_input(BlockerKind::HalfClaim);
    assert_eq!(in_progress.status, "in_progress");
    assert!(!in_progress.assignee_set());
    assert!(classify(&in_progress).contains(&BlockerKind::HalfClaim));
}

#[test]
fn heartbeat_hints_map_to_named_kinds() {
    for (raw, kind) in [
        ("ack_readback_missing", BlockerKind::AckIndeterminate),
        ("ACK_PANE_MISMATCH", BlockerKind::AckIndeterminate),
        (
            "DISPATCH_FAILED_BEFORE_RECEIPT",
            BlockerKind::PacketRefused,
        ),
        ("UNRANKED", BlockerKind::QueueUnranked),
        ("MISSING_RECEIVER_AGENT", BlockerKind::MissingReceiverAgent),
        ("CROSS_PANE_DUPLICATE", BlockerKind::CrossPaneHold),
    ] {
        let mut bead = open_idle("hb");
        bead.heartbeat_reason = Some(raw.into());
        if kind == BlockerKind::CrossPaneHold {
            bead.hold_pane = Some("%1414".into());
        }
        assert!(
            classify(&bead).contains(&kind),
            "{raw} did not produce {kind:?}"
        );
    }
}

#[test]
fn marker_and_stranded_grading_fire() {
    let marker = exhibiting_input(BlockerKind::PendingDispatchMarker);
    assert!(classify(&marker).contains(&BlockerKind::PendingDispatchMarker));
    let stranded = exhibiting_input(BlockerKind::StrandedGradingClaim);
    assert!(classify(&stranded).contains(&BlockerKind::StrandedGradingClaim));
}

#[test]
fn report_positive_control_hits_pinned_292_baseline() {
    let r = report(&baseline_292_fixture()).expect("baseline must report");
    assert_eq!(r.status_blocked, BASELINE_STATUS_BLOCKED);
    assert_eq!(r.dependency_blocked, BASELINE_DEP_BLOCKED);
    assert_eq!(r.dependency_blocked_p0, BASELINE_DEP_BLOCKED_P0);
    assert_eq!(
        r.status_blocked + r.dependency_blocked,
        BASELINE_BLOCKED_TOTAL
    );
    assert!(r.both_classes_reported);
    assert!(r.status_blocked > 0 && r.dependency_blocked > 0);
}

#[test]
fn report_that_shows_only_status_blocked_has_collapsed_the_classes() {
    let only_status: Vec<_> = baseline_292_fixture()
        .into_iter()
        .filter(|b| b.status == "blocked")
        .collect();
    let r = report(&only_status).expect("non-empty");
    assert_eq!(r.status_blocked, BASELINE_STATUS_BLOCKED);
    assert_eq!(r.dependency_blocked, 0);
    assert!(!r.both_classes_reported);
}

#[test]
fn empty_report_is_error_while_baseline_is_nonzero() {
    let err = report(&[]).expect_err("empty must error");
    assert_eq!(
        err,
        blocker_taxonomy::ReportError::VacuousWhileBaselineNonzero
    );
}

#[test]
fn live_set_empty_is_refused_before_any_dead_alive_comparison() {
    assert_eq!(LiveSet::new(Vec::<String>::new()), Err(LiveSetError::Empty));
}

#[test]
fn dead_pane_hold_is_released() {
    let live = LiveSet::new(["%1", "%9"]).expect("non-empty");
    let d = decide_redispatch(
        "held-by-ghost",
        BlockerKind::CrossPaneHold,
        Some("%1408"),
        &live,
    );
    assert_eq!(
        d,
        RedispatchDecision::Release {
            bead: "held-by-ghost".into(),
            pane: "%1408".into(),
        }
    );
}

#[test]
fn live_pane_hold_is_not_cleared() {
    let live = LiveSet::new(["%1", "%9"]).expect("non-empty");
    let d = decide_redispatch("held-live", BlockerKind::CrossPaneHold, Some("%9"), &live);
    assert_eq!(
        d,
        RedispatchDecision::KeepAlive {
            bead: "held-live".into(),
            pane: "%9".into(),
        }
    );
}

#[test]
fn known_bad_liveness_removed_would_clear_a_live_hold() {
    let live = LiveSet::new(["%9"]).expect("non-empty");
    let prod = decide_redispatch("x", BlockerKind::CrossPaneHold, Some("%9"), &live);
    let broken = decide_redispatch_ignoring_liveness("x", BlockerKind::CrossPaneHold, Some("%9"), &live);
    assert!(
        matches!(prod, RedispatchDecision::KeepAlive { .. }),
        "production must keep live hold, got {prod:?}"
    );
    assert!(
        matches!(broken, RedispatchDecision::Release { .. }),
        "removing liveness must be the failing twin, got {broken:?}"
    );
    assert_ne!(prod, broken);
}

#[test]
fn human_gated_kind_is_refused_with_the_bead_id() {
    let live = LiveSet::new(["%1"]).expect("non-empty");
    let d = decide_redispatch(
        "omp-orchestrator-missing-agent",
        BlockerKind::MissingReceiverAgent,
        None,
        &live,
    );
    match d {
        RedispatchDecision::RefuseHumanGated { bead, kind } => {
            assert_eq!(bead, "omp-orchestrator-missing-agent");
            assert_eq!(kind, BlockerKind::MissingReceiverAgent);
        }
        other => panic!("expected human-gated refusal, got {other:?}"),
    }
}

#[test]
fn kernel_clearable_kinds_are_exactly_the_hold_surfaces() {
    assert_eq!(remedy(BlockerKind::CrossPaneHold), RemedyClass::KernelClearable);
    assert_eq!(
        remedy(BlockerKind::PendingDispatchMarker),
        RemedyClass::KernelClearable
    );
    assert_eq!(
        remedy(BlockerKind::MissingReceiverAgent),
        RemedyClass::HumanGated
    );
    assert_eq!(
        remedy(BlockerKind::DependencyBlocked),
        RemedyClass::SelfClearing
    );
}

#[test]
fn tracker_class_splits_the_two_surfaces() {
    let status = exhibiting_input(BlockerKind::TrackerBlocked);
    let dep = exhibiting_input(BlockerKind::DependencyBlocked);
    assert_eq!(
        blocker_taxonomy::tracker_class(&status),
        Some(TrackerClass::StatusBlocked)
    );
    assert_eq!(
        blocker_taxonomy::tracker_class(&dep),
        Some(TrackerClass::DependencyBlocked)
    );
}

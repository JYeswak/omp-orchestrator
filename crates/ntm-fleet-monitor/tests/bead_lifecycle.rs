use ntm_fleet_monitor::bead_lifecycle::{
    BeadId, BeadLifecycle, BlockReason, BlockerEvidence, BlockerKind, DispatchReceipt,
    DispatchTarget, EscalationRef, EventId, EvidencePolicy, GradeReceipt, LifecycleError,
    LifecycleEventKind, LifecycleStatus, ReceiverEvidence, RedispatchPlan,
};
use ntm_fleet_monitor::{classify, Approved, Intent, TypedAction, WaveVerdict};

fn id(raw: &str) -> EventId {
    EventId::new(raw).unwrap()
}
fn bead(raw: &str) -> BeadId {
    BeadId::new(raw).unwrap()
}
fn target(pane: &str) -> DispatchTarget {
    DispatchTarget::new("control-plane", pane).unwrap()
}
fn approval() -> Approved {
    let wave = classify(Intent {
        action: TypedAction::DispatchPacket,
        pane_dispatchable: true,
        two_captures: true,
        packet_complete: true,
        finding_has_bead: true,
    });
    assert_eq!(wave.verdict, WaveVerdict::Autonomous);
    Approved::authorize(wave).unwrap()
}
fn selected() -> BeadLifecycle {
    BeadLifecycle::select(
        bead("cp-life"),
        target("4"),
        "Implement lifecycle",
        approval(),
        id("select-1"),
    )
    .unwrap()
}
fn dispatched() -> BeadLifecycle {
    let mut life = selected();
    life.dispatch(
        DispatchReceipt::new(
            id("dispatch-1"),
            bead("cp-life"),
            target("4"),
            "Implement lifecycle",
            100,
        )
        .unwrap(),
    )
    .unwrap();
    life
}
fn receiver_verified() -> BeadLifecycle {
    let mut life = dispatched();
    life.verify_receiver(
        ReceiverEvidence::new(
            id("receiver-1"),
            bead("cp-life"),
            target("4"),
            "Implement lifecycle",
            110,
        )
        .unwrap(),
        EvidencePolicy::new(120, 20),
    )
    .unwrap();
    life
}
fn grading() -> BeadLifecycle {
    let mut life = receiver_verified();
    life.start_grading(id("grading-1")).unwrap();
    life
}
fn external_blocker(observed_at_ms: u64) -> BlockerEvidence {
    BlockerEvidence::new(
        BlockerKind::ExternalDependency,
        BlockReason::new("awaiting owner").unwrap(),
        Some(EscalationRef::new("ESC-42").unwrap()),
        observed_at_ms,
    )
    .unwrap()
}

#[test]
fn valid_lifecycle_reaches_close_only_after_independent_pass_grade() {
    let mut life = grading();
    life.grade(
        GradeReceipt::pass(
            id("grade-1"),
            bead("cp-life"),
            target("4"),
            id("receiver-1"),
            130,
        ),
        EvidencePolicy::new(140, 20),
    )
    .unwrap();
    assert_eq!(life.status(), LifecycleStatus::GradedPass);
    life.close(id("close-1")).unwrap();
    assert_eq!(life.status(), LifecycleStatus::Closed);
    assert_eq!(
        life.history().last().unwrap().kind,
        LifecycleEventKind::Closed
    );
    assert!(life
        .history()
        .last()
        .unwrap()
        .serialized()
        .contains("\"status\":\"closed\""));
}

#[test]
fn fix_grade_requires_named_redispatch_before_dispatch() {
    let mut life = grading();
    life.grade(
        GradeReceipt::fix(
            id("grade-fix"),
            bead("cp-life"),
            target("4"),
            id("receiver-1"),
            "add-proof",
            130,
        )
        .unwrap(),
        EvidencePolicy::new(140, 20),
    )
    .unwrap();
    assert_eq!(life.status(), LifecycleStatus::GradedFix);
    life.require_redispatch(
        RedispatchPlan::new(
            id("redispatch-1"),
            bead("cp-life"),
            target("4"),
            "add-proof",
        )
        .unwrap(),
    )
    .unwrap();
    life.dispatch(
        DispatchReceipt::new(
            id("dispatch-2"),
            bead("cp-life"),
            target("4"),
            "Implement lifecycle",
            150,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(life.status(), LifecycleStatus::Dispatched);
}

#[test]
fn ungraded_branch_can_receive_evidence_later() {
    let mut life = dispatched();
    life.mark_ungraded(id("ungraded-1")).unwrap();
    assert_eq!(life.status(), LifecycleStatus::Ungraded);
    life.verify_receiver(
        ReceiverEvidence::new(
            id("receiver-2"),
            bead("cp-life"),
            target("4"),
            "Implement lifecycle",
            110,
        )
        .unwrap(),
        EvidencePolicy::new(120, 20),
    )
    .unwrap();
    assert_eq!(life.status(), LifecycleStatus::ReceiverVerified);
}

#[test]
fn invalid_close_paths_and_duplicate_events_are_rejected() {
    let mut life = selected();
    assert!(matches!(
        life.close(id("close-selected")),
        Err(LifecycleError::InvalidTransition { .. })
    ));
    let dispatch = DispatchReceipt::new(
        id("dispatch-1"),
        bead("cp-life"),
        target("4"),
        "Implement lifecycle",
        100,
    )
    .unwrap();
    life.dispatch(dispatch.clone()).unwrap();
    assert!(matches!(
        life.close(id("close-dispatched")),
        Err(LifecycleError::InvalidTransition { .. })
    ));
    assert!(matches!(
        life.dispatch(dispatch),
        Err(LifecycleError::DuplicateEvent(_))
    ));
    let evidence = ReceiverEvidence::new(
        id("receiver-1"),
        bead("cp-life"),
        target("4"),
        "Implement lifecycle",
        110,
    )
    .unwrap();
    life.verify_receiver(evidence.clone(), EvidencePolicy::new(120, 20))
        .unwrap();
    assert!(matches!(
        life.close(id("close-receiver")),
        Err(LifecycleError::InvalidTransition { .. })
    ));
    assert!(matches!(
        life.verify_receiver(evidence, EvidencePolicy::new(120, 20)),
        Err(LifecycleError::DuplicateEvent(_))
    ));
}

#[test]
fn malformed_wrong_target_bead_and_stale_evidence_fail_closed() {
    assert!(EventId::new(" ").is_err());
    assert!(BeadId::new("").is_err());
    assert!(DispatchTarget::new("control-plane", "").is_err());
    assert!(BlockReason::new(" ").is_err());
    let mut life = dispatched();
    let wrong_bead = ReceiverEvidence::new(
        id("wrong-bead"),
        bead("other"),
        target("4"),
        "Implement lifecycle",
        110,
    )
    .unwrap();
    assert!(matches!(
        life.verify_receiver(wrong_bead, EvidencePolicy::new(120, 20)),
        Err(LifecycleError::WrongBead)
    ));
    let wrong_pane = ReceiverEvidence::new(
        id("wrong-pane"),
        bead("cp-life"),
        target("3"),
        "Implement lifecycle",
        110,
    )
    .unwrap();
    assert!(matches!(
        life.verify_receiver(wrong_pane, EvidencePolicy::new(120, 20)),
        Err(LifecycleError::WrongTarget)
    ));
    let stale = ReceiverEvidence::new(
        id("stale"),
        bead("cp-life"),
        target("4"),
        "Implement lifecycle",
        1,
    )
    .unwrap();
    assert!(matches!(
        life.verify_receiver(stale, EvidencePolicy::new(120, 20)),
        Err(LifecycleError::StaleEvidence { .. })
    ));
    let future = ReceiverEvidence::new(
        id("future"),
        bead("cp-life"),
        target("4"),
        "Implement lifecycle",
        121,
    )
    .unwrap();
    assert!(matches!(
        life.verify_receiver(future, EvidencePolicy::new(120, 20)),
        Err(LifecycleError::EvidenceInFuture { .. })
    ));
}

#[test]
fn close_without_independent_grade_is_the_mutation_sensitive_guard() {
    let mut life = grading();
    assert!(matches!(
        life.close(id("close-before-grade")),
        Err(LifecycleError::InvalidTransition { .. })
    ));
    let grade = GradeReceipt::pass(
        id("grade-1"),
        bead("cp-life"),
        target("other-pane"),
        id("receiver-1"),
        130,
    );
    assert!(matches!(
        life.grade(grade, EvidencePolicy::new(140, 20)),
        Err(LifecycleError::WrongTarget)
    ));
    assert_ne!(life.status(), LifecycleStatus::Closed);
}

#[test]
fn blocked_retains_external_provenance_and_serializes_all_fields() {
    let mut life = selected();
    let blocker = external_blocker(110);
    life.block(id("block-1"), blocker, EvidencePolicy::new(120, 20))
        .unwrap();

    let retained = life.blocker().expect("blocked lifecycle retains blocker");
    assert_eq!(retained.kind(), BlockerKind::ExternalDependency);
    assert_eq!(retained.reason().as_str(), "awaiting owner");
    assert_eq!(retained.escalation_ref().as_str(), "ESC-42");
    assert_eq!(retained.observed_at_ms(), 110);

    let event = life.history().last().expect("block event is recorded");
    let serialized: serde_json::Value = serde_json::from_str(&event.serialized()).unwrap();
    assert_eq!(serialized["status"], "blocked");
    assert_eq!(serialized["blocker"]["kind"], "external-dependency");
    assert_eq!(serialized["blocker"]["reason"], "awaiting owner");
    assert_eq!(serialized["blocker"]["escalation_ref"], "ESC-42");
    assert_eq!(serialized["blocker"]["observed_at_ms"], 110);
}

#[test]
fn external_blocker_requires_escalation_and_fresh_evidence() {
    assert!(EscalationRef::new(" ").is_err());
    assert!(matches!(
        BlockerEvidence::new(
            BlockerKind::ExternalDependency,
            BlockReason::new("awaiting owner").unwrap(),
            None,
            110,
        ),
        Err(LifecycleError::MissingEscalationReference)
    ));

    let mut life = selected();
    assert!(matches!(
        life.block(
            id("block-stale"),
            external_blocker(10),
            EvidencePolicy::new(120, 20),
        ),
        Err(LifecycleError::StaleEvidence { .. })
    ));
    assert!(matches!(
        life.block(
            id("block-future"),
            external_blocker(130),
            EvidencePolicy::new(120, 20),
        ),
        Err(LifecycleError::EvidenceInFuture { .. })
    ));
    assert_eq!(life.status(), LifecycleStatus::Selected);
}

#[test]
fn local_only_blocker_cannot_create_blocked_lifecycle() {
    let local = BlockerEvidence::new(
        BlockerKind::LocalCondition,
        BlockReason::new("local queue is paused").unwrap(),
        Some(EscalationRef::new("LOCAL-ESC").unwrap()),
        110,
    )
    .unwrap();
    let mut life = selected();

    assert!(matches!(
        life.block(id("block-local"), local, EvidencePolicy::new(120, 20)),
        Err(LifecycleError::LocalBlockerNotAllowed)
    ));
    assert_eq!(life.status(), LifecycleStatus::Selected);
    assert!(life.blocker().is_none());
}

#[test]
fn closed_lifecycle_cannot_become_blocked() {
    let mut life = grading();
    life.grade(
        GradeReceipt::pass(
            id("grade-1"),
            bead("cp-life"),
            target("4"),
            id("receiver-1"),
            130,
        ),
        EvidencePolicy::new(140, 20),
    )
    .unwrap();
    life.close(id("close-1")).unwrap();

    assert!(matches!(
        life.block(
            id("block-after-close"),
            external_blocker(140),
            EvidencePolicy::new(140, 20),
        ),
        Err(LifecycleError::InvalidTransition { .. })
    ));
    assert_eq!(life.status(), LifecycleStatus::Closed);
}

#[test]
fn blocked_is_terminal_and_status_names_are_stable() {
    let mut life = selected();
    let blocker = external_blocker(110);
    life.block(id("block-1"), blocker, EvidencePolicy::new(120, 20))
        .unwrap();
    assert_eq!(life.status(), LifecycleStatus::Blocked);
    assert!(matches!(
        life.dispatch(
            DispatchReceipt::new(
                id("dispatch"),
                bead("cp-life"),
                target("4"),
                "Implement lifecycle",
                100
            )
            .unwrap()
        ),
        Err(LifecycleError::InvalidTransition { .. })
    ));
    for (status, name) in [
        (LifecycleStatus::Selected, "selected"),
        (LifecycleStatus::Dispatched, "dispatched"),
        (LifecycleStatus::ReceiverVerified, "receiver_verified"),
        (LifecycleStatus::Ungraded, "ungraded"),
        (LifecycleStatus::Grading, "grading"),
        (LifecycleStatus::GradedPass, "graded_pass"),
        (LifecycleStatus::GradedFix, "graded_fix"),
        (LifecycleStatus::Closed, "closed"),
        (LifecycleStatus::RedispatchRequired, "redispatch_required"),
        (LifecycleStatus::Blocked, "blocked"),
    ] {
        assert_eq!(status.as_str(), name);
    }
}

#[test]
fn non_autonomous_wave_cannot_be_selected() {
    let wave = classify(Intent::new(TypedAction::SpendMoney));
    assert!(matches!(wave.verdict, WaveVerdict::Required { .. }));
    assert!(matches!(
        BeadLifecycle::select_wave(
            bead("cp-life"),
            target("4"),
            "Implement lifecycle",
            wave,
            id("select-required")
        ),
        Err(LifecycleError::NotApproved(_))
    ));
}

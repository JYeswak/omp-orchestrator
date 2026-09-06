use ntm_fleet_monitor::bead_lifecycle::ledger::{
    packet_digest, AppendOutcome, InvokerClass, LedgerError, LedgerEvidence, LifecycleIdentity,
    LifecycleLedger,
};
use ntm_fleet_monitor::bead_lifecycle::{
    BeadId, BlockReason, BlockerEvidence, BlockerKind, DispatchReceipt, DispatchTarget,
    EvidencePolicy, EventId, GradeReceipt, LifecycleStatus, ReceiverEvidence, RedispatchPlan,
};
use ntm_fleet_monitor::{classify, Approved, Intent, TypedAction};
use tempfile::tempdir;

fn bead() -> BeadId {
    BeadId::new("omp-orchestrator-life").unwrap()
}

fn target() -> DispatchTarget {
    DispatchTarget::new("omp-orchestrator", "%1409").unwrap()
}

fn approval() -> Approved {
    let wave = classify(Intent {
        action: TypedAction::DispatchPacket,
        pane_dispatchable: true,
        two_captures: true,
        packet_complete: true,
        finding_has_bead: true,
    });
    Approved::authorize(wave).unwrap()
}

fn evidence(id: &str, observed_at_ms: u64) -> LedgerEvidence {
    LedgerEvidence::single(
        EventId::new(id).unwrap(),
        observed_at_ms,
        EvidencePolicy::new(200, 200),
        "source",
        "test-observation",
    )
    .unwrap()
}

fn writer(path: std::path::PathBuf) -> LifecycleLedger {
    let identity = LifecycleIdentity::new(
        bead(),
        "/repo/omp-orchestrator",
        target(),
        packet_digest(b"dispatch packet"),
        InvokerClass::from_tty(false),
    )
    .unwrap();
    LifecycleLedger::start(path, identity, "drive bead to a verified close", approval(), evidence("selected", 100))
        .unwrap()
}

fn dispatch_receipt(id: &str, at: u64) -> DispatchReceipt {
    DispatchReceipt::new(
        EventId::new(id).unwrap(),
        bead(),
        target(),
        "drive bead to a verified close",
        at,
    )
    .unwrap()
}

fn receiver(id: &str, at: u64) -> ReceiverEvidence {
    ReceiverEvidence::new(
        EventId::new(id).unwrap(),
        bead(),
        target(),
        "drive bead to a verified close",
        at,
    )
    .unwrap()
}

fn read_events(path: &std::path::Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn full_chain_emits_every_pass_side_lifecycle_row_with_identity() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("bead-lifecycle.jsonl");
    let mut ledger = writer(path.clone());
    ledger
        .dispatch(dispatch_receipt("dispatch", 101), evidence("dispatch", 101))
        .unwrap();
    ledger
        .verify_receiver(receiver("receiver", 102), evidence("receiver", 102))
        .unwrap();
    ledger
        .start_grading(evidence("grading", 103), "%1414")
        .unwrap();
    ledger
        .grade(
            GradeReceipt::pass(
                EventId::new("grade").unwrap(),
                bead(),
                target(),
                "%1414",
                EventId::new("receiver").unwrap(),
                104,
            ),
            evidence("grade", 104),
        )
        .unwrap();
    ledger.close(evidence("close", 105)).unwrap();
    assert_eq!(ledger.status(), LifecycleStatus::Closed);

    let rows = read_events(&path);
    let events: Vec<&str> = rows.iter().map(|row| row["event"].as_str().unwrap()).collect();
    assert_eq!(
        events,
        [
            "selected",
            "dispatch_receipt",
            "receiver_verified",
            "grading_started",
            "grade_receipt",
            "graded_pass",
            "closed",
        ]
    );
    for row in rows {
        for key in [
            "bead",
            "repo",
            "session",
            "pane",
            "packet_digest",
            "idempotency_key",
            "freshness",
            "invoker",
            "evidence",
        ] {
            assert!(!row[key].is_null(), "event={} missing {key}", row["event"]);
        }
        assert_eq!(row["bead"], "omp-orchestrator-life");
        assert_eq!(row["repo"], "/repo/omp-orchestrator");
        assert_eq!(row["invoker"], "SCHEDULED");
        assert_eq!(row["freshness"]["within_window"], true);
    }
    assert!(LifecycleLedger::receiver_verified_candidates(&path).unwrap().is_empty());

}

#[test]
fn dispatched_without_receiver_report_is_redispatch_required() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("bead-lifecycle.jsonl");
    let mut ledger = writer(path.clone());
    ledger
        .dispatch(dispatch_receipt("dispatch", 101), evidence("dispatch", 101))
        .unwrap();
    let plan = RedispatchPlan::new(
        EventId::new("redispatch-required").unwrap(),
        bead(),
        target(),
        "receiver_not_reported",
    )
    .unwrap();
    ledger
        .require_redispatch(plan, evidence("redispatch-required", 199))
        .unwrap();
    assert_eq!(ledger.status(), LifecycleStatus::RedispatchRequired);
    let row = read_events(&path).last().cloned().unwrap();
    assert_eq!(row["event"], "redispatch_required");
    assert_eq!(row["named_acceptance"], "receiver_not_reported");
    assert_eq!(row["bead"], "omp-orchestrator-life");
    assert_eq!(row["pane"], "%1409");
}

#[test]
fn peer_claim_replays_receiver_verified_and_refuses_self_grade() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("bead-lifecycle.jsonl");
    let mut ledger = writer(path.clone());
    ledger
        .dispatch(dispatch_receipt("dispatch", 101), evidence("dispatch", 101))
        .unwrap();
    ledger
        .verify_receiver(receiver("receiver", 102), evidence("receiver", 102))
        .unwrap();

    let candidates = LifecycleLedger::receiver_verified_candidates(&path).unwrap();
    assert_eq!(candidates.len(), 1);
    let candidate = candidates.into_iter().next().unwrap();
    let claimed = LifecycleLedger::claim_peer_grading(
        &path,
        candidate,
        approval(),
        "%1414",
        evidence("grading", 103),
    )
    .unwrap();
    assert!(!claimed.replayed());
    let rows = read_events(&path);
    let row = rows.last().unwrap();
    assert_eq!(row["event"], "grading_started");
    assert_eq!(row["grader_pane"], "%1414");

    let mut direct = writer(temp.path().join("self-grade.jsonl"));
    direct
        .dispatch(dispatch_receipt("dispatch", 101), evidence("dispatch", 101))
        .unwrap();
    direct
        .verify_receiver(receiver("receiver", 102), evidence("receiver", 102))
        .unwrap();
    assert!(matches!(
        direct.start_grading(evidence("grading", 103), "%1409"),
        Err(LedgerError::Lifecycle(
            ntm_fleet_monitor::bead_lifecycle::LifecycleError::SelfGradeRefused
        ))
    ));
}

#[test]
fn fix_chain_names_acceptance_and_requires_redispatch_before_dispatch() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("bead-lifecycle.jsonl");
    let mut ledger = writer(path.clone());
    ledger
        .dispatch(dispatch_receipt("dispatch", 101), evidence("dispatch", 101))
        .unwrap();
    ledger
        .verify_receiver(receiver("receiver", 102), evidence("receiver", 102))
        .unwrap();
    ledger.start_grading(evidence("grading", 103), "%1414").unwrap();
    ledger
        .grade(
            GradeReceipt::fix(
                EventId::new("grade-fix").unwrap(),
                bead(),
                target(),
                "%1414",
                EventId::new("receiver").unwrap(),
                "add-independent-acceptance-proof",
                104,
            )
            .unwrap(),
            evidence("grade-fix", 104),
        )
        .unwrap();
    assert_eq!(ledger.status(), LifecycleStatus::GradedFix);
    ledger
        .require_redispatch(
            RedispatchPlan::new(
                EventId::new("redispatch").unwrap(),
                bead(),
                target(),
                "add-independent-acceptance-proof",
            )
            .unwrap(),
            evidence("redispatch", 105),
        )
        .unwrap();
    ledger
        .dispatch(dispatch_receipt("dispatch-2", 106), evidence("dispatch-2", 106))
        .unwrap();
    assert_eq!(ledger.status(), LifecycleStatus::Dispatched);
    let events: Vec<String> = read_events(&path)
        .iter()
        .map(|row| row["event"].as_str().unwrap().to_owned())
        .collect();
    assert!(events.iter().any(|event| event == "grade_receipt"));
    assert!(events.iter().any(|event| event == "graded_fix"));
    assert!(events.iter().any(|event| event == "redispatch_required"));
    assert!(events.iter().filter(|event| event.as_str() == "dispatch_receipt").count() == 2);
}

#[test]
fn the_three_invalid_closes_are_refused() {
    let temp = tempdir().unwrap();

    let mut selected = writer(temp.path().join("selected.jsonl"));
    assert!(matches!(
        selected.close(evidence("close-selected", 101)),
        Err(LedgerError::Lifecycle(_))
    ));

    let mut dispatched = writer(temp.path().join("dispatched.jsonl"));
    dispatched
        .dispatch(dispatch_receipt("dispatch", 101), evidence("dispatch", 101))
        .unwrap();
    assert!(matches!(
        dispatched.close(evidence("close-dispatched", 102)),
        Err(LedgerError::Lifecycle(_))
    ));

    let mut receiver_verified = writer(temp.path().join("receiver.jsonl"));
    receiver_verified
        .dispatch(dispatch_receipt("dispatch", 101), evidence("dispatch", 101))
        .unwrap();
    receiver_verified
        .verify_receiver(receiver("receiver", 102), evidence("receiver", 102))
        .unwrap();
    assert!(matches!(
        receiver_verified.close(evidence("close-receiver", 103)),
        Err(LedgerError::Lifecycle(_))
    ));
}

#[test]
fn unnamed_fix_and_empty_evidence_are_refused() {
    for name in ["   ", "try again", "retry", "fix"] {
        assert!(matches!(
            GradeReceipt::fix(
                EventId::new("grade").unwrap(),
                bead(),
                target(),
                "%1414",
                EventId::new("receiver").unwrap(),
                name,
                100,
            ),
            Err(ntm_fleet_monitor::bead_lifecycle::LifecycleError::InvalidInput {
                field: "fix_name"
            })
        ));
    }

    assert!(matches!(
        LedgerEvidence::new(
            EventId::new("empty").unwrap(),
            100,
            EvidencePolicy::new(100, 1),
            std::iter::empty::<(String, String)>()
        ),
        Err(LedgerError::EmptyEvidence)
    ));
}

#[test]
fn idempotent_replay_is_not_a_second_row_but_conflicting_reuse_refuses() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("bead-lifecycle.jsonl");
    let mut ledger = writer(path.clone());
    let receipt = dispatch_receipt("dispatch", 101);
    let first = ledger
        .dispatch(receipt.clone(), evidence("dispatch", 101))
        .unwrap();
    let replay = ledger
        .dispatch(receipt, evidence("dispatch", 101))
        .unwrap();
    assert!(matches!(first, AppendOutcome::Appended { .. }));
    assert!(matches!(replay, AppendOutcome::Replayed { .. }));
    assert_eq!(read_events(&path).len(), 2);
    let conflict = ledger.dispatch(
        dispatch_receipt("dispatch", 101),
        LedgerEvidence::single(
            EventId::new("dispatch").unwrap(),
            101,
            EvidencePolicy::new(200, 200),
            "source",
            "different-observation",
        )
        .unwrap(),
    );
    assert!(matches!(
        conflict,
        Err(LedgerError::ConflictingIdempotencyKey { .. })
    ));
}

#[test]
fn invoker_classification_is_not_constant_and_packet_digest_is_stable() {
    assert_eq!(InvokerClass::from_tty(false).as_str(), "SCHEDULED");
    assert_eq!(InvokerClass::from_tty(true).as_str(), "MANUAL");
    assert_eq!(packet_digest(b"packet"), packet_digest(b"packet"));
    assert_ne!(packet_digest(b"packet"), packet_digest(b"other"));
}

#[test]
fn external_blocker_rows_retain_escalation_provenance() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("blocked.jsonl");
    let mut ledger = writer(path.clone());
    let blocker = BlockerEvidence::new(
        BlockerKind::ExternalDependency,
        BlockReason::new("grader service unavailable").unwrap(),
        Some(ntm_fleet_monitor::bead_lifecycle::EscalationRef::new("BEAD-h5o8").unwrap()),
        101,
    )
    .unwrap();
    ledger
        .block(evidence("blocked", 101), blocker)
        .unwrap();
    assert_eq!(ledger.status(), LifecycleStatus::Blocked);
    let row = read_events(&path).last().unwrap().clone();
    assert_eq!(row["event"], "blocked");
    assert_eq!(row["blocker"]["kind"], "external-dependency");
    assert_eq!(row["blocker"]["escalation_ref"], "BEAD-h5o8");
}

#[test]
fn stale_receiver_event_is_refused_after_redispatch() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("stale-receiver.jsonl");
    let mut ledger = writer(path.clone());

    ledger
        .dispatch(dispatch_receipt("dispatch-1", 101), evidence("dispatch-1", 101))
        .unwrap();
    ledger
        .verify_receiver(receiver("receiver-1", 102), evidence("receiver-1", 102))
        .unwrap();
    ledger
        .start_grading(evidence("grading-1", 103), "%1414")
        .unwrap();
    ledger
        .grade(
            GradeReceipt::fix(
                EventId::new("grade-fix-1").unwrap(),
                bead(),
                target(),
                "%1414",
                EventId::new("receiver-1").unwrap(),
                "repair-stale-receiver-binding",
                104,
            )
            .unwrap(),
            evidence("grade-fix-1", 104),
        )
        .unwrap();
    ledger
        .require_redispatch(
            RedispatchPlan::new(
                EventId::new("redispatch-1").unwrap(),
                bead(),
                target(),
                "repair-stale-receiver-binding",
            )
            .unwrap(),
            evidence("redispatch-1", 105),
        )
        .unwrap();
    ledger
        .dispatch(dispatch_receipt("dispatch-2", 106), evidence("dispatch-2", 106))
        .unwrap();
    ledger
        .verify_receiver(receiver("receiver-2", 107), evidence("receiver-2", 107))
        .unwrap();
    assert_eq!(
        read_events(&path)
            .iter()
            .filter(|row| row["event"] == "receiver_verified")
            .count(),
        2
    );
    ledger
        .start_grading(evidence("grading-2", 108), "%1414")
        .unwrap();

    let stale_grade = ledger.grade(
        GradeReceipt::pass(
            EventId::new("grade-stale").unwrap(),
            bead(),
            target(),
            "%1414",
            EventId::new("receiver-1").unwrap(),
            109,
        ),
        evidence("grade-stale", 109),
    );
    assert!(matches!(
        stale_grade,
        Err(LedgerError::Lifecycle(
            ntm_fleet_monitor::bead_lifecycle::LifecycleError::WrongReceiverEvent
        ))
    ));
}

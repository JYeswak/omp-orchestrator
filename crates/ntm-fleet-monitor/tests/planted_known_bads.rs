//! Planted known-bads from the ntm-fleet-monitor skill measurements.
//! Each fixture is a claim that used to be untyped and therefore re-derived wrong.

use ntm_fleet_monitor::{
    claimed_blocker_kind, classify, ApprovalKind, Intent, Refusal, TypedAction, WaveVerdict,
};

#[test]
fn one_capture_cannot_authorize_dispatch() {
    let w = classify(Intent {
        action: TypedAction::DispatchPacket,
        pane_dispatchable: true,
        two_captures: false,
        packet_complete: true,
        finding_has_bead: true,
    });
    assert_eq!(
        w.verdict,
        WaveVerdict::Refuse {
            reason: Refusal::SingleCaptureLiveness
        },
        "skill: a single capture is a screenshot; Working and Frozen render identically"
    );
}

#[test]
fn dispatch_onto_a_working_pane_is_refuse_not_required() {
    let w = classify(Intent {
        action: TypedAction::DispatchPacket,
        pane_dispatchable: false,
        two_captures: true,
        packet_complete: true,
        finding_has_bead: true,
    });
    assert_eq!(
        w.verdict,
        WaveVerdict::Refuse {
            reason: Refusal::PaneNotDispatchable
        },
        "cp-rfx78 FALSE FREE: this is a safety refuse, not an escalation to Joshua"
    );
    assert!(!w.verdict.apply_allowed());
}

#[test]
fn finding_without_bead_is_refuse() {
    let w = classify(Intent::new(TypedAction::ReportFindingWithoutBead));
    assert_eq!(
        w.verdict,
        WaveVerdict::Refuse {
            reason: Refusal::FindingWithoutBead
        }
    );
}

#[test]
fn measured_false_gates_are_not_human_blockers() {
    assert_eq!(
        claimed_blocker_kind("APPROVAL GATE: purge APFS snapshots + Time Machine exclusions"),
        None
    );
    assert_eq!(
        claimed_blocker_kind("Joshua must dispose before dcg's rule set is changed"),
        None,
        "editing a local guard is reversible; parking it is the anti-pattern"
    );
}

#[test]
fn measured_real_blockers_are_required() {
    assert_eq!(
        claimed_blocker_kind("bind approval: mutations post to third-party repos = publishing"),
        Some(ApprovalKind::Deploy)
    );
    assert_eq!(
        claimed_blocker_kind("spend money on a GPU cloud bill"),
        Some(ApprovalKind::Spend)
    );
    assert_eq!(
        claimed_blocker_kind("obtain API credentials for the provider"),
        Some(ApprovalKind::Credentials)
    );
}

#[test]
fn recycle_frozen_is_autonomous_once_two_captures_exist() {
    let w = classify(Intent {
        action: TypedAction::RecycleFrozen,
        pane_dispatchable: false,
        two_captures: true,
        packet_complete: false,
        finding_has_bead: true,
    });
    assert_eq!(
        w.verdict,
        WaveVerdict::Autonomous,
        "skill: keeping the fleet alive needs no human approval; false-freeze is the two-capture refuse"
    );
}

#[test]
fn recycle_frozen_without_two_captures_is_refuse_not_autonomous() {
    for action in [
        TypedAction::RecycleFrozen,
        TypedAction::InterruptWedged,
        TypedAction::ReGoalIdle,
    ] {
        let w = classify(Intent::new(action));
        assert_eq!(
            w.verdict,
            WaveVerdict::Refuse {
                reason: Refusal::SingleCaptureLiveness
            },
            "{} with one capture is a screenshot; Working and Frozen render identically",
            action.as_str()
        );
        assert!(
            !w.verdict.apply_allowed(),
            "{} without two captures must not apply",
            action.as_str()
        );
    }
}

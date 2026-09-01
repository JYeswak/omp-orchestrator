use finding_dispatch::{finding_for, FINDING_THRESHOLD};
use omp_orchestrator::SupervisorDecision;

fn recurring_decisions() -> [SupervisorDecision; 4] {
    [
        SupervisorDecision::EscalateIdleIncident {
            dispatchable_count: 2,
            ready_count: 7,
        },
        SupervisorDecision::QueueEmptyNeedsJosh {
            free_capacity_count: 3,
        },
        SupervisorDecision::MonitorBlind {
            detail: "tmux census unavailable".to_owned(),
        },
        SupervisorDecision::WorkspaceUnloaded {
            detail: "workspace marker missing".to_owned(),
        },
    ]
}

#[test]
fn first_occurrence_is_log_only_and_nth_occurrence_files() {
    for decision in recurring_decisions() {
        assert!(finding_for(&decision, 1).is_none());
        assert!(finding_for(&decision, 2).is_none());
        let finding = finding_for(&decision, FINDING_THRESHOLD).expect("third occurrence files");
        let body = finding.body();
        assert!(body.contains("WHAT:"));
        assert!(body.contains("WHY:"));
        assert!(body.contains("ACCEPTANCE:"));
        assert!(!finding.labels().is_empty());
    }
}

#[test]
fn crossing_is_single_shot_not_a_duplicate_bead_stream() {
    let decision = SupervisorDecision::MonitorBlind {
        detail: "no readable census".to_owned(),
    };
    assert!(finding_for(&decision, FINDING_THRESHOLD).is_some());
    assert!(finding_for(&decision, FINDING_THRESHOLD + 1).is_none());
}

#[test]
fn supervised_working_never_files_a_finding() {
    let decision = SupervisorDecision::SupervisedWorking {
        working_count: 6,
        ready_count: 4,
    };
    for count in [1, FINDING_THRESHOLD, 100] {
        assert!(finding_for(&decision, count).is_none());
    }
}

#[test]
fn non_recurring_decisions_never_file_findings() {
    let decisions = [
        SupervisorDecision::Dispatch {
            pane: "%1".to_owned(),
            bead_hint: "ready".to_owned(),
        },
        SupervisorDecision::QueueUnreadable {
            detail: "queue unavailable".to_owned(),
        },
        SupervisorDecision::AuthorizedIdle {
            pane_count: 2,
            expires_at: 99,
        },
    ];
    for decision in decisions {
        assert!(finding_for(&decision, FINDING_THRESHOLD).is_none());
    }
}

#[test]
fn mutation_leg_threshold_change_would_fail_boundary() {
    // A mutant that changes the threshold from 3 to 2 must fail: the second occurrence
    // is still log-only.
    let decision = SupervisorDecision::QueueEmptyNeedsJosh {
        free_capacity_count: 1,
    };
    assert_eq!(FINDING_THRESHOLD, 3);
    assert!(finding_for(&decision, 2).is_none());
    assert!(finding_for(&decision, 3).is_some());
}

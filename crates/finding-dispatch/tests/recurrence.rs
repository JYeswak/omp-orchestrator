use finding_dispatch::{finding_for, MaybeFinding, NotYet, SupervisorDecision, FINDING_THRESHOLD};

/// UPDATED by `omp-orchestrator-finding-l1-bypassable-py3`. `finding_for` returned
/// `Option<Finding>`, and `#[must_use]` does not propagate through `Option` -- so the
/// compiler did not object when the only producer's result was dropped. It now returns
/// `#[must_use] MaybeFinding`, and the compiler named all nine call sites in this file
/// with `E0599`.
///
/// These assertions do NOT map mechanically back to an Option. They assert the TYPED
/// reason, because splitting `None` into `BelowThreshold` / `AlreadyEmitted` /
/// `NotAFindableDecision` is the point of the change: a caller logging "nothing to file"
/// can now say WHICH nothing it saw.
fn expect_not_yet(outcome: MaybeFinding, expected: NotYet) {
    match outcome.expect_nothing_owed() {
        Ok(reason) => assert_eq!(reason, expected, "wrong NotYet reason"),
        Err(finding) => panic!(
            "expected {expected}, got an owed finding: {}",
            finding.body()
        ),
    }
}

fn recurring_decisions() -> [SupervisorDecision; 5] {
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
        SupervisorDecision::GateUnwired {
            unwired: vec![
                "POSITIVE_CONTROL_FAILED: no-shell-gate must be reachable".to_owned(),
            ],
        },
    ]
}

#[test]
fn first_occurrence_is_log_only_and_nth_occurrence_files() {
    for decision in recurring_decisions() {
        expect_not_yet(
            finding_for(&decision, 1),
            NotYet::BelowThreshold {
                seen: 1,
                threshold: FINDING_THRESHOLD,
            },
        );
        expect_not_yet(
            finding_for(&decision, 2),
            NotYet::BelowThreshold {
                seen: 2,
                threshold: FINDING_THRESHOLD,
            },
        );
        let finding = finding_for(&decision, FINDING_THRESHOLD)
            .into_owed()
            .expect("third occurrence files");
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
    assert!(finding_for(&decision, FINDING_THRESHOLD).is_owed());
    expect_not_yet(
        finding_for(&decision, FINDING_THRESHOLD + 1),
        NotYet::AlreadyEmitted {
            seen: FINDING_THRESHOLD + 1,
            threshold: FINDING_THRESHOLD,
        },
    );
}

#[test]
fn supervised_working_never_files_a_finding() {
    let decision = SupervisorDecision::SupervisedWorking {
        working_count: 6,
        ready_count: 4,
    };
    // A healthy decision is NotAFindableDecision only AT the crossing; below and above
    // it the threshold arms answer first. Asserting the exact reason per count is what
    // makes this leg able to detect a mis-ordered guard.
    for count in [1, FINDING_THRESHOLD, 100] {
        let expected = match count {
            n if n < FINDING_THRESHOLD => NotYet::BelowThreshold {
                seen: n,
                threshold: FINDING_THRESHOLD,
            },
            n if n > FINDING_THRESHOLD => NotYet::AlreadyEmitted {
                seen: n,
                threshold: FINDING_THRESHOLD,
            },
            _ => NotYet::NotAFindableDecision,
        };
        expect_not_yet(finding_for(&decision, count), expected);
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
        expect_not_yet(
            finding_for(&decision, FINDING_THRESHOLD),
            NotYet::NotAFindableDecision,
        );
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
    expect_not_yet(
        finding_for(&decision, 2),
        NotYet::BelowThreshold {
            seen: 2,
            threshold: FINDING_THRESHOLD,
        },
    );
    assert!(finding_for(&decision, 3).is_owed());
}

#[test]
fn k0i6_third_gate_unwired_preserves_every_unreachable_name() {
    let names = [
        "POSITIVE_CONTROL_FAILED: no-shell-gate must be reachable",
        "path-literal-guard",
    ];
    let decision = SupervisorDecision::GateUnwired {
        unwired: names.iter().map(|n| (*n).to_owned()).collect(),
    };
    expect_not_yet(
        finding_for(&decision, 1),
        NotYet::BelowThreshold {
            seen: 1,
            threshold: FINDING_THRESHOLD,
        },
    );
    expect_not_yet(
        finding_for(&decision, 2),
        NotYet::BelowThreshold {
            seen: 2,
            threshold: FINDING_THRESHOLD,
        },
    );
    let finding = finding_for(&decision, FINDING_THRESHOLD)
        .into_owed()
        .expect("third GateUnwired observation files");
    let body = finding.body();
    for name in names {
        assert!(
            body.contains(name),
            "finding must preserve unreachable gate name {name:?}, got {body}"
        );
    }
    assert!(
        body.contains("repair-gate-trigger"),
        "finding must route trigger repair before dispatch resumes, got {body}"
    );
    assert!(
        finding.labels().iter().any(|l| l == "repair-gate-trigger"),
        "repair-gate-trigger must be a label so the finding routes, got {:?}",
        finding.labels()
    );
    expect_not_yet(
        finding_for(&decision, FINDING_THRESHOLD + 1),
        NotYet::AlreadyEmitted {
            seen: FINDING_THRESHOLD + 1,
            threshold: FINDING_THRESHOLD,
        },
    );
}

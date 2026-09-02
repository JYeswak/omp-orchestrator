use omp_types::pane_observation::{
    CaptureSnapshot, DispatchAdmissibility, EvidenceGrade, MIN_TWO_CAPTURE_INTERVAL_SECS,
    PaneLiveness, PaneObservation, UnknownReason,
};

fn snapshot(at: u64, liveness: PaneLiveness, timer: Option<&str>, hash: &str) -> CaptureSnapshot {
    CaptureSnapshot::new(at, liveness, timer.map(str::to_owned), hash.to_owned())
}

#[test]
fn law_l1_two_capture_motion_dominates_single_capture() {
    let single = EvidenceGrade::SingleCapture;
    for interval in [MIN_TWO_CAPTURE_INTERVAL_SECS, 76, 150, 10_000] {
        for timer_changed in [false, true] {
            for hash_changed in [false, true] {
                let previous = snapshot(100, PaneLiveness::Idle, Some("1m"), "same");
                let current = snapshot(
                    100 + interval,
                    PaneLiveness::Working { elapsed_secs: 2 },
                    if timer_changed {
                        Some("2s")
                    } else {
                        Some("1m")
                    },
                    if hash_changed { "changed" } else { "same" },
                );
                let result = PaneObservation::from_two_captures(
                    "pane-1",
                    previous,
                    current,
                    DispatchAdmissibility::Unknown,
                );
                if timer_changed || hash_changed {
                    let observation = result.expect("meaningful two-capture motion");
                    assert!(observation.evidence().dominates(&single));
                } else {
                    assert!(
                        result.is_err(),
                        "unchanged captures cannot be strongest evidence"
                    );
                }
            }
        }
    }
    assert!(
        !single.dominates(&single),
        "one capture cannot dominate itself"
    );
}

#[test]
fn law_l2_unknown_never_becomes_idle() {
    for reason in [
        UnknownReason::Unreadable,
        UnknownReason::MissingLastStatusLine,
        UnknownReason::Unclassified,
    ] {
        let observation = PaneObservation::unknown("pane-unknown", reason);
        assert!(matches!(observation.liveness(), PaneLiveness::Unknown(_)));
        assert!(!matches!(observation.liveness(), PaneLiveness::Idle));
    }
}

#[test]
fn law_l3_constructor_uses_last_status_line_value() {
    let working = PaneObservation::from_last_status_line(
        "pane-working",
        PaneLiveness::Working { elapsed_secs: 4 },
        DispatchAdmissibility::Unknown,
    );
    assert!(matches!(
        working.liveness(),
        PaneLiveness::Working { elapsed_secs: 4 }
    ));
    assert!(matches!(working.evidence(), EvidenceGrade::SingleCapture));

    let idle = PaneObservation::from_last_status_line(
        "pane-idle",
        PaneLiveness::Idle,
        DispatchAdmissibility::Unknown,
    );
    assert!(matches!(idle.liveness(), PaneLiveness::Idle));
}

#[test]
fn law_l4_liveness_is_single_exclusive_value() {
    let values = [
        PaneLiveness::Unknown(UnknownReason::Unclassified),
        PaneLiveness::Idle,
        PaneLiveness::Working { elapsed_secs: 1 },
    ];
    for value in values {
        let observation = PaneObservation::from_last_status_line(
            "pane-exclusive",
            value,
            DispatchAdmissibility::Unknown,
        );
        let variants = [
            matches!(observation.liveness(), PaneLiveness::Unknown(_)),
            matches!(observation.liveness(), PaneLiveness::Idle),
            matches!(observation.liveness(), PaneLiveness::Working { .. }),
        ];
        assert_eq!(variants.into_iter().filter(|present| *present).count(), 1);
    }
}

#[test]
fn law_l5_dispatch_admissibility_is_independent() {
    let admissibilities = [
        DispatchAdmissibility::Unknown,
        DispatchAdmissibility::Allowed,
        DispatchAdmissibility::Refused,
    ];
    for admissibility in admissibilities {
        let observation = PaneObservation::from_last_status_line(
            "pane-independent",
            PaneLiveness::Unknown(UnknownReason::Unreadable),
            admissibility,
        );
        assert!(matches!(observation.liveness(), PaneLiveness::Unknown(_)));
        assert!(matches!(
            observation.evidence(),
            EvidenceGrade::SingleCapture
        ));
        assert_eq!(observation.dispatch_admissibility(), &admissibility);
    }

    let allowed = PaneObservation::from_last_status_line(
        "pane-independent",
        PaneLiveness::Working { elapsed_secs: 3 },
        DispatchAdmissibility::Allowed,
    );
    assert!(matches!(allowed.liveness(), PaneLiveness::Working { .. }));
    assert!(matches!(
        allowed.dispatch_admissibility(),
        DispatchAdmissibility::Allowed
    ));
}

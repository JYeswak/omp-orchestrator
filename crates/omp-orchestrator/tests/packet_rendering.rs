use dispatch_claim_fence::BeadSnapshot;
use omp_orchestrator::dispatch_packet::{render, PacketError};
use std::path::Path;

fn bead(id: &str, description: &str, acceptance: &str) -> BeadSnapshot {
    BeadSnapshot::new_with_acceptance(id, "packet fixture", description, acceptance, "open", None)
}

#[test]
fn zrq_positive_control_carries_required_packet_fields() {
    let packet = render(
        &bead(
            "omp-orchestrator-zrq",
            "body",
            "Run cargo test -p zrq; expect exit 0",
        ),
        Path::new("/repo"),
        Some("the selector chose this bead"),
        None,
    )
    .expect("zrq packet should render");
    for field in [
        "Objective:",
        "Target:",
        "Scope:",
        "Acceptance:",
        "Done:",
        "Stop:",
    ] {
        assert!(
            packet.contains(field),
            "missing required packet field {field}: {packet}"
        );
    }
    assert!(packet.contains("Run cargo test -p zrq; expect exit 0"));
    assert!(packet.contains("Why this, why now: the selector chose this bead"));
}

#[test]
fn empty_acceptance_is_refused_by_typed_field_name() {
    let error = render(
        &bead("empty", "no acceptance", ""),
        Path::new("/repo"),
        None,
        None,
    )
    .expect_err("empty acceptance must refuse");
    assert_eq!(error, PacketError::PacketFieldMissing("acceptance"));
}

#[test]
fn typed_acceptance_wins_over_description_fallback() {
    let packet = render(
        &bead("typed", "## ACCEPTANCE\nwrong fallback", "typed criterion"),
        Path::new("/repo"),
        None,
        None,
    )
    .expect("typed acceptance should render");
    assert!(packet.contains("typed criterion"));
    assert!(!packet.contains("wrong fallback"));
}

#[test]
fn bead_815_shape_carries_all_nonduplicated_acceptance_lines() {
    let acceptance = (1..=41)
        .map(|line| format!("{line}. acceptance item {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let packet = render(
        &bead("omp-orchestrator-815", "body omits acceptance", &acceptance),
        Path::new("/repo"),
        None,
        None,
    )
    .expect("815 packet should render");
    for line in acceptance.lines() {
        assert!(
            packet.contains(line),
            "missing typed acceptance line: {line}"
        );
    }
}

#[test]
fn traps_that_add_acceptance_scope_are_refused() {
    let error = render(
        &bead("8e1g", "body", "typed acceptance"),
        Path::new("/repo"),
        None,
        Some("ACCEPTANCE (mine)\n1. MUST add another criterion"),
    )
    .expect_err("hand traps must not add acceptance scope");
    assert!(matches!(
        error,
        PacketError::PacketAddsScope { line: 1, .. }
    ));
    assert!(error
        .to_string()
        .contains("br update 8e1g --acceptance-criteria"));
}

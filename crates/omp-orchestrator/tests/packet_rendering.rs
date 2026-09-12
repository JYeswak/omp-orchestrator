use dispatch_claim_fence::BeadSnapshot;
use omp_orchestrator::dispatch_packet::{render, render_with_pane, PacketError};
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
    assert!(
        packet.contains("br comments add omp-orchestrator-zrq"),
        "{packet}"
    );
    assert!(packet.contains("ACK zrq on $TMUX_PANE --"), "{packet}");
    assert!(
        packet.contains("An ACK proves arrival and reading, never the work."),
        "{packet}"
    );
    assert!(packet.contains("Every bead requires current-state validation: re-run br show omp-orchestrator-zrq --json immediately before editing;"), "{packet}");
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
#[test]
fn pane_and_supervisor_handoff_are_carried() {
    let snapshot = BeadSnapshot::new_with_acceptance(
        "handoff",
        "packet fixture",
        "body",
        "Run cargo test; expect exit 0",
        "in_progress",
        Some("supervisor:123;pane=%1414;incarnation=1;agent=WildStone"),
    );
    let packet = render_with_pane(
        &snapshot,
        Path::new("/repo"),
        Some("%1414"),
        None,
        None,
        None,
    )
    .expect("pane packet should render");
    assert!(packet.contains("Pane: %1414"), "{packet}");
    assert!(packet.contains("Handoff: supervisor:123"), "{packet}");
}

#[test]
fn every_rendered_bead_packet_carries_the_ack_instruction() {
    let packet = render_with_pane(
        &BeadSnapshot::new_with_acceptance(
            "omp-orchestrator-kxe.4",
            "packet fixture",
            "body",
            "Run cargo test -p kxe; expect exit 0",
            "in_progress",
            Some("pane=%1413;incarnation=1;agent=WildStone"),
        ),
        Path::new("/repo"),
        Some("%1413"),
        Some("WildStone"),
        None,
        None,
    )
    .expect("packet should render");
    assert!(
        packet.contains("br comments add omp-orchestrator-kxe.4"),
        "{packet}"
    );
    assert!(packet.contains("ACK kxe.4 on $TMUX_PANE --"), "{packet}");
}

#[test]
fn filed_only_record_is_refused_before_packet_render() {
    let error = render(
        &bead(
            "omp-orchestrator-s1-l0-t14-pccb",
            "STATUS: filed only; do not claim or implement in this bead.",
            "Run the recorded acceptance only; expect no implementation.",
        ),
        Path::new("/repo"),
        None,
        None,
    )
    .expect_err("filed-only records must not become work packets");
    assert!(error.to_string().contains("PACKET_REFUSED_FILED_ONLY"));
    assert!(matches!(
        error,
        PacketError::FiledOnlyRecord {
            bead,
            marker: "filed only"
        } if bead == "omp-orchestrator-s1-l0-t14-pccb"
    ));
}

#[test]
fn quoted_filed_only_language_is_not_a_marker() {
    let packet = render(
        &bead(
            "b4iv",
            "This audit quotes 'filed only' and 'do not claim' as examples.",
            "Run cargo test -p b4iv; expect exit 0",
        ),
        Path::new("/repo"),
        None,
        None,
    )
    .expect("quoted marker language must be ignored");
    assert!(packet.contains("Objective: Complete bead b4iv"));
}

#[test]
fn named_mutation_site_renders_and_vague_clause_is_refused() {
    let packet = render(
        &bead(
            "omp-orchestrator-dwj4v",
            "body",
            "Mutation: duplicate_block_mapping_key at gate-reachability.rs:216",
        ),
        Path::new("/repo"),
        None,
        None,
    )
    .expect("named file.rs:line must stay admissible");
    assert!(packet.contains("gate-reachability.rs:216"));

    let error = render(
        &bead(
            "omp-orchestrator-dwj4v",
            "body",
            "Mutation: un-call the validator",
        ),
        Path::new("/repo"),
        None,
        None,
    )
    .expect_err("vague mutation clause must refuse");
    assert_eq!(error.code(), "MUTATION_CLAUSE_TOO_COARSE");
}

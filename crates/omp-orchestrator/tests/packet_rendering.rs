use dispatch_claim_fence::BeadSnapshot;
use omp_orchestrator::dispatch_packet::{
    acceptance_text, classify_mutation_clause, render, render_with_pane, MutationClauseClass,
    PacketError, MUTATION_SITE_CUTOFF, POST_CUTOFF_COARSE_CEILING, PRE_CUTOFF_COARSE_CEILING,
};
use std::fs;
use std::path::{Path, PathBuf};

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

#[test]
fn live_ledger_mutation_site_debt_only_falls() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("CARGO_MANIFEST_DIR is crates/<pkg>")
        .to_path_buf();
    let path = repo.join(".beads/issues.jsonl");
    let text = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "ANTI-VACUITY: cannot read {}: {error}. An unreadable ledger is an ERROR, never a \
             zero-refusal pass.",
            path.display()
        )
    });
    assert!(
        !text.trim().is_empty(),
        "ANTI-VACUITY: {} is empty. An empty scan set is an ERROR, never a pass.",
        path.display()
    );

    let mut parsed = 0usize;
    let mut dispatchable = 0usize;
    let mut pre_cutoff_coarse = 0usize;
    let mut post_cutoff_coarse = 0usize;
    let mut named = 0usize;
    let mut missing = 0usize;
    let mut none = 0usize;
    let mut post_cutoff_seen = 0usize;

    for (number, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(line).unwrap_or_else(|error| {
            panic!("{}:{}: not JSON ({error})", path.display(), number + 1)
        });
        parsed += 1;
        let id = value["id"].as_str().unwrap_or_default();
        assert!(
            !id.is_empty(),
            "{}:{}: bead record has no id",
            path.display(),
            number + 1
        );
        let status = value["status"].as_str().unwrap_or_default();
        if status == "closed" || status == "tombstone" {
            continue;
        }
        let snapshot = BeadSnapshot::new_with_acceptance(
            id,
            value["title"].as_str().unwrap_or_default(),
            value["description"].as_str().unwrap_or_default(),
            value["acceptance_criteria"].as_str().unwrap_or_default(),
            status,
            None,
        );
        let Some(acceptance) = acceptance_text(&snapshot) else {
            continue;
        };
        dispatchable += 1;
        let created: String = value["created_at"]
            .as_str()
            .map(|stamp| stamp.chars().take(10).collect())
            .filter(|day: &String| day.len() == 10)
            .unwrap_or_else(|| "9999-99-99".to_owned());
        let post = created.as_str() >= MUTATION_SITE_CUTOFF;
        if post {
            post_cutoff_seen += 1;
        }
        match classify_mutation_clause(&acceptance) {
            MutationClauseClass::None => none += 1,
            MutationClauseClass::NamedSite => named += 1,
            MutationClauseClass::Missing => missing += 1,
            MutationClauseClass::Coarse(_) => {
                if post {
                    post_cutoff_coarse += 1;
                } else {
                    pre_cutoff_coarse += 1;
                }
            }
        }
    }

    assert!(
        parsed > 0,
        "ANTI-VACUITY: parsed zero beads from {}",
        path.display()
    );
    assert!(
        dispatchable > 0,
        "ANTI-VACUITY: zero dispatchable beads; the scan classified nothing"
    );
    assert!(
        post_cutoff_seen > 0,
        "ANTI-VACUITY: no live bead created on/after {MUTATION_SITE_CUTOFF}; the delta leg would \
         scan empty and report green"
    );
    assert!(
        pre_cutoff_coarse > 0,
        "ANTI-VACUITY: pre-cutoff coarse is 0; the classifier is no longer seeing historical \
         mutation clauses (a silent no-op wearing a falling ratchet)"
    );

    eprintln!(
        "SCAN: parsed={parsed} dispatchable={dispatchable} none={none} named={named} \
         missing={missing} pre_cutoff_coarse={pre_cutoff_coarse}/{PRE_CUTOFF_COARSE_CEILING} \
         post_cutoff_coarse={post_cutoff_coarse}/{POST_CUTOFF_COARSE_CEILING} \
         post_cutoff_seen={post_cutoff_seen}"
    );

    assert!(
        pre_cutoff_coarse <= PRE_CUTOFF_COARSE_CEILING,
        "pre-cutoff coarse {pre_cutoff_coarse} exceeded ceiling {PRE_CUTOFF_COARSE_CEILING}; the \
         ratchet may only fall"
    );
    assert!(
        post_cutoff_coarse <= POST_CUTOFF_COARSE_CEILING,
        "post-cutoff coarse {post_cutoff_coarse} exceeded ceiling {POST_CUTOFF_COARSE_CEILING}; \
         new work must name file.rs:N rather than raise the ceiling"
    );
}

use s1_coverage::{
    compute, render_markdown, validate_doc_only_reason, BeadRecord, CoverageError, CoverageInput,
    CoverageState, SourceText,
};

fn input(contract: &str, beads: Vec<BeadRecord>) -> CoverageInput {
    CoverageInput {
        contracts: vec![SourceText {
            path: "docs/contracts/s1_l1_doctor.md".to_owned(),
            text: contract.to_owned(),
        }],
        s1_toml: String::new(),
        beads,
    }
}

fn bead(id: &str, title: &str, acceptance: &str, description: &str) -> BeadRecord {
    BeadRecord {
        id: id.to_owned(),
        title: title.to_owned(),
        acceptance_criteria: acceptance.to_owned(),
        description: description.to_owned(),
    }
}

#[test]
fn description_only_id_does_not_cover() {
    let report = compute(
        &input(
            "required `L1-REAL`",
            vec![bead(
                "omp-orchestrator-description-only",
                "unrelated title",
                "",
                "L1-REAL is mentioned only here",
            )],
        ),
        "tree",
        "HEAD",
    )
    .unwrap();
    assert_eq!(report.requirements[0].state, CoverageState::Missing);
    assert_eq!(report.requirements[0].bead_id, None);
}

#[test]
fn title_and_acceptance_use_exact_word_boundary() {
    let report = compute(
        &input(
            "required `L1-REAL`",
            vec![bead(
                "omp-orchestrator-title-hit",
                "L1-REAL",
                "",
                "",
            )],
        ),
        "tree",
        "HEAD",
    )
    .unwrap();
    assert_eq!(report.requirements[0].state, CoverageState::Covered);

    let report = compute(
        &input(
            "required `L1-REAL`",
            vec![bead(
                "omp-orchestrator-acceptance-hit",
                "unrelated",
                "acceptance: `L1-REAL`",
                "",
            )],
        ),
        "tree",
        "HEAD",
    )
    .unwrap();
    assert_eq!(report.requirements[0].state, CoverageState::Covered);

    let report = compute(
        &input(
            "required `L1-REAL`",
            vec![bead(
                "omp-orchestrator-generated-id",
                "omp-orchestrator-l1-real-aaaa",
                "",
                "",
            )],
        ),
        "tree",
        "HEAD",
    )
    .unwrap();
    assert_eq!(report.requirements[0].state, CoverageState::Missing);
}

#[test]
fn cross_references_and_wbs_are_not_requirements() {
    let report = compute(
        &input(
            "main `L1-REAL`\n\n## WBS\n`L1-WBS`\n\n## Cross-References\n`L1-XREF`\n",
            Vec::new(),
        ),
        "tree",
        "HEAD",
    )
    .unwrap();
    let ids: Vec<_> = report
        .requirements
        .iter()
        .map(|row| row.stable_id.as_str())
        .collect();
    assert!(ids.contains(&"L1-REAL"));
    assert!(!ids.contains(&"L1-WBS"));
    assert!(!ids.contains(&"L1-XREF"));
}

#[test]
fn unbackticked_stable_id_is_declared_unextractable() {
    let report = compute(&input("missing **LAW-L1-HIDDEN**", Vec::new()), "tree", "HEAD").unwrap();
    assert_eq!(report.requirements[0].state, CoverageState::DeclaredUnextractable);
    assert!(report.requirements[0]
        .reason
        .as_deref()
        .is_some_and(|reason| reason.contains("backtick")));
    let report = compute(&input("missing LAW-L1-PLAIN", Vec::new()), "tree", "HEAD").unwrap();
    assert_eq!(report.requirements[0].state, CoverageState::DeclaredUnextractable);
}

#[test]
fn empty_scan_fails_closed() {
    let result = compute(
        &CoverageInput {
            contracts: Vec::new(),
            s1_toml: String::new(),
            beads: Vec::new(),
        },
        "tree",
        "HEAD",
    );
    assert_eq!(result, Err(CoverageError::ScanEmpty));
    let result = compute(&input("", Vec::new()), "tree", "HEAD");
    assert_eq!(result, Err(CoverageError::ScanEmpty));
}

#[test]
fn doc_only_without_reason_fails_closed() {
    assert_eq!(
        validate_doc_only_reason(CoverageState::DocOnly, ""),
        Err(CoverageError::InvalidDocOnlyReason)
    );
}

#[test]
fn s1_toml_produces_per_source_predicated_rows() {
    let report = compute(
        &CoverageInput {
            contracts: vec![SourceText {
                path: "docs/contracts/s1_l1_doctor.md".to_owned(),
                text: "`L1-REAL`".to_owned(),
            }],
            s1_toml: r#"
[[box.gap]]
what = "a gap"
resolves = "omp-orchestrator-gap-aaaa"
[[box.observability]]
layer = "L1"
[[box.hook]]
surface = "pre-commit"
[[box.layer]]
id = "L1"
decision = "HD-0009"
exists = "none"
branches = ["one", "two"]
"#
            .to_owned(),
            beads: Vec::new(),
        },
        "tree",
        "HEAD",
    )
    .unwrap();
    assert!(report
        .source_breakdown
        .iter()
        .any(|row| row.source == "box.observability"));
    assert!(report
        .source_breakdown
        .iter()
        .any(|row| row.source == "decisions.HD"));
    assert!(report
        .requirements
        .iter()
        .all(|row| !row.predicate.is_empty()));
    assert!(render_markdown(&report).contains("DECLARED-UNEXTRACTABLE"));
}

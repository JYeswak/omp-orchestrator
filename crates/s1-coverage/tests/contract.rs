use s1_coverage::{
    compare_reports, compute, compute_with_manifest, parse_beads_jsonl, render_markdown,
    validate_doc_only_reason, validate_manifest, BeadRecord, ConvergenceDecision, CoverageError, CoverageInput, CoverageState,
    InputManifest, InputState, ManifestVerdict, SourceText,
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
                "omp-orchestrator-L1-REAL-aaaa",
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
#[test]
fn tree_index_worktree_provenance_is_typed() {
    let manifest = InputManifest::new(
        "HEAD",
        vec!["tree-input".to_owned()],
        vec!["index-input".to_owned()],
        vec!["worktree-input".to_owned()],
        vec!["only-input".to_owned()],
    );
    assert_eq!(manifest.paths(InputState::Tree), &["tree-input".to_owned()]);
    assert_eq!(manifest.paths(InputState::Index), &["index-input".to_owned()]);
    assert_eq!(manifest.paths(InputState::Worktree), &["worktree-input".to_owned()]);
    assert_eq!(manifest.paths(InputState::WorktreeOnly), &["only-input".to_owned()]);
}

#[test]
fn clean_tree_is_consistent() {
    let manifest = InputManifest::new(
        "HEAD",
        vec!["docs/plan/flow/S1-COVERAGE.md".to_owned(), "crates/s1-coverage/Cargo.toml".to_owned(), "crates/s1-coverage/src/main.rs".to_owned()],
        vec!["docs/plan/flow/S1-COVERAGE.md".to_owned(), "crates/s1-coverage/Cargo.toml".to_owned(), "crates/s1-coverage/src/main.rs".to_owned()],
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(validate_manifest(&manifest), Ok(ManifestVerdict::DenominatorConsistent));
    let report = compute_with_manifest(
        &input("required `L1-REAL`", Vec::new()),
        "tree",
        "HEAD",
        manifest,
    )
    .expect("clean fixture computes");
    assert_eq!(report.manifest_verdict, ManifestVerdict::DenominatorConsistent);
    assert!(report.generator_tracked);
    assert!(render_markdown(&report).contains("GENERATOR_TRACKED=YES"));
    assert!(report.worktree_only.is_empty());
}

#[test]
fn worktree_only_requirement_is_restrictive() {
    let mut manifest = InputManifest::new(
        "HEAD",
        vec!["tracked".to_owned()],
        vec!["tracked".to_owned()],
        Vec::new(),
        Vec::new(),
    );
    let original = manifest.clone();
    manifest.worktree_only.push(".git/s1_cov.py".to_owned());
    let error = validate_manifest(&manifest).expect_err("worktree-only input must refuse");
    assert_eq!(error.exit_code(), 2);
    assert_eq!(
        error.to_string(),
        "DENOMINATOR_WORKTREE_ONLY: paths=.git/s1_cov.py"
    );
    manifest = original.clone();
    assert_eq!(manifest, original);
    assert_eq!(validate_manifest(&manifest), Ok(ManifestVerdict::DenominatorConsistent));
}

#[test]
fn empty_input_is_error() {
    let error = compute(
        &CoverageInput {
            contracts: Vec::new(),
            s1_toml: String::new(),
            beads: Vec::new(),
        },
        "tree",
        "HEAD",
    )
    .expect_err("empty input must refuse");
    assert_eq!(error.exit_code(), 3);
    assert_eq!(
        error.to_string(),
        "DENOMINATOR_EMPTY_SCAN_SET: no requirement inputs were provided"
    );
}

#[test]
fn growth_and_closure_use_relation_not_absolute_size() {
    let base = compute(&input("required `L1-REAL`", Vec::new()), "tree", "base").unwrap();
    let closed = compute(
        &input(
            "required `L1-REAL`",
            vec![bead("closed", "L1-REAL", "", "")],
        ),
        "tree",
        "closed",
    )
    .unwrap();
    let closure = compare_reports(&base, &closed);
    assert_eq!(closure.base_revision, "base");
    assert_eq!(closure.head_revision, "closed");
    assert_eq!(closure.growth, 0);
    assert_eq!(closure.closure, 1);
    assert_eq!(closure.decision, ConvergenceDecision::Converging);

    let expanded = compute(
        &input("required `L1-REAL`\nrequired `L1-NEW`", Vec::new()),
        "tree",
        "expanded",
    )
    .unwrap();
    let growth = compare_reports(&base, &expanded);
    assert_eq!(growth.growth, 1);
    assert_eq!(growth.closure, 0);
    assert_eq!(growth.decision, ConvergenceDecision::NonConverging);
}

#[test]
fn malformed_jsonl_is_a_distinct_named_error() {
    let error = parse_beads_jsonl("{not-json").expect_err("malformed JSONL must refuse");
    assert_eq!(error, CoverageError::MalformedBead { line: 1 });
    assert_eq!(error.to_string(), "BEAD_JSON_INVALID: line=1");
}

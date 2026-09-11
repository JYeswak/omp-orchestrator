use s1_coverage::{
    checkout_cannot_resolve, checkout_unusable, classify_convergence, compare_reports, compute,
    compute_with_manifest, parse_beads_jsonl, refusal_exit_code, render_markdown,
    validate_doc_only_reason, validate_manifest, BeadRecord, ConvergenceDecision, CoverageError,
    CoverageInput, CoverageState, CoverageInputManifest, InputState, ManifestVerdict, SourceText,
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
    let manifest = CoverageInputManifest::new(
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
    let manifest = CoverageInputManifest::new(
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
    let mut manifest = CoverageInputManifest::new(
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
fn equal_growth_and_closure_is_not_converging() {
    // THE RESTING STATE OF A WAVE, and the case production emits today: run 34554312190 on
    // 947e3e2 reported growth 0, closure 0, NON_CONVERGING. The two rows above survive both
    // `>` and `>=` (1>0 true and 0>=1 false under either), so the equality case is the ONLY
    // one that makes a `closure >= growth` mutation red. A stalled wave must never read as
    // converging.
    assert_eq!(classify_convergence(0, 0), ConvergenceDecision::NonConverging);
    assert_eq!(classify_convergence(3, 3), ConvergenceDecision::NonConverging);
    // The strict neighbours of the boundary, so the relation is pinned on both sides of equality.
    assert_eq!(classify_convergence(3, 4), ConvergenceDecision::Converging);
    assert_eq!(classify_convergence(4, 3), ConvergenceDecision::NonConverging);
}

#[test]
fn depth_zero_checkout_wording_is_attributed_to_the_checkout() {
    // MEASURED on an rch worker 2026-09-10 by StaleHD13: the remote repo is an initialised .git
    // with NO commits and NO objects, and `rev-parse HEAD` there fails with `ambiguous argument`,
    // NOT `invalid object name`. A needle set guessed from the shallow hypothesis misses it.
    let detail = "fatal: ambiguous argument 'HEAD': unknown revision or path not in the working tree.";
    assert!(checkout_cannot_resolve(detail));
    assert_eq!(refusal_exit_code(&checkout_unusable("HEAD", detail)), 4);
    assert!(checkout_cannot_resolve(
        "fatal: your current branch 'main' does not have any commits yet"
    ));
}

#[test]
fn malformed_jsonl_is_a_distinct_named_error() {
    let error = parse_beads_jsonl("{not-json").expect_err("malformed JSONL must refuse");
    assert_eq!(error, CoverageError::MalformedBead { line: 1 });
    assert_eq!(error.to_string(), "BEAD_JSON_INVALID: line=1");
}

#[test]
fn unresolvable_revision_is_attributed_to_the_checkout_not_the_crate() {
    // git's own wording for a rev the checkout cannot resolve. Observed remotely 2026-09-10.
    let detail = "fatal: invalid object name 'HEAD~1'.";
    assert!(checkout_cannot_resolve(detail));
    let message = checkout_unusable("HEAD~1", detail);
    assert_eq!(refusal_exit_code(&message), 4);
    assert_eq!(
        message,
        "S1_COVERAGE_CHECKOUT_UNUSABLE revision=HEAD~1 cause=CHECKOUT_CANNOT_RESOLVE_REVISION \
detail=fatal: invalid object name 'HEAD~1'. note=the checkout under test cannot resolve this \
revision; the denominator is UNKNOWN, not zero"
    );
    // The harm being fixed: the refusal must not read as a defect in this crate.
    assert!(!message.contains("S1_COVERAGE_TREE_READ"));
    assert!(message.contains("CHECKOUT"));
}

#[test]
fn crate_refusals_keep_exit_two_so_the_codes_stay_distinguishable() {
    assert_eq!(refusal_exit_code("S1_COVERAGE_TREE_READ path=x error=y"), 2);
    assert_eq!(
        refusal_exit_code(&CoverageError::ScanEmpty.to_string()),
        2,
        "an empty scan set is this crate's refusal, not the checkout's"
    );
}

#[test]
fn on_disk_hint_is_not_read_as_a_resolvability_oracle() {
    // This message varies with ON-DISK PRESENCE and says nothing about whether the rev resolved.
    // Three readers drew three different conclusions from it on 2026-09-10; keying on it is
    // forbidden. Negative control: an unrelated git failure must also not match.
    assert!(!checkout_cannot_resolve(
        "fatal: path 'docs/contracts/s1_l0_install.md' exists on disk, but not in 'c6d8cfe'"
    ));
    assert!(!checkout_cannot_resolve("fatal: does not exist in 'c6d8cfe'"));
    assert!(!checkout_cannot_resolve("error: permission denied"));
    // Positive control drawn from the same class, so the zeros above are interpretable.
    assert!(checkout_cannot_resolve("fatal: not a git repository"));
}

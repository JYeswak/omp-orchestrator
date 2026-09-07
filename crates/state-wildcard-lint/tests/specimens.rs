#![forbid(unsafe_code)]

use state_wildcard_lint::{
    apply_allowlist, find_findings_in_source, AllowRow, Finding, FindingKind, DECLARED_ALLOWLIST,
};

#[test]
fn known_bad_state_wildcard_is_flagged() {
    let source = r#"
enum PaneState { Working, Idle }
fn check(input: PaneState) {
    let state: PaneState = input;
    match state {
        PaneState::Working => (),
        _ => (),
    }
}
"#;
    let findings = find_findings_in_source(source);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].match_line, 5);
    assert_eq!(findings[0].wildcard_line, 7);
    assert_eq!(findings[0].inferred_type.as_deref(), Some("PaneState"));
    assert_eq!(findings[0].kind, FindingKind::WildcardState);
    assert_eq!(findings[0].wildcard_text, "_ => (),");
}

#[test]
fn wildcard_on_integer_and_string_passes() {
    let source = r#"
fn integer(value: i32) {
    let number: i32 = value;
    match number { 0 => (), _ => () }
}
fn text(value: &str) {
    let text: &str = value;
    match text { "yes" => (), _ => () }
}
"#;
    assert!(find_findings_in_source(source).is_empty());
}

#[test]
fn wildcard_on_non_state_enum_passes() {
    let source = r#"
enum ErrorKind { Io, Parse }
fn check(input: ErrorKind) {
    let error: ErrorKind = input;
    match error { ErrorKind::Io => (), _ => () }
}
"#;
    assert!(find_findings_in_source(source).is_empty());
}

#[test]
fn unresolved_state_like_type_is_not_reported_clean() {
    let source = r#"
fn check(input: RemoteState) {
    let state: RemoteState = input;
    match state { _ => () }
}
"#;
    let findings = find_findings_in_source(source);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, FindingKind::UnresolvedStateType);
    assert_eq!(findings[0].inferred_type.as_deref(), Some("RemoteState"));
}

#[test]
fn mutation_removing_state_wildcard_is_green() {
    let source = r#"
enum PaneState { Working, Idle }
fn check(state: PaneState) {
    match state {
        PaneState::Working => (),
        _ => (),
    }
}
"#;
    assert_eq!(find_findings_in_source(source).len(), 1);
    let repaired = source.replace("_ => ()", "PaneState::Idle => ()");
    assert!(find_findings_in_source(&repaired).is_empty());
    assert!(source.contains("_ => ()"), "mutation must not alter the original");
}

#[test]
fn empty_or_unreadable_workspace_is_an_error() {
    let root = std::env::temp_dir().join(format!(
        "state-wildcard-lint-empty-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create empty root");
    let report = state_wildcard_lint::lint_workspace(&root);
    assert!(report.scanned.is_empty());
    assert!(report.findings.is_empty());
    assert!(report.error.is_some());
    assert!(!report.is_pass());
    std::fs::remove_dir_all(&root).expect("remove empty root");
}

/// ANTI-VACUITY. `crates/` present but empty is the dangerous shape: the walk
/// SUCCEEDS and finds nothing, so without this the report is indistinguishable
/// from a clean workspace.
#[test]
fn present_but_empty_crates_dir_is_an_error_not_a_pass() {
    let root = std::env::temp_dir().join(format!(
        "state-wildcard-lint-vacuous-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("crates")).expect("create empty crates dir");
    let report = state_wildcard_lint::lint_workspace(&root);
    assert!(report.scanned.is_empty(), "{:?}", report.scanned);
    assert!(
        report.error.is_some(),
        "an empty scan set must be an ERROR, not a pass"
    );
    assert!(
        report.error.as_deref().unwrap_or("").contains("empty scan set"),
        "{:?}",
        report.error
    );
    assert!(!report.is_pass());
    std::fs::remove_dir_all(&root).expect("remove vacuous root");
}

/// REGRESSION for real finding 1, derived 2026-09-02 from the crate's own API:
/// `crates/cargo-lane-budget/src/main.rs: match line 29, wildcard arm line 60`.
/// Line 60 is the INNER `match args[index].as_str()` arm; the outer `match mode`
/// owns line 94. A refusal that names the wrong arm looks actionable and is not.
#[test]
fn nested_wildcard_is_not_attributed_to_the_outer_match() {
    let source = r#"
enum PaneState { Working, Idle }
fn dispatch(state: PaneState) {
    match state {
        PaneState::Working => {
            match 0u8 {
                1 => (),
                _ => (),
            }
        }
        _ => (),
    }
}
"#;
    let findings = find_findings_in_source(source);
    let outer: Vec<&Finding> = findings.iter().filter(|f| f.match_line == 4).collect();
    assert_eq!(outer.len(), 1, "{findings:?}");
    assert_eq!(
        outer[0].wildcard_line, 11,
        "the outer match's own arm is line 11, not the nested arm at line 8"
    );
}

/// REGRESSION for real finding 2, derived 2026-09-02:
/// `crates/check-publish/src/json.rs: match line 355, scrutinee="verdict"`.
/// `pub fn count_key(verdict: &str)` — a `match` on `&str` cannot be exhaustive,
/// so the `_` arm is compiler-required. The type was declared in the signature;
/// only the resolver could not see it.
#[test]
fn function_parameter_str_is_a_primitive_not_a_state() {
    let source = r#"
pub fn count_key(verdict: &str) -> Option<&'static str> {
    match verdict {
        "PASS" => Some("pass"),
        "RED" => Some("red"),
        _ => None,
    }
}
"#;
    assert!(
        find_findings_in_source(source).is_empty(),
        "{:?}",
        find_findings_in_source(source)
    );
}

/// A parameter whose type IS a state enum must still be caught. Resolving
/// parameters must not become a hole in the predicate.
#[test]
fn function_parameter_state_enum_is_still_a_finding() {
    let source = r#"
enum PaneState { Working, Idle }
fn dispatch(pane: PaneState) {
    match pane {
        PaneState::Working => (),
        _ => (),
    }
}
"#;
    let findings = find_findings_in_source(source);
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].kind, FindingKind::WildcardState);
}

/// A reference to a state enum is a state enum.
#[test]
fn reference_parameter_to_state_enum_is_a_finding() {
    let source = r#"
enum PaneState { Working, Idle }
fn dispatch(pane: &PaneState) {
    match pane {
        PaneState::Working => (),
        _ => (),
    }
}
"#;
    let findings = find_findings_in_source(source);
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].kind, FindingKind::WildcardState);
}

/// The refusal text must carry file, line and the arm. This is the reporting
/// contract the pre-commit gate formats with `{finding}`.
#[test]
fn finding_display_names_file_line_and_arm() {
    let finding = Finding {
        file: "crates/x/src/main.rs".to_owned(),
        match_line: 29,
        wildcard_line: 94,
        wildcard_text: "_ => {".to_owned(),
        scrutinee: "mode".to_owned(),
        inferred_type: None,
        kind: FindingKind::UnresolvedStateType,
    };
    let rendered = finding.to_string();
    assert!(rendered.starts_with("crates/x/src/main.rs:94:"), "{rendered}");
    assert!(rendered.contains("`_ => {`"), "{rendered}");
    assert!(rendered.contains("match mode"), "{rendered}");
    assert!(rendered.contains("line 29"), "{rendered}");
}

/// The suppression set is EMPTY BY DESIGN. If a row is ever added it must say
/// WHY and must name a real repo path — an exclusion without a reason is an
/// inferred exclusion wearing a table row.
#[test]
fn declared_allowlist_rows_carry_a_reason_and_a_real_path() {
    for row in DECLARED_ALLOWLIST {
        assert!(row.reason.len() > 40, "row {row:?} needs a real reason");
        assert!(!row.scrutinee.is_empty(), "row {row:?} needs a scrutinee");
        let path = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
            .join(row.file);
        assert!(path.exists(), "allowlisted path does not exist: {}", row.file);
    }
}

/// The scan boundary is DECLARED, not inferred: every pruned directory states
/// why it is not production source, and the gate prints them.
#[test]
fn declared_skip_dirs_each_carry_a_reason_and_are_stated() {
    let names: Vec<&str> = state_wildcard_lint::DECLARED_SKIP_DIRS
        .iter()
        .map(|row| row.name)
        .collect();
    for expected in [".git", "target", "tests", "fixtures"] {
        assert!(names.contains(&expected), "{names:?} must prune {expected}");
    }
    for row in state_wildcard_lint::DECLARED_SKIP_DIRS {
        assert!(row.reason.len() > 20, "skip row {row:?} needs a real reason");
    }
    for mode in [
        state_wildcard_lint::ScanMode::RepoWide,
        state_wildcard_lint::ScanMode::StagedPaths,
    ] {
        let stated = state_wildcard_lint::declared_scope_line(mode);
        for name in &names {
            assert!(stated.contains(name), "scope line must name {name}: {stated}");
        }
        assert!(
            stated.contains("masked"),
            "the scope line must state the specimen-masking mechanism: {stated}"
        );
    }
    // Each mode must declare a DIFFERENT scope, or the declaration is decoration.
    assert_ne!(
        state_wildcard_lint::declared_scope_line(state_wildcard_lint::ScanMode::RepoWide),
        state_wildcard_lint::declared_scope_line(state_wildcard_lint::ScanMode::StagedPaths)
    );
    assert!(
        state_wildcard_lint::declared_scope_line(state_wildcard_lint::ScanMode::StagedPaths)
            .contains("UNTRACKED"),
        "a scoped green must state that it does NOT cover the workspace"
    );
}

/// A suppression must subtract exactly one thing and be reported, not vanish.
#[test]
fn allowlist_suppresses_only_its_own_row() {
    let make = |file: &str, scrutinee: &str| Finding {
        file: file.to_owned(),
        match_line: 1,
        wildcard_line: 2,
        wildcard_text: "_ => ()".to_owned(),
        scrutinee: scrutinee.to_owned(),
        inferred_type: None,
        kind: FindingKind::UnresolvedStateType,
    };
    let rows = &[AllowRow {
        file: "a.rs",
        scrutinee: "mode",
        reason: "declared",
    }];
    let (kept, allowed, stale) =
        apply_allowlist(vec![make("a.rs", "mode"), make("b.rs", "mode")], rows);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].file, "b.rs");
    assert_eq!(allowed.len(), 1);
    assert_eq!(allowed[0].reason, "declared");
    assert!(stale.is_empty());
}

/// FIRES-ON-KNOWN-BAD for the allowlist itself: a row that suppresses nothing
/// is a silent carve-out, so it must surface as stale.
#[test]
fn stale_allowlist_row_is_reported() {
    let rows = &[AllowRow {
        file: "gone.rs",
        scrutinee: "mode",
        reason: "declared",
    }];
    let (kept, allowed, stale) = apply_allowlist(Vec::new(), rows);
    assert!(kept.is_empty());
    assert!(allowed.is_empty());
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].file, "gone.rs");
}

/// SELF-REFERENCE, handled deliberately and WITHOUT an exclusion.
///
/// This crate embeds the forbidden pattern as specimen data. It is not
/// allowlisted and the crate is not skipped; two structural mechanisms make
/// the self-scan honest instead:
///
///   1. `mask_line` blanks string-literal contents before scanning, so a
///      pattern quoted as test data is not code.
///   2. `visit_rs` prunes `tests/` and `fixtures/`, so this file is not
///      scanned at all.
///
/// This test is the standing proof of (1): the crate's own `src/lib.rs` — which
/// IS scanned — carries the forbidden pattern in its inline test module and
/// still yields zero findings.
#[test]
fn self_scan_of_lib_is_clean_without_an_exclusion() {
    let source = include_str!("../src/lib.rs");
    assert!(
        source.contains("_ => ()"),
        "specimen data must actually be present for this to prove anything"
    );
    let findings = find_findings_in_source(source);
    assert!(
        findings.is_empty(),
        "self-scan must be clean by masking, not by carve-out: {findings:?}"
    );
    for row in DECLARED_ALLOWLIST {
        assert!(
            !row.file.contains("state-wildcard-lint"),
            "the self-reference must not be handled by an allowlist row: {row:?}"
        );
    }
}

/// THE SCOPING LEG (omp-orchestrator-oej2, second gate). A wildcard arm in a file
/// that is not part of the change must not refuse the change; the sweep must still
/// find it. Measured 2026-09-02: this lint ran repo-wide from a hook keyed on the
/// staged set and refused a commit staging only `AGENTS.md` because of an arm in an
/// UNTRACKED file — the same fleet-blocking shape as `path-literal-guard`.
#[test]
fn an_unstaged_wildcard_cannot_refuse_a_clean_staged_change() {
    let root = std::env::temp_dir().join(format!("swl-scoped-{}", std::process::id()));
    let src = root.join("crates/example/src");
    std::fs::create_dir_all(&src).expect("create fixture tree");
    std::fs::write(src.join("clean.rs"), "fn main() {}\n").expect("write clean file");
    std::fs::write(
        src.join("scratch.rs"),
        "enum PaneState { Working, Idle }\nfn f(state: PaneState) {\n    match state {\n        PaneState::Working => (),\n        _ => (),\n    }\n}\n",
    )
    .expect("write unstaged dirty file");

    // RED direction: the sweep finds it and NAMES file:line.
    let sweep = state_wildcard_lint::lint_workspace(&root);
    assert_eq!(sweep.mode, state_wildcard_lint::ScanMode::RepoWide);
    assert_eq!(sweep.verdict(), state_wildcard_lint::Verdict::Violation, "{sweep:?}");
    let named: Vec<String> = sweep
        .findings
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    assert!(
        named.iter().any(|f| f.contains("scratch.rs:5")),
        "the sweep must NAME file:line: {named:?}"
    );

    // GREEN direction: scoped to the staged file, the same arm is not this commit's
    // problem. This is the direction that unblocks a shared checkout.
    let scoped = state_wildcard_lint::lint_paths(&root, &["crates/example/src/clean.rs"]);
    assert_eq!(scoped.mode, state_wildcard_lint::ScanMode::StagedPaths);
    assert_eq!(
        scoped.verdict(),
        state_wildcard_lint::Verdict::Clean,
        "an unstaged wildcard must not refuse an unrelated change: {scoped:?}"
    );

    // Scoped to the dirty file: RED, naming it. Scoping narrows the file set; it does
    // not weaken the predicate.
    let caught = state_wildcard_lint::lint_paths(&root, &["crates/example/src/scratch.rs"]);
    assert_eq!(caught.verdict(), state_wildcard_lint::Verdict::Violation);
    assert_eq!(caught.findings.len(), 1, "{:?}", caught.findings);
    assert_eq!(caught.findings[0].wildcard_line, 5);

    std::fs::remove_dir_all(&root).expect("remove fixture tree");
}

/// ISOMORPHISM. Scoping must narrow WHICH files are read and nothing else: staged mode
/// handed every file the sweep read must reach the same verdict, on the REAL repo.
#[test]
fn staged_mode_over_the_real_repo_equals_the_sweep() {
    let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .canonicalize()
        .expect("repo root");
    let sweep = state_wildcard_lint::lint_workspace(&root);
    assert!(
        !sweep.scanned.is_empty(),
        "anti-vacuity: the real repo must have files to compare"
    );
    let scoped = state_wildcard_lint::lint_paths(&root, &sweep.scanned);
    assert_eq!(scoped.scanned, sweep.scanned, "the same files must be read");
    assert_eq!(scoped.findings, sweep.findings, "the same findings, same file:line");
    assert_eq!(scoped.verdict(), sweep.verdict());
}

/// THREE OUTCOMES. Staging only a doc leaves this lint nothing to check, which is NOT
/// clean (omp-orchestrator-calr).
#[test]
fn staged_set_with_no_eligible_file_is_nothing_to_check_not_clean() {
    let root = std::env::temp_dir().join(format!("swl-ntc-{}", std::process::id()));
    std::fs::create_dir_all(root.join("crates/example/src")).expect("create fixture tree");
    let report = state_wildcard_lint::lint_paths(
        &root,
        &["AGENTS.md", "crates/example/tests/it.rs", "docs/PLAN.md"],
    );
    assert!(report.scanned.is_empty(), "{:?}", report.scanned);
    assert_eq!(report.verdict(), state_wildcard_lint::Verdict::NothingToCheck);
    assert!(!report.is_pass(), "nothing-to-check must not read as a pass");
    std::fs::remove_dir_all(&root).expect("remove fixture tree");
}

/// The shared eligibility predicate: both modes agree on the floor, including the
/// pruned directories the DECLARED table names.
#[test]
fn eligibility_predicate_matches_the_declared_floor() {
    use state_wildcard_lint::is_in_scan_scope;
    for inside in ["crates/x/src/lib.rs", "crates/x/src/bin/y.rs", "crates/x/benches/b.rs"] {
        assert!(is_in_scan_scope(std::path::Path::new(inside)), "{inside} must be in scope");
    }
    for outside in [
        "crates/x/tests/it.rs",
        "crates/x/src/fixtures/f.rs",
        "crates/x/target/debug/g.rs",
        "crates/x/Cargo.toml",
        "src/main.rs",
        "AGENTS.md",
    ] {
        assert!(
            !is_in_scan_scope(std::path::Path::new(outside)),
            "{outside} must be out of scope"
        );
    }
}

//! PRE-COMMIT EMPTY-INDEX GATE — the hook must distinguish a real clean scan
//! from an empty index that checked nothing.
//!
//! These tests exercise the actual pre-commit binary against throwaway git
//! indexes. The fixture keeps one harmless Rust source under `crates/` so the
//! other workspace-wide gates have a non-empty, clean scan set; only the file
//! named by each test is staged.

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

static FIXTURE_SEQ: AtomicU32 = AtomicU32::new(0);

fn fresh_git_tree(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "no-shell-gate-pre-commit-{}-{test}-{}",
        std::process::id(),
        FIXTURE_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(dir.join("crates/example/src")).expect("create fixture tree");
    fs::create_dir_all(dir.join("docs/plan")).expect("create fixture plan");
    fs::write(
        dir.join("crates/example/src/lib.rs"),
        "pub fn clean_fixture() {}\n",
    )
    .expect("write clean workspace source");
    fs::write(
        dir.join("docs/plan/HYPOTHESES.jsonl"),
        r#"{"id":"h1","prediction":"fixture","falsifier":"missing section","evidence_scope":"docs/plan","recorded_commit":"0123456789abcdef0123456789abcdef01234567","observed_result":null}"#.to_owned() + "\n",
    )
    .expect("write fixture hypotheses");
    run_git(&dir, &["init", "-q"], "git init");
    run_git(
        &dir,
        &["add", "--", "crates/example/src/lib.rs", "docs/plan/HYPOTHESES.jsonl"],
        "stage clean baseline",
    );
    run_git(
        &dir,
        &["-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture baseline [test]"],
        "fixture baseline commit",
    );
    let parent = String::from_utf8_lossy(&run_git(&dir, &["rev-parse", "HEAD"], "read fixture parent").stdout)
        .trim()
        .to_owned();
    let hypotheses_path = dir.join("docs/plan/HYPOTHESES.jsonl");
    let hypotheses = fs::read_to_string(&hypotheses_path)
        .expect("read fixture hypotheses")
        .replace("0123456789abcdef0123456789abcdef01234567", &parent);
    fs::write(&hypotheses_path, hypotheses).expect("update fixture parent");
    run_git(&dir, &["add", "--", "docs/plan/HYPOTHESES.jsonl"], "stage updated hypotheses");
    run_git(
        &dir,
        &["-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture hypotheses [test]"],
        "fixture hypotheses commit",
    );
    dir
}

fn run_git(dir: &Path, args: &[&str], what: &str) -> Output {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("spawn git");
    assert!(
        output.status.success(),
        "{what} failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn stage(dir: &Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).expect("write staged fixture");
    run_git(dir, &["add", "--", name], "git add");
}

fn run_gate(dir: &Path) -> Output {
    let scoped_index = dir.join(".git/calr-gate-index");
    fs::copy(dir.join(".git/index"), &scoped_index).expect("copy scoped index");
    let output = Command::new(env!("CARGO_BIN_EXE_pre-commit-gate"))
        .current_dir(dir)
        .env("GIT_INDEX_FILE", &scoped_index)
        .output()
        .expect("spawn pre-commit-gate");
    fs::remove_file(scoped_index).expect("remove scoped index");
    output
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn top_level_outcome(stderr: &str) -> &'static str {
    let outcomes = ["CLEAN:", "VIOLATION:", "NOTHING_TO_CHECK:", "ANCESTRY_ONLY_MERGE:"];
    let matches: Vec<_> = stderr
        .lines()
        .filter_map(|line| outcomes.iter().find(|outcome| line.starts_with(*outcome)))
        .copied()
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "gate output must contain exactly one top-level outcome marker: {stderr}"
    );
    matches[0]
}
fn assert_no_ambiguous_nested_outcomes(stderr: &str) {
    for gate in ["path-literal-guard", "state-wildcard-lint"] {
        assert!(
            !stderr.lines().any(|line| {
                line.starts_with(&format!("{gate}: NOTHING_TO_CHECK"))
            }),
            "per-gate scope diagnostics must not reuse the top-level NOTHING_TO_CHECK marker: {stderr}"
        );
    }
}

/// KNOWN-BAD: an empty index is not a successful no-op for the gate. It is a
/// typed NOTHING_TO_CHECK refusal with its own exit code, while `git commit`
/// remains responsible for its ordinary "nothing to commit" behavior.
#[test]
fn empty_staged_index_is_nothing_to_check_refusal() {
    let dir = fresh_git_tree("empty-index");
    let output = run_gate(&dir);
    let error = stderr(&output);

    assert_eq!(
        output.status.code(),
        Some(3),
        "empty staged index must have the distinct NOTHING_TO_CHECK outcome: {error}"
    );
    assert_eq!(
        top_level_outcome(&error),
        "NOTHING_TO_CHECK:",
        "empty scan must report the explicit top-level outcome: {error}"
    );
    assert_no_ambiguous_nested_outcomes(&error);
}

/// KNOWN-GOOD: one clean staged file is checked and reports CLEAN.
#[test]
fn one_clean_staged_file_is_clean() {
    let dir = fresh_git_tree("clean-file");
    stage(&dir, "README.md", "clean staged fixture\n");
    let output = run_gate(&dir);
    let error = stderr(&output);

    assert_eq!(
        output.status.code(),
        Some(0),
        "one clean staged file must pass: {error}"
    );
    assert_eq!(
        top_level_outcome(&error),
        "CLEAN:",
        "clean verdict must be the explicit top-level outcome: {error}"
    );
    assert_no_ambiguous_nested_outcomes(&error);
}

/// KNOWN-BAD: one staged forbidden-extension file reports VIOLATION, not a
/// generic empty-index refusal or a successful partial scan.
#[test]
fn one_violating_staged_file_is_violation() {
    let dir = fresh_git_tree("violating-file");
    stage(&dir, "evil.sh", "#!/bin/sh\necho known-bad\n");
    let output = run_gate(&dir);
    let error = stderr(&output);

    assert_eq!(
        output.status.code(),
        Some(1),
        "one violating staged file must be a violation refusal: {error}"
    );
    assert_eq!(
        top_level_outcome(&error),
        "VIOLATION:",
        "violation verdict must be the explicit top-level outcome: {error}"
    );
    assert!(
        error.contains("evil.sh"),
        "violation must name the staged file: {error}"
    );
    assert_no_ambiguous_nested_outcomes(&error);
}
#[test]
fn collapsing_whole_commit_marker_into_gate_note_is_red() {
    let dir = fresh_git_tree("scope-boundary-mutation");
    stage(&dir, "README.md", "clean staged fixture\n");
    let actual = stderr(&run_gate(&dir));
    assert_eq!(top_level_outcome(&actual), "CLEAN:");

    let collapsed = actual
        .lines()
        .map(|line| {
            line.strip_prefix("CLEAN:").map_or_else(
                || line.to_owned(),
                |rest| format!("path-literal-guard: CLEAN:{rest}"),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let red = std::panic::catch_unwind(|| top_level_outcome(&collapsed));
    assert!(
        red.is_err(),
        "a whole-commit marker collapsed into a per-gate note must be rejected: {collapsed}"
    );
}
/// The same run proves the two empty-index states are not collapsed: no merge is
/// NOTHING_TO_CHECK, while a real ancestry-only merge is an explicit successful decision.
#[test]
fn ancestry_only_merge_runs_gate_and_preserves_empty_index_refusal() {
    let dir = fresh_git_tree("ancestry-only-merge");
    run_git(&dir, &["branch", "-M", "main"], "rename fixture branch");

    let empty = run_gate(&dir);
    let empty_error = stderr(&empty);
    assert_eq!(empty.status.code(), Some(3), "a clean index without merge state must refuse: {empty_error}");
    assert!(
        empty_error.contains("NOTHING_TO_CHECK: no staged files to check"),
        "empty non-merge gate stderr: {empty_error}"
    );

    let marker = dir.join("docs/plan/merge-marker.txt");
    fs::write(&marker, "same patch\n").expect("write main patch");
    run_git(&dir, &["add", "--", "docs/plan/merge-marker.txt"], "stage main patch");
    run_git(&dir, &["-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "main equivalent patch [test]"], "commit main patch");
    let parent = String::from_utf8_lossy(&run_git(&dir, &["rev-parse", "HEAD^"], "read merge base").stdout).trim().to_owned();

    run_git(&dir, &["switch", "-c", "ci/fence-16l-13ca3f5", &parent], "create duplicate patch branch");
    fs::write(&marker, "same patch\n").expect("write duplicate patch");
    run_git(&dir, &["add", "--", "docs/plan/merge-marker.txt"], "stage duplicate patch");
    run_git(&dir, &["-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "duplicate patch branch [test]"], "commit duplicate patch");
    run_git(&dir, &["switch", "main"], "return to main");

    let cherry = run_git(&dir, &["cherry", "-v", "main", "ci/fence-16l-13ca3f5"], "verify patch-id duplicate");
    assert!(String::from_utf8_lossy(&cherry.stdout).lines().any(|line| line.trim_start().starts_with('-')), "branch commit must be already represented by patch-id: {:?}", String::from_utf8_lossy(&cherry.stdout));
    run_git(&dir, &["merge", "--no-commit", "--no-ff", "ci/fence-16l-13ca3f5"], "create ancestry-only merge");

    assert!(dir.join(".git/MERGE_HEAD").is_file(), "the live merge state must exist");
    for filter in ["ACMR", "D"] {
        let diff = run_git(&dir, &["diff", "--cached", "--name-only", &format!("--diff-filter={filter}")], "verify empty staged merge set");
        assert!(diff.stdout.is_empty(), "merge index must be empty for {filter}: {:?}", String::from_utf8_lossy(&diff.stdout));
    }
    let merge_tree = String::from_utf8_lossy(&run_git(&dir, &["write-tree"], "read merge index tree").stdout).trim().to_owned();
    let head_tree = String::from_utf8_lossy(&run_git(&dir, &["rev-parse", "HEAD^{tree}"], "read HEAD tree").stdout).trim().to_owned();
    assert_eq!(merge_tree, head_tree, "the merge must be ancestry-only");

    let output = run_gate(&dir);
    let error = stderr(&output);
    assert_eq!(output.status.code(), Some(0), "ancestry-only merge must reach the gate: {error}");
    assert_eq!(top_level_outcome(&error), "ANCESTRY_ONLY_MERGE:", "the merge decision must be explicit: {error}");
    assert!(!error.contains("NOTHING_TO_CHECK"), "merge state must not collapse into empty-index refusal: {error}");

    run_git(&dir, &["merge", "--abort"], "abort fixture merge");
    fs::remove_dir_all(dir).expect("remove merge fixture");
}

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
    fs::write(
        dir.join("crates/example/src/lib.rs"),
        "pub fn clean_fixture() {}\n",
    )
    .expect("write clean workspace source");
    run_git(&dir, &["init", "-q"], "git init");
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
    Command::new(env!("CARGO_BIN_EXE_pre-commit-gate"))
        .current_dir(dir)
        .output()
        .expect("spawn pre-commit-gate")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn top_level_outcome(stderr: &str) -> &'static str {
    let outcomes = ["CLEAN:", "VIOLATION:", "NOTHING_TO_CHECK:"];
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

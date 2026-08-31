//! The gate's own verification battery (bead omp-orchestrator-4ak).
//!
//! Legs, in acceptance-criteria order:
//! 1. BOTH directions: `planted_shell_is_red_then_green_after_delete` proves
//!    RED on a dirty index and GREEN on a clean one, in the same run;
//!    `this_repo_is_clean` is the standing clean leg, and the real-tree probe
//!    (documented in the bead) turns it RED on a staged `.sh`.
//! 2. PLANTED KNOWN-BAD: fixture trees below, planted and deleted in one run.
//! 3. MUTATION: the `.sh` legs below key on `FORBIDDEN_EXTENSIONS` containing
//!    "sh" — delete that pattern and `sh_is_flagged`,
//!    `uppercase_extensions_are_flagged`, `bare_dotname_scripts_are_flagged`,
//!    `planted_shell_is_red_then_green_after_delete`, and
//!    `binary_exits_1_on_planted_shell` go RED. A green mutation run would
//!    mean the legs are not attributable to the pattern and prove nothing.
//! 5. ANTI-VACUITY: `empty_scan_set_is_an_error_not_a_pass` (unit),
//!    `empty_index_is_an_error_not_a_pass` (end-to-end),
//!    `binary_exits_2_on_empty_index` (CLI exit code).
//! 6. NO-CLAIM: documented in `src/lib.rs` — extensions of tracked files only.

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use no_shell_gate::{check_repo, scan, GateError, Verdict, Violation};

// ---------------------------------------------------------------- helpers

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonicalize this repo's root")
}

static FIXTURE_SEQ: AtomicU32 = AtomicU32::new(0);

/// A fresh throwaway git repository: the fixture tree every planted leg uses.
/// Never committed to this repo — created and torn down at test runtime.
fn fresh_git_tree(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "no-shell-gate-{}-{test}-{}",
        std::process::id(),
        FIXTURE_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(&dir).expect("create fixture dir");
    run_git(&dir, &["init", "-q"], "git init");
    dir
}

fn run_git(dir: &Path, args: &[&str], what: &str) -> std::process::Output {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("spawn git");
    assert!(
        out.status.success(),
        "{what} failed: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    out
}

/// Write a file and stage it, so `git ls-files` (the index) reports it.
fn stage(dir: &Path, name: &str, content: &str) {
    let file = dir.join(name);
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).expect("create fixture parent dir");
    }
    fs::write(&file, content).expect("write fixture file");
    run_git(dir, &["add", "--", name], "git add");
}

/// Unstage (remove from the index) and delete from disk: the GREEN half of a
/// planted leg.
fn unstage_and_delete(dir: &Path, name: &str) {
    run_git(
        dir,
        &["rm", "--cached", "-q", "--", name],
        "git rm --cached",
    );
    let file = dir.join(name);
    if file.exists() {
        fs::remove_file(&file).expect("delete fixture file");
    }
}

/// Run the gate binary. `None` = let it default to THIS repo's root;
/// `Some(dir)` = check a fixture tree.
fn run_gate(dir: Option<&Path>) -> (Option<i32>, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_no-shell-gate"));
    if let Some(dir) = dir {
        cmd.arg(dir);
    }
    let out = cmd.output().expect("spawn gate binary");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

// ------------------------------------------------- unit legs on the matcher

/// Known-good leg. An attack-only suite ships an over-strict gate, and an
/// over-strict gate gets routed around — so the clean paths must be pinned:
/// Rust sources, manifests, markdown, `notes.sh.txt` (FINAL extension only),
/// and a dotfile whose stem is not an extension.
#[test]
fn clean_list_passes() {
    let clean = [
        "src/main.rs",
        "Cargo.toml",
        "README.md",
        "notes.sh.txt",
        ".gitignore",
        "docs/guide.md",
    ]
    .map(String::from);
    assert_eq!(scan(&clean).expect("clean scan"), vec![]);
}

/// Mutation-attributable `.sh` leg: keys on `"sh"` being in
/// `FORBIDDEN_EXTENSIONS`. Delete the pattern and this goes RED.
#[test]
fn sh_is_flagged() {
    let paths = ["scripts/deploy.sh"].map(String::from);
    assert_eq!(
        scan(&paths).expect("scan"),
        vec![Violation {
            path: "scripts/deploy.sh".into(),
            extension: "sh".into(),
        }]
    );
}

/// Mutation-attributable `.py` leg, independent of the `.sh` leg.
#[test]
fn py_is_flagged() {
    let paths = ["tools/hello.py"].map(String::from);
    assert_eq!(
        scan(&paths).expect("scan"),
        vec![Violation {
            path: "tools/hello.py".into(),
            extension: "py".into(),
        }]
    );
}

/// `SCRIPT.SH` is still a shell script; ASCII case-folding closes the trivial
/// bypass. The `.SH` assertion fails when the `sh` pattern is deleted.
#[test]
fn uppercase_extensions_are_flagged() {
    let paths = ["SCRIPT.SH", "App.PY"].map(String::from);
    let violations = scan(&paths).expect("scan");
    assert!(violations.contains(&Violation {
        path: "SCRIPT.SH".into(),
        extension: "sh".into(),
    }));
    assert!(violations.contains(&Violation {
        path: "App.PY".into(),
        extension: "py".into(),
    }));
}

/// A file whose entire name is `.sh` is treated as extension `sh`.
#[test]
fn bare_dotname_scripts_are_flagged() {
    let paths = [".sh"].map(String::from);
    assert_eq!(
        scan(&paths).expect("scan"),
        vec![Violation {
            path: ".sh".into(),
            extension: "sh".into(),
        }]
    );
}

// ------------------------------- anti-vacuity and fail-closed (unit level)

/// ANTI-VACUITY at the choke point: an empty scan set is an ERROR, never a
/// pass. A gate that scanned nothing reports identically to one that passed.
#[test]
fn empty_scan_set_is_an_error_not_a_pass() {
    assert!(matches!(
        scan(&[]).expect_err("empty scan set must error"),
        GateError::EmptyScanSet
    ));
}

// ----------------------- end-to-end legs through a real git index fixture

/// PLANTED KNOWN-BAD, full cycle, both directions asserted in the SAME run:
/// a real git tree with a staged `run.sh` goes RED naming the file; deleting
/// it (from the index and disk) goes GREEN. The `README.md` baseline keeps
/// the scan set non-empty for the GREEN half, so the clean verdict is a real
/// verdict, not vacuity.
#[test]
fn planted_shell_is_red_then_green_after_delete() {
    let dir = fresh_git_tree("shell-cycle");
    stage(
        &dir,
        "README.md",
        "clean baseline so the scan set is never empty\n",
    );
    stage(&dir, "run.sh", "#!/bin/sh\necho planted known-bad\n");
    match check_repo(&dir).expect("gate must render a verdict on a live index") {
        Verdict::Violations(violations) => assert!(
            violations.contains(&Violation {
                path: "run.sh".into(),
                extension: "sh".into(),
            }),
            "RED leg must name run.sh, got {violations:?}"
        ),
        other => panic!("RED leg failed: expected violations, got {other:?}"),
    }
    unstage_and_delete(&dir, "run.sh");
    assert_eq!(
        check_repo(&dir).expect("gate must render a verdict after the delete"),
        Verdict::Clean,
        "GREEN leg failed: deleting run.sh must leave a clean verdict"
    );
}

/// The same full cycle for `.py`, so neither forbidden extension is exercised
/// only at the unit level.
#[test]
fn planted_python_is_red_then_green_after_delete() {
    let dir = fresh_git_tree("python-cycle");
    stage(
        &dir,
        "README.md",
        "clean baseline so the scan set is never empty\n",
    );
    stage(&dir, "tool.py", "print('planted known-bad')\n");
    match check_repo(&dir).expect("gate must render a verdict on a live index") {
        Verdict::Violations(violations) => assert!(
            violations.contains(&Violation {
                path: "tool.py".into(),
                extension: "py".into(),
            }),
            "RED leg must name tool.py, got {violations:?}"
        ),
        other => panic!("RED leg failed: expected violations, got {other:?}"),
    }
    unstage_and_delete(&dir, "tool.py");
    assert_eq!(
        check_repo(&dir).expect("gate must render a verdict after the delete"),
        Verdict::Clean,
        "GREEN leg failed: deleting tool.py must leave a clean verdict"
    );
}

/// ANTI-VACUITY end to end: a repository whose index is EMPTY is an error,
/// never a pass.
#[test]
fn empty_index_is_an_error_not_a_pass() {
    let dir = fresh_git_tree("empty-index");
    assert!(matches!(
        check_repo(&dir).expect_err("empty index must error"),
        GateError::EmptyScanSet
    ));
}

/// Fail closed: a directory with no git metadata cannot render a verdict.
#[test]
fn missing_git_metadata_fails_closed() {
    let dir = std::env::temp_dir().join(format!("no-shell-gate-{}-not-a-repo", std::process::id()));
    fs::create_dir_all(&dir).expect("create non-repo dir");
    assert!(matches!(
        check_repo(&dir).expect_err("a non-repo must error"),
        GateError::GitFailed(_)
    ));
}

// --------------- the standing clean leg: THIS repository, via cargo test

/// The `cargo test` wiring. Any `cargo test` that includes this crate re-runs
/// the gate against the real index, so a tracked `.sh`/`.py` fails the suite
/// even when CI is skipped entirely.
#[test]
fn this_repo_is_clean() {
    assert_eq!(
        check_repo(&repo_root()).expect("gate must render a verdict on this repo"),
        Verdict::Clean,
        "a tracked .sh or .py is in the index — port it to Rust; the \
         exemption list is empty by design"
    );
}

// ------------------ the binary surface the CI workflow invokes directly

/// CI runs the binary with no argument: it must default to this repo and
/// exit 0 while the tree is clean.
#[test]
fn binary_is_green_on_this_repo() {
    let (code, stdout, stderr) = run_gate(None);
    assert_eq!(
        code,
        Some(0),
        "gate binary must exit 0 on a clean tree: {stderr}"
    );
    assert!(stdout.contains("ok:"), "clean run must say so: {stdout}");
}

/// CI-invocable RED: exit code 1 (not 2), with the offending path named.
#[test]
fn binary_exits_1_on_planted_shell() {
    let dir = fresh_git_tree("binary-shell");
    stage(&dir, "README.md", "clean baseline\n");
    stage(&dir, "evil.sh", "#!/bin/sh\necho planted known-bad\n");
    let (code, _stdout, stderr) = run_gate(Some(&dir));
    assert_eq!(
        code,
        Some(1),
        "planted .sh must exit 1 (violations), not 0 or 2: {stderr}"
    );
    assert!(
        stderr.contains("evil.sh"),
        "violation output must name the file: {stderr}"
    );
}

/// ANTI-VACUITY at the CLI: an empty index is exit 2 (gate error), never 0.
#[test]
fn binary_exits_2_on_empty_index() {
    let dir = fresh_git_tree("binary-empty");
    let (code, _stdout, stderr) = run_gate(Some(&dir));
    assert_eq!(
        code,
        Some(2),
        "empty index must exit 2 (error), never 0: {stderr}"
    );
}

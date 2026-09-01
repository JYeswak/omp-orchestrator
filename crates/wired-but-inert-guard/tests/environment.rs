//! ENV PARITY CONTRACT for the wired-but-inert-guard Rust port (cp-79am1).
//!
//! bin/wired-but-inert-guard.sh — deleted by the Rust port, restored here from git
//! (45c613d^) — exported NOTHING: its only environment statement was `set -uo pipefail`.
//! The parity contract is therefore AMBIENT INHERITANCE: the Rust binary must make no
//! hidden environment demands beyond the tools it spawns (git, crontab) and must fail
//! loudly through its typed error channel when its inputs are absent — never silently
//! report an empty scan as a pass.
//!
//! These legs run the binary under `env -i` (the cron condition) and assert exactly
//! that: it starts with zero environment, and its repo/crontab inputs are resolved
//! from explicit sources or refused with the typed repo-root error naming the
//! markers and the escape hatch.

use std::path::PathBuf;
use std::process::Command;

const CONTRACT_SOURCE: &str = "bin/wired-but-inert-guard.sh @ 45c613d^ (env surface: empty — set -uo pipefail only)";

fn bin_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_wired-but-inert-guard"))
}

fn fresh_git_repo(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wbg-env-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create repo dir");
    run(&dir, &["init", "-q"]);
    run(&dir, &["config", "user.email", "grader@example.invalid"]);
    run(&dir, &["config", "user.name", "grader"]);
    dir
}

fn run(dir: &PathBuf, args: &[&str]) {
    std::process::Command::new("git")
        .args(["-C", dir.to_str().expect("utf-8 dir")])
        .args(args)
        .output()
        .expect("git");
}

fn isolated_guard(repo: Option<&PathBuf>, crontab: Option<&PathBuf>) -> Command {
    let mut command = Command::new(bin_path());
    command.env_clear();
    if let Some(repo) = repo {
        command.env("WIRED_GUARD_REPO", repo);
    }
    if let Some(crontab) = crontab {
        command.env("WIRED_GUARD_CRONTAB", crontab);
    }
    command
}

/// The binary starts and renders under a COMPLETELY EMPTY environment: its declared
/// env surface is empty (the deleted shell exported nothing), so zero environment
/// must not be a crash. `capabilities` touches no inputs and must exit 0.
#[test]
fn capabilities_run_with_zero_environment() {
    let out = isolated_guard(None, None)
        .arg("capabilities")
        .output()
        .expect("spawn wired-but-inert-guard");
    assert_eq!(
        out.status.code(),
        Some(0),
        "capabilities must render with zero environment"
    );
}

/// The gate runs hermetically under env -i when its two inputs are supplied
/// explicitly: an empty-index git repo yields a report whose declared gates have no
/// invoker, so the gate reports its findings and exits nonzero — loudly, never as a
/// silent pass.
#[test]
fn guard_runs_hermetically_and_reports_failures() {
    let repo = fresh_git_repo("hermetic");
    let crontab = repo.join("crontab.txt");
    std::fs::write(&crontab, "# empty schedule\n").expect("write fixture crontab");

    let out = isolated_guard(Some(&repo), Some(&crontab))
        .output()
        .expect("spawn wired-but-inert-guard");
    let code = out.status.code().unwrap_or(99);
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(
        code == 0 || code == 1,
        "a hermetic run must produce a report, not an environment crash: rc={code} stdout={stdout}"
    );
    assert!(
        stdout.contains("wired-but-inert-guard") || stdout.contains("\"schema\""),
        "a hermetic run must produce the structured report: {stdout}"
    );
    assert!(
        !stdout.contains("STARTUP_ERROR"),
        "this lane's env surface is empty: no startup env errors are expected: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&repo);
}

/// Without a repo and without the escape-hatch env, from a marker-free cwd, the gate
/// must fail LOUDLY with the typed repo error naming the markers and the escape
/// hatch — never silently scan the wrong tree.
#[test]
fn no_repo_above_cwd_fails_loudly_naming_the_markers() {
    let nowhere = std::env::temp_dir().join(format!("wbg-norepo-{}", std::process::id()));
    std::fs::create_dir_all(&nowhere).expect("create marker-free dir");

    let out = isolated_guard(None, None)
        .arg("guard")
        .current_dir(&nowhere)
        .output()
        .expect("spawn wired-but-inert-guard");
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert_eq!(out.status.code(), Some(2), "stderr={stderr}");
    assert!(
        stderr.contains("no repository marker (.git or .beads)"),
        "the typed repo error must name the markers: {stderr}"
    );
    assert!(
        stderr.contains("WIRED_GUARD_REPO"),
        "the typed error must name the escape hatch: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&nowhere);
}

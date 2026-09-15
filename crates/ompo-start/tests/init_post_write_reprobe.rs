#![forbid(unsafe_code)]

//! L2-BUILD-REPROBE (2zrz): the post-write path re-runs the same predicates
//! the pre-write path accepted -- it does not merely read back JSON.
//!
//! The gap this closes: after `write_atomic`, the production path ran
//! `read_inception` (a JSON parse + shape check against the in-memory
//! manifest), emitted an `INIT_REPROBE_OK` journal row, and re-read the
//! journal. None of those re-observes the repository: a `source_revision`
//! or control-file change landing between pre-write acceptance and the
//! post-write path was invisible, yet success was reported. The mechanism
//! under test is `ompo_start::inception::verify_post_write_predicates`,
//! called in `initialize_inner` and `write_inception_inner` after the
//! readback and before any success evidence is emitted, so a halt leaves
//! zero new `INIT_REPROBE_OK` rows.
//!
//! On tools, stated plainly: the required-tools predicate is a declaration,
//! not a measurement -- no availability probe runs before the write anywhere
//! in the init path -- and `read_inception` already re-validates its
//! membership against the post-write bytes on every path. The fresh
//! observation this file proves is identity (git) and control files (fs).
//!
//! KNOWN-BAD (single-edit mutation): delete the
//! `verify_post_write_predicates(...)` call in `initialize_inner`. The named
//! black-box leg below must RED (it gets `Ok` where it demands a typed
//! halt) while the helper-direct legs stay green -- that split proves the
//! call site is the load-bearing wiring, not the helper alone. Gutting the
//! helper instead must RED the direct legs. Pre-write controls (the 0pc9
//! family, refusal legs) stay green under both.

use lifecycle_event::default_repo_journal;
use ompo_start::inception::{
    initialize, verify_post_write_predicates, PROJECT_AGENTS_OWNERSHIP_STAMP,
};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

const RECORDED_REVISION: &str = "reprobe-recorded-r1";
const OBSERVED_REVISION: &str = "reprobe-observed-r2";

/// Absolute path of the real `git`, resolved before any mock shadows PATH.
static GIT_REAL: std::sync::LazyLock<PathBuf> = std::sync::LazyLock::new(|| {
    for directory in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        let candidate = directory.join("git");
        if candidate.is_file() {
            return candidate;
        }
    }
    panic!("ANTI-VACUITY: no real git on PATH for repo fixtures");
});

/// Install a transparent `git` wrapper once per test-binary process. For
/// repos carrying a `.reprobe-arm` file it serves the armed revision on the
/// first `rev-parse HEAD` and a diverged revision after; every other
/// invocation -- and every invocation for unarmed repos -- delegates to the
/// real git. Arming is per-repo fixture state, so parallel tests sharing
/// this process cannot observe each other's flip.
static MOCK_INSTALLED: std::sync::LazyLock<()> = std::sync::LazyLock::new(|| {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .expect("test HOME");
    let directory = home
        .join(".local/state/zeststream/scratch/omp-orchestrator")
        .join(format!("ompo-start-test-{}", std::process::id()))
        .join("reprobe-git-2zrz");
    std::fs::create_dir_all(&directory).expect("git mock directory");
    let script = format!(
        r#"#!/bin/sh
if [ "$1" = "-C" ] && [ "$3" = "rev-parse" ] && [ "$4" = "HEAD" ] && [ -n "$2" ] && [ -f "$2/.reprobe-arm" ]; then
  CNT="$2/.reprobe-count"
  if [ ! -f "$CNT" ]; then printf '%s\n' "$(cat "$2/.reprobe-arm")"; echo 1 > "$CNT"; else printf '%s\n' "{OBSERVED_REVISION}"; fi
  exit 0
fi
exec "{real}" "$@"
"#,
        real = GIT_REAL.display()
    );
    let binary = directory.join("git");
    std::fs::write(&binary, script).expect("git mock program");
    use std::os::unix::fs::PermissionsExt as _;
    let mut permissions = std::fs::metadata(&binary)
        .expect("git mock metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&binary, permissions).expect("git mock executable");
    let mut paths =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect::<Vec<_>>();
    if !paths.iter().any(|path| path == &directory) {
        paths.insert(0, directory);
        std::env::set_var("PATH", std::env::join_paths(paths).expect("mock PATH"));
    }
});

fn run_git(repo: &Path, args: &[&str]) {
    // Fixture setup always reaches the real git even once the mock is on
    // PATH: the mock delegates everything except an armed rev-parse.
    let status = Command::new(&*GIT_REAL)
        .current_dir(repo)
        .args(args)
        .status()
        .expect("git must exist for repo fixtures");
    assert!(status.success(), "git {args:?} failed in fixture");
}

/// Passing-shape repo for the shared `initialize` entry. Deliberately light:
/// `initialize` (unlike the gated entry) probes control files, git identity,
/// and the AGENTS stamp only -- no beads, pin, hook, mail, or RCH fixtures.
fn init_repo_fixture() -> TempDir {
    let directory = tempfile::tempdir().expect("fixture directory");
    std::fs::create_dir(directory.path().join("docs")).expect("docs directory");
    for name in ["CLAUDE.md", "README.md", "SCHEMAS.toml"] {
        std::fs::write(directory.path().join(name), b"fixture\n").expect("control file");
    }
    // Stamp rule: the token is REFERENCED, never copied.
    std::fs::write(
        directory.path().join("AGENTS.md"),
        format!("fixture {PROJECT_AGENTS_OWNERSHIP_STAMP}\n"),
    )
    .expect("stamped AGENTS.md");
    std::fs::write(
        directory.path().join("Cargo.toml"),
        b"[package]\nname = \"ompo-start\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
    )
    .expect("Cargo metadata fixture");
    std::fs::write(directory.path().join("docs/decisions.jsonl"), b"{}\n")
        .expect("decision ledger");
    for args in [
        ["init", "-q"].as_slice(),
        ["add", "."].as_slice(),
        [
            "-c",
            "user.name=ompo-start-test",
            "-c",
            "user.email=ompo-start-test@example.invalid",
            "commit",
            "-qm",
            "fixture",
        ]
        .as_slice(),
    ] {
        run_git(directory.path(), args);
    }
    directory
}

fn control_message(error: &ompo_start::inception::InceptionError) -> String {
    error.to_string()
}

/// CONTROL: quiescent repo passes init and the direct re-probe. Green under
/// the call-site-removal mutation (it never reaches the removed call) and
/// under the helper-gutting mutation it REDs -- either way it discriminates.
#[test]
fn post_write_reprobe_passes_on_quiescent_repo() {
    let repo = init_repo_fixture();
    let output = repo.path().join(".omp-orchestrator/inception.json");
    let report = initialize(repo.path(), &output).expect("accepted init");
    verify_post_write_predicates(repo.path(), &report.manifest, &output)
        .expect("quiescent re-probe passes");
    println!("READBACK quiescent post-write re-probe ok");
}

/// Helper-direct: an advanced HEAD after acceptance halts naming identity.
#[test]
fn post_write_identity_change_halts_naming_identity() {
    let repo = init_repo_fixture();
    let output = repo.path().join(".omp-orchestrator/inception.json");
    let report = initialize(repo.path(), &output).expect("accepted init");
    std::fs::write(repo.path().join("later.txt"), b"later\n").expect("later file");
    run_git(repo.path(), &["add", "later.txt"]);
    run_git(
        repo.path(),
        &[
            "-c",
            "user.name=ompo-start-test",
            "-c",
            "user.email=ompo-start-test@example.invalid",
            "commit",
            "-qm",
            "later",
        ],
    );
    let error = verify_post_write_predicates(repo.path(), &report.manifest, &output)
        .expect_err("advanced HEAD must halt the re-probe");
    let text = control_message(&error);
    assert!(
        text.starts_with("INCEPTION_READBACK_FAILED"),
        "halt must be typed, got: {text}"
    );
    assert!(
        text.contains("POST_WRITE_PREDICATE_CHANGED") && text.contains("predicate=identity"),
        "halt must name the changed predicate, got: {text}"
    );
}

/// Helper-direct: a deleted control file after acceptance halts naming it.
#[test]
fn post_write_control_change_halts_naming_control() {
    let repo = init_repo_fixture();
    let output = repo.path().join(".omp-orchestrator/inception.json");
    let report = initialize(repo.path(), &output).expect("accepted init");
    std::fs::remove_file(repo.path().join("SCHEMAS.toml")).expect("remove control file");
    let error = verify_post_write_predicates(repo.path(), &report.manifest, &output)
        .expect_err("deleted control file must halt the re-probe");
    let text = control_message(&error);
    assert!(
        text.starts_with("INCEPTION_READBACK_FAILED"),
        "halt must be typed, got: {text}"
    );
    assert!(
        text.contains("POST_WRITE_PREDICATE_CHANGED")
            && text.contains("predicate=control_files")
            && text.contains("SCHEMAS.toml"),
        "halt must name the predicate and the missing file, got: {text}"
    );
}

/// NAMED BLACK-BOX LEG (mutation target): a revision change landing between
/// pre-write acceptance and the post-write re-probe -- simulated here by an
/// armed git that serves one revision to the pre-write probe and a diverged
/// revision after -- halts typed BEFORE any success evidence is emitted.
/// Deleting the `verify_post_write_predicates` call in `initialize_inner`
/// must RED this leg (it gets `Ok`): that split is the wiring proof.
#[test]
fn mid_call_identity_change_halts_before_any_success_emit() {
    std::sync::LazyLock::force(&MOCK_INSTALLED);
    let repo = init_repo_fixture();
    // The wrapper matches on the `-C` path production actually passes,
    // which is the canonicalized root -- arm exactly there so the match
    // cannot miss through a symlinked TMPDIR.
    let root = repo.path().canonicalize().expect("canonical fixture root");
    std::fs::write(root.join(".reprobe-arm"), RECORDED_REVISION).expect("arm file");
    let output = root.join(".omp-orchestrator/inception.json");
    let journal = default_repo_journal(&root);

    let error =
        initialize(&root, &output).expect_err("mid-call revision change must halt init");
    let text = control_message(&error);
    assert!(
        text.starts_with("INCEPTION_READBACK_FAILED"),
        "halt must be typed, got: {text}"
    );
    assert!(
        text.contains("POST_WRITE_PREDICATE_CHANGED") && text.contains("predicate=identity"),
        "halt must name the changed predicate, got: {text}"
    );
    assert!(
        text.contains(RECORDED_REVISION) && text.contains(OBSERVED_REVISION),
        "halt must carry expected versus observed revisions, got: {text}"
    );
    // The write happened; only the SUCCESS was withheld. The journal must
    // carry zero new INIT_REPROBE_OK rows -- the file is absent outright
    // because the emit never ran.
    assert!(
        output.is_file(),
        "the artifact write precedes the re-probe and must stand"
    );
    assert!(
        !journal.is_file(),
        "a halted init must emit no journal success evidence, found {}",
        journal.display()
    );
    println!("READBACK mid-call halt ok, zero success rows emitted");
}

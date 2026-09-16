//! L2-BUILD-HALT-NOT-TAKEN (dvni): a post-write re-probe that does not
//! take must halt typed with zero L3 advancement/success evidence.
//!
//! Scope split with 2zrz (`init_post_write_reprobe.rs`), which owns the
//! re-probe mechanism (`verify_post_write_predicates`), its helper-direct
//! legs, and the mid-call black-box leg. This file owns two properties 2zrz
//! does not pin: the halt carries the TYPED `InceptionError::Readback`
//! variant with the exact predicate detail (not merely a Display substring),
//! and a known-good accepted init advances EXACTLY ONE success row (not zero,
//! not two). The mid-call revision flip below mirrors 2zrz's transparent-git
//! technique (per-repo arm files, first `rev-parse HEAD` serves the recorded
//! revision, later ones the diverged revision); the technique is fixture
//! machinery, not a second re-probe implementation, and no production line
//! is duplicated here.
//!
//! KNOWN-BAD (single-edit mutation): delete the
//! `verify_post_write_predicates(...)` call in `initialize_inner`. The typed
//! halt leg below must RED (it gets `Ok` where it demands the `Readback`
//! variant) while the exactly-once success leg stays green.

#![forbid(unsafe_code)]

use lifecycle_event::default_repo_journal;
use ompo_start::inception::{initialize, InceptionError, PROJECT_AGENTS_OWNERSHIP_STAMP};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

const RECORDED_REVISION: &str = "halt-recorded-r1";
const OBSERVED_REVISION: &str = "halt-observed-r2";

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
/// real git. Mirrors the 2zrz mock shape under a distinct directory so the
/// two binaries can never share wrapper state.
static MOCK_INSTALLED: std::sync::LazyLock<()> = std::sync::LazyLock::new(|| {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .expect("test HOME");
    let directory = home
        .join(".local/state/zeststream/scratch/omp-orchestrator")
        .join(format!("ompo-start-test-{}", std::process::id()))
        .join("reprobe-git-dvni");
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

/// Passing-shape repo for the shared `initialize` entry: control files, git
/// identity, and the AGENTS stamp only.
fn init_repo_fixture() -> TempDir {
    let directory = tempfile::tempdir().expect("fixture directory");
    std::fs::create_dir(directory.path().join("docs")).expect("docs directory");
    for name in ["CLAUDE.md", "README.md", "SCHEMAS.toml"] {
        std::fs::write(directory.path().join(name), b"fixture\n").expect("control file");
    }
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

fn success_row_count(journal: &Path) -> usize {
    let text = std::fs::read_to_string(journal).expect("journal bytes");
    text.lines()
        .filter(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .and_then(|row| {
                    row.get("reason_code")
                        .and_then(|code| code.as_str())
                        .map(|code| code == "INIT_REPROBE_OK")
                })
                .unwrap_or(false)
        })
        .count()
}

/// NAMED LEG (mutation target): a revision change landing between pre-write
/// acceptance and the post-write re-probe halts as the TYPED
/// `InceptionError::Readback` variant carrying the exact predicate detail --
/// and the artifact write stands while zero success evidence is emitted.
#[test]
fn halt_returns_typed_readback_variant_naming_identity_change() {
    std::sync::LazyLock::force(&MOCK_INSTALLED);
    let repo = init_repo_fixture();
    // The wrapper matches on the `-C` path production actually passes,
    // which is the canonicalized root -- arm exactly there so the match
    // cannot miss through a symlinked TMPDIR.
    let root = repo.path().canonicalize().expect("canonical fixture root");
    std::fs::write(root.join(".reprobe-arm"), RECORDED_REVISION).expect("arm file");
    let output = root.join(".omp-orchestrator/inception.json");
    let journal = default_repo_journal(&root);

    let error = initialize(&root, &output).expect_err("mid-call revision change must halt init");
    match error {
        InceptionError::Readback { path, detail } => {
            assert_eq!(path, output, "halt must name the postcondition surface");
            assert_eq!(
                detail,
                format!(
                    "POST_WRITE_PREDICATE_CHANGED predicate=identity field=source_revision expected={RECORDED_REVISION} provided={OBSERVED_REVISION}"
                ),
                "halt must carry the exact predicate detail"
            );
        }
        other => panic!("halt must be the typed Readback variant: {other:?}"),
    }
    assert!(
        output.is_file(),
        "the artifact write precedes the re-probe and must stand"
    );
    assert!(
        !journal.is_file(),
        "a halted init must emit no journal success evidence, found {}",
        journal.display()
    );
    println!("READBACK typed halt ok, zero success rows emitted");
}

/// CONTROL: a quiescent accepted init advances EXACTLY ONE success row --
/// zero would be a silent stall, two would be a double emit. Green under the
/// halt-bypass mutation (the bypassed call never runs on this path), which is
/// what makes it a control rather than a second detector.
#[test]
fn accepted_init_advances_exactly_one_success_row() {
    std::sync::LazyLock::force(&MOCK_INSTALLED);
    let repo = init_repo_fixture();
    let root = repo.path().canonicalize().expect("canonical fixture root");
    let output = root.join(".omp-orchestrator/inception.json");
    let journal = default_repo_journal(&root);

    initialize(&root, &output).expect("accepted init");
    assert_eq!(
        success_row_count(&journal),
        1,
        "one accepted attempt emits exactly one success row"
    );
    println!("READBACK exactly-once success ok");
}

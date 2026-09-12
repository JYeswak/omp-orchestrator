//! STAGED-SET REFERENCE CLOSURE — a staged file must not reference a symbol
//! whose sole declaration lives in a dirty file outside the staged set
//! (bead omp-orchestrator-5fl28: two path-scoped commits jointly broke HEAD
//! for hours while worktree-reading lanes stayed green).
//!
//! These tests drive the real pre-commit binary against throwaway git repos,
//! mirroring tests/empty_staged.rs: the fixture carries exactly what the
//! workspace-wide gates need to stay quiet (R1 boxes, grader agent, bead
//! mirror, hook placeholder), so refusals below are this gate's alone.

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

static FIXTURE_SEQ: AtomicU32 = AtomicU32::new(0);

fn run_git(dir: &Path, args: &[&str], context: &str) -> Output {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .output()
        .unwrap_or_else(|_| panic!("spawn git {args:?} ({context})"));
    assert!(
        output.status.success(),
        "git {args:?} ({context}) failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn fresh_closure_tree(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "no-shell-gate-closure-{}-{test}-{}",
        std::process::id(),
        FIXTURE_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    // Workspace-wide gates must stay quiet: R1 population boxes, the grader
    // agent, a closed bead mirror, and the hypotheses registry the
    // preregistration gate reads from HEAD (absent registry refuses every
    // run, so the fixture commits one with the parent pinned).
    fs::create_dir_all(dir.join("docs/plan/flow/boxes")).expect("R1 fixture");
    fs::write(
        dir.join("docs/plan/flow/CONTRACT.md"),
        "R1_POPULATION_BOXES=fixture\n",
    )
    .expect("R1 contract");
    fs::write(
        dir.join("docs/plan/flow/boxes/fixture.toml"),
        "id = \"fixture\"\nkernel_input = \"input\"\nkernel_output = \"output\"\nevent_row = \"event\"\nvalidator = \"validator\"\nmeasured = \"measured\"\n",
    )
    .expect("R1 box");
    fs::create_dir_all(dir.join("docs/plan")).expect("create fixture plan");
    fs::write(
        dir.join("docs/plan/HYPOTHESES.jsonl"),
        r#"{"id":"h1","prediction":"fixture","falsifier":"missing section","evidence_scope":"docs/plan","recorded_commit":"0123456789abcdef0123456789abcdef01234567","observed_result":null}"#.to_owned() + "\n",
    )
    .expect("write fixture hypotheses");
    fs::create_dir_all(dir.join("crates/hold/src")).expect("hold crate");
    fs::write(
        dir.join("crates/hold/src/lib.rs"),
        "pub fn hold_fixture() {}\n",
    )
    .expect("hold lib");
    fs::create_dir_all(dir.join(".omp/agents")).expect("agent dir");
    fs::write(
        dir.join(no_shell_gate::project_agent::OMP_GRADER_PATH),
        include_str!("../../../.omp/agents/omp-grader.md"),
    )
    .expect("grader agent");
    fs::create_dir_all(dir.join(".beads")).expect("mirror dir");
    fs::write(
        dir.join(".beads/issues.jsonl"),
        "{\"id\":\"fixture-closed\",\"status\":\"closed\",\"close_reason\":\"DONE: fixture baseline worker=local\"}\n",
    )
    .expect("bead mirror");
    run_git(&dir, &["init", "-q"], "git init");
    fs::create_dir_all(dir.join(".git/hooks")).expect("hook dir");
    fs::write(dir.join(".git/hooks/pre-commit"), "fixture hook placeholder\n")
        .expect("hook placeholder");
    // Baseline: a declaring file and a using file with no shared symbols.
    fs::write(
        dir.join("crates/hold/src/decl.rs"),
        "pub struct SettledToken;\n",
    )
    .expect("decl baseline");
    fs::write(
        dir.join("crates/hold/src/user.rs"),
        "pub fn staged_probe() {}\n",
    )
    .expect("user baseline");
    run_git(
        &dir,
        &[
            "add",
            "--",
            "crates/hold/src/lib.rs",
            "docs/plan/HYPOTHESES.jsonl",
            ".beads/issues.jsonl",
            "crates/hold/src/decl.rs",
            "crates/hold/src/user.rs",
        ],
        "stage clean baseline",
    );
    run_git(&dir, &["commit", "-qm", "fixture baseline [test]"], "baseline commit");
    let parent = String::from_utf8_lossy(
        &run_git(&dir, &["rev-parse", "HEAD"], "read fixture parent").stdout,
    )
    .trim()
    .to_owned();
    let hypotheses_path = dir.join("docs/plan/HYPOTHESES.jsonl");
    let hypotheses = fs::read_to_string(&hypotheses_path)
        .expect("read fixture hypotheses")
        .replace("0123456789abcdef0123456789abcdef01234567", &parent);
    fs::write(&hypotheses_path, hypotheses).expect("update fixture parent");
    run_git(&dir, &["add", "--", "docs/plan/HYPOTHESES.jsonl"], "stage updated hypotheses");
    run_git(&dir, &["commit", "-qm", "fixture hypotheses [test]"], "hypotheses commit");
    dir
}

fn stage_paths(dir: &Path, paths: &[&str]) {
    let mut add = vec!["add", "--"];
    add.extend(paths.iter().copied());
    run_git(dir, &add, "stage fixture paths");
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

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn cleanup(dir: &Path) {
    fs::remove_dir_all(dir).ok();
}

/// KNOWN-BAD, reconstructing the 2026-09-11 incident: the staged file uses a
/// symbol whose sole declaration sits in a dirty file outside the staged
/// set. Refuses with the symbol, both files, and the remedy; message AND
/// exit code pinned together.
#[test]
fn staged_use_of_unstaged_declaration_refuses() {
    let dir = fresh_closure_tree("known-bad");
    fs::write(
        dir.join("crates/hold/src/decl.rs"),
        "pub struct SettledToken;\npub enum HoldReason {\n    StagedUseWithoutDeclaration,\n}\n",
    )
    .expect("dirty declaration");
    fs::write(
        dir.join("crates/hold/src/user.rs"),
        "pub fn staged_probe() -> HoldReason {\n    HoldReason::StagedUseWithoutDeclaration\n}\n",
    )
    .expect("staged use");
    stage_paths(&dir, &["crates/hold/src/user.rs"]);
    let output = run_gate(&dir);
    let stderr = stderr_of(&output);
    assert_eq!(output.status.code(), Some(1), "refusal exit:\n{stderr}");
    for needle in [
        "staged-reference-closure: REFUSED",
        "symbol=HoldReason",
        "declared_in=crates/hold/src/decl.rs",
        "used_in=crates/hold/src/user.rs",
    ] {
        assert!(stderr.contains(needle), "missing {needle}:\n{stderr}");
    }
    cleanup(&dir);
}

/// KNOWN-GOOD (a): both files staged together. This is the case the gate
/// must not block, and the remedy the refusal names.
#[test]
fn staged_declarer_and_user_together_is_clean() {
    let dir = fresh_closure_tree("good-both-staged");
    fs::write(
        dir.join("crates/hold/src/decl.rs"),
        "pub struct SettledToken;\npub enum HoldReason {\n    StagedUseWithoutDeclaration,\n}\n",
    )
    .expect("declaration");
    fs::write(
        dir.join("crates/hold/src/user.rs"),
        "pub fn staged_probe() -> HoldReason {\n    HoldReason::StagedUseWithoutDeclaration\n}\n",
    )
    .expect("use");
    stage_paths(
        &dir,
        &["crates/hold/src/decl.rs", "crates/hold/src/user.rs"],
    );
    let output = run_gate(&dir);
    let stderr = stderr_of(&output);
    assert_eq!(output.status.code(), Some(0), "both staged must pass:\n{stderr}");
    assert!(
        stderr.contains("staged-reference-closure: CLEAN"),
        "clean marker missing:\n{stderr}"
    );
    assert!(
        !stderr.contains("staged-reference-closure: REFUSED"),
        "unexpected refusal:\n{stderr}"
    );
    cleanup(&dir);
}

/// KNOWN-GOOD (b): a dirty file outside the staged set that declares nothing
/// the staged set uses. Without this arm the gate degenerates into refusing
/// whenever the tree is dirty. This is also the mutation discriminator: drop
/// the use-requirement and exactly this leg reddens (MINIMAL).
#[test]
fn dirty_unused_declaration_is_clean() {
    let dir = fresh_closure_tree("good-unused");
    fs::write(
        dir.join("crates/hold/src/decl.rs"),
        "pub struct SettledToken;\npub struct UnusedProbe;\n",
    )
    .expect("unused declaration");
    fs::write(
        dir.join("crates/hold/src/user.rs"),
        "pub fn staged_probe() {}\npub fn unrelated_probe() {}\n",
    )
    .expect("unrelated staged change");
    stage_paths(&dir, &["crates/hold/src/user.rs"]);
    let output = run_gate(&dir);
    let stderr = stderr_of(&output);
    assert_eq!(output.status.code(), Some(0), "unused dirt must pass:\n{stderr}");
    assert!(
        stderr.contains("staged-reference-closure: CLEAN"),
        "clean marker missing:\n{stderr}"
    );
    cleanup(&dir);
}

/// ANTI-VACUITY at the trigger: an empty staged set is the trigger's own
/// terminal outcome (exit 3), never a pass and never a CLEAN from this gate.
#[test]
fn empty_staged_set_is_terminal_not_pass() {
    let dir = fresh_closure_tree("empty");
    let output = run_gate(&dir);
    let stderr = stderr_of(&output);
    assert_eq!(output.status.code(), Some(3), "empty must terminate:\n{stderr}");
    assert!(stderr.contains("NOTHING_TO_CHECK"), "terminal marker:\n{stderr}");
    assert!(
        !stderr.contains("staged-reference-closure: CLEAN"),
        "empty must not certify:\n{stderr}"
    );
    cleanup(&dir);
}

/// REGRESSION (self-caught 2026-09-12): a worktree-DELETED file declares
/// nothing, so its absence is an answer, not a git ERROR. The first version
/// refused every commit in a tree with a deleted-but-unstaged file — and
/// refused its own author's commit first.
#[test]
fn deleted_unstaged_file_is_absent_not_error() {
    let dir = fresh_closure_tree("deleted-unstaged");
    fs::write(
        dir.join("crates/hold/src/gone.rs"),
        "pub struct VanishedToken;\n",
    )
    .expect("doomed file");
    run_git(&dir, &["add", "--", "crates/hold/src/gone.rs"], "stage doomed");
    run_git(&dir, &["commit", "-qm", "doomed baseline [test]"], "commit doomed");
    fs::remove_file(dir.join("crates/hold/src/gone.rs")).expect("delete worktree copy");
    fs::write(
        dir.join("crates/hold/src/user.rs"),
        "pub fn staged_probe() {}\npub fn unrelated_probe() {}\n",
    )
    .expect("unrelated staged change");
    stage_paths(&dir, &["crates/hold/src/user.rs"]);
    let output = run_gate(&dir);
    let stderr = stderr_of(&output);
    assert_eq!(output.status.code(), Some(0), "deleted dirt must pass:\n{stderr}");
    assert!(
        stderr.contains("staged-reference-closure: CLEAN"),
        "clean marker missing:\n{stderr}"
    );
    cleanup(&dir);
}

/// Direct classifier legs without the binary: the terminal and git-error
/// arms are typed outcomes, never silent passes.
#[test]
fn direct_terminal_and_git_error_are_typed() {
    use no_shell_gate::commit_ratchets::{check_staged_closure, StagedClosure};
    assert!(matches!(
        check_staged_closure(Path::new("/nonexistent"), &[], &[]),
        StagedClosure::TerminalEmpty { .. }
    ));
    assert!(matches!(
        check_staged_closure(
            Path::new("/nonexistent"),
            &["crates/hold/src/user.rs".to_owned()],
            &[]
        ),
        StagedClosure::GitError { .. }
    ));
}

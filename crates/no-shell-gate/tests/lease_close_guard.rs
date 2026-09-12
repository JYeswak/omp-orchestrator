//! CLOSE-LEASE-GUARD — a staged mirror close must not land while its own
//! lease record is still exclusively held (bead `omp-orchestrator-3w9l`).
//!
//! These tests exercise the actual pre-commit binary against throwaway git
//! indexes. Fixture harness duplicated from `empty_staged.rs` (each file in
//! `tests/` is its own crate, so helpers cannot be shared): `fresh_git_tree`
//! below is that file's tree verbatim, which keeps every workspace-wide arm
//! answerable instead of exempt.
//!
//! The daemon-reachable verdict (`StillHeld`) is covered where it is
//! provable: pure record×report matching lives in `agent-mail-native`'s
//! hermetic legs, and the live verify path in its ignored live legs. Here
//! the binary proves the wiring: record-less closes stay CLEAN, a dead
//! daemon is a typed ERROR (never a pass), and a malformed record refuses.

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

static FIXTURE_SEQ: AtomicU32 = AtomicU32::new(0);

fn fresh_git_tree(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "no-shell-gate-lease-guard-{}-{test}-{}",
        std::process::id(),
        FIXTURE_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(dir.join("docs/plan/flow/boxes")).expect("create R1 fixture");
    fs::write(
        dir.join("docs/plan/flow/CONTRACT.md"),
        "R1_POPULATION_BOXES=fixture\n",
    )
    .expect("write R1 contract fixture");
    fs::write(
        dir.join("docs/plan/flow/boxes/fixture.toml"),
        "id = \"fixture\"\nkernel_input = \"input\"\nkernel_output = \"output\"\nevent_row = \"event\"\nvalidator = \"validator\"\nmeasured = \"measured\"\n",
    )
    .expect("write R1 box fixture");
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
    fs::create_dir_all(dir.join(".omp/agents")).expect("create fixture agent dir");
    fs::write(
        dir.join(no_shell_gate::project_agent::OMP_GRADER_PATH),
        include_str!("../../../.omp/agents/omp-grader.md"),
    )
    .expect("write fixture grader agent");
    fs::create_dir_all(dir.join(".beads")).expect("create fixture bead mirror dir");
    fs::write(
        dir.join(".beads/issues.jsonl"),
        "{\"id\":\"fixture-closed\",\"status\":\"closed\",\"close_reason\":\"DONE: fixture baseline worker=local\"}\n",
    )
    .expect("write fixture bead mirror");
    run_git(&dir, &["init", "-q"], "git init");
    fs::create_dir_all(dir.join(".git/hooks")).expect("create fixture hook dir");
    fs::write(dir.join(".git/hooks/pre-commit"), "fixture hook placeholder\n")
        .expect("write fixture hook");
    run_git(
        &dir,
        &["add", "--", "crates/example/src/lib.rs", "docs/plan/HYPOTHESES.jsonl", ".beads/issues.jsonl"],
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

fn stage_close_mirror(dir: &Path, content: &str) {
    fs::create_dir_all(dir.join(".beads")).expect("create bead mirror directory");
    stage(dir, ".beads/issues.jsonl", content);
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

/// KNOWN-GOOD (acceptance item 5): a close carrying no lease record for its
/// own id commits CLEAN through the lease arm. The row is conforming on the
/// sibling close-reason arm (sanctioned prefix + worker authority) so the
/// lease verdict is the only thing this leg measures.
#[test]
fn close_without_lease_record_is_lease_clean() {
    let dir = fresh_git_tree("no-record-clean");
    stage_close_mirror(
        &dir,
        r#"{"id":"unleased-close","status":"closed","close_reason":"DONE: fixture work worker=local","comments":[{"text":"no leases were taken for this bead."}]}
"#,
    );
    let output = run_gate(&dir);
    let error = stderr(&output);
    assert_eq!(
        output.status.code(),
        Some(0),
        "a record-less close must commit: {error}"
    );
    assert!(
        error.contains("lease-guard: state=CLEAN"),
        "the arm must report its own CLEAN, not stay silent: {error}"
    );
}

/// KNOWN-BAD (daemon arm): a close WITH a lease record while the daemon
/// cannot be reached is a typed ERROR, never a pass. `AM_MCP_URL` points at
/// a dead port so the verdict is deterministic with or without a live
/// daemon: connection-refused, not a timeout. Only fixtures carrying a
/// LEASE record reach the endpoint read, so the process-global var cannot
/// affect any other test in this binary.
#[test]
fn close_with_lease_record_and_dead_daemon_is_typed_error() {
    let dir = fresh_git_tree("dead-daemon-error");
    stage_close_mirror(
        &dir,
        r#"{"id":"leased-close","status":"closed","close_reason":"DONE: fixture work worker=local","comments":[{"text":"LEASE bead=leased-close holder=FixtureHolder ack=%9 paths=zz-3w9l-fake.rs"}]}
"#,
    );
    std::env::set_var("AM_MCP_URL", "http://127.0.0.1:9/mcp/");
    let output = run_gate(&dir);
    std::env::remove_var("AM_MCP_URL");
    let error = stderr(&output);
    assert_eq!(
        output.status.code(),
        Some(1),
        "an unverifiable lease close must refuse: {error}"
    );
    assert!(
        error.contains("lease-guard: state=ERROR")
            && error.contains("daemon_unreachable"),
        "the refusal must be the typed daemon ERROR: {error}"
    );
}

/// KNOWN-BAD (parse arm): a malformed LEASE line in a newly closing row
/// refuses before any daemon contact — a typo'd record that silently
/// passed would be a close the guard claimed to check and did not.
#[test]
fn close_with_malformed_lease_record_is_typed_error() {
    let dir = fresh_git_tree("malformed-record-error");
    stage_close_mirror(
        &dir,
        r#"{"id":"bad-record-close","status":"closed","close_reason":"DONE: fixture work worker=local","comments":[{"text":"LEASE bead=bad-record-close holder=FixtureHolder"}]}
"#,
    );
    let output = run_gate(&dir);
    let error = stderr(&output);
    assert_eq!(
        output.status.code(),
        Some(1),
        "a malformed lease record must refuse: {error}"
    );
    assert!(
        error.contains("lease-guard: state=ERROR")
            && error.contains("lease_record_invalid")
            && error.contains("bad-record-close"),
        "the refusal must name the malformed-record cause and bead: {error}"
    );
}

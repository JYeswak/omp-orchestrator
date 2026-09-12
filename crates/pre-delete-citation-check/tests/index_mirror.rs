//! Legs for `omp-orchestrator-5lgku`: the citation gate's ORACLE is the INDEX blob of
//! `.beads/issues.jsonl`, never the worktree file.
//!
//! THE DEFECT. The index, the worktree and HEAD are three different trees and they diverge
//! constantly in a twelve-agent shared checkout -- measured 2026-09-11 on ONE file: worktree
//! 13637 B, index 11279 B, HEAD 0 B, with the index advanced by a third party mid-read. The
//! commit is made of the INDEX, so a pre-commit gate reading the worktree judges rows the
//! commit is not landing and misses rows it IS landing. Fifth instance of open P0
//! `omp-orchestrator-249hz`; the close-reason gate's copy landed the same day at f194a01.
//!
//! WHAT EACH LEG IS FOR:
//! * `a_staged_citation_refuses_with_its_exit_code` -- FIRES-ON-KNOWN-BAD, message AND code.
//! * `a_divergent_worktree_row_does_not_change_the_verdict` -- the leg the whole change
//!   exists for. Without it, a mutation reverting the reader to the worktree stays green.
//! * `the_index_read_returns_the_staged_bytes_not_the_worktree_bytes` -- the same claim at
//!   byte level, with the divergence itself asserted so the fixture cannot go stale.
//! * `a_mirror_absent_from_the_index_is_not_applicable_not_a_refusal` -- the unsatisfiable
//!   shape this repo removed twice on 2026-09-11 must not come back in through this door.
//! * `an_empty_staged_mirror_is_a_typed_error_not_a_pass` -- anti-vacuity: an oracle that
//!   was read and came back empty is an ERROR, distinct from one that is not applicable.
//!
//! Every binary leg pins the MESSAGE SUBSTRING AND THE EXIT CODE together. This repo has
//! measured each pin failing alone, in both directions: an exit-code-only leg goes green on
//! an unrelated panic, and a message-only leg goes green on a gate that printed the refusal
//! and then exited 0.

use pre_delete_citation_check::{read_index_mirror, IndexMirror};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The path the fixture beads cite and the fixture commit deletes.
const VICTIM: &str = "bin/omp-idle-dispatch.sh";

/// A closed row that cites the victim ONLY in a comment -- the shape 14 live beads have.
fn citing_row() -> String {
    format!(
        r#"{{"id":"fx-cites","status":"closed","close_reason":"DONE superseded","comments":[{{"author":"a","text":"replaced {VICTIM} with the crate"}}]}}"#
    )
}

/// A closed row that cites nothing. Same denominator, no conflict.
fn clean_row() -> String {
    r#"{"id":"fx-clean","status":"closed","close_reason":"DONE nothing cited","comments":[]}"#
        .to_owned()
}

fn run_git(root: &Path, args: &[&str]) -> Output {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git runs on the test host");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// A fresh repository with the victim file and a mirror committed, then the victim's
/// deletion STAGED. `mirror` is what the index carries for `.beads/issues.jsonl`.
fn repo_with_staged_deletion(label: &str, mirror: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "pre-delete-index-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("clear stale fixture");
    }
    std::fs::create_dir_all(root.join("bin")).expect("fixture bin");
    std::fs::create_dir_all(root.join(".beads")).expect("fixture beads");
    run_git(&root, &["init", "-q"]);
    run_git(&root, &["config", "user.email", "fixture@test"]);
    run_git(&root, &["config", "user.name", "fixture"]);
    run_git(&root, &["config", "commit.gpgsign", "false"]);

    std::fs::write(root.join(VICTIM), "legacy dispatcher\n").expect("victim");
    write_mirror(&root, mirror);
    run_git(&root, &["add", "-A"]);
    run_git(&root, &["commit", "--no-verify", "-q", "-m", "fixture base"]);

    // The commit under test deletes the cited file. `git rm` stages the deletion.
    run_git(&root, &["rm", "-q", "--", VICTIM]);
    root
}

fn write_mirror(root: &Path, contents: &str) {
    std::fs::write(root.join(".beads").join("issues.jsonl"), contents).expect("mirror");
}

fn stage_mirror(root: &Path) {
    run_git(root, &["add", "--", ".beads/issues.jsonl"]);
}

fn run_gate(root: &Path) -> Output {
    Command::new(PathBuf::from(env!(
        "CARGO_BIN_EXE_pre-delete-citation-check"
    )))
    .current_dir(root)
    .env_remove("PRE_DELETE_OVERRIDE")
    .output()
    .expect("the gate binary must run")
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// FIRES-ON-KNOWN-BAD: a STAGED closed row citing the staged deletion must refuse, and the
/// refusal must name the bead. Message substring AND exit code pinned together.
#[test]
fn a_staged_citation_refuses_with_its_exit_code() {
    let root = repo_with_staged_deletion("known-bad", &format!("{}\n", citing_row()));

    let output = run_gate(&root);
    let stderr = stderr_of(&output);
    assert_eq!(
        output.status.code(),
        Some(1),
        "a staged citation of a staged deletion must exit 1 (REFUSED). stderr={stderr}"
    );
    assert!(
        stderr.contains("REFUSED") && stderr.contains("fx-cites") && stderr.contains(VICTIM),
        "the refusal must name the bead and the path it cites; stderr={stderr}"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// THE LEG THIS CHANGE EXISTS FOR. The index carries a CLEAN mirror; the worktree carries a
/// citing one that this commit is not landing. The verdict must come from the index.
///
/// POSITIVE CONTROL IN THE SAME TEST, so a green cannot mean "the fixture never matched":
/// the identical citing row is then STAGED and the same gate refuses. Without that control
/// this leg would also pass against a gate that read no oracle at all.
#[test]
fn a_divergent_worktree_row_does_not_change_the_verdict() {
    let root = repo_with_staged_deletion("divergent", &format!("{}\n", clean_row()));

    // Divergence: worktree cites the deletion, index does not. Nothing staged after this.
    write_mirror(
        &root,
        &format!("{}\n{}\n", clean_row(), citing_row()),
    );

    let output = run_gate(&root);
    let stderr = stderr_of(&output);
    assert_eq!(
        output.status.code(),
        Some(0),
        "a citation that exists ONLY in the unstaged worktree must not refuse a commit that \
         does not contain it. stderr={stderr}"
    );
    assert!(
        stderr.contains("closed_beads=1") && stderr.contains("conflicts=0"),
        "the denominator line must prove the oracle was READ (one staged closed row) rather \
         than empty; an empty oracle would give conflicts=0 for the wrong reason. \
         stderr={stderr}"
    );

    // POSITIVE CONTROL: stage the very same worktree bytes and the gate must now refuse.
    stage_mirror(&root);
    let staged_output = run_gate(&root);
    let staged_stderr = stderr_of(&staged_output);
    assert_eq!(
        staged_output.status.code(),
        Some(1),
        "control: once the citing row IS staged the gate must refuse, otherwise the leg above \
         proves nothing. stderr={staged_stderr}"
    );
    assert!(
        staged_stderr.contains("fx-cites"),
        "control: the refusal must name the now-staged bead; stderr={staged_stderr}"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// The same claim at BYTE level, and the divergence itself is asserted so the fixture cannot
/// rot into "index and worktree happen to be equal".
#[test]
fn the_index_read_returns_the_staged_bytes_not_the_worktree_bytes() {
    let root = repo_with_staged_deletion("bytes", &format!("{}\n", clean_row()));
    let worktree_text = format!("{}\n{}\n", clean_row(), citing_row());
    write_mirror(&root, &worktree_text);

    let staged_text = String::from_utf8(
        run_git(&root, &["show", ":.beads/issues.jsonl"])
            .stdout
            .clone(),
    )
    .expect("staged mirror is UTF-8");
    assert_ne!(
        staged_text, worktree_text,
        "premise of the leg: the two trees must actually differ here"
    );

    match read_index_mirror(&root).expect("the index read must succeed") {
        IndexMirror::Staged(text) => assert_eq!(
            text, staged_text,
            "the reader must return the INDEX bytes; returning the worktree bytes is 249hz"
        ),
        other => panic!("expected IndexMirror::Staged; got {other:?}"),
    }
    std::fs::remove_dir_all(&root).ok();
}

/// NOT A REFUSAL. A commit whose tree carries no tracker has no oracle to check against, and
/// refusing every such commit is the unsatisfiable-gate shape this repo removed twice on
/// 2026-09-11. It must be ANNOUNCED, though: a silent pass is the other failure.
#[test]
fn a_mirror_absent_from_the_index_is_not_applicable_not_a_refusal() {
    let root = repo_with_staged_deletion("not-applicable", &format!("{}\n", citing_row()));
    // Remove the mirror from the INDEX while leaving the citing copy in the worktree: a
    // worktree reader would refuse here, an index reader has nothing to read.
    run_git(&root, &["rm", "-q", "--cached", "--", ".beads/issues.jsonl"]);
    write_mirror(&root, &format!("{}\n", citing_row()));

    assert_eq!(
        read_index_mirror(&root).expect("the index read must succeed"),
        IndexMirror::NotInIndex,
        "a mirror absent from the index must be typed NotInIndex, not an empty success"
    );

    let output = run_gate(&root);
    let stderr = stderr_of(&output);
    assert_eq!(
        output.status.code(),
        Some(0),
        "a commit whose tree carries no tracker must not be refused. stderr={stderr}"
    );
    assert!(
        stderr.contains("GATE_NOT_APPLICABLE") && stderr.contains("mirror_not_in_index"),
        "the not-applicable verdict must be NAMED on stderr, never silent; stderr={stderr}"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// ANTI-VACUITY: an oracle that WAS read and came back with no records is an error with a
/// named reason and its own exit code -- never a pass, and never confused with the
/// not-applicable case above.
#[test]
fn an_empty_staged_mirror_is_a_typed_error_not_a_pass() {
    let root = repo_with_staged_deletion("empty-oracle", "   \n\n");
    // A healthy worktree copy, to prove the error comes from the STAGED bytes.
    write_mirror(&root, &format!("{}\n", clean_row()));

    let output = run_gate(&root);
    let stderr = stderr_of(&output);
    assert_eq!(
        output.status.code(),
        Some(3),
        "an empty staged oracle must exit 3 (unreadable), not 0. stderr={stderr}"
    );
    assert!(
        stderr.contains("PRE_DELETE_BEADS_EMPTY reason=zero_bead_records_readable"),
        "the error must name WHY the oracle is unusable; stderr={stderr}"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// The repository root is the FIRST POSITIONAL and it must actually be honoured: the
/// manifest's `[package.metadata.gate] checks = [["{repo}"]]` has always passed one, and the
/// binary used to ignore it and read the process CWD instead.
#[test]
fn the_positional_repo_root_is_honoured_not_ignored() {
    let root = repo_with_staged_deletion("positional", &format!("{}\n", citing_row()));
    let elsewhere = std::env::temp_dir();

    let output = Command::new(PathBuf::from(env!(
        "CARGO_BIN_EXE_pre-delete-citation-check"
    )))
    .arg(&root)
    .current_dir(elsewhere)
    .env_remove("PRE_DELETE_OVERRIDE")
    .output()
    .expect("the gate binary must run");
    let stderr = stderr_of(&output);
    assert_eq!(
        output.status.code(),
        Some(1),
        "run from OUTSIDE the fixture with the root as argv[1], the gate must still see the \
         staged deletion and refuse. stderr={stderr}"
    );
    assert!(
        stderr.contains("fx-cites"),
        "the refusal must name the bead from the NAMED repository; stderr={stderr}"
    );
    std::fs::remove_dir_all(&root).ok();
}

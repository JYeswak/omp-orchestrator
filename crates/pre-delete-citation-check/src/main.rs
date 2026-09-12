#![forbid(unsafe_code)]

//! pre-delete-citation-check — pre-commit hook binary.
//!
//! Refuses `git commit` when any staged DELETION is cited by a CLOSED bead as
//! evidence. Scans close_reason AND comments. Override names the superseding
//! artifact and the caller writes a comment onto each affected bead.
//!
//! BOTH INPUTS COME FROM THE INDEX (`omp-orchestrator-5lgku`). The deletion list is
//! `git diff --cached` and the closed-bead oracle is the STAGED blob of
//! `.beads/issues.jsonl`, because the commit is made of the index and a gate that
//! judges any other tree reports on rows this commit is not landing.

use pre_delete_citation_check::{
    ChildOutcome, IndexMirror, MirrorReadError, GIT_DIFF_DEADLINE, MIRROR_PATH,
};
use std::path::PathBuf;
use std::process::ExitCode;

/// Exit code for a DEADLINE, kept distinct from 3 (spawn/failed child) and 1
/// (citation refusal). An operator must be able to tell "the subject is wedged"
/// from "the subject answered badly"; collapsing them is the conflation that cost
/// this repo a six-hour false diagnosis.
const EXIT_TIMED_OUT: u8 = 4;

/// Exit code for an ORACLE THAT WAS NOT READ. Distinct from 1 so a refusal for a real
/// citation is never confused with a gate that could not run.
const EXIT_UNREADABLE: u8 = 3;

fn main() -> ExitCode {
    // The repository root is the FIRST POSITIONAL, as `[package.metadata.gate]`
    // already advertises (`checks = [["{repo}"]]`) and as commit-build-fence's roster
    // records for this binary. It was declared and ignored: every read ran against the
    // process CWD, so a gate-runner invocation naming a repo checked whatever directory
    // it happened to be launched from. `.` keeps the hook's behaviour, which git runs
    // from the top level.
    let repo_root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    // 1. Get the staged deletions, UNDER A DEADLINE.
    let mut git_command = std::process::Command::new("git");
    git_command
        .arg("-C")
        .arg(&repo_root)
        .args(["diff", "--cached", "--diff-filter=D", "--name-only"]);
    let diff_stdout = match pre_delete_citation_check::run_bounded(
        &mut git_command,
        GIT_DIFF_DEADLINE,
    ) {
        ChildOutcome::Completed {
            code: Some(0),
            stdout,
            ..
        } => stdout,
        // A killed or failed git produces empty stdout — reading that as
        // "no staged deletions" would pass the gate on a dead child.
        ChildOutcome::Completed { code, .. } => {
            eprintln!(
                "pre-delete-citation-check: git diff exited {code:?} — refusing to pass on a killed child"
            );
            return ExitCode::from(EXIT_UNREADABLE);
        }
        ChildOutcome::TimedOut {
            after_ms,
            group_killed,
        } => {
            eprintln!(
                "pre-delete-citation-check: PRE_DELETE_GIT_TIMEOUT git diff did not return \
                 within {after_ms}ms; process group signalled={group_killed}. This is NOT a git \
                 failure verdict — no exit status was observed, so the staged-deletion set is \
                 UNKNOWN and the gate cannot certify this commit."
            );
            return ExitCode::from(EXIT_TIMED_OUT);
        }
        ChildOutcome::SpawnFailed { message } => {
            eprintln!("pre-delete-citation-check: git diff failed to spawn: {message}");
            return ExitCode::from(EXIT_UNREADABLE);
        }
    };
    let staged: Vec<String> = diff_stdout
        .lines()
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect();

    if staged.is_empty() {
        // Nothing deleted — pass trivially. This is NOT the anti-vacuity hole: the
        // empty set here is the COMMIT's deletion list, and a commit that deletes
        // nothing genuinely has nothing for this gate to check. The vacuous pass this
        // gate must refuse is an empty CLOSED-BEAD oracle while deletions ARE staged,
        // which `parse_closed_beads_jsonl_checked` below makes restrictive.
        return ExitCode::SUCCESS;
    }

    // 2. Get the closed beads from the STAGED mirror blob.
    //
    // This used to be `br list --json --status closed` under a 300s deadline, and both
    // halves were wrong. The br JSON carries no `comments` key at all, so the
    // comment-scanning half of `check_deletions` ran against an empty vector on every
    // bead (`omp-orchestrator-dpa4`), while 14 closed beads in this tracker cite a
    // deleted path ONLY in a comment. The spawn also took `.beads/.write.lock` on the
    // commit path, which is why it needed a contention-band deadline at all. An index
    // read has neither failure mode.
    let staged_mirror = match pre_delete_citation_check::read_index_mirror(&repo_root) {
        Ok(IndexMirror::Staged(text)) => text,
        Ok(IndexMirror::NotInIndex) => {
            // NOT A REFUSAL. This commit's tree carries no tracker, so there is no
            // oracle to cross-reference; refusing every such commit is an unsatisfiable
            // gate. It is announced, never silent, so a green run still says what ran.
            eprintln!(
                "pre-delete-citation-check: GATE_NOT_APPLICABLE reason=mirror_not_in_index \
                 path={MIRROR_PATH} staged_deletions={} — no closed-bead oracle in this \
                 commit's tree; nothing was checked.",
                staged.len()
            );
            return ExitCode::SUCCESS;
        }
        // A TIMEOUT IS NOT A VERDICT, and it keeps its own exit code.
        Err(error @ MirrorReadError::Timeout { .. }) => {
            eprintln!("pre-delete-citation-check: {error}");
            return ExitCode::from(EXIT_TIMED_OUT);
        }
        Err(error) => {
            eprintln!("pre-delete-citation-check: {error}");
            return ExitCode::from(EXIT_UNREADABLE);
        }
    };
    let closed_beads =
        match pre_delete_citation_check::parse_closed_beads_jsonl_checked(&staged_mirror) {
            Ok(beads) => beads,
            Err(error) => {
                eprintln!("pre-delete-citation-check: {error}");
                return ExitCode::from(EXIT_UNREADABLE);
            }
        };
    // 3. Cross-reference.
    let conflicts = pre_delete_citation_check::check_deletions(&staged, &closed_beads);

    // DENOMINATORS, so a reader can tell "checked nothing" from "checked everything and
    // found nothing". A bare "no conflicts" is the vacuous-green shape this gate refuses.
    eprintln!(
        "pre-delete-citation-check: staged_deletions={} closed_beads={} with_comments={} \
         conflicts={} oracle=index:{MIRROR_PATH}",
        staged.len(),
        closed_beads.len(),
        closed_beads
            .iter()
            .filter(|bead| !bead.comments.is_empty())
            .count(),
        conflicts.len()
    );

    // 4. Override: names the superseding artifact, allows the deletion.
    if let Ok(override_artifact) = std::env::var("PRE_DELETE_OVERRIDE") {
        if !override_artifact.trim().is_empty() && !conflicts.is_empty() {
            eprintln!(
                "pre-delete-citation-check: OVERRIDE active (superseding artifact: {override_artifact})"
            );
            eprintln!(
                "pre-delete-citation-check: {} citation conflict(s) overridden; \
                 write a comment on each affected bead to repair the citation",
                conflicts.len()
            );
            for conflict in &conflicts {
                eprintln!("  AFFECTED {conflict}");
            }
            return ExitCode::SUCCESS;
        }
    }

    // 5. Refuse if any conflicts.
    if !conflicts.is_empty() {
        eprintln!(
            "pre-delete-citation-check: REFUSED — {} staged deletion(s) cited by closed bead(s):",
            conflicts.len()
        );
        for conflict in &conflicts {
            eprintln!("  {conflict}");
        }
        eprintln!(
            "Set PRE_DELETE_OVERRIDE=<superseding-artifact> to proceed; \
             write a comment on each affected bead to repair the citation."
        );
        return ExitCode::from(1);
    }

    ExitCode::SUCCESS
}

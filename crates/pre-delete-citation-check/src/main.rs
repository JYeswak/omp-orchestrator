#![forbid(unsafe_code)]

//! pre-delete-citation-check — pre-commit hook binary.
//!
//! Refuses `git commit` when any staged DELETION is cited by a CLOSED bead as
//! evidence. Scans close_reason AND comments. Override names the superseding
//! artifact and the caller writes a comment onto each affected bead.

use pre_delete_citation_check::{ChildOutcome, BR_LIST_DEADLINE, GIT_DIFF_DEADLINE};
use std::process::ExitCode;

/// Exit code for a DEADLINE, kept distinct from 3 (spawn/failed child) and 1
/// (citation refusal). An operator must be able to tell "the subject is wedged"
/// from "the subject answered badly"; collapsing them is the conflation that cost
/// this repo a six-hour false diagnosis.
const EXIT_TIMED_OUT: u8 = 4;

fn main() -> ExitCode {
    // 1. Get the staged deletions, UNDER A DEADLINE.
    let mut git_command = std::process::Command::new("git");
    git_command.args(["diff", "--cached", "--diff-filter=D", "--name-only"]);
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
            return ExitCode::from(3);
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
            return ExitCode::from(3);
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
        // which `parse_closed_beads_checked` below makes restrictive.
        return ExitCode::SUCCESS;
    }

    // 2. Get the closed beads, UNDER A DEADLINE. This is the spawn that actually
    // wedges: ~31 MB of beads.db with several live writers.
    let mut br_command = std::process::Command::new(finding::BR);
    br_command.args(["list", "--json", "--status", "closed"]);
    let br_text = match pre_delete_citation_check::run_bounded(&mut br_command, BR_LIST_DEADLINE)
    {
        ChildOutcome::Completed {
            code: Some(0),
            stdout,
            ..
        } => stdout,
        ChildOutcome::Completed { code, .. } => {
            eprintln!(
                "pre-delete-citation-check: br list exited {code:?} — refusing to pass on a killed child"
            );
            return ExitCode::from(3);
        }
        ChildOutcome::TimedOut {
            after_ms,
            group_killed,
        } => {
            eprintln!(
                "pre-delete-citation-check: PRE_DELETE_BR_TIMEOUT br list did not return within \
                 {after_ms}ms; process group signalled={group_killed}. This is NOT a br failure \
                 verdict — the closed-bead set is UNKNOWN, so a staged deletion cannot be \
                 certified citation-free."
            );
            return ExitCode::from(EXIT_TIMED_OUT);
        }
        ChildOutcome::SpawnFailed { message } => {
            eprintln!("pre-delete-citation-check: br list failed to spawn: {message}");
            return ExitCode::from(3);
        }
    };
    let closed_beads = match pre_delete_citation_check::parse_closed_beads_checked(&br_text) {
        Ok(beads) => beads,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(3);
        }
    };
    // 3. Cross-reference.
    let conflicts = pre_delete_citation_check::check_deletions(&staged, &closed_beads);

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

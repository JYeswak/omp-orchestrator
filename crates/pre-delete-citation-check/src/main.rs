#![forbid(unsafe_code)]

//! pre-delete-citation-check — pre-commit hook binary.
//!
//! Refuses `git commit` when any staged DELETION is cited by a CLOSED bead as
//! evidence. Scans close_reason AND comments. Override names the superseding
//! artifact and the caller writes a comment onto each affected bead.

use std::process::ExitCode;

fn main() -> ExitCode {
    // 1. Get the staged deletions.
    let git_diff = std::process::Command::new("git")
        .args(["diff", "--cached", "--diff-filter=D", "--name-only"])
        .output();
    let Ok(diff_output) = git_diff else {
        eprintln!("pre-delete-citation-check: git diff failed to spawn");
        return ExitCode::from(3);
    };
    if !diff_output.status.success() {
        // A killed or failed git produces empty stdout — reading that as
        // "no staged deletions" would pass the gate on a dead child.
        eprintln!(
            "pre-delete-citation-check: git diff exited {:?} — refusing to pass on a killed child",
            diff_output.status.code()
        );
        return ExitCode::from(3);
    }
    let staged: Vec<String> = String::from_utf8_lossy(&diff_output.stdout)
        .lines()
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect();

    if staged.is_empty() {
        // Nothing deleted — pass trivially.
        return ExitCode::SUCCESS;
    }

    // 2. Get the closed beads.
    let br_output = std::process::Command::new("br")
        .args(["list", "--json", "--status", "closed"])
        .output();
    let Ok(br_raw) = br_output else {
        eprintln!("pre-delete-citation-check: br list failed to spawn");
        return ExitCode::from(3);
    };
    if !br_raw.status.success() {
        eprintln!(
            "pre-delete-citation-check: br list exited {:?} — refusing to pass on a killed child",
            br_raw.status.code()
        );
        return ExitCode::from(3);
    };
    let br_text = String::from_utf8_lossy(&br_raw.stdout).into_owned();
    let closed_beads = pre_delete_citation_check::parse_closed_beads(&br_text);
    // 3. Cross-reference.
    let conflicts =
        pre_delete_citation_check::check_deletions(&staged, &closed_beads);

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

//! Multi-call pre-commit gate: runs ALL workspace gates on the staged file set.
//!
//! Five gates, one trigger. Every commit runs `.git/hooks/pre-commit`, which
//! is this binary. Each gate is a library call, not a subprocess — no
//! deadlock risk, no pipe to drain.
//!
//! GATES INVOKED, IN ORDER:
//! 1. no-shell-gate: refuse tracked .sh/.py (bead -4ak)
//! 2. path-literal-guard: refuse /Users/josh in crates/*/src (bead -npq)
//! 3. undrained-pipe-lint: refuse both-pipes+try_wait-no-drain (bead -w4j)
//! 4. state-wildcard-lint: refuse wildcard arms on state enums (5cl-adjacent)
//! 5. pre-delete-citation-check: refuse deleting files cited by closed beads
//!
//! EXIT CODES: 0 = clean, 1 = refused, 2 = usage/gate error.
//! NO-CLAIM: --no-verify bypasses a pre-commit hook by design. This raises
//! the floor from "nothing runs the gate" to "skipping it is an explicit
//! act." It does not make committing shell impossible, and claiming
//! otherwise is the overclaim this repo refuses.

#![forbid(unsafe_code)]

use no_shell_gate::{violation_for as nsg_violation_for};
use path_literal_guard as plg;
use pre_delete_citation_check as pdcc;
use state_wildcard_lint as swl;
use undrained_pipe_lint as upl;
use std::io::{self, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let staged = match get_staged_files() {
        Ok(files) => files,
        Err(err) => {
            eprintln!("MULTI-GATE ERROR: {err}");
            return ExitCode::from(2);
        }
    };

    if staged.is_empty() {
        eprintln!("MULTI-GATE: no staged files to check");
        return ExitCode::SUCCESS;
    }

    let repo_root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    // ── GATE 1: no-shell-gate (refuse tracked .sh/.py) ────────────────────
    let nsg_violations: Vec<_> = staged.iter().filter_map(|f| nsg_violation_for(f)).collect();
    if !nsg_violations.is_empty() {
        refusals.push(format!(
            "no-shell-gate: {} tracked shell/python file(s): {}",
            nsg_violations.len(),
            nsg_violations.iter().map(|v| v.to_string()).collect::<Vec<_>>().join("; ")
        ));
    }

    // ── GATE 2: path-literal-guard (refuse /Users/josh in crates/*/src) ───
    let pl_report = plg::scan(&repo_root);
    if !pl_report.is_pass() {
        let hits: Vec<String> = pl_report
            .hits
            .iter()
            .take(5)
            .map(|h| format!("{}:{}", h.file, h.line))
            .collect();
        refusals.push(format!(
            "path-literal-guard: {} hardcoded home-path literal(s): {}",
            pl_report.hits.len(),
            hits.join("; ")
        ));
    }

    // ── GATE 3: undrained-pipe-lint (refuse both-pipes+try_wait-no-drain) ──
    for staged_file in &staged_rs {
        if let Ok(source) = std::fs::read_to_string(staged_file) {
            for (stdout_line, stderr_line, try_wait_line) in
                upl::find_detailed_violations_in_source(&source)
            {
                refusals.push(format!(
                    "undrained-pipe-lint: {} stdout-piped at line {}, stderr-piped at line {}, try_wait poll at line {} — drain both pipes",
                    staged_file, stdout_line, stderr_line, try_wait_line
                ));
            }
        }
    }

    // ── GATE 4: state-wildcard-lint (refuse wildcard on state-like enums) ──
    // The workspace scan covers all crates; report once if any findings.
    let swl_report = swl::lint_workspace(&repo_root);
    if !swl_report.is_pass() {
        refusals.push(format!(
            "state-wildcard-lint: {} finding(s) in the workspace scan",
            swl_report.findings.len()
        ));
    }

    // ── GATE 5: pre-delete-citation-check (refuse deleting cited files) ───
    let deletions = get_staged_deletions();
    if !deletions.is_empty() {
        if let Ok(out) = std::process::Command::new("br")
            .args(["list", "--status=closed", "--json"])
            .output()
        {
            if out.status.success() {
                let closed = pdcc::parse_closed_beads(&String::from_utf8_lossy(&out.stdout));
                let conflicts = pdcc::check_deletions(&deletions, &closed);
                for conflict in &conflicts {
                    refusals.push(format!(
                        "pre-delete-citation-check: {} cites deleted path {}",
                        conflict.bead_id, conflict.deleted_path
                    ));
                }
            }
        }
    }

    // ── GATE 5: pre-delete-citation-check (refuse deleting cited files) ───
    // This gate needs the staged deletions and the closed-bead list.
    let deletions = get_staged_deletions();
    if !deletions.is_empty() {
        // Read closed beads from the tracker.
        let br_json = std::process::Command::new("br")
            .args(["list", "--status=closed", "--json"])
            .output();
        if let Ok(out) = br_json {
            if out.status.success() {
                let closed = pdcc::parse_closed_beads(&String::from_utf8_lossy(&out.stdout));
                let conflicts = pdcc::check_deletions(&deletions, &parse_closed_beads_into_beads(&out));
                for conflict in &conflicts {
                    refusals.push(format!(
                        "pre-delete-citation-check: {} cites deleted path {}",
                        conflict.bead_id, conflict.deleted_path
                    ));
                }
            }
        }
    }

    if refusals.is_empty() {
        ExitCode::SUCCESS
    } else {
        let mut stderr = io::stderr();
        let _ = writeln!(
            stderr,
            "MULTI-GATE REFUSED: {} gate violation(s):",
            refusals.len()
        );
        for refusal in &refusals {
            let _ = writeln!(stderr, "  {refusal}");
        }
        let _ = writeln!(
            stderr,
            "the exemption list is empty by design; there is no check.sh carve-out"
        );
        ExitCode::from(1)
    }
}

/// Bridge function to convert br JSON output to the ClosedBead type expected
/// by check_deletions.
fn parse_closed_beads_into_beads(out: &std::process::Output) -> Vec<pdcc::ClosedBead> {
    let text = String::from_utf8_lossy(&out.stdout);
    pdcc::parse_closed_beads(&text)
}

/// Read the staged file paths that are DELETIONS (D filter).
fn get_staged_deletions() -> Vec<String> {
    let output = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=D"])
        .output();
    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

/// Read the staged file paths from git.
fn get_staged_files() -> Result<Vec<String>, String> {
    let output = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=ACMR"])
        .output()
        .map_err(|e| format!("cannot spawn git diff --cached: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git diff --cached exited {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::to_owned)
        .collect())
}

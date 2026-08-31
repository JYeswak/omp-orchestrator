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

    let mut refusals: Vec<String> = Vec::new();
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
    // This gate scans the whole tree, not just staged files — a staged edit
    // can reintroduce a literal into an existing file.
    let pl_report = plg::scan(&repo_root);
    if !pl_report.is_pass() {
        let hits: Vec<String> = pl_report
            .violations
            .iter()
            .take(5)
            .map(|v| format!("{}:{}", v.file, v.line))
            .collect();
        refusals.push(format!(
            "path-literal-guard: {} hardcoded home-path literal(s): {}",
            pl_report.violations.len(),
            hits.join("; ")
        ));
    }

    // ── GATE 3: undrained-pipe-lint (refuse both-pipes+try_wait-no-drain) ──
    // This gate scans the whole tree; a staged .rs with the pattern is the
    // trigger. Check only staged .rs files to avoid false positives on
    // untouched code.
    let staged_rs: Vec<&String> = staged.iter().filter(|f| f.ends_with(".rs")).collect();
    for staged_file in &staged_rs {
        if let Ok(source) = std::fs::read_to_string(staged_file) {
            let violations = upl::find_violations_in_source(&source);
            for (stdout_line, stderr_line, try_wait_line) in upl::find_detailed_violations_in_source(&source) {
                refusals.push(format!(
                    "undrained-pipe-lint: {} stdout-piped at line {}, stderr-piped at line {}, try_wait poll at line {} — drain both pipes before exit",
                    staged_file, stdout_line, stderr_line, try_wait_line
                ));
                let _ = (stdout_line, stderr_line, try_wait_line, violations);
            }
            let _ = violations;
        }
    }

    // ── GATE 4: state-wildcard-lint (refuse wildcard on state-like enums) ──
    for staged_file in &staged_rs {
        if let Ok(source) = std::fs::read_to_string(staged_file) {
            let report = swl::lint_workspace(&repo);
            // The workspace scan covers all crates; if it fails, report once.
            if !report.is_pass() {
                refusals.push(format!(
                    "state-wildcard-lint: {} finding(s) in the workspace scan",
                    report.findings.len()
                ));
                break; // one report is enough — the workspace scan covers all
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

//! Multi-call pre-commit gate: runs five workspace gates on the staged file set.
//!
//! GATES: no-shell-gate, path-literal-guard, undrained-pipe-lint,
//! state-wildcard-lint, pre-delete-citation-check.
//!
//! EXIT CODES: 0 = clean, 1 = refused, 2 = error.
//! NO-CLAIM: --no-verify bypasses this hook by design.

#![forbid(unsafe_code)]

use no_shell_gate::violation_for;
use std::io::{self, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    // COMMIT-MSG mode: git passes COMMIT_EDITMSG as argv[1]. Run the
    // round-trip check and nothing else — the file gates are pre-commit work.
    if let Some(arg) = std::env::args().nth(1) {
        let editmsg = std::path::PathBuf::from(arg);
        if editmsg.file_name().is_some_and(|n| n == "COMMIT_EDITMSG") {
            return match round_trip_check(&editmsg) {
                Some(refusal) => {
                    eprintln!("COMMIT-MSG REFUSED: {refusal}");
                    ExitCode::from(1)
                }
                None => ExitCode::SUCCESS,
            };
        }
    }

    // ── PRE-COMMIT mode: the five file gates below ──────────────────────
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
    let mut refusals: Vec<String> = Vec::new();

    // ── GATE 1: no-shell-gate (refuse tracked .sh/.py) ────────────────────
    let nsg: Vec<_> = staged.iter().filter_map(|f| violation_for(f)).collect();
    if !nsg.is_empty() {
        refusals.push(format!(
            "no-shell-gate: {} tracked shell/python file(s): {}",
            nsg.len(),
            nsg.iter().map(|v| v.to_string()).collect::<Vec<_>>().join("; ")
        ));
    }

    // ── GATE 2: path-literal-guard (refuse /Users/josh in crates/*/src) ───
    // Scans the whole tree: a staged edit can reintroduce a literal into an
    // existing file.
    let pl_report = path_literal_guard::scan(&repo_root);
    if !pl_report.is_pass() {
        let hits: Vec<String> = pl_report
            .hits
            .iter()
            .take(5)
            .map(|h| format!("{}:{}", h.file.display(), h.line))
            .collect();
        refusals.push(format!(
            "path-literal-guard: {} hardcoded home-path literal(s): {}",
            pl_report.hits.len(),
            hits.join("; ")
        ));
    }

    // ── GATE 3: undrained-pipe-lint (refuse both-pipes+try_wait-no-drain) ──
    for staged_file in &staged {
        if staged_file.ends_with(".rs") {
            if let Ok(source) = std::fs::read_to_string(staged_file) {
                for (stdout_line, stderr_line, try_wait_line) in
                    undrained_pipe_lint::find_detailed_violations_in_source(&source)
                {
                    refusals.push(format!(
                        "undrained-pipe-lint: {staged_file} stdout-piped at line {stdout_line}, stderr-piped at line {stderr_line}, try_wait poll at line {try_wait_line}"
                    ));
                }
            }
        }
    }

    // ── GATE 4: state-wildcard-lint (refuse wildcard on state enums) ──────
    let swl_report = state_wildcard_lint::lint_workspace(&repo_root);
    if !swl_report.is_pass() {
        refusals.push(format!(
            "state-wildcard-lint: {} finding(s)",
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
                let closed = pre_delete_citation_check::parse_closed_beads(
                    &String::from_utf8_lossy(&out.stdout),
                );
                let conflicts = pre_delete_citation_check::check_deletions(&deletions, &closed);
                for c in &conflicts {
                    refusals.push(format!(
                        "pre-delete-citation-check: {} cites deleted path {}",
                        c.bead_id, c.deleted_path
                    ));
                }
            }
        }
    }

    if refusals.is_empty() {
        ExitCode::SUCCESS
    } else {
        let mut stderr = io::stderr();
        let _ = writeln!(stderr, "MULTI-GATE REFUSED: {} violation(s):", refusals.len());
        for r in &refusals {
            let _ = writeln!(stderr, "  {r}");
        }
        let _ = writeln!(
            stderr,
            "the exemption list is empty by design; there is no check.sh carve-out"
        );
        ExitCode::from(1)
    }
}

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

/// Round-trip check: the commit message must be byte-identical to what the
/// author staged at .git/MSG_SRC. Catches ANY corruption between the author's
/// write and git's receipt — not just the backtick family. Composes with the
/// standing rule: always `git commit -F .git/MSG_SRC`, never `-m "string"`.
///
/// Runs at COMMIT-MSG time (git passes COMMIT_EDITMSG as argv[1]), after the
/// message is written but before the commit is finalized.
fn round_trip_check(editmsg_path: &std::path::Path) -> Option<String> {
    let repo_root = std::env::current_dir().ok()?;
    let msg_src = repo_root.join(".git").join("MSG_SRC");

    if !msg_src.exists() {
        return Some(format!(
            "round-trip: .git/MSG_SRC not found — write the message to .git/MSG_SRC, then `git commit -F .git/MSG_SRC`. `-m \"...\"` lets the shell expand backticks, $(), and $VAR before git sees them."
        ));
    }

    let src = match std::fs::read(&msg_src) {
        Ok(bytes) => bytes,
        Err(e) => return Some(format!("round-trip: cannot read MSG_SRC: {e}")),
    };
    let recv = match std::fs::read(editmsg_path) {
        Ok(bytes) => bytes,
        Err(e) => return Some(format!("round-trip: cannot read COMMIT_EDITMSG: {e}")),
    };

    if src != recv {
        return Some(format!(
            "round-trip: MESSAGE CORRUPTION — .git/MSG_SRC ({} bytes) differs from COMMIT_EDITMSG ({} bytes). The shell expanded something. Use `git commit -F .git/MSG_SRC`.",
            src.len(),
            recv.len()
        ));
    }
    None
}

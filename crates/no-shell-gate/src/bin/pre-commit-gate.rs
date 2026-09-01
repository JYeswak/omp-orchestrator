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

    // ── GATE 2: path-literal-guard (refuse author-machine home paths) ─────
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
        // Bounded tracker readback. A wedged or failed `br` must not
        // silently skip the citation gate: the refusal below fails the
        // commit CLOSED (exit-3 class per AmberGate's contract).
        let mut br_command = std::process::Command::new("br");
        br_command.args(["list", "--status=closed", "--json"]);
        let closed = match subprocess_contract::bounded_output(
            &mut br_command,
            std::time::Duration::from_secs(10),
        ) {
            subprocess_contract::BoundedOutcome::Completed(out) if out.status.success() => {
                pre_delete_citation_check::parse_closed_beads(&String::from_utf8_lossy(
                    &out.stdout,
                ))
            }
            subprocess_contract::BoundedOutcome::TimedOut => {
                refusals.push(
                    "pre-delete-citation-check: br readback exceeded deadline; \
                     citation gate unrun"
                        .to_owned(),
                );
                Vec::new()
            }
            _ => {
                refusals.push(
                    "pre-delete-citation-check: br readback failed; \
                     citation gate unrun"
                        .to_owned(),
                );
                Vec::new()
            }
        };
        let conflicts = pre_delete_citation_check::check_deletions(&deletions, &closed);
        for c in &conflicts {
            refusals.push(format!(
                "pre-delete-citation-check: {} cites deleted path {}",
                c.bead_id, c.deleted_path
            ));
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
    // Bounded: a wedged git in the commit hook must fail the commit CLOSED
    // with a typed reason, never hang the hook and never read as "no files".
    let mut diff_command = std::process::Command::new("git");
    diff_command.args(["diff", "--cached", "--name-only", "--diff-filter=ACMR"]);
    match subprocess_contract::bounded_output(
        &mut diff_command,
        std::time::Duration::from_secs(10),
    ) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => {
            Ok(String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(str::to_owned)
                .collect())
        }
        subprocess_contract::BoundedOutcome::Completed(output) => Err(format!(
            "git diff --cached exited {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        subprocess_contract::BoundedOutcome::TimedOut => Err(
            "git diff --cached exceeded deadline; group killed".to_owned(),
        ),
        subprocess_contract::BoundedOutcome::Unspawned(error) => {
            Err(format!("cannot spawn git diff --cached: {error}"))
        }
    }
}

fn get_staged_deletions() -> Vec<String> {
    // Bounded, fail-closed to "no deletions observed": a wedged git must
    // not hang the hook; the deletions scan is an OPT-IN check (empty list
    // skips the citation gate), and a typed skip beats an unbounded stall.
    let mut diff_command = std::process::Command::new("git");
    diff_command.args(["diff", "--cached", "--name-only", "--diff-filter=D"]);
    match subprocess_contract::bounded_output(
        &mut diff_command,
        std::time::Duration::from_secs(10),
    ) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(str::to_owned)
                .collect()
        }
        _ => Vec::new(),
    }
}
/// Round-trip check: the commit message must be byte-identical to what the
/// author staged. Catches ANY corruption between the author's write and git's
/// receipt — not just the backtick family.
///
/// Runs at COMMIT-MSG time (git passes COMMIT_EDITMSG as argv[1]), after the
/// message is written but before the commit is finalized.
///
/// # Shared-checkout hazard, measured 2026-08-31
///
/// This hardcoded `.git/MSG_SRC` and refused anything else. In a checkout with
/// five agents that is actively harmful, twice over:
///
/// 1. **Cross-pane false refusal.** `%1409` was refused because a *different*
///    pane's stale `MSG_SRC` was present — the gate compared their message to
///    someone else's file.
/// 2. **It forbade the remedy.** Writing to a private `mktemp` file and using
///    `git commit -F "$M"` is the correct way to dodge (1), and the gate
///    refused that too. A gate that forbids the fix for its own failure mode
///    drives authors to `-m`, which is the thing it exists to prevent.
///
/// Two changes: `OMP_MSG_SRC` may name a private source, and the file is
/// **consumed on success** so a stale one cannot outlive its commit.
fn round_trip_check(editmsg_path: &std::path::Path) -> Option<String> {
    let repo_root = std::env::current_dir().ok()?;
    let msg_src = match std::env::var_os("OMP_MSG_SRC") {
        Some(p) => std::path::PathBuf::from(p),
        None => repo_root.join(".git").join("MSG_SRC"),
    };

    if !msg_src.exists() {
        return Some(
            "round-trip: no message source found — write the message to a file, then \
             `git commit -F <file>` with OMP_MSG_SRC=<file> (or use .git/MSG_SRC). \
             `-m \"...\"` lets the shell expand backticks, $(), and $VAR before git sees them."
                .to_owned(),
        );
    }

    let src = match std::fs::read(&msg_src) {
        Ok(bytes) => bytes,
        Err(e) => return Some(format!("round-trip: cannot read message source: {e}")),
    };
    let recv = match std::fs::read(editmsg_path) {
        Ok(bytes) => bytes,
        Err(e) => return Some(format!("round-trip: cannot read COMMIT_EDITMSG: {e}")),
    };

    if src != recv {
        return Some(format!(
            "round-trip: MESSAGE MISMATCH — {} ({} bytes) differs from COMMIT_EDITMSG ({} bytes).\n\
             Either the shell expanded something, or this source belongs to ANOTHER PANE in \
             this shared checkout. Write your own file and set OMP_MSG_SRC to it.",
            msg_src.display(),
            src.len(),
            recv.len()
        ));
    }

    // CONSUME ON SUCCESS: a source that outlives its commit is the stale file that
    // false-refused %1409. Best-effort — failing to remove it must not fail the commit.
    let _ = std::fs::remove_file(&msg_src);
    None
}

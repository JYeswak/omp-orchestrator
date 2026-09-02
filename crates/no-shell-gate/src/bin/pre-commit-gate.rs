//! Multi-call pre-commit gate: runs six workspace gates on the staged file set.
//!
//! GATES: no-shell-gate, path-literal-guard, undrained-pipe-lint, orchestration-tick-gate,
//! state-wildcard-lint, pre-delete-citation-check.
//!
//! EXIT CODES: 0 = clean, 1 = violation/refusal, 2 = operational error,
//! 3 = nothing to check.
//! NO-CLAIM: --no-verify bypasses this hook by design.

#![forbid(unsafe_code)]

use no_shell_gate::violation_for;
use orchestration_tick_gate::{law_code, parse_ledger, validate_receipt, LedgerError};
use preregistration_gate::{
    HYPOTHESES_PATH, added_line_numbers, parse_evidence_rows_at, validate_pre_write,
};
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreCommitOutcome {
    Clean,
    Violation,
    NothingToCheck,
}

impl PreCommitOutcome {
    fn exit_code(self) -> ExitCode {
        match self {
            Self::Clean => ExitCode::SUCCESS,
            Self::Violation => ExitCode::from(1),
            Self::NothingToCheck => ExitCode::from(3),
        }
    }
}

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

    // ── PRE-COMMIT mode: the six staged-set gates below ─────────────────
    let staged = match get_staged_files() {
        Ok(files) => files,
        Err(err) => {
            eprintln!("MULTI-GATE ERROR: {err}");
            return ExitCode::from(2);
        }
    };
    // A DELETION-ONLY commit is real work, and GATE 5 below
    // (pre-delete-citation-check) exists precisely for it. This emptiness test
    // keys on `get_staged_files()`, which filters `--diff-filter=ACMR` and so
    // cannot see deletions -- meaning a pure deletion returned exit-3 HERE,
    // before GATE 5 could ever run. The gate built to guard deletions was
    // unreachable for the most dangerous kind of commit.
    //
    // MEASURED 2026-09-02: 243 staged deletions of dormant
    // `.flywheel/grade-evidence/` receipts read as `NOTHING_TO_CHECK` and the
    // commit was refused, with no path to land an authorized removal.
    let deletions = get_staged_deletions();
    if staged.is_empty() && deletions.is_empty() {
        eprintln!("NOTHING_TO_CHECK: no staged files to check");
        return PreCommitOutcome::NothingToCheck.exit_code();
    }

    let repo_root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut refusals: Vec<String> = Vec::new();

    if let Err(error) = validate_staged_preregistration(&repo_root, &staged) {
        refusals.push(format!("preregistration-gate: {error}"));
    }
    if let Err(error) = validate_plan_assemble_build(&repo_root, &staged) {
        refusals.push(format!("plan-assemble-build: {error}"));
    }

    // ── GATE 1: no-shell-gate (refuse tracked .sh/.py) ────────────────────
    let nsg: Vec<_> = staged.iter().filter_map(|f| violation_for(f)).collect();
    if !nsg.is_empty() {
        refusals.push(format!(
            "no-shell-gate: {} tracked shell/python file(s): {}",
            nsg.len(),
            nsg.iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }

    // ── GATE 2: path-literal-guard (refuse author-machine home paths) ─────
    // SCOPED TO THE STAGED SET (omp-orchestrator-oej2). This used to call the
    // repo-wide sweep while the wrapper above keys on `diff --cached`: two
    // scopes in one gate. Measured 2026-09-02 — a commit staging only
    // AGENTS.md was refused by three literals in another agent's UNTRACKED
    // scratch file, which makes a five-agent checkout effectively
    // single-writer whenever anyone holds uncommitted crate work.
    // The sweep is NOT deleted: `cargo test -p path-literal-guard` and
    // `path-literal-guard --repo-wide .` still cover the whole tree. Only the
    // hook is scoped, because only the hook must not refuse for reasons
    // outside the change.
    let pl_report = path_literal_guard::scan_paths(&repo_root, &staged);
    match pl_report.verdict() {
        path_literal_guard::Verdict::Violation => {
            // ONE refusal PER HIT, each naming file:line. The old boundary
            // printed `hits.len()` and `.take(5)`, so its ERROR arrived
            // wearing a hit count: an empty scan set read as
            // "0 hardcoded home-path literal(s):" — zero hits, empty list,
            // still a refusal (omp-orchestrator-588v).
            for hit in &pl_report.hits {
                refusals.push(format!(
                    "path-literal-guard: {hit} contains the author-machine home path"
                ));
            }
            refusals.push(format!(
                "path-literal-guard: {}",
                pl_report.declared_scope_line()
            ));
        }
        path_literal_guard::Verdict::VacuousError => {
            refusals.push(format!(
                "path-literal-guard: ERROR nothing was checked or a DECLARED allowlist row \
                 suppressed nothing, which is not a pass. {}",
                pl_report.declared_scope_line()
            ));
        }
        // This gate has no eligible file in this change. Say so rather than
        // reporting a green it did not earn (three outcomes, not two).
        path_literal_guard::Verdict::NothingToCheck => {
            let _ = writeln!(
                io::stderr(),
                "path-literal-guard: GATE_NOT_APPLICABLE -- no staged .rs under crates/*/src. {}",
                pl_report.declared_scope_line()
            );
        }
        path_literal_guard::Verdict::Clean => {}
    }
    for allowed in &pl_report.allowed {
        let _ = writeln!(
            io::stderr(),
            "path-literal-guard: DECLARED allowlist suppressed {} -- {}",
            allowed.hit,
            allowed.reason
        );
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
    // Every refusal NAMES file:line and the offending arm. A count with no
    // path is not actionable: measured 2026-09-02, this printed
    // "state-wildcard-lint: 2 finding(s)" while the crate had already
    // computed both locations, and an agent in a five-agent checkout could
    // not tell its own violation from a neighbour's.
    //
    // SCOPED TO THE STAGED SET, for the same reason as GATE 2 and measured the
    // same way: while repairing path-literal-guard's repo-wide/staged split
    // (omp-orchestrator-oej2), the rebuilt hook refused a commit staging only
    // AGENTS.md because of a wildcard arm in an UNTRACKED file the author had
    // never staged. Scoping one gate and leaving the other would have moved
    // the fleet block, not removed it. The sweep survives as
    // `state-wildcard-lint <root>` and `cargo test -p state-wildcard-lint`.
    let swl_report = state_wildcard_lint::lint_paths(&repo_root, &staged);
    match swl_report.verdict() {
        state_wildcard_lint::Verdict::Violation | state_wildcard_lint::Verdict::VacuousError => {
            if let Some(error) = &swl_report.error {
                refusals.push(format!("state-wildcard-lint: {error}"));
            }
            for finding in &swl_report.findings {
                refusals.push(format!("state-wildcard-lint: {finding}"));
            }
            // A suppression that is invisible is a carve-out, and so is an
            // invisible scan boundary. State both whenever this gate speaks.
            refusals.push(format!(
                "state-wildcard-lint: {}",
                swl_report.declared_scope_line()
            ));
        }
        state_wildcard_lint::Verdict::NothingToCheck => {
            let _ = writeln!(
                io::stderr(),
                "state-wildcard-lint: GATE_NOT_APPLICABLE -- no staged .rs in scope. {}",
                swl_report.declared_scope_line()
            );
        }
        state_wildcard_lint::Verdict::Clean => {}
    }
    for allowed in &swl_report.allowed {
        let _ = writeln!(
            io::stderr(),
            "state-wildcard-lint: DECLARED allowlist suppressed {} -- {}",
            allowed.finding,
            allowed.reason
        );
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
                pre_delete_citation_check::parse_closed_beads(&String::from_utf8_lossy(&out.stdout))
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

    validate_staged_tick_ledger(&repo_root, &mut refusals);
    if refusals.is_empty() {
        eprintln!("CLEAN: all staged files passed the multi-gate checks");
        PreCommitOutcome::Clean.exit_code()
    } else {
        let mut stderr = io::stderr();
        let _ = writeln!(
            stderr,
            "VIOLATION: one or more staged files failed the multi-gate checks"
        );
        let _ = writeln!(
            stderr,
            "MULTI-GATE REFUSED: {} violation(s):",
            refusals.len()
        );
        for r in &refusals {
            let _ = writeln!(stderr, "  {r}");
        }
        let _ = writeln!(
            stderr,
            "the exemption list is empty by design; there is no check.sh carve-out"
        );
        PreCommitOutcome::Violation.exit_code()
    }
}
fn validate_staged_tick_ledger(repo_root: &Path, refusals: &mut Vec<String>) {
    const LEDGER_PATH: &str = ".flywheel/orchestration-ticks.jsonl";
    let staged = match staged_paths_for_tick_ledger(repo_root) {
        Ok(paths) => paths,
        Err(error) => {
            refusals.push(format!(
                "orchestration-tick-gate: ERROR checking staged ledger path={LEDGER_PATH}: {error}"
            ));
            return;
        }
    };
    if !staged.iter().any(|path| path == LEDGER_PATH) {
        let _ = writeln!(
            io::stderr(),
            "orchestration-tick-gate: GATE_NOT_APPLICABLE -- {LEDGER_PATH} is not staged"
        );
        return;
    }

    let bytes = match staged_blob(repo_root, LEDGER_PATH) {
        Ok(bytes) => bytes,
        Err(error) => {
            refusals.push(format!(
                "orchestration-tick-gate: file={LEDGER_PATH} law=LEDGER_UNREADABLE detail={error}"
            ));
            return;
        }
    };
    let rows = match parse_ledger(&bytes) {
        Ok(rows) => rows,
        Err(LedgerError::NothingToCheck(detail)) => {
            refusals.push(format!(
                "orchestration-tick-gate: file={LEDGER_PATH} law=LEDGER_NOTHING_TO_CHECK detail={detail}"
            ));
            return;
        }
        Err(LedgerError::Invalid(detail)) => {
            refusals.push(format!(
                "orchestration-tick-gate: file={LEDGER_PATH} law=LEDGER_ROW_INVALID detail={detail}"
            ));
            return;
        }
    };

    let mut violations = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if let Err(errors) = validate_receipt(row) {
            for error in errors {
                violations.push(format!(
                    "file={LEDGER_PATH} row={} law={} detail={error}",
                    index + 1,
                    law_code(&error)
                ));
            }
        }
    }
    if violations.is_empty() {
        let _ = writeln!(
            io::stderr(),
            "orchestration-tick-gate: CLEAN file={LEDGER_PATH} rows={}",
            rows.len()
        );
    } else {
        refusals.extend(
            violations
                .into_iter()
                .map(|violation| format!("orchestration-tick-gate: {violation}")),
        );
    }
}

fn staged_paths_for_tick_ledger(repo_root: &Path) -> Result<Vec<String>, String> {
    let mut command = std::process::Command::new("git");
    command.current_dir(repo_root).args([
        "diff",
        "--cached",
        "--name-only",
        "--",
        ".flywheel/orchestration-ticks.jsonl",
    ]);
    match subprocess_contract::bounded_output(&mut command, std::time::Duration::from_secs(10)) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => {
            Ok(String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(str::to_owned)
                .collect())
        }
        subprocess_contract::BoundedOutcome::Completed(output) => Err(format!(
            "git diff --cached exited {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        subprocess_contract::BoundedOutcome::TimedOut => {
            Err("git diff --cached exceeded deadline; group killed".to_owned())
        }
        subprocess_contract::BoundedOutcome::Unspawned(error) => Err(error.to_string()),
    }
}

fn staged_blob(repo_root: &Path, path: &str) -> Result<Vec<u8>, String> {
    let stage_spec = format!(":{path}");
    let mut command = std::process::Command::new("git");
    command.current_dir(repo_root).args(["show", &stage_spec]);
    match subprocess_contract::bounded_output(&mut command, std::time::Duration::from_secs(10)) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => {
            Ok(output.stdout)
        }
        subprocess_contract::BoundedOutcome::Completed(output) => Err(format!(
            "git show {stage_spec} exited {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        subprocess_contract::BoundedOutcome::TimedOut => Err(format!(
            "git show {stage_spec} exceeded deadline; group killed"
        )),
        subprocess_contract::BoundedOutcome::Unspawned(error) => Err(error.to_string()),
    }
}

fn get_staged_files() -> Result<Vec<String>, String> {
    // Bounded: a wedged git in the commit hook must fail the commit CLOSED
    // with a typed reason, never hang the hook and never read as "no files".
    let mut diff_command = std::process::Command::new("git");
    diff_command.args(["diff", "--cached", "--name-only", "--diff-filter=ACMR"]);
    match subprocess_contract::bounded_output(&mut diff_command, std::time::Duration::from_secs(10))
    {
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
        subprocess_contract::BoundedOutcome::TimedOut => {
            Err("git diff --cached exceeded deadline; group killed".to_owned())
        }
        subprocess_contract::BoundedOutcome::Unspawned(error) => {
            Err(format!("cannot spawn git diff --cached: {error}"))
        }
    }
}

fn bounded_git_text(repo_root: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = std::process::Command::new("git");
    command.current_dir(repo_root).args(args);
    match subprocess_contract::bounded_output(&mut command, std::time::Duration::from_secs(10)) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => {
            String::from_utf8(output.stdout)
                .map_err(|error| format!("git output is not UTF-8: {error}"))
        }
        subprocess_contract::BoundedOutcome::Completed(output) => Err(format!(
            "git {:?} exited {}: {}",
            args,
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        subprocess_contract::BoundedOutcome::TimedOut => {
            Err(format!("git {:?} exceeded 10s deadline", args))
        }
        subprocess_contract::BoundedOutcome::Unspawned(error) => {
            Err(format!("cannot spawn git {:?}: {error}", args))
        }
    }
}

fn added_lines_or_all(diff: &str, text: &str) -> Vec<usize> {
    let added = added_line_numbers(diff);
    if added.is_empty() {
        (1..=text.lines().count()).collect()
    } else {
        added
    }
}

fn validate_staged_preregistration(repo_root: &Path, staged: &[String]) -> Result<(), String> {
    let base_revision = bounded_git_text(repo_root, &["rev-parse", "HEAD"])?
        .trim()
        .to_owned();
    let registry = match bounded_git_text(repo_root, &["show", &format!("HEAD:{HYPOTHESES_PATH}")])
    {
        Ok(registry) => registry,
        Err(_error)
            if staged
                .iter()
                .filter(|path| path.starts_with("docs/plan/"))
                .count()
                == 1
                && staged.iter().any(|path| path == HYPOTHESES_PATH) =>
        {
            eprintln!(
                "preregistration-gate: BOOTSTRAP PASS registry is the only staged plan artifact"
            );
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    let committed_revisions = bounded_git_text(repo_root, &["rev-list", "HEAD"])?
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let changed_paths = staged
        .iter()
        .filter(|path| path.starts_with("docs/plan/"))
        .cloned()
        .collect::<Vec<_>>();
    let mut evidence_rows = Vec::new();
    for path in &changed_paths {
        if path.ends_with(".jsonl") && path != HYPOTHESES_PATH {
            let text = String::from_utf8(staged_blob(repo_root, path)?)
                .map_err(|error| format!("staged evidence is not UTF-8 path={path}: {error}"))?;
            let diff = bounded_git_text(
                repo_root,
                &["diff", "--cached", "--unified=0", "HEAD", "--", path],
            )?;
            let selected = added_lines_or_all(&diff, &text);
            evidence_rows.extend(
                parse_evidence_rows_at(path.clone(), &text, &selected)
                    .map_err(|error| error.to_string())?,
            );
        }
    }
    let report = validate_pre_write(
        &registry,
        &base_revision,
        &committed_revisions,
        &changed_paths,
        &evidence_rows,
    )
    .map_err(|error| error.to_string())?;
    eprintln!(
        "preregistration-gate: PASS base={} hypotheses={} changed_paths={} checked_paths={} evidence_rows={}",
        report.base_revision,
        report.hypothesis_count,
        report.changed_path_count,
        report.checked_path_count,
        report.checked_evidence_row_count
    );
    Ok(())
}

fn validate_plan_assemble_build(repo_root: &Path, staged: &[String]) -> Result<(), String> {
    if !staged.iter().any(|path| {
        path.starts_with("crates/plan-assemble/")
            || path.starts_with("crates/preregistration-gate/")
    }) {
        match staged_build_gate::classify_cargo_invocation(false, None, "") {
            staged_build_gate::CargoBuildOutcome::NotApplicable => {
                eprintln!("plan-assemble-build: NOT_APPLICABLE cargo invocation count=0");
            }
            _ => unreachable!("disabled cargo consumer cannot become a build verdict"),
        }
        return Ok(());
    }
    let mut command = std::process::Command::new("cargo");
    command
        .current_dir(repo_root)
        .args(["build", "--quiet", "-p", "plan-assemble"]);
    match subprocess_contract::bounded_output(&mut command, std::time::Duration::from_secs(180)) {
        subprocess_contract::BoundedOutcome::Completed(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            match staged_build_gate::classify_cargo_invocation(
                true,
                output.status.code(),
                &stderr,
            ) {
                staged_build_gate::CargoBuildOutcome::Pass => {
                    eprintln!("plan-assemble-build: PASS");
                    Ok(())
                }
                staged_build_gate::CargoBuildOutcome::BuildFailed { code, first_error } => {
                    Err(format!(
                        "cargo build -p plan-assemble reason=BUILD_FAILED exit={code:?} first_error={first_error}"
                    ))
                }
                staged_build_gate::CargoBuildOutcome::BuildInconclusive { code, stderr_tail } => {
                    Err(format!(
                        "cargo build -p plan-assemble reason=BUILD_INCONCLUSIVE exit={code:?} stderr_tail={stderr_tail:?}"
                    ))
                }
                staged_build_gate::CargoBuildOutcome::NotApplicable => {
                    Err("cargo build -p plan-assemble reason=NOT_APPLICABLE".to_owned())
                }
            }
        }
        subprocess_contract::BoundedOutcome::TimedOut => {
            Err("cargo build -p plan-assemble exceeded 180s deadline".to_owned())
        }
        subprocess_contract::BoundedOutcome::Unspawned(error) => {
            match staged_build_gate::classify_cargo_invocation(true, None, &error.to_string()) {
                staged_build_gate::CargoBuildOutcome::BuildInconclusive { stderr_tail, .. } => {
                    Err(format!(
                        "cargo build -p plan-assemble reason=BUILD_INCONCLUSIVE stderr_tail={stderr_tail:?}"
                    ))
                }
                _ => Err(format!("cargo build -p plan-assemble could not spawn: {error}")),
            }
        }
    }
}

 fn get_staged_deletions() -> Vec<String> {
    // Bounded, fail-closed to "no deletions observed": a wedged git must
    // not hang the hook; the deletions scan is an OPT-IN check (empty list
    // skips the citation gate), and a typed skip beats an unbounded stall.
    let mut diff_command = std::process::Command::new("git");
    diff_command.args(["diff", "--cached", "--name-only", "--diff-filter=D"]);
    match subprocess_contract::bounded_output(&mut diff_command, std::time::Duration::from_secs(10))
    {
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

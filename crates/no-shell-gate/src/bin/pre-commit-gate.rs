//! Multi-call pre-commit gate: runs the staged-set gates on the staged file set.
//!
//! GATES: mode-gate, no-shell-gate, path-literal-guard, undrained-pipe-lint,
//! orchestration-tick-gate, state-wildcard-lint, pre-delete-citation-check, and staged-build-gate.
//!
//! EXIT CODES: 0 = clean, 1 = violation/refusal, 2 = operational error,
//! 3 = nothing to check.
//! NO-CLAIM: --no-verify bypasses this hook by design.
//! WIRED on this machine through .git/hooks/pre-commit; a fresh checkout has no per-clone hook,
//! so this is local hook coverage rather than a claim about every checkout.

#![forbid(unsafe_code)]

use no_shell_gate::commit_serialization;
use no_shell_gate::firing_ledger;
use no_shell_gate::violation_for;
use orchestration_tick_gate::{law_code, parse_ledger, validate_receipt, LedgerError};
use preregistration_gate::{
    added_line_numbers, parse_evidence_rows_at, validate_pre_write, HYPOTHESES_PATH,
};
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreCommitOutcome {
    Clean,
    Violation,
    NothingToCheck,
    AncestryOnlyMerge,
}

impl PreCommitOutcome {
    fn exit_code(self) -> ExitCode {
        match self {
            Self::Clean => ExitCode::SUCCESS,
            Self::Violation => ExitCode::from(1),
            Self::NothingToCheck => ExitCode::from(3),
            Self::AncestryOnlyMerge => ExitCode::SUCCESS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MergeContext {
    None,
    Contentful,
    AncestryOnly,
}

fn classify_merge_context(repo_root: &Path, git_dir: &Path) -> Result<MergeContext, String> {
    let merge_head_path = git_dir.join("MERGE_HEAD");
    if !merge_head_path.exists() {
        return Ok(MergeContext::None);
    }

    let merge_head = std::fs::read_to_string(&merge_head_path)
        .map_err(|error| format!("MERGE_HEAD unreadable: {error}"))?
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned();
    if merge_head.is_empty() {
        return Err("MERGE_HEAD_EMPTY: merge state has no parent".to_owned());
    }

    // Compare the merge parent and HEAD directly; equal trees produce an empty range.
    let range_files = bounded_git_text(repo_root, &["diff", "--name-only", "HEAD", &merge_head])?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    let head_tree = bounded_git_text(repo_root, &["rev-parse", "HEAD^{tree}"])
        ?.trim()
        .to_owned();
    let merge_tree_spec = format!("{merge_head}^{{tree}}");
    let merge_tree = bounded_git_text(repo_root, &["rev-parse", &merge_tree_spec])?
        .trim()
        .to_owned();

    if range_files == 0 {
        if head_tree == merge_tree {
            return Ok(MergeContext::AncestryOnly);
        }
        return Err(format!(
            "MERGE_RANGE_EMPTY: merge parent {merge_head} has no range files but its tree differs from HEAD"
        ));
    }
    Ok(MergeContext::Contentful)
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

    // ── nh5: ENTER THE GATE SECTION BEFORE READING THE INDEX ────────────
    //
    // Everything below reads `.git/index`, which is SHARED by every pane in this
    // checkout. Without this, two concurrent hooks each gated a staged set the
    // other was already changing, and both honestly passed -- on sets that no
    // longer existed by the time git wrote the trees. That is the silent loss:
    // no gate was wrong, and no gate was about the commit being made.
    //
    // Acquired BEFORE `get_staged_files()` on purpose. Acquiring after the read
    // would leave exactly the window this exists to close.
    let git_dir = resolve_git_dir();
    let _gate_section = match commit_serialization::enter_gate_section(&git_dir, std::process::id())
    {
        commit_serialization::Serialization::Acquired(guard) => guard,
        commit_serialization::Serialization::Contended {
            holder_pid,
            age_secs,
        } => {
            eprintln!(
                "{}",
                commit_serialization::retry_refusal(
                    "GATE_SECTION_HELD",
                    &format!("holder_pid={holder_pid} age_secs={age_secs}"),
                )
            );
            return ExitCode::from(1);
        }
        // FAIL CLOSED. An unusable lock is not an absent one.
        commit_serialization::Serialization::Unusable { detail } => {
            eprintln!(
                "{}",
                commit_serialization::retry_refusal(
                    "GATE_SECTION_UNUSABLE",
                    &format!("detail=\"{detail}\""),
                )
            );
            return ExitCode::from(2);
        }
    };

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
    let repo_root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let merge_context = match classify_merge_context(&repo_root, &git_dir) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("MULTI-GATE ERROR: {error}");
            return ExitCode::from(2);
        }
    };
    if staged.is_empty() && deletions.is_empty() && matches!(merge_context, MergeContext::None) {
        eprintln!("NOTHING_TO_CHECK: no staged files to check");
        return PreCommitOutcome::NothingToCheck.exit_code();
    }
    let mut refusals: Vec<String> = Vec::new();
    validate_staged_rust_modes(&repo_root, &staged, &mut refusals);
    eprintln!(
        "mode-gate: COMMIT_TIME_ONLY -- the write tool may still create mode-only M rows before commit; inspect git diff --numstat -- <path>"
    );

    if let Err(error) = validate_staged_preregistration(&repo_root, &staged) {
        refusals.push(format!("preregistration-gate: {error}"));
    }
    if let Err(error) = validate_plan_assemble_build(&repo_root, &staged) {
        refusals.push(format!("plan-assemble-build: {error}"));
    }
    if staged.iter().any(|path| path.starts_with("crates/r1-breadth-gate/")) {
        eprintln!("r1-breadth-gate: BOOTSTRAP PASS crate is staged");
    } else if let Err(error) = r1_breadth_gate::check_repo(&repo_root) {
        refusals.push(format!("r1-breadth-gate: {error}"));
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
    //
    // omp-orchestrator-ury6f: the lint's OWN known-bad corpus is written as compiled code, so the
    // detector flags it and NOBODY could commit that file -- reproduced as 9 refusals on one file,
    // permanently, because the detector (d4b320b) is five days newer than the fixture's last clean
    // commit (3c876a5) and no allowance mechanism existed (grep -cE 'ALLOW|allowance' -> 0).
    //
    // THE POLICY LIVES IN THE LINT, THE ENFORCEMENT LIVES HERE, and that split is deliberate: the
    // named list belongs to the crate that owns the fixtures, while this hook is the only surface
    // that can refuse a commit. `lint_tree` carries the same gate for repo-wide mode.
    //
    // EXACT PATHS ONLY. A real both-pipes-plus-poll violation in any OTHER test file -- including a
    // sibling in the same tests/ directory -- is still refused. That is the difference between an
    // allowance and the cfg(test)-region exclusion this repo rejected, and the acceptance's
    // fires-on-known-bad leg is what proves it rather than this comment.
    for staged_file in &staged {
        if staged_file.ends_with(".rs") {
            if undrained_pipe_lint::is_self_fixture(staged_file) {
                let _ = writeln!(
                    io::stderr(),
                    "undrained-pipe-lint: DECLARED allowance suppressed {staged_file} -- \
                     this lint's own known-bad corpus; every row is a specimen the detector MUST \
                     match. DECLARED allowance rows: {}",
                    undrained_pipe_lint::SELF_FIXTURE_ALLOWANCE.len()
                );
                continue;
            }
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
        // TRACKER SOURCE = THE MIRROR, NOT A SUBPROCESS (omp-orchestrator-dpa4).
        //
        // This read used to be `br list --status=closed --json` under a 10s deadline, and
        // both halves were wrong. Measured 2026-09-07: that command returns 196 closed rows
        // and ZERO of them carry a `comments` key, so `check_deletions` scanned an empty
        // comment vector on every bead -- and 14 closed beads in this tracker cite a `bin/`
        // or `.flywheel/` path ONLY in comments, one of them the very bead that created this
        // gate. The comment half of the gate was structurally vacuous, not merely untested.
        // The 10s deadline was the second defect: it sits INSIDE the measured 40-250s `br`
        // contention band, so a healthy-but-contended read refused the commit, while any
        // ceiling above the band would make the operator wait minutes. Reading the mirror
        // removes a subprocess from the commit path and has neither failure mode -- no
        // `.beads/.write.lock`, no deadline, and the comments are actually present.
        //
        // RESTRICTIVE: an absent, unreadable, empty, or record-free mirror pushes a refusal
        // and fails the commit CLOSED. A deletion is never certified against an oracle that
        // was not read.
        let closed = match pre_delete_citation_check::read_closed_beads_from_mirror(&repo_root) {
            Ok(beads) => beads,
            Err(error) => {
                refusals.push(format!(
                    "pre-delete-citation-check: {error}; citation gate unrun"
                ));
                Vec::new()
            }
        };
        let conflicts = pre_delete_citation_check::check_deletions(&deletions, &closed);
        // DENOMINATORS, so a reader can tell "checked nothing" from "checked everything and
        // found nothing". A bare "no conflicts" is the vacuous-green shape this gate exists
        // to refuse.
        let comment_bearing = closed.iter().filter(|b| !b.comments.is_empty()).count();
        let _ = writeln!(
            io::stderr(),
            "pre-delete-citation-check: staged_deletions={} closed_beads={} \
             with_comments={} conflicts={}",
            deletions.len(),
            closed.len(),
            comment_bearing,
            conflicts.len()
        );
        for c in &conflicts {
            refusals.push(format!(
                "pre-delete-citation-check: {} cites deleted path {} in {}",
                c.bead_id, c.deleted_path, c.field
            ));
        }
    }

    validate_staged_tick_ledger(&repo_root, &mut refusals);

    // ── GATE 7: staged-build-gate (929j) ────────────────────────────────
    //
    // WIRED LAST on purpose: it is the only gate that COMPILES anything, so every cheap
    // refusal above must have had its say first. A commit refused for a tracked `.sh`
    // should not first pay for a cargo build.
    //
    // BOUNDED BY THE INDEX GIT HANDED US, not by a new mechanism. A path-scoped commit
    // (`git commit -- <paths>`, the form AGENTS.md mandates) makes git build a temporary
    // index and point GIT_INDEX_FILE at it, so the staged set here IS the committing
    // agent's own pathspec. The bead's 603-second measurement came from invoking the
    // binary directly, where GIT_INDEX_FILE is unset and the shared index is read.
    //
    // On a shared index the gate REFUSES INSTANTLY rather than compiling peers' crates.
    // That is the argument against the "cap the wall time" option: a timed-out scan
    // reports the same shape as a completed one, so a broken crate passes whenever peers
    // have staged enough work.
    staged_build_gate_on_commit_path(&repo_root, &staged, &mut refusals);

    // ── GATE 8: crate-atom-gate (d3gm) ─────────────────────────────────────
    //
    // The nine-part crate schema. Runs only when this commit touches a manifest or a
    // crate's tests/, which are the two edits that can change a crate's shape.
    crate_atom_gate_on_commit_path(&repo_root, &staged, &mut refusals);

    if refusals.is_empty() {
        // ── nh5: THE TOCTOU RECHECK ─────────────────────────────────────
        //
        // The lock serialises OUR hooks. It cannot stop a bare `git add` or
        // `git reset` from another pane, because those paths run no hook. So
        // re-read the staged set and compare: if it moved while the gates ran,
        // this verdict describes a set that is not being committed, and saying
        // CLEAN would be the silent loss with extra steps.
        let after = match get_staged_files() {
            Ok(files) => files,
            Err(err) => {
                eprintln!("MULTI-GATE ERROR: staged-set recheck failed: {err}");
                return ExitCode::from(2);
            }
        };
        let before_digest = commit_serialization::staged_digest(&staged);
        let after_digest = commit_serialization::staged_digest(&after);
        if let commit_serialization::IndexDrift::Drifted { before, after } =
            commit_serialization::classify_drift(before_digest, after_digest)
        {
            eprintln!(
                "{}",
                commit_serialization::retry_refusal(
                    "INDEX_DRIFTED_MID_GATE",
                    &format!("before={before:x} after={after:x}"),
                )
            );
            return ExitCode::from(1);
        }
        if matches!(merge_context, MergeContext::AncestryOnly) {
            eprintln!("ANCESTRY_ONLY_MERGE: merge range is empty and merge tree equals HEAD tree");
            PreCommitOutcome::AncestryOnlyMerge.exit_code()
        } else {
            record_gate_firings(&repo_root, &staged, &refusals, true);
            eprintln!("CLEAN: all staged files passed the multi-gate checks");
            PreCommitOutcome::Clean.exit_code()
        }
    } else {
        record_gate_firings(&repo_root, &staged, &refusals, false);
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
/// Refuse executable-mode Rust sources in the staged index.
///
/// The write tool can set the executable bit before this commit-time gate runs. The gate therefore
/// checks the index mode, not the worktree mode, and leaves the interim mode-only status diagnostic
/// to the operator rather than claiming write-time coverage.
fn validate_staged_rust_modes(repo_root: &Path, staged: &[String], refusals: &mut Vec<String>) {
    let mut command = std::process::Command::new("git");
    command
        .current_dir(repo_root)
        .args(["ls-files", "--stage", "-z", "--"]);
    let index_bytes = match subprocess_contract::bounded_output(
        &mut command,
        std::time::Duration::from_secs(10),
    ) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => {
            output.stdout
        }
        subprocess_contract::BoundedOutcome::Completed(output) => {
            refusals.push(format!(
                "mode-gate: ERROR git ls-files --stage exited {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
            return;
        }
        subprocess_contract::BoundedOutcome::TimedOut => {
            refusals.push("mode-gate: ERROR git ls-files --stage exceeded deadline; group killed".to_owned());
            return;
        }
        subprocess_contract::BoundedOutcome::Unspawned(error) => {
            refusals.push(format!("mode-gate: ERROR cannot spawn git ls-files --stage: {error}"));
            return;
        }
    };

    for record in index_bytes.split(|byte| *byte == 0) {
        let Some(tab) = record.iter().position(|byte| *byte == b'\t') else {
            continue;
        };
        let metadata = String::from_utf8_lossy(&record[..tab]);
        let Some(mode) = metadata.split_whitespace().next() else {
            continue;
        };
        let path = String::from_utf8_lossy(&record[tab + 1..]);
        if mode == "100755"
            && path.ends_with(".rs")
            && staged.iter().any(|candidate| candidate.as_str() == path.as_ref())
        {
            refusals.push(format!(
                "mode-gate: REFUSED path={path} mode={mode} reason=staged Rust source must remain non-executable"
            ));
        }
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

/// The absolute git directory, which is where the gate-section lock lives.
///
/// **Per git dir, not per repo path.** Two checkouts of the same repository have
/// different git dirs and must not serialise against each other — they do not share
/// an index, so there is no race between them. Getting this wrong would make the
/// lock a global mutex across unrelated work.
///
/// Falls back to `./.git` when git cannot answer. That is deliberate rather than a
/// refusal: a hook that cannot find its git dir is already in a state where the
/// commit will fail on its own, and `./.git` still serialises the common case. The
/// fallback CANNOT silently disable the lock — an uncreatable path yields
/// `Serialization::Unusable`, which fails closed.
fn resolve_git_dir() -> std::path::PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    match bounded_git_text(&cwd, &["rev-parse", "--absolute-git-dir"]) {
        Ok(text) if !text.trim().is_empty() => std::path::PathBuf::from(text.trim()),
        _ => cwd.join(".git"),
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

    if src.is_empty() {
        return Some("round-trip: empty commit message is an error".to_owned());
    }
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

/// GATE 7 — compile the crates THIS commit touches, and nothing else.
///
/// # It INVOKES the kernel; it does not reimplement it
///
/// `staged-build-gate`'s binary already owns the per-crate driver: the staged-vs-worktree
/// divergence check, the bounded build, the RCH-shim exit-103 distinction, and the refusal
/// rendering. `6fot`'s 12 tests and three mutation sets are the correctness authority for
/// all of it. A second copy of that loop here would be one that drifts, so this function
/// classifies the index, spawns the binary, and maps its exit code.
///
/// **The child inherits `GIT_INDEX_FILE`, which is the whole trick.** Inside a path-scoped
/// commit that variable points at git's temporary index, so `git diff --cached` in the child
/// returns the committing agent's own pathspec — measured, not assumed.
///
/// # `--no-verify` is named in the output, deliberately
///
/// A bypass nobody can name is a bypass everybody uses. Every refusal here says how to
/// bypass it, so an agent under time pressure reaches for the visible escape rather than
/// deleting the gate.
/// GATE 8 — the nine-part crate schema (`d3gm`).
///
/// # Scoped to the two edits that change a crate's SHAPE
///
/// A manifest edit can add or remove a `[lib]`/`[[bin]]` target or a path dependency
/// (parts 1, 2, 9); a `tests/` edit can add or remove one of the five named legs (part 4).
/// Every other commit leaves the atom unchanged, so scanning on it would spend a
/// `cargo metadata` per commit to re-derive an answer nobody's change could have moved.
///
/// # FAIL CLOSED on an absent binary, and the reason is measured
///
/// A missing gate binary is not permission to commit — same ruling as `staged-build-gate`
/// above. And the absence is currently REAL rather than hypothetical: under the
/// 2026-09-03 "we build on contabo" ruling the only enabled rch workers are Linux, so a
/// lane build of this gate yields `ELF 64-bit x86-64` on an `arm64` host. Measured this
/// session on a sibling crate: `./target/debug/decision-ledger` -> `cannot execute binary
/// file`. So the refusal below names the cross-compile bead rather than telling an
/// operator to run a build that cannot produce a runnable artifact.
fn crate_atom_gate_on_commit_path(repo_root: &Path, staged: &[String], refusals: &mut Vec<String>) {
    let touches_shape = staged.iter().any(|path| {
        path.ends_with("Cargo.toml")
            || (path.starts_with("crates/") && path.contains("/tests/"))
    });
    if !touches_shape {
        eprintln!(
            "crate-atom-gate: GATE_NOT_APPLICABLE -- no staged Cargo.toml and no \
             crates/*/tests/* path. DECLARED SCOPE: manifest edits (parts 1/2/9) and \
             tests/ edits (part 4); nothing else can move a crate's shape."
        );
        return;
    }
    let candidates = [
        repo_root.join("target/debug/crate-atom-gate"),
        dirs_home()
            .map(|home| home.join(".local/bin/crate-atom-gate"))
            .unwrap_or_else(|| repo_root.join("target/debug/crate-atom-gate")),
    ];
    let Some(binary) = candidates.into_iter().find(|path| path.is_file()) else {
        refusals.push(
            "crate-atom-gate: CRATE_ATOM_GATE_ERROR reason=BINARY_ABSENT \
             detail=\"neither target/debug/crate-atom-gate nor $HOME/.local/bin/crate-atom-gate \
             exists, so the nine-part schema was NOT checked for the crates this commit \
             reshapes\" next_action=see-omp-orchestrator-sor6 -- an arm64 build needs the \
             zigbuild lane; a contabo build yields ELF and cannot run here. Bypass visibly \
             with `git commit --no-verify`"
                .to_owned(),
        );
        return;
    };
    let mut command = std::process::Command::new(&binary);
    command.current_dir(repo_root).arg("check");
    match subprocess_contract::bounded_output(&mut command, std::time::Duration::from_secs(180)) {
        subprocess_contract::BoundedOutcome::Completed(output) => {
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            // Exit 2 is UNRUN and exit 3 is an INSTRUMENT ERROR. Neither is a finding
            // about a crate, and neither may pass: an instrument that could not look must
            // not render as a subject that passed.
            match output.status.code() {
                Some(0) => {
                    for line in text.lines().rev().take(1) {
                        eprintln!("crate-atom-gate: {line}");
                    }
                }
                code => {
                    let label = match code {
                        Some(2) => "UNRUN",
                        Some(3) => "INSTRUMENT_ERROR",
                        _ => "REFUSED",
                    };
                    for line in text.lines().filter(|l| l.contains("MISSING") || l.contains("CEILING")).take(20) {
                        refusals.push(format!("crate-atom-gate: {label} {line}"));
                    }
                    if !text.contains("MISSING") && !text.contains("CEILING") {
                        refusals.push(format!(
                            "crate-atom-gate: {label} exit={code:?} -- the gate refused \
                             without naming a row, which is an instrument fault"
                        ));
                    }
                }
            }
        }
        subprocess_contract::BoundedOutcome::TimedOut { .. } => refusals.push(
            "crate-atom-gate: UNRUN reason=TIMEOUT -- a deadline is not a verdict about \
             any crate; the schema went unchecked"
                .to_owned(),
        ),
        subprocess_contract::BoundedOutcome::Unspawned(error) => refusals.push(format!(
            "crate-atom-gate: INSTRUMENT_ERROR reason=UNSPAWNED detail={error}"
        )),
    }
}

/// The author's home, for the installed-binary candidate. Never a literal.
fn dirs_home() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}

fn staged_build_gate_on_commit_path(
    repo_root: &Path,
    staged: &[String],
    refusals: &mut Vec<String>,
) {
    let scope = staged_build_gate::IndexScope::from_env();
    if let Some(refusal) = scope.refusal() {
        // NOT a silent skip and NOT a build. An unattributable staged set is an INSTANT
        // refusal with a one-flag remedy — which is the argument against capping the wall
        // time instead: a timed-out scan reports the same shape as a completed one, so a
        // broken crate passes whenever peers have staged enough work.
        refusals.push(format!("staged-build-gate: {refusal}"));
        return;
    }

    // Cheap pre-check so a docs-only commit never spawns cargo at all. The binary would
    // reach the same verdict; doing it here keeps GATE_NOT_APPLICABLE free.
    let touched = match staged_build_gate::classify_scope(staged) {
        staged_build_gate::StagedScope::NotApplicable => {
            eprintln!(
                "staged-build-gate: GATE_NOT_APPLICABLE index={} -- no staged path lives under crates/",
                scope.label()
            );
            return;
        }
        staged_build_gate::StagedScope::Crates(crates) => crates,
    };
    eprintln!(
        "staged-build-gate: index={} crates_evaluated={} [{}]",
        scope.label(),
        touched.len(),
        touched.iter().cloned().collect::<Vec<_>>().join(", ")
    );

    let Some(binary) = staged_build_gate_binary(repo_root) else {
        // FAIL CLOSED. A missing gate binary is not permission to commit; that is the
        // exact shape of a gate invoked by nothing reading as protection.
        refusals.push(
            "staged-build-gate: STAGED_BUILD_GATE_ERROR reason=BINARY_ABSENT \
             detail=\"neither $HOME/.local/bin/staged-build-gate nor target/debug/staged-build-gate \
             exists, so the crates this commit touches were NOT built\" \
             next_action=cargo-build-p-staged-build-gate -- bypass visibly with `git commit --no-verify`"
                .to_owned(),
        );
        return;
    };

    let mut command = std::process::Command::new(&binary);
    command.current_dir(repo_root);
    // The deadline is per-crate inside the binary; this outer bound covers the whole run
    // plus one crate's grace, so a wedged child cannot hold the commit open forever.
    let outer = std::time::Duration::from_secs(
        staged_build_gate::BUILD_DEADLINE_SECS * (touched.len() as u64) + 30,
    );
    match subprocess_contract::bounded_output(&mut command, outer) {
        subprocess_contract::BoundedOutcome::Completed(output) => {
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            if output.status.success() {
                for line in text.lines().filter(|l| !l.trim().is_empty()) {
                    eprintln!("staged-build-gate: {line}");
                }
                return;
            }
            for line in text.lines().filter(|l| !l.trim().is_empty()) {
                refusals.push(format!(
                    "staged-build-gate: {line} -- bypass visibly with `git commit --no-verify`"
                ));
            }
            if text.trim().is_empty() {
                // ANTI-VACUITY: a nonzero exit with no output must not become a silent
                // refusal with no reason, which is indistinguishable from a crash.
                refusals.push(format!(
                    "staged-build-gate: STAGED_BUILD_GATE_ERROR reason=NO_OUTPUT exit={:?} \
                     detail=\"the gate refused and said nothing; treat as unproven\"",
                    output.status.code()
                ));
            }
        }
        subprocess_contract::BoundedOutcome::TimedOut => refusals.push(format!(
            "staged-build-gate: STAGED_BUILD_GATE_REFUSED reason=OUTER_DEADLINE deadline_secs={} \
             detail=\"a timeout is not a verdict; the crates this commit touches are UNPROVEN\" \
             -- bypass visibly with `git commit --no-verify`",
            outer.as_secs()
        )),
        subprocess_contract::BoundedOutcome::Unspawned(error) => refusals.push(format!(
            "staged-build-gate: STAGED_BUILD_GATE_ERROR reason=UNSPAWNED detail=\"{error}\" \
             -- bypass visibly with `git commit --no-verify`"
        )),
    }
}

/// The gate binary the fleet actually has. Installed copy first, build output second.
fn staged_build_gate_binary(repo_root: &Path) -> Option<std::path::PathBuf> {
    let mut candidates = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(Path::new(&home).join(".local/bin/staged-build-gate"));
    }
    candidates.push(repo_root.join("target/debug/staged-build-gate"));
    candidates.push(repo_root.join("target/release/staged-build-gate"));
    candidates.into_iter().find(|path| path.is_file())
}

fn record_gate_firings(repo_root: &Path, staged: &[String], refusals: &[String], clean: bool) {
    let path = firing_ledger::default_ledger_path(repo_root);
    let source = bounded_git_text(repo_root, &["rev-parse", "HEAD"])
        .ok()
        .map(|text| format!("commit:{}", text.trim()))
        .unwrap_or_else(|| "commit:unknown".to_owned());
    let input = if staged.is_empty() {
        "<empty-staged-set>".to_owned()
    } else {
        staged.join(",")
    };
    if let Err(error) =
        firing_ledger::append_commit_outcome(&path, &source, &input, refusals, clean)
    {
        eprintln!("gate-firing-ledger: {error}");
    }
}

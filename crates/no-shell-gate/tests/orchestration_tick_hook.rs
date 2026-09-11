use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

const GOOD_ROW: &str = r#"{"ts":1700000000,"tick":2,"observed":{"free_capacity":["%1408"],"attention":0,"dead":0,"source":"tick-monitor observe"},"dispatched":[{"pane":"%1408","bead":"omp-orchestrator-demo","claimed_first":true,"receipt":"idle->working t=6"}],"refused":[],"landed":{"commits_since_last_tick":0,"beads_closed":0,"graded_by_non_implementer":0},"claims":[{"figure":"free_capacity=1","command":"tick-monitor observe --session omp-orchestrator"}],"destructive":[],"not_done":["no edits to delegated paths"]}"#;

#[test]
fn unstaged_tick_ledger_is_not_applicable_not_nothing_to_check() {
    let repo = fixture_repo("not-applicable");
    write_and_stage(&repo, "README.txt", "fixture\n");

    let output = run_gate(&repo);
    let stderr = text(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        stderr.contains("orchestration-tick-gate: GATE_NOT_APPLICABLE"),
        "{stderr}"
    );
    cleanup(&repo);
}

#[test]
fn staged_empty_tick_ledger_is_a_typed_refusal() {
    let repo = fixture_repo("empty-ledger");
    write_and_stage(&repo, ".flywheel/orchestration-ticks.jsonl", "\n");

    let output = run_gate(&repo);
    let stderr = text(&output.stderr);
    assert!(
        !output.status.success(),
        "empty staged ledger passed: {stderr}"
    );
    assert!(
        stderr.contains("orchestration-tick-gate: file=.flywheel/orchestration-ticks.jsonl")
            && stderr.contains("LEDGER_NOTHING_TO_CHECK"),
        "{stderr}"
    );
    cleanup(&repo);
}

#[test]
fn staged_good_tick_ledger_is_clean() {
    let repo = fixture_repo("good-ledger");
    write_and_stage(&repo, ".flywheel/orchestration-ticks.jsonl", GOOD_ROW);

    let output = run_gate(&repo);
    let stderr = text(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        stderr.contains(
            "orchestration-tick-gate: CLEAN file=.flywheel/orchestration-ticks.jsonl rows=1"
        ),
        "{stderr}"
    );
    cleanup(&repo);
}

#[test]
fn hook_validates_staged_bytes_not_a_dirty_worktree_copy() {
    let repo = fixture_repo("staged-bytes");
    write_and_stage(&repo, ".flywheel/orchestration-ticks.jsonl", GOOD_ROW);
    fs::write(
        repo.join(".flywheel/orchestration-ticks.jsonl"),
        "{\"investigation\":1}\n",
    )
    .unwrap();

    let output = run_gate(&repo);
    let stderr = text(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        stderr.contains("orchestration-tick-gate: CLEAN"),
        "{stderr}"
    );
    cleanup(&repo);
}

/// A throwaway tree that can ANSWER the workspace-wide gates, rather than one that is exempted
/// from them.
///
/// # Measured: three of this file's four legs were RED BY CONSTRUCTION, and none of the three
/// reds was about ticks
///
/// `hook_validates_staged_bytes_not_a_dirty_worktree_copy` died at
/// `assert!(output.status.success())` while its OWN subject assertion held — the gate printed
/// `orchestration-tick-gate: CLEAN file=.flywheel/orchestration-ticks.jsonl rows=1`, which is
/// the invariant the leg is named for. The exit code was nonzero because the fixture could not
/// answer three UNRELATED gates:
///
/// ```text
/// hook_freshness:        REFUSED reason=HOOK_UNREADABLE  (no .git/hooks/pre-commit to stat)
/// project-agent-gate:    PROJECT_AGENT_REFUSED           (no .omp/agents/omp-grader.md)
/// preregistration-gate:  git rev-parse HEAD exited 128   (git init, never committed: NO HEAD)
/// ```
///
/// A RED WHOSE PANIC LINE IS A BLANKET `assert!(status.success())` IS THE FALSE RED THAT TRAINS
/// OPERATORS TO IGNORE A SUITE, and this one nearly justified "fixing" a gate that was correct:
/// the tick gate reads the INDEX through `staged_blob(repo_root, LEDGER_PATH)`
/// (pre-commit-gate.rs:685) and is the counter-example, not the victim. Read the panic, not the
/// test name.
///
/// So the remedy is to make the tree ANSWERABLE, never to exempt it — the same fix
/// `empty_staged.rs::fresh_git_tree` already carries (1183d02). THE THREE FILES ALONE ARE NOT
/// SUFFICIENT: writing them creates no commit, so `rev-parse HEAD` still exits 128. The baseline
/// commit below is what makes HEAD resolvable, and it is the half an inference-from-shape would
/// have missed.
///
/// Every input stays ARMED. The grader definition is the repository's real one, embedded at
/// compile time, so a grader that stops satisfying its contract reddens these fixtures too; the
/// mirror carries one closed row citing nothing, which the citation gate must accept and would
/// refuse if it were unreadable.
fn fixture_repo(label: &str) -> PathBuf {
    let repo = std::env::temp_dir().join(format!(
        "orchestration-tick-hook-{label}-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(&repo).unwrap();
    run_command(&repo, &["git", "init", "-q"]);
    run_command(
        &repo,
        &["git", "config", "user.email", "test@example.invalid"],
    );
    run_command(&repo, &["git", "config", "user.name", "Tick Gate Test"]);

    fs::create_dir_all(repo.join(".omp/agents")).expect("create fixture agent dir");
    fs::write(
        repo.join(no_shell_gate::project_agent::OMP_GRADER_PATH),
        include_str!("../../../.omp/agents/omp-grader.md"),
    )
    .expect("write fixture grader agent");

    fs::create_dir_all(repo.join(".beads")).expect("create fixture bead mirror dir");
    fs::write(
        repo.join(".beads/issues.jsonl"),
        "{\"id\":\"fixture-closed\",\"status\":\"closed\",\"close_reason\":\"DONE: fixture baseline worker=local\"}\n",
    )
    .expect("write fixture bead mirror");

    // Written AFTER `git init`, which creates `.git`. The fixture carries no hook SOURCES, so
    // freshness is NOT_APPLICABLE here; this file only lets the gate reach that determination
    // instead of refusing an unstattable path. Mode 0644 keeps git from executing it.
    fs::create_dir_all(repo.join(".git/hooks")).expect("create fixture hook dir");
    fs::write(
        repo.join(".git/hooks/pre-commit"),
        "fixture hook placeholder\n",
    )
    .expect("write fixture hook");

    // The mirror is STAGED as well as written: the close-reason reader is moving from the
    // worktree to the index (GradeCloseReason's residual on pre-commit-gate.rs), and a fixture
    // that satisfies only one of the two surfaces would redden the moment that lands.
    run_command(&repo, &["git", "add", "--", ".beads/issues.jsonl"]);
    run_command(&repo, &["git", "commit", "-qm", "fixture baseline [test]"]);

    // MEASURED IN TWO STEPS, NEITHER OF WHICH IS VISIBLE IN THE FIXTURE'S SHAPE. Writing the
    // three files plus one commit makes HEAD resolvable, and the gate then advances TWICE:
    //
    //   1. git ["show", "HEAD:docs/plan/HYPOTHESES.jsonl"] exited 128
    //      -- the ledger is read OUT OF HEAD, so a worktree copy is not enough;
    //   2. PREREGISTRATION_ERROR hypothesis=h1 recorded_commit=0123… is not the committed
    //      parent=<sha>
    //      -- and `validate_pre_write` (preregistration-gate/src/lib.rs:249-256) requires every
    //      hypothesis's `recorded_commit` to be a MEMBER OF THIS REPOSITORY'S HISTORY. A
    //      placeholder sha can never satisfy that, in any fixture, ever.
    //
    // Hence TWO commits: the baseline above gives a real sha, and the hypothesis records THAT
    // sha, so the row is preregistered against a commit that genuinely exists here. Each step
    // was named by a run; the second could not have been reached without fixing the first, which
    // is why "port the three inputs" was necessary and not sufficient.
    let baseline = capture(&repo, &["git", "rev-parse", "HEAD"]);
    assert_eq!(baseline.len(), 40, "fixture baseline sha: {baseline:?}");
    fs::create_dir_all(repo.join("docs/plan")).expect("create fixture plan dir");
    fs::write(
        repo.join("docs/plan/HYPOTHESES.jsonl"),
        format!(
            r#"{{"id":"h1","prediction":"fixture","falsifier":"missing section","evidence_scope":"docs/plan","recorded_commit":"{baseline}","observed_result":null}}"#
        ) + "\n",
    )
    .expect("write fixture hypotheses");
    run_command(&repo, &["git", "add", "--", "docs/plan/HYPOTHESES.jsonl"]);
    run_command(&repo, &["git", "commit", "-qm", "fixture hypotheses [test]"]);
    repo
}

fn write_and_stage(repo: &Path, relative: &str, contents: &str) {
    let path = repo.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, contents).unwrap();
    run_command(repo, &["git", "add", "--", relative]);
}

fn run_gate(repo: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pre-commit-gate"))
        .current_dir(repo)
        .output()
        .expect("spawn pre-commit-gate")
}

fn run_command(repo: &Path, args: &[&str]) {
    let output = Command::new(args[0])
        .args(&args[1..])
        .current_dir(repo)
        .output()
        .expect("run fixture command");
    assert!(
        output.status.success(),
        "fixture command failed: {}",
        text(&output.stderr)
    );
}

/// `run_command`, but returning trimmed stdout. Separate rather than a changed signature so the
/// existing call sites keep asserting success and discarding output, which is what they want.
fn capture(repo: &Path, args: &[&str]) -> String {
    let output = Command::new(args[0])
        .args(&args[1..])
        .current_dir(repo)
        .output()
        .expect("run fixture command");
    assert!(
        output.status.success(),
        "fixture command failed: {}",
        text(&output.stderr)
    );
    text(&output.stdout).trim().to_owned()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn cleanup(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

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

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn cleanup(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

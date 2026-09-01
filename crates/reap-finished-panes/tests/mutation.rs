//! Fires-on-known-bad mutation legs. Each names its rule on a column-0 RED line.

use reap_finished_panes::{
    acquire_lock, apply_deadline, is_worker_pane, spawn_timeout, ReapFinishedPanesLockOutcome, ReapFinishedPanesRules, SweepStats,
};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_reap-finished-panes"))
}

fn reaper() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bin/pane-result-reaper.sh")
}

/// rfp-001: pane 0 is the human shell. Deleting the skip reaps Joshua's pane.
#[test]
fn mutation_skip_human_shell() {
    let mut off = ReapFinishedPanesRules::default();
    assert!(off.disable("skip_human_shell"));
    assert!(
        is_worker_pane("0", &off),
        "disabled skip_human_shell must admit pane 0"
    );
    println!("MUTATION skip_human_shell disabled -> pane 0 admitted (would reap the human shell)");

    let on = ReapFinishedPanesRules::default();
    assert!(
        !is_worker_pane("0", &on),
        "rule skip_human_shell: pane 0 is never a worker"
    );
    println!("MUTATION RED skip_human_shell: pane 0 refused (human shell is never reaped)");
}

/// rfp-002: a deadline that truncates silently looks like "nothing needed reaping".
#[test]
fn mutation_deadline_reports_unswept() {
    let mut off = ReapFinishedPanesRules::default();
    assert!(off.disable("deadline_reports_unswept"));
    let mut stats = SweepStats::default();
    let started = Instant::now() - Duration::from_secs(9);
    assert!(
        !apply_deadline(&mut stats, started, Duration::from_secs(0), &off),
        "disabled deadline must not count unswept"
    );
    assert_eq!(stats.unswept, 0);
    println!("MUTATION deadline_reports_unswept disabled -> unswept=0 (silent truncate)");

    let mut on_stats = SweepStats::default();
    assert!(apply_deadline(
        &mut on_stats,
        started,
        Duration::from_secs(0),
        &ReapFinishedPanesRules::default()
    ));
    assert_eq!(on_stats.deadline_hit, 1);
    assert_eq!(on_stats.unswept, 1);
    println!(
        "MUTATION RED deadline_reports_unswept: deadline_hit=1 unswept=1 (remainder is named)"
    );
}

/// rfp-003: a skip that cannot name its holder is undiagnosable.
#[test]
fn mutation_lock_names_holder() {
    let dir = std::env::temp_dir().join(format!("reap-hold-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let lock = dir.join("lock");
    let led = dir.join("led.jsonl");
    let lane = dir.join("lane.jsonl");
    let acquired = acquire_lock(&lock);
    let ReapFinishedPanesLockOutcome::Acquired(guard) = acquired else {
        panic!("test could not acquire lock: {acquired:?}");
    };
    let mut cmd = Command::new(rust_bin());
    cmd.env("REAP_SWEEP_LOCK", &lock)
        .env("REAPER_LEDGER", &led)
        .env("REAP_LANE_LEDGER", &lane)
        .env("REAP_APPLY", "0")
        .env("REAP_PANE_LIST", "alpha 1\n")
        .env("REAPER", reaper())
        .env(
            "CP",
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
        );
    let out = spawn_timeout(cmd, Duration::from_secs(15)).expect("bin");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("SKIPPED") && text.contains("another_sweep_running"),
        "rule lock_names_holder: skip must be on stdout, got {text}"
    );
    assert!(
        !text.contains("pid=unknown"),
        "rule lock_names_holder: holder must be named, got {text}"
    );
    assert!(
        text.contains(&format!("pid={}", std::process::id()))
            || text.contains("pid=") && !text.contains("pid=unknown"),
        "MUTATION RED lock_names_holder expected a real pid, got {text}"
    );
    println!("MUTATION RED lock_names_holder: {text}");
    drop(guard);
    let _ = std::fs::remove_dir_all(&dir);
}

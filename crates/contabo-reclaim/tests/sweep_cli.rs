//! Sweep trigger wiring proof (bead 4bem8): the `reclaim-sweep` binary is
//! the reachable trigger the sweep was missing. `probe::run` had zero
//! callers; this binary is its first. These legs prove the surface builds,
//! runs, and refuses bad input with typed exits -- without touching ssh,
//! the daemon, or any worker.

#![forbid(unsafe_code)]

use std::process::Command;

fn sweep(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_reclaim-sweep"))
        .args(args)
        .output()
        .expect("spawn reclaim-sweep")
}

#[test]
fn help_is_a_clean_zero() {
    let output = sweep(&["--help"]);
    assert_eq!(output.status.code(), Some(0), "help must exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--apply"), "help must name the apply flag: {stdout}");
    assert!(stdout.contains("dry run"), "help must state the default: {stdout}");
}

#[test]
fn missing_selection_is_a_typed_usage_refusal() {
    let output = sweep(&["--base", "/base"]);
    assert_eq!(output.status.code(), Some(2), "no selection must refuse");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("SELECTION_REQUIRED"), "got {stderr}");
}

#[test]
fn relative_base_is_refused_before_any_spawn() {
    let output = sweep(&["--base", "relative/path", "--all-workers"]);
    assert_eq!(output.status.code(), Some(2), "relative base must refuse");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("absolute"), "got {stderr}");
}

#[test]
fn unknown_worker_is_refused_by_name() {
    let output = sweep(&["--base", "/base", "--worker", "contabo-9"]);
    assert_eq!(output.status.code(), Some(2), "unknown worker must refuse");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("INVALID_WORKER"), "got {stderr}");
}

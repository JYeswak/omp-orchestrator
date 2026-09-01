use pane_truth::run_external;
use std::process::Command;
use std::time::Duration;

#[test]
fn selftest_runs_through_top_level_entrypoint() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_pane-truth"));
    command.arg("--selftest");
    let out = run_external(command, Duration::from_secs(20)).expect("entrypoint");
    let stdout = out.stdout;
    assert_eq!(out.status, Some(0), "top-level selftest failed: {stdout}");
    assert!(
        stdout.contains("SELFTEST PASS"),
        "top-level entrypoint omitted proof: {stdout}"
    );
}

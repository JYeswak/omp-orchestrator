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

/// GATE RULE 7 ON THE LIVE ORACLE — bead `omp-orchestrator-e4wp`.
///
/// `selftest_runs_through_top_level_entrypoint` above asserts both facts but treats them as
/// two independent expectations, so it cannot express the thing that was broken: the exit
/// status AGREEING with the emitted verdict. `main.rs:49` narrowed the selftest result with
/// `as u8`, and at 256 failures that produced `SELFTEST FAIL …` on stdout with a SUCCESS exit
/// — a broken oracle reporting itself healthy, which every dispatch decision keyed on
/// `pane-truth` would have inherited.
///
/// This reads the token and the status out of ONE run and asserts the biconditional, so a
/// future change that makes them drift fails here even if both individual expectations are
/// still satisfiable in isolation.
#[test]
fn the_live_oracle_verdict_and_exit_status_agree() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_pane-truth"));
    command.arg("--selftest");
    let out = run_external(command, Duration::from_secs(20)).expect("entrypoint");
    let stdout = out.stdout;

    let says_pass = stdout.contains("SELFTEST PASS pane-truth");
    let says_fail = stdout.contains("SELFTEST FAIL");
    assert!(
        says_pass ^ says_fail,
        "anti-vacuity: the oracle emitted neither verdict token, or both — a run that says \
         nothing reports identically to one that passed: {stdout}"
    );

    let exited_zero = out.status == Some(0);
    assert_eq!(
        says_pass, exited_zero,
        "the emitted verdict and the exit status disagree (status={:?}): {stdout}",
        out.status
    );
}

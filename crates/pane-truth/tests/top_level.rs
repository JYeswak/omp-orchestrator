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
///
/// USES `Command::output`, NOT `run_external`, AND THE REASON IS A MEASURED RACE. When this
/// leg called `run_external` it failed intermittently — 1 of 5 runs, finishing in 0.03s with
/// an EMPTY stdout, so it was not a timeout. `run_external` stages the child's output in a
/// temp directory named `pane-truth-child-{pid}-{nanos}` and `remove_dir_all`s it on the way
/// out. Two callers in the SAME test binary share the pid, so when the nanosecond stamps
/// collide one call deletes the other's staging directory and reads back nothing. Nothing
/// exercised that before: this crate had exactly one `run_external` caller per binary until
/// this leg became the second. `Command::output` drains both pipes on its own and stages
/// nothing, so it cannot participate in the collision. The `run_external` defect is reported
/// separately rather than patched here — it is not this bead's subject, and a flaky assertion
/// on the fleet's ground-truth oracle is worse than no assertion.
#[test]
fn the_live_oracle_verdict_and_exit_status_agree() {
    let out = Command::new(env!("CARGO_BIN_EXE_pane-truth"))
        .arg("--selftest")
        .output()
        .expect("entrypoint");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();

    let says_pass = stdout.contains("SELFTEST PASS pane-truth");
    let says_fail = stdout.contains("SELFTEST FAIL");
    assert!(
        says_pass ^ says_fail,
        "anti-vacuity: the oracle emitted neither verdict token, or both — a run that says \
         nothing reports identically to one that passed: {stdout}"
    );

    let exited_zero = out.status.code() == Some(0);
    assert_eq!(
        says_pass, exited_zero,
        "the emitted verdict and the exit status disagree (status={:?}): {stdout}",
        out.status.code()
    );
}

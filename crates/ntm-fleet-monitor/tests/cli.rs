//! Operator binary must mint apply=yes only through Approved::authorize.
//! The runtime bool apply_allowed() is not enough: xtask already takes Approved,
//! and this binary must not be a weaker sibling.

use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ntm-fleet-monitor"))
        .args(args)
        .output()
        .expect("ntm-fleet-monitor binary must run")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("binary output must be UTF-8")
}

#[test]
fn weaken_gate_exits_nonzero_and_is_not_applyable() {
    let output = run(&["classify", "--action", "weaken-gate"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).contains("verdict=required kind=gate-weaken apply=no"));
}

#[test]
fn recycle_frozen_without_two_captures_exits_nonzero() {
    let output = run(&["classify", "--action", "recycle-frozen"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).contains("verdict=refuse reason=single-capture-liveness apply=no"));
}

#[test]
fn observe_scan_is_applyable() {
    let output = run(&["classify", "--action", "observe-scan"]);
    assert!(output.status.success());
    assert!(stdout(&output).contains("verdict=autonomous apply=yes"));
}

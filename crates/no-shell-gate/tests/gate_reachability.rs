#![forbid(unsafe_code)]

//! Conformance legs for the gate-trigger reachability census.
//!
//! The production census is exercised through its binary output, not by duplicating its scan
//! algorithm here. Fixtures are deliberately tiny and make every expected trigger explicit.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("no-shell-gate must be nested under the workspace root")
        .to_path_buf()
}

fn binary_path() -> PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_gate-reachability") {
        return PathBuf::from(path);
    }
    std::env::current_exe()
        .expect("test executable path")
        .parent()
        .and_then(Path::parent)
        .expect("target/debug directory")
        .join("gate-reachability")
}

fn fixture_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("omp-gate-reachability-{label}-{nonce}"));
    fs::create_dir_all(&root).expect("fixture root");
    root
}

fn write_fixture(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("fixture parent")).expect("fixture parent");
    fs::write(path, contents).expect("fixture file");
}

fn run_census(root: &Path) -> std::process::Output {
    Command::new(binary_path())
        .args(["--root", root.to_str().expect("utf8 fixture root")])
        .output()
        .expect("gate-reachability binary must run")
}

fn json_output(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "binary must emit JSON on stdout; status={:?} stderr={} error={error}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn rows(report: &Value) -> &Vec<Value> {
    report
        .get("rows")
        .and_then(Value::as_array)
        .expect("machine report rows")
}

#[test]
fn known_good_fixture_reports_ci_trigger_and_unwired_gate() {
    let root = fixture_root("good");
    write_fixture(&root, "crates/foo-gate/src/lib.rs", "pub fn gate() {}\n");
    write_fixture(
        &root,
        "crates/foo-gate/tests/gate.rs",
        "// KNOWN-GOOD\n#[test] fn gate() {}\n",
    );
    write_fixture(&root, "crates/bar-lint/src/lib.rs", "pub fn lint() {}\n");
    write_fixture(&root, "crates/plain/src/lib.rs", "pub fn plain() {}\n");
    write_fixture(
        &root,
        "crates/plain/tests/mutation.rs",
        "// MUTATION\n#[test] fn mutation() {}\n",
    );
    write_fixture(
        &root,
        ".github/workflows/gate.yml",
        "jobs:\n  foo:\n    steps:\n      - run: cargo test -p foo-gate\n  plain:\n    steps:\n      - run: cargo test -p plain\n",
    );

    let output = run_census(&root);
    assert!(output.status.success(), "{output:?}");
    let report = json_output(&output);
    assert_eq!(report["status"], "ok");
    let foo = rows(&report)
        .iter()
        .find(|row| row["name"] == "foo-gate" && row["kind"] == "crate")
        .expect("foo-gate row");
    assert_eq!(foo["reachable"], true);
    assert!(foo["triggers"][0]
        .as_str()
        .unwrap_or_default()
        .contains("CI"));
    let bar = rows(&report)
        .iter()
        .find(|row| row["name"] == "bar-lint" && row["kind"] == "crate")
        .expect("bar-lint row");
    assert_eq!(bar["reachable"], false);
    let plain = rows(&report)
        .iter()
        .find(|row| row["name"] == "plain/tests/mutation.rs" && row["kind"] == "test_gate")
        .expect("plain mutation test gate row");
    assert_eq!(plain["reachable"], true);
    assert!(plain["proof_command"]
        .as_str()
        .unwrap_or_default()
        .contains("cargo test -p plain"));
    fs::remove_dir_all(root).expect("fixture cleanup");
}
#[test]
fn removing_ci_trigger_flips_gate_to_unreachable() {
    let root = fixture_root("mutation");
    write_fixture(&root, "crates/foo-gate/src/lib.rs", "pub fn gate() {}\n");
    write_fixture(
        &root,
        ".github/workflows/gate.yml",
        "jobs:\n  foo:\n    steps:\n      - run: cargo test -p foo-gate\n",
    );

    let reachable = json_output(&run_census(&root));
    let row = rows(&reachable)
        .iter()
        .find(|row| row["name"] == "foo-gate" && row["kind"] == "crate")
        .expect("foo-gate row");
    assert_eq!(row["reachable"], true);

    fs::remove_file(root.join(".github/workflows/gate.yml")).expect("remove trigger");
    let unreachable = json_output(&run_census(&root));
    let row = rows(&unreachable)
        .iter()
        .find(|row| row["name"] == "foo-gate" && row["kind"] == "crate")
        .expect("foo-gate row after mutation");
    assert_eq!(row["reachable"], false);
    fs::remove_dir_all(root).expect("fixture cleanup");
}

#[test]
fn empty_gate_set_is_an_error_not_a_pass() {
    let root = fixture_root("empty");
    let output = run_census(&root);
    assert!(!output.status.success(), "empty gate set must refuse");
    let report = json_output(&output);
    assert_eq!(report["status"], "error");
    assert_eq!(report["error"], "EMPTY_GATE_SET");
    fs::remove_dir_all(root).expect("fixture cleanup");
}

#[test]
fn report_excludes_its_own_source_and_has_a_no_shell_positive_control() {
    let output = run_census(&repo_root());
    assert!(output.status.success(), "{output:?}");
    let report = json_output(&output);
    assert_eq!(report["status"], "ok");
    let excluded = report["excluded_paths"].as_array().expect("excluded paths");
    assert!(excluded.iter().any(|path| {
        path.as_str() == Some("crates/no-shell-gate/src/bin/gate-reachability.rs")
    }));
    assert!(excluded
        .iter()
        .any(|path| { path.as_str() == Some("crates/no-shell-gate/tests/gate_reachability.rs") }));
    assert!(!rows(&report)
        .iter()
        .any(|row| row["name"] == "no-shell-gate/tests/gate_reachability.rs"));
    let positive = &report["positive_control"];
    assert_eq!(positive["name"], "no-shell-gate");
    assert_eq!(positive["reachable"], true);
    assert!(positive["proof_command"]
        .as_str()
        .unwrap_or_default()
        .contains(".git/hooks/pre-commit"));
}

#[test]
fn positive_control_runs_real_hook_and_refuses_staged_shell() {
    let root = fixture_root("hook");
    let hook = repo_root().join(".git/hooks/pre-commit");
    assert!(hook.is_file(), "installed pre-commit hook must exist");
    write_fixture(&root, "bad.sh", "#!/bin/sh\necho bad\n");
    let git_init = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(&root)
        .output()
        .expect("git init");
    assert!(git_init.status.success(), "git init: {git_init:?}");
    fs::create_dir_all(root.join(".git/hooks")).expect("hook directory");
    fs::copy(&hook, root.join(".git/hooks/pre-commit")).expect("copy hook");
    let git_add = Command::new("git")
        .args(["add", "bad.sh"])
        .current_dir(&root)
        .output()
        .expect("git add");
    assert!(git_add.status.success(), "git add: {git_add:?}");
    let hook_output = Command::new(root.join(".git/hooks/pre-commit"))
        .current_dir(&root)
        .output()
        .expect("run copied pre-commit hook");
    let stderr = String::from_utf8_lossy(&hook_output.stderr);
    assert_eq!(hook_output.status.code(), Some(1), "hook stderr={stderr}");
    assert!(
        stderr.contains("no-shell-gate"),
        "hook must name positive-control gate: {stderr}"
    );
    fs::remove_dir_all(root).expect("fixture cleanup");
}

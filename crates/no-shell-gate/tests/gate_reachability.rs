#![forbid(unsafe_code)]

//! Conformance legs for the gate-trigger reachability census.
//!
//! The production census is exercised through its binary output. Fixtures keep executor triggers,
//! document mentions, comments, YAML parsing, read-state exclusions, and anti-vacuity observable.

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

fn row<'a>(report: &'a Value, name: &str, kind: &str) -> &'a Value {
    rows(report)
        .iter()
        .find(|candidate| candidate["name"] == name && candidate["kind"] == kind)
        .unwrap_or_else(|| panic!("missing row {kind}:{name}"))
}

#[test]
fn known_good_fixture_separates_executors_documents_and_controls() {
    let root = fixture_root("good");
    write_fixture(&root, "crates/foo-gate/src/lib.rs", "pub fn gate() {}\n");
    write_fixture(&root, "crates/foo-gate/tests/gate.rs", "#[test] fn gate() {}\n");
    write_fixture(&root, "crates/bar-lint/src/lib.rs", "pub fn lint() {}\n");
    write_fixture(&root, "crates/plain/src/lib.rs", "pub fn plain() {}\n");
    write_fixture(&root, "crates/plain/tests/gate_mutation.rs", "#[test] fn mutation() {}\n");
    write_fixture(
        &root,
        ".github/workflows/gate.yml",
        "jobs:\n  foo:\n    steps:\n      - run: cargo test -p foo-gate\n  plain:\n    steps:\n      - run: cargo test -p plain\n",
    );
    write_fixture(&root, ".flywheel/NOTE.txt", "foo-gate is documented here, not executed.\n");
    write_fixture(
        &root,
        ".omp/crontab",
        "* * * * * ntm --robot-send\n* * * * * ntm --robot-tail\n* * * * * orchestrator-tick --apply\n# ntm foo-gate\n",
    );

    let output = run_census(&root);
    assert!(output.status.success(), "{output:?}");
    let report = json_output(&output);
    assert_eq!(report["status"], "ok");
    let foo = row(&report, "foo-gate", "crate");
    assert_eq!(foo["reachable"], true);
    assert_eq!(foo["verdict"], "UNRUN");
    assert_eq!(foo["read_state"], "UNREAD");
    assert!(foo["triggers"].as_array().is_some_and(|items| {
        items.iter().any(|item| item.as_str().unwrap_or_default().contains("CI"))
    }));
    assert!(foo["documents"].as_array().is_some_and(|items| {
        items.iter().any(|item| item == ".flywheel/NOTE.txt")
    }));
    assert!(!foo["documents"].as_array().unwrap().iter().any(|item| {
        foo["triggers"].as_array().unwrap().contains(item)
    }));

    let bar = row(&report, "bar-lint", "crate");
    assert_eq!(bar["reachable"], false);
    assert_eq!(bar["verdict"], "INERT");
    let plain = row(&report, "plain/tests/gate_mutation.rs", "test_gate");
    assert_eq!(plain["verdict"], "UNRUN");
    assert!(plain["proof_command"].as_str().unwrap_or_default().contains("cargo test -p plain"));

    assert_eq!(report["controls"]["ntm"], 2);
    assert_eq!(report["controls"]["orchestrator_tick"], 1);
    assert_eq!(report["controls"]["zzz_cannot_exist"], 0);
    fs::remove_dir_all(root).expect("fixture cleanup");
}

#[test]
fn removing_ci_trigger_flips_gate_to_inert() {
    let root = fixture_root("mutation");
    write_fixture(&root, "crates/foo-gate/src/lib.rs", "pub fn gate() {}\n");
    write_fixture(
        &root,
        ".github/workflows/gate.yml",
        "jobs:\n  foo:\n    steps:\n      - run: cargo test -p foo-gate\n",
    );

    let reachable = json_output(&run_census(&root));
    assert_eq!(row(&reachable, "foo-gate", "crate")["verdict"], "UNRUN");
    fs::remove_file(root.join(".github/workflows/gate.yml")).expect("remove trigger");
    let inert = json_output(&run_census(&root));
    assert_eq!(row(&inert, "foo-gate", "crate")["verdict"], "INERT");
    fs::remove_dir_all(root).expect("fixture cleanup");
}

#[test]
fn empty_gate_set_is_typed_error_exit_two() {
    let root = fixture_root("empty");
    let output = run_census(&root);
    assert_eq!(output.status.code(), Some(2), "empty gate set must refuse");
    let report = json_output(&output);
    assert_eq!(report["status"], "error");
    assert_eq!(report["error"], "EMPTY_GATE_SET");
    fs::remove_dir_all(root).expect("fixture cleanup");
}

#[test]
fn duplicate_yaml_is_strict_parse_error_exit_one() {
    let root = fixture_root("duplicate-yaml");
    write_fixture(&root, "crates/foo-gate/src/lib.rs", "pub fn gate() {}\n");
    write_fixture(
        &root,
        ".github/workflows/gate.yml",
        "jobs:\n  foo:\n    steps: []\njobs:\n  duplicate:\n    steps: []\n",
    );
    let output = run_census(&root);
    assert_eq!(output.status.code(), Some(1), "strict YAML mutation must be a violation");
    let report = json_output(&output);
    assert_eq!(report["status"], "error");
    assert!(report["error"].as_str().unwrap_or_default().starts_with("STRICT_YAML_PARSE"));
    fs::remove_dir_all(root).expect("fixture cleanup");
}

#[test]
fn flow_yaml_duplicate_is_strict_parse_error_exit_one() {
    let root = fixture_root("flow-duplicate-yaml");
    write_fixture(&root, "crates/foo-gate/src/lib.rs", "pub fn gate() {}\n");
    write_fixture(
        &root,
        ".github/workflows/gate.yml",
        "jobs:\n  foo: {runs-on: ubuntu-latest, runs-on: ubuntu-latest}\n",
    );
    let output = run_census(&root);
    assert_eq!(output.status.code(), Some(1), "flow YAML duplicate must be a violation");
    let report = json_output(&output);
    assert_eq!(report["status"], "error");
    let error = report["error"].as_str().unwrap_or_default();
    assert!(error.starts_with("STRICT_YAML_PARSE"), "error prefix: {error}");
    assert!(
        error.contains("duplicate mapping key"),
        "must name the duplicate, not a serde last-key-wins success: {error}"
    );
    fs::remove_dir_all(root).expect("fixture cleanup");
}

#[test]
fn unique_flow_yaml_mapping_is_not_a_parse_error() {
    let root = fixture_root("flow-unique-yaml");
    write_fixture(&root, "crates/foo-gate/src/lib.rs", "pub fn gate() {}\n");
    write_fixture(
        &root,
        ".github/workflows/gate.yml",
        "jobs:\n  foo: {steps: [{run: cargo test -p foo-gate}]}\n",
    );
    let output = run_census(&root);
    assert_eq!(output.status.code(), Some(0), "unique flow mapping must parse");
    let report = json_output(&output);
    assert_ne!(report["status"], "error", "unique flow mapping must not STRICT_YAML_PARSE");
    fs::remove_dir_all(root).expect("fixture cleanup");
}

#[test]
fn comments_do_not_create_executors() {
    let root = fixture_root("comments");
    write_fixture(&root, "crates/foo-gate/src/lib.rs", "pub fn gate() {}\n");
    write_fixture(&root, ".github/workflows/gate.yml", "# cargo test -p foo-gate\n");
    write_fixture(&root, "crates/no-shell-gate/src/bin/pre-commit-gate.rs", "// foo-gate\n");
    write_fixture(&root, ".git/hooks/pre-commit", "#!/bin/sh\n# foo-gate\n");
    write_fixture(&root, ".omp/crontab", "# foo-gate ntm orchestrator-tick\n");
    let report = json_output(&run_census(&root));
    let foo = row(&report, "foo-gate", "crate");
    assert_eq!(foo["verdict"], "INERT");
    assert!(foo["triggers"].as_array().unwrap().is_empty());
    assert_eq!(report["controls"]["ntm"], 0);
    assert_eq!(report["controls"]["orchestrator_tick"], 0);
    fs::remove_dir_all(root).expect("fixture cleanup");
}

#[test]
fn read_state_excludes_self_and_reports_full_population() {
    let root = fixture_root("read-state");
    write_fixture(&root, "crates/foo-gate/src/lib.rs", "pub fn gate() {}\n");
    write_fixture(&root, ".github/workflows/gate.yml", "jobs:\n  foo:\n    steps:\n      - run: cargo test -p foo-gate\n");
    write_fixture(
        &root,
        ".beads/issues.jsonl",
        "{\"id\":\"omp-orchestrator-6nhj\",\"title\":\"foo-gate gate-reachability observed status check\"}\n{\"id\":\"external-row\",\"title\":\"foo-gate gate-reachability observed status check\"}\n",
    );
    let report = json_output(&run_census(&root));
    assert_eq!(report["read_state"]["population"], "full-file");
    assert_eq!(report["read_state"]["window"], "full-file");
    assert!(report["read_state"]["bytes"].as_u64().unwrap_or_default() > 0);
    assert_eq!(report["read_state"]["excluded_bead_ids"][0], "omp-orchestrator-6nhj");
    assert_eq!(report["read_state"]["observed_rows"], serde_json::json!(["external-row"]));
    let foo = row(&report, "foo-gate", "crate");
    assert_eq!(foo["verdict_observed"], true);
    assert_eq!(foo["read_state"], "OBSERVED");
    fs::remove_dir_all(root).expect("fixture cleanup");
}

#[test]
fn portable_no_shell_positive_control_does_not_require_hooks() {
    let root = fixture_root("portable");
    write_fixture(&root, "crates/no-shell-gate/src/lib.rs", "pub fn gate() {}\n");
    write_fixture(
        &root,
        ".github/workflows/gate.yml",
        "jobs:\n  no_shell:\n    steps:\n      - run: cargo test -p no-shell-gate\n",
    );
    let good = json_output(&run_census(&root));
    assert_eq!(good["positive_control"]["reachable"], true);
    assert!(!good["positive_control"]["proof_command"].as_str().unwrap_or_default().contains(".git/hooks"));
    fs::remove_file(root.join(".github/workflows/gate.yml")).expect("remove known-good trigger");
    let bad = json_output(&run_census(&root));
    assert_eq!(row(&bad, "no-shell-gate", "crate")["verdict"], "INERT");
    fs::remove_dir_all(root).expect("fixture cleanup");
}

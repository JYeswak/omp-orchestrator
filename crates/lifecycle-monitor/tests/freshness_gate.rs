#![forbid(unsafe_code)]

//! Freshness gate legs (3s6a): the `gate` verb refuses a stale layer set.
//!
//! Known-good runs all six layers fresh and expects GATE_OK exit 0.
//! Known-bad runs five fresh plus one stale L4 row and expects a typed
//! STALE_LAYERS refusal naming L4 with exit 1 -- never a clean pass.
//! Empty input is the typed EMPTY_SCAN error, never a pass.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const METRICS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../METRICS.toml");
const LAYERS: &[&str] = &["L0", "L1", "L2", "L3", "L4", "L5"];

fn gate(journal: &Path) -> Output {
    let metrics = Path::new(METRICS_PATH);
    Command::new(env!("CARGO_BIN_EXE_lifecycle-monitor"))
        .args([
            "gate",
            "--journal",
            journal.to_str().expect("journal path utf8"),
            "--metrics",
            metrics.to_str().expect("metrics path utf8"),
        ])
        .output()
        .expect("lifecycle-monitor gate must launch")
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_secs()
}

fn row(layer: &str, ts: u64) -> String {
    format!(
        r#"{{"schema":"omp.lifecycle_event.v1","layer":"{layer}","stage_from":"S1.X","stage_to":"S1.Y","actor":"test","outcome":"emitted","reason_code":"GATE_OK","ts_unix":{ts}}}"#
    )
}

fn journal_path(dir: &tempfile::TempDir, name: &str) -> PathBuf {
    dir.path().join(name)
}

#[test]
fn fresh_layers_pass_the_gate() {
    let dir = tempfile::tempdir().expect("gate fixture");
    let now = now_secs();
    let body: Vec<String> = LAYERS.iter().map(|layer| row(layer, now)).collect();
    let journal = journal_path(&dir, "fresh.jsonl");
    std::fs::write(&journal, body.join("\n")).expect("write fresh journal");
    let output = gate(&journal);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "all-fresh gate must pass: {stdout}"
    );
    assert!(stdout.contains("GATE_OK"), "{stdout}");
}

#[test]
fn one_stale_layer_vetoes_the_gate() {
    let dir = tempfile::tempdir().expect("gate fixture");
    let now = now_secs();
    let mut body: Vec<String> = LAYERS
        .iter()
        .filter(|layer| **layer != "L4")
        .map(|layer| row(layer, now))
        .collect();
    body.push(row("L4", 1));
    let journal = journal_path(&dir, "mixed.jsonl");
    std::fs::write(&journal, body.join("\n")).expect("write mixed journal");
    let output = gate(&journal);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(1),
        "stale layer must refuse: stdout={stdout} stderr={stderr}"
    );
    assert!(
        stderr.contains("LIFECYCLE_MONITOR_STALE_LAYERS"),
        "refusal must name the stale verdict: {stderr}"
    );
    assert!(
        stderr.contains("L4:silent"),
        "refusal must name the stale layer: {stderr}"
    );
    assert!(
        !stdout.contains("GATE_OK"),
        "stale layers must never report clean: {stdout}"
    );
}

#[test]
fn empty_journal_is_typed_gate_refusal() {
    let dir = tempfile::tempdir().expect("gate fixture");
    let journal = journal_path(&dir, "empty.jsonl");
    std::fs::write(&journal, "").expect("write empty journal");
    let output = gate(&journal);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "empty scan must refuse: {stderr}"
    );
    assert!(
        stderr.contains("LIFECYCLE_MONITOR_EMPTY_SCAN"),
        "empty refusal must be typed: {stderr}"
    );
}

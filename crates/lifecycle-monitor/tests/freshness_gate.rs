#![forbid(unsafe_code)]

//! Freshness gate legs (3s6a): the `gate` verb refuses a stale layer set.
//!
//! Known-good runs all six layers fresh and expects GATE_OK exit 0.
//! Known-bad runs five fresh plus one stale L4 row and expects a typed
//! STALE_LAYERS refusal naming L4 with exit 1 -- never a clean pass.
//! Empty input is the typed EMPTY_SCAN error, never a pass.

use lifecycle_event::{emit_host, DurableJournal, EmitOutcome, Layer, LifecycleEvent, ReasonCode};
use lifecycle_monitor::gate_claimed_write_readback;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const METRICS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../METRICS.toml");
const LAYERS: &[&str] = &["L0", "L1", "L2", "L3", "L4", "L5"];

fn gate(journal: &Path, known_bad: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lifecycle-monitor"));
    command.args([
        "gate",
        "--journal",
        journal.to_str().expect("journal path utf8"),
        "--metrics",
        Path::new(METRICS_PATH).to_str().expect("metrics path utf8"),
    ]);
    if known_bad {
        command.arg("--known-bad");
    }
    command
        .output()
        .expect("lifecycle-monitor gate must launch")
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
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_secs();
    let body: Vec<String> = LAYERS.iter().map(|layer| row(layer, now)).collect();
    let journal = journal_path(&dir, "fresh.jsonl");
    std::fs::write(&journal, format!("{}\n", body.join("\n"))).expect("write fresh journal");
    let output = gate(&journal, false);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "all-fresh gate must pass: {stdout}"
    );
    assert!(stdout.contains("GATE_OK"), "{stdout}");
    let journal_writer = DurableJournal::open(&journal).expect("open tamper journal");
    let claimed = LifecycleEvent::new(
        Layer::L5,
        "S1.L4",
        "S1.L5",
        "ompo-init",
        EmitOutcome::Emitted,
        ReasonCode::new("INIT_WRITE_OK").expect("reason code"),
    );
    emit_host(&journal_writer, std::slice::from_ref(&claimed)).expect("durable write");
    let tampered = LifecycleEvent::new(
        Layer::L5,
        "S1.L4",
        "S1.L5",
        "ompo-init",
        EmitOutcome::Emitted,
        ReasonCode::new("TAMPERED_CHECKSUM").expect("reason code"),
    );
    std::fs::write(&journal, format!("{}\n", tampered.to_json_line()))
        .expect("tamper readback artifact");
    let error = gate_claimed_write_readback(&journal_writer, std::slice::from_ref(&claimed))
        .expect_err("tampered readback must refuse");
    let diagnostic = error.to_string();
    assert!(
        diagnostic.contains("LIFECYCLE_MONITOR_READBACK_FAILED"),
        "{diagnostic}"
    );
    assert!(
        diagnostic.contains("the write succeeded is not evidence the artifact exists"),
        "{diagnostic}"
    );
}

#[test]
fn one_stale_layer_vetoes_the_gate() {
    let dir = tempfile::tempdir().expect("gate fixture");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_secs();
    let mut body: Vec<String> = LAYERS
        .iter()
        .filter(|layer| **layer != "L4")
        .map(|layer| row(layer, now))
        .collect();
    body.push(row("L4", 1));
    let journal = journal_path(&dir, "mixed.jsonl");
    std::fs::write(&journal, body.join("\n")).expect("write mixed journal");
    let output = gate(&journal, false);
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
    let output = gate(&journal, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(2),
        "empty scan must refuse: {stderr}"
    );
    assert!(
        stderr.contains("LIFECYCLE_MONITOR_EMPTY_SCAN"),
        "empty refusal must be typed: {stderr}"
    );
    let known_bad = gate(&journal, true);
    let known_bad_stdout = String::from_utf8_lossy(&known_bad.stdout);
    assert_eq!(known_bad.status.code(), Some(0), "{known_bad_stdout}");
    assert!(
        known_bad_stdout.contains("GATE_KNOWN_BAD_FIRED"),
        "{known_bad_stdout}"
    );
    assert!(
        known_bad_stdout.contains("LIFECYCLE_MONITOR_READBACK_FAILED"),
        "{known_bad_stdout}"
    );
}

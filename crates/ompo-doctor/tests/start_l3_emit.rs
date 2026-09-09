#![forbid(unsafe_code)]

//! L3 writer leg (hilk): one `ompo start` run emits one S1.L3 row.
//!
//! Chokepoint under grade: `run_start` in `crates/ompo-doctor/src/main.rs`
//! (the single fan-in of every `ompo start` invocation), via `emit_s1_l3_start`
//! with `lifecycle_event::ReasonCode` (a validated string, not an enum -- this
//! tree's several *Reason enums are NOT accepted by the event writer).
//! A distinct S1.L3 emitter already exists for the supervisor tick loop
//! (`omp-orchestrator/src/resident.rs: emit_s1_l3_l5`); this leg covers the
//! start-command path, not that loop.

use std::path::{Path, PathBuf};
use std::process::Command;

fn ompo_start(repo: &Path, session: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ompo"))
        .args([
            "start",
            "--repo",
            repo.to_str().expect("fixture path is utf8"),
            "--session",
            session,
        ])
        .output()
        .expect("ompo start must launch")
}

fn journal(repo: &Path) -> PathBuf {
    repo.join(".omp-orchestrator/work/s1/lifecycle.jsonl")
}

/// Typed scan refusal: zero S1.L3 rows is an ERROR with its own reason,
/// distinct from a content mismatch. Absence of evidence is not a pass.
fn find_s1_l3_row(journal: &Path) -> Result<serde_json::Value, String> {
    let text = std::fs::read_to_string(journal).map_err(|error| {
        format!(
            "L3_SCAN_UNREADABLE_JOURNAL path={} detail={error}",
            journal.display()
        )
    })?;
    for line in text.lines() {
        let value: serde_json::Value = serde_json::from_str(line)
            .map_err(|error| format!("L3_SCAN_UNPARSEABLE_ROW detail={error}"))?;
        if value.get("stage_to").and_then(|stage| stage.as_str()) == Some("S1.L3") {
            return Ok(value);
        }
    }
    Err(format!(
        "L3_SCAN_ZERO_S1_L3_ROWS journal={} lines={}",
        journal.display(),
        text.lines().count()
    ))
}

#[test]
fn start_run_emits_one_s1_l3_row() {
    let repo = tempfile::tempdir().expect("fixture repo");
    let output = ompo_start(repo.path(), "hilk-leg");
    assert!(
        output.status.success(),
        "ompo start failed: {:?} stderr={:?}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let row = find_s1_l3_row(&journal(repo.path())).expect("S1.L3 row must exist");
    assert_eq!(row["stage_to"], "S1.L3");
    assert_eq!(row["stage_from"], "S1.L2");
    assert_eq!(row["layer"], "L3");
    assert_eq!(row["reason_code"], "START_OK");
    assert_eq!(row["actor"], "ompo");
    assert_eq!(row["outcome"], "emitted");
}

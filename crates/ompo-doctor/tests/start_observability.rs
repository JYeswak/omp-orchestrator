#![forbid(unsafe_code)]

//! L3 monitor leg (vyzr): `ompo start --json` carries an `observability`
//! block with TUI/JSON ID parity and the HD-0009 halt state.
//!
//! The default run (no `--hd-0010-decided`) leaves the L3-HD0009 row Blocked,
//! so the halt is engaged; both ID lists derive from the same post-predicate
//! steps, so parity holds. A missing block is a typed refusal, not a pass.

use std::path::Path;
use std::process::Command;

fn ompo_start_json(repo: &Path, session: &str) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_ompo"))
        .args([
            "start",
            "--json",
            "--repo",
            repo.to_str().expect("fixture path is utf8"),
            "--session",
            session,
        ])
        .output()
        .expect("ompo start must launch");
    assert!(
        output.status.success(),
        "ompo start failed: {:?} stderr={:?}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("start emits JSON")
}

/// Typed refusal: a missing observability block is an ERROR with its own
/// reason, distinct from a value mismatch. A document naming a monitor
/// does not run it.
fn observability(report: &serde_json::Value) -> &serde_json::Value {
    report.get("data").and_then(|data| data.get("observability")).filter(|obs| !obs.is_null()).expect(
        "L3_OBSERVABILITY_MISSING: start --json carries no observability block",
    )
}

#[test]
fn start_json_reports_parity_and_halt() {
    let repo = tempfile::tempdir().expect("fixture repo");
    let report = ompo_start_json(repo.path(), "vyzr-leg");
    assert_eq!(report["command"], "start");
    assert_eq!(report["status"], "OK");
    let obs = observability(&report);
    assert_eq!(obs["parity_ok"], true, "id lists: {obs}");
    assert_eq!(
        obs["tui_ids"].as_array().map(Vec::len),
        obs["json_ids"].as_array().map(Vec::len),
        "both lists present with equal length: {obs}"
    );
    assert_eq!(obs["halt"]["engaged"], true);
    assert_eq!(obs["halt"]["step"], "L3-HD0009");
    assert_eq!(obs["halt"]["reason_code"], "HD-0009");
}

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
    ompo_start_json_with(repo, session, &[])
}

/// Same launch, with extra flags appended. Two-sided legs need the decided
/// side of a predicate, and a harness that can only produce one side cannot
/// tell a live function from a constant.
fn ompo_start_json_with(repo: &Path, session: &str, extra: &[&str]) -> serde_json::Value {
    let mut args = vec![
        "start",
        "--json",
        "--repo",
        repo.to_str().expect("fixture path is utf8"),
        "--session",
        session,
    ];
    args.extend_from_slice(extra);
    let output = Command::new(env!("CARGO_BIN_EXE_ompo"))
        .args(&args)
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
    // Halt first: a parity divergence must leave these green, proving the
    // mutation under grade touches only the ID paths, never the halt signal.
    assert_eq!(obs["halt"]["engaged"], true);
    assert_eq!(obs["halt"]["step"], "L3-HD0009");
    assert_eq!(obs["halt"]["reason_code"], "HD-0009");
    assert_eq!(obs["parity_ok"], true, "id lists: {obs}");
    assert_eq!(
        obs["tui_ids"].as_array().map(Vec::len),
        obs["json_ids"].as_array().map(Vec::len),
        "both lists present with equal length: {obs}"
    );
}

/// L3-OBS-COUNT (st8w): `step_count` lives INSIDE the observability block.
/// Known-bad is the bead's own: count 0 (or absent) while `ompo start` is
/// claimed wired. Cross-checked against the rendered arrays so a hardcoded
/// constant cannot satisfy it.
#[test]
fn observability_reports_step_count() {
    let repo = tempfile::tempdir().expect("fixture repo");
    let report = ompo_start_json(repo.path(), "st8w-leg");
    let obs = observability(&report);
    println!("READBACK .data.observability.step_count = {}", obs["step_count"]);
    let count = obs["step_count"]
        .as_u64()
        .expect("L3_STEP_COUNT_MISSING: observability carries no numeric step_count");
    assert!(count >= 1, "STEPS is non-empty, so count must be >=1: {obs}");
    assert_eq!(
        Some(count as usize),
        report["data"]["steps"].as_array().map(Vec::len),
        "step_count must equal the rendered steps array, not a constant"
    );
    assert_eq!(
        Some(count as usize),
        obs["tui_ids"].as_array().map(Vec::len),
        "step_count must equal the TUI id list length"
    );
}

/// L3-OBS-CURSOR (jb5m): `next_step_id` lives inside the observability block
/// and equals the TUI cursor. The TUI cursor is re-derived here from the
/// rendered rows -- first non-PASSED, non-SKIPPED in array order -- so the
/// writer cannot agree with itself by construction.
#[test]
fn observability_next_step_id_equals_tui_cursor() {
    let repo = tempfile::tempdir().expect("fixture repo");
    let report = ompo_start_json(repo.path(), "jb5m-leg");
    let obs = observability(&report);
    println!("READBACK .data.observability.next_step_id = {}", obs["next_step_id"]);
    let emitted = obs["next_step_id"]
        .as_str()
        .expect("L3_NEXT_STEP_ID_MISSING: observability carries no next_step_id");
    let rendered = report["data"]["steps"]
        .as_array()
        .expect("start renders a steps array");
    let tui_cursor = rendered
        .iter()
        .find(|step| {
            let status = step["status"].as_str().unwrap_or_default();
            status != "PASSED" && status != "SKIPPED"
        })
        .and_then(|step| step["id"].as_str())
        .expect("fixture leaves at least one non-elapsed step");
    assert_eq!(emitted, tui_cursor, "JSON next_step_id vs TUI cursor: {obs}");
}

/// L3-OBS-HD0009 (n5tt): `hd0009_status` lives inside the observability block
/// and reads `Blocked` while HD-0009 is undecided on this run. Tied to the
/// existing halt signal so the two cannot drift apart silently.
#[test]
fn observability_reports_hd0009_status() {
    let repo = tempfile::tempdir().expect("fixture repo");
    let report = ompo_start_json(repo.path(), "n5tt-leg");
    let obs = observability(&report);
    println!("READBACK .data.observability.hd0009_status = {}", obs["hd0009_status"]);
    assert_eq!(
        obs["hd0009_status"], "Blocked",
        "HD-0009 undecided on this run, so the step sits Blocked: {obs}"
    );
    assert_eq!(
        obs["hd0009_status"] == "Blocked",
        obs["halt"]["engaged"] == true,
        "hd0009_status and halt.engaged must not drift: {obs}"
    );
}

/// L3-OBS-HD0009 (n5tt), THE OTHER SIDE. Every Blocked-only assertion above is
/// satisfied by a field hardcoded to the string "Blocked" -- 5 passed, both
/// proof lines, and the value never connected to its cause. That is a SEMANTIC
/// vacuous green that no proof-line or `0 passed` rule detects.
///
/// So this leg drives the same binary one flag apart and requires the value to
/// MOVE with its named authority. A constant fails here by construction.
#[test]
fn hd0009_status_moves_with_the_decision_and_is_not_a_constant() {
    let repo = tempfile::tempdir().expect("fixture repo");
    let undecided = ompo_start_json(repo.path(), "n5tt-undecided");
    let decided = ompo_start_json_with(repo.path(), "n5tt-decided", &["--hd-0010-decided"]);
    let undecided_obs = observability(&undecided);
    let decided_obs = observability(&decided);
    println!(
        "READBACK undecided={} decided={}",
        undecided_obs["hd0009_status"], decided_obs["hd0009_status"]
    );
    assert_eq!(undecided_obs["hd0009_status"], "Blocked", "{undecided_obs}");
    assert_ne!(
        decided_obs["hd0009_status"], "Blocked",
        "a constant would read Blocked on BOTH runs: {decided_obs}"
    );
    assert_eq!(decided_obs["hd0009_status"], "Ready", "{decided_obs}");
    // The halt must follow the same authority, not just the status string.
    assert_eq!(undecided_obs["halt"]["engaged"], true, "{undecided_obs}");
    assert_eq!(decided_obs["halt"]["engaged"], false, "{decided_obs}");
}

/// Known-bad legs for the parity gate itself (yto0): the typed contract is
/// asserted directly, without running the binary. Misorder, absence, and
/// vacuity each refuse with their own variant.
#[test]
fn parity_gate_names_tui_only_and_refuses_empty() {
    use ompo_start::{check_id_parity, ParityMismatch};
    assert_eq!(check_id_parity(&["a", "b"], &["a", "b"]), Ok(()));
    assert_eq!(
        check_id_parity(&["a", "b"], &["b", "a"]),
        Err(ParityMismatch::TuiOnly {
            id: "a",
            tui_index: 0
        })
    );
    assert_eq!(
        check_id_parity(&["a"], &[]),
        Err(ParityMismatch::TuiOnly {
            id: "a",
            tui_index: 0
        })
    );
    assert_eq!(check_id_parity(&[], &[]), Err(ParityMismatch::Empty));
}

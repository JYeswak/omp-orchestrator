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

/// Write a decisions ledger into a fixture repo at the path the resolver reads.
fn write_ledger(repo: &std::path::Path, lines: &[&str]) {
    let docs = repo.join("docs");
    std::fs::create_dir_all(&docs).expect("fixture docs dir");
    std::fs::write(docs.join("decisions.jsonl"), format!("{}\n", lines.join("\n")))
        .expect("fixture ledger");
}

const HD0009_ASKED: &str =
    r#"{"id":"HD-0009","question":"which substrate?","decision":""}"#;
const HD0009_ANSWERED: &str =
    r#"{"answers":"HD-0009","decider":"Joshua","decision":"frankentui rust yes"}"#;

/// 812ax, THE WHOLE BEAD (item 3): with an ANSWERED HD-0009 row present and NO
/// flag, the status must NOT read Blocked. This leg fails against the previous
/// resolver by construction -- it never read the ledger, so no ledger content
/// could move the field.
#[test]
fn a_recorded_hd0009_decision_unblocks_the_step_without_any_flag() {
    let repo = tempfile::tempdir().expect("fixture repo");
    write_ledger(repo.path(), &[HD0009_ASKED, HD0009_ANSWERED]);
    let report = ompo_start_json(repo.path(), "812ax-recorded");
    let obs = observability(&report);
    println!(
        "READBACK status={} authority={} refusal={}",
        obs["hd0009_status"], obs["hd0009_authority"], obs["hd0009_refusal"]
    );
    assert_ne!(
        obs["hd0009_status"], "Blocked",
        "a recorded decision must not read Blocked: {obs}"
    );
    assert_eq!(obs["hd0009_status"], "Ready", "{obs}");
    assert_eq!(obs["hd0009_authority"], "ledger_decided", "{obs}");
    assert_eq!(obs["hd0009_decided"], true, "{obs}");
    assert!(obs["hd0009_refusal"].is_null(), "a reading is not a refusal: {obs}");
    // The halt must follow the ledger too, not just the status string.
    assert_eq!(obs["halt"]["engaged"], false, "{obs}");
}

/// ⛔ THE TRAP the resolver must not fall into: an ASKED-but-unanswered HD-0009
/// row makes `grep -c HD-0009` read 2 while nothing has been decided. A
/// presence predicate reports Ready here and is wrong.
#[test]
fn an_asked_but_unanswered_hd0009_row_does_not_unblock_the_step() {
    let repo = tempfile::tempdir().expect("fixture repo");
    write_ledger(repo.path(), &[HD0009_ASKED]);
    let obs_owner = ompo_start_json(repo.path(), "812ax-asked");
    let obs = observability(&obs_owner);
    println!("READBACK asked-only authority={}", obs["hd0009_authority"]);
    assert_eq!(obs["hd0009_status"], "Blocked", "{obs}");
    assert_eq!(obs["hd0009_authority"], "ledger_undecided", "{obs}");
    assert_eq!(obs["hd0009_decided"], false, "{obs}");
    assert!(obs["hd0009_refusal"].is_null(), "{obs}");
}

/// ANTI-VACUITY (item 6): an ABSENT ledger and a RECORDED non-decision agree on
/// the status string and MUST NOT share a representation. They differ in the
/// authority pair, which is the whole point of emitting it.
#[test]
fn an_absent_ledger_does_not_share_a_representation_with_a_recorded_non_decision() {
    let absent = tempfile::tempdir().expect("fixture repo");
    let recorded = tempfile::tempdir().expect("fixture repo");
    write_ledger(recorded.path(), &[HD0009_ASKED]);

    let absent_report = ompo_start_json(absent.path(), "812ax-absent");
    let recorded_report = ompo_start_json(recorded.path(), "812ax-recorded-none");
    let absent_obs = observability(&absent_report);
    let recorded_obs = observability(&recorded_report);
    println!(
        "READBACK absent={} recorded={}",
        absent_obs["hd0009_authority"], recorded_obs["hd0009_authority"]
    );

    // Both read Blocked, which is exactly why the status alone is insufficient.
    assert_eq!(absent_obs["hd0009_status"], "Blocked", "{absent_obs}");
    assert_eq!(recorded_obs["hd0009_status"], "Blocked", "{recorded_obs}");
    assert_ne!(
        absent_obs["hd0009_authority"], recorded_obs["hd0009_authority"],
        "a missing ledger and a recorded non-decision must be distinguishable"
    );
    assert_eq!(absent_obs["hd0009_authority"], "ledger_absent", "{absent_obs}");
    assert!(
        absent_obs["hd0009_refusal"]
            .as_str()
            .is_some_and(|d| d.contains("HD0009_LEDGER_ABSENT")),
        "an absent ledger names its refusal: {absent_obs}"
    );
}

/// PRECEDENCE (item 1) at the binary: the flag still decides, and it reports
/// ITSELF rather than borrowing the ledger's authority. Without this, an
/// operator assertion would be indistinguishable from a recorded decision --
/// the original defect, in the opposite direction.
#[test]
fn the_flag_reports_itself_as_an_override_and_not_as_ledger_provenance() {
    let repo = tempfile::tempdir().expect("fixture repo");
    write_ledger(repo.path(), &[HD0009_ASKED]);
    let report = ompo_start_json_with(repo.path(), "812ax-override", &["--hd-0010-decided"]);
    let obs = observability(&report);
    println!("READBACK override authority={}", obs["hd0009_authority"]);
    assert_eq!(obs["hd0009_status"], "Ready", "{obs}");
    assert_eq!(obs["hd0009_decided"], true, "{obs}");
    assert_eq!(obs["hd0009_authority"], "flag_override", "{obs}");
    assert_ne!(
        obs["hd0009_authority"], "ledger_decided",
        "an operator assertion must never be reported as provenance: {obs}"
    );
}

/// THE PIN: `hd0009_refusal` is non-null EXACTLY when the authority is one of
/// the refusal tokens, observed through the binary across a matrix carrying
/// BOTH polarities. Two encodings of one fact drift unless something holds them
/// together; a single-polarity matrix would satisfy this vacuously, so the
/// polarity anchors below are part of the assertion.
#[test]
fn hd0009_refusal_is_non_null_exactly_for_the_refusal_authorities() {
    let absent = tempfile::tempdir().expect("fixture repo");
    let asked = tempfile::tempdir().expect("fixture repo");
    write_ledger(asked.path(), &[HD0009_ASKED]);
    let answered = tempfile::tempdir().expect("fixture repo");
    write_ledger(answered.path(), &[HD0009_ASKED, HD0009_ANSWERED]);

    let reports = [
        ompo_start_json(absent.path(), "812ax-pin-absent"),
        ompo_start_json(asked.path(), "812ax-pin-asked"),
        ompo_start_json(answered.path(), "812ax-pin-answered"),
        ompo_start_json_with(asked.path(), "812ax-pin-override", &["--hd-0010-decided"]),
    ];
    let refusal_tokens = ["ledger_absent", "ledger_unreadable"];
    let mut saw_refusal = false;
    let mut saw_reading = false;
    for report in &reports {
        let obs = observability(report);
        let authority = obs["hd0009_authority"].as_str().expect("authority is a string");
        let is_refusal = refusal_tokens.contains(&authority);
        if is_refusal {
            saw_refusal = true;
        } else {
            saw_reading = true;
        }
        assert_eq!(
            !obs["hd0009_refusal"].is_null(),
            is_refusal,
            "refusal detail and authority must not disagree: {obs}"
        );
    }
    assert!(saw_refusal, "matrix must contain a refusal, else the pin is vacuous");
    assert!(saw_reading, "matrix must contain a reading, else the pin is vacuous");
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

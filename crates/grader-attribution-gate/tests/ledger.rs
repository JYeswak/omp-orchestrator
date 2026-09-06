//! Production scan is a shrinking ceiling. Detection is proven on a planted row.
//!
//! C38 still holds for the production count: the ceiling is measured from
//! `.beads/issues.jsonl`, not from the 23-row bead prose. The fixture below
//! only proves the detector fires.

use grader_attribution_gate::{
    attribution_hits, ledger_gate_exit, parse_closed_beads, unattributed_close_ids,
    CeilingVerdict, DEFAULT_AUTHORS, UNATTRIBUTED_CLOSE_CEILING,
};
use std::path::PathBuf;

fn production_jsonl() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.beads/issues.jsonl");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("production ledger unreadable at {}: {e}", path.display()))
}

const PLANTED_UNATTRIBUTED: &str = r#"{"id":"omp-orchestrator-planted-unattributed-gcyf","status":"closed","closed_at":"2026-09-06T00:00:00Z","close_reason":"MUTATION-VERIFIED by SnowyCanyon -- a NON-IMPLEMENTER","comments":[{"author":"josh","text":"closed without --actor"}]}"#;

#[test]
fn empty_ledger_is_an_error() {
    let error = parse_closed_beads("").expect_err("empty scan must not pass");
    assert!(error.to_string().contains("ATTRIBUTION_SCAN_EMPTY"));
    let error = parse_closed_beads("{\"status\":\"open\",\"id\":\"x\"}\n")
        .expect_err("zero closed rows must not pass");
    assert!(error.to_string().contains("ATTRIBUTION_SCAN_EMPTY"));
}

#[test]
fn detector_fires_on_a_planted_unattributed_close() {
    let rows = parse_closed_beads(PLANTED_UNATTRIBUTED).expect("planted closed row");
    let unattributed = unattributed_close_ids(&rows, DEFAULT_AUTHORS);
    assert_eq!(
        unattributed,
        vec!["omp-orchestrator-planted-unattributed-gcyf".to_owned()],
        "FIXTURE_PROVES_DETECTION: a josh-only close must be named"
    );
}

#[test]
fn production_live_count_must_not_exceed_the_ceiling() {
    let rows = parse_closed_beads(&production_jsonl()).expect("production has closed beads");
    let unattributed = unattributed_close_ids(&rows, DEFAULT_AUTHORS);
    let live = unattributed.len();
    let verdict = CeilingVerdict::from_counts(live, UNATTRIBUTED_CLOSE_CEILING);
    assert!(
        !verdict.refuses(),
        "CEILING_BREACHED live={live} ceiling={UNATTRIBUTED_CLOSE_CEILING} ids={unattributed:?}"
    );
    assert_eq!(ledger_gate_exit(&unattributed), 0);
}

#[test]
fn a_clean_ledger_stays_green() {
    assert_eq!(ledger_gate_exit(&[]), 0);
    assert!(!CeilingVerdict::from_counts(0, UNATTRIBUTED_CLOSE_CEILING).refuses());
}

#[test]
fn iis6_close_is_not_unattributed() {
    let rows = parse_closed_beads(&production_jsonl()).expect("production has closed beads");
    let unattributed = unattributed_close_ids(&rows, DEFAULT_AUTHORS);
    assert!(
        !unattributed.iter().any(|id| id == "omp-orchestrator-iis6"),
        "KNOWN-GOOD: iis6 carries WildStone --actor and must PASS"
    );
}

#[test]
fn leht_and_mj8w_have_attribution_hits() {
    let rows = parse_closed_beads(&production_jsonl()).expect("production has closed beads");
    let leht = rows
        .iter()
        .find(|row| row.id == "omp-orchestrator-leht")
        .expect("POSITIVE CONTROL leht must exist in the ledger");
    let mj8w = rows
        .iter()
        .find(|row| row.id == "omp-orchestrator-mj8w")
        .expect("POSITIVE CONTROL mj8w must exist in the ledger");
    let leht_hits = attribution_hits(&leht.comment_authors, &leht.close_reason, DEFAULT_AUTHORS);
    let mj8w_hits = attribution_hits(&mj8w.comment_authors, &mj8w.close_reason, DEFAULT_AUTHORS);
    assert!(
        leht_hits.iter().any(|h| h == "GreenFrog"),
        "POSITIVE CONTROL leht: {leht_hits:?}"
    );
    assert!(
        mj8w_hits.iter().any(|h| h == "AmberGate") || mj8w_hits.iter().any(|h| h == "BlueLantern"),
        "POSITIVE CONTROL mj8w: {mj8w_hits:?}"
    );
}

//! Production-ledger legs. C38: a fixture drifted from production certifies nothing.
//!
//! The 23-row measurement in `omp-orchestrator-gcyf` is the known-bad set. Some of
//! those rows have since gained a non-default author; the gate keys on the rows
//! that remain unattributed in `.beads/issues.jsonl` today.

use grader_attribution_gate::{
    attribution_hits, ledger_gate_exit, parse_closed_beads, unattributed_close_ids,
    DEFAULT_AUTHORS,
};
use std::path::PathBuf;

/// Original 23 IDs named in the bead. The scanner must still fire on those
/// that remain unattributed in production.
const ORIGINAL_UNATTRIBUTED: &[&str] = &[
    "omp-orchestrator-11ou",
    "omp-orchestrator-193",
    "omp-orchestrator-58u2",
    "omp-orchestrator-d6q2",
    "omp-orchestrator-ground-truth-contract-gm1",
    "omp-orchestrator-h5q",
    "omp-orchestrator-ii8",
    "omp-orchestrator-kxe.6",
    "omp-orchestrator-lifecycle-type-algebra-sxz",
    "omp-orchestrator-u622",
    "omp-orchestrator-yv5k",
];

fn production_jsonl() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.beads/issues.jsonl");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("production ledger unreadable at {}: {e}", path.display()))
}

#[test]
fn empty_ledger_is_an_error() {
    let error = parse_closed_beads("").expect_err("empty scan must not pass");
    assert!(error.to_string().contains("ATTRIBUTION_SCAN_EMPTY"));
    let error = parse_closed_beads("{\"status\":\"open\",\"id\":\"x\"}\n")
        .expect_err("zero closed rows must not pass");
    assert!(error.to_string().contains("ATTRIBUTION_SCAN_EMPTY"));
}

#[test]
fn production_unattributed_closes_are_named_and_exit_nonzero() {
    let rows = parse_closed_beads(&production_jsonl()).expect("production has closed beads");
    let unattributed = unattributed_close_ids(&rows, DEFAULT_AUTHORS);
    assert_eq!(
        ledger_gate_exit(&unattributed),
        1,
        "KNOWN-BAD: a real unattributed close must make the caller exit nonzero; named={unattributed:?}"
    );
    let still_bad: Vec<&str> = ORIGINAL_UNATTRIBUTED
        .iter()
        .copied()
        .filter(|id| unattributed.iter().any(|u| u == id))
        .collect();
    assert!(
        !still_bad.is_empty(),
        "KNOWN-BAD: none of the bead's measured unattributed IDs remain in the ledger scan; \
         scanner={unattributed:?}"
    );
    for id in &still_bad {
        assert!(
            unattributed.iter().any(|u| u == id),
            "KNOWN-BAD must name {id}"
        );
    }
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
    let unattributed = unattributed_close_ids(&rows, DEFAULT_AUTHORS);
    assert!(!unattributed.iter().any(|id| id == "omp-orchestrator-leht"));
    assert!(!unattributed.iter().any(|id| id == "omp-orchestrator-mj8w"));
}

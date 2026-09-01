#![forbid(unsafe_code)]
//! GAP PROPAGATION — a refutation that lands in one section is not propagated.
//!
//! Seven gaps this plan named as unsolved have upstream types in the tool we wrap.
//! A section arguing from a refuted absence, without naming the type that refutes it,
//! is stale. This gate measures that rather than letting a commit message assert it.
//!
//! It fails on a REGRESSION, not on the current backlog: `BASELINE` is the measured
//! count when this gate was written, and the gate refuses any increase.

use std::{fs, path::PathBuf};

/// (gap, needle proving the section knows, pattern showing it argues the absence)
const GAPS: &[(&str, &str, &[&str])] = &[
    ("completion", "AgentEndEvent",
     &["precedent-free", "no completion path", "cannot complete", "completion protocol"]),
    ("receipts", "IrcDeliveryReceipt",
     &["no receipt", "missing receipt", "cp-z42vu"]),
    ("claims", "ownershipToken",
     &["claim vocabulary", "unclaimed bead"]),
    ("idle", "GuestIdleReconcilerCtx",
     &["NewlyIdle", "ConfirmedIdle"]),
    ("roster", "HubRosterCounts", &["roster re-derived", "roster by hand"]),
    ("cost", "ContextUsage", &["cost is unmeasured", "no cost telemetry"]),
    ("compaction", "CompactEvent", &["85% context", "context was lost"]),
];

/// Measured 2026-08-31 at commit 7f7e0f6 **by the detector below**, not carried from
/// another instrument. The first value here was 24 — taken from a python scan whose
/// needles differ from these — which left an 11-pair slack window in which the gate
/// could not fire. That is the vacuity this file's own anti-vacuity assert exists to
/// prevent, committed by its author minutes after writing it.
///
/// Ratchet DOWN only. Raising it requires a measurement in the same commit.
const BASELINE: usize = 13;

fn plan_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
        .parent().unwrap().join("docs/plan")
}

fn stale_pairs() -> Vec<(String, &'static str)> {
    let dir = plan_dir();
    let mut out = Vec::new();
    let mut sections = 0usize;
    for e in fs::read_dir(&dir).expect("docs/plan must exist") {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if !name.ends_with(".md") || !name.chars().next().unwrap().is_ascii_digit() { continue; }
        sections += 1;
        let t = fs::read_to_string(&p).unwrap();
        for (gap, needle, stale) in GAPS {
            if t.contains(needle) { continue; }
            if stale.iter().any(|s| t.contains(s)) { out.push((name.clone(), *gap)); }
        }
    }
    // ANTI-VACUITY: an empty scan set is an ERROR, never a pass.
    assert!(sections >= 10, "scanned {sections} sections; the plan has 12 — scan set collapsed");
    out
}

#[test]
fn gap_propagation_does_not_regress() {
    let stale = stale_pairs();
    assert!(stale.len() <= BASELINE,
        "gap propagation REGRESSED: {} stale pairs, baseline {}\n{}",
        stale.len(), BASELINE,
        stale.iter().map(|(f,g)| format!("  {f} argues the {g} gap without naming its type"))
            .collect::<Vec<_>>().join("\n"));
}

#[test]
fn the_detector_finds_the_known_stale_pairs() {
    // KNOWN-BAD leg: 10-prior-art carries every type, so it must NEVER appear.
    // If it does, the needle matching is broken rather than the docs.
    let stale = stale_pairs();
    assert!(!stale.iter().any(|(f, _)| f.starts_with("10-")),
        "10-prior-art names all seven types and must never be flagged — detector is broken");
    assert!(!stale.is_empty(),
        "zero stale pairs while BASELINE is {BASELINE} — either the work is done \
         (lower BASELINE in the same commit) or the needles stopped matching");
}

#[test]
fn print_measured_count() {
    let s = stale_pairs();
    println!("MEASURED_BY_THIS_DETECTOR={}", s.len());
    for (f, g) in &s { println!("  {f} :: {g}"); }
}

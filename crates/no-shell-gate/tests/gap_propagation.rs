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
/// 13 (written) -> 6: SilverWolf (%1409) cleared all five pairs in 09-milestones.md and
/// 11-lifecycle.md (receipts/idle/claims named their upstream types at true strength),
/// MEASURED_BY_THIS_DETECTOR=6 at commit time; remaining six pairs are 00/01/05 (other panes).
const BASELINE: usize = 13;

// ── WHAT THIS NUMBER IS AND IS NOT ──────────────────────────────────────────
// It has been 24 -> 13 -> 6 -> 15 -> 13 in one session. Every move was a real
// measurement and every move came from changing the INSTRUMENT, not the docs:
//
//   24  a python scan's count, carried across (vacuous: 11-pair slack)
//   13  re-derived by this detector, file-grained suppression
//    6  %1409 cleared five pairs and re-measured in-commit (correct)
//   15  suppression narrowed to paragraph-local, exposing claims a
//       file-grained needle hid — including 11-lifecycle asserting
//       "precedent-free" past its own correction
//   13  retraction markers added so state-then-refute prose is not flagged
//
// THEREFORE: the absolute value is instrument-dependent and is NOT a count of
// defects in the world. This gate is a smoke alarm — it refuses regression
// under a FIXED instrument. Any commit changing GAPS, RETRACTION, or the window
// MUST re-derive BASELINE in the same commit and append a line above.
//
// The four 10-prior-art rows are known imprecision: that section states each
// dead claim in a table cell whose refutation is >2 paragraphs away. Tightening
// further starts fitting the detector to one file, which is how a gate stops
// measuring and starts agreeing.

/// A paragraph carrying one of these is *discussing* a dead claim, not making it.
///
/// Without this, `10-prior-art` — the section whose entire job is state-then-refute —
/// is flagged for all seven gaps it documents. Same principle the retired-figure gate
/// uses: a retraction names itself, and quoting a corpse is not reanimating it.
const RETRACTION: &[&str] = &[
    "REFUTED", "refuted", "retracted", "RETRACTED", "no longer", "was wrong",
    "corrected", "CORRECTED", "SETTLED", "WIRE-PROVEN", "superseded", "VOID",
];

fn plan_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
        .parent().unwrap().join("docs/plan")
}

/// The discriminator, isolated so it can be tested on known inputs rather than
/// on production files whose state I would then have to keep constant.
fn paragraph_is_stale(paras: &[&str], i: usize, needle: &str, stale: &[&str]) -> bool {
    let para = paras[i];
    if !stale.iter().any(|s| para.contains(s)) { return false; }
    if RETRACTION.iter().any(|m| para.contains(m)) { return false; }
    let lo = i.saturating_sub(2);
    let hi = (i + 3).min(paras.len());
    !paras[lo..hi].iter().any(|p| p.contains(needle))
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
        // PARAGRAPH-GRAINED, not file-grained. A file mentioning AgentEndEvent once was
        // previously treated as knowing everything, so `11-lifecycle` kept a row asserting
        // "precedent-free" past its own S6->S7 correction and this gate could not see it
        // (%1409, wave 8). Suppression must be local to the claim, not global to the file.
        let paras: Vec<&str> = t.split("\n\n").collect();
        for (gap, needle, stale) in GAPS {
            for i in 0..paras.len() {
                if !paragraph_is_stale(&paras, i, needle, stale) { continue; }
                out.push((name.clone(), *gap));
                break; // one pair per (file, gap)
            }
        }
    }
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
fn the_discriminator_separates_assertion_from_retraction() {
    let (gap_needle, gap_stale) = ("AgentEndEvent", ["precedent-free"]);

    // KNOWN-BAD: asserts the absence, type nowhere near it.
    let bad = ["intro", "the signal is precedent-free across the corpus", "unrelated"];
    assert!(paragraph_is_stale(&bad, 1, gap_needle, &gap_stale),
        "must flag a bare assertion of the absence");

    // KNOWN-GOOD 1: the type is named in the adjacent paragraph.
    let ok1 = ["intro", "the signal is precedent-free across the corpus",
               "AgentEndEvent closes it"];
    assert!(!paragraph_is_stale(&ok1, 1, gap_needle, &gap_stale),
        "must not flag a claim whose type is named next door");

    // KNOWN-GOOD 2: the paragraph is quoting the claim to refute it.
    let ok2 = ["intro", "we said precedent-free; that is REFUTED", "x"];
    assert!(!paragraph_is_stale(&ok2, 1, gap_needle, &gap_stale),
        "must not flag prose that retracts the claim it quotes");

    // KNOWN-GOOD 3: silence is not a finding.
    let ok3 = ["intro", "nothing relevant here", "x"];
    assert!(!paragraph_is_stale(&ok3, 1, gap_needle, &gap_stale),
        "must not flag a paragraph that makes no such claim");
}

#[test]
fn the_scan_is_not_vacuous() {
    assert!(!stale_pairs().is_empty(),
        "zero pairs while BASELINE is {BASELINE} — either the work is done \
         (lower BASELINE in the same commit) or the needles stopped matching");
}

#[test]
fn print_measured_count() {
    let s = stale_pairs();
    println!("MEASURED_BY_THIS_DETECTOR={}", s.len());
    for (f, g) in &s { println!("  {f} :: {g}"); }
}

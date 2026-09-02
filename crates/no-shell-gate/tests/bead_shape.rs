#![forbid(unsafe_code)]
//! BEAD SHAPE GATE — the label index must stay an index.
//!
//! # What went wrong, measured 2026-09-01
//!
//! `.beads/issues.jsonl` carried **119 distinct labels over 142 beads — one label
//! per 1.2 beads**, with **75 of the 119 used exactly once** and 29 beads carrying
//! no label at all. The reference corpus (166,757 beads / 150 repos) runs 1,314
//! labels over 166,757 beads: **one per 127**.
//!
//! At one-per-1.2 a label is an adjective, not an index. `bv -l <label>
//! --robot-insights` returns one bead or zero, and the `bv --severity warning`
//! alert surface has nothing to group by — which disables the *navigation* half of
//! the whole method while every individual bead still looks well-formed.
//!
//! Consolidation to an 18-label controlled taxonomy fixed it once. This gate is
//! what stops it growing back, because it grows back one well-intentioned
//! one-off label at a time and no single addition ever looks like the problem.
//!
//! # Contract
//!
//! - The allowlist lives in **`.beads/LABEL-TAXONOMY.md`**, in a fenced
//!   ```` ```taxonomy ```` block. This file does **not** contain a second copy.
//!   Two copies of an allowlist is two allowlists, and they drift.
//! - Every non-`tombstone` bead carries >= 1 label, and every label it carries is
//!   in the allowlist. Tombstones are excluded because `br update` refuses to
//!   mutate them (`cannot update tombstone issue`), so their labels are frozen
//!   and unfixable — three of them (`plan-derived`, `contract`, `convergence`)
//!   are still in the JSONL for exactly that reason.
//! - Two **ratchets**, both seeded from *this gate's own scan* at the moment the
//!   consolidation landed, and both of which may only fall. Seeding a ratchet from
//!   a neighbouring measurement (a `jq` count, a sibling test) is a measured
//!   defect from this same session: a ceiling seeded at 42 while the scan counted
//!   41 let the mutation probe PASS when it should have failed.
//! - **Anti-vacuity is an ERROR, not a skip.** A missing JSONL, an unparseable
//!   one, and a genuinely clean board are indistinguishable from a `return`. The
//!   sibling `bead_standard.rs` prints `SKIP:` and returns green in that case;
//!   this gate refuses to.
//!
//! Reads the committed JSONL rather than shelling out to `br` on purpose:
//! bead `omp-orchestrator-hermetic-detector-tests-47m` in this very tracker is a
//! defect filed against tests coupled to live tracker state. The artifact under
//! review is the tracked file, so the tracked file is what gets read.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Ratchets. Seeded from this gate's own scan, 2026-09-01. DOWN only.
// ---------------------------------------------------------------------------

/// Distinct labels in use across non-tombstone beads.
///
/// This is the sprawl guard. Adding a 19th label to the taxonomy AND applying it
/// takes the count to 19 and turns this RED, which forces the addition to be a
/// deliberate edit here rather than a side effect of filing one bead.
/// SEEDED FROM THIS GATE'S OWN SCAN, 2026-09-01. With the constant set to a `0`
/// sentinel the gate printed, verbatim:
/// `SCAN: distinct labels in use across live beads = 18 (ceiling 0)`.
/// 18 is that number. It is deliberately NOT the `jq` count taken alongside it:
/// a ceiling seeded from a neighbouring measurement sat one above the scan
/// earlier in this same session and let a mutation probe pass.
const DISTINCT_LABEL_CEILING: usize = 18;

/// Labels applied to exactly one non-tombstone bead.
///
/// A singleton label is the seed crystal of the 1-per-1.2 failure: it indexes
/// nothing, and its existence invites the next one. It is not zero today, and the
/// honest floor is what was measured, not what would be tidy.
/// SEEDED FROM THIS GATE'S OWN SCAN, 2026-09-01, which printed verbatim:
/// `SCAN: singleton labels = 1 (ceiling 0): security`.
/// `security` is the one: a single bead (commit-message backtick injection)
/// carries it. It stays because it is a corpus-standard category that future
/// beads land in, not an ad-hoc adjective — but a SECOND singleton turns this
/// RED, which is the point.
const SINGLETON_LABEL_CEILING: usize = 1;

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/<crate> has two ancestors")
        .to_path_buf()
}

/// The allowlist, parsed out of the fenced ```taxonomy block in
/// `.beads/LABEL-TAXONOMY.md`. Panics rather than defaulting: a gate that cannot
/// find its allowlist must not fall back to "allow everything".
fn taxonomy() -> BTreeSet<String> {
    let path = repo_root().join(".beads/LABEL-TAXONOMY.md");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "ANTI-VACUITY: cannot read the allowlist at {}: {e}. Without it this gate would \
             pass everything, which is worse than failing.",
            path.display()
        )
    });

    let mut labels = BTreeSet::new();
    let mut inside = false;
    let mut saw_block = false;
    for line in text.lines() {
        let t = line.trim();
        if !inside {
            if t == "```taxonomy" {
                inside = true;
                saw_block = true;
            }
            continue;
        }
        if t.starts_with("```") {
            inside = false;
            continue;
        }
        if t.is_empty() {
            continue;
        }
        assert!(
            labels.insert(t.to_owned()),
            "duplicate label `{t}` in the taxonomy block — the allowlist is not a set"
        );
    }

    assert!(
        saw_block,
        "ANTI-VACUITY: {} has no ```taxonomy fenced block. The allowlist moved or was \
         renamed; this gate is not silently permissive when that happens.",
        path.display()
    );
    assert!(
        !labels.is_empty(),
        "ANTI-VACUITY: the ```taxonomy block in {} is empty. An empty allowlist would fail \
         every bead, which reads like a catastrophe rather than a missing list.",
        path.display()
    );
    labels
}

struct Bead {
    id: String,
    status: String,
    labels: Vec<String>,
}

/// Parse `.beads/issues.jsonl`. Every failure mode is a panic with the reason
/// named, because each one is indistinguishable from "clean board" if swallowed.
fn beads() -> Vec<Bead> {
    let path = repo_root().join(".beads/issues.jsonl");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("ANTI-VACUITY: cannot read {}: {e}", path.display())
    });

    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line).unwrap_or_else(|e| {
            panic!(
                "{}:{}: not valid JSON ({e}). A tracker that does not parse is not a \
                 tracker that passes.",
                path.display(),
                n + 1
            )
        });
        let id = v["id"].as_str().unwrap_or_default().to_owned();
        assert!(
            !id.is_empty(),
            "{}:{}: bead record has no `id` — the parser is misaligned with the schema",
            path.display(),
            n + 1
        );
        out.push(Bead {
            id,
            status: v["status"].as_str().unwrap_or_default().to_owned(),
            labels: v["labels"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
        });
    }

    assert!(
        !out.is_empty(),
        "ANTI-VACUITY: {} parsed to ZERO beads. An empty scan set is an ERROR, never a pass: \
         a deliverable that was never checked reports exactly like one that passed.",
        path.display()
    );
    out
}

fn live(beads: &[Bead]) -> Vec<&Bead> {
    beads.iter().filter(|b| b.status != "tombstone").collect()
}

fn distinct_live_labels(beads: &[Bead]) -> BTreeMap<String, usize> {
    let mut m: BTreeMap<String, usize> = BTreeMap::new();
    for b in live(beads) {
        for l in &b.labels {
            *m.entry(l.clone()).or_default() += 1;
        }
    }
    m
}

// ---------------------------------------------------------------------------
// Anti-vacuity — runs first in spirit; every other test depends on it holding.
// ---------------------------------------------------------------------------

#[test]
fn the_scan_set_is_real() {
    let all = beads();
    let live = live(&all);
    let tax = taxonomy();

    assert!(
        !live.is_empty(),
        "ANTI-VACUITY: every bead is a tombstone. Nothing below would be checked, and all of \
         it would report green."
    );
    assert!(
        live.iter().any(|b| !b.labels.is_empty()),
        "ANTI-VACUITY: not one live bead carries a label. Either the `labels` key moved or the \
         index was wiped; both look like a pass to the tests below."
    );

    eprintln!(
        "SCAN: {} beads total, {} live (non-tombstone), {} tombstone, {} labels in the allowlist",
        all.len(),
        live.len(),
        all.len() - live.len(),
        tax.len()
    );
}

// ---------------------------------------------------------------------------
// KNOWN-GOOD leg — MANDATORY. The taxonomy as landed must pass.
// ---------------------------------------------------------------------------

#[test]
fn every_live_bead_carries_at_least_one_taxonomy_label() {
    let all = beads();
    let tax = taxonomy();

    let mut unlabelled = Vec::new();
    for b in live(&all) {
        if b.labels.is_empty() {
            unlabelled.push(b.id.clone());
        }
    }
    assert!(
        unlabelled.is_empty(),
        "{} live bead(s) carry NO label, so `bv -l` cannot reach them at all:\n  {}",
        unlabelled.len(),
        unlabelled.join("\n  ")
    );

    let mut offenders = Vec::new();
    for b in live(&all) {
        let bad: Vec<&str> = b
            .labels
            .iter()
            .filter(|l| !tax.contains(l.as_str()))
            .map(String::as_str)
            .collect();
        if !bad.is_empty() {
            offenders.push(format!("{}: {}", b.id, bad.join(", ")));
        }
    }
    assert!(
        offenders.is_empty(),
        "{} bead(s) carry a label outside the controlled taxonomy in \
         .beads/LABEL-TAXONOMY.md. Add a mapping row and reuse an existing label, or make the \
         addition a deliberate taxonomy change — do NOT let a one-off label in, because that \
         is precisely how 119 labels happened:\n  {}",
        offenders.len(),
        offenders.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// Ratchets
// ---------------------------------------------------------------------------

#[test]
fn distinct_label_count_does_not_grow() {
    let all = beads();
    let counts = distinct_live_labels(&all);
    let n = counts.len();
    eprintln!("SCAN: distinct labels in use across live beads = {n} (ceiling {DISTINCT_LABEL_CEILING})");
    assert!(
        n <= DISTINCT_LABEL_CEILING,
        "{n} distinct labels in use across live beads; the ratchet ceiling is \
         {DISTINCT_LABEL_CEILING} and may only FALL. In use:\n  {}\n\nIf this addition is \
         intended, edit DISTINCT_LABEL_CEILING and .beads/LABEL-TAXONOMY.md in the same commit \
         and say why. If it is not, you are re-growing the one-label-per-1.2-beads index that \
         made `bv -l` useless.",
        counts
            .iter()
            .map(|(k, v)| format!("{k} ({v})"))
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

#[test]
fn singleton_labels_do_not_multiply() {
    let all = beads();
    let counts = distinct_live_labels(&all);
    let singles: Vec<&str> = counts
        .iter()
        .filter(|(_, v)| **v == 1)
        .map(|(k, _)| k.as_str())
        .collect();
    eprintln!(
        "SCAN: singleton labels = {} (ceiling {SINGLETON_LABEL_CEILING}): {}",
        singles.len(),
        if singles.is_empty() { "-".to_owned() } else { singles.join(", ") }
    );
    assert!(
        singles.len() <= SINGLETON_LABEL_CEILING,
        "{} label(s) apply to exactly one live bead (ceiling {SINGLETON_LABEL_CEILING}, DOWN \
         only): {}. A label with one carrier indexes nothing — 75 of our original 119 were \
         singletons. Either the concept deserves siblings or it belongs in the bead body.",
        singles.len(),
        singles.join(", ")
    );
}

// ---------------------------------------------------------------------------
// The allowlist itself
// ---------------------------------------------------------------------------

#[test]
fn the_allowlist_is_small_enough_to_be_an_index() {
    let tax = taxonomy();
    eprintln!("SCAN: allowlist size = {}", tax.len());
    assert!(
        tax.len() <= 25,
        "{} labels in the taxonomy. Past ~25 the allowlist stops being a controlled \
         vocabulary and becomes a record of everything anyone ever wanted to say.",
        tax.len()
    );
    for l in &tax {
        assert!(
            l.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "taxonomy label `{l}` is not lowercase-kebab; `gates` vs `gate` and `S3` vs `s3` \
             are how a controlled vocabulary silently forks"
        );
    }
}

#[test]
fn every_allowlist_label_is_actually_used() {
    // An allowlist entry with no carrier is aspiration, not vocabulary — and it
    // is the one direction the ratchets above cannot see.
    let all = beads();
    let counts = distinct_live_labels(&all);
    let unused: Vec<String> = taxonomy()
        .iter()
        .filter(|l| !counts.contains_key(l.as_str()))
        .cloned()
        .collect();
    assert!(
        unused.is_empty(),
        "{} allowlist label(s) are carried by no live bead: {}. Remove them or apply them; a \
         vocabulary you do not speak is not enforcing anything.",
        unused.len(),
        unused.join(", ")
    );
}

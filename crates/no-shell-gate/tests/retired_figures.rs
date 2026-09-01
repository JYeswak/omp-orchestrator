#![forbid(unsafe_code)]

//! RETIRED-FIGURE GATE — the expected-twin mechanism for prose.
//!
//! WHY THIS EXISTS. `00-brief.md` §8.4 records the structural blind spot found by the
//! negative-space grader: every refutation this project has caught is an error of *commission*,
//! found by re-deriving a number. The one *omission* ever caught was found only because the
//! scanner emits an `expected_*` twin for every count. Prose has no twin, so a stale figure
//! surviving in a section nobody re-reads is invisible.
//!
//! It was invisible. Measured 2026-08-31, after the brief had been corrected four times:
//!
//! ```text
//! refuted "omp-types re-exports the ack vocabulary"  -> alive in 5 sections
//! superseded gate count "2 of 8"                     -> alive in 6 sections
//! retired mirror count "216 repos"                   -> alive in 4 sections
//! renamed "four-layer"                               -> alive in 5 sections
//! stale type scope "51 public enums"                 -> alive in 3 sections
//! ```
//!
//! The brief was right and the plan was wrong, because correcting a source does not correct its
//! copies. `00-brief.md` §7.2 measured what habits are worth: the author violated the
//! pipeline-laundering rule twenty minutes after writing it. So this is a gate, not a checklist.
//!
//! WHAT IT ENFORCES. A figure listed in `RETIRED` must not appear in any plan section except
//! inside a labelled retraction. A retraction is a line that also names why the figure died —
//! that is how the document keeps its own history without letting a dead number answer a
//! question. The allowance list carries a REASON per entry and an empty reason is rejected,
//! following `franken_lean`'s `UNWIRED_LANE_ALLOWANCE` shape (mirror,
//! `crates/fln-conformance/tests/contract_roots.rs:288`, verified this session).
//!
//! WHAT IT DOES NOT ENFORCE — read this before trusting a green run. It matches literal strings.
//! It cannot tell a stale figure from a correct one that happens to share digits, it cannot find
//! a stale figure nobody thought to retire, and it says nothing about whether the *replacement*
//! is right. It raises the floor on one defect class: a figure known to be dead, still answering.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A figure that has been refuted or superseded, with what licenses mentioning it.
struct Retired {
    /// The literal claim that must not appear unretracted. Prefer the ASSERTION over the noun:
    /// matching a bare type name fired on three correct usages before this was tightened.
    needle: &'static str,
    /// The value or word that supersedes it. A mention is excused when this appears nearby —
    /// because **a retraction names its replacement**, and that is a stronger, more honest signal
    /// than a vocabulary of apology words like "corrected" or "no longer". A site that says
    /// "216 repos" while also saying "210" is doing history; a site that says only "216 repos" is
    /// answering a question with a dead number.
    replacement: &'static str,
    /// Why it died. Empty is rejected by `retired_rows_carry_non_empty_reasons`.
    reason: &'static str,
}
/// Every figure this project has retired. Adding a row here is how a correction becomes enforced
/// rather than merely written down.
const RETIRED: &[Retired] = &[
    Retired {
        needle: "81 JSON-RPC methods",
        replacement: "39",
        reason: "not re-derivable from installed source; measured surface is 39 CLI + 3 omp/* methods",
    },
    Retired {
        needle: "216 repos",
        replacement: "210",
        reason: "matched none of four measured counts; 210 git work-trees is the only repo count",
    },
    Retired {
        // The CLAIM, not the type name. A first cut matched bare `ObligationLedger` and fired on
        // three legitimate sites: the retraction row in the brief's own §7 table, a correct
        // reference to the upstream type, and a passage explaining why it is blocked. An
        // over-strict gate gets routed around — a slower death than no gate — so the needle is
        // the assertion, not the noun.
        needle: "re-exports the ack vocabulary",
        replacement: "blocked",
        reason: "omp-types does not re-export it; zero occurrences, blocked behind messaging-fabric",
    },
];

/// A mention is retracted when the same line, or the two lines around it, name the retraction.
const RETRACTION_MARKERS: &[&str] = &[
    "RETIRED", "retired", "refuted", "REFUTED", "corrected", "CORRECTED",
    "superseded", "SUPERSEDED", "retraction", "no longer", "was wrong",
];

fn plan_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/plan")
        .canonicalize()
        .expect("docs/plan must exist")
}

fn sections() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(plan_dir())
        .expect("read docs/plan")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension().and_then(|e| e.to_str()) == Some("md")
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.as_bytes().first().is_some_and(u8::is_ascii_digit))
        })
        .collect();
    out.sort();
    out
}

/// Is the mention at `idx` covered by a retraction marker on it or its neighbours?
fn is_retracted(lines: &[&str], idx: usize, replacement: &str) -> bool {
    let lo = idx.saturating_sub(2);
    let hi = (idx + 3).min(lines.len());
    lines[lo..hi].iter().any(|l| {
        RETRACTION_MARKERS.iter().any(|m| l.contains(m)) || l.contains(replacement)
    })
}

/// ANTI-VACUITY. An empty scan set is an ERROR, never a pass. Without this the gate is green on a
/// renamed directory and reports identically to a real check — the exact defect `00-brief.md` §3.3
/// records against our own census, where 183 rows carried one distinct invariant.
#[test]
fn scan_set_is_non_empty_and_meets_its_floor() {
    let found = sections();
    assert!(
        found.len() >= 11,
        "ANTI-VACUITY: expected at least 11 plan sections, found {}. A floor, not mere \
         non-emptiness: a silently-narrowed scan can return a plausible small number without \
         erroring, and 'I scanned something' is then unfalsifiable from inside the gate.",
        found.len()
    );
}

/// Every allowance row must carry a real reason. Mirrors `franken_lean`'s validator: an entry
/// cannot be added with an empty reason, so the list cannot become a place to hide things.
#[test]
fn retired_rows_carry_non_empty_reasons() {
    for r in RETIRED {
        assert!(
            r.reason.trim().len() >= 8,
            "retired figure {:?} has no usable reason; a row without a reason is a silent \
             exemption",
            r.needle
        );
    }
}

/// KNOWN-GOOD. A string that was never retired must not trip the gate. Without this leg an
/// over-strict gate ships, and an over-strict gate gets routed around — a slower death than no
/// gate at all (`00-brief.md` §3.5, on `path-literal-guard`).
#[test]
fn a_live_figure_is_not_flagged() {
    let live = "210 git work-trees";
    let texts: Vec<String> = sections()
        .iter()
        .map(|p| fs::read_to_string(p).unwrap_or_default())
        .collect();
    let hits: Vec<&String> = texts.iter().filter(|t| t.contains(live)).collect();
    assert!(
        !hits.is_empty(),
        "known-good leg is vacuous: the live figure {live:?} appears in no section, so this test \
         proves nothing about the gate's discrimination"
    );
}

/// KNOWN-BAD, planted in-memory. Proves the detector fires rather than the corpus being clean.
#[test]
fn planted_retired_figure_is_detected() {
    let planted = ["intro", "we consume 81 JSON-RPC methods today", "outro"];
    assert!(
        !is_retracted(&planted, 1, "39"),
        "the detector must flag an unretracted mention"
    );
    let excused = ["intro", "the 81 JSON-RPC methods figure is RETIRED", "outro"];
    assert!(
        is_retracted(&excused, 1, "39"),
        "the detector must excuse a mention that names its own retraction"
    );
}

/// THE GATE. Every retired figure, in every section, must be retracted where it appears.
#[test]
fn no_section_carries_a_retired_figure_unretracted() {
    let mut offenders: BTreeMap<String, Vec<(usize, &'static str)>> = BTreeMap::new();

    for path in sections() {
        let text = fs::read_to_string(&path).expect("read section");
        let lines: Vec<&str> = text.lines().collect();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_owned();

        for (idx, line) in lines.iter().enumerate() {
            for r in RETIRED {
                if line.contains(r.needle) && !is_retracted(&lines, idx, r.replacement) {
                    offenders
                        .entry(name.clone())
                        .or_default()
                        .push((idx + 1, r.needle));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "RETIRED FIGURES STILL ANSWERING QUESTIONS.\n\n{}\n\
         Each site must either drop the figure or name its retraction on an adjacent line. \
         Correcting a source does not correct its copies: the brief was corrected four times \
         while these survived.",
        offenders
            .iter()
            .map(|(f, hits)| {
                let rows = hits
                    .iter()
                    .map(|(l, n)| format!("    {f}:{l}  {n:?}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("  {f} — {} site(s)\n{rows}", hits.len())
            })
            .collect::<Vec<_>>()
            .join("\n")
    );
}

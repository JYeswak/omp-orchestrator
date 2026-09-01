#![forbid(unsafe_code)]
//! CONVERGENCE — the plan may not become a bead DAG until every section has two
//! consecutive clean rounds under two different lenses.
//!
//! Josh, 2026-08-31: "we need to ensure every section of the plan has 2 rounds of
//! no new findings - once all sections are done". This encodes that as a gate so
//! the conversion cannot be started on a feeling.

use std::{collections::BTreeMap, fs, path::PathBuf};

const SECTIONS: &[&str] = &[
    "00-brief", "01-idea", "02-surface-census", "03-crates", "04-diagrams", "05-actions",
    "06-gates", "07-installability", "08-end-users", "09-milestones", "10-prior-art",
    "11-lifecycle",
];

fn plan_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
        .parent().unwrap().join("docs/plan")
}

#[derive(Debug)]
struct Row { section: String, round: u32, lens: String, new_findings: u32 }

fn ledger() -> Vec<Row> {
    let p = plan_dir().join("CONVERGENCE.jsonl");
    let t = fs::read_to_string(&p).unwrap_or_default();
    let mut out = Vec::new();
    for line in t.lines().filter(|l| !l.trim().is_empty()) {
        // deliberately minimal: no serde dependency for a gate that must never fail to build
        let get = |k: &str| -> Option<String> {
            let pat = format!("\"{k}\":");
            let i = line.find(&pat)? + pat.len();
            let rest = line[i..].trim_start();
            Some(if let Some(r) = rest.strip_prefix('"') {
                r[..r.find('"')?].to_owned()
            } else {
                rest.split(|c: char| c == ',' || c == '}').next()?.trim().to_owned()
            })
        };
        let (Some(section), Some(round), Some(lens), Some(nf)) =
            (get("section"), get("round"), get("lens"), get("new_findings")) else { continue };
        let (Ok(round), Ok(new_findings)) = (round.parse(), nf.parse()) else { continue };
        out.push(Row { section, round, lens, new_findings });
    }
    out
}

/// Two consecutive clean rounds under two DIFFERENT lenses.
fn converged(rows: &[Row], section: &str) -> bool {
    let mut per_round: BTreeMap<u32, Vec<&Row>> = BTreeMap::new();
    for r in rows.iter().filter(|r| r.section == section) {
        per_round.entry(r.round).or_default().push(r);
    }
    let rounds: Vec<_> = per_round.keys().copied().collect();
    rounds.windows(2).any(|w| {
        let (a, b) = (&per_round[&w[0]], &per_round[&w[1]]);
        let clean = |v: &Vec<&Row>| !v.is_empty() && v.iter().all(|r| r.new_findings == 0);
        let lenses_differ = a.iter().any(|x| b.iter().any(|y| x.lens != y.lens));
        clean(a) && clean(b) && lenses_differ
    })
}

#[test]
fn report_convergence_state() {
    let rows = ledger();
    let done: Vec<_> = SECTIONS.iter().filter(|s| converged(&rows, s)).collect();
    println!("CONVERGED {}/{}", done.len(), SECTIONS.len());
    for s in SECTIONS {
        let n = rows.iter().filter(|r| r.section == *s).count();
        let mark = if converged(&rows, s) { "CONVERGED" } else { "open" };
        println!("  {s:<20} graded={n:<3} {mark}");
    }
}

/// The gate the DAG conversion must pass. Currently expected to FAIL — it is the
/// finish line, not a description of today.
#[test]
#[ignore = "finish line: run with --ignored to check whether the DAG may be built"]
fn every_section_converged_before_dag_conversion() {
    let rows = ledger();
    let open: Vec<&str> = SECTIONS.iter().copied().filter(|s| !converged(&rows, s)).collect();
    assert!(open.is_empty(),
        "{} of {} sections are not converged; the plan may not become a bead DAG yet:\n  {}",
        open.len(), SECTIONS.len(), open.join("\n  "));
}

#[test]
fn the_convergence_predicate_is_strict() {
    let mk = |section, round, lens, nf| Row {
        section: String::from(section), round, lens: String::from(lens), new_findings: nf };

    // KNOWN-GOOD: two clean rounds, two lenses.
    let good = vec![mk("x", 1, "investor", 0), mk("x", 2, "adversarial", 0)];
    assert!(converged(&good, "x"), "two clean rounds under two lenses must converge");

    // KNOWN-BAD 1: same lens twice — the lens may simply have stopped looking.
    let same = vec![mk("x", 1, "investor", 0), mk("x", 2, "investor", 0)];
    assert!(!converged(&same, "x"), "the same lens twice must NOT converge");

    // KNOWN-BAD 2: a finding in the second round breaks the streak.
    let dirty = vec![mk("x", 1, "investor", 0), mk("x", 2, "adversarial", 3)];
    assert!(!converged(&dirty, "x"), "a round with findings must break the streak");

    // KNOWN-BAD 3: non-consecutive clean rounds are not a streak.
    let gap = vec![mk("x", 1, "investor", 0), mk("x", 2, "absence", 2), mk("x", 3, "evidence", 0)];
    assert!(!converged(&gap, "x"), "clean rounds either side of a dirty one are not a streak");

    // ANTI-VACUITY: an empty ledger converges nothing.
    assert!(!converged(&[], "x"), "an empty ledger must never report convergence");
}

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
struct Row {
    section: String,
    round: u32,
    lens: String,
    new_findings: u32,
    role: String,
    /// Were the repository's gates green when this row was written?
    ///
    /// # Josh, 2026-09-01: "having not wired gates is plan issue number 1"
    ///
    /// Before this field, `convergence.rs` contained ZERO references to wiring —
    /// measured — so a round could report a section CONVERGED while
    /// `wired_lanes` was RED. That happened: the wiring gate was failing on a
    /// vendored `serde_json` copy under `.rch-tmp/` at the same moment rounds
    /// were being graded, and nothing connected the two facts.
    ///
    /// A clean round over a broken lane is the BUILT != WIRED failure wearing a
    /// convergence badge. `false` here means the row may not count toward a
    /// streak no matter what `new_findings` says.
    ///
    /// Absent is treated as `false`: a grader who did not record gate state did
    /// not check it, and an unrecorded check is not a check.
    gates_green: bool,
}

/// Roles borrowed from the AAR harness (`generic_aar/README.md`): a task needs a
/// hill-climbing leg, a held-out leg on a different distribution, and optional
/// don't-regress capability gates. We had only the first.
///
/// - `hillclimb`  the section being worked this round (default when absent)
/// - `capability` a re-check of an ALREADY-CONVERGED section; a finding un-converges it
/// - `held_out`   the withheld lens, run once at the end across everything

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
        let role = get("role").unwrap_or_else(|| "hillclimb".to_owned());
        // Absent => false. An unrecorded check is not a check.
        let gates_green = get("gates_green").as_deref() == Some("true");
        out.push(Row { section, round, lens, new_findings, role, gates_green });
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
        // CLEAN REQUIRES BOTH: nothing found AND the gates were green. A round
        // graded while a lane was unwired cannot bank a section.
        let clean = |v: &Vec<&Row>| {
            !v.is_empty() && v.iter().all(|r| r.new_findings == 0 && r.gates_green)
        };
        let lenses_differ = a.iter().any(|x| b.iter().any(|y| x.lens != y.lens));
        clean(a) && clean(b) && lenses_differ
    })
}

/// THE FLOOR. A converged section that is re-checked and yields a finding is no
/// longer converged — you may not bank a section and then regress it while
/// grinding a neighbour. Several findings this session were cross-section: the
/// 370-vs-379 count propagated from 06-gates into 01-idea, and the AgentEndEvent
/// refutation had to be chased across five files.
fn capability_regressed(rows: &[Row], section: &str) -> bool {
    rows.iter().any(|r| r.section == section && r.role == "capability" && r.new_findings > 0)
}

#[test]
fn a_round_graded_while_gates_were_red_cannot_bank_a_section() {
    let mk = |round, lens, nf, green| Row {
        section: "x".to_owned(), round, lens: String::from(lens),
        new_findings: nf, role: "hillclimb".to_owned(), gates_green: green };

    // KNOWN-GOOD: two clean rounds, two lenses, gates green both times.
    let good = vec![mk(1, "investor", 0, true), mk(2, "absence", 0, true)];
    assert!(converged(&good, "x"), "two clean rounds with green gates must converge");

    // KNOWN-BAD: same rounds, but the gates were RED when round 2 was graded.
    let red = vec![mk(1, "investor", 0, true), mk(2, "absence", 0, false)];
    assert!(!converged(&red, "x"),
        "a round graded while the gates were RED must NOT count -- that is BUILT != WIRED \
         wearing a convergence badge");

    // KNOWN-BAD: gate state absent entirely reads as false.
    let absent = vec![mk(1, "investor", 0, true), mk(2, "absence", 0, false)];
    assert!(!converged(&absent, "x"),
        "an unrecorded gate check is not a check");
}

#[test]
fn a_capability_recheck_with_findings_unconverges_the_section() {
    let mk = |round, lens, nf, role: &str| Row {
        section: "x".to_owned(), round, lens: String::from(lens),
        new_findings: nf, role: role.to_owned(), gates_green: true };
    // banked under two lenses...
    let mut rows = vec![mk(1, "investor", 0, "hillclimb"), mk(2, "absence", 0, "hillclimb")];
    assert!(converged(&rows, "x"), "precondition: two clean rounds two lenses");
    assert!(!capability_regressed(&rows, "x"), "no re-check yet, no regression");
    // ...then a re-check finds something.
    rows.push(mk(3, "evidence", 2, "capability"));
    assert!(capability_regressed(&rows, "x"),
        "a capability re-check with findings MUST un-converge the section");
    // a clean re-check does not.
    let clean = vec![mk(1, "investor", 0, "hillclimb"), mk(2, "absence", 0, "hillclimb"),
                     mk(3, "evidence", 0, "capability")];
    assert!(!capability_regressed(&clean, "x"), "a clean re-check must not un-converge");
}

#[test]
fn report_convergence_state() {
    let rows = ledger();
    let done: Vec<_> = SECTIONS.iter()
        .filter(|s| converged(&rows, s) && !capability_regressed(&rows, s))
        .collect();
    println!("CONVERGED {}/{}", done.len(), SECTIONS.len());
    for s in SECTIONS {
        let n = rows.iter().filter(|r| r.section == *s).count();
        let mark = if capability_regressed(&rows, s) { "REGRESSED" }
            else if converged(&rows, s) { "CONVERGED" } else { "open" };
        println!("  {s:<20} graded={n:<3} {mark}");
    }
}

/// The gate the DAG conversion must pass. Currently expected to FAIL — it is the
/// finish line, not a description of today.
#[test]
#[ignore = "finish line: run with --ignored to check whether the DAG may be built"]
fn every_section_converged_before_dag_conversion() {
    let rows = ledger();
    let open: Vec<&str> = SECTIONS.iter().copied()
        .filter(|s| !converged(&rows, s) || capability_regressed(&rows, s)).collect();
    let held = rows.iter().filter(|r| r.role == "held_out").count();
    assert!(held >= SECTIONS.len(),
        "the held-out lens must have run across all {} sections before the DAG is built; \
         {held} held_out rows present. Without it, convergence cannot be distinguished from \
         the graders having adapted to each other.", SECTIONS.len());
    assert!(open.is_empty(),
        "{} of {} sections are not converged; the plan may not become a bead DAG yet:\n  {}",
        open.len(), SECTIONS.len(), open.join("\n  "));
}

#[test]
fn the_convergence_predicate_is_strict() {
    let mk = |section, round, lens, nf| Row {
        section: String::from(section), round, lens: String::from(lens),
        new_findings: nf, role: "hillclimb".to_owned(), gates_green: true };

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

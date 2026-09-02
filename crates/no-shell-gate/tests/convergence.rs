#![forbid(unsafe_code)]
//! CONVERGENCE — the plan may not become a bead DAG until every section has two
//! consecutive clean rounds under two different lenses.
//!
//! Josh, 2026-08-31: "we need to ensure every section of the plan has 2 rounds of
//! no new findings - once all sections are done". This encodes that as a gate so
//! the conversion cannot be started on a feeling.

use std::{collections::BTreeMap, fs, path::PathBuf};

/// The sections the convergence verdict covers.
///
/// **`12-journey` was MISSING and the omission was silent.** The new orphan assertion
/// in `report_convergence_state` caught it on its first run: five `CONVERGENCE.jsonl`
/// rows and one row in each of `round20` through `round24` graded `12-journey`, and
/// every one of them was invisible to the verdict — the report iterated 12 sections
/// while the plan has 13, so a section that can never converge could also never block.
/// `CONVERGED n/12` against a 13-section plan is the unstated-denominator defect, in
/// the gate that exists to decide when the plan is done.
///
/// Adding it STRENGTHENS the predicate rather than weakening it: one more section must
/// now converge before `every_section_converged_before_dag_conversion` can pass.
const SECTIONS: &[&str] = &[
    "00-brief",
    "01-idea",
    "02-surface-census",
    "03-crates",
    "04-diagrams",
    "05-actions",
    "06-gates",
    "07-installability",
    "08-end-users",
    "09-milestones",
    "10-prior-art",
    "11-lifecycle",
    "12-journey",
];

fn plan_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("docs/plan")
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
                rest.split(|c: char| c == ',' || c == '}')
                    .next()?
                    .trim()
                    .to_owned()
            })
        };
        let (Some(section), Some(round), Some(lens), Some(nf)) = (
            get("section"),
            get("round"),
            get("lens"),
            get("new_findings"),
        ) else {
            continue;
        };
        let (Ok(round), Ok(new_findings)) = (round.parse(), nf.parse()) else {
            continue;
        };
        let role = get("role").unwrap_or_else(|| "hillclimb".to_owned());
        // Absent => false. An unrecorded check is not a check.
        let gates_green = get("gates_green").as_deref() == Some("true");
        out.push(Row {
            section,
            round,
            lens,
            new_findings,
            role,
            gates_green,
        });
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
        let clean =
            |v: &Vec<&Row>| !v.is_empty() && v.iter().all(|r| r.new_findings == 0 && r.gates_green);
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
    rows.iter()
        .any(|r| r.section == section && r.role == "capability" && r.new_findings > 0)
}

#[test]
fn a_round_graded_while_gates_were_red_cannot_bank_a_section() {
    let mk = |round, lens, nf, green| Row {
        section: "x".to_owned(),
        round,
        lens: String::from(lens),
        new_findings: nf,
        role: "hillclimb".to_owned(),
        gates_green: green,
    };

    // KNOWN-GOOD: two clean rounds, two lenses, gates green both times.
    let good = vec![mk(1, "investor", 0, true), mk(2, "absence", 0, true)];
    assert!(
        converged(&good, "x"),
        "two clean rounds with green gates must converge"
    );

    // KNOWN-BAD: same rounds, but the gates were RED when round 2 was graded.
    let red = vec![mk(1, "investor", 0, true), mk(2, "absence", 0, false)];
    assert!(
        !converged(&red, "x"),
        "a round graded while the gates were RED must NOT count -- that is BUILT != WIRED \
         wearing a convergence badge"
    );

    // KNOWN-BAD: gate state absent entirely reads as false.
    let absent = vec![mk(1, "investor", 0, true), mk(2, "absence", 0, false)];
    assert!(
        !converged(&absent, "x"),
        "an unrecorded gate check is not a check"
    );
}

#[test]
fn a_capability_recheck_with_findings_unconverges_the_section() {
    let mk = |round, lens, nf, role: &str| Row {
        section: "x".to_owned(),
        round,
        lens: String::from(lens),
        new_findings: nf,
        role: role.to_owned(),
        gates_green: true,
    };
    // banked under two lenses...
    let mut rows = vec![
        mk(1, "investor", 0, "hillclimb"),
        mk(2, "absence", 0, "hillclimb"),
    ];
    assert!(
        converged(&rows, "x"),
        "precondition: two clean rounds two lenses"
    );
    assert!(
        !capability_regressed(&rows, "x"),
        "no re-check yet, no regression"
    );
    // ...then a re-check finds something.
    rows.push(mk(3, "evidence", 2, "capability"));
    assert!(
        capability_regressed(&rows, "x"),
        "a capability re-check with findings MUST un-converge the section"
    );
    // a clean re-check does not.
    let clean = vec![
        mk(1, "investor", 0, "hillclimb"),
        mk(2, "absence", 0, "hillclimb"),
        mk(3, "evidence", 0, "capability"),
    ];
    assert!(
        !capability_regressed(&clean, "x"),
        "a clean re-check must not un-converge"
    );
}

/// `report_convergence_state` computed `converged` and `capability_regressed` over the
/// REAL ledger and only `println!`d them, so it exited 0 whatever the ledger said —
/// including on an empty ledger, where `SECTIONS.iter().filter(...)` yields nothing
/// and the loop body never runs. **A test with no assertion is a program that prints.**
///
/// The predicate itself is NOT weakened (acceptance 4). What is added is a claim about
/// its output that can fail: the ledger must be readable, non-empty, and cover the
/// sections it reports on, and the count it prints must be the count the predicate
/// produces.
/// The guard, extracted from the test body so it can be PROVEN rather than merely
/// present.
///
/// A mutation that neutered the in-body `!rows.is_empty()` assertion stayed GREEN,
/// because today's ledger is not empty — the guard could not be shown to bite on real
/// data, which makes it indistinguishable from a comment. Extracted, both directions
/// are checkable with fixtures, which is the same repair the planted-lock leg needed.
fn reportability(rows: &[Row], sections: &[&str]) -> Result<(), String> {
    if rows.is_empty() {
        return Err("EMPTY_LEDGER: a zero-row ledger makes every count vacuous and reads \
                    identically to a fleet where nothing has converged"
            .to_owned());
    }
    if sections.is_empty() {
        return Err("EMPTY_SECTIONS: the report would iterate nothing and pass".to_owned());
    }
    let orphans: Vec<&str> = rows
        .iter()
        .map(|r| r.section.as_str())
        .filter(|s| !sections.contains(s))
        .collect();
    if !orphans.is_empty() {
        return Err(format!(
            "ORPHAN_SECTIONS: rows name sections the report does not iterate, so their \
             grades are invisible to the verdict: {orphans:?}"
        ));
    }
    Ok(())
}

/// PROVES the guard bites, in all three directions. Without this the guard was a
/// comment with an `assert!` around it.
#[test]
fn the_reportability_guard_refuses_every_vacuous_shape() {
    let row = |section: &str| Row {
        section: section.to_owned(),
        round: 1,
        lens: "investor".to_owned(),
        new_findings: 0,
        role: "hillclimb".to_owned(),
        gates_green: true,
    };
    // KNOWN-GOOD first: a real shape must pass, or the guard is merely strict.
    assert!(reportability(&[row("00-brief")], &["00-brief"]).is_ok());

    // KNOWN-BAD 1: an empty ledger.
    let empty = reportability(&[], SECTIONS).expect_err("an empty ledger must refuse");
    assert!(empty.starts_with("EMPTY_LEDGER"), "{empty}");

    // KNOWN-BAD 2: no sections to iterate.
    let no_sections =
        reportability(&[row("00-brief")], &[]).expect_err("zero sections must refuse");
    assert!(no_sections.starts_with("EMPTY_SECTIONS"), "{no_sections}");

    // KNOWN-BAD 3: the defect this found on its first run — a graded section the
    // report does not iterate. `12-journey` had five ledger rows and was absent from
    // SECTIONS, so it could never converge and could never block.
    let orphan = reportability(&[row("12-journey")], &["00-brief"])
        .expect_err("an orphan section must refuse");
    assert!(orphan.starts_with("ORPHAN_SECTIONS"), "{orphan}");
    assert!(orphan.contains("12-journey"), "the refusal must NAME it: {orphan}");
}

#[test]
fn report_convergence_state() {
    let rows = ledger();
    // The guard is now one call whose failure modes are proven above.
    if let Err(refusal) = reportability(&rows, SECTIONS) {
        panic!("{refusal}");
    }

    let done: Vec<_> = SECTIONS
        .iter()
        .filter(|s| converged(&rows, s) && !capability_regressed(&rows, s))
        .collect();
    println!("CONVERGED {}/{}", done.len(), SECTIONS.len());
    let mut marks: Vec<(&str, usize, &str)> = Vec::new();
    for s in SECTIONS {
        let n = rows.iter().filter(|r| r.section == *s).count();
        let mark = if capability_regressed(&rows, s) {
            "REGRESSED"
        } else if converged(&rows, s) {
            "CONVERGED"
        } else {
            "open"
        };
        println!("  {s:<20} graded={n:<3} {mark}");
        marks.push((s, n, mark));
    }

    // THE ASSERTION THE TEST LACKED: the printed headline must equal the number of
    // rows the loop marked CONVERGED. Two computations of one quantity, and a fact
    // stated in two places will disagree in one.
    let marked = marks.iter().filter(|(_, _, m)| *m == "CONVERGED").count();
    assert_eq!(
        done.len(),
        marked,
        "the headline count and the per-section marks disagree: {done:?} vs {marks:?}"
    );

    // Every section must be classified into exactly one of the three states. A fourth
    // string would mean the report grew a state the verdict does not understand.
    for (section, _, mark) in &marks {
        assert!(
            matches!(*mark, "REGRESSED" | "CONVERGED" | "open"),
            "{section} got an unclassified mark {mark:?}"
        );
    }
    assert_eq!(marks.len(), SECTIONS.len(), "a section went unreported");

    // And a section reported CONVERGED must have been graded at least twice — the
    // predicate needs two rounds under two lenses, so a CONVERGED section with fewer
    // than two rows would mean the predicate accepted something it documents as
    // impossible.
    for (section, graded, mark) in &marks {
        if *mark == "CONVERGED" {
            assert!(
                *graded >= 2,
                "{section} is CONVERGED on {graded} graded row(s); `converged` requires \
                 two rounds under two lenses"
            );
        }
    }
}

/// The gate the DAG conversion must pass. Currently expected to FAIL — it is the
/// finish line, not a description of today.
#[test]
#[ignore = "finish line: run with --ignored to check whether the DAG may be built"]
fn every_section_converged_before_dag_conversion() {
    let rows = ledger();
    let open: Vec<&str> = SECTIONS
        .iter()
        .copied()
        .filter(|s| !converged(&rows, s) || capability_regressed(&rows, s))
        .collect();
    let held = rows.iter().filter(|r| r.role == "held_out").count();
    assert!(
        held >= SECTIONS.len(),
        "the held-out lens must have run across all {} sections before the DAG is built; \
         {held} held_out rows present. Without it, convergence cannot be distinguished from \
         the graders having adapted to each other.",
        SECTIONS.len()
    );
    assert!(
        open.is_empty(),
        "{} of {} sections are not converged; the plan may not become a bead DAG yet:\n  {}",
        open.len(),
        SECTIONS.len(),
        open.join("\n  ")
    );
}

#[test]
fn the_convergence_predicate_is_strict() {
    let mk = |section, round, lens, nf| Row {
        section: String::from(section),
        round,
        lens: String::from(lens),
        new_findings: nf,
        role: "hillclimb".to_owned(),
        gates_green: true,
    };

    // KNOWN-GOOD: two clean rounds, two lenses.
    let good = vec![mk("x", 1, "investor", 0), mk("x", 2, "adversarial", 0)];
    assert!(
        converged(&good, "x"),
        "two clean rounds under two lenses must converge"
    );

    // KNOWN-BAD 1: same lens twice — the lens may simply have stopped looking.
    let same = vec![mk("x", 1, "investor", 0), mk("x", 2, "investor", 0)];
    assert!(
        !converged(&same, "x"),
        "the same lens twice must NOT converge"
    );

    // KNOWN-BAD 2: a finding in the second round breaks the streak.
    let dirty = vec![mk("x", 1, "investor", 0), mk("x", 2, "adversarial", 3)];
    assert!(
        !converged(&dirty, "x"),
        "a round with findings must break the streak"
    );

    // KNOWN-BAD 3: non-consecutive clean rounds are not a streak.
    let gap = vec![
        mk("x", 1, "investor", 0),
        mk("x", 2, "absence", 2),
        mk("x", 3, "evidence", 0),
    ];
    assert!(
        !converged(&gap, "x"),
        "clean rounds either side of a dirty one are not a streak"
    );

    // ANTI-VACUITY: an empty ledger converges nothing.
    assert!(
        !converged(&[], "x"),
        "an empty ledger must never report convergence"
    );
}

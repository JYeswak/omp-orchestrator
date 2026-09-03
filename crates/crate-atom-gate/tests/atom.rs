#![forbid(unsafe_code)]
//! d3gm — the five legs, over the kernel that decides what a crate IS.
//!
//! Every fixture is a shape MEASURED on this workspace 2026-09-03, not an invented one:
//! 68 packages, 66 `[lib]`, 58 `[[bin]]`, 60 with a test target, **0** in-crate fuzz
//! targets, **0** claim rows, **0** SLO rows, and `fires_on_known_bad` present in exactly
//! ONE crate (`inbox-monitor`).

use std::collections::BTreeSet;

use crate_atom_gate::{
    assess_crate, ceiling_breaches, parse_allowances, verdict, Allowances, Caller, CrateAllowance,
    CrateFacts, GateVerdict, Part, PartStatus, Row, SystemicAllowance, REQUIRED_TEST_LEGS,
    UNWIRED_ALLOWANCE, WIRED_CALLERS,
};

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

/// A crate that satisfies all nine parts, so the RED legs below are attributable.
fn complete(name: &str) -> CrateFacts {
    CrateFacts {
        name: name.to_owned(),
        has_lib: true,
        has_bin: true,
        verdict_markers: set(&["Unrun", "InstrumentError"]),
        test_fn_names: REQUIRED_TEST_LEGS
            .iter()
            .map(|leg| format!("{leg}_case"))
            .collect(),
        in_crate_fuzz_targets: set(&["parse.rs"]),
        claim_ids: set(&["C-0001"]),
        slo_rows: set(&["p95_latency_ms"]),
        oracle: Some("tmux".to_owned()),
        on_tick_path: true,
        has_fuzzable_kernel: true,
        callers: vec![Caller::ManifestDependency {
            dependent: "no-shell-gate".to_owned(),
        }],
    }
}

fn status_of(rows: &[Row], part: Part) -> &PartStatus {
    &rows
        .iter()
        .find(|row| row.part == part)
        .expect("every part is assessed for every crate")
        .status
}

/// LEG 1 — fires on known bad: a crate with a lib and no `tests/` goes RED, named.
#[test]
fn fires_on_known_bad() {
    let mut facts = complete("planted-fixture");
    facts.test_fn_names = BTreeSet::new();
    let rows = assess_crate(&facts, &Allowances::default());
    let status = status_of(&rows, Part::Tests);
    assert!(status.refuses(), "a lib with no tests/ must refuse: {status:?}");
    let PartStatus::Missing { detail } = status else {
        panic!("expected Missing, got {status:?}")
    };
    // The refusal must NAME the absent legs, or nobody can act on it.
    for leg in REQUIRED_TEST_LEGS {
        assert!(detail.contains(leg), "{leg} not named in {detail:?}");
    }
    let outcome = verdict(&rows, 1, &Allowances::default());
    assert_eq!(outcome.exit_code(), 1, "{outcome:?}");
    assert!(
        matches!(&outcome, GateVerdict::Refused { reasons }
            if reasons.iter().any(|r| r.contains("planted-fixture") && r.contains("part4"))),
        "the whole-scan verdict must name the crate AND the part: {outcome:?}"
    );
}

/// LEG 2 — passes known good: `subprocess-contract`'s real shape, with allowance rows for
/// parts 5/7/8, is GREEN.
///
/// The bead specifies exactly this fixture. Without it an over-strict gate — one that
/// refused everything — would satisfy every RED leg above and get routed around.
#[test]
fn passes_known_good() {
    let mut facts = complete("subprocess-contract");
    // Its measured reality: no in-crate fuzz, no SLO row, no named oracle.
    facts.in_crate_fuzz_targets = BTreeSet::new();
    facts.slo_rows = BTreeSet::new();
    facts.oracle = None;
    let allowances = Allowances {
        per_crate: [Part::Fuzz, Part::Slo, Part::Oracle]
            .into_iter()
            .map(|part| CrateAllowance {
                crate_name: "subprocess-contract".to_owned(),
                part,
                owner: "josh".to_owned(),
                dies_when: "an SLO row lands".to_owned(),
            })
            .collect(),
        systemic: Vec::new(),
    };
    let rows = assess_crate(&facts, &allowances);
    let outcome = verdict(&rows, 1, &allowances);
    assert_eq!(outcome, GateVerdict::Pass, "{outcome:?}");
    assert_eq!(outcome.exit_code(), 0);
    for part in [Part::Fuzz, Part::Slo, Part::Oracle] {
        let status = status_of(&rows, part);
        assert!(
            matches!(status, PartStatus::Allowed { scope, owner, dies_when }
                if *scope == "crate" && owner == "josh" && !dies_when.is_empty()),
            "part{} must be ALLOWED naming owner and dies_when: {status:?}",
            part.number()
        );
    }
}

/// LEG 3 — the mutation goes red, and it is the bead's own mutation.
///
/// # THE BEAD CONTRADICTS THE REPO HERE, and the contradiction is the finding
///
/// The bead says *"delete `fires_on_known_bad` from `no-shell-gate/tests` -> RED on part
/// 4"*. MEASURED 2026-09-03: `fires_on_known_bad` exists in **exactly one** crate and it
/// is **`inbox-monitor`** (`crates/inbox-monitor/tests/monitor.rs`) — `no-shell-gate` does
/// not carry the symbol, so deleting it there would be a no-op and the mutation would
/// prove nothing. The leg therefore runs against the crate that actually has it, and the
/// discrepancy is reported rather than silently re-aimed.
#[test]
fn mutation_goes_red() {
    let mut facts = complete("inbox-monitor");
    assert!(
        matches!(status_of(&assess_crate(&facts, &Allowances::default()), Part::Tests),
            PartStatus::Present { .. }),
        "baseline must be GREEN on part 4 or the mutation below is unattributable"
    );
    // The mutation: remove exactly `fires_on_known_bad` and nothing else.
    facts
        .test_fn_names
        .retain(|name| !name.contains("fires_on_known_bad"));
    let rows = assess_crate(&facts, &Allowances::default());
    let status = status_of(&rows, Part::Tests);
    assert!(status.refuses(), "removing one leg must go RED: {status:?}");
    let PartStatus::Missing { detail } = status else {
        panic!("expected Missing")
    };
    assert!(detail.contains("fires_on_known_bad"), "{detail:?}");
    // ATTRIBUTABLE: only that leg is named, so the RED is keyed on the mutation and not on
    // an unrelated absence.
    assert!(
        !detail.contains("passes_known_good"),
        "the other four legs are still present; naming them would make the RED unattributable: {detail:?}"
    );
}

/// LEG 4 — anti-vacuity: an empty scan is an ERROR, not a pass.
#[test]
fn empty_scan_is_error() {
    let outcome = verdict(&[], 0, &Allowances::default());
    assert!(matches!(&outcome, GateVerdict::Unrun { reason } if reason.contains("SCAN_EMPTY")));
    assert_eq!(outcome.exit_code(), 2, "the bead specifies exit 2 for SCAN_EMPTY");
    assert_ne!(outcome.exit_code(), 0, "an empty scan must never be a pass");

    // And the OTHER vacuity: crates scanned, zero rows produced, is the assessor broken —
    // a distinct arm, because it is an instrument fault and not a finding about a crate.
    let broken = verdict(&[], 68, &Allowances::default());
    assert!(
        matches!(&broken, GateVerdict::InstrumentError { reason } if reason.contains("68")),
        "{broken:?}"
    );
    assert_eq!(broken.exit_code(), 3);

    // POSITIVE CONTROL: a real scan with a real row is neither.
    let rows = assess_crate(&complete("x"), &Allowances::default());
    assert_eq!(verdict(&rows, 1, &Allowances::default()), GateVerdict::Pass);
}

/// LEG 5 — the claim header: this crate's own claim is stated as a floor-raise.
///
/// A residual "guarantees / proves / makes impossible" in a gate's own description is
/// itself a defect, because a reader stops looking. So the words are banned here, over the
/// crate's own doc comment.
#[test]
fn claim_header_states_a_floor_and_not_a_guarantee() {
    let lib = include_str!("../src/lib.rs");
    let header: String = lib
        .lines()
        .take_while(|line| line.starts_with("//!") || line.starts_with("#!") || line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!header.is_empty(), "ANTI-VACUITY: no header was read");
    for banned in ["guarantees", "makes it impossible", "proves that every"] {
        assert!(
            !header.to_lowercase().contains(banned),
            "the header overclaims with {banned:?}"
        );
    }
    // ENFORCES / STILL PASSES: the header must say what the gate cannot do.
    assert!(
        header.contains("worth zero") || header.contains("vacuously green"),
        "the header must name what still passes, not only what it enforces"
    );
}

/// The ratchet, in BOTH directions. This is what makes a systemic allowance shrink.
#[test]
fn a_systemic_ceiling_refuses_slack_as_well_as_breach() {
    let systemic = |ceiling| Allowances {
        per_crate: Vec::new(),
        systemic: vec![SystemicAllowance {
            part: Part::Claim,
            owner: "josh".to_owned(),
            dies_when: "registries/claims.toml exists".to_owned(),
            ceiling,
        }],
    };
    // Two crates missing part 6, both systemically allowed.
    let mut rows = Vec::new();
    for name in ["a", "b"] {
        let mut facts = complete(name);
        facts.claim_ids = BTreeSet::new();
        rows.extend(assess_crate(&facts, &systemic(2)));
    }
    assert!(
        ceiling_breaches(&rows, &systemic(2)).is_empty(),
        "an EXACT ceiling is the only clean state"
    );
    let breach = ceiling_breaches(&rows, &systemic(1));
    assert!(
        breach.iter().any(|r| r.contains("CEILING_BREACHED") && r.contains("live=2")),
        "{breach:?}"
    );
    let slack = ceiling_breaches(&rows, &systemic(5));
    assert!(
        slack.iter().any(|r| r.contains("CEILING_HAS_SLACK") && r.contains("live=2")),
        "a ceiling above the live count cannot detect the next regression: {slack:?}"
    );
    // And the whole verdict refuses on a breach even though every cell is ALLOWED.
    assert!(matches!(
        verdict(&rows, 2, &systemic(1)),
        GateVerdict::Refused { .. }
    ));
}

/// Part 9 is REACHABILITY, and the answer names the machine.
#[test]
fn part_nine_is_reachability_and_names_the_machine() {
    // `reap-finished-panes` today: no manifest dependent, no hook, no unit.
    let mut orphan = complete("reap-finished-panes");
    orphan.callers = Vec::new();
    let status = status_of(&assess_crate(&orphan, &Allowances::default()), Part::WiredCaller);
    assert!(status.refuses(), "{status:?}");
    let PartStatus::Missing { detail } = status else {
        panic!()
    };
    assert!(detail.contains("NO REACHABLE TRIGGER"), "{detail:?}");

    // POSITIVE CONTROL, per the bead: `no-shell-gate` via the installed hook, and the
    // evidence names the machine — because a hook is untracked and per-clone, so the same
    // crate is reachable here and unreachable in a fresh checkout elsewhere.
    let mut wired = complete("no-shell-gate");
    wired.callers = vec![Caller::InstalledHook {
        hook: ".git/hooks/pre-commit".to_owned(),
        machine: "joshs-mac-studio".to_owned(),
    }];
    let status = status_of(&assess_crate(&wired, &Allowances::default()), Part::WiredCaller);
    let PartStatus::Present { evidence } = status else {
        panic!("expected Present, got {status:?}")
    };
    assert!(evidence.contains(".git/hooks/pre-commit"), "{evidence:?}");
    assert!(
        evidence.contains("joshs-mac-studio"),
        "the answer must name the machine it holds for: {evidence:?}"
    );
}

/// Applicability is not coverage: a crate with no parser has nothing to fuzz, and saying
/// so must not inflate the numbers the materializer files beads from.
#[test]
fn not_applicable_is_distinct_from_present() {
    let mut facts = complete("pure-types");
    facts.has_fuzzable_kernel = false;
    facts.in_crate_fuzz_targets = BTreeSet::new();
    facts.on_tick_path = false;
    facts.slo_rows = BTreeSet::new();
    let rows = assess_crate(&facts, &Allowances::default());
    for part in [Part::Fuzz, Part::Slo] {
        let status = status_of(&rows, part);
        assert!(matches!(status, PartStatus::NotApplicable { .. }), "{status:?}");
        assert_eq!(status.word(), "n/a", "the report must show n/a, not present");
        assert!(!status.refuses());
    }
    // A fuzzable kernel with no target is a DIFFERENT answer for the same crate name.
    let mut fuzzable = facts.clone();
    fuzzable.has_fuzzable_kernel = true;
    assert!(status_of(&assess_crate(&fuzzable, &Allowances::default()), Part::Fuzz).refuses());
}

/// A malformed allowance row is REFUSED by line, never skipped.
///
/// A skipped row is a silently unexcused crate, which reads as a real finding.
#[test]
fn a_malformed_allowance_row_is_named_not_skipped() {
    let good = parse_allowances(
        "[[allowance]]\ncrate = \"x\"\npart = 5\nowner = \"josh\"\ndies_when = \"a target lands\"\n",
    )
    .expect("a complete row parses");
    assert_eq!(good.per_crate.len(), 1);
    assert_eq!(good.per_crate[0].part, Part::Fuzz);

    // Each absence names the field, and an empty owner is refused as loudly as a missing
    // one — "owner = \"\"" is the shape a permanent exception takes.
    for (text, needle) in [
        ("[[allowance]]\ncrate = \"x\"\npart = 5\nowner = \"josh\"\n", "dies_when"),
        ("[[allowance]]\ncrate = \"x\"\npart = 5\ndies_when = \"y\"\n", "owner"),
        (
            "[[allowance]]\ncrate = \"x\"\npart = 5\nowner = \"\"\ndies_when = \"y\"\n",
            "owner",
        ),
        ("[[allowance]]\ncrate = \"x\"\npart = 99\nowner = \"j\"\ndies_when = \"y\"\n", "99"),
        ("[[systemic]]\npart = 6\nowner = \"j\"\ndies_when = \"y\"\n", "ceiling"),
    ] {
        let error = parse_allowances(text).expect_err("must refuse");
        assert!(error.contains("ALLOWANCE_MALFORMED"), "{error}");
        assert!(error.contains(needle), "{error} did not name {needle}");
    }

    // Two ceilings for one part cannot both be authoritative.
    let dup = parse_allowances(
        "[[systemic]]\npart = 6\nowner = \"j\"\ndies_when = \"y\"\nceiling = 1\n\
         [[systemic]]\npart = 6\nowner = \"j\"\ndies_when = \"y\"\nceiling = 2\n",
    )
    .expect_err("duplicate systemic part must refuse");
    assert!(dup.contains("duplicate systemic"), "{dup}");

    // ANTI-VACUITY on the parser: empty input is an empty registry, not an error, and an
    // empty registry excuses NOTHING.
    let empty = parse_allowances("").expect("empty is a real state");
    assert!(empty.per_crate.is_empty() && empty.systemic.is_empty());
}

/// Part 5 of the acceptance: both wired call sites resolve, and the allowance is empty.
#[test]
fn both_wired_callers_resolve_in_this_tree() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("crates/<name> has a root two levels up");
    assert!(
        UNWIRED_ALLOWANCE.is_empty(),
        "a gate that is not invoked is worth zero; this allowance is empty BY DESIGN"
    );
    assert_eq!(WIRED_CALLERS.len(), 2, "the bead names exactly two call sites");
    for (path, why) in WIRED_CALLERS {
        let full = root.join(path);
        assert!(full.is_file(), "declared caller {path} does not exist");
        assert!(!why.is_empty(), "{path} must say WHY it is a caller");
        let text = std::fs::read_to_string(&full).expect("caller source is readable");
        assert!(
            text.contains("crate_atom_gate") || text.contains("crate-atom-gate"),
            "{path} is declared as a caller but does not reference this gate -- that is \
             BUILT != WIRED aimed at this very crate"
        );
    }
}

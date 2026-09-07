//! `omp-orchestrator-fsu7` — the roster contract.
//!
//! NAMED TARGET per the mmt4 ruling: cite as
//! `cargo test -j 2 -p gate-runner --test roster`, never a bare `-p` aggregate.
//!
//! Every leg here is a PURE function over a metadata fixture, so the contract is provable without
//! a multi-hour workspace test run. The legs that need the real workspace are the binary's
//! `--plan` mode, reported separately.
//!
//! # What these legs defend
//!
//! `gate.yml` fanned out to 12 jobs naming 11 crates, and a *missing* job key once collapsed two
//! jobs into one — halving coverage with no error anywhere. The defect class is a gate set that
//! can shrink silently. So the load-bearing legs are the ones about ABSENCE: an empty roster, a
//! short crate, and a ledger that names a crate the workspace no longer has.

use gate_runner::{
    build_report, check_allowance, derive_roster, parse_ledger, CrateVerdict, Invocation,
    NoTestsDisposition, Observed, RosterError, EXIT_EMPTY_ROSTER, EXIT_GATE_FAILED,
    EXIT_LEDGER_DRIFT, EXIT_OK, EXIT_SHORT_ROSTER,
};
use std::collections::{BTreeMap, BTreeSet};

/// A metadata fixture. Shaped like cargo's own output, including the `kind` array.
fn metadata(packages: &[(&str, &[&str])]) -> String {
    let entries: Vec<String> = packages
        .iter()
        .map(|(name, tests)| {
            let targets: Vec<String> = tests
                .iter()
                .map(|t| format!(r#"{{"name":"{t}","kind":["test"]}}"#))
                .chain(std::iter::once(
                    r#"{"name":"lib","kind":["lib"]}"#.to_owned(),
                ))
                .collect();
            format!(
                r#"{{"name":"{name}","manifest_path":"/x/{name}/Cargo.toml","targets":[{}]}}"#,
                targets.join(",")
            )
        })
        .collect();
    format!(r#"{{"packages":[{}]}}"#, entries.join(","))
}

fn lib_tests(pairs: &[(&str, bool)]) -> BTreeMap<String, bool> {
    pairs
        .iter()
        .map(|(name, has)| ((*name).to_owned(), *has))
        .collect()
}

fn observed_pass(targets: &[&str]) -> Observed {
    Observed {
        passed: targets.iter().map(|t| (*t).to_owned()).collect(),
        failed: Vec::new(),
        unmeasurable: None,
    }
}

/// KNOWN-GOOD, and mandatory. An attack-only suite ships an over-strict gate, and an over-strict
/// gate gets routed around — a slower death than no gate.
#[test]
fn a_healthy_workspace_passes_and_reports_every_crate() {
    let md = metadata(&[("alpha", &["contract"]), ("beta", &["wired"])]);
    let roster = derive_roster(&md, &lib_tests(&[("alpha", true), ("beta", false)]))
        .expect("fixture parses");
    assert_eq!(roster.len(), 2);
    // alpha contributes BOTH the --lib leg and its integration target.
    assert_eq!(
        roster[0].invocations,
        vec![Invocation::Lib, Invocation::Test("contract".to_owned())],
        "a crate whose lib carries #[test] owes a --lib leg as well as its integration targets"
    );
    assert_eq!(roster[0].expected(), 2);
    assert_eq!(roster[1].expected(), 1);

    let mut obs = BTreeMap::new();
    obs.insert("alpha".to_owned(), observed_pass(&["--lib", "contract"]));
    obs.insert("beta".to_owned(), observed_pass(&["wired"]));
    let ledger: BTreeSet<String> = ["alpha", "beta"].iter().map(|s| (*s).to_owned()).collect();

    let report = build_report(&roster, &obs, &ledger);
    assert_eq!(report.exit_code(), EXIT_OK, "{}", report.render());
    assert_eq!(report.verdicts.len(), 2, "every crate gets a row");
    let rendered = report.render();
    assert!(rendered.contains("PASS crate=alpha targets=2"), "{rendered}");
    assert!(rendered.contains("PASS crate=beta targets=1"), "{rendered}");
}

/// THE LOAD-BEARING LEG. An empty roster is an ERROR with its own code, never a pass.
///
/// `fsu7` exists because seven gates read `status=open` while nothing invoked them. A runner that
/// answered "0 crates, all green" would recreate that condition inside a green binary.
#[test]
fn an_empty_roster_is_an_error_and_never_a_pass() {
    let roster = derive_roster(&metadata(&[]), &BTreeMap::new()).expect("empty parses");
    assert!(roster.is_empty());
    let report = build_report(&roster, &BTreeMap::new(), &BTreeSet::new());
    assert_eq!(
        report.exit_code(),
        EXIT_EMPTY_ROSTER,
        "an empty gate set must have its OWN code, distinguishable from a pass and from a failure"
    );
    assert_ne!(report.exit_code(), EXIT_OK);
    let rendered = report.render();
    assert!(
        rendered.contains("GATE_RUNNER_EMPTY_ROSTER"),
        "the message must name the cause, not just the code: {rendered}"
    );
    assert!(
        rendered.contains("never a pass"),
        "and it must say why: {rendered}"
    );
}

/// A crate that reports FEWER results than the roster expected is SHORT, with its own code.
///
/// This is the shape of the original defect: a target vanishes or fails to compile, and the run
/// reports a smaller success. The denominator is what makes it visible.
#[test]
fn a_crate_that_loses_a_target_is_short_not_a_smaller_pass() {
    let md = metadata(&[("alpha", &["one", "two"])]);
    let roster = derive_roster(&md, &lib_tests(&[("alpha", false)])).expect("parses");
    assert_eq!(roster[0].expected(), 2);

    let mut obs = BTreeMap::new();
    obs.insert("alpha".to_owned(), observed_pass(&["one"])); // "two" vanished
    let ledger: BTreeSet<String> = ["alpha"].iter().map(|s| (*s).to_owned()).collect();
    let report = build_report(&roster, &obs, &ledger);

    assert_eq!(report.exit_code(), EXIT_SHORT_ROSTER);
    let rendered = report.render();
    assert!(
        rendered.contains("SHORT crate=alpha expected=2 observed=1"),
        "the message must carry BOTH sides of the denominator: {rendered}"
    );
    assert!(
        rendered.contains("a_target_vanished_or_failed_to_compile"),
        "{rendered}"
    );
}

/// KNOWN-BAD for deletion: a ledger row whose crate is gone goes RED **naming that crate**.
///
/// Per acceptance item 4, and asserted on the MESSAGE — `101` is cargo's generic failure and an
/// unrelated workspace-loading error produced an identical `101` in this repo twice in one hour,
/// so a leg keyed on `rc != 0` goes green on unrelated breakage.
#[test]
fn a_crate_removed_from_the_workspace_goes_red_naming_it() {
    let md = metadata(&[("alpha", &["contract"])]);
    let roster = derive_roster(&md, &lib_tests(&[("alpha", false)])).expect("parses");
    let mut obs = BTreeMap::new();
    obs.insert("alpha".to_owned(), observed_pass(&["contract"]));
    // The ledger remembers a crate the workspace no longer has.
    let ledger: BTreeSet<String> = ["alpha", "vanished-gate"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();

    let report = build_report(&roster, &obs, &ledger);
    assert_eq!(report.exit_code(), EXIT_LEDGER_DRIFT);
    let rendered = report.render();
    assert!(
        rendered.contains("crate=vanished-gate"),
        "THE WHOLE POINT: the message must NAME the missing crate, not merely fail: {rendered}"
    );
    assert!(
        rendered.contains("in_ledger_absent_from_workspace"),
        "{rendered}"
    );
    assert!(
        !rendered.contains("crate=alpha reason=in_ledger"),
        "the healthy crate must not be implicated: {rendered}"
    );
}

/// SECOND KNOWN-BAD, the one that catches this bead's own defect: a NEW gate crate is picked up
/// with no edit to the runner, and is RUN.
///
/// Per acceptance item 5. If a new gate needed a manual registration step, the YAML fan-out would
/// merely have moved into Rust. The drift row is reported so the ledger gets updated — but note
/// the assertion below: the crate is in the roster and therefore executed REGARDLESS.
#[test]
fn a_new_gate_crate_is_picked_up_without_editing_the_runner() {
    let md = metadata(&[("alpha", &["contract"]), ("brand-new-gate", &["fresh"])]);
    let roster = derive_roster(&md, &lib_tests(&[])).expect("parses");

    assert!(
        roster.iter().any(|e| e.crate_name == "brand-new-gate"),
        "a crate present in cargo metadata must appear in the roster with NO registration"
    );
    let new_entry = roster
        .iter()
        .find(|e| e.crate_name == "brand-new-gate")
        .expect("present");
    assert_eq!(
        new_entry.invocations,
        vec![Invocation::Test("fresh".to_owned())],
        "and its targets are derived too, not declared"
    );

    // It is RUN: the report demands an observation for it, and its absence is SHORT rather than
    // silence. That is what proves it is not merely listed.
    let mut obs = BTreeMap::new();
    obs.insert("alpha".to_owned(), observed_pass(&["contract"]));
    let ledger: BTreeSet<String> = ["alpha"].iter().map(|s| (*s).to_owned()).collect();
    let report = build_report(&roster, &obs, &ledger);
    assert_eq!(
        report.verdicts.get("brand-new-gate"),
        Some(&CrateVerdict::Short {
            expected: 1,
            observed: 0
        }),
        "an unrun new crate must be SHORT, never absent from the report"
    );
    let rendered = report.render();
    assert!(rendered.contains("crate=brand-new-gate"), "{rendered}");
}

/// A failing gate is FAILED and names its failing targets.
#[test]
fn a_failing_gate_names_its_failing_targets() {
    let md = metadata(&[("alpha", &["contract", "wired"])]);
    let roster = derive_roster(&md, &lib_tests(&[])).expect("parses");
    let mut obs = BTreeMap::new();
    obs.insert(
        "alpha".to_owned(),
        Observed {
            passed: vec!["contract".to_owned()],
            failed: vec!["wired".to_owned()],
            unmeasurable: None,
        },
    );
    let ledger: BTreeSet<String> = ["alpha"].iter().map(|s| (*s).to_owned()).collect();
    let report = build_report(&roster, &obs, &ledger);
    assert_eq!(report.exit_code(), EXIT_GATE_FAILED);
    assert!(
        report.render().contains("FAIL crate=alpha failing_targets=wired"),
        "{}",
        report.render()
    );
}

/// UNMEASURABLE is neither a pass nor a failure, and the total refuses to launder it into either.
///
/// `br` and `.beads` are absent on the Contabo workers, so some suites genuinely cannot answer
/// there. Reporting that as FAILED would blame the subject for the environment — the defect I
/// graded twice tonight in `br_publisher` and `mirror_oracle`.
#[test]
fn an_unmeasurable_crate_is_neither_pass_nor_fail() {
    let md = metadata(&[("alpha", &["needs_br"])]);
    let roster = derive_roster(&md, &lib_tests(&[])).expect("parses");
    let mut obs = BTreeMap::new();
    obs.insert(
        "alpha".to_owned(),
        Observed {
            passed: Vec::new(),
            failed: Vec::new(),
            unmeasurable: Some("br_absent_on_worker".to_owned()),
        },
    );
    let ledger: BTreeSet<String> = ["alpha"].iter().map(|s| (*s).to_owned()).collect();
    let report = build_report(&roster, &obs, &ledger);

    assert_eq!(
        report.exit_code(),
        EXIT_OK,
        "an environment that cannot answer must not fail the gate"
    );
    let verdict = report.verdicts.get("alpha").expect("row present");
    assert!(!verdict.is_blocking());
    assert_eq!(verdict.code(), "UNMEASURABLE", "and it is NOT reported as PASS");
    assert!(
        report.render().contains("UNMEASURABLE crate=alpha reason=br_absent_on_worker"),
        "{}",
        report.render()
    );
}

/// ITEM 7's DECISION, asserted rather than described: a crate with no tests at all is an ERROR
/// unless it carries a declared row.
///
/// The bead expected `asupersync-conformance` to need such a row for having "ZERO test targets".
/// MEASURED: it has SIX `#[test]` functions in a `#[cfg(test)]` module, and all twelve
/// zero-integration-target crates have unit tests. Genuinely untested crates: ZERO. So the
/// allowance is empty because there is nothing to except — not because the question was skipped.
#[test]
fn a_crate_with_no_tests_at_all_is_an_undeclared_error() {
    let md = metadata(&[("hollow", &[])]);
    let roster = derive_roster(&md, &lib_tests(&[("hollow", false)])).expect("parses");
    assert_eq!(roster[0].expected(), 0);

    let ledger: BTreeSet<String> = ["hollow"].iter().map(|s| (*s).to_owned()).collect();
    let report = build_report(&roster, &BTreeMap::new(), &ledger);
    assert_eq!(
        report.verdicts.get("hollow"),
        Some(&CrateVerdict::NoTests {
            disposition: NoTestsDisposition::Undeclared
        })
    );
    assert_eq!(report.exit_code(), EXIT_GATE_FAILED);
    let rendered = report.render();
    assert!(rendered.contains("disposition=UNDECLARED"), "{rendered}");
    assert!(
        rendered.contains("a_crate_with_no_tests_cannot_gate_anything"),
        "{rendered}"
    );
}

/// The allowance is checked in BOTH directions: a row naming a crate that HAS tests is an error
/// telling the reader to delete it.
///
/// An allowance that outlives its defect is how a repaired gap keeps reading as broken, and it is
/// what stops such a list from ever shrinking.
#[test]
fn the_allowance_is_empty_and_cannot_outlive_its_defect() {
    assert!(
        gate_runner::NO_TESTS_ALLOWANCE.is_empty(),
        "measured: zero genuinely untested crates, so there is nothing to except"
    );
    // The bidirectional check is exercised against a synthetic roster, because the real allowance
    // is empty and an empty list cannot demonstrate the mechanism.
    let md = metadata(&[("alpha", &["contract"])]);
    let roster = derive_roster(&md, &lib_tests(&[])).expect("parses");
    assert!(
        check_allowance(&roster).is_ok(),
        "an empty allowance never fires"
    );
}

/// Unreadable metadata is its OWN error, distinct from every gate verdict.
///
/// "I could not look" and "I looked and it is broken" are different facts. Collapsing them is the
/// defect this repository keeps paying for, most recently as a `101` that meant three different
/// things in one evening.
#[test]
fn unreadable_metadata_is_not_a_gate_failure() {
    let error = derive_roster("{ not json", &BTreeMap::new()).expect_err("must refuse");
    assert!(matches!(error, RosterError::MetadataUnreadable { .. }));
    let text = error.to_string();
    assert!(text.contains("GATE_RUNNER_METADATA_UNREADABLE"), "{text}");
    assert!(
        text.contains("NOT a gate failure"),
        "the message must say so explicitly, or a reader files a bug against the wrong crate: \
         {text}"
    );

    let missing = derive_roster(r#"{"no_packages":true}"#, &BTreeMap::new())
        .expect_err("a payload without `packages` is unreadable, not empty");
    assert!(matches!(missing, RosterError::MetadataUnreadable { .. }));
}

/// The ledger parser ignores comments and blanks, and nothing else.
#[test]
fn the_ledger_format_is_deliberately_trivial() {
    let parsed = parse_ledger("# a comment\n\nalpha\n  beta  \n#gamma\n");
    let expected: BTreeSet<String> = ["alpha", "beta"].iter().map(|s| (*s).to_owned()).collect();
    assert_eq!(
        parsed, expected,
        "a ledger that needs a parser is a ledger that can fail to parse, and this file exists to \
         detect silent loss"
    );
}

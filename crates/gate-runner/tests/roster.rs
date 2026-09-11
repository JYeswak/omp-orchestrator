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
    build_report, check_allowance, derive_checks, derive_roster, expand, parse_ledger,
    CrateVerdict, Invocation, NoTestsDisposition, Observed, RosterError, Subsumption,
    UnmeasurablePrecondition,
    EXIT_EMPTY_ROSTER, EXIT_GATE_FAILED, EXIT_LEDGER_DRIFT, EXIT_OK, EXIT_SHORT_ROSTER,
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
            unmeasurable: Some(UnmeasurablePrecondition::MissingExecutable {
                executable: "br".to_owned(),
                detail: "fixture missing command".to_owned(),
            }),
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
        report.render().contains("UNMEASURABLE crate=alpha reason=MISSING_EXECUTABLE executable=br detail=fixture missing command"),
        "{}",
        report.render()
    );
}
#[test]
fn measured_environment_does_not_hide_a_real_failure() {
    let md = metadata(&[("alpha", &["bad"])]);
    let roster = derive_roster(&md, &lib_tests(&[])).expect("parses");
    let mut observations = BTreeMap::new();
    observations.insert(
        "alpha".to_owned(),
        Observed {
            passed: Vec::new(),
            failed: vec!["bad".to_owned()],
            unmeasurable: Some(UnmeasurablePrecondition::MissingPath {
                path: ".git".to_owned(),
                detail: "fixture missing git metadata".to_owned(),
            }),
        },
    );
    let ledger: BTreeSet<String> = ["alpha"].iter().map(|s| (*s).to_owned()).collect();
    let report = build_report(&roster, &observations, &ledger);
    assert_eq!(report.exit_code(), EXIT_GATE_FAILED);
    let rendered = report.render();
    assert!(rendered.contains("FAIL crate=alpha failing_targets=bad"), "{rendered}");
    assert!(rendered.contains("unmeasurable=MISSING_PATH path=.git"), "{rendered}");
}

/// Pull `key=value` out of a whitespace-tokenised gate-runner line.
fn field<'a>(line: &'a str, key: &str) -> &'a str {
    let prefix = format!("{key}=");
    line.split_whitespace()
        .find_map(|token| token.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("line {line:?} carries no {key}="))
}

/// Find the single line beginning with `token ` in a rendered report.
fn line_with<'a>(rendered: &'a str, token: &str) -> &'a str {
    let prefix = format!("{token} ");
    let mut hits = rendered.lines().filter(|line| line.starts_with(&prefix));
    let first = hits
        .next()
        .unwrap_or_else(|| panic!("render() emitted no `{token}` line:\n{rendered}"));
    assert!(
        hits.next().is_none(),
        "`{token}` must appear EXACTLY once — a counter that can appear twice cannot be \
         reconciled by counting:\n{rendered}"
    );
    first
}

/// THE RECONCILIATION LEG — `omp-orchestrator-86zjl` legs 2, 3 and 6.
///
/// # The defect this defends, and why "the rows exist" was not the answer
///
/// Re-derived twice, independently, from CI run `34543513267` (462,955 bytes, head
/// `568f2dfe`, the newest completed run):
///
/// ```text
/// grep -c GATE_RUNNER                 ->  9    the whole documented marker family
/// of those 9, naming a crate verdict  ->  0    <- the defect
/// distinct FAIL crate= names          -> 17    == the aggregate's fail=17
/// ```
///
/// Eighty-eight named rows WERE in that log and they reconcile exactly. What was missing is a
/// line that a reader grepping the ONE documented marker can see — the bead was filed by an
/// honest `grep GATE_RUNNER` that returned nine lines, none of them a crate. Counting the rows
/// instead does not rescue it either: each row is printed twice (streamed, then reported —
/// byte-identically, and deliberately) and `CHECK_FAIL crate=` contains `FAIL crate=` as a
/// substring, so the naive count is 42 against an aggregate of 17.
///
/// So this leg asserts the three numbers that must agree and pins them to one line: the
/// aggregate's field, the count of rows in that class, and the `count=` on the work-list line,
/// whose `names=` list must have exactly that many entries.
///
/// **Anti-vacuity: this fixture FAILS on purpose.** A report with `fail=0` cannot demonstrate
/// naming — there is nothing to name — so the leg asserts `fail` is non-zero before it asserts
/// anything reconciles. The same rule is why the demonstration run for this bead is a real red
/// CI run and never a green one.
///
/// **Leg 3, in the same fixture:** the two UNMEASURABLE crates carry DIFFERENT codes.
/// `MISSING_EXECUTABLE` is repaired by reaching a tool and `POLICY_UNAVAILABLE` by supplying an
/// oracle; neither is repaired by fixing a test. A work list that printed both as
/// `unmeasurable` would send both repairs to the wrong place.
#[test]
fn the_work_list_names_every_non_passing_crate_and_reconciles_with_the_aggregate() {
    let md = metadata(&[
        ("alpha", &["contract"]),
        ("bravo", &["contract"]),
        ("charlie", &["contract"]),
        ("delta", &["contract"]),
        ("foxtrot", &["contract"]),
    ]);
    let roster = derive_roster(&md, &lib_tests(&[])).expect("fixture parses");
    let failed_on = |target: &str| Observed {
        passed: Vec::new(),
        failed: vec![target.to_owned()],
        unmeasurable: None,
    };
    let blocked_by = |reason: UnmeasurablePrecondition| Observed {
        passed: Vec::new(),
        failed: Vec::new(),
        unmeasurable: Some(reason),
    };
    let mut obs = BTreeMap::new();
    obs.insert("alpha".to_owned(), observed_pass(&["contract"]));
    obs.insert("bravo".to_owned(), failed_on("contract"));
    obs.insert("charlie".to_owned(), failed_on("contract"));
    obs.insert(
        "delta".to_owned(),
        blocked_by(UnmeasurablePrecondition::MissingExecutable {
            executable: "br".to_owned(),
            detail: "fixture missing command".to_owned(),
        }),
    );
    obs.insert(
        "foxtrot".to_owned(),
        blocked_by(UnmeasurablePrecondition::PolicyUnavailable {
            policy: "admission-reason-differential".to_owned(),
            detail: "fixture missing oracle".to_owned(),
        }),
    );
    let ledger: BTreeSet<String> = ["alpha", "bravo", "charlie", "delta", "foxtrot"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();

    let report = build_report(&roster, &obs, &ledger);
    let rendered = report.render();
    let aggregate = line_with(&rendered, "GATE_RUNNER");

    // ANTI-VACUITY FIRST. A green report proves nothing about naming failures.
    let declared_failures: usize = field(aggregate, "fail").parse().expect("fail is a number");
    let declared_unmeasurable: usize = field(aggregate, "unmeasurable")
        .parse()
        .expect("unmeasurable is a number");
    assert!(
        declared_failures > 0 && declared_unmeasurable > 0,
        "this leg cannot demonstrate naming from a clean run: {aggregate}"
    );

    // Pinned TOGETHER: the class that produced the exit code, and the exit code it produced.
    assert_eq!(
        report.exit_code(),
        EXIT_GATE_FAILED,
        "two failing crates and zero short ones must exit GATE_FAILED: {rendered}"
    );

    for (token, class, declared) in [
        ("GATE_RUNNER_FAILING", "FAIL", declared_failures),
        (
            "GATE_RUNNER_UNMEASURABLE",
            "UNMEASURABLE",
            declared_unmeasurable,
        ),
    ] {
        let work_list = line_with(&rendered, token);
        let listed: Vec<&str> = field(work_list, "names").split(',').collect();
        let counted: usize = field(work_list, "count")
            .parse()
            .expect("count is a number");
        let rows = rendered
            .lines()
            .filter(|line| line.starts_with(&format!("{class} crate=")))
            .count();
        assert_eq!(
            counted, declared,
            "{token} disagrees with the aggregate's own tally — the mismatch 86zjl was filed \
             for:\n{rendered}"
        );
        assert_eq!(
            listed.len(),
            declared,
            "{token} claims {declared} but names {} — a count without that many names is not a \
             work list:\n{rendered}",
            listed.len()
        );
        assert_eq!(
            rows, declared,
            "{class} rows ({rows}) do not reconcile with the aggregate ({declared}); a crate \
             counted without a row is a failure nobody can claim:\n{rendered}"
        );
    }

    // Every failing crate is nameable FROM THE WORK LIST ALONE, with no other line consulted.
    let failing = field(line_with(&rendered, "GATE_RUNNER_FAILING"), "names");
    assert_eq!(failing, "bravo,charlie", "{rendered}");

    // LEG 3: the remedy-selecting code travels WITH the name, and the two differ.
    let unmeasurable = field(line_with(&rendered, "GATE_RUNNER_UNMEASURABLE"), "names");
    assert_eq!(
        unmeasurable, "delta:MISSING_EXECUTABLE,foxtrot:POLICY_UNAVAILABLE",
        "an absent tool and an absent oracle have different repairs and must not share a \
         label:\n{rendered}"
    );

    // ZERO IS NOT ABSENCE. An empty class says so rather than vanishing.
    for token in ["GATE_RUNNER_SHORT", "GATE_RUNNER_NO_TESTS"] {
        let line = line_with(&rendered, token);
        assert_eq!(field(line, "count"), "0", "{rendered}");
        assert_eq!(field(line, "names"), "NONE", "{rendered}");
    }
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

/// A gate's check invocation is read from ITS OWN manifest, so adding a gate needs no edit here.
///
/// This is acceptance item 5 for the RUN half. The roster's test half is derivable from cargo's
/// target discovery; a repo-scanning verb's argv is not, and a central list of them in this crate
/// would be the YAML fan-out moved into Rust.
#[test]
fn a_gate_declares_its_own_check_invocation_in_its_own_manifest() {
    let md = r#"{"packages":[
        {"name":"alpha","manifest_path":"/x/a/Cargo.toml","targets":[],
         "metadata":{"gate":{"checks":[["--repo","{repo}"]]}}},
        {"name":"plain-lib","manifest_path":"/x/p/Cargo.toml","targets":[]}
    ]}"#;
    let checks = derive_checks(md).expect("parses");
    assert_eq!(checks.len(), 1, "a crate with no stanza declares no check, and that is not an error");
    assert_eq!(checks[0].crate_name, "alpha");
    assert_eq!(checks[0].phases.len(), 1);
    assert_eq!(checks[0].phases[0].bin, None, "a bare array means the crate's default bin");
    assert_eq!(checks[0].phases[0].args, vec!["--repo", "{repo}"]);
}

/// A multi-phase gate keeps its ORDER. `commit-build-fence` is `init` then `check`, and running
/// them the other way round would fence against a stamp that does not exist yet.
#[test]
fn a_multi_phase_gate_keeps_its_phase_order() {
    let md = r#"{"packages":[{"name":"fence","manifest_path":"/x/f/Cargo.toml","targets":[],
        "metadata":{"gate":{"checks":[
            {"bin":"fence","args":["init","--repo","{repo}"],"setup":true},
            {"bin":"fence","args":["check","--repo","{repo}"]}
          ]}}}]}"#;
    let checks = derive_checks(md).expect("parses");
    assert_eq!(checks[0].phases.len(), 2);
    assert_eq!(checks[0].phases[0].args[0], "init", "phase order is load-bearing");
    assert_eq!(checks[0].phases[1].args[0], "check");
}

/// THE CASE THE SCHEMA EXISTS FOR: one crate hosting several gate bins.
///
/// MEASURED: `gate.yml` runs **15 distinct gate bins across 13 crates**, and `no-shell-gate` alone
/// hosts three — `no-shell-gate`, `gate-reachability`, `head-compiles-gate`. A crate-keyed schema
/// silently collapses those to one, which is the fan-out surviving the fix meant to remove it.
/// `%6` replaced `jobs` with `bins` as item 3's denominator for this reason: the fan-out grows
/// INSIDE jobs, so a job count is structurally blind to it.
#[test]
fn a_crate_hosting_several_gate_bins_keeps_all_of_them() {
    let md = r#"{"packages":[{"name":"multi","manifest_path":"/x/m/Cargo.toml","targets":[],
        "metadata":{"gate":{"checks":[
          {"bin":"multi","args":[],"takes_no_args":true},
          {"bin":"reachability","args":["--root","{repo}"]},
          {"bin":"head-compiles","args":["--repo","{repo}","--receipt","{scratch}/r"]}
        ]}}}]}"#;
    let checks = derive_checks(md).expect("parses");
    assert_eq!(checks.len(), 1, "one crate");
    assert_eq!(
        checks[0].phases.len(),
        3,
        "THREE bins must survive; collapsing them to one is the defect fsu7 exists to end"
    );
    let bins: Vec<Option<&str>> = checks[0]
        .phases
        .iter()
        .map(|p| p.bin.as_deref())
        .collect();
    assert_eq!(
        bins,
        vec![Some("multi"), Some("reachability"), Some("head-compiles")],
        "each phase names its OWN bin, in order"
    );
    // And the placeholders in a named-bin phase expand like any other.
    let expanded = expand(&checks[0].phases[2].args, "/w/repo", "/w/scratch");
    assert_eq!(expanded, vec!["--repo", "/w/repo", "--receipt", "/w/scratch/r"]);
}

/// A table phase with no `bin` is UNREADABLE, not a default-bin phase.
///
/// Defaulting here would silently run the wrong binary — and on a multi-bin crate the wrong
/// binary is a different gate entirely.
#[test]
fn a_table_phase_without_a_bin_is_unreadable() {
    let md = r#"{"packages":[{"name":"alpha","manifest_path":"/x/a/Cargo.toml","targets":[],
        "metadata":{"gate":{"checks":[{"args":["--repo","{repo}"]}]}}}]}"#;
    let error = derive_checks(md).expect_err("a table phase must name its bin");
    assert!(matches!(error, RosterError::MetadataUnreadable { .. }));
    assert!(
        error.to_string().contains("must name a `bin`"),
        "the message must say what was missing: {error}"
    );
}

/// Placeholders are substituted by the runner, never hardcoded.
///
/// A manifest carrying `/Users/<someone>/...` is exactly what `path-literal-guard` refuses, and it
/// would make every other machine's run wrong.
#[test]
fn placeholders_are_substituted_rather_than_hardcoded() {
    let argv = vec!["--repo".to_owned(), "{repo}".to_owned(), "--bin-dir".to_owned(), "{scratch}".to_owned()];
    let out = expand(&argv, "/w/repo", "/w/scratch");
    assert_eq!(out, vec!["--repo", "/w/repo", "--bin-dir", "/w/scratch"]);
    assert!(
        !out.iter().any(|a| a.contains('{')),
        "an unsubstituted placeholder would reach the gate as a literal brace: {out:?}"
    );
}

/// A malformed stanza is UNREADABLE, not an absent check.
///
/// Silently treating a broken declaration as "no check declared" is how a gate stops running while
/// everything reads green — the exact defect `fsu7` exists to end.
#[test]
fn a_malformed_check_stanza_is_unreadable_not_absent() {
    let md = r#"{"packages":[{"name":"alpha","manifest_path":"/x/a/Cargo.toml","targets":[],
        "metadata":{"gate":{"checks":["--repo ."]}}}]}"#;
    let error = derive_checks(md).expect_err("a string where an argv list belongs must refuse");
    assert!(matches!(error, RosterError::MetadataUnreadable { .. }));
    assert!(
        error.to_string().contains("list of argv lists"),
        "the message must name the shape it wanted: {error}"
    );
}

/// ITEM 10, MECHANICALLY. A job whose binary half has no declared check is NOT subsumed, and the
/// reason names it — so no job can be deleted on the strength of a prose table.
#[test]
fn a_job_whose_run_half_is_undeclared_is_not_subsumed() {
    let md = metadata(&[("scanner", &["contract"])]);
    let roster = derive_roster(&md, &lib_tests(&[])).expect("parses");
    let no_checks: Vec<gate_runner::CheckInvocation> = Vec::new();

    // test-only job: covered by the roster alone.

    assert_eq!(
        subsumption_of("scanner", false, &roster, &no_checks),
        Subsumption::Covered { tests: true, checks: false }
    );

    // same crate, but the old job ALSO ran its binary and nothing declares that.
    match subsumption_of("scanner", true, &roster, &no_checks) {
        Subsumption::NotSubsumed { reason } => {
            assert!(reason.contains("scanner"), "the reason must NAME the crate: {reason}");
            assert!(
                reason.contains("run half is NOT"),
                "and must say which half is uncovered: {reason}"
            );
        }
        other => panic!("a run half with no declared check must NOT be subsumed: {other:?}"),
    }
}

/// And with the check declared, the same job IS subsumed on both halves.
#[test]
fn a_job_with_a_declared_check_is_subsumed_on_both_halves() {
    let md = metadata(&[("scanner", &["contract"])]);
    let roster = derive_roster(&md, &lib_tests(&[])).expect("parses");
    let checks = vec![gate_runner::CheckInvocation {
        crate_name: "scanner".to_owned(),
        phases: vec![gate_runner::CheckPhase { setup: false, takes_no_args: false, bin: None, args: vec!["{repo}".to_owned()] }],
    }];
    assert_eq!(
        subsumption_of("scanner", true, &roster, &checks),
        Subsumption::Covered { tests: true, checks: true }
    );
}

/// A crate absent from the roster is never quietly subsumed.
#[test]
fn a_job_naming_a_crate_with_no_tests_is_not_subsumed() {
    let md = metadata(&[("hollow", &[])]);
    let roster = derive_roster(&md, &lib_tests(&[("hollow", false)])).expect("parses");
    match subsumption_of("hollow", false, &roster, &[]) {
        Subsumption::NotSubsumed { reason } => {
            assert!(reason.contains("hollow"), "{reason}");
            assert!(reason.contains("no test invocation"), "{reason}");
        }
        other => panic!("expected NotSubsumed, got {other:?}"),
    }
}

fn subsumption_of(
    job_crate: &str,
    ran_binary: bool,
    roster: &[gate_runner::RosterEntry],
    checks: &[gate_runner::CheckInvocation],
) -> Subsumption {
    gate_runner::subsumption(job_crate, ran_binary, roster, checks)
}

/// REGRESSION: `--only` narrows what is RUN, never the workspace the ledger is compared against.
///
/// # The defect this pins, found by the anti-vacuity leg
///
/// `--run --only <one crate>` compared the 88-row ledger against a roster filtered to ONE entry
/// and reported **87 spurious** `in_ledger_absent_from_workspace` rows. Because
/// `exit_code()` ranks drift ABOVE gate failures, the scoped run returned `EXIT_LEDGER_DRIFT`
/// regardless of the actual verdict — making `--only` useless for exactly the decision it exists
/// to support (deciding R9 without a 286-invocation run).
///
/// The anti-vacuity case — `--only` on a name that matches nothing — is what surfaced it. A leg
/// written to prove an empty scope is an ERROR found a second defect on the way, which is the
/// argument for anti-vacuity legs generally: they exercise the boundary nothing else visits.
#[test]
fn scoping_the_run_does_not_manufacture_ledger_drift() {
    let md = metadata(&[("alpha", &["a"]), ("beta", &["b"]), ("gamma", &["g"])]);
    let full = derive_roster(&md, &lib_tests(&[])).expect("parses");
    let ledger: BTreeSet<String> = ["alpha", "beta", "gamma"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();

    // Scope to one crate, as `--only alpha` does.
    let scoped: Vec<_> = full
        .iter()
        .filter(|e| e.crate_name == "alpha")
        .cloned()
        .collect();
    let mut obs = BTreeMap::new();
    obs.insert("alpha".to_owned(), observed_pass(&["a"]));

    let report = gate_runner::build_report_scoped(&scoped, &full, &obs, &ledger);
    assert!(
        report.ledger_only.is_empty(),
        "beta and gamma are in the workspace; scoping the RUN must not report them as missing \
         from it: {:?}",
        report.ledger_only
    );
    assert_eq!(
        report.exit_code(),
        EXIT_OK,
        "a scoped run of a passing crate must report the crate's verdict, not drift: {}",
        report.render()
    );
    assert_eq!(
        report.verdicts.len(),
        1,
        "and only the scoped crate is executed, so only it gets a verdict"
    );

    // KNOWN-BAD, so this leg cannot pass by disabling drift detection entirely: a ledger row with
    // no crate in the FULL roster is still reported.
    let stale: BTreeSet<String> = ["alpha", "beta", "gamma", "deleted-gate"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    let report = gate_runner::build_report_scoped(&scoped, &full, &obs, &stale);
    assert_eq!(report.exit_code(), EXIT_LEDGER_DRIFT);
    assert!(
        report.render().contains("crate=deleted-gate"),
        "real drift must still be named under a scoped run: {}",
        report.render()
    );
}

/// HAZARD 2 leg (a): a bare `[]` is REFUSED as ambiguous, naming the stanza.
///
/// `%6`'s ruling on `etyur`: a field that cannot distinguish *"no arguments"* from *"someone
/// truncated this"* IS the defect, and picking a reading only chooses which failure is silent.
#[test]
fn a_bare_empty_argv_is_refused_as_ambiguous() {
    let md = r#"{"packages":[{"name":"pa","manifest_path":"/x/p/Cargo.toml","targets":[],
        "metadata":{"gate":{"checks":[[]]}}}]}"#;
    let error = derive_checks(md).expect_err("a bare [] must be REFUSED, not read as zero-arg");
    let text = error.to_string();
    assert!(text.contains("pa"), "the refusal must name the stanza: {text}");
    assert!(
        text.contains("AMBIGUOUS") && text.contains("takes_no_args"),
        "the refusal must name the cause AND the remedy: {text}"
    );
}

/// HAZARD 2 leg (b): the explicit marker is ACCEPTED and yields a real zero-arg phase.
#[test]
fn an_explicitly_declared_zero_arg_check_is_accepted() {
    let md = r#"{"packages":[{"name":"pa","manifest_path":"/x/p/Cargo.toml","targets":[],
        "metadata":{"gate":{"checks":[{"bin":"pa","args":[],"takes_no_args":true}]}}}]}"#;
    let checks = derive_checks(md).expect("the marker must be accepted");
    assert_eq!(checks[0].phases.len(), 1);
    assert!(checks[0].phases[0].args.is_empty());
    assert!(checks[0].phases[0].takes_no_args);
}

/// HAZARD 2 leg (c) — NON-WIDENING, and this is the ruling's whole safety.
///
/// Every marker that admits a previously-refused case is a candidate bypass. So a stanza
/// malformed some OTHER way must STILL be refused after `takes_no_args` exists.
#[test]
fn the_no_arg_marker_does_not_widen_into_a_bypass() {
    // a table with no `bin` is still unreadable, marker or not
    let no_bin = r#"{"packages":[{"name":"pa","manifest_path":"/x/p/Cargo.toml","targets":[],
        "metadata":{"gate":{"checks":[{"args":[],"takes_no_args":true}]}}}]}"#;
    assert!(
        derive_checks(no_bin).is_err(),
        "a table phase with no bin must still be refused"
    );
    // a non-array, non-table phase is still unreadable
    let scalar = r#"{"packages":[{"name":"pa","manifest_path":"/x/p/Cargo.toml","targets":[],
        "metadata":{"gate":{"checks":["takes_no_args"]}}}]}"#;
    assert!(
        derive_checks(scalar).is_err(),
        "a scalar phase must still be refused"
    );
    // and the marker must not make a REPEATED BIN skip its setup declaration
    let repeated = r#"{"packages":[{"name":"f","manifest_path":"/x/f/Cargo.toml","targets":[],
        "metadata":{"gate":{"checks":[
          {"bin":"f","args":[],"takes_no_args":true},
          {"bin":"f","args":["check"]}
        ]}}}]}"#;
    assert!(
        derive_checks(repeated).is_err(),
        "takes_no_args must not exempt a repeated bin from declaring setup"
    );
}

/// HAZARD 1 leg: a repeated bin that does not declare `setup` is REFUSED.
///
/// FIRES-ON-KNOWN-BAD against the shape real production data had until this landed:
/// `commit-build-fence` declared `["init", ...]` then `["check", ...]` as bare arrays, and its
/// `init` writes `git_dir(repo)/omp-build-registration.json`.
#[test]
fn a_repeated_bin_without_a_setup_declaration_is_refused() {
    let md = r#"{"packages":[{"name":"fence","manifest_path":"/x/f/Cargo.toml","targets":[],
        "metadata":{"gate":{"checks":[["init","--repo","{repo}"],["check","--repo","{repo}"]]}}}]}"#;
    let error = derive_checks(md).expect_err("a same-bin sequence must declare setup");
    let text = error.to_string();
    assert!(
        text.contains("setup = true") && text.contains("SEQUENCE"),
        "the refusal must name the remedy and the reason: {text}"
    );
}

/// KNOWN-GOOD, and the leg that caught my over-broad first rule: DISTINCT bins are a FAN-OUT,
/// not a sequence, and need no setup declaration.
///
/// My first version keyed the refusal on POSITION -- every phase before the last -- which refused
/// `no-shell-gate`'s three genuine checks. The rule was wrong, not the fixture.
#[test]
fn distinct_bins_are_a_fan_out_and_need_no_setup_declaration() {
    let md = r#"{"packages":[{"name":"nsg","manifest_path":"/x/n/Cargo.toml","targets":[],
        "metadata":{"gate":{"checks":[
          {"bin":"nsg","args":["--repo","{repo}"]},
          {"bin":"gate-reachability","args":["--root","{repo}"]},
          {"bin":"head-compiles-gate","args":["--repo","{repo}"]}
        ]}}}]}"#;
    let checks = derive_checks(md).expect("three distinct bins are all checks");
    assert_eq!(checks[0].phases.len(), 3);
    assert!(
        checks[0].phases.iter().all(|p| !p.setup),
        "none of them is setup, and none needed to say so"
    );
}

// ---------------------------------------------------------------------------
// eov8a: the committed ledger read is a THREE-WAY FACT, not a value with a default.
//
// `read_to_string(...).map(parse_ledger).unwrap_or_default()` coerced a read ERROR into the
// EMPTY SET, so moving `docs/gate-roster.txt` aside produced 91
// `in_workspace_absent_from_ledger` lines and exit=0. The detector could not detect its own
// deletion.
// ---------------------------------------------------------------------------

/// A throwaway workspace whose roster content is the variable under test.
///
/// `.git` is an EMPTY DIRECTORY, not a repository: it is the marker `repo_root()` stops at.
/// No worktree is created, so the zero-worktree policy needs no exception here.
fn ledger_fixture(name: &str, roster: Option<&str>) -> std::path::PathBuf {
    let root =
        std::env::temp_dir().join(format!("gate-runner-eov8a-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join(".git")).expect("marker");
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"crates/only\"]\n",
    )
    .expect("workspace manifest");
    let dir = root.join("crates/only");
    std::fs::create_dir_all(dir.join("src")).expect("crate dir");
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"only\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("crate manifest");
    std::fs::write(
        dir.join("src/lib.rs"),
        "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n",
    )
    .expect("crate source");
    if let Some(text) = roster {
        std::fs::create_dir_all(root.join("docs")).expect("docs dir");
        std::fs::write(root.join("docs/gate-roster.txt"), text).expect("roster");
    }
    root
}

fn plan_against(root: &std::path::Path) -> (i32, String, String) {
    // `repo_root()` walks UP FROM THE CURRENT DIRECTORY for a `.git`/`.beads` marker; there is
    // no `--repo` flag, and passing one makes the binary print usage and exit 2 — which is how
    // the first version of these legs failed. The fixture root IS the marker directory.
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_gate-runner"))
        .arg("--plan")
        .current_dir(root)
        .output()
        .expect("spawn gate-runner");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// ACCEPTANCE 1 + 3. An ABSENT roster is a TYPED refusal naming the path, with an exit code
/// distinct from both "agrees" and "drifted".
///
/// AND IT MUST NOT NAME DRIFT. A leg that reddens on any ledger problem cannot tell you which
/// one you got wrong; that is the whole defect being fixed, so the discrimination is asserted
/// in both directions.
#[test]
fn an_absent_ledger_is_a_typed_refusal_and_never_drift() {
    let root = ledger_fixture("absent", None);
    let (code, stdout, stderr) = plan_against(&root);

    assert_eq!(
        code,
        i32::from(gate_runner::EXIT_LEDGER_UNREADABLE),
        "an absent ledger must exit EXIT_LEDGER_UNREADABLE, not 0 and not drift: {stderr}"
    );
    assert_ne!(
        code,
        i32::from(EXIT_LEDGER_DRIFT),
        "absence is not disagreement"
    );
    assert!(
        stderr.contains("GATE_RUNNER_LEDGER_UNREAD"),
        "the refusal must be typed: {stderr}"
    );
    assert!(stderr.contains("reason=absent"), "{stderr}");
    assert!(
        stderr.contains("docs/gate-roster.txt"),
        "the refusal must name the PATH so it is actionable without a lookup: {stderr}"
    );
    // ACCEPTANCE 5, anti-vacuity: the population is reported on both sides, so a reader can see
    // the refusal is about the FILE and not an empty workspace.
    assert!(stderr.contains("entries=0"), "{stderr}");
    assert!(stderr.contains("workspace_members=1"), "{stderr}");
    // THE DISCRIMINATION: the old behaviour printed one LEDGER_DRIFT line per crate here.
    assert!(
        !stdout.contains("GATE_RUNNER_LEDGER_DRIFT"),
        "an absent ledger must not be reported as drift against it: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// ACCEPTANCE 2. An EMPTY-BUT-PRESENT roster is distinguishable from an absent one.
///
/// Both are the empty set and they are different FACTS with different repairs: repair a
/// truncated file versus restore a deleted one. Comments-only counts as empty, because
/// `parse_ledger` drops comments and a commented-out roster is a truncation.
#[test]
fn a_present_but_empty_ledger_is_distinct_from_an_absent_one() {
    let empty = ledger_fixture("empty", Some("# every row commented out\n\n"));
    let (empty_code, _, empty_err) = plan_against(&empty);
    let absent = ledger_fixture("absent2", None);
    let (absent_code, _, absent_err) = plan_against(&absent);

    assert_eq!(empty_code, i32::from(gate_runner::EXIT_LEDGER_UNREADABLE));
    assert_eq!(absent_code, i32::from(gate_runner::EXIT_LEDGER_UNREADABLE));
    assert!(empty_err.contains("reason=present_but_empty"), "{empty_err}");
    assert!(absent_err.contains("reason=absent"), "{absent_err}");
    assert_ne!(
        empty_err, absent_err,
        "a truncation and a deletion must not produce the same line"
    );
    // The REMEDIES differ, which is why one reason token would not have been enough.
    assert_ne!(
        gate_runner::LedgerRead::PresentButEmpty.remedy(),
        gate_runner::LedgerRead::Absent.remedy()
    );
    let _ = std::fs::remove_dir_all(&empty);
    let _ = std::fs::remove_dir_all(&absent);
}

/// ACCEPTANCE 4, NEGATIVE CONTROL. With the roster PRESENT and AGREEING the new arm must not
/// fire. A refusal that fires on the healthy input is worse than the silent success it replaced.
#[test]
fn an_agreeing_ledger_does_not_trip_the_new_refusal() {
    let root = ledger_fixture("agrees", Some("only\n"));
    let (code, stdout, stderr) = plan_against(&root);

    assert_ne!(
        code,
        i32::from(gate_runner::EXIT_LEDGER_UNREADABLE),
        "the healthy input must not refuse: {stderr}"
    );
    assert!(
        !stderr.contains("GATE_RUNNER_LEDGER_UNREAD"),
        "no refusal on a present, agreeing roster: {stderr}"
    );
    // And it is not vacuously quiet: the plan really ran over the fixture.
    assert!(stdout.contains("GATE_RUNNER_PLAN crates=1"), "{stdout}");
    assert!(
        !stdout.contains("GATE_RUNNER_LEDGER_DRIFT"),
        "an agreeing roster has no drift: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// The classifier itself, without a filesystem: four facts, four reasons, and `rows()` is the
/// refusal gate for every variant that is not `Rows`.
#[test]
fn the_ledger_read_classifies_four_distinct_facts() {
    use gate_runner::LedgerRead;
    use std::io::{Error, ErrorKind};

    let rows = LedgerRead::classify(Ok("alpha\nbeta\n".to_owned()));
    assert_eq!(rows.reason(), "rows");
    assert_eq!(rows.rows().map(BTreeSet::len), Some(2));

    let empty = LedgerRead::classify(Ok("# comment\n\n".to_owned()));
    let absent = LedgerRead::classify(Err(Error::from(ErrorKind::NotFound)));
    let unreadable = LedgerRead::classify(Err(Error::from(ErrorKind::PermissionDenied)));

    // THE COLLAPSE THIS TYPE EXISTS TO PREVENT: three variants carry NO rows and are still
    // three different facts. Under `unwrap_or_default` all three were one empty set.
    for read in [&empty, &absent, &unreadable] {
        assert!(read.rows().is_none(), "{read:?} must not yield rows");
    }
    let reasons: BTreeSet<&str> = [&rows, &empty, &absent, &unreadable]
        .iter()
        .map(|read| read.reason())
        .collect();
    assert_eq!(
        reasons.len(),
        4,
        "every fact needs its own reason: {reasons:?}"
    );
    assert_eq!(empty.reason(), "present_but_empty");
    assert_eq!(absent.reason(), "absent");
    assert_eq!(unreadable.reason(), "unreadable");
    // The OS detail survives only where there is one, so a reader is never shown a fabricated
    // cause.
    assert_ne!(unreadable.detail(), "none");
    assert_eq!(absent.detail(), "none");
}

/// `EXIT_LEDGER_UNREADABLE` is distinct from every other code, so a consumer keying on the exit
/// alone can tell this cause from the others.
#[test]
fn the_new_exit_code_is_distinct_from_every_other() {
    let codes = [
        EXIT_OK,
        EXIT_GATE_FAILED,
        EXIT_EMPTY_ROSTER,
        EXIT_SHORT_ROSTER,
        EXIT_LEDGER_DRIFT,
        gate_runner::EXIT_METADATA_UNREADABLE,
        gate_runner::EXIT_LEDGER_UNREADABLE,
    ];
    let distinct: BTreeSet<u8> = codes.iter().copied().collect();
    assert_eq!(
        distinct.len(),
        codes.len(),
        "exit codes must be distinct per cause: {codes:?}"
    );
}

#![forbid(unsafe_code)]

//! `omp-orchestrator-nu8lc` legs: the commit-path verdict must be partitioned by STAGED
//! MEMBERSHIP, so this gate stops refusing a commit for a crate the commit never touches.
//!
//! WHY NOT AN INDEX READER -- the distinction the bead turns on: this gate's subject is a
//! CENSUS, and `measure_untracked_members` reconciles disk against HEAD against the staged
//! set ON PURPOSE, because an untracked member is invisible to any index read by
//! definition. The surface is correct; ATTRIBUTION is the defect.
//!
//! PER-TARGET CLASS: PURE-LOGIC. No filesystem, no git, no `cargo metadata` -- these legs
//! feed reason strings and a staged list straight in, so one box suffices.

use crate_atom_gate::{commit_touches_crate, partition_by_staged};

fn reasons() -> Vec<String> {
    vec![
        "MISSING pane-truth part4 tests -- no test fn names found".to_owned(),
        "MISSING fleet-monitor part7 tick-path -- no Unrun arm".to_owned(),
    ]
}

/// KNOWN-BAD: a violation in a crate THIS COMMIT touches must stay attributable, so the
/// caller still refuses. Partitioning must never become an escape hatch.
#[test]
fn a_violation_in_a_staged_crate_stays_attributable() {
    let staged = vec![
        "crates/pane-truth/src/lib.rs".to_owned(),
        "docs/PLAN.md".to_owned(),
    ];
    let split = partition_by_staged(&reasons(), &staged);
    assert_eq!(
        split.attributable.len(),
        1,
        "the staged crate's violation must refuse: {split:?}"
    );
    assert!(split.attributable[0].contains("pane-truth"));
    assert_eq!(
        split.foreign.len(),
        1,
        "and the untouched crate's violation must be reported, not refused: {split:?}"
    );
    assert!(split.foreign[0].contains("fleet-monitor"));
}

/// KNOWN-GOOD, and it is the leg the whole change exists for: a commit touching NO crate
/// named in any violation is attributable for NOTHING -- while the foreign rows survive to
/// be printed. An empty `foreign` here would be the silencing this must not do.
#[test]
fn a_commit_touching_no_named_crate_refuses_nothing_and_still_reports() {
    let staged = vec!["AGENTS.md".to_owned(), "docs/plan/00-brief.md".to_owned()];
    let split = partition_by_staged(&reasons(), &staged);
    assert!(
        split.attributable.is_empty(),
        "a commit that touches no named crate must not be refused: {split:?}"
    );
    assert_eq!(
        split.foreign.len(),
        2,
        "SILENCE IS THE REAL WEAKENING: both violations must survive to be printed \
         {split:?}"
    );
}

/// AN UNPARSED REASON IS ATTRIBUTABLE, never silently foreign. A ceiling breach does not
/// carry the `MISSING <crate>` shape, and a reason this partitioner cannot read must fail
/// CLOSED -- otherwise a future reason format silently stops refusing anything.
#[test]
fn an_unparsable_reason_fails_closed_into_attributable() {
    let ceiling = "CEILING_HAS_SLACK part4 live=3 ceiling=5".to_owned();
    let split = partition_by_staged(&[ceiling.clone()], &["AGENTS.md".to_owned()]);
    assert_eq!(
        split.attributable,
        vec![ceiling],
        "a reason whose crate cannot be identified must REFUSE, not vanish: {split:?}"
    );
    assert!(split.foreign.is_empty());
}

/// ANTI-VACUITY plus the membership predicate's own boundaries: a prefix match must not
/// treat a SIBLING crate whose name starts with the same letters as the same crate.
#[test]
fn membership_is_exact_and_an_empty_input_is_not_a_pass() {
    assert!(commit_touches_crate(
        "pane-truth",
        &["crates/pane-truth/tests/x.rs".to_owned()]
    ));
    assert!(
        !commit_touches_crate("pane", &["crates/pane-truth/tests/x.rs".to_owned()]),
        "crates/pane-truth/ must NOT count as touching a crate named `pane` -- the \
         boundary is the directory separator"
    );
    assert!(!commit_touches_crate("pane-truth", &[]));

    let empty = partition_by_staged(&[], &["crates/pane-truth/src/lib.rs".to_owned()]);
    assert!(
        empty.attributable.is_empty() && empty.foreign.is_empty(),
        "no reasons in, no reasons out -- and the CALLER, not this function, decides that \
         zero scanned crates is Unrun rather than Pass: {empty:?}"
    );
}

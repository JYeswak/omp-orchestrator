#![forbid(unsafe_code)]

//! Legs for the r1_breadth ATTRIBUTION ruling (2026-09-11): the repo-wide READ stays, the
//! VERDICT is partitioned by staged membership.
//!
//! WHY NOT NARROW THE READ -- the distinction the ruling turns on: this gate's subject is
//! the whole flow population's breadth, and narrowing the read would change what the gate
//! MEANS. The defect is that on the commit path it refused the committer for a population
//! property nobody in this commit created.
//!
//! PER-TARGET CLASS: PURE-LOGIC. No filesystem, no git -- reasons and a staged list go
//! straight in, so one box suffices.

use r1_breadth_gate::{
    attribution_of, commit_touches_inputs, is_gate_input, Attribution, CheckError,
};

fn delta() -> CheckError {
    CheckError::DeltaExceeded {
        max: 4,
        median: 2,
        delta: 2,
        table: "box-a level=4\nbox-b level=2\n".to_owned(),
    }
}

/// KNOWN-BAD: a commit that stages an input the score is computed from must still REFUSE.
/// Partitioning is not an escape hatch, and this is the leg that says so.
#[test]
fn a_commit_staging_a_scored_subject_still_refuses() {
    for staged in [
        vec!["docs/plan/flow/boxes/box-a.toml".to_owned()],
        vec!["docs/plan/flow/CONTRACT.md".to_owned()],
        vec!["docs/plan/05-actions.md".to_owned()],
    ] {
        assert_eq!(
            attribution_of(&delta(), &staged),
            Attribution::Refuse,
            "staging {staged:?} feeds the score and must refuse"
        );
    }
}

/// KNOWN-GOOD, and the leg the change exists for: a commit touching none of the gate's
/// inputs is not refused -- while the finding SURVIVES for the caller to print. The caller
/// prints it typed; silence here would be the actual weakening.
#[test]
fn a_commit_touching_no_input_is_reported_not_refused() {
    let staged = vec![
        "crates/pane-truth/src/lib.rs".to_owned(),
        "AGENTS.md".to_owned(),
    ];
    assert_eq!(attribution_of(&delta(), &staged), Attribution::ReportForeign);
    assert_eq!(
        attribution_of(
            &CheckError::ScoreUncited {
                id: "box-a".to_owned()
            },
            &staged
        ),
        Attribution::ReportForeign
    );
    assert!(!commit_touches_inputs(&staged));
}

/// FAIL-CLOSED, carried over from GATE 8 where it was invented: an INSTRUMENT error refuses
/// whatever is staged. A gate that could not measure has not found the commit innocent, and
/// softening that into a report would convert a blind instrument into a clean bill.
#[test]
fn an_instrument_error_refuses_regardless_of_attribution() {
    let foreign = vec!["AGENTS.md".to_owned()];
    assert_eq!(
        attribution_of(
            &CheckError::Io {
                detail: "docs/plan/flow/CONTRACT.md: No such file".to_owned()
            },
            &foreign
        ),
        Attribution::Refuse,
        "an unreadable input is not a foreign finding"
    );
    assert_eq!(
        attribution_of(&CheckError::PopulationUnpinned, &foreign),
        Attribution::Refuse,
        "an unpinned population means the denominator is unknown, not that the commit is clean"
    );
}

/// ANTI-VACUITY AND BOUNDARIES: the membership predicate must match what the READER reads
/// and nothing wider, or a staged input would be called foreign -- the one direction that
/// must never happen. An EMPTY staged set touches nothing, and that is the absence of a
/// commit rather than a pass: the hook's own empty-staged gate owns that case.
#[test]
fn membership_matches_the_reader_and_nothing_wider() {
    assert!(is_gate_input("docs/plan/flow/CONTRACT.md"));
    assert!(is_gate_input("docs/plan/flow/boxes/anything.toml"));
    assert!(is_gate_input("docs/plan/00-brief.md"));
    assert!(is_gate_input("docs/plan/12-journey.md"));

    // `list_numbered_plan` does NOT recurse, so a nested numbered file is not scored and
    // must not be claimed as an input.
    assert!(!is_gate_input("docs/plan/dag/00-nested.md"));
    // Not numbered, not scored.
    assert!(!is_gate_input("docs/plan/PLAN.md"));
    assert!(!is_gate_input("docs/planning/00-brief.md"));
    assert!(!is_gate_input("crates/r1-breadth-gate/src/lib.rs"));

    assert!(!commit_touches_inputs(&[]));
    assert_eq!(
        attribution_of(&delta(), &[]),
        Attribution::ReportForeign,
        "an empty staged set is the absence of a commit; the empty-staged gate refuses it \
         upstream, and this partitioner must not invent a refusal of its own"
    );
}

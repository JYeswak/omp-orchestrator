//! Acceptance for `omp-orchestrator-block-non-arc-behind-s0-f3g5`.
//!
//! NAMED TARGET per the mmt4 ruling: `cargo test -p loop-queue-filter --test phase_gate`.
//!
//! **In-tree fixtures only.** Acceptance items 3 and 4 both forbid an applied `.patch`: `git apply`
//! silently no-ops when the index hash misses HEAD, so a patch-based known-bad is a fooled
//! certificate. Every fixture below is constructed in this file.
//!
//! **Every negative leg pins the message AND the code.** A prefix is not a message: on
//! `omp-orchestrator-djfu` a mutation left the `SALVAGE_UNKNOWN` prefix printed while the decision
//! underneath it was wrong, so a `contains(prefix)` assertion passed a mutation it was written to
//! catch. These legs assert the discriminating substrings and the exit code.

use admission_reason::Withheld;
use loop_queue_filter::phase_gate::{
    apply_phase_gate, exception_set_is_complete, is_arc_member, is_exempt, GateError,
    ARC_CENSUS_POSITIVE_CONTROL, ARC_PREFIX, EXCEPTION_SET, GATED_PHASE, PHASE_GATE_ENABLED,
    SWITCH_ON_PRECONDITION,
};

fn ids(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).to_owned()).collect()
}

/// One arc bead and one non-arc bead, both open and unblocked, phase INCOMPLETE.
fn mixed_fixture() -> Vec<String> {
    ids(&[
        "omp-orchestrator-jplf.7.2",
        "omp-orchestrator-some-unrelated-task-abcd",
    ])
}

/// ITEM 4 — FIRES-ON-KNOWN-BAD with an in-tree specimen: with the phase incomplete the non-arc
/// bead is WITHHELD, and the reason names the phase gate.
#[test]
fn a_non_arc_bead_is_withheld_with_a_typed_reason_naming_the_phase() {
    let outcome = apply_phase_gate(&mixed_fixture(), ARC_CENSUS_POSITIVE_CONTROL, false, true)
        .expect("a populated queue with a populated arc must decide, not refuse");

    assert_eq!(outcome.admitted, ids(&["omp-orchestrator-jplf.7.2"]));
    assert_eq!(outcome.withheld.len(), 1, "exactly the non-arc bead");
    assert!(outcome.gate_active, "the gate must report that it RAN");

    let (id, reason) = &outcome.withheld[0];
    assert_eq!(id, "omp-orchestrator-some-unrelated-task-abcd");

    // ITEM 6 — the reason is a CLOSED ENUM ARM from admission-reason, not free text. Matching on
    // the variant is the assertion a free string could not support.
    match reason {
        Withheld::PhaseGate {
            phase,
            blocking_bead,
        } => {
            assert_eq!(phase, GATED_PHASE, "the refusal must name the PHASE");
            assert!(
                blocking_bead.starts_with(ARC_PREFIX),
                "the refusal must name the BLOCKING BEAD, got {blocking_bead}"
            );
        }
    }
    assert_eq!(reason.code(), "phase_gate", "a stable machine token");
    // The detail must carry both facts a reader needs; a refusal naming neither is
    // indistinguishable from a dispatcher that simply stopped.
    let detail = reason.detail();
    assert!(
        detail.contains("WITHHELD_PHASE_GATE")
            && detail.contains(GATED_PHASE)
            && detail.contains("declared exception set"),
        "detail must name the gate, the phase and the exception set, got: {detail}"
    );
}

/// ITEM 5 — KNOWN-GOOD, mandatory. With the phase COMPLETE the non-arc bead is ADMITTED. Without
/// this leg the gate could withhold everything forever and still pass.
#[test]
fn a_completed_phase_admits_the_non_arc_bead() {
    let outcome = apply_phase_gate(&mixed_fixture(), ARC_CENSUS_POSITIVE_CONTROL, true, true)
        .expect("decision");
    assert_eq!(
        outcome.admitted,
        mixed_fixture(),
        "a completed phase admits everything, in input order"
    );
    assert!(outcome.withheld.is_empty());
    assert!(
        outcome.gate_active,
        "the gate RAN and chose to admit; that is a different fact from being disabled"
    );
}

/// ITEM 9 — REVERSIBLE IN ONE LINE, AND PROVEN SO. The disabled path admits the pre-gate set
/// unchanged, and reports `gate_active=false` so the two identical admitted sets remain
/// distinguishable.
#[test]
fn the_disabled_path_admits_the_pre_gate_set_unchanged() {
    let pre_gate = mixed_fixture();
    let outcome =
        apply_phase_gate(&pre_gate, ARC_CENSUS_POSITIVE_CONTROL, false, false).expect("decision");
    assert_eq!(outcome.admitted, pre_gate, "byte-for-byte the input set");
    assert!(outcome.withheld.is_empty());
    assert!(
        !outcome.gate_active,
        "a disabled gate MUST NOT report itself active — an admitted count alone cannot tell a \
         released phase from a switched-off gate"
    );

    // The shipped default is OFF, and that is asserted rather than assumed: a landing that
    // silently enabled the gate would be HD-0016's total halt with extra steps.
    assert!(
        !PHASE_GATE_ENABLED,
        "the gate must ship DISABLED; enabling it is a conductor action against a measured \
         precondition"
    );
}

/// ITEM 8 — ANTI-VACUITY WITH A POSITIVE CONTROL, and three causes with three distinct codes.
/// Withholding everything because the arc vanished is indistinguishable from a correct total halt.
#[test]
fn every_vacuous_input_is_a_distinct_typed_error_never_an_empty_admitted_set() {
    let empty: Vec<String> = Vec::new();
    let err = apply_phase_gate(&empty, ARC_CENSUS_POSITIVE_CONTROL, false, true)
        .expect_err("an empty queue is not 'nothing to dispatch'");
    assert_eq!(err, GateError::EmptyCandidateSet);
    assert_eq!(err.exit_code(), 31);
    assert!(err.to_string().contains("not 'nothing to dispatch'"));

    let err = apply_phase_gate(&mixed_fixture(), 0, false, true)
        .expect_err("a zero arc census must REFUSE, not withhold everything");
    assert_eq!(err, GateError::ArcCensusZero);
    assert_eq!(err.exit_code(), 30);
    assert!(
        err.to_string().contains(&ARC_CENSUS_POSITIVE_CONTROL.to_string()),
        "the refusal must cite the positive control so a reader can see the collapse, got: {err}"
    );

    // The three codes must be pairwise distinct, or a counter conflates two causes.
    let codes = [
        GateError::ArcCensusZero.exit_code(),
        GateError::EmptyCandidateSet.exit_code(),
        GateError::ExceptionSetIncomplete.exit_code(),
    ];
    assert_eq!(
        codes.iter().collect::<std::collections::BTreeSet<_>>().len(),
        3,
        "distinct exit code per cause"
    );

    // AND the anti-vacuity check must NOT fire on the disabled path: erroring there would make
    // the disable flag unusable in the emergency it exists for.
    let outcome = apply_phase_gate(&mixed_fixture(), 0, false, false)
        .expect("a zero census cannot make a passthrough wrong");
    assert_eq!(outcome.admitted, mixed_fixture());
}

/// ITEM 2 — the exception set is EXPLICIT, ENUMERATED and NON-EMPTY, every entry carries a reason,
/// and the three mandated members are present. An empty set is a total halt, which is HD-0016.
#[test]
fn the_exception_set_is_enumerated_non_empty_and_carries_a_reason_per_entry() {
    assert!(exception_set_is_complete(), "no entry may lack a reason");
    assert!(!EXCEPTION_SET.is_empty(), "an empty exception set FAILS item 2");

    for entry in EXCEPTION_SET {
        assert!(
            entry.reason.len() > 20,
            "a one-word reason is not a justification a reviewer can disagree with: {entry:?}"
        );
    }

    let selectors: Vec<&str> = EXCEPTION_SET.iter().map(|e| e.selector).collect();
    assert!(
        selectors
            .iter()
            .any(|s| s.contains("selector-blind-to-unpartitioned-open-kt0m")),
        "item 2 names the arc-visibility defect explicitly"
    );
    assert!(selectors.contains(&"grade"), "HD-0015 grading lane");
    assert!(selectors.contains(&"hygiene"), "HD-0015 hygiene lane");
    assert!(selectors.contains(&"unblock"), "the Phase 0 unblock path");

    // And the exceptions must actually exempt: a declared list that does not match is a list.
    assert!(is_exempt("omp-orchestrator-grade-something-1234"));
    assert!(is_exempt("omp-orchestrator-selector-blind-to-unpartitioned-open-kt0m"));
    assert!(!is_exempt("omp-orchestrator-some-unrelated-task-abcd"));
}

/// The exception path is what keeps the gate from being a total halt, so it gets its own leg
/// rather than riding on the withhold test.
#[test]
fn an_exempt_non_arc_bead_is_admitted_while_its_neighbour_is_withheld() {
    let candidates = ids(&[
        "omp-orchestrator-grade-pane2-beads-rkzv",
        "omp-orchestrator-some-unrelated-task-abcd",
    ]);
    let outcome =
        apply_phase_gate(&candidates, ARC_CENSUS_POSITIVE_CONTROL, false, true).expect("decision");
    assert_eq!(outcome.admitted, ids(&["omp-orchestrator-grade-pane2-beads-rkzv"]));
    assert_eq!(outcome.withheld.len(), 1);
    assert_eq!(
        outcome.withheld[0].0,
        "omp-orchestrator-some-unrelated-task-abcd"
    );
}

/// ITEM 7's SUBJECT — arc membership. The mutation leg deletes this check, and this test is what
/// must go RED. Kept separate from the withhold test so the failure set is attributable to the
/// membership predicate alone: a conjunctive test name is a conflated assertion advertising
/// itself, which cost `djfu` its independence claim.
#[test]
fn arc_membership_is_decided_by_the_declared_prefix_only() {
    assert!(is_arc_member("omp-orchestrator-jplf"));
    assert!(is_arc_member("omp-orchestrator-jplf.7.2"));
    assert!(!is_arc_member("omp-orchestrator-jplf-lookalike-not-arc"));
    assert!(!is_arc_member("omp-orchestrator-some-unrelated-task-abcd"));
    assert!(
        !is_arc_member("jplf.1"),
        "the prefix is the FULL id prefix, not a bare token — a bare-token match would sweep any \
         bead whose title-derived id happens to contain it"
    );
}

/// The precondition must be keyed on the LEAVES, never on `jplf.1`.
///
/// `jplf.1` is an epic and `br ready` excludes epics fleet-wide — correctly, since a container is
/// not work. A precondition naming it can never fire, which would park this gate forever. That
/// hypothesis was tested and refuted on the live tracker: dropping `jplf.1`'s parent-child edge
/// left it absent from `br ready` and cost a close-gate count, so it was restored.
#[test]
fn the_switch_on_precondition_is_not_keyed_on_an_epic() {
    assert!(
        SWITCH_ON_PRECONDITION.contains("NON-EPIC"),
        "the precondition must name non-epic members as the trigger"
    );
    assert!(
        SWITCH_ON_PRECONDITION.contains("never keyed on jplf.1"),
        "the refuted keying must stay named, so nobody re-adopts it: {SWITCH_ON_PRECONDITION}"
    );
}

/// Duplicate candidates must not produce duplicate decisions — a doubled row would double a
/// withheld count and make the operator line wrong.
#[test]
fn a_duplicated_candidate_is_decided_once() {
    let candidates = ids(&[
        "omp-orchestrator-some-unrelated-task-abcd",
        "omp-orchestrator-some-unrelated-task-abcd",
    ]);
    let outcome =
        apply_phase_gate(&candidates, ARC_CENSUS_POSITIVE_CONTROL, false, true).expect("decision");
    assert!(outcome.admitted.is_empty());
    assert_eq!(outcome.withheld.len(), 1, "decided once, not twice");
    assert!(outcome.render().contains("withheld=1"));
    assert!(
        outcome.render().contains("active=true"),
        "the operator line must state whether the gate ran: {}",
        outcome.render()
    );
}

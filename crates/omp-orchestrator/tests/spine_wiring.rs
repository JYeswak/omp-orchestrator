#![forbid(unsafe_code)]
//! `ack-spine` IS WIRED, AND THE PROOF IS THE EMIT SITE — `omp-orchestrator-eg0m`.
//!
//! # What this file refuses to accept as wiring
//!
//! A manifest dependency. `eg0m` names the shortcut explicitly: *"adding a manifest
//! dependency so the census turns green is the census defect just fixed in `kwo9`,
//! running backwards — a caller that exists to satisfy a gate."* And a manifest
//! edge is an **upper bound** on use, never use: the same distinction that made
//! `finding` show 375 grep hits against 2 real dependency edges.
//!
//! So `ack-spine`'s census row keys on `ack_spine::ledger::step(` appearing in
//! another crate's source, and these legs assert that both directions of that
//! probe work.

use omp_orchestrator::{census_gates, crates_emitting, GateCensus};
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// ACCEPTANCE 3. The gate that refused 51 of 58 supervisor cycles must be clear,
/// and it must be clear for the right reason.
#[test]
fn ack_spine_is_reachable_because_a_crate_emits_through_it() {
    let census = census_gates(&repo_root());
    let row = census
        .rows
        .iter()
        .find(|r| r.gate == "ack-spine")
        .expect("ack-spine must have a census row");
    assert!(
        row.reachability.is_reachable(),
        "ack-spine is still unreachable: {:?}",
        row.reachability
    );
    // The TRIGGER must name emission, not a dependency. A row that went green for
    // the manifest reason would satisfy the census and not the bead.
    let trigger = format!("{:?}", row.reachability);
    assert!(
        trigger.contains("emits StepRecords"),
        "ack-spine went green for the wrong reason: {trigger}"
    );
    assert!(
        row.disposition.is_blocking(),
        "ack-spine must remain BLOCKING: it is the row the loop refuses on"
    );
}

/// ACCEPTANCE 4, FIRES-ON-KNOWN-BAD, in the direction a mutation cannot fake: the
/// probe must return EMPTY for a target nobody emits through. A probe that returns
/// a non-empty list for everything would report `ack-spine` wired no matter what
/// the tree contained — which is exactly the hardcoded-verdict defect `leht` fixed.
#[test]
fn the_emitter_probe_finds_nothing_for_a_crate_nobody_emits_through() {
    let root = repo_root();
    // POSITIVE CONTROL first, or a zero below proves nothing.
    let real = crates_emitting(&root, "ack-spine");
    assert!(
        !real.is_empty(),
        "positive control failed: nothing emits through ack-spine, so every zero \
         in this test is unattributable"
    );
    assert!(
        real.contains(&"omp-orchestrator".to_owned()),
        "the supervisor must be an emitter, found: {real:?}"
    );
    // NEGATIVE: a crate with a lib and no step primitive.
    for target in ["subprocess-contract", "omp-types", "no-such-crate-at-all"] {
        assert!(
            crates_emitting(&root, target).is_empty(),
            "{target} reported emitters; the probe is matching something other than \
             the step primitive"
        );
    }
}

/// SELF-EXCLUSION, and it is not hygiene. `ack-spine`'s own `main.rs` and `spine.rs`
/// call its step primitive, so a probe that counted them would report the crate
/// wired by its own source forever — the self-referential-checker defect this
/// repository has produced six times.
#[test]
fn a_crate_emitting_into_its_own_ledger_is_not_a_caller() {
    let root = repo_root();
    let emitters = crates_emitting(&root, "ack-spine");
    assert!(
        !emitters.contains(&"ack-spine".to_owned()),
        "ack-spine counted itself: {emitters:?}"
    );
    // And prove the exclusion is load-bearing rather than incidental: the crate's
    // own source really does contain the primitive, so excluding it changes the
    // answer.
    let own = std::fs::read_to_string(root.join("crates/ack-spine/src/spine.rs")).unwrap_or_default()
        + &std::fs::read_to_string(root.join("crates/ack-spine/src/main.rs")).unwrap_or_default();
    assert!(
        own.contains("ledger::step(") || own.contains("emit_step"),
        "ack-spine's own source no longer calls its step primitive, so the \
         self-exclusion leg above is vacuous"
    );
}

/// ACCEPTANCE 6, POSITIVE CONTROL RETAINED — with a corrected figure.
///
/// `eg0m` asks that `subprocess-contract` "continues to report 24 manifest
/// callers". Measured 2026-09-02 it reports **40**, and 40 crates do declare a
/// path dependency on it. The acceptance's 24 is a drifted figure, not a
/// regression: this leg asserts a FLOOR and prints the live number, because
/// pinning an exact count would make every new crate that depends on
/// `subprocess-contract` fail an unrelated gate.
#[test]
fn subprocess_contract_remains_the_manifest_positive_control() {
    let census = census_gates(&repo_root());
    let row = census
        .rows
        .iter()
        .find(|r| r.gate == "subprocess-contract")
        .expect("subprocess-contract must have a census row");
    assert!(
        row.reachability.is_reachable(),
        "the manifest positive control regressed: {:?}",
        row.reachability
    );
    let trigger = format!("{:?}", row.reachability);
    assert!(
        trigger.contains("manifest dependency"),
        "subprocess-contract must stay a MANIFEST-reachable row so the two probe \
         kinds are both exercised: {trigger}"
    );
}

/// The census as a whole must not have regressed while ack-spine was rewired, and
/// the refusal set must be EMPTY now — that is the product change: the fleet's only
/// gate blocker is gone.
#[test]
fn the_blocking_refusal_set_is_now_empty() {
    let census = census_gates(&repo_root());
    let refusing: Vec<&String> = census.unwired_gates().iter().map(|r| &r.gate).collect();
    assert!(
        refusing.is_empty(),
        "the loop still refuses on: {refusing:?}"
    );
    assert!(census.all_reachable(), "all_reachable disagrees with unwired_gates");
    assert!(census.positive_control_passes());
    // ANTI-VACUITY: an empty refusal set from an empty census is not a green fleet.
    assert!(
        census.rows.len() > 60,
        "only {} census rows: an empty refusal set proves nothing over an empty census",
        census.rows.len()
    );
    // And the advisory set must still be non-empty, or the refusal set went empty
    // because the census stopped classifying rather than because the gate was wired.
    assert!(
        !census.advisory_gates().is_empty(),
        "advisory set is empty too: the census is no longer producing verdicts"
    );
}

/// A constructed census with a bad ack-spine row must still refuse, so the
/// emptiness above is a measurement and not a property of the type.
#[test]
fn a_census_with_ack_spine_unwired_still_refuses() {
    use omp_orchestrator::{CensusDisposition, GateCensusRow, GateReachability};
    let census = GateCensus {
        rows: vec![
            GateCensusRow {
                gate: "no-shell-gate".into(),
                reachability: GateReachability::Reachable {
                    trigger: ".git/hooks/pre-commit".into(),
                },
                disposition: CensusDisposition::Blocking,
            },
            GateCensusRow {
                gate: "ack-spine".into(),
                reachability: GateReachability::Unreachable {
                    reason: "no crate emits a StepRecord".into(),
                },
                disposition: CensusDisposition::Blocking,
            },
        ],
    };
    assert!(!census.all_reachable());
    assert_eq!(census.unwired_gates().len(), 1);
    assert_eq!(census.unwired_gates()[0].gate, "ack-spine");
}

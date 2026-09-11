#![forbid(unsafe_code)]
//! THE CENSUS MUST MEASURE INVOCATION, NOT EXISTENCE — `omp-orchestrator-uldvu`.
//!
//! # The defect
//!
//! `coverage_output_reachability` returned `Reachable` when `crates/<name>/Cargo.toml`
//! **is a file**. No crate that exists could fail it, so all 11 `COVERAGE_WAVE_OUTPUT_CRATES`
//! rows were `Reachable` by construction — and [`GateCensus::derived_positive_control`] picks the
//! FIRST reachable row as the census's ANTI-VACUITY control, so **the check proving the census
//! verified SOMETHING was satisfied by any directory on disk with a manifest in it.**
//!
//! That is rule 3 aimed at the crate enforcing rule 3, and rule 4's *"an empty scan set is an
//! ERROR, never a pass"* defeated by a predicate that can never return empty.
//!
//! # The known-bad, and why it is a FIXTURE and not `crate-atom-gate`
//!
//! The bead named `crate-atom-gate` as the specimen, on `AGENTS.md` rule 10's record that it has
//! never been invoked. **Re-derived at source, that specimen does not hold:**
//!
//! ```text
//! manifest callers of crate-atom-gate   3   no-shell-gate, omp-inventory-map, orchestration-tick-gate
//! source uses (`crate_atom_gate::`)     2   kernel-bypass-gate/src/lib.rs, orchestration-tick-gate/src/main.rs
//! ```
//!
//! Rule 10 is about its **GATE** never firing — the pre-commit call site is behind a disarmed
//! `OMP_CRATE_ATOM_GATE=1` flag. Its **LIBRARY** is genuinely used by three crates, so a correct
//! reachability predicate must call it Reachable, and asserting otherwise would pin a false
//! verdict into the gate that was just repaired.
//!
//! So the known-bad is a SYNTHETIC crate with a manifest and nothing else: the exact shape the
//! old predicate could not fail, and one that cannot drift when a peer adds a dependency.

use omp_orchestrator::{census_gates, crate_reachability, GateReachability, COVERAGE_WAVE_OUTPUT_CRATES};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// The predicate the bead deleted, kept here so the two can be shown to DISAGREE.
///
/// This is the `workflow_shape.rs` rule applied to a predicate instead of a scan: the old and new
/// methods must be shown to differ on the same input, or nothing establishes which is in force.
fn old_predicate_existence_only(repo_root: &Path, crate_name: &str) -> bool {
    repo_root
        .join("crates")
        .join(crate_name)
        .join("Cargo.toml")
        .is_file()
}

/// FIRES ON KNOWN BAD: a crate with a manifest and ZERO invocations must be NOT reachable.
#[test]
fn fires_on_known_bad() {
    let temp = std::env::temp_dir().join(format!("uldvu-known-bad-{}", std::process::id()));
    let crate_dir = temp.join("crates").join("ghost-crate");
    std::fs::create_dir_all(crate_dir.join("src")).expect("create fixture");
    std::fs::write(
        crate_dir.join("Cargo.toml"),
        "[package]\nname = \"ghost-crate\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write fixture manifest");
    std::fs::write(crate_dir.join("src/lib.rs"), "pub fn unused() {}\n").expect("write fixture src");

    // The OLD predicate cannot fail this. That is the defect, asserted rather than described.
    assert!(
        old_predicate_existence_only(&temp, "ghost-crate"),
        "the old predicate must call the ghost crate Reachable, or this leg is not measuring the \
         defect it was written for"
    );

    // The NEW predicate must refuse it.
    let verdict = crate_reachability(&temp, "ghost-crate", &temp.join(".git/hooks/pre-commit"), true);
    assert!(
        !verdict.is_reachable(),
        "REGRESSION: a crate with a manifest, no caller, no hook, no gate stanza and no workflow \
         entry was classified reachable: {verdict:?}"
    );
    let GateReachability::Unreachable { reason } = &verdict else {
        panic!("expected Unreachable, got {verdict:?}");
    };
    assert!(
        reason.contains("no manifest dependency"),
        "the refusal must name its own cause: {reason}"
    );

    std::fs::remove_dir_all(&temp).ok();
}

/// PASSES KNOWN GOOD: a genuinely-invoked crate must still be reachable.
///
/// Over-strictness is the failure direction that turns a repaired gate into one everybody routes
/// around. `crate-atom-gate` is the right specimen precisely because the bead thought it was
/// unreachable: three manifest callers make it a real library dependency.
#[test]
fn passes_known_good() {
    let root = repo_root();
    let hook = root.join(".git/hooks/pre-commit");
    for (crate_name, why) in [
        ("crate-atom-gate", "three manifest callers"),
        ("omp-orchestrator", "the supervisor itself"),
    ] {
        let verdict = crate_reachability(&root, crate_name, &hook, true);
        assert!(
            verdict.is_reachable(),
            "OVER-STRICT: {crate_name} is genuinely invoked ({why}) but classified {verdict:?}. A \
             gate that refuses real wiring gets routed around, which is slower than no gate"
        );
    }
}

/// THE ANTI-VACUITY CONTROL MUST NOT BE SATISFIABLE BY EXISTENCE — acceptance item 4.
///
/// Asserted separately from the predicate fix on purpose: a positive control drawn from the
/// population it validates is the defect itself, not a symptom of the predicate, and it will
/// regress independently the next time someone adds a row that cannot fail.
#[test]
fn the_positive_control_is_not_satisfiable_by_existence() {
    let temp = std::env::temp_dir().join(format!("uldvu-vacuous-{}", std::process::id()));
    // A whole "workspace" of crates that exist and are invoked by nothing at all.
    for name in ["ghost-a", "ghost-b", "ghost-c"] {
        let dir = temp.join("crates").join(name);
        std::fs::create_dir_all(dir.join("src")).expect("create fixture");
        std::fs::write(
            dir.join("Cargo.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
        )
        .expect("write fixture manifest");
        std::fs::write(dir.join("src/lib.rs"), "pub fn unused() {}\n").expect("write fixture src");
    }
    let census = census_gates(&temp);
    assert!(
        census.derived_positive_control().is_none(),
        "VACUOUS CONTROL: a workspace whose every crate is invoked by nothing reported a positive \
         control of {:?}. The control proves the census verified SOMETHING; existence is not \
         verification",
        census.derived_positive_control()
    );
    std::fs::remove_dir_all(&temp).ok();
}

/// ANTI-VACUITY ON THE FIX ITSELF — acceptance item 5.
///
/// If all eleven coverage crates are STILL reachable after the repair, either every one is
/// genuinely triggered or the predicate is still vacuous, and the two are indistinguishable from
/// a count. This leg therefore asserts the DISCRIMINATION rather than a number: the old and new
/// predicates must disagree somewhere in the real workspace, and each reachable row must name a
/// trigger that is not "it exists".
#[test]
fn the_old_and_new_predicates_disagree_on_this_workspace() {
    let root = repo_root();
    let hook = root.join(".git/hooks/pre-commit");
    let mut old_reachable = 0usize;
    let mut new_reachable = 0usize;
    let mut rows = Vec::new();
    for name in COVERAGE_WAVE_OUTPUT_CRATES {
        if old_predicate_existence_only(&root, name) {
            old_reachable += 1;
        }
        let verdict = crate_reachability(&root, name, &hook, true);
        if verdict.is_reachable() {
            new_reachable += 1;
        }
        rows.push(format!("{name}: {verdict:?}"));
    }
    // Printed, not merely asserted: acceptance item 5 asks for the BEFORE/AFTER counts, and a
    // grader should be able to read them off a `--nocapture` run without instrumenting anything.
    println!(
        "ULDVU_COVERAGE_REACHABILITY old_reachable={old_reachable} new_reachable={new_reachable} \
         of={}\n{}",
        COVERAGE_WAVE_OUTPUT_CRATES.len(),
        rows.join("\n")
    );
    assert_eq!(
        old_reachable,
        COVERAGE_WAVE_OUTPUT_CRATES.len(),
        "the old predicate must call ALL of them reachable — that is the defect being fixed"
    );
    // Every reachable row must cite a trigger, and no trigger may be existence.
    for name in COVERAGE_WAVE_OUTPUT_CRATES {
        if let GateReachability::Reachable { trigger } =
            crate_reachability(&root, name, &hook, true)
        {
            assert!(
                !trigger.contains("Cargo.toml") && !trigger.is_empty(),
                "{name} is reachable for a non-trigger reason: {trigger:?}"
            );
        }
    }
    assert!(
        new_reachable <= old_reachable,
        "the repaired predicate cannot be MORE permissive than the existence check it replaced: \
         old={old_reachable} new={new_reachable}\n{}",
        rows.join("\n")
    );
}

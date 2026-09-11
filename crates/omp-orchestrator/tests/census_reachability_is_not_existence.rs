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
//!
//! # SECOND PASS — the scheduler arm, and the leg that survived a revert
//!
//! The first pass shipped three of acceptance leg 2's four trigger classes and omitted **the
//! crontab/launchd row**. That omission emitted a FALSE VERDICT, not merely an incomplete one:
//! `fleet-composite` is executed every twenty minutes by a live cron row and the census called it
//! *"a binary with no invocation site"*. Two legs are added here — one binding the scheduler
//! surface to the verdict, one proving a commented-out or path-mentioning row is NOT a trigger —
//! and `the_old_and_new_predicates_disagree_on_this_workspace` is tightened, because its
//! assertions (`new <= old`, plus a ban on a substring the old trigger never contained) all held
//! under a literal revert of the coverage path.

use omp_orchestrator::{
    census_gates, crate_bin_names, crate_reachability, GateReachability, SchedulerSurfaces,
    COVERAGE_WAVE_OUTPUT_CRATES,
};
use std::path::{Path, PathBuf};

/// A crontab body carrying the row measured on this machine, plus three adversarial rows that
/// pin the probe's boundaries.
///
/// A FIXTURE rather than the live `crontab -l`, because the verdict must be the same on this Mac
/// and on the build worker that actually compiles this crate — a test whose answer depends on
/// which host ran it is the load-dependent-oracle defect `census_gates` already carries a comment
/// about. The live surface is observed separately, in
/// [`the_live_scheduler_surface_is_observed_and_reported_honestly`].
///
/// Measured 2026-09-10, `crontab -l | sed -n '74p'` on this machine:
///
/// ```text
/// 6,26,46 * * * * FLEET_INVOKER=SCHEDULED ~/.local/bin/fleet-composite >> …log 2>&1
/// ```
///
/// **NOT byte-verbatim in one respect, and the difference is deliberate:** the author-machine
/// home prefix is replaced by `/usr/local`, because `path-literal-guard` refuses that literal in
/// source and building it with `concat!` to slip past the gate is gate-laundering. Nothing is
/// lost — the probe keys on the executor's BASENAME, so the leading directory is not an input to
/// the property under test. The schedule expression, the `FLEET_INVOKER=` env prefix and the
/// redirection are unmodified.
const MEASURED_CRONTAB: &str = concat!(
    "# fleet grade must be computed ON A SCHEDULE (wired-but-inert-guard WIRED_ROWS)\n",
    "MAILTO=\"\"\n",
    "6,26,46 * * * * FLEET_INVOKER=SCHEDULED /usr/local/bin/fleet-composite",
    " >> /usr/local/state/flywheel/fleet-composite.log 2>&1\n",
    "*/10 * * * * cd /usr/local/src/omp-orchestrator && /usr/bin/true\n",
    "#0 * * * * /usr/local/bin/kernel-only-operator-hook\n",
);

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

    // The NEW predicate must refuse it — including with the scheduler surface FULLY POPULATED,
    // so the new arm cannot be the thing that rescues it.
    let scheduler = SchedulerSurfaces::from_crontab_text(MEASURED_CRONTAB);
    let verdict = crate_reachability(
        &temp,
        "ghost-crate",
        &temp.join(".git/hooks/pre-commit"),
        true,
        &scheduler,
    );
    assert!(
        !verdict.is_reachable(),
        "REGRESSION: a crate with a manifest, no caller, no hook, no gate stanza, no workflow \
         entry and no scheduler row was classified reachable: {verdict:?}"
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
    let scheduler = SchedulerSurfaces::empty();
    for (crate_name, why) in [
        ("crate-atom-gate", "three manifest callers"),
        ("omp-orchestrator", "the supervisor itself"),
    ] {
        let verdict = crate_reachability(&root, crate_name, &hook, true, &scheduler);
        assert!(
            verdict.is_reachable(),
            "OVER-STRICT: {crate_name} is genuinely invoked ({why}) but classified {verdict:?}. A \
             gate that refuses real wiring gets routed around, which is slower than no gate"
        );
    }
}

/// THE SCHEDULER IS A TRIGGER SURFACE — acceptance leg 2's fourth class, and its known-bad.
///
/// `fleet-composite` has no manifest caller, is not named by the hook, declares no
/// `[package.metadata.gate]` and appears in no workflow. Every one of those is true and the crate
/// still runs every twenty minutes. The whole verdict therefore turns on ONE input, which is what
/// makes the known-bad exact: remove the cron row from the probe's input and the row must go back
/// to Unreachable.
#[test]
fn the_scheduler_surface_decides_fleet_composite() {
    let root = repo_root();
    let hook = root.join(".git/hooks/pre-commit");

    let with_cron = SchedulerSurfaces::from_crontab_text(MEASURED_CRONTAB);
    let verdict = crate_reachability(&root, "fleet-composite", &hook, true, &with_cron);
    let GateReachability::Reachable { trigger } = &verdict else {
        panic!(
            "FALSE UNREACHABLE: fleet-composite is executed by a live cron row and the census \
             said {verdict:?}"
        );
    };
    assert!(
        trigger.starts_with("crontab row (6,26,46 * * * *)")
            && trigger.ends_with("/fleet-composite"),
        "the trigger must CITE the row that fires it, not merely assert reachability: {trigger}"
    );

    // KNOWN-BAD: the same crate, the same tree, the cron row removed from the probe's input.
    let without_cron = SchedulerSurfaces::empty();
    let verdict = crate_reachability(&root, "fleet-composite", &hook, true, &without_cron);
    let GateReachability::Unreachable { reason } = &verdict else {
        panic!("VACUOUS SCHEDULER ARM: with no scheduler surface at all, {verdict:?}");
    };
    assert!(
        reason.contains("surfaces probed") && reason.contains("no scheduler surface was readable"),
        "a refusal must name what it consulted instead of asserting an absence: {reason}"
    );
    assert!(
        !reason.contains("no invocation site:"),
        "the old reason string asserted absence in the confident voice; it must not come back: \
         {reason}"
    );
}

/// A MENTION IS NOT AN INVOCATION — the anti-widening pin on the new arm.
///
/// The bead exists because a predicate was over-broad, so the repair must not be. Three rows in
/// [`MEASURED_CRONTAB`] are traps: a COMMENTED-OUT row naming `kernel-only-operator-hook`, a live
/// row whose command merely `cd`s into a directory called `omp-orchestrator`, and a `MAILTO=`
/// configuration line. None of the three is an invocation of a crate's binary, and a probe that
/// matched the crate NAME as a substring of the line would fire on all three.
#[test]
fn a_mentioned_crate_is_not_a_scheduled_crate() {
    let root = repo_root();
    let hook = root.join(".git/hooks/pre-commit");
    let scheduler = SchedulerSurfaces::from_crontab_text(MEASURED_CRONTAB);

    // Comments are stripped BEFORE matching: two live rows out of five lines.
    let executors: Vec<&str> = scheduler
        .rows()
        .iter()
        .map(|row| row.executor_name())
        .collect();
    assert_eq!(
        executors,
        vec!["fleet-composite", "cd"],
        "expected exactly the two live rows, with the `FLEET_INVOKER=` env prefix skipped and the \
         commented row and the MAILTO= line dropped: {executors:?}"
    );

    // `kernel-only-operator-hook` is named in the crontab TEXT and is still not scheduled.
    assert!(
        scheduler
            .invoker_of(&crate_bin_names(&root, "kernel-only-operator-hook"))
            .is_none(),
        "a commented-out cron row is not a trigger"
    );
    let verdict = crate_reachability(&root, "kernel-only-operator-hook", &hook, true, &scheduler);
    assert!(
        !verdict.is_reachable(),
        "OVER-BROAD: kernel-only-operator-hook has no live trigger on any surface and must stay \
         the census's one genuine BUILT != WIRED finding: {verdict:?}"
    );
}

/// THE LIVE SURFACE IS OBSERVED, and what it found is reported rather than assumed.
///
/// This leg asserts a CONDITIONAL, on purpose: the crontab of the host that compiles this crate
/// is not the crontab of the host that owns the fleet, so "a cron row exists" is not a property
/// of the source tree and must not be asserted as one. What IS asserted is the implication the
/// census depends on — if the live surface names a crate's binary, that crate is Reachable and
/// cites the row.
#[test]
fn the_live_scheduler_surface_is_observed_and_reported_honestly() {
    let root = repo_root();
    let hook = root.join(".git/hooks/pre-commit");
    let live = SchedulerSurfaces::observe();
    println!(
        "ULDVU_LIVE_SCHEDULER surfaces=[{}] rows={}",
        live.describe(),
        live.rows().len()
    );
    for name in COVERAGE_WAVE_OUTPUT_CRATES {
        let bins = crate_bin_names(&root, name);
        let Some(row) = live.invoker_of(&bins) else {
            continue;
        };
        println!("ULDVU_LIVE_SCHEDULED {name} <- {} {}", row.surface, row.executor);
        let verdict = crate_reachability(&root, name, &hook, true, &live);
        assert!(
            verdict.is_reachable(),
            "{name} is invoked by a live scheduler row on this host and the census said \
             {verdict:?}"
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
/// # Why this leg was rewritten
///
/// **It passed under the grader's vacuity mutation, which is the thing it exists to catch.** Its
/// assertions were `old_reachable == 11`, `new_reachable <= old_reachable` — and `11 <= 11` holds
/// — and a ban on the substring `"Cargo.toml"` appearing in a trigger, **which the old trigger
/// string (`supervisor:coverage-output-census -> crates/<name>`) never contained.** A literal
/// revert of the coverage path therefore left this named leg green. The suite still fired,
/// through `fires_on_known_bad` and the positive-control leg, so this was a MISLABELLED LEG
/// rather than an unprotected property — but a leg whose name claims discrimination must assert
/// discrimination.
///
/// Three changes: the count comparison is STRICT, the ban covers the old trigger's actual text,
/// and the rows that move are pinned BY NAME so the leg cannot be satisfied by a different row
/// happening to drop out.
#[test]
fn the_old_and_new_predicates_disagree_on_this_workspace() {
    let root = repo_root();
    let hook = root.join(".git/hooks/pre-commit");
    let scheduler = SchedulerSurfaces::from_crontab_text(MEASURED_CRONTAB);
    let mut old_reachable = 0usize;
    let mut new_reachable = 0usize;
    let mut rows = Vec::new();
    for name in COVERAGE_WAVE_OUTPUT_CRATES {
        if old_predicate_existence_only(&root, name) {
            old_reachable += 1;
        }
        let verdict = crate_reachability(&root, name, &hook, true, &scheduler);
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
    // Every reachable row must cite a trigger, and no trigger may be existence. The second needle
    // is the OLD trigger's literal text: banning `Cargo.toml` banned a string it never emitted.
    for name in COVERAGE_WAVE_OUTPUT_CRATES {
        if let GateReachability::Reachable { trigger } =
            crate_reachability(&root, name, &hook, true, &scheduler)
        {
            assert!(
                !trigger.contains("Cargo.toml")
                    && !trigger.contains("coverage-output-census")
                    && !trigger.is_empty(),
                "{name} is reachable for a non-trigger reason: {trigger:?}"
            );
        }
    }
    // STRICT. `<=` is satisfied by a literal revert, which is how this leg survived the mutation.
    assert!(
        new_reachable < old_reachable,
        "NO DISCRIMINATION: the repaired predicate must refuse at least one row the existence \
         check accepted, or nothing establishes which predicate is in force: old={old_reachable} \
         new={new_reachable}\n{}",
        rows.join("\n")
    );
    // THE TWO ROWS THAT MOVE, PINNED BY NAME. A count alone cannot tell "the scheduler arm
    // rescued fleet-composite and kernel-only-operator-hook is genuinely dead" apart from any
    // other 10/11 split.
    let fleet = crate_reachability(&root, "fleet-composite", &hook, true, &scheduler);
    let GateReachability::Reachable { trigger } = &fleet else {
        panic!("fleet-composite must be Reachable via its scheduler row, got {fleet:?}");
    };
    assert!(
        trigger.starts_with("crontab row "),
        "fleet-composite must cite the cron row, not some other arm: {trigger}"
    );
    let hook_crate = crate_reachability(&root, "kernel-only-operator-hook", &hook, true, &scheduler);
    assert!(
        !hook_crate.is_reachable(),
        "kernel-only-operator-hook has no trigger on any probed surface and is the census's one \
         genuine BUILT != WIRED finding; a predicate that calls it reachable has widened back to \
         existence: {hook_crate:?}"
    );
}

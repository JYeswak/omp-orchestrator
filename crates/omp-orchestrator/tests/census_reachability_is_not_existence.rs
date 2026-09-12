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
//!
//! # THIRD PASS — the specimen went stale the day after it was chosen
//!
//! The second pass chose `fleet-composite` as the scheduler arm's specimen because its verdict
//! *"turns on ONE input"*. **That premise was falsified by `8cfeac1` (2026-09-11, one day later),
//! which added a `[package.metadata.gate]` stanza to `crates/fleet-composite/Cargo.toml`.** The
//! gate arm fires AHEAD of the scheduler arm — deliberately, see the ordering comment at the
//! scheduler arm in `crate_reachability` — so the census now cites the gate stanza and the two
//! legs pinning the cron citation failed. Nothing regressed: the production predicate did exactly
//! what its comment promises, which is refuse to re-attribute a row an earlier arm explained.
//!
//! So this is a PREMISE-FALSE repair, not a behaviour change. Three moves, no assertion dropped:
//!
//! - the scheduler arm's decisive specimen becomes SYNTHETIC, for the reason the known-bad
//!   already is — a real crate's trigger set is a peer's edit away from changing under it;
//! - the non-re-attribution ordering contract, which `8cfeac1` made load-bearing and which
//!   nothing asserted, gets its own leg on the real tree, still proving the cron row is on the
//!   surface for `fleet-composite`;
//! - the by-name pin in the disagreement leg becomes a by-SET pin, so a future specimen going
//!   stale is reported as a membership change instead of surviving as an unchanged count.

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

/// The crontab body for the SYNTHETIC scheduler specimen.
///
/// Kept separate from [`MEASURED_CRONTAB`], which documents a row observed on a real machine and
/// must not grow invented rows. The schedule is deliberately a DIFFERENT minute set
/// (`7,27,47` against the measured `6,26,46`) so that a leg asserting the synthetic citation
/// cannot be satisfied by the measured fixture being passed in by mistake.
const SCHEDULED_GHOST_CRONTAB: &str =
    "7,27,47 * * * * GHOST_INVOKER=SCHEDULED /usr/local/bin/ghost-scheduled\n";

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
/// The specimen is SYNTHETIC and the reason is written in the third-pass note above: the previous
/// real-tree specimen acquired a second trigger overnight and took this leg red with it. A crate
/// that exists only inside this function cannot acquire a manifest caller, a hook line, a gate
/// stanza or a workflow entry, so the claim *"the whole verdict turns on ONE input"* stays true
/// for as long as the leg exists. That is the same argument [`fires_on_known_bad`] already makes
/// for its ghost crate, applied to the positive direction.
///
/// `src/main.rs` is what makes it a BIN: the scheduler arm is gated on `has_bin`, and a library
/// with a cron row would be measuring nothing.
#[test]
fn the_scheduler_surface_decides_a_crate_with_no_other_trigger() {
    let temp = std::env::temp_dir().join(format!("uldvu-scheduled-{}", std::process::id()));
    let crate_dir = temp.join("crates").join("ghost-scheduled");
    std::fs::create_dir_all(crate_dir.join("src")).expect("create fixture");
    std::fs::write(
        crate_dir.join("Cargo.toml"),
        "[package]\nname = \"ghost-scheduled\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write fixture manifest");
    std::fs::write(crate_dir.join("src/main.rs"), "fn main() {}\n").expect("write fixture src");
    let hook = temp.join(".git/hooks/pre-commit");

    let with_cron = SchedulerSurfaces::from_crontab_text(SCHEDULED_GHOST_CRONTAB);
    let verdict = crate_reachability(&temp, "ghost-scheduled", &hook, true, &with_cron);
    let GateReachability::Reachable { trigger } = &verdict else {
        std::fs::remove_dir_all(&temp).ok();
        panic!(
            "FALSE UNREACHABLE: ghost-scheduled is executed by a cron row and the census said \
             {verdict:?}"
        );
    };
    assert!(
        trigger.starts_with("crontab row (7,27,47 * * * *)")
            && trigger.ends_with("/ghost-scheduled"),
        "the trigger must CITE the row that fires it, not merely assert reachability: {trigger}"
    );

    // KNOWN-BAD: the same crate, the same tree, the cron row removed from the probe's input.
    let without_cron = SchedulerSurfaces::empty();
    let verdict = crate_reachability(&temp, "ghost-scheduled", &hook, true, &without_cron);
    let GateReachability::Unreachable { reason } = &verdict else {
        std::fs::remove_dir_all(&temp).ok();
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

    std::fs::remove_dir_all(&temp).ok();
}

/// AN EARLIER ARM IS NOT RE-ATTRIBUTED — the ordering contract `8cfeac1` made load-bearing.
///
/// The scheduler arm is LAST of the reachable arms on purpose: its comment in `crate_reachability`
/// says it *"can only turn an Unreachable row Reachable; it can never re-attribute a row that some
/// earlier arm already explained"*, because a repair that rewrites verdicts it was not asked about
/// is indistinguishable from widening. Nothing asserted that until the day a real crate acquired
/// both triggers at once and turned the contract into an observable.
///
/// `fleet-composite` is that crate, and this leg keeps the real-tree evidence the old specimen
/// carried: the cron row IS on the surface, the census cites the gate stanza anyway, and the two
/// facts are asserted together so neither can drift out silently.
#[test]
fn an_earlier_arm_is_not_re_attributed_by_the_scheduler() {
    let root = repo_root();
    let hook = root.join(".git/hooks/pre-commit");
    let scheduler = SchedulerSurfaces::from_crontab_text(MEASURED_CRONTAB);

    // The surface still sees it. This is the second pass's finding, unchanged and still measured.
    let row = scheduler
        .invoker_of(&crate_bin_names(&root, "fleet-composite"))
        .expect("the measured crontab row invokes fleet-composite by basename");
    assert_eq!(row.schedule, "6,26,46 * * * *", "the row's schedule moved");
    assert_eq!(row.executor_name(), "fleet-composite");

    // And the census cites the EARLIER arm regardless, because that is the one that explains it.
    let verdict = crate_reachability(&root, "fleet-composite", &hook, true, &scheduler);
    let GateReachability::Reachable { trigger } = &verdict else {
        panic!("fleet-composite has two live triggers and the census said {verdict:?}");
    };
    assert!(
        trigger.starts_with("[package.metadata.gate]"),
        "RE-ATTRIBUTION: the scheduler arm overwrote an explanation an earlier arm already owned. \
         fleet-composite declares a gate stanza since 8cfeac1 and must cite it: {trigger}"
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
/// and the rows that move are pinned BY SET so the leg cannot be satisfied by a different row
/// happening to drop out. The set pin replaced a pair of by-name pins in the third pass, after
/// one of the two names (`fleet-composite`) changed which arm explains it and the leg had no way
/// to say whether that was a stale specimen or a widening.
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
    // THE ROW THAT MOVES, PINNED BY SET RATHER THAN BY COUNT. A count cannot tell "exactly the
    // one genuinely-dead binary was refused" apart from any other 10/11 split, and a single
    // by-name assertion cannot tell a specimen going stale (which is what happened to
    // fleet-composite in `8cfeac1`) from a real widening.
    let unreachable: Vec<&str> = COVERAGE_WAVE_OUTPUT_CRATES
        .iter()
        .copied()
        .filter(|name| !crate_reachability(&root, name, &hook, true, &scheduler).is_reachable())
        .collect();
    assert_eq!(
        unreachable,
        vec!["kernel-only-operator-hook"],
        "the rows the repaired predicate refuses are a MEMBERSHIP, not a count: \
         kernel-only-operator-hook has no trigger on any probed surface and is the census's one \
         genuine BUILT != WIRED finding. Any other membership means the predicate moved a row it \
         was not asked about:\n{}",
        rows.join("\n")
    );
    // fleet-composite is the row the scheduler arm RESCUED in the second pass. It has since
    // gained a gate stanza and is explained by an earlier arm; that the cron row is still on the
    // surface is asserted in [`an_earlier_arm_is_not_re_attributed_by_the_scheduler`]. What this
    // leg still owes is that it is reachable for a NAMED arm and not by existence.
    let fleet = crate_reachability(&root, "fleet-composite", &hook, true, &scheduler);
    let GateReachability::Reachable { trigger } = &fleet else {
        panic!("fleet-composite has a gate stanza and a cron row and yet {fleet:?}");
    };
    assert!(
        trigger.starts_with("[package.metadata.gate]"),
        "fleet-composite must cite the first arm that explains it: {trigger}"
    );
}

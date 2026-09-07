//! Uncaptured platform-scope reporter for the process-group test property.
//!
//! `omp-orchestrator-nndr`. **This file carried the same false claim as `src/lib.rs` and is
//! corrected here too** — it said *"Linux is explicitly UNMEASURED"*, and Linux measures this
//! property green. The stale `retry_if=darwin-production-group-kill-reap-defect` token pointed at
//! `o3eb`, six `/bin/kill` sites passing a negative pgid with no `--`; that was fixed at `e4c9138`,
//! an ancestor of the scoping commit `0622849` and nineteen hours earlier.
//!
//! HARNESS-FREE ON PURPOSE, and it is the reason this target exists at all: libtest CAPTURES stdout
//! for a passing test, so a verdict printed from a `#[test]` is invisible in a green run without
//! `--nocapture` (measured in `crates/pre-delete-citation-check/tests/mirror_oracle.rs:147-153`).
//! A `fn main()` target's output is never captured, so the operator sees the verdict. Assertions
//! here panic, which still fails the run — and a panic is distinguishable from the deliberate
//! `UNMEASURED` exit by its message.
//!
//! THE EXIT CODE NOW MEANS SOMETHING. It was `20` on Linux, i.e. this target failed on every Linux
//! run while the property was in fact being measured there. Nonzero now means exactly *"this
//! platform produced no evidence in either direction"*, which is a signal worth acting on. A
//! measurement is not a failure.

/// WHICH CHANNEL EMITTED THIS LINE — item `3b`, extended to the third channel.
///
/// `src/lib.rs` labels its markers `group-leg` and `routing-oracle`. This target is a THIRD emitter
/// of the same headlines, and a kind identified only by the ABSENCE of a field is a state
/// distinguished by a default branch — the shape `AGENTS.md` names, where adding a case silently
/// reacquires the defect. So it carries its own token, and the leg grep stays exact even when both
/// targets run under one command.
///
/// NOT A CHANGE TO WHAT %7 VERIFIED: exit 0 on Linux, `verdict_code=21`, the caveat, and both
/// witness names are untouched. This is an additive field.
const EMITTER: &str = "platform-scope-reporter";

/// The property under scope.
const PROPERTY: &str = "process_group_kill_and_reap";

/// The four legs in `src/lib.rs` that route through `group_kill_platform_verdict`.
const GROUP_LEGS: usize = 4;

/// Genuinely no evidence: not the shipped platform, and not a platform we measure on.
const UNMEASURED_VERDICT_CODE: i32 = 20;
/// Measured green here, and here is not where this contract ships.
const MEASURED_OFF_PLATFORM_VERDICT_CODE: i32 = 21;
/// Verified on the shipped runtime.
const VERIFIED_VERDICT_CODE: i32 = 0;

/// The platform this contract actually ships on. The surviving justification for scoping at all is
/// this asymmetry — not any tool defect — so it is named as data rather than left in prose.
const SHIPPED_PLATFORM: &str = "darwin";

/// The fifth, deliberately unscoped test, and what it does and does not establish.
///
/// ITEM 6, decided: it is the canonical Linux measurement of the **reap half only**. It runs
/// `/bin/sh -c "sleep 30"` — a single simple command that `sh` execs directly, so there is no
/// grandchild and a pid-directed kill would satisfy its descendant assertion. The group half is
/// established only by `bounded_status_signals_the_group_so_grandchildren_die_too`, which plants a
/// real background grandchild and asserts its marker was never touched.
const REAP_HALF_LINUX_WITNESS: &str = "process_count_returns_to_baseline_after_contract_runs";
const GROUP_HALF_WITNESS: &str = "bounded_status_signals_the_group_so_grandchildren_die_too";

fn main() {
    // ITEM 2 — PROVEN, NOT ASSERTED. Three verdicts must carry three codes; any collision lets a
    // reader consuming the code alone mistake a measurement for an absence, which is the exact
    // defect this bead corrects.
    let codes = [
        VERIFIED_VERDICT_CODE,
        MEASURED_OFF_PLATFORM_VERDICT_CODE,
        UNMEASURED_VERDICT_CODE,
    ];
    let mut distinct = codes;
    distinct.sort_unstable();
    let before = distinct.len();
    distinct.sort_unstable();
    let mut deduped = distinct.to_vec();
    deduped.dedup();
    assert_eq!(
        deduped.len(),
        before,
        "verdict codes must be pairwise distinct, got {codes:?} — a collision is how \
         'measured off the shipped platform' becomes indistinguishable from 'unmeasured'"
    );

    // ITEM 4, KNOWN-GOOD HALF: the UNMEASURED state must still EXIST. This bead adds a state; it
    // must not delete one. The surviving justification for `UNMEASURED` outlives `o3eb` intact —
    // a platform that is neither the shipped runtime nor a lane we measure on yields no evidence
    // in either direction, and that is not the same fact as a green Linux run.
    assert_eq!(
        UNMEASURED_VERDICT_CODE, 20,
        "the UNMEASURED code must be preserved, not repurposed"
    );
    assert_ne!(
        UNMEASURED_VERDICT_CODE, MEASURED_OFF_PLATFORM_VERDICT_CODE,
        "adding a state must not collapse it into the one it was carved out of"
    );

    // The two witnesses are distinct tests. If someone renames one onto the other, the item-6
    // decision silently becomes the overclaim it was written to prevent.
    assert_ne!(
        REAP_HALF_LINUX_WITNESS, GROUP_HALF_WITNESS,
        "the reap witness and the group witness must remain different tests"
    );

    if cfg!(target_os = "macos") {
        println!(
            "VERIFIED_ON_THIS_PLATFORM property={PROPERTY} emitter={EMITTER} \
             platform={SHIPPED_PLATFORM} legs={GROUP_LEGS} verdict_code={VERIFIED_VERDICT_CODE} \
             group_witness={GROUP_HALF_WITNESS}"
        );
        return;
    }

    if cfg!(target_os = "linux") {
        println!(
            "MEASURED_OFF_SHIPPED_PLATFORM property={PROPERTY} emitter={EMITTER} platform={} \
             shipped_platform={SHIPPED_PLATFORM} legs={GROUP_LEGS} \
             verdict_code={MEASURED_OFF_PLATFORM_VERDICT_CODE} \
             reap_witness={REAP_HALF_LINUX_WITNESS} group_witness={GROUP_HALF_WITNESS} \
             caveat=a-linux-pass-is-not-evidence-about-{SHIPPED_PLATFORM} \
             retry_if=darwin-native-run (human-consumed; nothing reads this token)",
            std::env::consts::OS
        );
        // EXIT 0. The legs run here and pass here; a measurement is not a failure. This target
        // previously exited 20 on Linux, which is the false claim in its most operator-visible
        // form — a red target on every lane run for a property the lane was measuring green.
        return;
    }

    println!(
        "UNMEASURED_ON_THIS_PLATFORM property={PROPERTY} emitter={EMITTER} platform={} \
         legs={GROUP_LEGS} verdict_code={UNMEASURED_VERDICT_CODE} \
         retry_if=darwin-native-run (human-consumed; nothing reads this token)",
        std::env::consts::OS
    );
    std::process::exit(UNMEASURED_VERDICT_CODE);
}

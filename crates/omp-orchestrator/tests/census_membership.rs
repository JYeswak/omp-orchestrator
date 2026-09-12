#![forbid(unsafe_code)]
//! THE CENSUS MEMBERSHIP RATCHET — `omp-orchestrator-leht`.
//!
//! # What was wrong
//!
//! MEASURED 2026-09-02: `census_gates` returned **22 rows against 65 crates on
//! disk**. 43 crates — 66% of the tree — had no row at all, and `all_reachable()`
//! was therefore true over a set that never included them. A crate could be
//! uninstalled, unwired, and **invisible** at the same time, and the third was
//! silent.
//!
//! `lib.rs` already contained the sentence that condemns this, written about two
//! hardcoded verdicts while the membership list twenty lines below stayed
//! hand-written:
//!
//! > *"A frozen snapshot presented as a census is the defect class this whole
//! > repository keeps finding — a hand-maintained list masquerading as a probe."*
//!
//! An observation in a comment that the code ignores.
//!
//! # The ruling this file enforces
//!
//! Advisory-first, because refusing dispatch across 43 newly-visible crates would
//! be refusing **on absence of evidence rather than evidence of absence**. Blocking
//! rows keep gating; derived rows are reported and do not. The danger of advisory
//! is that it becomes a nicer name for silence, so:
//!
//! 1. an advisory non-reachable row that no allowance names FAILS,
//! 2. the allowance length is CEILINGED, so growth is a visible diff,
//! 3. **an allowance row for a crate that is now reachable, or gone, FAILS** — so
//!    wiring a crate forces the deletion of its row and the count cannot stay high
//!    after the work is done.
//!
//! Leg 3 is the ratchet. Without it, legs 1 and 2 permit permanent silence.

use omp_orchestrator::{
    census_gates, crates_on_disk, CensusDisposition, GateCensus, GateReachability,
    ADVISORY_ALLOWANCE, ADVISORY_CEILING, ADVISORY_CEILING_RECORDED_AT_UNIX, ADVISORY_RATCHET,
    untriaged_amnesty_rows,
    advisory_ratchet_overdue, quoted_in_call_position, rust_line_invokes, UNDETERMINED_CEILING,
    ADVISORY_RATCHET_DEADLINE_TICKS, CURATED_BLOCKING_ROSTER, PRE_LEHT_BLOCKING_ROWS,
};
use std::path::PathBuf;
use std::fs;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Crates with zero tracked files are peer WIP, not tree members (ky6yx
/// 2026-09-11: kernel-only-gate + omp-host-tool-guard). Rows for them would
/// break CI's stale-row leg, so the naming demand excuses them -- derived per
/// run via `git ls-files`, so the excuse expires the moment the crate lands
/// and this leg demands its row.
fn is_tracked(root: &PathBuf, crate_name: &str) -> bool {
    let output = std::process::Command::new("git")
        .current_dir(root)
        .args(["ls-files", &format!("crates/{crate_name}")])
        .output();
    matches!(output, Ok(out) if out.status.success() && !out.stdout.is_empty())
}

/// ACCEPTANCE 1 + ANTI-VACUITY. Membership is derived, and the two counts must be
/// equal. A census smaller than the tree is the defect; a census of ZERO reports
/// identically to a fully-wired fleet, so both are errors.
#[test]
fn every_crate_on_disk_has_exactly_one_census_row() {
    let root = repo_root();
    let disk = crates_on_disk(&root);
    let census = census_gates(&root);
    println!(
        "census membership: crates_on_disk={} census_rows={}",
        disk.len(),
        census.rows.len()
    );

    assert!(
        disk.len() > 20,
        "ANTI-VACUITY: {} crates on disk is not a workspace -- the directory walk is broken, \
         and an empty walk would make every assertion below pass vacuously",
        disk.len()
    );
    assert_eq!(
        census.rows.len(),
        disk.len(),
        "MEMBERSHIP GAP: {} census rows for {} crates on disk. Missing: {:?}",
        census.rows.len(),
        disk.len(),
        disk.iter()
            .filter(|d| !census.rows.iter().any(|r| &r.gate == *d))
            .collect::<Vec<_>>()
    );
    // Every row must name a real directory: a census row for a crate that does not
    // exist is the `NotExtracted` case, which the curated sites handle explicitly.
    for row in &census.rows {
        assert!(
            disk.contains(&row.gate) || matches!(row.reachability, GateReachability::NotExtracted { .. }),
            "census row `{}` names no directory and is not NotExtracted",
            row.gate
        );
    }
    // No duplicates: a name rowed twice can carry two verdicts and the reader sees
    // whichever it finds first.
    let mut names: Vec<&String> = census.rows.iter().map(|r| &r.gate).collect();
    names.sort();
    let before = names.len();
    names.dedup();
    assert_eq!(before, names.len(), "duplicate census rows");
}
#[test]
fn a_new_cargo_manifest_gets_one_row_and_removal_withdraws_it() {
    let temp = tempfile::tempdir().expect("create census fixture");
    let crates = temp.path().join("crates");
    fs::create_dir_all(&crates).expect("create crates directory");
    let baseline = census_gates(temp.path());
    let candidate = crates.join("planted-membership");
    fs::create_dir_all(&candidate).expect("create planted crate directory");
    fs::write(
        candidate.join("Cargo.toml"),
        "[package]\nname = \"planted-membership\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write planted manifest");

    let planted = census_gates(temp.path());
    assert_eq!(
        planted.rows.len(),
        baseline.rows.len() + 1,
        "a new Cargo directory must add exactly one census row"
    );
    let row = planted
        .rows
        .iter()
        .find(|row| row.gate == "planted-membership")
        .expect("planted crate must be visible in the census");
    match &row.reachability {
        GateReachability::Unreachable { reason } => {
            assert!(
                reason.contains("manifest dependency"),
                "unexpected verdict: {reason}"
            );
        }
        other => panic!("caller-less planted crate must name its absent trigger: {other:?}"),
    }

    fs::remove_dir_all(&candidate).expect("remove planted crate directory");
    let restored = census_gates(temp.path());
    assert_eq!(
        restored.rows.len(),
        baseline.rows.len(),
        "removing the planted crate must restore the original census count"
    );
    assert!(
        restored
            .rows
            .iter()
            .all(|row| row.gate != "planted-membership"),
        "removed crate must not leave a census row behind"
    );
}

/// ACCEPTANCE 4, KNOWN-GOOD. **A membership fix must not change a single verdict.**
/// The blocking row count must still equal the pre-`leht` census, and every crate
/// that gated before must still gate.
#[test]
fn no_pre_existing_verdict_was_flipped_by_derivation() {
    let census = census_gates(&repo_root());
    let blocking: Vec<_> = census
        .rows
        .iter()
        .filter(|r| r.disposition.is_blocking())
        .collect();
    assert_eq!(
        blocking.len(),
        PRE_LEHT_BLOCKING_ROWS,
        "the blocking set changed size: {} now, {} before leht. Derivation may add ADVISORY \
         rows only. Blocking now: {:?}",
        blocking.len(),
        PRE_LEHT_BLOCKING_ROWS,
        blocking.iter().map(|r| &r.gate).collect::<Vec<_>>()
    );
    // Every roster name must be present AND blocking. `ack-spine` is the one that
    // refused the loop in `kwo9`; demoting it to advisory would silently unblock
    // the fleet, which is the opposite of a census fix.
    for name in CURATED_BLOCKING_ROSTER {
        let row = census
            .rows
            .iter()
            .find(|r| r.gate == *name)
            .unwrap_or_else(|| panic!("roster crate `{name}` has no census row"));
        assert!(
            row.disposition.is_blocking(),
            "roster crate `{name}` was demoted to advisory"
        );
    }
}

/// THE RULING, ASSERTED ON THE LIVE CENSUS. `decide` refuses through
/// `unwired_gates()` (`lib.rs:1054-1055`), so this is the number that actually
/// stops the fleet. **It must not have grown**, or derivation converted one blocker
/// into forty-three — the outage the ruling exists to prevent.
#[test]
fn derivation_did_not_convert_one_blocker_into_forty_three() {
    let census = census_gates(&repo_root());
    let refusing = census.unwired_gates();
    let advisory = census.advisory_gates();
    assert!(
        refusing.len() <= 1,
        "the refusal set grew to {}: {:?}. Only rows that refused BEFORE leht may refuse now.",
        refusing.len(),
        refusing.iter().map(|r| &r.gate).collect::<Vec<_>>()
    );
    // ANTI-VACUITY, and it is the important half: if the advisory set were also
    // empty, this test would pass because the census found NOTHING, which is
    // indistinguishable from a fully-wired fleet. The whole finding is that 24
    // crates are unreachable and were invisible.
    assert!(
        !advisory.is_empty(),
        "advisory set is EMPTY -- either the tree is fully wired or the derivation is \
         broken, and this test cannot tell those apart"
    );
    assert_eq!(
        refusing.len() + advisory.len(),
        census
            .rows
            .iter()
            .filter(|r| !r.reachability.is_reachable())
            .count(),
        "every non-reachable row must be either refusing or advisory -- a row that is \
         neither is invisible, which is the defect this bead reported"
    );
}

/// THE POSITIVE CONTROL, restated at the new membership. A census that reports
/// everything unreachable is indistinguishable from a broken one.
#[test]
fn the_positive_control_still_passes_and_reachable_rows_exist() {
    let census = census_gates(&repo_root());
    assert!(
        census.positive_control_passes(),
        "no-shell-gate is not reachable: every verdict in this census is suspect"
    );
    let reachable = census
        .rows
        .iter()
        .filter(|r| r.reachability.is_reachable())
        .count();
    assert!(
        reachable > census.rows.len() / 3,
        "only {reachable}/{} rows reachable -- a census that condemns the whole tree is \
         reporting about itself",
        census.rows.len()
    );
}

/// RATCHET LEG 1. Growth cannot be silent: an advisory non-reachable row with no
/// allowance row fails, and the failure names the crate and the file to edit.
#[test]
fn every_advisory_unreachable_row_is_named_in_the_allowance() {
    let root = repo_root();
    let census = census_gates(&root);
    let unnamed: Vec<&String> = census
        .advisory_gates()
        .iter()
        .filter(|r| {
            // ⛔ AN UNDETERMINED CRATE OWES NOTHING (tzz74). `has_remote` is an input this host
            // may not supply; when it is absent the census says UNDETERMINED rather than
            // inventing `Unreachable`, and you cannot demand an acknowledgement for a fact
            // nobody measured. The stale-allowance leg is symmetric by construction — it keys
            // on `is_reachable()`, so it does not FORBID a row either. Neither obligation
            // attaches to an unmeasured verdict.
            r.reachability.is_determined()
                && !ADVISORY_ALLOWANCE
                    .iter()
                    .any(|(name, _)| *name == r.gate.as_str())
                && is_tracked(&root, &r.gate)
        })
        .map(|r| &r.gate)
        .collect();
    assert!(
        unnamed.is_empty(),
        "these TRACKED crates are unreachable and advisory but NOT named in ADVISORY_ALLOWANCE \
         (crates/omp-orchestrator/src/lib.rs): {unnamed:?}. Either wire them, or add a row \
         with the reason they are not yet blocking. Silence is not an option the ratchet \
         admits."
    );
}
// ⛔ DELETED 2026-09-11 — `s1_coverage_allowance_carries_its_suspension_and_dies_when`.
//
// It asserted that the `s1-coverage` ADVISORY_ALLOWANCE row names its suspension and its
// dies-when. **That row is gone**, because the crate became REACHABLE via
// `.github/workflows/gate.yml` and the census flagged the row STALE by name. Its reason text
// claimed *"no production caller is honest while S1 is frozen"* — a statement that is now
// FALSE, so keeping the row to keep the test green would have preserved a lie to protect an
// assertion about it.
//
// ⭐ THIS IS A SUBJECT-VANISHED DELETION, NOT A FAILING-TEST DELETION, and the difference is
// the whole point: the test still PASSED when it was removed. Deleting a RED test to get
// green is laundering; deleting a GREEN test whose subject was correctly removed is the
// cleanup that makes the removal complete. `the_allowance_never_grows_past_its_ceiling` and
// `an_allowance_row_for_a_wired_or_absent_crate_is_stale_and_fails` still guard every
// remaining row, so no property lost a guard.

/// RATCHET LEG 2. The ceiling bounds UNTRIAGED AMNESTY -- rows whose reason states no death
/// condition -- so adding one is a visible diff and cannot be done by accident.
///
/// It used to bound `ADVISORY_ALLOWANCE.len()`. That could not tell growth from regression
/// (tvqqg): a COMPLIANT new crate needs a row and breached the bound exactly as a defect
/// would. The row count is now governed per-row by the naming leg and the stale-row leg; what
/// a ratchet can honestly forbid is amnesty granted without a stated death condition.
#[test]
fn the_allowance_never_grows_past_its_ceiling() {
    let untriaged = untriaged_amnesty_rows();
    assert!(
        untriaged.len() <= ADVISORY_CEILING,
        "UNTRIAGED AMNESTY grew to {} rows against a ceiling of {ADVISORY_CEILING}: {untriaged:?}. \
         Give the new row a real `Dies when ...`, or wire the crate. The ceiling only falls.",
        untriaged.len()
    );
    // Every row must carry a REASON, not an empty string. franken_lean's principle:
    // writing the reason forces the author to say why, which is the sentence a
    // reflexive exception cannot honestly produce.
    for (name, reason) in ADVISORY_ALLOWANCE {
        assert!(
            reason.len() > 20,
            "allowance row `{name}` has no real reason: {reason:?}"
        );
    }
    // No duplicate names: two rows for one crate means deleting one leaves the
    // exception in place, and leg 3 would report the crate as still allowanced.
    let mut names: Vec<&str> = ADVISORY_ALLOWANCE.iter().map(|(n, _)| *n).collect();
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(before, names.len(), "duplicate allowance rows");
}

/// RATCHET LEG 3 — THE TEETH. **A crate that has been wired must be REMOVED from
/// the allowance.** Its row goes stale the moment the work lands, and a stale row
/// is a build failure, so the count cannot stay at 23 after 23 crates are fixed.
///
/// This is what makes advisory-first distinguishable from permanent silence, and it
/// is mechanical rather than a reminder: nobody has to remember to lower anything.
#[test]
fn an_allowance_row_for_a_wired_or_absent_crate_is_stale_and_fails() {
    let root = repo_root();
    let census = census_gates(&root);
    let disk = crates_on_disk(&root);
    let mut stale = Vec::new();
    for (name, _) in ADVISORY_ALLOWANCE {
        if !disk.iter().any(|d| d == name) {
            stale.push(format!("{name} (no longer on disk)"));
            continue;
        }
        match census.rows.iter().find(|r| r.gate == *name) {
            Some(row) if row.reachability.is_reachable() => {
                stale.push(format!("{name} (now REACHABLE -- delete the row)"));
            }
            Some(_) => {}
            None => stale.push(format!("{name} (no census row)")),
        }
    }
    assert!(
        stale.is_empty(),
        "STALE ALLOWANCE ROWS -- delete them and LOWER ADVISORY_CEILING to match: {stale:?}"
    );
}

/// SnowyCanyon's own falsifier, as a number rather than a preference: *"if the
/// advisory count has not decreased after a stated number of ticks, advisory-first
/// has failed and triage-first was the right call."*
///
/// This test does not fail on the deadline — a test that goes red on a clock would
/// block every unrelated commit, which is the outage the ruling avoided. It asserts
/// the deadline is REPRESENTABLE and non-vacuous; the supervisor prints the overdue
/// verdict in its own decision output every cycle, which is the only signalling path
/// that has ever reached a human here.
#[test]
// These three ARE compile-time, and clippy is right to say so. They are const-drift
// guards: their whole job is to fail the build when someone edits the constant, and
// a runtime assertion cannot do that job. The LIVE half is asserted below them.
#[allow(clippy::assertions_on_constants)]
fn the_ratchet_deadline_is_a_real_number_and_not_a_sentiment() {
    assert!(
        ADVISORY_RATCHET_DEADLINE_TICKS > 0 && ADVISORY_RATCHET_DEADLINE_TICKS < 10_000,
        "the deadline must be a bounded tick count, got {ADVISORY_RATCHET_DEADLINE_TICKS}"
    );
    assert!(
        ADVISORY_CEILING_RECORDED_AT_UNIX > 1_700_000_000,
        "the ceiling must carry the time it was recorded, or 'has it decreased since' is \
         unanswerable"
    );
    assert!(
        ADVISORY_RATCHET.is_consistent(),
        "ceiling changed without its recorded ceiling anchor"
    );
    assert_eq!(
        ADVISORY_CEILING,
        ADVISORY_RATCHET.ceiling(),
        "compatibility ceiling projection drifted from the ratchet"
    );
    assert_eq!(
        ADVISORY_CEILING_RECORDED_AT_UNIX,
        ADVISORY_RATCHET.recorded_at_unix(),
        "compatibility timestamp projection drifted from the ratchet"
    );
    // 200 ticks at the supervisor's 90s interval. Stated so a reader can check the
    // arithmetic rather than trust the comment.
    assert_eq!(ADVISORY_RATCHET_DEADLINE_TICKS * 90 / 3600, 5, "5 hours");

    // UNACKNOWLEDGED GROWTH, PER CRATE (tvqqg). This used to read `live <= ADVISORY_CEILING`,
    // which conflated a COMPLIANT new advisory crate with a regression -- fleet-idle-monitor,
    // correct and tested and carrying a full row in UNWIRED_LANE_ALLOWANCE, breached `live 30
    // > ceiling 29` exactly as a defect would, and every future compliant crate would too.
    // The honest invariant is not how MANY advisory crates exist but whether each one is
    // ACKNOWLEDGED, and the refusal now names the crate instead of printing arithmetic.
    let census = census_gates(&repo_root());
    let named: std::collections::BTreeSet<&str> =
        ADVISORY_ALLOWANCE.iter().map(|(name, _)| *name).collect();
    let unacknowledged: Vec<&str> = census
        .advisory_gates()
        .iter()
        // SAME RULE AS THE NAMING LEG (tzz74): an UNDETERMINED verdict is an unsupplied input,
        // not a finding, so it cannot owe an acknowledgement. Without this the worker demands
        // a row that CI forbids, which is the contradiction the variant exists to end.
        .filter(|row| row.reachability.is_determined())
        .map(|row| row.gate.as_str())
        .filter(|gate| !named.contains(gate))
        .collect();
    assert!(
        unacknowledged.is_empty(),
        "unacknowledged advisory crates: {unacknowledged:?} -- wire each one or give it an \
         ADVISORY_ALLOWANCE row with a real `Dies when ...`. Never raise a ceiling to hide it."
    );
    // BANKED SLACK, on the ratchet's own subject. A ceiling above the live untriaged count is
    // headroom nobody measured, so it must fall to meet it.
    let untriaged = untriaged_amnesty_rows().len();
    assert!(
        ADVISORY_CEILING <= untriaged,
        "banked slack: ceiling {ADVISORY_CEILING} exceeds live untriaged amnesty {untriaged} -- \
         lower the ceiling, do not bank headroom"
    );
}

/// THE FALSIFIER, BOTH DIRECTIONS. A deadline already past at the moment it is
/// recorded is not a deadline — it prints on tick 1 and trains the operator to
/// ignore the line.
///
/// MEASURED 2026-09-02: the first recorded value was one year off, and the live run
/// printed `CENSUS_ADVISORY_RATCHET_OVERDUE` immediately — 350,394 elapsed ticks
/// against a 200-tick deadline. Only running it exposed that; the constant looked
/// plausible in the diff.
#[test]
fn overdue_is_false_at_the_moment_of_recording_and_true_past_the_deadline() {
    let at = ADVISORY_CEILING_RECORDED_AT_UNIX;
    let deadline = ADVISORY_RATCHET_DEADLINE_TICKS;

    // Direction 1: at record time, and one tick short of the deadline.
    assert!(
        !advisory_ratchet_overdue(at, at, 90, deadline, ADVISORY_CEILING, ADVISORY_CEILING),
        "overdue at the instant of recording: the deadline is already in the past"
    );
    assert!(!advisory_ratchet_overdue(
        at + (deadline * 90),
        at,
        90,
        deadline,
        ADVISORY_CEILING,
        ADVISORY_CEILING
    ));

    // Direction 2: past it, with no decrease. The verdict MUST fire, or the
    // falsifier is unfalsifiable and the ruling is a preference again.
    assert!(
        advisory_ratchet_overdue(
            at + (deadline * 90) + 90,
            at,
            90,
            deadline,
            ADVISORY_CEILING,
            ADVISORY_CEILING
        ),
        "past the deadline with no decrease and still not overdue"
    );

    // Direction 3: past the deadline but the count DECREASED. Not overdue — the
    // falsifier is about a stalled ratchet, not elapsed time. Without this the
    // verdict would nag forever after 5 hours no matter how much work landed.
    // ⛔ Do NOT use `ADVISORY_CEILING - 1`: the live ceiling is now 0 (all amnesty
    // rows triaged) and usize underflow is not a decrease. The pair (live=0,
    // recorded=1) is the decrease the function must recognise.
    assert!(!advisory_ratchet_overdue(
        at + 10_000_000,
        at,
        90,
        deadline,
        0,
        1
    ));

    // A zero interval must not divide by zero: the supervisor's interval is
    // configurable and a bad config must not panic the loop.
    let _ = advisory_ratchet_overdue(at + 1, at, 0, deadline, 1, 1);

    // And the recorded moment must be real, not a placeholder.
    assert!(
        at > 1_780_000_000,
        "the recorded time is {at}, which predates this repository -- the ceiling was \
         never actually recorded"
    );
}

/// The `Unprobed` variant must be reachable in principle — a variant no input can
/// produce is decoration. Constructed directly, since the census only emits it for
/// a crate directory with no readable manifest and planting one would race peers
/// in this shared checkout.
#[test]
fn unprobed_is_distinct_from_unreachable_in_both_label_and_remedy() {
    let unprobed = GateReachability::Unprobed {
        reason: "no readable manifest".into(),
    };
    let unreachable = GateReachability::Unreachable {
        reason: "no readable manifest".into(),
    };
    assert_ne!(unprobed.label(), unreachable.label());
    assert_ne!(unprobed.next_action(), unreachable.next_action());
    assert!(!unprobed.is_reachable());
    // The remedy is the point: `repair-gate-trigger` would send an operator to wire
    // a crate whose manifest is the actual problem.
    assert_eq!(unprobed.next_action(), "repair-the-crate-manifest");
}

/// A blocking non-reachable row must still refuse, and an advisory one must not.
/// Constructed rather than measured, so the predicate is proven on both directions
/// even when today's census happens to contain only one of them.
#[test]
fn disposition_decides_whether_a_verdict_stops_the_fleet() {
    let bad = || GateReachability::Unreachable {
        reason: "x".into(),
    };
    let blocking = GateCensus {
        rows: vec![
            omp_orchestrator::GateCensusRow {
                gate: "no-shell-gate".into(),
                reachability: GateReachability::Reachable {
                    trigger: "hook".into(),
                },
                disposition: CensusDisposition::Blocking,
            },
            omp_orchestrator::GateCensusRow {
                gate: "b".into(),
                reachability: bad(),
                disposition: CensusDisposition::Blocking,
            },
        ],
    };
    assert!(!blocking.all_reachable(), "a blocking bad row must refuse");
    assert_eq!(blocking.unwired_gates().len(), 1);
    assert_eq!(blocking.advisory_gates().len(), 0);

    let advisory = GateCensus {
        rows: vec![omp_orchestrator::GateCensusRow {
            gate: "b".into(),
            reachability: bad(),
            disposition: CensusDisposition::Advisory {
                reason: "untriaged".into(),
            },
        }],
    };
    assert!(
        advisory.all_reachable(),
        "an advisory bad row must NOT refuse -- that is the ruling"
    );
    assert_eq!(advisory.unwired_gates().len(), 0);
    assert_eq!(
        advisory.advisory_gates().len(),
        1,
        "and it must still be COUNTED -- advisory means reported, not ignored"
    );
}

// ── tzz74: THE CENSUS CONSUMES THE SPAWN PROBE, AND A MENTION IS STILL NOT A CALLER ────────

/// POSITIVE CONTROL: the census now sees an env-resolved `Command::new` spawn.
///
/// `cargo-lane-budget` is spawned ONLY from Rust — `fast-dispatch/src/main.rs:994`,
/// `Command::new(configured_rust_binary("FD_BUDGET", "cargo-lane-budget"))`. No workflow names
/// it, so the workflow/manifest census called it Unreachable while `wired_lanes`' probe could
/// already see it. Two judgements about one question, and the CONSUMING oracle held the
/// weaker one.
#[test]
fn the_census_classifies_a_rust_only_spawn_target_reachable() {
    let census = census_gates(&repo_root());
    let row = census
        .rows
        .iter()
        .find(|r| r.gate == "cargo-lane-budget")
        .expect("RULE spawn_probe_non_vacuous: cargo-lane-budget must have a census row at all");
    assert!(
        row.reachability.is_reachable(),
        "RULE spawn_probe_consumed: cargo-lane-budget is spawned at \
         fast-dispatch/src/main.rs:994 and must classify Reachable; got {:?}",
        row.reachability
    );
}

/// ⛔ KNOWN-BAD DIRECTION: A MENTION IS NOT A CALLER — the leg that stops this fix from being
/// a `contains`.
///
/// `loop-coverage/src/lib.rs:212` carries the literal `cargo-lane-budget` inside a
/// `TypedEdgeCase` DESCRIPTION. A substring scan reports that as an invocation, and acting on
/// it would DELETE A CORRECT ALLOWANCE ROW — strictly worse than the red it was chasing.
/// The rule is POSITION, so it is asserted directly on the predicate over the real lines.
#[test]
fn a_described_crate_is_not_a_caller_but_a_quoted_spawn_is() {
    let described =
        r#"        "controller-tick refused on code != Some(0); cargo-lane-budget 77 means UNKNOWN","#;
    assert!(
        !rust_line_invokes(described, "cargo-lane-budget", "cargo_lane_budget"),
        "RULE mention_is_not_invocation: a description field naming the crate must NOT count \
         as a caller, or the census deletes correct allowance rows"
    );
    let allowance_row =
        r#"    ("cargo-lane-budget", "instrument limitation: census_gates does not see it"),"#;
    assert!(
        !rust_line_invokes(allowance_row, "cargo-lane-budget", "cargo_lane_budget"),
        "RULE tuple_is_not_invocation: an allowance ROW naming the crate must not make it \
         Reachable -- that would be the registry certifying itself out of existence"
    );
    let spawn =
        r#"    let mut budget_cmd = Command::new(configured_rust_binary("FD_BUDGET", "cargo-lane-budget"));"#;
    assert!(
        rust_line_invokes(spawn, "cargo-lane-budget", "cargo_lane_budget"),
        "RULE spawn_is_invocation: the real spawn site must count, or the fix is a no-op"
    );
}

/// ANTI-VACUITY ON THE PREDICATE ITSELF.
///
/// An empty needle matches every line, which would make EVERY crate Reachable and silently
/// empty the advisory registry — the inverse of a never-matching needle, and the more
/// dangerous direction because it presents as a clean-up.
#[test]
fn an_empty_needle_never_reports_an_invocation() {
    assert!(
        !rust_line_invokes("anything at all", "", ""),
        "RULE empty_needle_non_vacuous: an empty needle must never report an invocation"
    );
    assert!(
        !quoted_in_call_position("no parens here", 3),
        "RULE position_requires_a_paren: a line with no call cannot be a call position"
    );
}

/// ⛔ ANTI-VACUITY ON `Undetermined` ITSELF, AND IT IS NOT OPTIONAL (tzz74 ruling item 4).
///
/// `Undetermined` releases a crate from BOTH obligations — the naming leg may not demand a row
/// and the stale-allowance leg may not forbid one. That is correct for a crate whose input was
/// missing, and CATASTROPHIC if it becomes the residual for everything: a remote-less host
/// would silently satisfy both legs and we would have rebuilt the very defect the variant was
/// introduced to fix — an absent input producing no verdict, and no verdict being
/// indistinguishable from a pass.
///
/// So the escape hatch is bounded from inside: the Undetermined set must be a STRICT SUBSET,
/// and at least one crate must be POSITIVELY classified. A census that determined nothing is
/// an ERROR, never a green.
#[test]
fn undetermined_is_a_strict_subset_and_never_the_whole_census() {
    let census = census_gates(&repo_root());
    assert!(
        !census.rows.is_empty(),
        "RULE undetermined_non_vacuous: an empty census cannot certify anything about \
         Undetermined; the scan itself is broken"
    );
    let undetermined = census
        .rows
        .iter()
        .filter(|r| !r.reachability.is_determined())
        .count();
    // ⛔ THE CLAUSE THAT ACTUALLY BITES. "Strict subset" alone is NECESSARY AND INSUFFICIENT:
    // a mutation making Undetermined the residual for EVERY BIN CRATE left this leg green,
    // because library crates kept the subset strict. The population released from both
    // obligation legs is the quantity that must not grow quietly, so it is ratcheted.
    assert!(
        undetermined <= UNDETERMINED_CEILING,
        "RULE undetermined_ceiling: {undetermined} rows are Undetermined, above the ceiling of \
         {UNDETERMINED_CEILING} -- an absent input is becoming the residual for the census, \
         which releases that many crates from BOTH obligation legs at once. Lower the ceiling \
         when the input becomes available; never raise it to absorb a spreading unknown"
    );
    assert!(
        undetermined < census.rows.len(),
        "RULE undetermined_bounded: ALL {} census rows are Undetermined -- an absent input has \
         become the residual for the whole census, which makes a remote-less host satisfy every \
         obligation leg by measuring nothing. That is an ERROR, not a pass",
        census.rows.len()
    );
    assert!(
        census.rows.iter().any(|r| r.reachability.is_determined()),
        "RULE undetermined_has_a_control: at least one crate must be POSITIVELY classified, or \
         the census has no positive control and its greens mean nothing"
    );
}

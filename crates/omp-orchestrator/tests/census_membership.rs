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
    ADVISORY_ALLOWANCE, ADVISORY_CEILING, ADVISORY_CEILING_RECORDED_AT_UNIX,
    advisory_ratchet_overdue, ADVISORY_RATCHET_DEADLINE_TICKS, CURATED_BLOCKING_ROSTER,
    PRE_LEHT_BLOCKING_ROWS,
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
    let census = census_gates(&repo_root());
    let unnamed: Vec<&String> = census
        .advisory_gates()
        .iter()
        .filter(|r| {
            !ADVISORY_ALLOWANCE
                .iter()
                .any(|(name, _)| *name == r.gate.as_str())
        })
        .map(|r| &r.gate)
        .collect();
    assert!(
        unnamed.is_empty(),
        "these crates are unreachable and advisory but NOT named in ADVISORY_ALLOWANCE \
         (crates/omp-orchestrator/src/lib.rs): {unnamed:?}. Either wire them, or add a row \
         with the reason they are not yet blocking. Silence is not an option the ratchet \
         admits."
    );
}
#[test]
fn s1_coverage_allowance_carries_its_suspension_and_dies_when() {
    let reason = ADVISORY_ALLOWANCE
        .iter()
        .find(|(name, _)| *name == "s1-coverage")
        .map(|(_, reason)| *reason)
        .expect("s1-coverage must have an explicit allowance row");
    assert!(reason.contains("S1 depth is suspended"), "{reason}");
    assert!(reason.contains("Dies when"), "{reason}");
    assert!(reason.contains("approved S1 build wave"), "{reason}");
}

/// RATCHET LEG 2. The ceiling bounds the set, so adding a name is a visible diff
/// and cannot be done by accident.
#[test]
fn the_allowance_never_grows_past_its_ceiling() {
    assert!(
        ADVISORY_ALLOWANCE.len() <= ADVISORY_CEILING,
        "ADVISORY_ALLOWANCE has {} rows against a ceiling of {ADVISORY_CEILING}. The ceiling \
         may only be LOWERED.",
        ADVISORY_ALLOWANCE.len()
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
    // 200 ticks at the supervisor's 90s interval. Stated so a reader can check the
    // arithmetic rather than trust the comment.
    assert_eq!(ADVISORY_RATCHET_DEADLINE_TICKS * 90 / 3600, 5, "5 hours");

    // THE SLACK HOLE, closed. Leg 2 alone permits the ceiling to sit above the
    // allowance forever: delete a row, `len() <= CEILING` still passes, and the
    // ceiling quietly stops meaning anything. Requiring EQUALITY makes deleting a
    // row and lowering the ceiling one edit, which is what "the set is required to
    // shrink" has to mean to be enforceable.
    assert_eq!(
        ADVISORY_ALLOWANCE.len(),
        ADVISORY_CEILING,
        "the ceiling must EQUAL the allowance length, or slack accumulates and the ratchet \
         degrades into a bound nobody is near"
    );

    // And the LIVE half, so this test is not purely constant: the ceiling must
    // describe the census that actually runs. A ceiling that has drifted from the
    // measurement is the frozen-snapshot defect this whole bead is about.
    let census = census_gates(&repo_root());
    assert_eq!(
        census.advisory_gates().len(),
        ADVISORY_CEILING,
        "the live advisory count is {} and the ceiling says {ADVISORY_CEILING}. Either wire \
         the difference or re-record the ceiling -- a ceiling that does not match the \
         measurement is a hand-maintained number masquerading as a bound.",
        census.advisory_gates().len()
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
    assert!(!advisory_ratchet_overdue(
        at + 10_000_000,
        at,
        90,
        deadline,
        ADVISORY_CEILING - 1,
        ADVISORY_CEILING
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

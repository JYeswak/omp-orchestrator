//! `omp-orchestrator-3kcl`: the two PRODUCTION supervisor probes must have a bounded lifetime.
//!
//! NAMED TARGET per the mmt4 ruling: cite as
//! `cargo test -p omp-orchestrator --test bounded_supervisor_probes`, never a bare `-p`
//! aggregate, which truncates at the first failing target.
//!
//! SCOPE IS TWO SITES, NOT THREE, and that is deliberate. The `zaxp` census named three
//! `.output()` calls; `resident_tick.rs`'s `shasum` sits inside `#[cfg(test)] mod tests`
//! (which begins at `resident_tick.rs:307`), so it is a TEST HELPER and is left alone. An
//! attack-only list that finds every site guilty produces an over-strict design, and an
//! over-strict design gets routed around — `kvsq` item 4 exists for exactly this.

use omp_orchestrator::target_directory::{DF_DEADLINE, LSOF_DEADLINE};
use std::time::Duration;

/// THE ARGUMENT, PINNED. Both deadlines were MEASURED, not picked:
///
///   df -k on this repo's path            78 ms
///   lsof -nP +D over 617,897 files       17,359 ms
///   lsof -nP +D over  16,083 files        2,215 ms   (~28 us/file)
///
/// A shared constant would be wrong by ~200x in one direction or the other. This test is what
/// stops the next editor from "simplifying" them into one value: a deadline argued only in a
/// prose comment regresses silently, which is the defect class this repo keeps paying for.
#[test]
fn deadlines_are_ordered_and_argued() {
    assert!(
        LSOF_DEADLINE > DF_DEADLINE,
        "the RECURSIVE probe must never be bounded tighter than the constant-time one: \
         lsof={:?} df={:?}",
        LSOF_DEADLINE,
        DF_DEADLINE
    );

    // df: above a wedge-detecting floor, below operator patience for a statfs.
    assert!(
        DF_DEADLINE >= Duration::from_secs(10),
        "df measured 78ms; a bound under 10s risks firing on a merely loaded host, converting \
         load into a false capacity refusal. got {DF_DEADLINE:?}"
    );
    assert!(
        DF_DEADLINE <= Duration::from_secs(120),
        "df is a constant-time statfs; a bound this wide is indistinguishable from none for an \
         operator waiting on a disk-pressure answer. got {DF_DEADLINE:?}"
    );

    // lsof: must clear the MEASURED 17.4s worst case with real headroom for tree growth.
    assert!(
        LSOF_DEADLINE >= Duration::from_secs(60),
        "lsof -nP +D MEASURED 17,359ms over 617,897 files and scales with file count; a bound \
         below 60s fires on a merely LARGE target tree, which strands disk instead of reaping \
         it. got {LSOF_DEADLINE:?}"
    );
    assert!(
        LSOF_DEADLINE <= Duration::from_secs(600),
        "a bound this wide cannot distinguish a big walk from a wedged mount. got {LSOF_DEADLINE:?}"
    );
}

/// FIRES-ON-KNOWN-BAD, attributable, and it asserts the OUTCOME VARIANT rather than an exit
/// code: a child that never returns must produce `TimedOut`, never `Completed`.
///
/// This exercises the exact primitive both production sites now route through. Bounding the
/// real `df`/`lsof` calls cannot be tested by making them hang — a wedged mount is not
/// constructible in a test — so the leg proves the mechanism the sites depend on.
///
/// MUTATION: widen the deadline past the child's lifetime, or drop the bound, and this goes RED.
#[test]
fn the_primitive_both_sites_use_times_out_rather_than_completing() {
    let mut command = std::process::Command::new("/bin/sleep");
    command.arg("300");
    let started = std::time::Instant::now();
    let outcome = subprocess_contract::bounded_output(&mut command, Duration::from_millis(600));
    let elapsed = started.elapsed();

    match outcome {
        subprocess_contract::BoundedOutcome::TimedOut => {}
        subprocess_contract::BoundedOutcome::Completed(_) => panic!(
            "a 300s child returned Completed under a 600ms deadline after {}ms — the deadline \
             did not fire, so the supervisor probes are effectively unbounded",
            elapsed.as_millis()
        ),
        subprocess_contract::BoundedOutcome::Unspawned(error) => {
            panic!("/bin/sleep must spawn: {error}")
        }
    }
    assert!(
        elapsed < Duration::from_secs(30),
        "the call returned only after {}ms; a bound that does not bound is not a bound",
        elapsed.as_millis()
    );
}

/// KNOWN-GOOD, MANDATORY: the primitive must still return real stdout for a fast child. Both
/// production sites PARSE stdout — `df`'s second line, `lsof`'s line count — so a bound that
/// captured nothing would silently break both while looking green.
#[test]
fn the_primitive_still_captures_stdout_for_a_fast_child() {
    let mut command = std::process::Command::new("/bin/echo");
    command.arg("supervisor-probe-known-good");
    match subprocess_contract::bounded_output(&mut command, DF_DEADLINE) {
        subprocess_contract::BoundedOutcome::Completed(out) => {
            assert_eq!(out.status.code(), Some(0));
            assert_eq!(
                String::from_utf8_lossy(&out.stdout).trim(),
                "supervisor-probe-known-good",
                "captured stdout must be unaltered — both sites parse it"
            );
        }
        other => panic!(
            "expected Completed for /bin/echo; got {:?} — the bound is over-strict and would \
             refuse healthy probes",
            std::mem::discriminant(&other)
        ),
    }
}

/// THE REAL `df` PROBE must complete well inside its bound. This is the production argv, not a
/// synthetic child, so it is the leg that would catch a bound set absurdly low.
#[test]
fn the_real_df_probe_completes_far_inside_its_deadline() {
    let mut command = std::process::Command::new("df");
    command.args(["-k", "."]);
    let started = std::time::Instant::now();
    match subprocess_contract::bounded_output(&mut command, DF_DEADLINE) {
        subprocess_contract::BoundedOutcome::Completed(out) => {
            let text = String::from_utf8_lossy(&out.stdout);
            assert!(
                text.lines().nth(1).is_some(),
                "df must produce a data row; the production caller parses line 2"
            );
        }
        other => panic!(
            "the production df argv did not complete within {:?}: {:?}",
            DF_DEADLINE,
            std::mem::discriminant(&other)
        ),
    }
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "df took {}ms; measured healthy is 78ms, so this host is anomalous and the 30s bound \
         should be re-argued rather than assumed",
        started.elapsed().as_millis()
    );
}

/// ANTI-VACUITY / SCOPE PROOF: exactly the two production sites are bounded and the test helper
/// is not. Keyed on a PREDICATE over the source, not on line numbers, because these drift.
///
/// This is the leg that fails if someone bounds the `cfg(test)` helper too (over-strict) or
/// un-bounds a production site (regression).
#[test]
fn exactly_the_two_production_probes_are_bounded() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut raw_output_sites = Vec::new();
    let mut bounded_sites = Vec::new();

    for name in ["resident.rs", "target_directory.rs", "resident_tick.rs"] {
        let path = root.join(name);
        let text = std::fs::read_to_string(&path).expect("source readable");
        for (index, line) in text.lines().enumerate() {
            if line.trim() == ".output()" {
                raw_output_sites.push(format!("{name}:{}", index + 1));
            }
        }
        if text.contains("subprocess_contract::bounded_output") {
            bounded_sites.push(name);
        }
    }

    assert!(
        !raw_output_sites.is_empty() || !bounded_sites.is_empty(),
        "ANTI-VACUITY: the scan found neither a raw nor a bounded site, so it read nothing"
    );

    // The ONE surviving raw `.output()` must be the cfg(test) helper, deliberately left alone.
    assert_eq!(
        raw_output_sites.len(),
        1,
        "expected exactly ONE remaining raw .output() — the cfg(test) shasum helper in \
         resident_tick.rs. Got {raw_output_sites:?}. More than one means a production site \
         regressed; zero means the test helper was bounded too, which is the over-strict \
         failure kvsq item 4 warns about."
    );
    assert!(
        raw_output_sites[0].starts_with("resident_tick.rs:"),
        "the surviving raw site must be the test helper, not a production probe: {raw_output_sites:?}"
    );

    assert!(
        bounded_sites.contains(&"resident.rs") && bounded_sites.contains(&"target_directory.rs"),
        "both production sites must route through bounded_output; got {bounded_sites:?}"
    );
    assert!(
        !bounded_sites.contains(&"resident_tick.rs"),
        "the cfg(test) helper must NOT be bounded — an over-strict list is the defect, not the fix"
    );
}

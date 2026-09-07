//! `omp-orchestrator-mmt4` item 3: refuse a cited test figure that does not state its scope.
//!
//! NAMED TARGET ON PURPOSE. Per the ruling this target is cited as
//! `cargo test -p no-shell-gate --test cited_figure_denominator`, never through the crate
//! aggregate -- which is exactly the truncation this file exists to police. The crate's
//! aggregate is red for 31 environment reasons on the lane; a named target cannot truncate.

use no_shell_gate::cited_figure::{scan_blob, scan_jsonl, summarise};

/// FIRES-ON-KNOWN-BAD: a bare pair is flagged, and the refusal names the bead, the field, the
/// figure, and what would have satisfied it. Asserts the MESSAGE, never a bare bool --
/// `cargo` returns 101 for unrelated causes and a bare `!is_empty()` has the same weakness.
#[test]
fn a_bare_pass_fail_pair_is_refused_and_names_what_is_missing() {
    let rows = scan_blob("fx-1", "close_reason", "DONE -- 33 passed / 3 failed, all green.");
    assert_eq!(rows.len(), 1, "the scanner must find the figure: {rows:?}");
    assert!(
        !rows[0].denominated,
        "a bare pair states no scope and must not be denominated"
    );
    let text = rows[0].to_string();
    assert!(
        text.contains("CITED_FIGURE_WITHOUT_SCOPE"),
        "typed, not prose: {text:?}"
    );
    assert!(text.contains("bead=fx-1"), "{text:?}");
    assert!(text.contains("field=close_reason"), "{text:?}");
    assert!(text.contains("33 passed / 3 failed"), "{text:?}");
    assert!(
        text.contains("--test <target>") && text.contains("--no-fail-fast"),
        "the refusal must name what WOULD satisfy it, or a reader cannot comply: {text:?}"
    );
}

/// KNOWN-GOOD, MANDATORY, and all four admissible spellings. An attack-only suite ships an
/// over-strict gate, and over-strict here would refuse every honest grade in the repo.
#[test]
fn every_admissible_spelling_is_accepted() {
    let cases = [
        "cargo test -p no-shell-gate --test cited_figure_denominator -> 5 passed / 0 failed",
        "--no-fail-fast: 251 passed / 55 failed across 52 targets",
        "cargo test -p ack-spine --lib -> 22 passed / 0 failed",
        "targets=52 251 passed / 55 failed",
    ];
    for case in cases {
        let rows = scan_blob("fx-ok", "comment[0]", case);
        assert_eq!(rows.len(), 1, "must find one figure in {case:?}: {rows:?}");
        assert!(
            rows[0].denominated,
            "this spelling states its scope and must be accepted: {case:?}"
        );
    }
}

/// The window is BOUNDED on purpose: one `--test` in a 6 KB report must not launder every bare
/// figure in it. This is the difference between a check and a rubber stamp.
#[test]
fn a_distant_target_mention_does_not_launder_a_bare_figure() {
    let far = format!(
        "cargo test -p x --test alpha -> ok.{}Later, unrelated: 9 passed / 1 failed",
        " ".repeat(400)
    );
    let rows = scan_blob("fx-far", "comment[1]", &far);
    let bare = rows.iter().filter(|r| !r.denominated).count();
    assert_eq!(
        bare, 1,
        "a figure 400 chars from any scope statement must stay bare: {rows:?}"
    );
}

/// Three spellings of the same figure all parse. All three occur in this tracker.
#[test]
fn slash_semicolon_and_newline_spellings_all_parse() {
    for text in ["7 passed / 0 failed", "7 passed; 0 failed", "7 passed / 0\nfailed"] {
        assert_eq!(
            scan_blob("fx", "f", text).len(),
            1,
            "spelling {text:?} must parse"
        );
    }
}

/// A number that is not a test figure must NOT be harvested. Over-matching would make the
/// census meaningless and the gate unsatisfiable.
#[test]
fn unrelated_numbers_are_not_harvested() {
    for text in [
        "85 workspace bin targets",
        "31 MB with several live writers",
        "passed the gate with 0 problems",
        "0 failed to spawn",
    ] {
        assert!(
            scan_blob("fx", "f", text).is_empty(),
            "must not harvest a non-figure: {text:?}"
        );
    }
}

/// ANTI-VACUITY: every way of having no input is an ERROR with a named reason.
#[test]
fn an_empty_or_malformed_scan_set_is_an_error() {
    let empty = scan_jsonl("").expect_err("an empty mirror must refuse");
    assert!(
        empty.contains("CITED_FIGURE_SCAN_EMPTY reason=zero_bead_records_readable"),
        "got {empty:?}"
    );
    let malformed = scan_jsonl("{not json\n").expect_err("malformed JSONL must refuse");
    assert!(
        malformed.contains("CITED_FIGURE_SCAN_UNREADABLE reason=malformed_jsonl line=0"),
        "the refusal must name the offending line: {malformed:?}"
    );
}

/// The summary always carries BOTH denominators, so a bare ratio cannot be produced by the
/// instrument that polices bare ratios.
#[test]
fn the_summary_never_emits_a_bare_ratio() {
    let rows = scan_blob("fx", "f", "3 passed / 1 failed");
    let text = summarise(&rows);
    assert!(text.contains("figures=1"), "{text:?}");
    assert!(text.contains("denominated=0"), "{text:?}");
    assert!(text.contains("bare=1"), "{text:?}");
}

/// THE LIVE CENSUS -- REPORTING, NOT GATING, and the reason is stated rather than assumed.
///
/// Measured 2026-09-07: 601 cited figures in this tracker, 351 bare. Gating that would be RED
/// BY CONSTRUCTION over history nobody can retroactively re-run, and a gate red by
/// construction gets routed around -- the failure this repo has measured three times. So this
/// leg asserts only that the instrument still WORKS against real data, and prints the split.
/// Enforcement of NEW citations belongs at the close boundary and is not built.
///
/// It DECLINES on a synced worker: `rch` ships source without `.beads`, so the discriminator
/// is positive -- no `.git` means this tree is not a checkout and cannot answer. Absence alone
/// never satisfies it. libtest captures stdout on a pass, so the declaration needs
/// `-- --nocapture` to be seen.
#[test]
fn the_live_tracker_census_still_runs_and_reports_both_denominators() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    if !repo_root.join(".git").exists() {
        println!(
            "UNMEASURED reason=not_a_repo_checkout root={} -- rch syncs source without .beads, \
             so the live census is unobservable here. This is not a pass for the subject.",
            repo_root.display()
        );
        return;
    }
    let path = repo_root.join(".beads").join("issues.jsonl");
    assert!(
        path.exists(),
        "this IS a checkout ({}) and the mirror is absent at {}",
        repo_root.display(),
        path.display()
    );
    let text = std::fs::read_to_string(&path).expect("mirror readable");
    let rows = scan_jsonl(&text).expect("the real mirror must scan");
    println!("{}", summarise(&rows));
    assert!(
        rows.len() >= 100,
        "the census collapsed to {} figures -- a shrinking scan set is how this goes vacuously \
         green",
        rows.len()
    );
    let bare = rows.iter().filter(|row| !row.denominated).count();
    assert!(
        bare > 0,
        "zero bare citations would mean the ruling is already satisfied everywhere; that is \
         not the measured state and almost certainly means the matcher broke"
    );
}

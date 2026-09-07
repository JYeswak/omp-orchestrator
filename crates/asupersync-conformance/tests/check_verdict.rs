//! `omp-orchestrator-fsu7` — the `--check` comparison contract.
//!
//! NAMED TARGET per the mmt4 ruling: cite as
//! `cargo test -j 2 -p asupersync-conformance --test check_verdict`.
//!
//! # Why this is a test and not a shell sequence
//!
//! The obvious known-good leg is "regenerate to a temp path, then `--check` that path, expect 0".
//! It cannot run on the lane: `rch` admits only compilation commands and refuses a `sh -c`
//! sequence with `[RCH-E301] (non-compilation command)`. Measured, not assumed. So the comparison
//! had to move out of the binary and into the library, where a named target can reach it — which
//! is a better shape anyway, and the constraint is what forced it.
//!
//! # What `--check` replaces
//!
//! `gate.yml` ran `--write ASUPERSYNC-CONFORMANCE.md` then `git diff --exit-code`. Two defects:
//! it MUTATES the tree mid-gate, and `git diff` compares against the WORKTREE — so in a
//! five-agent shared checkout the verdict depends on a peer's uncommitted work and is false in
//! EITHER direction.

use asupersync_conformance::{
    check_document, first_differing_line, render_document, scan_source_text, CheckVerdict,
    CrateRow, Report,
};
use std::path::PathBuf;


fn fixture_report(source: &str) -> Report {
    let spawn_sites =
        scan_source_text("fixture", "src/main.rs", source).expect("fixture source scans");
    Report {
        root: PathBuf::from("fixture"),
        crates: vec![CrateRow {
            name: "fixture".to_owned(),
            forbid_unsafe: false,
            dep_asupersync: false,
            dep_subprocess_contract: false,
            async_fns: 0,
            cx_first: 0,
            checkpoints: 0,
            raw_command: spawn_sites.len(),
            forbidden_deps: Vec::new(),
        }],
        spawn_sites,
        undrained_violations: 0,
    }
}

/// KNOWN-GOOD, mandatory. A document that matches is CURRENT and must not be reported as drift.
///
/// An attack-only suite ships an over-strict gate, and an over-strict gate gets routed around.
#[test]
fn a_matching_document_is_current() {
    let doc = "# header\nline one\nline two\n";
    assert_eq!(
        check_document(doc, doc),
        CheckVerdict::Current,
        "byte-identical input must be CURRENT, or the gate is red forever and gets bypassed"
    );
}

/// FIRES-ON-KNOWN-BAD, and the verdict carries what a reader needs to act.
///
/// The figures are the point: a bare "they differ" sends the reader to `git diff`, which is the
/// worktree comparison this flag exists to avoid.
#[test]
fn a_drifted_document_is_stale_and_names_the_first_differing_line() {
    let stored = "# header\nline one\nline two\n";
    let fresh = "# header\nline ONE\nline two\n";
    match check_document(stored, fresh) {
        CheckVerdict::Stale {
            stored_bytes,
            regenerated_bytes,
            first_differing_line,
        } => {
            assert_eq!(first_differing_line, 2, "1-indexed, and it must name the LINE");
            assert_eq!(stored_bytes, stored.len());
            assert_eq!(regenerated_bytes, fresh.len());
        }
        other => panic!("a differing document must be STALE, got {other:?}"),
    }
}

/// A doc that is a strict PREFIX of the regenerated one differs only in trailing content, so
/// there is no differing line — reported as `0` rather than as a false line number.
///
/// This is the shape drift actually takes here: the report grows when a crate is added. Measured
/// on the real tree — stored 44,668 bytes vs regenerated 48,432 — so getting this case wrong would
/// mislabel the common case.
#[test]
fn trailing_only_growth_reports_line_zero_rather_than_a_wrong_line() {
    let stored = "# header\nline one\n";
    let fresh = "# header\nline one\nline two\nline three\n";
    match check_document(stored, fresh) {
        CheckVerdict::Stale {
            first_differing_line,
            stored_bytes,
            regenerated_bytes,
        } => {
            assert_eq!(
                first_differing_line, 0,
                "no line differs; inventing one would send the reader to the wrong place"
            );
            assert!(regenerated_bytes > stored_bytes);
        }
        other => panic!("expected STALE, got {other:?}"),
    }
}

/// An EMPTY stored document is drift, not a match.
///
/// Anti-vacuity: a zero-byte file must never compare equal to a rendered report. A gate that
/// passes on an empty artifact is the never-fires class.
#[test]
fn an_empty_stored_document_is_never_current() {
    let fresh = "# header\ncontent\n";
    assert_ne!(
        check_document("", fresh),
        CheckVerdict::Current,
        "an empty artifact must never read as a current one"
    );
    assert!(matches!(
        check_document("", fresh),
        CheckVerdict::Stale { stored_bytes: 0, .. }
    ));
}

/// Comparison is byte-exact ON PURPOSE. The document is generated, so any difference is drift,
/// and a normalising comparison would hide the whitespace churn that signals a renderer change.
#[test]
fn comparison_is_byte_exact_and_does_not_normalise() {
    assert_ne!(
        check_document("a\n", "a\n\n"),
        CheckVerdict::Current,
        "a trailing-newline difference is still drift in a generated document"
    );
    assert_ne!(
        check_document("a b\n", "a  b\n"),
        CheckVerdict::Current,
        "internal whitespace must not be normalised away"
    );
}

/// The helper is total: equal inputs, and inputs where one side has no lines at all.
#[test]
fn the_line_helper_is_total() {
    assert_eq!(first_differing_line("same\n", "same\n"), 0);
    assert_eq!(first_differing_line("", ""), 0);
    assert_eq!(first_differing_line("", "x\n"), 0, "no pair to compare");
    assert_eq!(first_differing_line("x\n", "y\n"), 1);
}


/// A source-line insertion must not change a report when the scanned verdict is unchanged.
#[test]
fn a_line_shift_without_a_verdict_change_is_current() {
    let before = "fn run() {\n    let _ = Command::new(\"echo\").status();\n}\n";
    let after = "\n\nfn run() {\n    let _ = Command::new(\"echo\").status();\n}\n";
    let stored = render_document(
        &fixture_report(before),
        "asupersync-conformance fixture",
        "fixture",
    );
    let regenerated = render_document(
        &fixture_report(after),
        "asupersync-conformance fixture",
        "fixture",
    );

    assert_eq!(
        check_document(&stored, &regenerated),
        CheckVerdict::Current,
        "moving a source site without changing its verdict must not stale the document"
    );
}

/// A real triage change must remain visible as drift after source coordinates are removed.
#[test]
fn a_verdict_change_is_stale_and_names_the_site() {
    let no_pipes = "fn run() {\n    let _ = Command::new(\"echo\").status();\n}\n";
    let stdout_piped = "fn run() {\n    let _ = Command::new(\"echo\").stdout(Stdio::piped()).output();\n}\n";
    let stored = render_document(
        &fixture_report(no_pipes),
        "asupersync-conformance fixture",
        "fixture",
    );
    let regenerated = render_document(
        &fixture_report(stdout_piped),
        "asupersync-conformance fixture",
        "fixture",
    );

    assert!(stored.contains("src/main.rs"));
    assert!(regenerated.contains("src/main.rs"));
    assert!(stored.contains("DEADLOCK_SAFE_NO_PIPES"));
    assert!(regenerated.contains("DEADLOCK_SAFE_STDOUT_ONLY"));
    assert!(
        matches!(check_document(&stored, &regenerated), CheckVerdict::Stale { .. }),
        "a changed triage verdict must remain RED rather than disappear with the line number"
    );
}

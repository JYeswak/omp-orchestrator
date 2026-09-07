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
use std::fs;
use std::path::PathBuf;
use std::process::Command;


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

fn fixture_repo(source: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("asupersync-conformance-cli-{nonce}"));
    fs::create_dir_all(root.join("crates/fixture/src")).expect("fixture source directory");
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/fixture\"]\n",
    )
    .expect("fixture workspace manifest");
    fs::write(
        root.join("crates/fixture/Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("fixture package manifest");
    fs::write(root.join("crates/fixture/src/main.rs"), source).expect("fixture source");
    root
}

fn run_cli(root: &std::path::Path, args: &[&str]) -> std::process::Output {
    let root = root.to_str().expect("fixture root utf8");
    Command::new(env!("CARGO_BIN_EXE_asupersync-conformance"))
        .args(["--repo", root])
        .args(args)
        .output()
        .expect("conformance CLI must run")
}

#[test]
fn cli_check_ignores_source_line_shift_when_verdict_unchanged() {
    let source = "fn run() {\n    let _ = Command::new(\"echo\").status();\n}\n";
    let root = fixture_repo(source);
    let document = root.join("ASUPERSYNC-CONFORMANCE.md");
    let document_arg = document.to_str().expect("document path utf8");
    let generated = run_cli(&root, &["--write", document_arg]);
    assert!(generated.status.success(), "write failed: {generated:?}");

    fs::write(
        root.join("crates/fixture/src/main.rs"),
        format!("\n\n{source}"),
    )
    .expect("shift fixture source");
    let checked = run_cli(&root, &["--check", document_arg]);
    assert_eq!(
        checked.status.code(),
        Some(0),
        "line-only source movement must remain current: {}",
        String::from_utf8_lossy(&checked.stdout)
    );
    assert!(String::from_utf8_lossy(&checked.stdout).contains("ASUPERSYNC_CONFORMANCE_CURRENT"));
    fs::remove_dir_all(root).expect("fixture cleanup");
}

#[test]
fn cli_check_refuses_a_verdict_change_and_keeps_the_site_in_the_document() {
    let no_pipes = "fn run() {\n    let _ = Command::new(\"echo\").status();\n}\n";
    let stdout_piped =
        "fn run() {\n    let _ = Command::new(\"echo\").stdout(Stdio::piped()).output();\n}\n";
    let root = fixture_repo(no_pipes);
    let document = root.join("ASUPERSYNC-CONFORMANCE.md");
    let document_arg = document.to_str().expect("document path utf8");
    let generated = run_cli(&root, &["--write", document_arg]);
    assert!(generated.status.success(), "write failed: {generated:?}");
    let stored = fs::read_to_string(&document).expect("stored fixture document");
    assert!(stored.contains("src/main.rs"), "stored document must name the site");

    fs::write(root.join("crates/fixture/src/main.rs"), stdout_piped)
        .expect("change fixture verdict");
    let checked = run_cli(&root, &["--check", document_arg]);
    assert_eq!(checked.status.code(), Some(1), "verdict change must be stale: {checked:?}");
    let stdout = String::from_utf8_lossy(&checked.stdout);
    assert!(stdout.contains("ASUPERSYNC_CONFORMANCE_DRIFT reason=STALE"));
    assert!(stdout.contains("first_differing_line="));
    fs::remove_dir_all(root).expect("fixture cleanup");
}
/// The write operation is deterministic: two independent destinations must receive
/// identical bytes, so a caller can compare or publish either artifact safely.
#[test]
fn cli_write_is_byte_identical_across_distinct_paths() {
    let source = "fn run() {\n    let _ = Command::new(\"echo\").status();\n}\n";
    let root = fixture_repo(source);
    let first = root.join("first/ASUPERSYNC-CONFORMANCE.md");
    let second = root.join("second/ASUPERSYNC-CONFORMANCE.md");
    fs::create_dir_all(first.parent().expect("first parent")).expect("first output directory");
    fs::create_dir_all(second.parent().expect("second parent")).expect("second output directory");
    let first_arg = first.to_str().expect("first output path utf8");
    let second_arg = second.to_str().expect("second output path utf8");

    let first_run = run_cli(&root, &["--write", first_arg]);
    assert!(first_run.status.success(), "first write failed: {first_run:?}");
    let second_run = run_cli(&root, &["--write", second_arg]);
    assert!(second_run.status.success(), "second write failed: {second_run:?}");

    let first_bytes = fs::read(&first).expect("first output must exist");
    let second_bytes = fs::read(&second).expect("second output must exist");
    assert!(!first_bytes.is_empty(), "first write produced an empty document");
    assert!(!second_bytes.is_empty(), "second write produced an empty document");
    assert_eq!(
        first_bytes, second_bytes,
        "consecutive --write runs to distinct paths must be byte-identical"
    );
    fs::remove_dir_all(root).expect("fixture cleanup");
}

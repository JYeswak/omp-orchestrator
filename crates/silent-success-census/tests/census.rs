#![forbid(unsafe_code)]

use serde_json::Value;
use silent_success_census::{render_json, scan_workspace, Classification, Predicate};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("silent-success-census-{label}-{nonce}"));
    fs::create_dir_all(root.join("crates/demo/src")).expect("create fixture");
    root
}

fn write_source(root: &Path, source: &str) {
    fs::write(root.join("crates/demo/src/main.rs"), source).expect("write fixture");
}

#[test]
fn empty_scan_set_is_an_error_not_a_vacuous_pass() {
    let root = temp_root("empty");
    let report = scan_workspace(&root);
    assert_eq!(report.status, "error");
    assert_eq!(report.file_count, 0);
    assert!(report
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("empty"));
    assert!(report.candidates.is_empty());
    fs::remove_dir_all(root).expect("remove fixture");
}

#[test]
fn output_is_deterministic_and_has_the_machine_contract() {
    let root = temp_root("deterministic");
    write_source(
        &root,
        "fn value() -> Result<usize, ()> { Ok(0) }\nfn main() { let _ = value(); }\n",
    );
    let first = scan_workspace(&root);
    let second = scan_workspace(&root);
    let first_json = render_json(&first);
    assert_eq!(first_json, render_json(&second));

    let artifact: Value = serde_json::from_str(&first_json).expect("valid JSON");
    assert_eq!(artifact["schema"], "silent-success-census");
    assert_eq!(artifact["version"], 1);
    assert_eq!(artifact["status"], "ok");
    assert_eq!(artifact["file_count"], 1);
    assert_eq!(artifact["positive_controls"].as_array().unwrap().len(), 2);
    assert!(artifact["predicate_counts"]["ok_zero"] == 1);
    let row = &artifact["candidates"][0];
    assert!(row["file"]
        .as_str()
        .unwrap_or_default()
        .ends_with("main.rs"));
    assert!(row["line"].as_u64().unwrap_or_default() >= 1);
    assert_eq!(row["predicate"], "ok_zero");
    assert!(matches!(
        row["classification"].as_str(),
        Some("typed_nonzero")
    ));
    assert!(!row["reason"].as_str().unwrap_or_default().is_empty());
    assert!(!row["source_excerpt"]
        .as_str()
        .unwrap_or_default()
        .is_empty());
    fs::remove_dir_all(root).expect("remove fixture");
}

/// THE POSITIVE CONTROL: the scanner must re-find real live rows in the real workspace.
///
/// `omp-orchestrator-poumg.5`. This replaces `positive_controls_refind_named_live_rows`, which
/// asserted a hard-coded `(file, LINE, predicate)` table over files in OTHER crates. That test
/// pinned line POSITIONS it did not own, so any edit above a pinned line reded this crate. It
/// had already been re-pinned once — its own comment predicted the recurrence — and it then died
/// again. Measured at `cb9d3941`: `crates/state-wildcard-lint/src/main.rs:78` was
/// `report(&linted);` at HEAD and `Verdict::Clean => ExitCode::SUCCESS,` in the worktree, so the
/// pin held only because of a PEER's uncommitted edit. Deleted, not re-pinned, per AGENTS.md.
///
/// THE PROPERTY IS KEPT AND STRENGTHENED. The anchors are now content-based and live in THIS
/// crate's own `src/`, so no unrelated edit in another crate can move them — and each row is
/// checked for predicate AND classification AND an excerpt token, where the old form checked a
/// line number.
#[test]
fn positive_controls_refind_named_live_rows() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let report = scan_workspace(&root);
    assert_eq!(report.status, "ok");
    assert!(report.file_count >= 3);

    // ANTI-VACUITY (leg 4): a control that passes on an empty scan proves nothing. Assert a
    // NONZERO found-count before asserting anything about individual rows.
    let found = report
        .positive_controls
        .iter()
        .filter(|control| control.found)
        .count();
    assert!(
        found > 0,
        "VACUOUS: the scanner re-found NO positive control. Either the scan set is empty or the \
         detector is broken; both are errors, never a pass. controls: {:?}",
        report.positive_controls
    );
    assert_eq!(
        found,
        report.positive_controls.len(),
        "every declared control must be re-found; a partially-found set means the detector \
         regressed for one predicate. controls: {:?}",
        report.positive_controls
    );

    for control in &report.positive_controls {
        assert!(
            control.file.starts_with("crates/silent-success-census/"),
            "a control anchored OUTSIDE this crate is hostage to another crate's author — the \
             defect poumg.5 deleted. Offending row: {control:?}"
        );
        assert!(
            control.found,
            "positive control not re-found: {control:?}"
        );
        assert!(
            control.line >= 1,
            "a found control must report the line it was found AT; 0 means not found: {control:?}"
        );
        // The classification, not merely the predicate — leg 4.
        let row = report
            .candidates
            .iter()
            .find(|candidate| {
                candidate.file == control.file
                    && candidate.predicate == control.predicate
                    && candidate.source_excerpt.contains(&control.excerpt_token)
            })
            .unwrap_or_else(|| panic!("control claims found but no candidate matches: {control:?}"));
        assert!(
            row.source_excerpt.contains(&control.excerpt_token),
            "the excerpt must carry the anchoring token: {row:?}"
        );
        assert_eq!(
            row.classification,
            control.predicate.classification(),
            "classification drifted for {:?}",
            control.predicate
        );
    }
}

#[test]
fn known_noop_and_fallback_rows_are_not_all_defects() {
    let root = temp_root("classification");
    write_source(
        &root,
        "fn fallback() -> Vec<u8> { return Vec::new(); }\nfn main() -> std::process::ExitCode { std::process::ExitCode::SUCCESS }\n",
    );
    let report = scan_workspace(&root);
    assert!(report
        .candidates
        .iter()
        .any(|row| row.predicate == Predicate::EmptyCollectionReturn
            && row.classification == Classification::Unresolved));
    assert!(report
        .candidates
        .iter()
        .any(|row| row.predicate == Predicate::ExitCodeSuccess
            && row.classification == Classification::HealthyNoop));
    fs::remove_dir_all(root).expect("remove fixture");
}

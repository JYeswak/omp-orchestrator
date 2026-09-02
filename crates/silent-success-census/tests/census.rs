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
    assert_eq!(artifact["positive_controls"].as_array().unwrap().len(), 3);
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

#[test]
fn positive_controls_refind_named_live_rows() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let report = scan_workspace(&root);
    assert_eq!(report.status, "ok");
    assert!(report.file_count >= 3);

    let expected = [
        (
            "crates/admission-reason/src/main.rs",
            40,
            Predicate::ExitCodeSuccess,
        ),
        ("crates/loop-tick/src/lib.rs", 77, Predicate::UnwrapOrDefault),
        (
            "crates/state-wildcard-lint/src/main.rs",
            80,
            Predicate::ExitCodeSuccess,
        ),
    ];
    let found = report
        .positive_controls
        .iter()
        .filter(|control| control.found)
        .count();
    assert!(
        found >= 3,
        "positive controls must re-find at least three known rows, found {found}: {:?}",
        report.positive_controls
    );
    for (file, line, predicate) in expected {
        assert!(
            report.positive_controls.iter().any(|control| {
                control.found
                    && control.file == file
                    && control.line == line
                    && control.predicate == predicate
            }),
            "missing positive control {file}:{line} {predicate:?}: {:?}",
            report.positive_controls
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

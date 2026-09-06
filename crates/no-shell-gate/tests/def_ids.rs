//! DEF-* uniqueness checker (bead omp-orchestrator-def-id-namespace-collision-vjfo).
//!
//! Fixture legs live here so a known-bad tree is not planted under `crates/`.
//! The live-tree unique leg is the production caller: `cargo test -p no-shell-gate --test def_ids`.

#![forbid(unsafe_code)]

use no_shell_gate::def_ids::{parse_def_id_line, scan, ScanError};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    // Tests must not write under crates/. Use process temp, then remove.
    let dir = std::env::temp_dir().join(format!(
        "def-id-gate-{}-{}-{}",
        label,
        std::process::id(),
        nanos
    ));
    fs::create_dir_all(dir.join("docs/plan")).expect("plan tree");
    fs::create_dir_all(dir.join("docs/planning")).expect("planning tree");
    dir
}

fn write_def(root: &Path, rel: &str, id: &str) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent");
    }
    fs::write(path, format!("[[def]]\nid = \"{id}\"\nclass = \"fixture\"\n")).expect("write def");
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

#[test]
fn parse_accepts_definition_line_and_rejects_comments_and_other_families() {
    assert_eq!(parse_def_id_line("id = \"DEF-001\""), Some("DEF-001"));
    assert_eq!(parse_def_id_line("  id = \"DEF-R3-001\""), Some("DEF-R3-001"));
    assert_eq!(parse_def_id_line("id=\"DEF-U-002\""), Some("DEF-U-002"));
    assert_eq!(parse_def_id_line("# id = \"DEF-001\""), None);
    assert_eq!(parse_def_id_line("id = \"NOTE-DANGLING\""), None);
    assert_eq!(parse_def_id_line("id = \"HD-0001\""), None);
}

#[test]
fn missing_tree_is_an_error_not_a_pass() {
    let dir = std::env::temp_dir().join(format!(
        "def-id-missing-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("root");
    let err = scan(&dir).expect_err("missing trees must refuse");
    assert!(
        matches!(err, ScanError::TreeMissing { .. }),
        "got {err:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn empty_scan_set_is_an_error_not_a_pass() {
    let dir = scratch("empty");
    let err = scan(&dir).expect_err("empty trees must refuse");
    assert!(
        matches!(err, ScanError::EmptyScanSet),
        "got {err:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn files_with_no_def_ids_are_an_error_not_a_pass() {
    let dir = scratch("zero");
    fs::write(dir.join("docs/plan/README.md"), "no ids here\n").expect("file");
    fs::write(dir.join("docs/planning/note.txt"), "still none\n").expect("file");
    let err = scan(&dir).expect_err("zero definitions must refuse");
    assert!(
        matches!(err, ScanError::ZeroDefinitions),
        "got {err:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn seven_def_001_definitions_report_defined_7x() {
    let dir = scratch("seven");
    for i in 0..7 {
        write_def(
            &dir,
            &format!("docs/planning/r{i}/DEFECTS.toml"),
            "DEF-001",
        );
    }
    let scan = scan(&dir).expect("seven colliding defs are a readable scan");
    let msgs = scan.duplicate_messages();
    assert!(
        msgs.iter().any(|m| m == "DEF-001 defined 7x"),
        "falsifier form missing: {msgs:?}"
    );
    assert_eq!(scan.by_id["DEF-001"].len(), 7);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn planted_eighth_def_001_reports_defined_8x() {
    let dir = scratch("eight");
    for i in 0..7 {
        write_def(
            &dir,
            &format!("docs/planning/r{i}/DEFECTS.toml"),
            "DEF-001",
        );
    }
    write_def(&dir, "docs/plan/flow/unknowns/DEFECTS.toml", "DEF-001");
    let scan = scan(&dir).expect("eight colliding defs are a readable scan");
    let msgs = scan.duplicate_messages();
    assert!(
        msgs.iter().any(|m| m == "DEF-001 defined 8x"),
        "anti-vacuity plant missing: {msgs:?}"
    );
    assert_eq!(scan.by_id["DEF-001"].len(), 8);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn unique_round_scoped_ids_are_clean() {
    let dir = scratch("unique");
    write_def(&dir, "docs/plan/flow/unknowns/DEFECTS.toml", "DEF-U-001");
    write_def(&dir, "docs/planning/round3/DEFECTS.toml", "DEF-R3-001");
    let scan = scan(&dir).expect("unique ids are a readable scan");
    assert!(scan.duplicates().is_empty(), "{:?}", scan.duplicate_messages());
    assert_eq!(scan.definition_count(), 2);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn live_tree_def_ids_are_unique() {
    let scan = scan(&repo_root()).expect("live docs/plan and docs/planning must scan");
    assert!(
        scan.files_scanned > 0,
        "ANTI-VACUITY: live scan covered no files"
    );
    assert!(
        scan.definition_count() >= 14,
        "ANTI-VACUITY: live definition count collapsed to {}",
        scan.definition_count()
    );
    assert!(
        scan.duplicates().is_empty(),
        "live DEF-* collisions: {:?}",
        scan.duplicate_messages()
    );
}

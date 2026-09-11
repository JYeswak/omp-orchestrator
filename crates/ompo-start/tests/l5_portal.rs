#![forbid(unsafe_code)]

use ompo_start::portal::{data_hash_without_self, seal, SCHEMA_ID};
use serde_json::json;

use ompo_start::portal::queue_depth;

#[test]
fn portal_hash_excludes_its_own_field() {
    let row = json!({
        "schema_id": SCHEMA_ID,
        "schema_version": "1",
        "sources": {},
        "_alerts": [],
        "one_next_action": {"command": "halt", "reason_code": "HD-0009"},
        "data_hash": "placeholder",
    });
    let sealed = seal(row.clone()).expect("object row");
    let hash = sealed["data_hash"].as_str().expect("hash string");
    assert_eq!(hash.len(), 64);
    assert_eq!(hash, data_hash_without_self(&row));

    let mut changed = sealed.clone();
    changed["data_hash"] = json!("different");
    assert_eq!(hash, data_hash_without_self(&changed));
    changed["one_next_action"]["command"] = json!("ompo start --json");
    assert_ne!(hash, data_hash_without_self(&changed));
}

#[test]
fn portal_seal_refuses_non_object_rows() {
    let error = seal(json!(["not", "a", "row"]))
        .expect_err("portal rows must be objects");
    assert_eq!(error, "L5_PORTAL_ROW_NOT_OBJECT");
}

fn mirror(dir: &std::path::Path, body: &str) {
    let beads = dir.join(".beads");
    std::fs::create_dir_all(&beads).unwrap();
    std::fs::write(beads.join("issues.jsonl"), body).unwrap();
}

/// KNOWN-GOOD: exact depth + distribution over a known mirror. Terminal is
/// closed + tombstone; everything else counts.
#[test]
fn queue_depth_counts_non_terminal() {
    let dir = std::env::temp_dir().join(format!("omp-queue-full-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    mirror(
        &dir,
        "{\"id\":\"a\",\"status\":\"open\"}\n{\"id\":\"b\",\"status\":\"in_progress\"}\n{\"id\":\"c\",\"status\":\"closed\"}\n{\"id\":\"d\",\"status\":\"tombstone\"}\n{\"id\":\"e\",\"status\":\"blocked\"}\n",
    );
    let queue = queue_depth(&dir);
    assert_eq!(queue["state"], "FULL");
    assert_eq!(queue["depth"], 3);
    assert_eq!(queue["by_status"]["open"], 1);
    assert_eq!(queue["by_status"]["closed"], 1);
    assert_eq!(queue["source"], ".beads/issues.jsonl");
    let _ = std::fs::remove_dir_all(&dir);
}

/// KNOWN-BAD (mutation target): a missing mirror is TYPED UNOBSERVABLE, never
/// an empty depth and never a panic. Break the source -> typed refusal.
#[test]
fn queue_depth_missing_mirror_is_unobservable() {
    let dir = std::env::temp_dir().join(format!("omp-queue-absent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let queue = queue_depth(&dir);
    assert_eq!(queue["state"], "UNOBSERVABLE");
    assert!(queue.get("reason").is_some(), "refusal carries why: {queue}");
    assert!(
        queue.get("depth").is_none(),
        "no depth when nothing was read: {queue}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// PARTIAL: corrupt lines bound the result instead of failing it or
/// silently dropping rows. Break some lines -> typed PARTIAL with bounds.
#[test]
fn queue_depth_corrupt_lines_are_partial() {
    let dir = std::env::temp_dir().join(format!("omp-queue-partial-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    mirror(
        &dir,
        "{\"id\":\"a\",\"status\":\"open\"}\nNOT-JSON\n{\"id\":\"c\",\"status\":\"closed\"}\n",
    );
    let queue = queue_depth(&dir);
    assert_eq!(queue["state"], "PARTIAL");
    assert_eq!(queue["bound_kind"], "unparseable_lines");
    assert_eq!(queue["bound_value"], 2);
    assert_eq!(queue["lines_total"], 3);
    assert_eq!(queue["depth"], 1);
    let _ = std::fs::remove_dir_all(&dir);
}

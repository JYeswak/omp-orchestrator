#![forbid(unsafe_code)]

use ompo_start::portal::{
    data_hash_without_self, gates_verdict, parse_gates_aggregate, queue_depth, seal, SCHEMA_ID,
    SCHEMA_VERSION,
};
use serde_json::json;

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

/// KNOWN-GOOD: a well-formed aggregate line parses to its six keys with the
/// invariant holding. Grammar mirrors ci_citation::parse_aggregate.
#[test]
fn gates_aggregate_parses_valid_line() {
    let values =
        parse_gates_aggregate("GATE_RUNNER crates=88 pass=80 fail=5 unmeasurable=3 short=0 no_tests=0\n")
            .expect("valid line parses");
    assert_eq!(values["crates"], 88);
    assert_eq!(values["pass"], 80);
    assert_eq!(values["fail"], 5);
}

/// KNOWN-BAD (mutation targets): each refusal shape is pinned so the grammar
/// cannot drift -- missing line, missing key, broken invariant, non-numeric.
#[test]
fn gates_aggregate_refusals_are_typed() {
    assert!(parse_gates_aggregate("nothing here\n").is_err());
    assert!(parse_gates_aggregate("GATE_RUNNER crates=2 pass=2 fail=0 unmeasurable=0 short=0\n").is_err());
    assert!(parse_gates_aggregate("GATE_RUNNER crates=2 pass=1 fail=0 unmeasurable=0 short=0 no_tests=0\n").is_err());
    assert!(parse_gates_aggregate("GATE_RUNNER crates=2 pass=x fail=0 unmeasurable=0 short=0 no_tests=0\n").is_err());
}

/// KNOWN-BAD end to end: no aggregate file is TYPED UNOBSERVABLE (never a
/// zero, never a panic). Break the source -> typed refusal.
#[test]
fn gates_verdict_missing_file_is_unobservable() {
    let dir = std::env::temp_dir().join(format!("omp-gates-absent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let gates = gates_verdict(&dir);
    assert_eq!(gates["state"], "UNOBSERVABLE");
    assert!(gates.get("reason").is_some(), "refusal carries why: {gates}");
    assert!(
        gates.get("crates").is_none() && gates.get("fail").is_none(),
        "no verdict fields when nothing was read: {gates}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// KNOWN-GOOD end to end: a written aggregate file reads FULL with its keys.
#[test]
fn gates_verdict_present_file_is_full() {
    let dir = std::env::temp_dir().join(format!("omp-gates-present-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let target = dir.join("target");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(
        target.join("gate-runner-aggregate.line"),
        "GATE_RUNNER crates=4 pass=3 fail=1 unmeasurable=0 short=0 no_tests=0\n",
    )
    .unwrap();
    let gates = gates_verdict(&dir);
    assert_eq!(gates["state"], "FULL");
    assert_eq!(gates["crates"], 4);
    assert_eq!(gates["fail"], 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// ciay, THE ROW-LEVEL SCHEMA IDENTITY: both halves of the portal row's schema
/// contract are pinned to the LITERAL tokens the contract names, not to each
/// other. Asserting `SCHEMA_ID == SCHEMA_ID` would be a tautology that survives
/// any rename; these compare against the spelled-out strings, so changing
/// either constant reddens this leg and nothing else.
///
/// `schema_version` is covered here because until now it was an inline literal
/// in `ompo-doctor`'s `run_portal` -- the row emitted a version that no test in
/// `ompo-start` could see, so deleting or changing it was undetectable.
#[test]
fn portal_row_schema_identity_is_contract() {
    assert_eq!(SCHEMA_ID, "ompo:portal:v1");
    assert_eq!(SCHEMA_VERSION, "1");
}

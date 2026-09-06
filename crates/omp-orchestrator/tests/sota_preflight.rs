#![forbid(unsafe_code)]

use omp_orchestrator::jsm_suggest::{count_named, parse_jsm_suggest, skill_count};
use std::path::PathBuf;

fn retained_jsm() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.flywheel/sota-preflight/jsm-suggest.json")
}

#[test]
fn retained_jsm_suggest_has_sha_and_nonzero_named_skill() {
    let path = retained_jsm();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("retained jsm suggest missing at {}: {error}", path.display()));
    let parsed = parse_jsm_suggest(&text).expect("retained jsm suggest must parse");
    assert!(parsed.success, "jsm suggest success=false");
    let count = skill_count(&parsed);
    assert!(count > 0, "derived count must be non-zero; names={:?}", parsed.skill_names);
    assert!(
        count_named(&parsed, "ntm") > 0,
        "positive control: known-present skill ntm; names={:?}",
        parsed.skill_names
    );
    let sha_path = path.with_file_name("jsm-suggest.json.sha256");
    let sha = std::fs::read_to_string(&sha_path)
        .unwrap_or_else(|error| panic!("sha256 sidecar missing at {}: {error}", sha_path.display()));
    assert!(
        sha.trim().len() >= 64,
        "sha256 sidecar too short: {sha}"
    );
}

#[test]
fn parser_positive_control_on_known_present_skill_is_nonzero() {
    let fixture = r#"{"success":true,"suggestions":[{"skill_name":"ntm"}]}"#;
    let parsed = parse_jsm_suggest(fixture).expect("fixture");
    assert!(count_named(&parsed, "ntm") > 0);
}

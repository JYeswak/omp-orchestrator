#![forbid(unsafe_code)]

use omp_inventory_map::count_twins::{
    evaluate_envelope, evaluate_envelope_guarded, TwinError, COUNT_TWINS,
};
use serde_json::{json, Value};
use std::process::Command;

fn matched_counts() -> Value {
    json!({
        "cli_commands": 39,
        "type_roots": 57,
        "declarations": 14,
        "rpc_handlers": 42,
        "slash_commands": 136,
        "omp_methods": 3,
        "workspace_crates": 72,
        "expected_cli_commands": 39,
        "expected_type_roots": 57,
        "expected_declarations": 14,
        "expected_rpc_handlers": 42,
        "expected_slash_commands": 136,
        "expected_omp_methods": 3,
        "expected_workspace_crates": 72
    })
}

fn counts_with_slash(observed: usize, expected: usize) -> Value {
    let mut counts = matched_counts();
    counts["slash_commands"] = json!(observed);
    counts["expected_slash_commands"] = json!(expected);
    counts
}

fn envelope(status: &str, counts: Value, error: Option<&str>) -> Value {
    json!({
        "status": status,
        "data": { "counts": counts },
        "error": error
    })
}

#[test]
fn seven_pairs_are_enumerated_and_nonempty() {
    assert_eq!(COUNT_TWINS.len(), 7);
    assert!(!COUNT_TWINS.is_empty());
    for (observed, expected) in COUNT_TWINS {
        assert!(expected.starts_with("expected_"));
        assert_eq!(&expected["expected_".len()..], *observed);
    }
}

#[test]
fn unknown_pair_name_in_envelope_is_typed_error() {
    let mut counts = matched_counts();
    counts["expected_widgets"] = json!(1);
    let err = evaluate_envelope(&envelope("OK", counts, None)).expect_err("extra pair");
    assert!(
        err.to_string().contains("COUNT_TWIN_UNKNOWN_PAIR pair=expected_widgets"),
        "{err}"
    );
}

#[test]
fn expected_without_observed_twin_is_typed_error() {
    let mut counts = matched_counts();
    counts.as_object_mut().unwrap().remove("slash_commands");
    let err = evaluate_envelope(&envelope("OK", counts, None)).expect_err("missing observed");
    assert!(
        err.to_string()
            .contains("COUNT_TWIN_EXPECTED_WITHOUT_OBSERVED pair=slash_commands"),
        "{err}"
    );
}

#[test]
fn known_bad_mismatch_with_status_ok_fails_naming_the_pair() {
    let err = evaluate_envelope(&envelope(
        "OK",
        counts_with_slash(0, 136),
        None,
    ))
    .expect_err("mismatch+OK");
    let text = err.to_string();
    assert!(text.contains("pair=slash_commands"), "{text}");
    assert!(text.contains("observed=0"), "{text}");
    assert!(text.contains("expected=136"), "{text}");
}

#[test]
fn known_bad_unknown_without_named_pair_fails() {
    let err = evaluate_envelope(&envelope(
        "UNKNOWN",
        counts_with_slash(0, 136),
        None,
    ))
    .expect_err("UNKNOWN unnamed");
    assert!(
        err.to_string().contains("COUNT_TWIN_UNKNOWN_WITHOUT_NAMED_PAIR"),
        "{err}"
    );
}

#[test]
fn known_bad_unknown_naming_a_matching_pair_fails() {
    let err = evaluate_envelope(&envelope(
        "UNKNOWN",
        matched_counts(),
        Some("COUNT_TWIN_MISMATCH pair=slash_commands observed=136 expected=136"),
    ))
    .expect_err("names a match");
    assert!(
        err.to_string()
            .contains("COUNT_TWIN_UNKNOWN_NAMES_MATCHING_PAIR pair=slash_commands"),
        "{err}"
    );
}

#[test]
fn known_good_all_seven_match_status_ok() {
    evaluate_envelope(&envelope("OK", matched_counts(), None)).expect("all match");
}

#[test]
fn known_good_one_mismatch_unknown_and_named() {
    evaluate_envelope(&envelope(
        "UNKNOWN",
        counts_with_slash(0, 136),
        Some("COUNT_TWIN_MISMATCH pair=slash_commands observed=0 expected=136"),
    ))
    .expect("one named mismatch");
}

#[test]
fn known_good_two_mismatches_both_named() {
    let mut counts = counts_with_slash(0, 136);
    counts["omp_methods"] = json!(0);
    evaluate_envelope(&envelope(
        "UNKNOWN",
        counts,
        Some(
            "COUNT_TWIN_MISMATCH pair=slash_commands observed=0 expected=136; COUNT_TWIN_MISMATCH pair=omp_methods observed=0 expected=3",
        ),
    ))
    .expect("two named mismatches");
}

#[test]
fn empty_envelope_is_typed_error_not_a_pass() {
    let err = evaluate_envelope(&json!({})).expect_err("empty");
    assert!(matches!(err, TwinError::EnvelopeEmpty), "{err}");
    let err = evaluate_envelope(&json!({"status": "OK"})).expect_err("no counts");
    assert!(matches!(err, TwinError::NoPairsFound), "{err}");
    let err = evaluate_envelope(&json!({"data": {"counts": {}}})).expect_err("no status");
    assert!(matches!(err, TwinError::NoStatus), "{err}");
}

#[test]
fn mutation_inverting_the_guard_goes_red_then_restores() {
    let bad = envelope("OK", counts_with_slash(0, 136), None);
    assert!(evaluate_envelope(&bad).is_err(), "guard present");
    assert!(
        evaluate_envelope_guarded(&bad, false).is_ok(),
        "inverted guard misses mismatch+OK — RED"
    );
    assert!(evaluate_envelope(&bad).is_err(), "restore still refuses");
    let source = include_str!("../src/count_twins.rs");
    assert!(source.contains("evaluate_envelope_guarded(envelope, true)"));
}

#[test]
fn live_inventory_envelope_names_slash_commands_unknown() {
    let bin = env!("CARGO_BIN_EXE_omp-inventory-map");
    let output = Command::new(bin)
        .arg("doctor")
        .arg("--json")
        .output()
        .expect("spawn omp-inventory-map doctor");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let code = output.status.code().unwrap_or(-1);
    assert!(
        !stdout.trim().is_empty(),
        "live envelope stdout empty; stderr={stderr}"
    );
    let envelope: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|_| {
        panic!("live envelope is not JSON status={code} stdout={stdout} stderr={stderr}")
    });
    let status = envelope.get("status").and_then(Value::as_str).unwrap_or("");
    let error = envelope.get("error").and_then(Value::as_str).unwrap_or("");
    assert_eq!(status, "UNKNOWN", "live status={status} error={error}");
    assert!(
        error.contains("pair=slash_commands"),
        "live error must name slash_commands, got {error}"
    );
    assert!(
        error.contains("observed=0"),
        "live error must name observed=0, got {error}"
    );
    assert!(
        error.contains("expected=136"),
        "live error must name expected=136, got {error}"
    );
    evaluate_envelope(&envelope).expect("live envelope must satisfy the named-mismatch conditional");
    assert_eq!(code, 2, "live exit must be 2; error={error}");
}

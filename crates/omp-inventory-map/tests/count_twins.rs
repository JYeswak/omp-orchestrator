#![forbid(unsafe_code)]

use omp_inventory_map::count_twins::{
    evaluate_envelope, evaluate_envelope_guarded, mismatches, ProbeMismatch, REQUIRED_PROBES,
};
use omp_inventory_map::{ProbeEvidence, ProbeState};
use serde_json::json;

fn known_probes() -> Vec<ProbeEvidence> {
    REQUIRED_PROBES
        .iter()
        .map(|name| ProbeEvidence {
            name: (*name).to_owned(),
            command: vec!["live-probe".to_owned(), (*name).to_owned()],
            state: ProbeState::Known,
            observed: Some(1),
            output: "measured".to_owned(),
            detail: "live evidence".to_owned(),
        })
        .collect()
}

fn envelope(status: &str, probes: &[ProbeEvidence], error: Option<&str>) -> serde_json::Value {
    json!({
        "status": status,
        "data": {"probes": probes},
        "error": error,
    })
}

#[test]
fn required_probe_set_is_nonempty_and_unique() {
    assert!(!REQUIRED_PROBES.is_empty());
    for (index, name) in REQUIRED_PROBES.iter().enumerate() {
        assert!(!name.is_empty());
        assert!(!REQUIRED_PROBES[index + 1..].contains(name));
    }
}

#[test]
fn known_nonempty_live_evidence_has_no_mismatches() {
    assert!(mismatches(&known_probes()).is_empty());
}

#[test]
fn missing_probe_is_a_typed_mismatch() {
    let mut probes = known_probes();
    probes.pop();
    let rows = mismatches(&probes);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, REQUIRED_PROBES[REQUIRED_PROBES.len() - 1]);
}

#[test]
fn zero_probe_is_not_a_match() {
    let mut probes = known_probes();
    probes[0].observed = Some(0);
    let rows = mismatches(&probes);
    assert_eq!(rows, vec![ProbeMismatch {
        name: REQUIRED_PROBES[0].to_owned(),
        state: ProbeState::Known,
        observed: Some(0),
    }]);
}

#[test]
fn unknown_probe_is_not_a_match() {
    let mut probes = known_probes();
    probes[1].state = ProbeState::Unknown;
    let rows = mismatches(&probes);
    assert_eq!(rows[0].name, REQUIRED_PROBES[1]);
}

#[test]
fn known_good_envelope_is_accepted() {
    evaluate_envelope(&envelope("OK", &known_probes(), None)).expect("known evidence");
}

#[test]
fn known_bad_ok_status_names_the_probe() {
    let mut probes = known_probes();
    probes[0].observed = Some(0);
    let error = format!("{}", omp_inventory_map::count_twins::format_mismatches(&mismatches(&probes)));
    let err = evaluate_envelope(&envelope("OK", &probes, Some(&error))).expect_err("bad OK");
    assert!(err.to_string().contains("MISMATCH_CLAIMED_OK"));
}

#[test]
fn unknown_status_requires_matching_named_probe() {
    let mut probes = known_probes();
    probes[0].state = ProbeState::Unknown;
    let error = format!("{}", omp_inventory_map::count_twins::format_mismatches(&mismatches(&probes)));
    evaluate_envelope(&envelope("UNKNOWN", &probes, Some(&error))).expect("named unknown");
}

#[test]
fn mutation_disabling_guard_is_not_the_production_contract() {
    let mut probes = known_probes();
    probes[0].observed = Some(0);
    let error = format!("{}", omp_inventory_map::count_twins::format_mismatches(&mismatches(&probes)));
    evaluate_envelope_guarded(&envelope("OK", &probes, Some(&error)), false)
        .expect("mutation leg isolates the guard");
    assert!(evaluate_envelope(&envelope("OK", &probes, Some(&error))).is_err());
}

#[test]
fn empty_probe_set_is_an_error_not_a_vacuous_pass() {
    let err = evaluate_envelope(&json!({"status":"OK","data":{"probes":[]}}))
        .expect_err("empty evidence");
    assert!(err.to_string().contains("COUNT_PROBE"));
}

#[test]
fn lsp_method_constant_cannot_reappear() {
    let source = include_str!("../src/lib.rs");
    assert!(!source.contains("EXPECTED_OMP_METHODS"));
    assert!(!source.contains("parse_omp_methods_source"));
    assert!(!source.contains("omp_methods"));
}

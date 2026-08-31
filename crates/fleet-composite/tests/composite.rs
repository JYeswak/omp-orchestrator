use std::collections::BTreeMap;

use fleet_composite::{
    arithmetic_headline, closed_fraction, compute, compute_json, geometric_headline, parse_raw,
    run_selftest, FACTORS,
};

fn raw(commits: f64, busy: f64, fresh: f64, closed: f64) -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("commits_1h".to_owned(), commits),
        ("omp_busy".to_owned(), busy),
        ("ledger_fresh".to_owned(), fresh),
        ("beads_closed_1h".to_owned(), closed),
    ])
}

#[test]
fn measured_row_scores_zero_and_names_dead_factors() {
    let report = compute(&raw(7.0, 0.0, 0.0, 0.0));
    assert_eq!(report.headline_pct, 0.0);
    assert_eq!(
        report.dead_factors,
        vec!["beads_closed_1h", "ledger_fresh", "omp_busy"]
    );
    assert_eq!(report.verdict, "DEAD");
}

#[test]
fn healthy_row_scores_full_credit() {
    assert_eq!(compute(&raw(6.0, 3.0, 1.0, 3.0)).headline_pct, 100.0);
}

#[test]
fn geometric_mean_prefers_balanced_over_spiky_at_equal_total() {
    let spiky = compute(&raw(6.0, 3.0, 0.0, 0.0));
    let balanced = compute(&raw(3.0, 1.5, 0.5, 1.5));
    assert_eq!(spiky.headline_pct, 0.0);
    assert_eq!(balanced.headline_pct, 50.0);
    assert!(balanced.headline_pct > spiky.headline_pct);
}

#[test]
fn regression_clamps_to_zero_without_crashing() {
    assert_eq!(compute(&raw(-1.0, 3.0, 1.0, 3.0)).headline_pct, 0.0);
}

#[test]
fn overshoot_caps_at_optimum() {
    assert_eq!(compute(&raw(600.0, 1.5, 1.0, 1.5)).headline_pct, 70.71);
}

#[test]
fn arithmetic_mutation_would_hide_measured_dead_fleet() {
    let measured = raw(7.0, 0.0, 0.0, 0.0);
    let closed: BTreeMap<_, _> = measured
        .iter()
        .map(|(name, score)| {
            let spec = FACTORS.iter().find(|factor| factor.name == name).unwrap();
            (
                name.clone(),
                closed_fraction(*score, spec.baseline, spec.optimum),
            )
        })
        .collect();
    let mutation = arithmetic_headline(&closed);
    assert!(mutation > 0.0);
    assert_eq!((mutation * 100.0 * 100.0).round() / 100.0, 25.0);
    assert_eq!(geometric_headline(&closed), 0.0);
}

#[test]
fn empty_input_fails_closed_without_inventing_factors() {
    let report = compute(&BTreeMap::new());
    assert_eq!(report.headline_pct, 0.0);
    assert_eq!(report.verdict, "DEAD");
    assert!(report.dead_factors.is_empty());
}

#[test]
fn malformed_json_fails_closed_with_structured_error() {
    let report = compute_json("{not-json");
    assert_eq!(report.headline_pct, 0.0);
    assert_eq!(report.verdict, "DEAD");
    assert!(report.input_error.is_some());
    assert!(parse_raw("[]").is_err());
}

#[test]
fn non_numeric_factor_fails_closed() {
    let report = compute_json(r#"{"omp_busy":"3"}"#);
    assert_eq!(report.headline_pct, 0.0);
    assert_eq!(report.verdict, "DEAD");
    assert!(report.input_error.is_some());
}

#[test]
fn selftest_preserves_all_eleven_oracle_assertions() {
    let result = run_selftest();
    assert_eq!(result.checked, 11);
    assert!(result.failures.is_empty(), "{:?}", result.failures);
}

#[test]
fn report_has_stable_schema_and_mean_kind() {
    let report = compute(&raw(6.0, 3.0, 1.0, 3.0));
    assert_eq!(report.schema, "zs.fleet-composite.v1");
    assert_eq!(report.mean_kind, "geometric");
}

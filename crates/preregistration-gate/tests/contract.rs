use preregistration_gate::{
    EvidenceRow, GateError, parse_evidence_rows, parse_registry, validate_pre_write,
};

const BASE: &str = "0123456789abcdef0123456789abcdef01234567";
const REGISTRY: &str = r#"{"id":"h1","prediction":"plan remains materialized","falsifier":"missing section","evidence_scope":"docs/plan","recorded_commit":"0123456789abcdef0123456789abcdef01234567","observed_result":null}"#;

#[test]
fn known_good_parent_and_cited_evidence_pass() {
    let paths = vec!["docs/plan/01-idea.md".to_owned(), "docs/PLAN.md".to_owned()];
    let evidence = vec![EvidenceRow {
        path: "docs/plan/01-idea.md".to_owned(),
        line: 1,
        hypothesis_id: Some("h1".to_owned()),
    }];
    let report = validate_pre_write(REGISTRY, BASE, &[BASE.to_owned()], &paths, &evidence)
        .expect("valid gate");
    assert_eq!(report.schema_version, "preregistration-gate/v1");
    assert_eq!(report.hypothesis_count, 1);
    assert_eq!(report.changed_path_count, 2);
    assert_eq!(report.checked_path_count, 1);
    assert_eq!(report.checked_evidence_row_count, 1);
}

#[test]
fn missing_registry_is_an_error_not_a_vacuous_pass() {
    let error = parse_registry("").expect_err("empty registry must refuse");
    assert_eq!(error, GateError::EmptyRegistry);
}

#[test]
fn missing_prior_prediction_citation_is_refused() {
    let paths = vec!["docs/plan/FINDINGS.jsonl".to_owned()];
    let evidence = vec![EvidenceRow {
        path: paths[0].clone(),
        line: 4,
        hypothesis_id: None,
    }];
    let error = validate_pre_write(REGISTRY, BASE, &[BASE.to_owned()], &paths, &evidence)
        .expect_err("uncited evidence must refuse");
    assert!(matches!(
        error,
        GateError::MissingHypothesisCitation { line: 4, .. }
    ));
}

#[test]
fn unknown_prediction_citation_is_refused() {
    let paths = vec!["docs/plan/FINDINGS.jsonl".to_owned()];
    let evidence = vec![EvidenceRow {
        path: paths[0].clone(),
        line: 7,
        hypothesis_id: Some("missing".to_owned()),
    }];
    let error = validate_pre_write(REGISTRY, BASE, &[BASE.to_owned()], &paths, &evidence)
        .expect_err("unknown hypothesis must refuse");
    assert!(matches!(
        error,
        GateError::UnknownHypothesis { line: 7, .. }
    ));
}

#[test]
fn mutation_of_parent_revision_is_refused() {
    let error = validate_pre_write(
        REGISTRY,
        "fedcba9876543210fedcba9876543210fedcba98",
        &[],
        &[],
        &[],
    )
    .expect_err("hypothesis not in parent must refuse");
    assert!(matches!(error, GateError::HypothesisOutOfParent { .. }));
}

#[test]
fn evidence_parser_requires_jsonl_rows() {
    let rows = parse_evidence_rows(
        "docs/plan/FINDINGS.jsonl",
        r#"{"hypothesis_id":"h1"}
{"hypothesis_id":null}
"#,
    )
    .expect("evidence rows parse");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].hypothesis_id.as_deref(), Some("h1"));
    assert_eq!(rows[1].hypothesis_id, None);
}

#[test]
fn evidence_parser_can_limit_validation_to_added_rows() {
    let rows = preregistration_gate::parse_evidence_rows_at(
        "docs/plan/FINDINGS.jsonl",
        r#"{"round":15}
{"hypothesis_id":"h1"}
"#,
        &[2],
    )
    .expect("selected evidence row parses");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].line, 2);
    assert_eq!(rows[0].hypothesis_id.as_deref(), Some("h1"));
}

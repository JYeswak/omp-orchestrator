use porting_gate::{assess, assess_many, ClauseState, GateStatus, PortingFacts};

fn facts() -> PortingFacts {
    PortingFacts {
        wired: true,
        surface_declared: true,
        asupersync_conformant: true,
        repository_green: true,
        no_shell_or_python: true,
        inventory_fields_complete: true,
    }
}

#[test]
fn known_good_ported_crate_passes_all_clauses() {
    let report = assess("known-good", facts());
    assert_eq!(report.status, GateStatus::Pass);
    assert!(report
        .clauses
        .iter()
        .all(|clause| clause.state == ClauseState::Pass));
}

#[test]
fn no_caller_is_refused_by_clause_one() {
    let mut candidate = facts();
    candidate.wired = false;
    let report = assess("unwired", candidate);
    assert_eq!(report.status, GateStatus::Refused);
    assert_eq!(report.clauses[0].code, "CLAUSE_1_WIRED");
    assert_eq!(report.clauses[0].state, ClauseState::Refused);
}

#[test]
fn missing_surface_declaration_is_refused_by_clause_two() {
    let mut candidate = facts();
    candidate.surface_declared = false;
    let report = assess("undeclared", candidate);
    assert_eq!(report.status, GateStatus::Refused);
    assert_eq!(report.clauses[1].code, "CLAUSE_2_SURFACE_DECLARED");
    assert_eq!(report.clauses[1].state, ClauseState::Refused);
}

#[test]
fn zero_candidates_is_a_typed_error() {
    assert!(matches!(assess_many(Vec::new()), Err(error) if error.to_string().contains("ANTI_VACUITY")));
}

#[test]
fn every_clause_is_visible_in_the_report() {
    let report = assess("candidate", facts());
    let codes: Vec<_> = report
        .clauses
        .iter()
        .map(|clause| clause.code)
        .collect();
    assert_eq!(
        codes,
        vec![
            "CLAUSE_1_WIRED",
            "CLAUSE_2_SURFACE_DECLARED",
            "CLAUSE_3_ASUPERSYNC",
            "CLAUSE_4_REPOSITORY_GREEN",
            "CLAUSE_5_NO_SH_PY",
            "CLAUSE_6_INVENTORY_FIELDS",
        ]
    );
}

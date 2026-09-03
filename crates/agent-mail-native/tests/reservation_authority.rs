#![forbid(unsafe_code)]

use agent_mail_native::journey::{
    parse_authoritative_reservation_report, ReservationConflictReport,
};
use serde_json::json;

fn two_conflicts() -> serde_json::Value {
    json!({
        "conflict_free": false,
        "conflicts": [
            {"agent": "AmberGate", "path": "src/lib.rs", "exclusive": true},
            {"agent": "AmberGate", "path": "src/main.rs", "exclusive": true}
        ],
        "total_conflicting_reservations": 2,
        "clear_paths": ["Cargo.toml"],
        "authoritative_source": "database_snapshot"
    })
}

#[test]
fn authoritative_report_preserves_exclusive_conflicts_and_clear_paths() {
    let report: ReservationConflictReport =
        parse_authoritative_reservation_report(&two_conflicts()).expect("report");
    assert!(!report.conflict_free());
    assert_eq!(report.total_conflicting_reservations(), 2);
    assert_eq!(report.conflicts().len(), 2);
    assert!(report.conflicts().iter().all(|conflict| conflict.exclusive));
    assert_eq!(report.clear_paths(), &["Cargo.toml"]);
    assert_eq!(report.authoritative_source(), "database_snapshot");
}

#[test]
fn an_empty_array_is_not_a_reservation_report() {
    let error = parse_authoritative_reservation_report(&json!([]))
        .expect_err("a roster reader returning [] must be refused");
    assert!(error.to_string().contains("object"), "{error}");
}

#[test]
fn inconsistent_empty_success_is_refused_not_treated_as_clear() {
    let error = parse_authoritative_reservation_report(&json!({
        "conflict_free": false,
        "conflicts": [],
        "total_conflicting_reservations": 0,
        "authoritative_source": "database_snapshot"
    }))
    .expect_err("conflict-free false with no rows is an invalid report");
    assert!(error.to_string().contains("inconsistent"), "{error}");
}

#[test]
fn a_valid_empty_authoritative_report_is_distinct_from_unreadable() {
    let report = parse_authoritative_reservation_report(&json!({
        "conflict_free": true,
        "conflicts": [],
        "total_conflicting_reservations": 0,
        "authoritative_source": "database_snapshot"
    }))
    .expect("a readable empty reservation set is valid");
    assert!(report.conflict_free());
    assert_eq!(report.total_conflicting_reservations(), 0);
}

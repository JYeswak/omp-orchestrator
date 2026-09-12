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

use agent_mail_native::close_lease::{
    catalogue_has_lease_listing, lease_refusal_text, match_held_leases, parse_lease_records,
    LeaseRecord,
};
use agent_mail_native::MailError;

fn held_report() -> agent_mail_native::journey::ReservationConflictReport {
    agent_mail_native::journey::parse_authoritative_reservation_report(&two_conflicts())
        .expect("fixture report parses")
}

fn full_record() -> LeaseRecord {
    parse_lease_records(
        "LEASE bead=omp-orchestrator-3w9l holder=AmberGate ack=RubyGate paths=src/lib.rs,src/main.rs",
    )
    .expect("valid record")[0]
        .clone()
}

/// The recorded live catalogue: all 45 daemon tool names probed 2026-09-12.
/// Pinned so a drift that ADDS a listing tool reddens the ABSENT leg below.
fn recorded_catalogue_45() -> Vec<String> {
    [
        "health_check",
        "ensure_project",
        "register_agent",
        "create_agent_identity",
        "retire_agent",
        "unretire_agent",
        "deregister_agent",
        "whois",
        "resolve_pane_identity",
        "cleanup_pane_identities",
        "list_agents",
        "send_message",
        "reply_message",
        "fetch_inbox",
        "fetch_topic",
        "fetch_inbox_events",
        "mark_message_read",
        "mark_all_read",
        "acknowledge_message",
        "get_message_delivery_receipt",
        "request_contact",
        "respond_contact",
        "list_contacts",
        "set_contact_policy",
        "check_file_reservation_conflicts",
        "file_reservation_paths",
        "release_file_reservations",
        "renew_file_reservations",
        "force_release_file_reservation",
        "install_precommit_guard",
        "uninstall_precommit_guard",
        "search_messages",
        "summarize_thread",
        "macro_start_session",
        "macro_prepare_thread",
        "macro_file_reservation_cycle",
        "macro_contact_handshake",
        "ensure_product",
        "products_link",
        "search_messages_product",
        "fetch_inbox_product",
        "summarize_thread_product",
        "acquire_build_slot",
        "renew_build_slot",
        "release_build_slot",
    ]
    .iter()
    .map(ToString::to_string)
    .collect()
}

#[test]
fn lease_record_parses_full_shape() {
    let record = full_record();
    assert_eq!(record.bead_id, "omp-orchestrator-3w9l");
    assert_eq!(record.holder_mail, "AmberGate");
    assert_eq!(record.ack_name.as_deref(), Some("RubyGate"));
    assert_eq!(record.paths, &["src/lib.rs", "src/main.rs"]);
}

#[test]
fn lease_record_ack_is_optional() {
    let records =
        parse_lease_records("LEASE bead=b holder=H paths=a/b.rs").expect("ack optional");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].ack_name, None);
}

#[test]
fn lease_record_malformed_lines_are_errors_never_skips() {
    for bad in [
        "LEASE",
        "LEASE bead=b holder=H",
        "LEASE bead=b paths=a.rs",
        "LEASE holder=H paths=a.rs",
        "LEASE bead=b holder=H paths=",
        "LEASE bead=b holder=H paths=a.rs bogus=1",
        "LEASE bead=b bead=c holder=H paths=a.rs",
        "LEASE beadb holder=H paths=a.rs",
    ] {
        assert!(
            parse_lease_records(bad).is_err(),
            "must refuse, never skip: {bad:?}"
        );
    }
}

#[test]
fn lease_record_non_lease_text_is_ignored() {
    let records = parse_lease_records(
        "Some prose\nACK abc on %1 -- working\nLEASE bead=b holder=H paths=a.rs\nMore prose",
    )
    .expect("prose ignored");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].bead_id, "b");
}

#[test]
fn match_finds_only_holder_exclusive_on_record_paths() {
    let held = match_held_leases(&full_record(), &held_report());
    assert_eq!(held.len(), 2);
    assert!(held.iter().all(|lease| lease.exclusive));
    assert!(held.iter().all(|lease| lease.holder_mail == "AmberGate"));
    assert_eq!(held[0].ack_name.as_deref(), Some("RubyGate"));
}

#[test]
fn match_ignores_other_holder_non_exclusive_and_off_path_rows() {
    let report = agent_mail_native::journey::parse_authoritative_reservation_report(&serde_json::json!({
        "conflict_free": false,
        "conflicts": [
            {"agent": "SomeoneElse", "path": "src/lib.rs", "exclusive": true},
            {"agent": "AmberGate", "path": "src/lib.rs", "exclusive": false},
            {"agent": "AmberGate", "path": "elsewhere.rs", "exclusive": true}
        ],
        "total_conflicting_reservations": 3,
        "clear_paths": [],
        "authoritative_source": "database_snapshot"
    }))
    .expect("mixed report parses");
    assert!(
        match_held_leases(&full_record(), &report).is_empty(),
        "another holder, a shared lock, and an off-record path must not refuse this bead's close"
    );
}

#[test]
fn refusal_names_mail_holder_ack_and_paths() {
    let text = lease_refusal_text("omp-orchestrator-3w9l", &match_held_leases(&full_record(), &held_report()))
        .expect("non-empty held set refuses");
    assert!(text.contains("lease-guard: REFUSED"), "got {text}");
    assert!(text.contains("bead=omp-orchestrator-3w9l"), "got {text}");
    assert!(text.contains("AmberGate"), "mail holder named: {text}");
    assert!(text.contains("ack=RubyGate"), "ACK join printed: {text}");
    assert!(text.contains("src/lib.rs"), "paths named: {text}");
    assert!(text.contains("release_file_reservations"), "release path named: {text}");
}

#[test]
fn refusal_over_empty_set_is_none() {
    assert!(
        lease_refusal_text("b", &[]).is_none(),
        "no refusal text may exist over an empty held set"
    );
}

#[test]
fn recorded_catalogue_has_no_lease_listing() {
    let catalogue = recorded_catalogue_45();
    assert_eq!(catalogue.len(), 45, "fixture must stay the full recorded set");
    assert!(
        !catalogue_has_lease_listing(&catalogue),
        "the take/query/release verbs are not enumeration; ABSENT stands"
    );
}

#[test]
fn catalogue_matcher_names_a_listing_tool_when_one_exists() {
    for name in [
        "list_file_reservations",
        "enumerate_reservation_leases",
        "all_reservations_list",
    ] {
        assert!(
            catalogue_has_lease_listing(&[name.to_owned()]),
            "a real listing verb must flip the matcher: {name}"
        );
    }
}

#[test]
fn no_lease_enumeration_is_typed_absent() {
    let error = MailError::NoLeaseEnumeration { catalogue_tools: 45 };
    assert!(
        error.is_inconclusive_about_mail(),
        "ABSENT must never read as clear"
    );
    let rendered = error.to_string();
    assert!(rendered.contains("NO_LEASE_ENUMERATION"), "got {rendered}");
    assert!(rendered.contains("45"), "catalogue size carried: {rendered}");
}

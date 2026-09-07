//! Production scan is a shrinking ceiling. Detection is proven on a planted row.
//!
//! C38 still holds for the production count: the ceiling is measured from
//! `.beads/issues.jsonl`, not from the 23-row bead prose. The fixture below
//! only proves the detector fires.

use grader_attribution_gate::{
    actor_provenance_gate_exit, actor_provenance_violations, attribution_hits, ledger_gate_exit,
    parse_actor_provenance, parse_closed_beads, unattributed_close_ids, CeilingVerdict,
    ACTOR_PROVENANCE_CUTOFF, ACTOR_PROVENANCE_CUTOFF_REASON, ACTOR_PROVENANCE_EXIT_EMPTY,
    ACTOR_PROVENANCE_EXIT_INVALID, ACTOR_PROVENANCE_EXIT_OK, ACTOR_PROVENANCE_EXIT_VIOLATION,
    DEFAULT_AUTHORS, UNATTRIBUTED_CLOSE_CEILING,
};
use std::path::PathBuf;

fn production_jsonl() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.beads/issues.jsonl");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("production ledger unreadable at {}: {e}", path.display()))
}

const ACTOR_FIXTURE_SOURCE: &str = ".beads/issues.jsonl";
const ACTOR_FIXTURE_SOURCE_SHA256: &str = "1f48b592328672749f4fae723738ca729ce4d3a5a65aed06aecbf3f359c3423d";
const ACTOR_FIXTURE_CAPTURED_AT: &str = "2026-09-07T19:55:44Z";

fn actor_fixture_jsonl() -> &'static str {
    include_str!("fixtures/actor_provenance.jsonl")
}

fn actor_fixture_rows() -> Vec<grader_attribution_gate::ActorProvenanceRow> {
    parse_actor_provenance(actor_fixture_jsonl()).expect("committed actor fixture parses")
}


const PLANTED_UNATTRIBUTED: &str = r#"{"id":"omp-orchestrator-planted-unattributed-gcyf","status":"closed","closed_at":"2026-09-06T00:00:00Z","close_reason":"MUTATION-VERIFIED by SnowyCanyon -- a NON-IMPLEMENTER","comments":[{"author":"josh","text":"closed without --actor"}]}"#;

#[test]
fn empty_ledger_is_an_error() {
    let error = parse_closed_beads("").expect_err("empty scan must not pass");
    assert!(error.to_string().contains("ATTRIBUTION_SCAN_EMPTY"));
    let error = parse_closed_beads("{\"status\":\"open\",\"id\":\"x\"}\n")
        .expect_err("zero closed rows must not pass");
    assert!(error.to_string().contains("ATTRIBUTION_SCAN_EMPTY"));
}

#[test]
fn detector_fires_on_a_planted_unattributed_close() {
    let rows = parse_closed_beads(PLANTED_UNATTRIBUTED).expect("planted closed row");
    let unattributed = unattributed_close_ids(&rows, DEFAULT_AUTHORS);
    assert_eq!(
        unattributed,
        vec!["omp-orchestrator-planted-unattributed-gcyf".to_owned()],
        "FIXTURE_PROVES_DETECTION: a josh-only close must be named"
    );
}

#[test]
#[ignore = "requires live .beads ledger; the gate binary is the opt-in caller"]
fn production_live_count_must_not_exceed_the_ceiling() {
    let rows = parse_closed_beads(&production_jsonl()).expect("production has closed beads");
    let unattributed = unattributed_close_ids(&rows, DEFAULT_AUTHORS);
    let live = unattributed.len();
    let verdict = CeilingVerdict::from_counts(live, UNATTRIBUTED_CLOSE_CEILING);
    assert!(
        !verdict.refuses(),
        "CEILING_BREACHED live={live} ceiling={UNATTRIBUTED_CLOSE_CEILING} ids={unattributed:?}"
    );
    assert_eq!(ledger_gate_exit(&unattributed), 0);
}

#[test]
fn a_clean_ledger_stays_green() {
    assert_eq!(ledger_gate_exit(&[]), 0);
    assert!(!CeilingVerdict::from_counts(0, UNATTRIBUTED_CLOSE_CEILING).refuses());
}

#[test]
#[ignore = "requires live .beads ledger; the gate binary is the opt-in caller"]
fn iis6_close_is_not_unattributed() {
    let rows = parse_closed_beads(&production_jsonl()).expect("production has closed beads");
    let unattributed = unattributed_close_ids(&rows, DEFAULT_AUTHORS);
    assert!(
        !unattributed.iter().any(|id| id == "omp-orchestrator-iis6"),
        "KNOWN-GOOD: iis6 carries WildStone --actor and must PASS"
    );
}

#[test]
#[ignore = "requires live .beads ledger; the gate binary is the opt-in caller"]
fn leht_and_mj8w_have_attribution_hits() {
    let rows = parse_closed_beads(&production_jsonl()).expect("production has closed beads");
    let leht = rows
        .iter()
        .find(|row| row.id == "omp-orchestrator-leht")
        .expect("POSITIVE CONTROL leht must exist in the ledger");
    let mj8w = rows
        .iter()
        .find(|row| row.id == "omp-orchestrator-mj8w")
        .expect("POSITIVE CONTROL mj8w must exist in the ledger");
    let leht_hits = attribution_hits(&leht.comment_authors, &leht.close_reason, DEFAULT_AUTHORS);
    let mj8w_hits = attribution_hits(&mj8w.comment_authors, &mj8w.close_reason, DEFAULT_AUTHORS);
    assert!(
        leht_hits.iter().any(|h| h == "GreenFrog"),
        "POSITIVE CONTROL leht: {leht_hits:?}"
    );
    assert!(
        mj8w_hits.iter().any(|h| h == "AmberGate") || mj8w_hits.iter().any(|h| h == "BlueLantern"),
        "POSITIVE CONTROL mj8w: {mj8w_hits:?}"
    );
}
#[test]
fn cutoff_is_a_source_contract_with_a_reason() {
    assert_eq!(ACTOR_PROVENANCE_CUTOFF, "2026-09-07T18:42:29Z");
    assert!(
        ACTOR_PROVENANCE_CUTOFF_REASON.contains("not retro-attributed"),
        "the legacy-data boundary needs its reason"
    );
}

#[test]
fn post_cutoff_default_author_fires_and_names_id_and_field() {
    let rows = actor_fixture_rows();
    let violations = actor_provenance_violations(&rows).expect("non-empty scan");
    assert_eq!(actor_provenance_gate_exit(&violations), ACTOR_PROVENANCE_EXIT_VIOLATION);
    assert_eq!(violations.len(), 1);
    let message = violations[0].to_string();
    assert!(message.contains("bead=omp-orchestrator-f02x"), "{message}");
    assert!(message.contains("field=created_by"), "{message}");
    assert!(message.contains("code=ACTOR_PROVENANCE_VIOLATION"), "{message}");
}

#[test]
fn future_agent_and_legacy_josh_are_both_allowed_in_their_scopes() {
    let rows: Vec<_> = actor_fixture_rows()
        .into_iter()
        .filter(|row| row.id != "omp-orchestrator-f02x")
        .collect();
    let violations = actor_provenance_violations(&rows).expect("non-empty scan");
    assert!(violations.is_empty(), "legacy josh and future agent must pass: {violations:?}");
    assert_eq!(actor_provenance_gate_exit(&violations), ACTOR_PROVENANCE_EXIT_OK);
}

#[test]
fn actor_fixture_records_source_hash_and_capture_time() {
    assert_eq!(ACTOR_FIXTURE_SOURCE, ".beads/issues.jsonl");
    assert_eq!(
        ACTOR_FIXTURE_SOURCE_SHA256,
        "1f48b592328672749f4fae723738ca729ce4d3a5a65aed06aecbf3f359c3423d"
    );
    assert_eq!(ACTOR_FIXTURE_CAPTURED_AT, "2026-09-07T19:55:44Z");
    assert_eq!(actor_fixture_rows().len(), 3);
}

#[test]
fn missing_future_actor_fires_with_a_missing_value() {
    let jsonl = r#"{"id":"omp-orchestrator-missing-actor-0luy","created_at":"2026-09-07T18:42:30Z"}"#;
    let rows = parse_actor_provenance(jsonl).expect("missing actor is a ledger row");
    let violations = actor_provenance_violations(&rows).expect("non-empty scan");
    let message = violations[0].to_string();
    assert!(message.contains("bead=omp-orchestrator-missing-actor-0luy"), "{message}");
    assert!(message.contains("field=created_by"), "{message}");
    assert!(message.contains("value=<missing>"), "{message}");
}

#[test]
fn empty_or_malformed_actor_scan_is_a_typed_nonzero_error() {
    let empty = parse_actor_provenance("\n").expect_err("empty actor scan must not pass");
    assert_eq!(empty.exit_code(), ACTOR_PROVENANCE_EXIT_EMPTY);
    assert!(empty.to_string().contains("code=ACTOR_PROVENANCE_EMPTY"));

    let malformed = parse_actor_provenance("not-json\n").expect_err("malformed ledger must refuse");
    assert_eq!(malformed.exit_code(), ACTOR_PROVENANCE_EXIT_INVALID);
    assert!(malformed.to_string().contains("code=ACTOR_PROVENANCE_MALFORMED"));
}

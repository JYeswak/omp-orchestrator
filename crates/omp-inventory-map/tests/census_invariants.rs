//! Census invariant anti-vacuity legs (bead omp-orchestrator-plan-02-lwdo.1).
//!
//! Known-bad: INV-2026-08-31 shape (n=183, distinct=1).
//! Known-good: one distinct invariant set per row kind, crate row names itself.
//! Empty: ERROR, never pass.
//! Structural: vacuity_mode=structural with a reason is accepted.

#![forbid(unsafe_code)]

use omp_inventory_map::census_invariants::{
    CensusInvariantError, CensusInvariantRow, ROW_KIND_COUNT, ROW_KINDS, VacuityMode,
    check_census_invariants, invariants_for_kind,
};
use omp_inventory_map::{InventoryInputs, InventoryRow, ProbeState, build_inventory_map};
use serde_json::json;

fn metadata(names: &[&str]) -> String {
    json!({
        "workspace_root": "/fixture/workspace",
        "packages": names.iter().map(|name| json!({
            "name": name,
            "version": "0.1.0",
            "manifest_path": format!("/fixture/workspace/crates/{name}/Cargo.toml"),
            "targets": [{"name": name.replace('-', "_"), "kind": ["lib"]}],
            "dependencies": []
        })).collect::<Vec<_>>()
    })
    .to_string()
}

fn row(
    id: &str,
    kind: &str,
    must_be_true: Vec<String>,
    negative_evidence: Vec<String>,
    vacuity_mode: Option<VacuityMode>,
    vacuity_reason: Option<String>,
) -> CensusInvariantRow {
    CensusInvariantRow {
        id: id.to_owned(),
        kind: kind.to_owned(),
        must_be_true,
        negative_evidence,
        vacuity_mode,
        vacuity_reason,
    }
}

/// Retained INV-2026-08-31 shape: 183 rows, one must_be_true, one negative_evidence.
fn inv_2026_08_31() -> Vec<CensusInvariantRow> {
    const TEMPLATE_TRUE: &str =
        "The source probe is non-empty before a known verdict is emitted.";
    const TEMPLATE_NEG: &str =
        "No repository source grep was used; ownership is derived from metadata and direct probes.";
    (0..183)
        .map(|index| {
            row(
                &format!("surface:cli_command:{index}"),
                "cli_command",
                vec![TEMPLATE_TRUE.to_owned()],
                vec![TEMPLATE_NEG.to_owned()],
                None,
                None,
            )
        })
        .collect()
}

fn diverse_fixture() -> Vec<CensusInvariantRow> {
    let mut rows = Vec::new();
    for kind in ROW_KINDS {
        let identity = if kind == "workspace_crate" {
            "ack-spine"
        } else {
            kind
        };
        let (must_be_true, negative_evidence) = invariants_for_kind(kind, identity);
        let id = if kind == "workspace_crate" {
            "crate:ack-spine".to_owned()
        } else {
            format!("surface:{kind}:{kind}")
        };
        rows.push(row(
            &id,
            kind,
            must_be_true,
            negative_evidence,
            None,
            None,
        ));
    }
    rows
}

#[test]
fn census_invariants_are_row_specific() {
    let known_bad = inv_2026_08_31();
    assert_eq!(known_bad.len(), 183);
    match check_census_invariants(&known_bad) {
        Err(CensusInvariantError::VacuousInvariantSet {
            field,
            n,
            distinct,
            repeated,
            ..
        }) => {
            assert_eq!(field, "must_be_true");
            assert_eq!(n, 183);
            assert_eq!(distinct, 1);
            assert!(
                repeated.contains("The source probe is non-empty"),
                "repeated={repeated}"
            );
        }
        other => panic!("INV-2026-08-31 must be RED, got {other:?}"),
    }

    let good = diverse_fixture();
    assert_eq!(good.len(), ROW_KIND_COUNT);
    check_census_invariants(&good).expect("per-kind fixture must be GREEN");
    let crate_row = good
        .iter()
        .find(|row| row.kind == "workspace_crate")
        .expect("crate row");
    let cli_row = good
        .iter()
        .find(|row| row.kind == "cli_command")
        .expect("cli row");
    assert_ne!(
        crate_row.must_be_true, cli_row.must_be_true,
        "a per-crate row must differ from a cli_command row"
    );

    match check_census_invariants(&[]) {
        Err(CensusInvariantError::EmptyCensus) => {}
        other => panic!("empty census must be ERROR, got {other:?}"),
    }
}

#[test]
fn census_invariants_structural_vacuity_is_carried() {
    let mut rows = diverse_fixture();
    rows.push(row(
        "surface:cli_command:alpha",
        "cli_command",
        vec!["cli_command rows are enumerated from the omp --help COMMANDS block".to_owned()],
        vec!["a missing COMMANDS block is UNKNOWN, not a healthy zero-command census".to_owned()],
        Some(VacuityMode::Structural),
        Some("COMMANDS-block members share one probe, not per-command contracts".to_owned()),
    ));
    check_census_invariants(&rows)
        .expect("structural vacuity with a reason is accepted");

    let mut missing_reason = diverse_fixture();
    missing_reason.push(row(
        "surface:cli_command:beta",
        "cli_command",
        vec!["x".to_owned()],
        vec!["y".to_owned()],
        Some(VacuityMode::Structural),
        Some("   ".to_owned()),
    ));
    match check_census_invariants(&missing_reason) {
        Err(CensusInvariantError::StructuralVacuityMissingReason { id }) => {
            assert_eq!(id, "surface:cli_command:beta");
        }
        other => panic!("structural without reason must fail, got {other:?}"),
    }
}

#[test]
fn census_invariants_emitted_map_is_kind_specific() {
    let map = build_inventory_map(InventoryInputs {
        cargo_metadata: metadata(&["ack-spine", "omp-inventory-map"]),
        cli_help: Some(
            "omp v18.0.11\n\nCOMMANDS\n  alpha       positive control\n  beta        another\n\nEXAMPLES\n"
                .to_owned(),
        ),
        type_roots: Some(vec!["jsonrpc".to_owned()]),
        declarations: Some(vec!["index.d.ts".to_owned()]),
        rpc_handlers: Some(vec!["prompt".to_owned()]),
        slash_commands: Some(vec!["compact".to_owned()]),
        omp_methods: Some(vec!["omp/muxConnect".to_owned()]),
        transport_modes: Some(vec!["rpc".to_owned()]),
        omp_version: Some("omp/18.0.11".to_owned()),
        ..InventoryInputs::default()
    })
    .expect("fixture map");

    let views: Vec<CensusInvariantRow> = map
        .rows
        .iter()
        .map(|row| CensusInvariantRow {
            id: row.id.clone(),
            kind: row.kind.clone(),
            must_be_true: row.must_be_true.clone(),
            negative_evidence: row.negative_evidence.clone(),
            vacuity_mode: row.vacuity_mode,
            vacuity_reason: row.vacuity_reason.clone(),
        })
        .collect();

    let kinds: std::collections::BTreeSet<_> =
        views.iter().map(|row| row.kind.as_str()).collect();
    assert!(
        kinds.len() >= ROW_KIND_COUNT,
        "emitted kinds={kinds:?} count={}",
        kinds.len()
    );

    let crate_row = views
        .iter()
        .find(|row| row.id == "crate:ack-spine")
        .expect("crate:ack-spine");
    assert!(
        crate_row
            .must_be_true
            .iter()
            .any(|item| item.contains("ack-spine")),
        "crate row must name its identifier: {:?}",
        crate_row.must_be_true
    );
    let cli_row = views
        .iter()
        .find(|row| row.kind == "cli_command")
        .expect("cli_command");
    assert_ne!(crate_row.must_be_true, cli_row.must_be_true);

    let mut must = std::collections::BTreeSet::new();
    let mut neg = std::collections::BTreeSet::new();
    for row in &views {
        must.insert(serde_json::to_string(&row.must_be_true).unwrap());
        neg.insert(serde_json::to_string(&row.negative_evidence).unwrap());
    }
    assert!(
        must.len() >= ROW_KIND_COUNT,
        "distinct must_be_true={} < {ROW_KIND_COUNT}",
        must.len()
    );
    assert!(
        neg.len() >= ROW_KIND_COUNT,
        "distinct negative_evidence={} < {ROW_KIND_COUNT}",
        neg.len()
    );

    // Two cli_command rows share the kind template → partitioned vacuity.
    match check_census_invariants(&views) {
        Err(CensusInvariantError::VacuousInvariantSet { kind, n, .. }) => {
            assert_eq!(kind, "cli_command");
            assert!(n >= 2, "n={n}");
        }
        other => panic!("kind-template cli_command rows must still be VACUOUS, got {other:?}"),
    }

    assert_eq!(map.rows.iter().find(|row| row.id == "crate:ack-spine").map(|row| row.status), Some(ProbeState::Known));
}

#[test]
fn census_invariants_structural_mode_is_in_the_envelope() {
    let row = InventoryRow {
        id: "surface:cli_command:alpha".to_owned(),
        surface: "cli_command:alpha".to_owned(),
        kind: "cli_command".to_owned(),
        what_it_provides: "alpha".to_owned(),
        provides: "alpha".to_owned(),
        crate_consumes_today: "NONE".to_owned(),
        crate_should_own: "NONE".to_owned(),
        inputs: vec![],
        outputs: vec![],
        must_be_true: vec!["cli_command rows are enumerated from the omp --help COMMANDS block".to_owned()],
        negative_evidence: vec!["a missing COMMANDS block is UNKNOWN, not a healthy zero-command census".to_owned()],
        classification: "CAPABILITY_NOT_USED".to_owned(),
        status: ProbeState::Known,
        map_to_none_reason: None,
        orphan_disposition: "NAMED_REASON".to_owned(),
        orphan_reason: "fixture".to_owned(),
        vacuity_mode: Some(VacuityMode::Structural),
        vacuity_reason: Some("COMMANDS-block members share one probe".to_owned()),
    };
    let encoded = serde_json::to_value(&row).expect("row serializes");
    assert_eq!(encoded["vacuity_mode"], "structural");
    assert_eq!(encoded["vacuity_reason"], "COMMANDS-block members share one probe");
    let decoded: InventoryRow = serde_json::from_value(encoded).expect("row deserializes");
    assert_eq!(decoded.vacuity_mode, Some(VacuityMode::Structural));
    let view = CensusInvariantRow {
        id: decoded.id.clone(),
        kind: decoded.kind.clone(),
        must_be_true: decoded.must_be_true.clone(),
        negative_evidence: decoded.negative_evidence.clone(),
        vacuity_mode: decoded.vacuity_mode,
        vacuity_reason: decoded.vacuity_reason.clone(),
    };
    check_census_invariants(&[view]).expect("structural row with reason is accepted");
}

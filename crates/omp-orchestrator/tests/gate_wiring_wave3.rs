//! Wave OMP-COVERAGE-WIRING: every WIRE output from the authoritative surface
//! map must be visible to the supervisor census with a live trigger.
//!
//! NO-CLAIM: this proves trigger registration and supervisor reachability only.
//! It does not prove that any coverage wave's implementation is correct.

use omp_orchestrator::{census_gates, GateReachability, COVERAGE_WAVE_OUTPUT_CRATES};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;

const SURFACE_MAP: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/plan/SURFACE-MAP.jsonl"
));

fn wire_output_crates() -> BTreeSet<String> {
    let mut crates = BTreeSet::new();
    for (line_number, line) in SURFACE_MAP.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap_or_else(|error| {
            panic!(
                "surface map line {} is invalid JSON: {error}",
                line_number + 1
            )
        });
        if row.get("disposition").and_then(Value::as_str) != Some("WIRE") {
            continue;
        }
        if let Some(crate_name) = row.get("maps_to_crate").and_then(Value::as_str) {
            assert!(
                !crate_name.trim().is_empty(),
                "WIRE row has an empty output crate"
            );
            crates.insert(crate_name.to_owned());
        }
    }
    crates
}

#[test]
fn surface_map_has_the_eleven_wave_outputs() {
    let outputs = wire_output_crates();
    assert_eq!(
        outputs.len(),
        11,
        "coverage output scan is stale or vacuous: {outputs:?}"
    );
    let declared = COVERAGE_WAVE_OUTPUT_CRATES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        declared,
        outputs.iter().map(String::as_str).collect(),
        "production census registry diverged from the authoritative map"
    );
}

#[test]
fn every_wave_output_has_a_reachable_census_trigger() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let census = census_gates(repo_root);

    for output_crate in wire_output_crates() {
        let row = census
            .rows
            .iter()
            .find(|row| row.gate == output_crate)
            .unwrap_or_else(|| panic!("coverage output {output_crate} has no census row"));
        match &row.reachability {
            GateReachability::Reachable { trigger } if !trigger.trim().is_empty() => {}
            other => panic!("coverage output {output_crate} has no reachable trigger: {other:?}"),
        }
    }
}
#[test]
fn an_unreachable_wave_output_blocks_supervisor_decision() {
    let observation = omp_orchestrator::Observation {
        panes: Vec::new(),
        queue: omp_orchestrator::QueueState {
            ready_count: 0,
            readable: true,
        },
        gate_census: Some(omp_orchestrator::GateCensus {
            rows: vec![
                omp_orchestrator::GateCensusRow {
                    gate: "no-shell-gate".to_owned(),
                    reachability: GateReachability::Reachable {
                        trigger: "test-positive-control".to_owned(),
                    },
                },
                omp_orchestrator::GateCensusRow {
                    gate: "finding-dispatch".to_owned(),
                    reachability: GateReachability::NotInstalled,
                },
            ],
        }),
    };

    let decision = omp_orchestrator::decide(
        &observation,
        &omp_orchestrator::IdleAuthorization::Unauthorized { why: "test" },
    );
    match decision {
        omp_orchestrator::SupervisorDecision::GateUnwired { unwired } => {
            assert!(unwired.iter().any(|gate| gate == "finding-dispatch"));
        }
        other => panic!("unreachable coverage output bypassed decision gate: {other:?}"),
    }
}

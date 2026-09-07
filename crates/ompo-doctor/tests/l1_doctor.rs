#![forbid(unsafe_code)]

//! Named L1 target for the S1 layer gate.
//!
//! The target exercises the real `ompo-doctor` library and umbrella binary rather than a
//! manifest-only count. The gate invokes this file directly with Cargo's `--test l1_doctor`.

use ompo_doctor::{lifecycle_events, ProbeDecision, ARTIFACT_REFERENCE, PROBES};
use std::collections::BTreeSet;
use std::process::Command;

fn fixture_decisions() -> Vec<ProbeDecision> {
    PROBES
        .iter()
        .map(|probe| ProbeDecision {
            name: probe.name.to_owned(),
            status: "OK".to_owned(),
            reason_code: format!("L1_PROBE_{}_OK", probe.name.replace('-', "_")),
            detail: "named-target fixture".to_owned(),
        })
        .collect()
}

#[test]
fn l1_named_target_reaches_declared_probe_surface() {
    assert_eq!(PROBES.len(), 11, "the L1 probe denominator must be non-empty");
    let names: BTreeSet<&str> = PROBES.iter().map(|probe| probe.name).collect();
    assert_eq!(names.len(), PROBES.len(), "probe names must be unique");
    assert!(PROBES
        .iter()
        .all(|probe| !probe.command.is_empty() && !probe.args.is_empty()));
}

#[test]
fn l1_named_target_requires_one_lifecycle_event_per_probe() {
    let decisions = fixture_decisions();
    let events = lifecycle_events(PROBES, &decisions).expect("complete probe set emits events");
    assert_eq!(events.len(), PROBES.len());
    for (event, probe) in events.iter().zip(PROBES) {
        let value: serde_json::Value =
            serde_json::from_str(&event.to_json_line()).expect("event is JSON");
        assert_eq!(value["step"], probe.name);
        assert_eq!(value["status"], "OK");
        assert_eq!(value["next_command"], format!("readback={ARTIFACT_REFERENCE}"));
    }

    let missing = lifecycle_events(PROBES, &decisions[..decisions.len() - 1])
        .expect_err("a missing probe event must refuse");
    assert!(missing
        .to_string()
        .contains("L1_DOCTOR_MISSING_LIFECYCLE_EVENT"));
}

#[test]
fn l1_named_target_exercises_real_umbrella_capabilities() {
    let output = Command::new(env!("CARGO_BIN_EXE_ompo"))
        .args(["capabilities", "--json"])
        .output()
        .expect("the real ompo binary must run");
    assert!(output.status.success(), "capabilities failed: {:?}", output.status);
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_eq!(value["schema_version"], "omp.umbrella/v1");
    assert_eq!(value["status"], "OK");
    assert_eq!(
        value["data"]["probe_ids"].as_array().map(Vec::len),
        Some(PROBES.len())
    );
    assert!(value["data"]["adapters"]
        .as_array()
        .is_some_and(|adapters| !adapters.is_empty()));
}

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

/// The `run_doctor` envelope contract (25u5): required keys exist, remediation is
/// nonempty exactly when a probe is non-OK with one explicit rerun row per
/// non-OK probe, and `next_action` is the report readback -- never a placeholder.
/// Runs the production probes against an isolated fixture repo, so the probe mix
/// varies by machine and every assertion below is mix-invariant.
#[test]
fn doctor_run_reports_complete_envelope_with_typed_remediation() {
    let repo = tempfile::tempdir().expect("doctor fixture repo");
    let summary = ompo_doctor::run_doctor(repo.path(), "system").expect("doctor runs");
    assert_eq!(summary.schema, "ompo.doctor.v1");
    assert!(!summary.run_id.is_empty(), "run_id must be nonempty");
    assert_eq!(summary.scope, "system", "requested scope is preserved");
    assert_eq!(summary.probe_count, summary.probes.len());
    assert_eq!(summary.artifact, ompo_doctor::ARTIFACT_REFERENCE);
    assert_eq!(
        summary.next_action,
        format!("readback={}", ompo_doctor::ARTIFACT_REFERENCE),
        "next_action must be the report readback"
    );
    assert!(
        summary.lifecycle_journal.exists(),
        "journal must exist: {}",
        summary.lifecycle_journal.display()
    );
    let non_ok: Vec<_> = summary
        .probes
        .iter()
        .filter(|probe| probe.status != "OK")
        .collect();
    assert_eq!(
        !summary.remediation.is_empty(),
        !non_ok.is_empty(),
        "remediation is nonempty exactly when a probe is non-OK"
    );
    assert_eq!(summary.remediation.len(), non_ok.len());
    for probe in non_ok {
        assert!(
            summary
                .remediation
                .iter()
                .any(|row| row.contains("rerun") && row.contains(&probe.name)),
            "non-OK probe {} lacks an explicit rerun row: {:?}",
            probe.name,
            summary.remediation
        );
    }
}

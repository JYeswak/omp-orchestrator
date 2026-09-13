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
            presence: Some(format!("/fixture/{}", probe.command)),
            version: Some("fixture-version".to_owned()),
        })
        .collect()
}

#[test]
fn l1_named_target_reaches_declared_probe_surface() {
    assert!(!PROBES.is_empty(), "the L1 probe denominator must be non-empty");
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
    assert!(matches!(summary.exit_code, 0 | 1));
    assert_eq!(
        summary.status,
        if summary.exit_code == 0 { "OK" } else { "DEGRADED" }
    );
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

#[test]
fn doctor_command_reports_two_signals_and_exit_band() {
    let repo = tempfile::tempdir().expect("doctor fixture repo");
    let output = Command::new(env!("CARGO_BIN_EXE_ompo"))
        .args([
            "doctor",
            "--json",
            "--scope",
            "system",
            "--repo",
            repo.path().to_str().expect("fixture path is UTF-8"),
        ])
        .output()
        .expect("doctor command runs");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("doctor emits JSON even for a degraded subject");
    assert_eq!(value["command"], "doctor");
    let data = &value["data"];
    let reported_exit = data["exit_code"].as_u64().expect("typed doctor exit code");
    assert!(matches!(reported_exit, 0 | 1));
    assert_eq!(output.status.code(), Some(reported_exit as i32));
    assert_eq!(
        data["status"],
        if reported_exit == 0 { "OK" } else { "DEGRADED" }
    );

    let probes = data["probes"].as_array().expect("probe rows");
    assert!(!probes.is_empty(), "empty probe output is not a doctor pass");
    for probe in probes {
        assert!(probe.get("presence").is_some(), "presence signal is required");
        assert!(probe.get("version").is_some(), "version signal is required");
        if probe["status"] == "OK" {
            assert!(probe["presence"].as_str().is_some_and(|value| !value.is_empty()));
            assert!(probe["version"].as_str().is_some_and(|value| !value.is_empty()));
        }
    }
}

/// L1-TEST-EXIT-LATTICE (contract s1_l1_doctor.md `L1-BUILD-EXIT`): healthy,
/// refusing, and unrun fixtures land in three distinct exit bands, and UNRUN
/// never reads as refusal. Uses the real `exit_code` mapping, not a copy of
/// its arms: a copy would agree with the subject by construction.
///
/// KNOWN-BAD: map the unmeasured arms to Refused (or delete them so unrun
/// reads all-live) and this leg fails: an unprobed tool reading exit 0 or 1
/// certifies what was never measured, which is the false-green this envelope
/// exists to prevent. Both the message and the exit are pinned: `cargo`
/// returns 101 for unrelated causes, so an exit code alone cannot tell which
/// band moved.
#[test]
fn exit_bands_match_verdicts() {
    use ompo_doctor::adapter_exec::{
        exit_code, AdapterStatus, AdapterVerdict, EXIT_ALL_LIVE, EXIT_DEGRADED,
        EXIT_UNMEASURABLE,
    };
    fn verdict(status: AdapterStatus) -> AdapterVerdict {
        AdapterVerdict {
            adapter: "lattice-fixture".to_owned(),
            status,
            resolved: None,
            exit: None,
            detail: "exit-lattice fixture".to_owned(),
        }
    }
    // Healthy band: every adapter live.
    assert_eq!(
        exit_code(&[verdict(AdapterStatus::Live)]).expect("healthy set codes"),
        EXIT_ALL_LIVE,
        "healthy verdicts must exit {EXIT_ALL_LIVE}"
    );
    // Refusing band: a live adapter that answers without a help contract.
    assert_eq!(
        exit_code(&[verdict(AdapterStatus::Live), verdict(AdapterStatus::NoHelpContract)])
            .expect("refusing set codes"),
        EXIT_DEGRADED,
        "a refusing verdict must exit {EXIT_DEGRADED}, never 0"
    );
    // Unrun band: foreign, absent, and killed adapters each read unmeasured,
    // never healthy and never refused.
    for status in [
        AdapterStatus::Foreign,
        AdapterStatus::NotInstalled,
        AdapterStatus::TimedOut,
    ] {
        let code = exit_code(&[verdict(status)]).expect("unrun set codes");
        assert_eq!(
            code, EXIT_UNMEASURABLE,
            "unrun verdict {status:?} must exit {EXIT_UNMEASURABLE}"
        );
        assert_ne!(
            code, EXIT_ALL_LIVE,
            "unrun verdict {status:?} must never read healthy"
        );
        assert_ne!(
            code, EXIT_DEGRADED,
            "unrun verdict {status:?} must never read as refusal"
        );
    }
    // Empty band: a verb that executed nothing errors, never passes.
    let error = exit_code(&[]).expect_err("empty set must refuse to code");
    assert!(
        error.contains("UAD_EXECUTE_EMPTY_SET"),
        "empty-set refusal must name its code: {error}"
    );
}

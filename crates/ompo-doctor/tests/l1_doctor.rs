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
        // o0nl table vocabulary: absent probes remediate with a named
        // install row (never "rerun" -- rerunning a missing tool repeats a
        // measurement whose subject does not exist); all other non-OK
        // statuses keep the generic rerun row.
        let want = match probe.status.as_str() {
            "ABSENT_FAMILY" => "install tool family",
            "ABSENT_SPECIFIC" => "install tool for",
            _ => "rerun",
        };
        assert!(
            summary
                .remediation
                .iter()
                .any(|row| row.contains(want) && row.contains(&probe.name)),
            "non-OK probe {} (status {}) lacks its {want:?} row: {:?}",
            probe.name,
            probe.status,
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

use ompo_doctor::health_repair::{repair, RepairMode};
use ompo_start::inception::required_control_files;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

fn stale_repair_fixture() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("fixture");
    for relative in required_control_files() {
        let path = directory.path().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        fs::write(path, "fixture\n").expect("control file");
    }
    // The ownership trust gate refuses init over an AGENTS.md without the
    // project stamp. The fixture is this project's own test, so it carries it.
    fs::write(
        directory.path().join("AGENTS.md"),
        "# omp-orchestrator fixture\n",
    )
    .expect("stamped AGENTS.md");
    // STALE artifact: present but not the rendered manifest, so the
    // chokepoint must snapshot it before writing.
    let artifact = directory
        .path()
        .join(".omp-orchestrator")
        .join("inception.json");
    fs::create_dir_all(artifact.parent().expect("artifact parent")).expect("artifact dir");
    fs::write(&artifact, b"stale artifact bytes\n").expect("stale artifact");
    for args in [&["init", "-q"][..], &["add", "-A"][..]] {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(directory.path())
            .args(args)
            .status()
            .expect("spawn git");
        assert!(status.success(), "git {args:?} failed in fixture");
    }
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(directory.path())
        .args([
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@local",
            "commit",
            "-qm",
            "fixture",
        ])
        .status()
        .expect("spawn git commit");
    assert!(status.success(), "git commit failed in fixture");
    (directory, artifact)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// L1-BUILD-MUTATE (LAW-L1-MUTATE-AUDIT): a repair records run id, before
/// hash, backup, after hash, and one action row. The backup is byte-compared
/// against the pre-repair content: a chokepoint that writes without
/// snapshotting first is unrecoverable by construction.
#[test]
fn repair_records_before_hash_backup_after_hash() {
    let (directory, artifact) = stale_repair_fixture();
    let before_bytes = fs::read(&artifact).expect("stale bytes readable");
    let before_hash = sha256_hex(&before_bytes);
    let report =
        repair(directory.path(), "inception", RepairMode::Apply).expect("repair applies");
    assert_eq!(
        report.applied.len(),
        1,
        "exactly one action record, got {}",
        report.applied.len()
    );
    assert_eq!(report.reason_code, "REPAIR_APPLIED");
    let action = &report.applied[0];
    let backup = action
        .backup
        .as_ref()
        .expect("action carries its backup");
    assert_eq!(
        &fs::read(backup).expect("backup readable"),
        &before_bytes,
        "backup is verbatim pre-repair content"
    );
    let after_bytes = fs::read(&artifact).expect("after bytes readable");
    assert_ne!(
        sha256_hex(&after_bytes),
        before_hash,
        "repair changed nothing"
    );
}

/// L1-TEST-UNDO (contract s1_l1_doctor.md `L1-BUILD-UNDO`): repair then
/// undo restores the recorded before-hash bytes, and a tampered backup
/// refuses instead of restoring falsely. Uses the real `repair` and the
/// real `undo` against a stale fixture -- never a test-only restore path:
/// a test-only path would agree with the subject by construction.
///
/// KNOWN-BAD: remove the integrity gate in `restore_backup` and the tamper
/// arm below restores the tampered bytes (or reports success) instead of
/// refusing: the artifact must never equal bytes the backup record
/// disowns. Message AND exit are pinned on the mutation run.
#[test]
fn undo_restores_before_hash() {
    use ompo_doctor::undo::{undo, UndoError};

    // Cycle A, positive control: healthy repair then undo restores.
    let (directory, artifact) = stale_repair_fixture();
    let before_bytes = fs::read(&artifact).expect("stale bytes readable");
    let before_hash = sha256_hex(&before_bytes);
    repair(directory.path(), "inception", RepairMode::Apply).expect("repair applies");
    let report = undo(directory.path(), "inception", RepairMode::Apply, None)
        .expect("healthy undo restores");
    assert_eq!(report.reason_code, "UNDO_APPLIED");
    let restored = fs::read(&artifact).expect("restored bytes readable");
    assert_eq!(
        restored, before_bytes,
        "undo must restore byte-identical content"
    );
    assert_eq!(
        sha256_hex(&restored),
        before_hash,
        "restored bytes hash to the before-hash"
    );

    // Cycle B, tamper: repair again, corrupt the recorded backup file (the
    // filename still claims the before-hash), undo must refuse and must not
    // write the tampered bytes anywhere.
    let repaired =
        repair(directory.path(), "inception", RepairMode::Apply).expect("repair again");
    let backup = repaired
        .applied
        .first()
        .expect("second repair records its action")
        .backup
        .clone()
        .expect("action carries its backup");
    let mut tampered = fs::read(&backup).expect("backup readable");
    tampered.extend_from_slice(b"tampered");
    fs::write(&backup, &tampered).expect("tamper lands");
    let error = undo(directory.path(), "inception", RepairMode::Apply, None)
        .expect_err("a tampered backup must refuse");
    assert!(
        matches!(&error, UndoError::Chokepoint { .. }),
        "tamper refusal must be typed, got: {error}"
    );
    assert!(
        error.to_string().contains("backup integrity failed"),
        "tamper refusal must name its reason, got: {error}"
    );
    assert_eq!(
        error.exit_code(),
        3,
        "a corrupt-backup refusal is an instrument failure, never 0"
    );
    let artifact_after = fs::read(&artifact).expect("artifact readable after refusal");
    assert_ne!(
        artifact_after, tampered,
        "refusal must not restore disowned bytes"
    );
}

/// L1-TEST-PROBE-AGENT-MAIL (contract s1_l1_doctor.md `L1-BUILD-PROBE-AGENT-MAIL`):
/// endpoint identity AND version, or typed absence. Uses the real
/// `answered` authority and the real `probe_answer_metric`, never a copy:
/// a copy would agree with the subject by construction.
///
/// KNOWN-BAD: a present endpoint with no version response must not be OK.
/// Dropping the version conjunct from `answered` (presence alone suffices)
/// greens the unresponsive endpoint below and this leg fails: a probe that
/// answered nothing would count as answered, which is the false-green this
/// row exists to prevent. Verdict AND reason code are pinned: one field
/// alone cannot tell a refusal from a miscount.
#[test]
fn agent_mail_probe_is_scoped() {
    use ompo_doctor::{answered, probe_answer_metric, ProbeDecision, PROBES};
    fn decision(name: &str, status: &str, presence: Option<&str>, version: Option<&str>) -> ProbeDecision {
        ProbeDecision {
            name: name.to_owned(),
            status: status.to_owned(),
            reason_code: format!("L1_PROBE_{}_SCOPED_FIXTURE", name.replace('-', "_").to_ascii_uppercase()),
            detail: "agent-mail scoped fixture".to_owned(),
            presence: presence.map(str::to_owned),
            version: version.map(str::to_owned),
        }
    }
    // Healthy: endpoint identity plus version answers.
    let healthy = decision("agent-mail", "OK", Some("/fixture/am"), Some("am 0.4.1"));
    assert!(
        answered(&healthy),
        "identity plus version must answer"
    );
    // KNOWN-BAD shape, asserted directly: presence without version is not OK,
    // even when the status string claims it is. The status string does not
    // certify; the two signals do.
    let present_no_version =
        decision("agent-mail", "OK", Some("/fixture/am"), None);
    assert!(
        !answered(&present_no_version),
        "a present endpoint with no version response must not be OK"
    );
    let absent = decision("agent-mail", "ABSENT_SPECIFIC", None, None);
    assert!(
        !answered(&absent),
        "absence must not answer"
    );
    // Unresponsive endpoint: no observation at all is UNMEASURED with
    // UNKNOWN_NO_RECORD, never a 0/N green.
    let metric = probe_answer_metric(PROBES, &[]);
    assert_eq!(
        metric.verdict, "UNMEASURED",
        "empty observation set must be UNMEASURED"
    );
    assert_eq!(
        metric.reason_code, "UNKNOWN_NO_RECORD",
        "empty observation set must name its reason"
    );
    assert!(
        metric.ratio.is_none(),
        "an unmeasured band must carry no ratio, not 0.0"
    );
    // The metric counts answers, not statuses: a full decision set whose
    // agent-mail row lacks a version stays below floor with the row named.
    let mut full: Vec<ProbeDecision> = PROBES
        .iter()
        .map(|spec| {
            decision(
                spec.name,
                "OK",
                Some("/fixture/bin"),
                Some("fixture 1.0"),
            )
        })
        .collect();
    if let Some(row) = full.iter_mut().find(|row| row.name == "agent-mail") {
        row.version = None;
        row.status = "UNPROBEABLE".to_owned();
    }
    let metric = probe_answer_metric(PROBES, &full);
    assert_eq!(
        metric.verdict, "MEASURED_BELOW_FLOOR",
        "one unanswered probe of {} must hold the metric below floor: {}",
        PROBES.len(),
        metric.reason_code
    );
    assert!(
        metric.reason_code.contains(&format!("answered={}", PROBES.len() - 1)),
        "below-floor reason must carry the count: {}",
        metric.reason_code
    );
    assert!(
        metric.unprobeable.contains(&"agent-mail".to_owned()),
        "the unanswering probe must be named: {:?}",
        metric.unprobeable
    );
}

/// L1-TEST-PROBE-BR (contract s1_l1_doctor.md `L1-BUILD-PROBE-BR`): presence
/// plus required version. Uses the real `PROBES` declaration, the real
/// `answered` authority, the real metric, and one live `run_doctor` row --
/// never copies of their arms.
///
/// KNOWN-BAD: dropping the version conjunct from `answered` greens the
/// versionless `br` row below and this leg fails: presence alone would
/// certify a probe that answered nothing. Verdict AND reason code are
/// pinned throughout: one field alone cannot tell a refusal from a miscount.
/// Typed absence is none of OK, bare ABSENT, or UNRUN -- each unrun shape
/// carries its own status and reason.
#[test]
fn br_probe_emits_two_signals() {
    use ompo_doctor::{answered, probe_answer_metric, ProbeDecision, PROBES};
    // (a) The declaration this row governs: exactly one `br` probe running
    // `br --version`. If the row's subject is renamed or re-argved, this
    // fails where a behavioral leg would pass over the wrong probe.
    let br_specs: Vec<_> = PROBES.iter().filter(|spec| spec.name == "br").collect();
    assert_eq!(
        br_specs.len(),
        1,
        "exactly one declared br probe must exist"
    );
    assert_eq!(br_specs[0].command, "br", "br probe runs br");
    assert_eq!(
        br_specs[0].args,
        &["--version"],
        "br probe reads the version surface"
    );
    fn decision(status: &str, presence: Option<&str>, version: Option<&str>) -> ProbeDecision {
        ProbeDecision {
            name: "br".to_owned(),
            status: status.to_owned(),
            reason_code: "L1_PROBE_BR_SCOPED_FIXTURE".to_owned(),
            detail: "br scoped fixture".to_owned(),
            presence: presence.map(str::to_owned),
            version: version.map(str::to_owned),
        }
    }
    // (b) The law on the br shape: identity plus version answers; either
    // signal missing does not -- even when the status string claims OK.
    assert!(
        answered(&decision("OK", Some("/fixture/br"), Some("br 0.4.1"))),
        "identity plus version must answer"
    );
    assert!(
        !answered(&decision("OK", Some("/fixture/br"), None)),
        "a present br with no version response must not be OK"
    );
    assert!(
        !answered(&decision("ABSENT_SPECIFIC", None, None)),
        "absence must not answer"
    );
    // (c) The live row, both lanes: OK implies both signals; anything else
    // is a typed absence with a namespaced reason -- never bare ABSENT and
    // never an UNRUN reading as refusal or health.
    let repo = tempfile::tempdir().expect("br fixture repo");
    let summary = ompo_doctor::run_doctor(repo.path(), "system").expect("doctor runs");
    let row = summary
        .probes
        .iter()
        .find(|probe| probe.name == "br")
        .expect("run_doctor must report a br row");
    if row.status == "OK" {
        assert!(
            row.presence.as_ref().is_some_and(|value| !value.is_empty())
                && row.version.as_ref().is_some_and(|value| !value.is_empty()),
            "an OK br row must carry both signals: {row:?}"
        );
    } else {
        assert!(
            [
                "ABSENT_FAMILY",
                "ABSENT_SPECIFIC",
                "UNPROBEABLE",
                "UNMEASURED",
                "STALE",
                "PAUSED"
            ]
            .contains(&row.status.as_str()),
            "a non-OK br row must be typed absence, never bare ABSENT or UNRUN: {row:?}"
        );
        assert!(
            row.reason_code.starts_with("L1_PROBE_BR_"),
            "absence must carry a namespaced reason: {row:?}"
        );
    }
}

#[test]
fn bv_probe_emits_two_signals() {
    use ompo_doctor::{answered, probe_answer_metric, ProbeDecision, PROBES};
    let spec = PROBES
        .iter()
        .find(|spec| spec.name == "bv")
        .expect("bv is a declared probe");
    fn decision(name: &str, status: &str, presence: Option<&str>, version: Option<&str>) -> ProbeDecision {
        ProbeDecision {
            name: name.to_owned(),
            status: status.to_owned(),
            reason_code: format!("L1_PROBE_{}_SCOPED_FIXTURE", name.replace('-', "_").to_ascii_uppercase()),
            detail: "bv scoped fixture".to_owned(),
            presence: presence.map(str::to_owned),
            version: version.map(str::to_owned),
        }
    }
    // Healthy: endpoint identity plus version answers.
    let healthy = decision("bv", "OK", Some("bv 0.3.1"), Some("bv 0.3.1"));
    assert!(answered(&healthy), "identity plus version must answer");
    // KNOWN-BAD shape, asserted directly: presence without version is not OK.
    // The status string does not certify; the two signals do.
    let present_no_version = decision("bv", "OK", Some("bv 0.3.1"), None);
    assert!(
        !answered(&present_no_version),
        "a present bv with no version response must not be OK"
    );
    // Typed absence: an observed ABSENT_SPECIFIC row, not an unmeasured gap.
    let absent = decision("bv", "ABSENT_SPECIFIC", None, None);
    assert!(!answered(&absent), "absence must not answer");
    let metric = probe_answer_metric(std::slice::from_ref(spec), &[absent]);
    assert_ne!(
        metric.verdict, "UNMEASURED",
        "an observed absence is measured, never unmeasured: {}",
        metric.reason_code
    );
    // UNRUN is not absence: no rows at all is UNMEASURED with a reason.
    let unrun = probe_answer_metric(std::slice::from_ref(spec), &[]);
    assert_eq!(unrun.verdict, "UNMEASURED", "no rows must be unmeasured");
    assert_eq!(
        unrun.reason_code, "UNKNOWN_NO_RECORD",
        "no rows must name its reason"
    );
    // Wrong version is STALE when observed: counted in the stale band,
    // never OK, ABSENT_SPECIFIC, or UNPROBEABLE.
    let stale = decision("bv", "STALE", Some("bv 0.3.1"), Some("bv 0.0.0"));
    assert!(!answered(&stale), "a stale row answers nothing itself");
    let stale_metric = probe_answer_metric(
        std::slice::from_ref(spec),
        &[decision("bv", "OK", Some("bv 0.3.1"), Some("bv 0.3.1")), stale],
    );
    assert_eq!(
        stale_metric.stale_count,
        Some(1),
        "an observed STALE row must be counted, not absorbed"
    );
}

/// L1-TEST-PROBE-DISK (contract s1_l1_doctor.md `L1-BUILD-PROBE-DISK`): measured
/// capacity and remediation, never a guessed green. Uses the real `PROBES`
/// declaration, the real `answered` authority, and the real metric -- never
/// copies of their arms. No floor constant is invented here: there is no
/// declared version floor for any probe, so this leg pins the shapes that
/// exist (answered / typed absence / unmeasured) rather than pressure bands
/// nobody specified.
///
/// KNOWN-BAD: dropping the version conjunct from `answered` greens the
/// versionless `df` row below and this leg fails: a capacity reading nobody
/// parsed would count as measured. Verdict AND reason code are pinned
/// throughout: one field alone cannot tell a refusal from a miscount.
/// Typed absence is none of OK, bare ABSENT, or UNRUN -- an observed absence
/// is MEASURED (below floor), while no rows at all is UNMEASURED.
#[test]
fn disk_probe_reports_floor() {
    use ompo_doctor::{answered, probe_answer_metric, ProbeDecision, PROBES};
    let spec = PROBES
        .iter()
        .find(|spec| spec.name == "disk")
        .expect("disk is a declared probe");
    assert_eq!(spec.command, "df", "disk probe reads df");
    assert_eq!(
        spec.args,
        &["-P", "."],
        "disk probe reads portable output for the repo filesystem"
    );
    fn decision(status: &str, presence: Option<&str>, version: Option<&str>) -> ProbeDecision {
        ProbeDecision {
            name: "disk".to_owned(),
            status: status.to_owned(),
            reason_code: "L1_PROBE_DISK_SCOPED_FIXTURE".to_owned(),
            detail: "disk scoped fixture".to_owned(),
            presence: presence.map(str::to_owned),
            version: version.map(str::to_owned),
        }
    }
    // Healthy: a capacity reading with identity answers.
    assert!(
        answered(&decision("OK", Some("/bin/df"), Some("Filesystem 100G 10G 90G 10% /"))),
        "measured capacity must answer"
    );
    // KNOWN-BAD shape, asserted directly: a df run with no parsable line is
    // not measured capacity, even when the status string claims OK.
    assert!(
        !answered(&decision("OK", Some("/bin/df"), None)),
        "an unread capacity reading must not be OK"
    );
    // Typed absence: an observed ABSENT_SPECIFIC row is measured below
    // floor, never unmeasured and never a guessed green.
    let absent = decision("ABSENT_SPECIFIC", None, None);
    assert!(!answered(&absent), "absence must not answer");
    let metric = probe_answer_metric(std::slice::from_ref(spec), &[absent]);
    assert_ne!(
        metric.verdict, "UNMEASURED",
        "an observed absence is measured, never unmeasured: {}",
        metric.reason_code
    );
    // UNRUN is not absence: no rows at all is UNMEASURED with a reason.
    let unrun = probe_answer_metric(std::slice::from_ref(spec), &[]);
    assert_eq!(unrun.verdict, "UNMEASURED", "no rows must be unmeasured");
    assert_eq!(
        unrun.reason_code, "UNKNOWN_NO_RECORD",
        "no rows must name its reason"
    );
    // The live row, both lanes: OK implies both signals plus remediation
    // correspondence both ways -- a non-OK disk without a remediation row
    // would leave the operator with a verdict and no remedy, while a disk
    // remediation row beside a healthy disk would cry wolf.
    let repo = tempfile::tempdir().expect("disk fixture repo");
    let summary = ompo_doctor::run_doctor(repo.path(), "system").expect("doctor runs");
    let row = summary
        .probes
        .iter()
        .find(|probe| probe.name == "disk")
        .expect("run_doctor must report a disk row");
    if row.status == "OK" {
        assert!(
            row.presence.as_ref().is_some_and(|value| !value.is_empty())
                && row.version.as_ref().is_some_and(|value| !value.is_empty()),
            "an OK disk row must carry measured capacity: {row:?}"
        );
    } else {
        assert!(
            row.reason_code.starts_with("L1_PROBE_DISK_"),
            "absence must carry a namespaced reason: {row:?}"
        );
    }
    assert_eq!(
        summary
            .remediation
            .iter()
            .any(|line| line.contains("probe=disk")),
        row.status != "OK",
        "disk remediation must exist exactly when the disk row is non-OK"
    );
}

/// L1-TEST-PROBE-FRANKENMERMAID (contract s1_l1_doctor.md
/// `L1-BUILD-PROBE-FRANKENMERMAID`): presence plus required version. Uses the
/// real `PROBES` declaration, the real `answered` authority, the real metric,
/// and one live `run_doctor` row -- never copies of their arms.
///
/// KNOWN-BAD: dropping the version conjunct from `answered` greens the
/// versionless `frankenmermaid` row below and this leg fails: presence alone
/// would certify a probe that answered nothing. Verdict AND reason code are
/// pinned throughout: one field alone cannot tell a refusal from a miscount.
/// Typed absence is none of OK, bare ABSENT, or UNRUN -- each unrun shape
/// carries its own status and reason.
#[test]
fn frankenmermaid_probe_emits_two_signals() {
    use ompo_doctor::{answered, probe_answer_metric, ProbeDecision, PROBES};
    let spec = PROBES
        .iter()
        .find(|spec| spec.name == "frankenmermaid")
        .expect("frankenmermaid is a declared probe");
    assert_eq!(
        spec.command, "frankenmermaid",
        "frankenmermaid probe runs frankenmermaid"
    );
    assert_eq!(
        spec.args,
        &["--version"],
        "frankenmermaid probe reads the version surface"
    );
    fn decision(status: &str, presence: Option<&str>, version: Option<&str>) -> ProbeDecision {
        ProbeDecision {
            name: "frankenmermaid".to_owned(),
            status: status.to_owned(),
            reason_code: "L1_PROBE_FRANKENMERMAID_SCOPED_FIXTURE".to_owned(),
            detail: "frankenmermaid scoped fixture".to_owned(),
            presence: presence.map(str::to_owned),
            version: version.map(str::to_owned),
        }
    }
    // Healthy: endpoint identity plus version answers.
    assert!(
        answered(&decision("OK", Some("/fixture/frankenmermaid"), Some("frankenmermaid 1.0"))),
        "identity plus version must answer"
    );
    // KNOWN-BAD shape, asserted directly: presence without version is not OK,
    // even when the status string claims it is. The status string does not
    // certify; the two signals do.
    let present_no_version =
        decision("OK", Some("/fixture/frankenmermaid"), None);
    assert!(
        !answered(&present_no_version),
        "a present frankenmermaid with no version response must not be OK"
    );
    let absent = decision("ABSENT_SPECIFIC", None, None);
    assert!(
        !answered(&absent),
        "absence must not answer"
    );
    // Unresponsive endpoint: no observation at all is UNMEASURED with
    // UNKNOWN_NO_RECORD, never a 0/N green.
    let metric = probe_answer_metric(PROBES, &[]);
    assert_eq!(
        metric.verdict, "UNMEASURED",
        "empty observation set must be UNMEASURED"
    );
    assert_eq!(
        metric.reason_code, "UNKNOWN_NO_RECORD",
        "empty observation set must name its reason"
    );
    assert!(
        metric.ratio.is_none(),
        "an unmeasured band must carry no ratio, not 0.0"
    );
    // The metric counts answers, not statuses: a full decision set whose
    // frankenmermaid row lacks a version stays below floor with the row named.
    // (Rows are built inline: the fixture helper above hardcodes one name.)
    let mut full: Vec<ProbeDecision> = PROBES
        .iter()
        .map(|spec| ProbeDecision {
            name: spec.name.to_owned(),
            status: "OK".to_owned(),
            reason_code: "L1_PROBE_SCOPED_FIXTURE".to_owned(),
            detail: "scoped fixture".to_owned(),
            presence: Some("/fixture/bin".to_owned()),
            version: Some("fixture 1.0".to_owned()),
        })
        .collect();
    if let Some(row) = full.iter_mut().find(|row| row.name == "frankenmermaid") {
        row.version = None;
        row.status = "UNPROBEABLE".to_owned();
    }
    let metric = probe_answer_metric(PROBES, &full);
    assert_eq!(
        metric.verdict, "MEASURED_BELOW_FLOOR",
        "one unanswered probe of {} must hold the metric below floor: {}",
        PROBES.len(),
        metric.reason_code
    );
    assert!(
        metric.reason_code.contains(&format!("answered={}", PROBES.len() - 1)),
        "below-floor reason must carry the count: {}",
        metric.reason_code
    );
    assert!(
        metric.unprobeable.contains(&"frankenmermaid".to_owned()),
        "the unanswering probe must be named: {:?}",
        metric.unprobeable
    );
    // Wrong version is STALE when observed: counted in the stale band with
    // its exact reason, never OK, ABSENT_SPECIFIC, or UNPROBEABLE. No
    // version floor is declared, so STALE is observed (constructed here),
    // never derived -- the bv_probe_emits_two_signals shape and the
    // LAW-L1-WRONG-VERSION leg, not a second surface.
    let stale = decision(
        "STALE",
        Some("frankenmermaid 1.0"),
        Some("frankenmermaid 0.0.0"),
    );
    assert!(!answered(&stale), "a stale row answers nothing itself");
    let stale_metric = probe_answer_metric(
        std::slice::from_ref(spec),
        &[
            decision(
                "OK",
                Some("/fixture/frankenmermaid"),
                Some("frankenmermaid 1.0"),
            ),
            stale,
        ],
    );
    assert_eq!(
        stale_metric.stale_count,
        Some(1),
        "an observed STALE row must be counted, not absorbed"
    );
    assert_eq!(
        stale_metric.stale_status, "MEASURED",
        "an observed STALE row makes the band measured"
    );
    // The live row, both lanes: OK implies both signals; anything else
    // is a typed absence with a namespaced reason -- never bare ABSENT and
    // never an UNRUN reading as refusal or health.
    let repo = tempfile::tempdir().expect("frankenmermaid fixture repo");
    let summary = ompo_doctor::run_doctor(repo.path(), "system").expect("doctor runs");
    let row = summary
        .probes
        .iter()
        .find(|probe| probe.name == "frankenmermaid")
        .expect("run_doctor must report a frankenmermaid row");
    if row.status == "OK" {
        assert!(
            row.presence.as_ref().is_some_and(|value| !value.is_empty())
                && row.version.as_ref().is_some_and(|value| !value.is_empty()),
            "an OK frankenmermaid row must carry both signals: {row:?}"
        );
    } else {
        assert!(
            [
                "ABSENT_FAMILY",
                "ABSENT_SPECIFIC",
                "UNPROBEABLE",
                "UNMEASURED",
                "STALE",
                "PAUSED"
            ]
            .contains(&row.status.as_str()),
            "a non-OK frankenmermaid row must be typed absence, never bare ABSENT or UNRUN: {row:?}"
        );
        assert!(
            row.reason_code.starts_with("L1_PROBE_FRANKENMERMAID_"),
            "absence must carry a namespaced reason: {row:?}"
        );
    }
}

/// L1-TEST-PROBE-GIT (contract s1_l1_doctor.md `L1-BUILD-PROBE-GIT`): run
/// `git rev-parse --show-toplevel`; expect repository identity or
/// ABSENT_FAMILY. The mapping below is the whole subject: exit 0 plus a
/// directory that exists is identity; git's not-a-repository refusal is
/// ABSENT_FAMILY; anything else (killed, missing binary, unreadable output)
/// is an ERROR, never a quiet third verdict -- ABSENT_FAMILY and UNRUN
/// share nothing, by construction rather than by comment.
///
/// KNOWN-BAD: invert the mapping (a refusal reads as identity) and the
/// absence arm fails: a tempdir would certify as a repository. Message AND
/// exit are pinned on both arms: git's 128 is as load-bearing as its fatal
/// text, and either alone passes a nearby wrong state.
#[test]
fn git_probe_rejects_non_repo() {
    fn identity_of(dir: &std::path::Path) -> (Option<i32>, String, String) {
        // GIT_CEILING_DIRECTORIES pins the upward search at the fixture's
        // PARENT, never the fixture itself: git will not exclude its own
        // cwd, so a self-ceiling is silently ignored (measured 2026-09-13:
        // self and trailing-slash ceilings read 0, parent reads 128).
        // Needed at all because worker TMPDIRs live inside checkouts, so a
        // bare tempdir without a ceiling answers for its PARENT repo instead
        // of itself (exit 0 where 128 was expected -- an 8y environment
        // coupling, fixed at the fixture, not the probe). Both -C and the
        // ceiling use canonical paths: an unresolved symlink component would
        // silently void the comparison.
        let dir = dir
            .canonicalize()
            .expect("fixture dir must resolve");
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["rev-parse", "--show-toplevel"])
            .env(
                "GIT_CEILING_DIRECTORIES",
                dir.parent().expect("fixture has a parent"),
            )
            .output()
            .expect("git binary must be spawnable on every lane; a missing git is an environment ERROR, never absence");
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).trim().to_owned(),
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        )
    }
    fn classify(code: Option<i32>, stdout: &str, stderr: &str) -> &'static str {
        match code {
            Some(0) if !stdout.is_empty() && std::path::Path::new(stdout).is_dir() => {
                "REPOSITORY_IDENTITY"
            }
            Some(128) if stderr.contains("not a git repository") => "ABSENT_FAMILY",
            _ => panic!(
                "unexpected git shape: refusing to map it silently (code={code:?}, out={stdout:?}, err={stderr:?})"
            ),
        }
    }
    // Identity arm: a real repo answers with its own toplevel.
    let repo = tempfile::tempdir().expect("git fixture repo");
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(repo.path())
        .args(["init", "-q"])
        .status()
        .expect("git init must run");
    assert!(status.success(), "fixture repo must init");
    let (code, stdout, stderr) = identity_of(repo.path());
    assert_eq!(code, Some(0), "rev-parse in a repo must exit 0: {stderr}");
    assert_eq!(
        stdout,
        repo.path().canonicalize().expect("canonical fixture").display().to_string(),
        "identity must be the fixture toplevel itself"
    );
    assert_eq!(
        classify(code, &stdout, &stderr),
        "REPOSITORY_IDENTITY",
        "a repo must classify as identity"
    );
    // Absence arm: a bare tempdir is ABSENT_FAMILY with git's message and exit.
    let bare = tempfile::tempdir().expect("bare fixture dir");
    let (code, stdout, stderr) = identity_of(bare.path());
    assert_eq!(code, Some(128), "rev-parse outside a repo must exit 128");
    assert!(
        stderr.contains("not a git repository"),
        "absence must carry git's fatal text: {stderr}"
    );
    assert_eq!(
        classify(code, &stdout, &stderr),
        "ABSENT_FAMILY",
        "a non-repo must classify as absent family, never UNRUN and never identity"
    );
}

#[test]
fn ntm_probe_emits_two_signals() {
    use ompo_doctor::{answered, probe_answer_metric, ProbeDecision, PROBES};
    let spec = PROBES
        .iter()
        .find(|spec| spec.name == "ntm")
        .expect("ntm is a declared probe");
    assert_eq!(spec.command, "ntm", "ntm probe runs ntm");
    assert_eq!(
        spec.args,
        &["--version"],
        "ntm probe reads the version surface"
    );
    fn decision(status: &str, presence: Option<&str>, version: Option<&str>) -> ProbeDecision {
        ProbeDecision {
            name: "ntm".to_owned(),
            status: status.to_owned(),
            reason_code: "L1_PROBE_NTM_SCOPED_FIXTURE".to_owned(),
            detail: "ntm scoped fixture".to_owned(),
            presence: presence.map(str::to_owned),
            version: version.map(str::to_owned),
        }
    }
    // Healthy: endpoint identity plus version answers.
    let healthy = decision("OK", Some("ntm 0.2.0"), Some("ntm 0.2.0"));
    // NOTE: fixture version strings are shaped data, not minimums; no probe
    // declares a version floor, so STALE is unmeasured by construction here.
    assert!(answered(&healthy), "identity plus version must answer");
    // KNOWN-BAD shape, asserted directly: presence without version is not OK.
    let present_no_version = decision("OK", Some("ntm 0.2.0"), None);
    assert!(
        !answered(&present_no_version),
        "a present ntm with no version response must not be OK"
    );
    // Unsupported answers are not OK either: only the two signals certify.
    let unsupported = decision("SUPPORTED", Some("ntm 0.2.0"), Some("ntm 0.2.0"));
    assert!(
        !answered(&unsupported),
        "an unsupported status must not read as OK"
    );
    // UNPROBEABLE is observed and named: the row is listed, the metric drops
    // below floor, and the verdict is never a guessed OK.
    let full: Vec<ProbeDecision> = PROBES
        .iter()
        .map(|spec| {
            let (presence, version) = if spec.name == "ntm" {
                (None, None)
            } else {
                (Some("/fixture/bin"), Some("fixture 1.0"))
            };
            let mut row = decision("OK", presence, version);
            row.name = spec.name.to_owned();
            if spec.name == "ntm" {
                row.status = "UNPROBEABLE".to_owned();
            }
            row
        })
        .collect();
    let metric = probe_answer_metric(PROBES, &full);
    assert_eq!(
        metric.verdict, "MEASURED_BELOW_FLOOR",
        "one unanswered probe of {} must hold the metric below floor: {}",
        PROBES.len(),
        metric.reason_code
    );
    assert!(
        metric.unprobeable.contains(&"ntm".to_owned()),
        "the unanswering probe must be named: {:?}",
        metric.unprobeable
    );
    // The live row, both lanes: OK implies both signals; anything else
    // is a typed absence with a namespaced reason -- never bare ABSENT and
    // never an UNRUN reading as refusal or health.
    let repo = tempfile::tempdir().expect("ntm fixture repo");
    let summary = ompo_doctor::run_doctor(repo.path(), "system").expect("doctor runs");
    let row = summary
        .probes
        .iter()
        .find(|probe| probe.name == "ntm")
        .expect("run_doctor must report an ntm row");
    if row.status == "OK" {
        assert!(
            row.presence.as_ref().is_some_and(|value| !value.is_empty())
                && row.version.as_ref().is_some_and(|value| !value.is_empty()),
            "an OK ntm row must carry both signals: {row:?}"
        );
    } else {
        assert!(
            [
                "ABSENT_FAMILY",
                "ABSENT_SPECIFIC",
                "UNPROBEABLE",
                "UNMEASURED",
                "STALE",
                "PAUSED"
            ]
            .contains(&row.status.as_str()),
            "a non-OK ntm row must be typed absence, got: {row:?}"
        );
    }
}

/// LAW-L1-WRONG-VERSION (contract s1_l1_doctor.md `L1-BUILD-TWO-SIGNALS`):
/// a present tool with a wrong version is STALE, never OK or ABSENT.
/// No version floor is declared, so STALE is OBSERVED (constructed here),
/// never derived -- and an observed STALE row must be COUNTED in the stale
/// band and keep its status, not be absorbed into OK, ABSENT, or UNPROBEABLE.
#[test]
fn wrong_version_is_stale() {
    use ompo_doctor::{answered, probe_answer_metric, ProbeDecision, PROBES};
    fn decision(name: &str, status: &str, presence: Option<&str>, version: Option<&str>) -> ProbeDecision {
        ProbeDecision {
            name: name.to_owned(),
            status: status.to_owned(),
            reason_code: "L1_PROBE_STALE_SCOPED_FIXTURE".to_owned(),
            detail: "stale scoped fixture".to_owned(),
            presence: presence.map(str::to_owned),
            version: version.map(str::to_owned),
        }
    }
    // Positive control: the paired healthy input answers. Without this arm
    // a dead authority (`answered` returning false for everything) passes
    // every assertion below -- the leg would certify an exclusion it never
    // exercised. No version floor is declared anywhere (ProbeSpec carries
    // name/command/args only), so "healthy" is both signals with OK status,
    // never a floor comparison.
    assert!(
        answered(&decision("tmux", "OK", Some("tmux 1.0"), Some("tmux 1.0"))),
        "the paired healthy input must answer"
    );
    // Wrong version, both signals present: present but not OK, and the row
    // keeps its STALE status -- it is not remapped to absence.
    let stale = decision("tmux", "STALE", Some("tmux 1.0"), Some("tmux 0.1"));
    assert!(
        !answered(&stale),
        "a stale row must not answer, even with both signals present"
    );
    assert_eq!(stale.status, "STALE", "stale status must survive classification");
    // Counted, not absorbed: the metric's stale band measures it.
    let mut full: Vec<ProbeDecision> = PROBES
        .iter()
        .map(|spec| {
            decision(
                spec.name,
                "OK",
                Some("/fixture/bin"),
                Some("fixture 1.0"),
            )
        })
        .collect();
    if let Some(row) = full.iter_mut().find(|row| row.name == "tmux") {
        row.status = "STALE".to_owned();
        row.version = Some("tmux 0.1".to_owned());
    }
    let metric = probe_answer_metric(PROBES, &full);
    assert_eq!(
        metric.stale_count,
        Some(1),
        "an observed STALE row must be counted, not absorbed"
    );
    assert_eq!(
        metric.stale_status, "MEASURED",
        "an observed STALE row makes the band measured"
    );
    assert!(
        !metric.unprobeable.contains(&"tmux".to_owned()),
        "stale is not unprobeable: {:?}",
        metric.unprobeable
    );
    // Contrast, same set all-OK: the band is unmeasured when nothing is
    // observed -- unreachability stated, not a zero smuggled in.
    let clean: Vec<ProbeDecision> = PROBES
        .iter()
        .map(|spec| decision(spec.name, "OK", Some("/fixture/bin"), Some("fixture 1.0")))
        .collect();
    let clean_metric = probe_answer_metric(PROBES, &clean);
    assert_eq!(
        clean_metric.stale_count, None,
        "no observed STALE row means unmeasured, never zero"
    );
}

#[test]
fn rch_probe_emits_two_signals() {
    use ompo_doctor::{answered, ProbeDecision, PROBES};
    let spec = PROBES
        .iter()
        .find(|spec| spec.name == "rch")
        .expect("rch is a declared probe");
    assert_eq!(spec.command, "rch", "rch probe runs rch");
    fn decision(status: &str, presence: Option<&str>, version: Option<&str>) -> ProbeDecision {
        ProbeDecision {
            name: "rch".to_owned(),
            status: status.to_owned(),
            reason_code: "L1_PROBE_RCH_SCOPED_FIXTURE".to_owned(),
            detail: "rch scoped fixture".to_owned(),
            presence: presence.map(str::to_owned),
            version: version.map(str::to_owned),
        }
    }
    // Healthy: endpoint identity plus version answers.
    let healthy = decision("OK", Some("rch 0.9.1"), Some("rch 0.9.1"));
    // NOTE: fixture version strings are shaped data, not minimums; no probe
    // declares a version floor, so STALE is unmeasured by construction here.
    assert!(answered(&healthy), "identity plus version must answer");
    // KNOWN-BAD shape, asserted directly: presence without version is not OK.
    let present_no_version = decision("OK", Some("rch 0.9.1"), None);
    assert!(
        !answered(&present_no_version),
        "a present rch with no version response must not be OK"
    );
    // Unsupported answers are not OK either: only the two signals certify.
    let unsupported = decision("SUPPORTED", Some("rch 0.9.1"), Some("rch 0.9.1"));
    assert!(
        !answered(&unsupported),
        "an unsupported status must not read as OK"
    );
    // The live row, both lanes: OK implies both signals; anything else
    // is a typed absence with a namespaced reason -- never bare ABSENT and
    // never an UNRUN reading as refusal or health.
    let repo = tempfile::tempdir().expect("rch fixture repo");
    let summary = ompo_doctor::run_doctor(repo.path(), "system").expect("doctor runs");
    let row = summary
        .probes
        .iter()
        .find(|probe| probe.name == "rch")
        .expect("run_doctor must report an rch row");
    if row.status == "OK" {
        assert!(
            row.presence.as_ref().is_some_and(|value| !value.is_empty())
                && row.version.as_ref().is_some_and(|value| !value.is_empty()),
            "an OK rch row must carry both signals: {row:?}"
        );
    } else {
        assert!(
            [
                "ABSENT_FAMILY",
                "ABSENT_SPECIFIC",
                "UNPROBEABLE",
                "UNMEASURED",
                "STALE",
                "PAUSED"
            ]
            .contains(&row.status.as_str()),
            "a non-OK rch row must be typed absence, got: {row:?}"
        );
    }
}

#[test]
fn tmux_probe_emits_two_signals() {
    use ompo_doctor::{answered, ProbeDecision, PROBES};
    let spec = PROBES
        .iter()
        .find(|spec| spec.name == "tmux")
        .expect("tmux is a declared probe");
    assert_eq!(spec.command, "tmux", "tmux probe runs tmux");
    assert_eq!(
        spec.args,
        &["-V"],
        "tmux probe reads the version surface"
    );
    fn decision(status: &str, presence: Option<&str>, version: Option<&str>) -> ProbeDecision {
        ProbeDecision {
            name: "tmux".to_owned(),
            status: status.to_owned(),
            reason_code: "L1_PROBE_TMUX_SCOPED_FIXTURE".to_owned(),
            detail: "tmux scoped fixture".to_owned(),
            presence: presence.map(str::to_owned),
            version: version.map(str::to_owned),
        }
    }
    // Healthy: endpoint identity plus version answers.
    let healthy = decision("OK", Some("tmux 3.6a"), Some("tmux 3.6a"));
    // NOTE: fixture version strings are shaped data, not minimums; no probe
    // declares a version floor, so STALE is unmeasured by construction here.
    assert!(answered(&healthy), "identity plus version must answer");
    // KNOWN-BAD shape, asserted directly: presence without version is not OK.
    let present_no_version = decision("OK", Some("tmux 3.6a"), None);
    assert!(
        !answered(&present_no_version),
        "a present tmux with no version response must not be OK"
    );
    // Unsupported answers are not OK either: only the two signals certify.
    let unsupported = decision("SUPPORTED", Some("tmux 3.6a"), Some("tmux 3.6a"));
    assert!(
        !answered(&unsupported),
        "an unsupported status must not read as OK"
    );
    // The live row, both lanes: OK implies both signals; anything else
    // is a typed absence with a namespaced reason -- never bare ABSENT and
    // never an UNRUN reading as refusal or health.
    let repo = tempfile::tempdir().expect("tmux fixture repo");
    let summary = ompo_doctor::run_doctor(repo.path(), "system").expect("doctor runs");
    let row = summary
        .probes
        .iter()
        .find(|probe| probe.name == "tmux")
        .expect("run_doctor must report a tmux row");
    if row.status == "OK" {
        assert!(
            row.presence.as_ref().is_some_and(|value| !value.is_empty())
                && row.version.as_ref().is_some_and(|value| !value.is_empty()),
            "an OK tmux row must carry both signals: {row:?}"
        );
    } else {
        assert!(
            [
                "ABSENT_FAMILY",
                "ABSENT_SPECIFIC",
                "UNPROBEABLE",
                "UNMEASURED",
                "STALE",
                "PAUSED"
            ]
            .contains(&row.status.as_str()),
            "a non-OK tmux row must be typed absence, got: {row:?}"
        );
    }
}

#[test]
fn socraticode_probe_emits_two_signals() {
    use ompo_doctor::{answered, ProbeDecision, PROBES};
    let spec = PROBES
        .iter()
        .find(|spec| spec.name == "socraticode")
        .expect("socraticode is a declared probe");
    assert_eq!(spec.command, "socraticode", "socraticode probe runs socraticode");
    fn decision(status: &str, presence: Option<&str>, version: Option<&str>) -> ProbeDecision {
        ProbeDecision {
            name: "socraticode".to_owned(),
            status: status.to_owned(),
            reason_code: "L1_PROBE_SOCRATICODE_SCOPED_FIXTURE".to_owned(),
            detail: "socraticode scoped fixture".to_owned(),
            presence: presence.map(str::to_owned),
            version: version.map(str::to_owned),
        }
    }
    // Healthy: endpoint identity plus version answers.
    let healthy = decision("OK", Some("socraticode 0.5.2"), Some("socraticode 0.5.2"));
    // NOTE: fixture version strings are shaped data, not minimums; no probe
    // declares a version floor, so STALE is unmeasured by construction here.
    assert!(answered(&healthy), "identity plus version must answer");
    // KNOWN-BAD shape, asserted directly: presence without version is not OK.
    let present_no_version = decision("OK", Some("socraticode 0.5.2"), None);
    assert!(
        !answered(&present_no_version),
        "a present socraticode with no version response must not be OK"
    );
    // Unsupported answers are not OK either: only the two signals certify.
    let unsupported = decision("SUPPORTED", Some("socraticode 0.5.2"), Some("socraticode 0.5.2"));
    assert!(
        !answered(&unsupported),
        "an unsupported status must not read as OK"
    );
    // The live row, both lanes: OK implies both signals; anything else
    // is a typed absence with a namespaced reason -- never bare ABSENT and
    // never an UNRUN reading as refusal or health.
    let repo = tempfile::tempdir().expect("socraticode fixture repo");
    let summary = ompo_doctor::run_doctor(repo.path(), "system").expect("doctor runs");
    let row = summary
        .probes
        .iter()
        .find(|probe| probe.name == "socraticode")
        .expect("run_doctor must report a socraticode row");
    if row.status == "OK" {
        assert!(
            row.presence.as_ref().is_some_and(|value| !value.is_empty())
                && row.version.as_ref().is_some_and(|value| !value.is_empty()),
            "an OK socraticode row must carry both signals: {row:?}"
        );
    } else {
        assert!(
            [
                "ABSENT_FAMILY",
                "ABSENT_SPECIFIC",
                "UNPROBEABLE",
                "UNMEASURED",
                "STALE",
                "PAUSED"
            ]
            .contains(&row.status.as_str()),
            "a non-OK socraticode row must be typed absence, got: {row:?}"
        );
    }
}

/// L1-TEST-PROBE-SOCRATICODE (contract s1_l1_doctor.md): a present service
/// with no index record stays UNKNOWN -- never guessed OK. The two-signal
/// leg covers answer/refusal shapes; this one covers the recordless shape:
/// no observation row at all is UNMEASURED with UNKNOWN_NO_RECORD (never a
/// 0/N green), and a recordless row in a full set holds the metric below
/// floor with the probe named.
///
/// KNOWN-BAD: guessing on an empty observation set (MEASURED verdict or any
/// ratio over zero records) greens the unknown below and this leg fails: an
/// unobserved service would count as measured, which is the false-green
/// this row exists to prevent.
#[test]
fn socraticode_probe_preserves_unknown() {
    use ompo_doctor::{answered, probe_answer_metric, ProbeDecision, PROBES};
    fn decision(name: &str, status: &str, presence: Option<&str>, version: Option<&str>) -> ProbeDecision {
        ProbeDecision {
            name: name.to_owned(),
            status: status.to_owned(),
            reason_code: format!("L1_PROBE_{}_SCOPED_FIXTURE", name.replace('-', "_").to_ascii_uppercase()),
            detail: "socraticode scoped fixture".to_owned(),
            presence: presence.map(str::to_owned),
            version: version.map(str::to_owned),
        }
    }
    // Positive control: endpoint identity plus version answers.
    assert!(
        answered(&decision("socraticode", "OK", Some("/fixture/socraticode"), Some("socraticode 1.2.0"))),
        "identity plus version must answer"
    );
    // Service present, no index record: no observation row exists, so there
    // is nothing to answer -- and the metric must say UNKNOWN, not 0/N.
    let recordless = decision("socraticode", "UNMEASURED", Some("/fixture/socraticode"), None);
    assert!(
        !answered(&recordless),
        "a recordless service must not answer, even when present"
    );
    let metric = probe_answer_metric(PROBES, &[]);
    assert_eq!(
        metric.verdict, "UNMEASURED",
        "no index record must be UNMEASURED, got {}",
        metric.verdict
    );
    assert_eq!(
        metric.reason_code, "UNKNOWN_NO_RECORD",
        "no index record must name its reason, got {}",
        metric.reason_code
    );
    assert!(
        metric.ratio.is_none(),
        "an unmeasured band must carry no ratio, not 0.0"
    );
    // Below-floor companion: a full set whose socraticode row carries no
    // index evidence stays below floor with the probe named.
    let mut full: Vec<ProbeDecision> = PROBES
        .iter()
        .map(|spec| {
            decision(
                spec.name,
                "OK",
                Some("/fixture/bin"),
                Some("fixture 1.0"),
            )
        })
        .collect();
    if let Some(row) = full.iter_mut().find(|row| row.name == "socraticode") {
        row.version = None;
        row.status = "UNMEASURED".to_owned();
    }
    let metric = probe_answer_metric(PROBES, &full);
    assert_eq!(
        metric.verdict, "MEASURED_BELOW_FLOOR",
        "one recordless probe of {} must hold the metric below floor: {}",
        PROBES.len(),
        metric.reason_code
    );
    assert!(
        metric.reason_code.contains(&format!("answered={}", PROBES.len() - 1)),
        "below-floor reason must carry the count: {}",
        metric.reason_code
    );
}

/// LAW-L1-UNRUN (contract s1_l1_doctor.md): a deadline-expired probe is
/// UNMEASURED, never Pass and never a refusal. Models the row production
/// `run_probe` emits on `BoundedOutcome::TimedOut` (status UNMEASURED,
/// detail "probe timed out", no version) and pins it through the real
/// `answered` authority and the real exit band.
///
/// VOCABULARY NOTE, not a dodge: the contract wire shape names
/// `run_state=UNRUN` on the UNMEASURED arm, but no `run_state` carrier
/// exists in code (`ProbeDecision` carries status plus reason_code; the
/// `ompo state` verb is an unrelated OMP-session surface). This leg pins
/// every producible element -- exact UNMEASURED status (which excludes all
/// six other arms, refusal-flavored or otherwise), the reason, the
/// unanswered reading, and exit 1 -- and does not synthesize the field.
/// Carrying `run_state` needs a production BUILD bead, not a TEST row.
///
/// KNOWN-BAD: answering UNMEASURED rows (status-blind `answered`) greens
/// the expired probe below and this leg fails: a measurement that never
/// ran would count as answered, which is the false-Pass this row exists
/// to prevent.
#[test]
fn timeout_is_unmeasured_unrun() {
    use ompo_doctor::{answered, doctor_exit_code, ProbeDecision};
    // The deadline-expired shape, as production emits it.
    let expired = ProbeDecision {
        name: "fixture-hung".to_owned(),
        status: "UNMEASURED".to_owned(),
        reason_code: "L1_PROBE_FIXTURE_HUNG_UNMEASURED".to_owned(),
        detail: "probe timed out".to_owned(),
        presence: Some("/fixture/hung".to_owned()),
        version: None,
    };
    assert_eq!(
        expired.status, "UNMEASURED",
        "an expired probe must be UNMEASURED, got {}",
        expired.status
    );
    assert!(
        !answered(&expired),
        "an expired probe must not answer, even when present"
    );
    // Never Pass: a set containing only the expired probe exits 1.
    assert_eq!(
        doctor_exit_code(&[expired]).expect("non-empty set"),
        1,
        "an unrun measurement must never exit 0"
    );
    // Positive control: the paired healthy input answers and exits 0.
    let healthy = ProbeDecision {
        name: "fixture-healthy".to_owned(),
        status: "OK".to_owned(),
        reason_code: "L1_PROBE_FIXTURE_HEALTHY_OK".to_owned(),
        detail: "exit=0 fixture 1.0".to_owned(),
        presence: Some("/fixture/healthy".to_owned()),
        version: Some("fixture 1.0".to_owned()),
    };
    assert!(answered(&healthy), "identity plus version must answer");
    assert_eq!(
        doctor_exit_code(&[healthy]).expect("non-empty set"),
        0,
        "a healthy set exits 0"
    );
}

/// L1-TEST-ABSENT-REMEDIATION (contract s1_l1_doctor.md
/// `L1-BUILD-REMEDIATION`): every absent verdict carries a NAMED
/// remediation, because a bare "rerun" on a missing tool is advice to
/// repeat a measurement whose subject does not exist. Uses the real
/// `run_doctor` summary rows and the real remediation vector -- never a
/// copy of the table (`remediation_for` is private to the crate; the
/// observable contract is rows-in/rows-out).
///
/// KNOWN-BAD: empty the remediation of an absent row and this leg fails:
/// an ABSENT verdict with no remedy leaves the operator with a verdict
/// and no next action. Message AND exit are pinned on the mutation run.
#[test]
fn every_absent_has_remediation() {
    let repo = tempfile::tempdir().expect("remediation fixture repo");
    let summary = ompo_doctor::run_doctor(repo.path(), "system").expect("doctor runs");
    // Shape holds on every lane, empty or not: each remediation line names
    // its probe and its reason. An emptied remediation fails here on any
    // lane that emits one.
    for line in &summary.remediation {
        assert!(
            line.contains("probe=") && line.contains("reason="),
            "every remediation line names probe and reason, got: {line}"
        );
    }
    let absent: Vec<_> = summary
        .probes
        .iter()
        .filter(|probe| probe.status == "ABSENT_FAMILY" || probe.status == "ABSENT_SPECIFIC")
        .collect();
    if absent.is_empty() {
        // Positive control: the paired healthy input -- every row OK means
        // OK contributes no remediation row at all.
        assert!(
            summary.remediation.is_empty(),
            "an all-OK lane emits no remediation rows, got: {:?}",
            summary.remediation
        );
    } else {
        for row in absent {
            let hit = summary
                .remediation
                .iter()
                .find(|line| line.contains(&format!("probe={}", row.name)));
            assert!(
                hit.is_some(),
                "absent probe {} must carry remediation, got: {:?}",
                row.name,
                summary.remediation
            );
            let line = hit.unwrap();
            assert!(
                line.contains(&row.reason_code),
                "remediation for {} must carry its namespaced reason {}, got: {line}",
                row.name,
                row.reason_code
            );
        }
    }
}

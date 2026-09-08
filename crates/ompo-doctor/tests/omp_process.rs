//! Integration and pure-boundary legs for the ompo ps operator surface.
//!
//! The live legs only start the bounded omp ps --json --dir child. They never connect to the
//! broker socket and never invoke stop, kill, or restart.

use ompo_doctor::omp_process::{
    self, classify_ps_output, envelope, parse_ps_json, ProcessParseError, ProcessProbeVerdict,
};
use std::process::Command;

fn ompo() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ompo"))
}

#[test]
fn ps_is_reachable_and_never_reports_an_unknown_verb() {
    let output = ompo().arg("ps").output().expect("ompo runs");
    assert!(
        matches!(output.status.code(), Some(0) | Some(1) | Some(4)),
        "ps must answer from its own dictionary (0, 1, or 4), got {:?}",
        output.status.code()
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !combined.contains("UAD_UNKNOWN_VERB"),
        "dispatched ps must not report itself unknown: {combined:?}"
    );
}

#[test]
fn ps_unknown_flag_is_an_invocation_error_before_starting_omp() {
    let output = ompo()
        .args(["ps", "--not-a-ps-flag"])
        .output()
        .expect("ompo runs");
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.trim().is_empty(),
        "argument refusal must not write stdout: {stdout:?}"
    );
    assert!(
        stderr.contains("unknown argument"),
        "stderr must name the bad flag: {stderr:?}"
    );
    assert!(
        !stderr.contains("OMP_PS_"),
        "a caller typo must not be reported as an OMP probe result: {stderr:?}"
    );
}

#[test]
fn ps_missing_repo_value_is_an_invocation_error() {
    let output = ompo().args(["ps", "--repo"]).output().expect("ompo runs");
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("UAD_MISSING_VALUE"));
}

fn measured_fixture() -> &'static str {
    r#"[
      {
        "kind": "project",
        "projectDir": "/repo",
        "runtimeDir": "/run/daemons/abcd",
        "brokerPid": 99,
        "broker": {"token": "super-secret-broker-token"},
        "daemons": [
          {
            "name": "omp.test",
            "id": "daemon-1",
            "state": "ready",
            "pid": 123,
            "createdAt": 1,
            "startedAt": 2,
            "readyAt": 3,
            "restartCount": 0,
            "outputBytes": 4,
            "readyMatch": "ready",
            "persist": false,
            "detached": false,
            "command": "omp test",
            "cwd": "/repo",
            "supervised": true
          },
          {
            "name": "omp.exited",
            "id": "daemon-2",
            "state": "exited",
            "createdAt": 5,
            "startedAt": 6,
            "readyAt": 7,
            "restartCount": 1,
            "outputBytes": 8,
            "readyMatch": "ready",
            "persist": false,
            "detached": false,
            "command": "omp exited",
            "cwd": "/repo",
            "supervised": true,
            "exitCode": 17,
            "exitedAt": 9
          }
        ]
      }
    ]"#
}

#[test]
fn parser_accepts_measured_project_and_exited_daemon_shapes() {
    let scopes = parse_ps_json(measured_fixture()).expect("measured ps fixture");
    assert_eq!(scopes.len(), 1);
    assert_eq!(scopes[0].project_dir, "/repo");
    assert_eq!(scopes[0].daemons.len(), 2);
    assert_eq!(scopes[0].daemons[0].pid, Some(123));
    assert_eq!(scopes[0].daemons[1].pid, None);
    assert_eq!(scopes[0].daemons[1].exit_code, Some(17));
}

#[test]
fn success_envelope_exposes_scopes_and_daemons_without_broker_token() {
    let scopes = parse_ps_json(measured_fixture()).expect("measured ps fixture");
    let outcome = omp_process::ProcessProbeOutcome {
        exit_code: Some(0),
        verdict: ProcessProbeVerdict::Answered(scopes),
    };
    let encoded = serde_json::to_string(&envelope(&outcome)).expect("envelope serializes");
    let value: serde_json::Value = serde_json::from_str(&encoded).expect("envelope is JSON");
    assert_eq!(value["command"], "ps");
    assert_eq!(value["status"], "OK");
    assert_eq!(value["data"]["project_scope_count"], 1);
    assert_eq!(value["data"]["daemon_row_count"], 2);
    assert_eq!(
        value["data"]["project_scopes"][0]["daemons"][1]["pid"],
        serde_json::Value::Null
    );
    assert!(!encoded.contains("super-secret-broker-token"));
    assert!(value["data"]["project_scopes"][0]["broker"].is_null());
}

#[test]
fn parser_and_classifier_keep_failure_boundaries_typed() {
    assert!(matches!(
        parse_ps_json("{not-json"),
        Err(ProcessParseError::InvalidJson { .. })
    ));
    assert!(matches!(
        parse_ps_json("{}"),
        Err(ProcessParseError::InvalidShape { .. })
    ));

    let answered = classify_ps_output(Some(0), measured_fixture(), "");
    assert_eq!(answered.exit_code(), omp_process::EXIT_OK);
    assert_eq!(answered.reason_code(), "OMP_PS_OK");
    assert!(matches!(answered.verdict, ProcessProbeVerdict::Answered(_)));

    let child_failed = classify_ps_output(Some(17), "", "child refused");
    assert_eq!(child_failed.exit_code(), omp_process::EXIT_FAILED);
    assert_eq!(child_failed.reason_code(), "OMP_PS_CHILD_FAILED");
    assert!(matches!(
        child_failed.verdict,
        ProcessProbeVerdict::ChildFailed { .. }
    ));

    let malformed = classify_ps_output(Some(0), "[] trailing", "");
    assert_eq!(malformed.exit_code(), omp_process::EXIT_FAILED);
    assert_eq!(malformed.reason_code(), "OMP_PS_INVALID_JSON");
}

//! UDS target-gate consumer: version then projection, UNPROVEN vs UNWIRED.
//!
//! Resident owns heartbeat I/O. This module owns parse, observe, and admission.

#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use asupersync::process::{Command, Output};
use asupersync::time::timeout;
use asupersync::Cx;
use serde_json::{Map, Value};
use subprocess_contract::run_output;

const UDS_ENVELOPE_SCHEMA: &str = "uds/v2";
const UDS_PROJECTION_SCHEMA: &str = "uds-target-gates/v1";
const UDS_TARGET_GATE_VERB: &str = "target-gate";
const UDS_VERSION_VERB: &str = "version";

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdsProcessResult {
    process_exit: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl UdsProcessResult {
    fn from_output(output: Output) -> Self {
        Self {
            process_exit: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdsTargetGateExpected {
    target: String,
    root: String,
    owner: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UdsTargetGateRequirement {
    pub(crate) id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UdsTargetGateObservation {
    pub(crate) trigger: String,
    pub(crate) requirements: Vec<UdsTargetGateRequirement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UdsObserveInput<'a> {
    pub(crate) binary: Option<&'a Path>,
    pub(crate) registry: Option<&'a Path>,
    pub(crate) repo: &'a Path,
    pub(crate) tmux_tmpdir: &'a Path,
    pub(crate) command_timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UdsCycleAdmission {
    Observed(UdsTargetGateObservation),
    Unproven { detail: String },
    Unwired { detail: String },
}

fn one_line_stderr(detail: &str) -> String {
    detail.replace('\r', " ").replace('\n', " ")
}

fn uds_target_gate_unwired(detail: impl AsRef<str>) -> String {
    format!("UDS_TARGET_GATE_UNWIRED {}", detail.as_ref())
}

fn uds_target_gate_unproven(detail: impl AsRef<str>) -> String {
    format!("UDS_TARGET_GATE_UNPROVEN {}", detail.as_ref())
}

fn uds_admission_is_unproven(error: &str) -> bool {
    error.starts_with("UDS_TARGET_GATE_UNPROVEN ")
}

fn uds_target_gate_exit(code: &str) -> Option<i32> {
    match code {
        "EC-PASS" => Some(0),
        "EC-RED" => Some(1),
        "EC-USAGE" => Some(2),
        "EC-UNRUN" => Some(77),
        _ => None,
    }
}

fn expected_from_host(host: &UdsObserveInput<'_>) -> UdsTargetGateExpected {
    let name = host
        .repo
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_owned();
    UdsTargetGateExpected {
        target: name.clone(),
        root: host.repo.display().to_string(),
        owner: name,
    }
}

fn parse_uds_envelope(
    result: &UdsProcessResult,
    expected_verb: &str,
    fail_prefix: &str,
) -> Result<(Value, i32, String), String> {
    let fail = |detail: String| format!("{fail_prefix} {detail}");
    let process_exit = result
        .process_exit
        .ok_or_else(|| fail("process_exit=signal".to_owned()))?;
    let value: Value = serde_json::from_slice(&result.stdout).map_err(|error| {
        fail(format!(
            "malformed_envelope detail={} stderr={}",
            error,
            one_line_stderr(&String::from_utf8_lossy(&result.stderr))
        ))
    })?;
    let schema = value
        .get("schema")
        .and_then(Value::as_str)
        .ok_or_else(|| fail("malformed_envelope missing=schema".to_owned()))?;
    if schema != UDS_ENVELOPE_SCHEMA {
        return Err(fail(format!(
            "envelope_schema={schema} expected={UDS_ENVELOPE_SCHEMA}"
        )));
    }
    let verb = value
        .get("verb")
        .and_then(Value::as_str)
        .ok_or_else(|| fail("malformed_envelope missing=verb".to_owned()))?;
    if verb != expected_verb {
        return Err(fail(format!(
            "envelope_verb={verb} expected={expected_verb}"
        )));
    }
    let code = value
        .get("code")
        .and_then(Value::as_str)
        .ok_or_else(|| fail("malformed_envelope missing=code".to_owned()))?
        .to_owned();
    let exit_status = value
        .get("exit_status")
        .and_then(Value::as_i64)
        .ok_or_else(|| fail("malformed_envelope missing=exit_status".to_owned()))?;
    if exit_status != i64::from(process_exit) {
        return Err(fail(format!(
            "envelope_mismatch code={code} exit_status={exit_status} process_exit={process_exit}"
        )));
    }
    let Some(expected_exit) = uds_target_gate_exit(&code) else {
        return Err(fail(format!(
            "unsupported_code code={code} exit_status={exit_status}"
        )));
    };
    if exit_status != i64::from(expected_exit) {
        return Err(fail(format!(
            "code_exit_mismatch code={code} exit_status={exit_status} expected={expected_exit}"
        )));
    }
    Ok((value, process_exit, code))
}

fn uds_version_is_usable(version: &str) -> bool {
    let version = version.trim();
    if version.is_empty() || version.eq_ignore_ascii_case("unknown") {
        return false;
    }
    version.bytes().any(|byte| byte.is_ascii_digit())
}

fn uds_version_advertises_target_gate(value: &Value) -> bool {
    let lists = ["commands", "capabilities"];
    for key in lists {
        let Some(entries) = value.get(key).and_then(Value::as_array) else {
            continue;
        };
        for entry in entries {
            if entry.as_str() == Some(UDS_TARGET_GATE_VERB) {
                return true;
            }
            if entry.get("verb").and_then(Value::as_str) == Some(UDS_TARGET_GATE_VERB) {
                return true;
            }
        }
    }
    false
}

fn parse_uds_version(result: UdsProcessResult) -> Result<(), String> {
    let (value, _process_exit, code) =
        parse_uds_envelope(&result, UDS_VERSION_VERB, "UDS_TARGET_GATE_UNPROVEN")?;
    if code != "EC-PASS" {
        let detail = value
            .get("detail")
            .and_then(Value::as_str)
            .unwrap_or("typed UDS version refusal");
        return Err(uds_target_gate_unproven(format!(
            "uds_code={code} detail={detail}"
        )));
    }
    let version = value
        .get("version")
        .and_then(Value::as_str)
        .ok_or_else(|| uds_target_gate_unproven("malformed_envelope missing=version"))?;
    if !uds_version_is_usable(version) {
        return Err(uds_target_gate_unproven(format!(
            "unverified_version version={version}"
        )));
    }
    match value.get("commit_verified") {
        Some(Value::Bool(true)) => {}
        Some(Value::Bool(false)) => {
            return Err(uds_target_gate_unproven(
                "unverified_version commit_verified=false",
            ));
        }
        Some(_) => {
            return Err(uds_target_gate_unproven(
                "malformed_envelope commit_verified",
            ));
        }
        None => {
            return Err(uds_target_gate_unproven(
                "unverified_version missing=commit_verified",
            ));
        }
    }
    if !uds_version_advertises_target_gate(&value) {
        return Err(uds_target_gate_unproven(
            "advertised_capability_missing=target-gate",
        ));
    }
    Ok(())
}

fn projection_object(value: &Value) -> Result<&Map<String, Value>, String> {
    value
        .get("projection")
        .and_then(Value::as_object)
        .ok_or_else(|| uds_target_gate_unwired("malformed_projection missing=projection"))
}

fn parse_projection_identity(
    projection: &Map<String, Value>,
    expected: &UdsTargetGateExpected,
) -> Result<String, String> {
    let projection_schema = projection
        .get("schema")
        .and_then(Value::as_str)
        .ok_or_else(|| uds_target_gate_unwired("malformed_projection missing=schema"))?;
    if projection_schema != UDS_PROJECTION_SCHEMA {
        return Err(uds_target_gate_unwired(format!(
            "projection_schema={projection_schema} expected={UDS_PROJECTION_SCHEMA}"
        )));
    }
    let trigger = projection
        .get("trigger")
        .and_then(Value::as_str)
        .ok_or_else(|| uds_target_gate_unwired("malformed_projection missing=trigger"))?;
    if trigger.trim().is_empty() {
        return Err(uds_target_gate_unwired("empty_trigger"));
    }
    let target = projection
        .get("target")
        .and_then(Value::as_str)
        .ok_or_else(|| uds_target_gate_unwired("malformed_projection missing=target"))?;
    if target != expected.target {
        return Err(uds_target_gate_unwired(format!(
            "target_mismatch expected={} found={target}",
            expected.target
        )));
    }
    let root = projection
        .get("canonical_root")
        .and_then(Value::as_str)
        .ok_or_else(|| uds_target_gate_unwired("malformed_projection missing=canonical_root"))?;
    if root != expected.root {
        return Err(uds_target_gate_unwired(format!(
            "root_mismatch expected={} found={root}",
            expected.root
        )));
    }
    let owner = projection
        .get("owner")
        .and_then(Value::as_str)
        .ok_or_else(|| uds_target_gate_unwired("malformed_projection missing=owner"))?;
    if owner != expected.owner {
        return Err(uds_target_gate_unwired(format!(
            "owner_mismatch expected={} found={owner}",
            expected.owner
        )));
    }
    Ok(trigger.to_owned())
}

fn parse_one_requirement(
    row: &Value,
    index: usize,
    order: &[Value],
    seen: &mut BTreeSet<String>,
) -> Result<UdsTargetGateRequirement, String> {
    let row = row
        .as_object()
        .ok_or_else(|| uds_target_gate_unwired(format!("missing_requirement index={index}")))?;
    let id = row
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| uds_target_gate_unwired(format!("missing_requirement index={index}")))?;
    if id.trim().is_empty() {
        return Err(uds_target_gate_unwired(format!(
            "empty_requirement_id index={index}"
        )));
    }
    if !seen.insert(id.to_owned()) {
        return Err(uds_target_gate_unwired(format!(
            "duplicate_requirement={id}"
        )));
    }
    let ordered = order.get(index).and_then(Value::as_str).ok_or_else(|| {
        uds_target_gate_unwired(format!("requirement_order missing index={index}"))
    })?;
    if ordered != id {
        return Err(uds_target_gate_unwired(format!(
            "requirement_order index={index} expected={ordered} found={id}"
        )));
    }
    let command = row
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| uds_target_gate_unwired(format!("missing_command={id}")))?;
    if command.trim().is_empty() {
        return Err(uds_target_gate_unwired(format!("empty_command={id}")));
    }
    Ok(UdsTargetGateRequirement { id: id.to_owned() })
}

fn parse_projection_requirements(
    projection: &Map<String, Value>,
) -> Result<Vec<UdsTargetGateRequirement>, String> {
    let rows = projection
        .get("requirements")
        .and_then(Value::as_array)
        .ok_or_else(|| uds_target_gate_unwired("malformed_projection missing=requirements"))?;
    if rows.is_empty() {
        return Err(uds_target_gate_unwired("empty_requirements"));
    }
    let declared_count = projection
        .get("requirement_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| uds_target_gate_unwired("malformed_projection missing=requirement_count"))?;
    if declared_count != rows.len() as u64 {
        return Err(uds_target_gate_unwired(format!(
            "requirement_count declared={declared_count} actual={}",
            rows.len()
        )));
    }
    let order = projection
        .get("requirement_order")
        .and_then(Value::as_array)
        .ok_or_else(|| uds_target_gate_unwired("malformed_projection missing=requirement_order"))?;
    let mut requirements = Vec::with_capacity(rows.len());
    let mut seen = BTreeSet::new();
    for (index, row) in rows.iter().enumerate() {
        requirements.push(parse_one_requirement(row, index, order, &mut seen)?);
    }
    if order.len() != requirements.len() {
        return Err(uds_target_gate_unwired(format!(
            "requirement_order_len={} actual={}",
            order.len(),
            requirements.len()
        )));
    }
    Ok(requirements)
}

fn parse_uds_target_gate(
    result: UdsProcessResult,
    expected: &UdsTargetGateExpected,
) -> Result<UdsTargetGateObservation, String> {
    let (value, _process_exit, code) =
        parse_uds_envelope(&result, UDS_TARGET_GATE_VERB, "UDS_TARGET_GATE_UNWIRED")?;
    if code != "EC-PASS" {
        let detail = value
            .get("detail")
            .and_then(Value::as_str)
            .unwrap_or("typed UDS target-gate refusal");
        return Err(uds_target_gate_unwired(format!(
            "uds_code={code} detail={detail}"
        )));
    }
    let projection = projection_object(&value)?;
    let trigger = parse_projection_identity(projection, expected)?;
    let requirements = parse_projection_requirements(projection)?;
    Ok(UdsTargetGateObservation {
        trigger,
        requirements,
    })
}

pub(crate) async fn observe_uds_target_gate(
    cx: &Cx,
    host: &UdsObserveInput<'_>,
) -> Result<UdsTargetGateObservation, String> {
    let Some(binary) = host.binary else {
        return Err(uds_target_gate_unproven("config_missing=uds_binary"));
    };
    let Some(registry) = host.registry else {
        return Err(uds_target_gate_unproven("config_missing=uds_registry"));
    };
    cx.checkpoint()
        .map_err(|_| "CANCELLED supervisor context".to_owned())?;
    let mut version_cmd = Command::new(binary);
    version_cmd
        .args([UDS_VERSION_VERB, "--json"])
        .current_dir(host.repo)
        .env("TMUX_TMPDIR", host.tmux_tmpdir);
    let version_output = match timeout(
        cx.now_for_observability(),
        host.command_timeout,
        run_output(cx, version_cmd),
    )
    .await
    {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            return Err(uds_target_gate_unproven(format!("version_invoke={error}")));
        }
        Err(_) => {
            return Err(uds_target_gate_unproven(format!(
                "version_invoke=TIMEOUT program={} after={}s",
                binary.display(),
                host.command_timeout.as_secs()
            )));
        }
    };
    parse_uds_version(UdsProcessResult::from_output(version_output))?;
    cx.checkpoint()
        .map_err(|_| "CANCELLED supervisor context".to_owned())?;
    let mut gate_cmd = Command::new(binary);
    gate_cmd
        .arg(UDS_TARGET_GATE_VERB)
        .arg(registry)
        .arg(host.repo)
        .arg("--json")
        .current_dir(host.repo)
        .env("TMUX_TMPDIR", host.tmux_tmpdir);
    let output = match timeout(
        cx.now_for_observability(),
        host.command_timeout,
        run_output(cx, gate_cmd),
    )
    .await
    {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => return Err(uds_target_gate_unwired(format!("invoke={error}"))),
        Err(_) => {
            return Err(uds_target_gate_unwired(format!(
                "invoke=TIMEOUT program={} after={}s",
                binary.display(),
                host.command_timeout.as_secs()
            )));
        }
    };
    parse_uds_target_gate(
        UdsProcessResult::from_output(output),
        &expected_from_host(host),
    )
}

pub(crate) fn uds_cycle_admission(
    result: Result<UdsTargetGateObservation, String>,
) -> UdsCycleAdmission {
    match result {
        Ok(observation) => UdsCycleAdmission::Observed(observation),
        Err(detail) if uds_admission_is_unproven(&detail) => UdsCycleAdmission::Unproven { detail },
        Err(detail) => UdsCycleAdmission::Unwired { detail },
    }
}

pub(crate) fn uds_cycle_allows_new_claim_or_dispatch(admission: &UdsCycleAdmission) -> bool {
    matches!(admission, UdsCycleAdmission::Observed(_))
}

#[cfg(test)]
mod tests {
    use super::*;
    use asupersync::runtime::RuntimeBuilder;
    use serde_json::{json, Value};
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    struct ObserveFixture {
        temp: tempfile::TempDir,
        repo: PathBuf,
        binary: Option<PathBuf>,
        registry: Option<PathBuf>,
        tmux_tmpdir: PathBuf,
    }

    impl ObserveFixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().expect("uds fixture");
            let repo = temp.path().join("repo");
            let tmux_tmpdir = temp.path().join("tmux");
            std::fs::create_dir_all(&repo).expect("repo");
            std::fs::create_dir_all(&tmux_tmpdir).expect("tmux");
            Self {
                temp,
                repo,
                binary: None,
                registry: None,
                tmux_tmpdir,
            }
        }

        fn host(&self) -> UdsObserveInput<'_> {
            UdsObserveInput {
                binary: self.binary.as_deref(),
                registry: self.registry.as_deref(),
                repo: &self.repo,
                tmux_tmpdir: &self.tmux_tmpdir,
                command_timeout: Duration::from_secs(5),
            }
        }
    }

    fn uds_process(exit: Option<i32>, stdout: &str) -> UdsProcessResult {
        UdsProcessResult {
            process_exit: exit,
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    fn uds_expected(target: &str, root: &str, owner: &str) -> UdsTargetGateExpected {
        UdsTargetGateExpected {
            target: target.to_owned(),
            root: root.to_owned(),
            owner: owner.to_owned(),
        }
    }

    fn uds_version_json(verified: bool, advertise: bool, version: &str) -> String {
        let mut value = json!({
            "schema": "uds/v2",
            "verb": "version",
            "code": "EC-PASS",
            "exit_status": 0,
            "version": version,
            "commit_verified": verified,
        });
        if advertise {
            value["commands"] = json!([{"verb": "target-gate", "mutates": false}]);
        }
        value.to_string()
    }

    fn uds_gate_json(
        trigger: &str,
        target: &str,
        root: &str,
        owner: &str,
        ids: &[&str],
        order: &[&str],
        count: usize,
        command: &str,
    ) -> String {
        let requirements: Vec<Value> = ids
            .iter()
            .map(|id| json!({"id": id, "command": command}))
            .collect();
        json!({
            "schema": "uds/v2",
            "verb": "target-gate",
            "code": "EC-PASS",
            "exit_status": 0,
            "projection": {
                "schema": "uds-target-gates/v1",
                "trigger": trigger,
                "target": target,
                "canonical_root": root,
                "owner": owner,
                "requirement_count": count,
                "requirement_order": order,
                "requirements": requirements,
            }
        })
        .to_string()
    }

    fn assert_unproven_blocks_observer(error: String) {
        assert!(uds_admission_is_unproven(&error), "{error}");
        assert!(!error.contains("UDS_TARGET_GATE_UNWIRED"), "{error}");
        assert!(!uds_cycle_allows_new_claim_or_dispatch(
            &uds_cycle_admission(Err(error))
        ));
    }

    fn assert_unwired_blocks_observer(error: String) {
        assert!(error.starts_with("UDS_TARGET_GATE_UNWIRED "), "{error}");
        assert!(!uds_admission_is_unproven(&error), "{error}");
        assert!(!uds_cycle_allows_new_claim_or_dispatch(
            &uds_cycle_admission(Err(error))
        ));
    }

    #[test]
    fn uds_version_signal_is_unproven_not_unwired() {
        let error = parse_uds_version(uds_process(None, "{}")).unwrap_err();
        assert!(error.contains("process_exit=signal"), "{error}");
        assert_unproven_blocks_observer(error);
    }

    #[test]
    fn uds_version_malformed_json_is_unproven() {
        let error = parse_uds_version(uds_process(Some(0), "not-json")).unwrap_err();
        assert!(uds_admission_is_unproven(&error), "{error}");
        assert!(error.contains("malformed_envelope"), "{error}");
    }

    #[test]
    fn uds_version_missing_or_false_commit_verified_is_unproven() {
        let missing = json!({
            "schema": "uds/v2",
            "verb": "version",
            "code": "EC-PASS",
            "exit_status": 0,
            "version": "0.1.0",
            "commands": [{"verb": "target-gate"}],
        })
        .to_string();
        let error = parse_uds_version(uds_process(Some(0), &missing)).unwrap_err();
        assert!(uds_admission_is_unproven(&error), "{error}");
        assert!(error.contains("commit_verified"), "{error}");
        let error = parse_uds_version(uds_process(
            Some(0),
            &uds_version_json(false, true, "0.1.0"),
        ))
        .unwrap_err();
        assert!(uds_admission_is_unproven(&error), "{error}");
        assert!(error.contains("commit_verified=false"), "{error}");
    }

    #[test]
    fn uds_version_schema_or_verb_mismatch_is_unproven() {
        let schema = json!({
            "schema": "uds/v1",
            "verb": "version",
            "code": "EC-PASS",
            "exit_status": 0,
            "version": "0.1.0",
            "commit_verified": true,
            "commands": [{"verb": "target-gate"}],
        })
        .to_string();
        let error = parse_uds_version(uds_process(Some(0), &schema)).unwrap_err();
        assert!(uds_admission_is_unproven(&error), "{error}");
        assert!(error.contains("envelope_schema"), "{error}");
        let verb = json!({
            "schema": "uds/v2",
            "verb": "doctor",
            "code": "EC-PASS",
            "exit_status": 0,
            "version": "0.1.0",
            "commit_verified": true,
            "commands": [{"verb": "target-gate"}],
        })
        .to_string();
        let error = parse_uds_version(uds_process(Some(0), &verb)).unwrap_err();
        assert!(uds_admission_is_unproven(&error), "{error}");
        assert!(error.contains("envelope_verb"), "{error}");
    }

    #[test]
    fn uds_version_exit_consistency_and_unknown_version_are_unproven() {
        let mismatch = json!({
            "schema": "uds/v2",
            "verb": "version",
            "code": "EC-PASS",
            "exit_status": 1,
            "version": "0.1.0",
            "commit_verified": true,
            "commands": [{"verb": "target-gate"}],
        })
        .to_string();
        let error = parse_uds_version(uds_process(Some(0), &mismatch)).unwrap_err();
        assert!(uds_admission_is_unproven(&error), "{error}");
        assert!(error.contains("envelope_mismatch"), "{error}");
        let error = parse_uds_version(uds_process(
            Some(0),
            &uds_version_json(true, true, "unknown"),
        ))
        .unwrap_err();
        assert!(uds_admission_is_unproven(&error), "{error}");
        assert!(error.contains("unverified_version"), "{error}");
    }

    #[test]
    fn uds_version_without_target_gate_capability_is_unproven() {
        let error = parse_uds_version(uds_process(
            Some(0),
            &uds_version_json(true, false, "0.1.0"),
        ))
        .unwrap_err();
        assert!(uds_admission_is_unproven(&error), "{error}");
        assert!(
            error.contains("advertised_capability_missing=target-gate"),
            "{error}"
        );
    }

    #[test]
    fn uds_version_known_good_is_not_success_without_parser_ok() {
        parse_uds_version(uds_process(Some(0), &uds_version_json(true, true, "0.1.0")))
            .expect("verified version with advertised target-gate");
    }

    #[test]
    fn uds_version_missing_envelope_fields_are_unproven() {
        for body in [
            "{}",
            r#"{"schema":"uds/v2"}"#,
            r#"{"schema":"uds/v2","verb":"version"}"#,
            r#"{"schema":"uds/v2","verb":"version","code":"EC-PASS"}"#,
        ] {
            let error = parse_uds_version(uds_process(Some(0), body)).unwrap_err();
            assert_unproven_blocks_observer(error);
        }
    }

    #[test]
    fn uds_version_process_vs_json_exit_mismatch_is_unproven() {
        let body = json!({
            "schema": "uds/v2",
            "verb": "version",
            "code": "EC-PASS",
            "exit_status": 0,
            "version": "0.1.0",
            "commit_verified": true,
            "commands": [{"verb": "target-gate"}],
        })
        .to_string();
        let error = parse_uds_version(uds_process(Some(1), &body)).unwrap_err();
        assert_unproven_blocks_observer(error.clone());
        assert!(error.contains("envelope_mismatch"), "{error}");
    }

    #[test]
    fn uds_version_typed_nonzero_codes_are_unproven_not_unwired() {
        for (code, exit) in [("EC-RED", 1), ("EC-USAGE", 2), ("EC-UNRUN", 77)] {
            let body = json!({
                "schema": "uds/v2",
                "verb": "version",
                "code": code,
                "exit_status": exit,
                "version": "0.1.0",
                "commit_verified": true,
                "commands": [{"verb": "target-gate"}],
            })
            .to_string();
            let error = parse_uds_version(uds_process(Some(exit), &body)).unwrap_err();
            assert_unproven_blocks_observer(error.clone());
            assert!(error.contains(code), "{error}");
        }
    }

    #[test]
    fn uds_version_unverified_empty_or_nondigit_is_unproven() {
        for version in ["", " ", "unknown", "abc"] {
            let error =
                parse_uds_version(uds_process(Some(0), &uds_version_json(true, true, version)))
                    .unwrap_err();
            assert_unproven_blocks_observer(error.clone());
            assert!(error.contains("unverified_version"), "{error}");
        }
    }

    #[test]
    fn uds_target_gate_signal_and_malformed_json_are_unwired() {
        let expected = uds_expected("repo", "/tmp/repo", "repo");
        let error = parse_uds_target_gate(uds_process(None, "{}"), &expected).unwrap_err();
        assert!(error.starts_with("UDS_TARGET_GATE_UNWIRED "), "{error}");
        assert!(error.contains("process_exit=signal"), "{error}");
        let error = parse_uds_target_gate(uds_process(Some(0), "{"), &expected).unwrap_err();
        assert!(error.starts_with("UDS_TARGET_GATE_UNWIRED "), "{error}");
        assert!(error.contains("malformed_envelope"), "{error}");
    }

    #[test]
    fn uds_target_gate_schema_and_verb_mismatch_are_unwired() {
        let expected = uds_expected("repo", "/tmp/repo", "repo");
        let schema = json!({
            "schema": "nope/v2",
            "verb": "target-gate",
            "code": "EC-PASS",
            "exit_status": 0,
            "projection": {}
        })
        .to_string();
        let error = parse_uds_target_gate(uds_process(Some(0), &schema), &expected).unwrap_err();
        assert!(error.contains("envelope_schema"), "{error}");
        let verb = json!({
            "schema": "uds/v2",
            "verb": "version",
            "code": "EC-PASS",
            "exit_status": 0,
            "projection": {}
        })
        .to_string();
        let error = parse_uds_target_gate(uds_process(Some(0), &verb), &expected).unwrap_err();
        assert!(error.contains("envelope_verb"), "{error}");
    }

    #[test]
    fn uds_target_gate_exit_classes_are_fail_closed() {
        let expected = uds_expected("repo", "/tmp/repo", "repo");
        for (code, exit) in [("EC-RED", 1), ("EC-USAGE", 2), ("EC-UNRUN", 77)] {
            let body = json!({
                "schema": "uds/v2",
                "verb": "target-gate",
                "code": code,
                "exit_status": exit,
                "detail": "typed"
            })
            .to_string();
            let error =
                parse_uds_target_gate(uds_process(Some(exit), &body), &expected).unwrap_err();
            assert!(error.starts_with("UDS_TARGET_GATE_UNWIRED "), "{error}");
            assert!(error.contains(code), "{error}");
        }
        let mismatch = json!({
            "schema": "uds/v2",
            "verb": "target-gate",
            "code": "EC-PASS",
            "exit_status": 0,
        })
        .to_string();
        let error = parse_uds_target_gate(uds_process(Some(1), &mismatch), &expected).unwrap_err();
        assert!(error.contains("envelope_mismatch"), "{error}");
        let code_mismatch = json!({
            "schema": "uds/v2",
            "verb": "target-gate",
            "code": "EC-RED",
            "exit_status": 0,
        })
        .to_string();
        let error =
            parse_uds_target_gate(uds_process(Some(0), &code_mismatch), &expected).unwrap_err();
        assert!(error.contains("code_exit_mismatch"), "{error}");
    }

    #[test]
    fn uds_target_gate_unknown_code_is_unwired() {
        let expected = uds_expected("repo", "/tmp/repo", "repo");
        let body = json!({
            "schema": "uds/v2",
            "verb": "target-gate",
            "code": "EC-NOPE",
            "exit_status": 3,
        })
        .to_string();
        let error = parse_uds_target_gate(uds_process(Some(3), &body), &expected).unwrap_err();
        assert!(error.starts_with("UDS_TARGET_GATE_UNWIRED "), "{error}");
        assert!(error.contains("unsupported_code"), "{error}");
        assert!(error.contains("EC-NOPE"), "{error}");
    }

    #[test]
    fn uds_target_gate_missing_envelope_fields_are_unwired() {
        let expected = uds_expected("repo", "/tmp/repo", "repo");
        for body in [
            "{}",
            r#"{"schema":"uds/v2"}"#,
            r#"{"schema":"uds/v2","verb":"target-gate"}"#,
            r#"{"schema":"uds/v2","verb":"target-gate","code":"EC-PASS"}"#,
        ] {
            let error = parse_uds_target_gate(uds_process(Some(0), body), &expected).unwrap_err();
            assert_unwired_blocks_observer(error);
        }
    }

    #[test]
    fn uds_target_gate_process_vs_json_exit_mismatch_is_unwired() {
        let expected = uds_expected("repo", "/tmp/repo", "repo");
        let body = uds_gate_json(
            "live-trigger",
            "repo",
            "/tmp/repo",
            "repo",
            &["alpha"],
            &["alpha"],
            1,
            "echo",
        );
        let error = parse_uds_target_gate(uds_process(Some(1), &body), &expected).unwrap_err();
        assert_unwired_blocks_observer(error.clone());
        assert!(error.contains("envelope_mismatch"), "{error}");
    }

    #[test]
    fn uds_target_gate_projection_schema_or_missing_projection_is_unwired() {
        let expected = uds_expected("repo", "/tmp/repo", "repo");
        let missing = json!({
            "schema": "uds/v2",
            "verb": "target-gate",
            "code": "EC-PASS",
            "exit_status": 0,
        })
        .to_string();
        let error = parse_uds_target_gate(uds_process(Some(0), &missing), &expected).unwrap_err();
        assert_unwired_blocks_observer(error.clone());
        assert!(error.contains("malformed_projection"), "{error}");
        let bad_schema = json!({
            "schema": "uds/v2",
            "verb": "target-gate",
            "code": "EC-PASS",
            "exit_status": 0,
            "projection": {
                "schema": "uds-target-gates/v0",
                "trigger": "live-trigger",
                "target": "repo",
                "canonical_root": "/tmp/repo",
                "owner": "repo",
                "requirement_count": 1,
                "requirement_order": ["alpha"],
                "requirements": [{"id": "alpha", "command": "echo"}]
            }
        })
        .to_string();
        let error =
            parse_uds_target_gate(uds_process(Some(0), &bad_schema), &expected).unwrap_err();
        assert_unwired_blocks_observer(error.clone());
        assert!(error.contains("projection_schema"), "{error}");
    }

    #[test]
    fn uds_target_gate_identity_mismatch_is_unwired() {
        let expected = uds_expected("acme", "/tmp/repo", "repo");
        let target = uds_gate_json(
            "live-trigger",
            "other-target",
            "/tmp/repo",
            "repo",
            &["alpha"],
            &["alpha"],
            1,
            "echo",
        );
        let error = parse_uds_target_gate(uds_process(Some(0), &target), &expected).unwrap_err();
        assert!(error.contains("target_mismatch"), "{error}");
        let root = uds_gate_json(
            "live-trigger",
            "acme",
            "/tmp/other",
            "repo",
            &["alpha"],
            &["alpha"],
            1,
            "echo",
        );
        let error = parse_uds_target_gate(uds_process(Some(0), &root), &expected).unwrap_err();
        assert!(error.contains("root_mismatch"), "{error}");
        let owner = uds_gate_json(
            "live-trigger",
            "acme",
            "/tmp/repo",
            "other-owner",
            &["alpha"],
            &["alpha"],
            1,
            "echo",
        );
        let error = parse_uds_target_gate(uds_process(Some(0), &owner), &expected).unwrap_err();
        assert!(error.contains("owner_mismatch"), "{error}");
    }

    #[test]
    fn uds_target_gate_empty_trigger_or_requirements_is_unwired() {
        let expected = uds_expected("repo", "/tmp/repo", "repo");
        let empty_trigger =
            uds_gate_json("", "repo", "/tmp/repo", "repo", &["a"], &["a"], 1, "echo");
        let error =
            parse_uds_target_gate(uds_process(Some(0), &empty_trigger), &expected).unwrap_err();
        assert!(error.contains("empty_trigger"), "{error}");
        let empty_rows = json!({
            "schema": "uds/v2",
            "verb": "target-gate",
            "code": "EC-PASS",
            "exit_status": 0,
            "projection": {
                "schema": "uds-target-gates/v1",
                "trigger": "live-trigger",
                "target": "repo",
                "canonical_root": "/tmp/repo",
                "owner": "repo",
                "requirement_count": 0,
                "requirement_order": [],
                "requirements": []
            }
        })
        .to_string();
        let error =
            parse_uds_target_gate(uds_process(Some(0), &empty_rows), &expected).unwrap_err();
        assert!(error.contains("empty_requirements"), "{error}");
    }

    #[test]
    fn uds_target_gate_empty_id_or_command_is_unwired() {
        let expected = uds_expected("repo", "/tmp/repo", "repo");
        let empty_id = uds_gate_json(
            "live-trigger",
            "repo",
            "/tmp/repo",
            "repo",
            &[""],
            &[""],
            1,
            "echo",
        );
        let error = parse_uds_target_gate(uds_process(Some(0), &empty_id), &expected).unwrap_err();
        assert!(error.contains("empty_requirement_id"), "{error}");
        let empty_command = uds_gate_json(
            "live-trigger",
            "repo",
            "/tmp/repo",
            "repo",
            &["alpha"],
            &["alpha"],
            1,
            "",
        );
        let error =
            parse_uds_target_gate(uds_process(Some(0), &empty_command), &expected).unwrap_err();
        assert!(error.contains("empty_command=alpha"), "{error}");
    }

    #[test]
    fn uds_target_gate_duplicate_id_order_swap_and_count_mismatch_are_unwired() {
        let expected = uds_expected("repo", "/tmp/repo", "repo");
        let dup = uds_gate_json(
            "live-trigger",
            "repo",
            "/tmp/repo",
            "repo",
            &["a", "a"],
            &["a", "a"],
            2,
            "echo",
        );
        let error = parse_uds_target_gate(uds_process(Some(0), &dup), &expected).unwrap_err();
        assert!(error.contains("duplicate_requirement=a"), "{error}");
        let swap = uds_gate_json(
            "live-trigger",
            "repo",
            "/tmp/repo",
            "repo",
            &["a", "b"],
            &["b", "a"],
            2,
            "echo",
        );
        let error = parse_uds_target_gate(uds_process(Some(0), &swap), &expected).unwrap_err();
        assert!(error.contains("requirement_order"), "{error}");
        let count = uds_gate_json(
            "live-trigger",
            "repo",
            "/tmp/repo",
            "repo",
            &["a", "b"],
            &["a", "b"],
            1,
            "echo",
        );
        let error = parse_uds_target_gate(uds_process(Some(0), &count), &expected).unwrap_err();
        assert!(error.contains("requirement_count"), "{error}");
    }

    #[test]
    fn uds_target_gate_accepts_projection_ids_that_are_not_a_hardcoded_roster() {
        let expected = uds_expected("repo", "/tmp/repo", "repo");
        let body = uds_gate_json(
            "live-trigger",
            "repo",
            "/tmp/repo",
            "repo",
            &["alpha", "beta"],
            &["alpha", "beta"],
            2,
            "touch ./uds-command-executed",
        );
        let observed =
            parse_uds_target_gate(uds_process(Some(0), &body), &expected).expect("projection");
        assert_eq!(observed.trigger, "live-trigger");
        assert_eq!(
            observed
                .requirements
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "beta"]
        );
    }

    #[test]
    fn observe_uds_target_gate_missing_binary_is_unproven_not_success() {
        let fx = ObserveFixture::new();
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        let error = runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            observe_uds_target_gate(&cx, &fx.host()).await.unwrap_err()
        });
        assert!(uds_admission_is_unproven(&error), "{error}");
        assert!(error.contains("config_missing=uds_binary"), "{error}");
        assert!(!error.contains("UDS_TARGET_GATE_UNWIRED"), "{error}");
    }

    #[test]
    fn observe_uds_target_gate_missing_registry_is_unproven() {
        let mut fx = ObserveFixture::new();
        fx.binary = Some(fx.temp.path().join("uds.bin"));
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        let error = runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            observe_uds_target_gate(&cx, &fx.host()).await.unwrap_err()
        });
        assert_unproven_blocks_observer(error.clone());
        assert!(error.contains("config_missing=uds_registry"), "{error}");
    }

    #[test]
    fn observe_uds_target_gate_unexecutable_binary_is_unproven() {
        let mut fx = ObserveFixture::new();
        let binary = fx.temp.path().join("uds-not-exec");
        std::fs::write(&binary, "#!/bin/sh\nexit 0\n").expect("write non-exec uds");
        fx.binary = Some(binary);
        fx.registry = Some(fx.temp.path().join("registry.toml"));
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        let error = runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            observe_uds_target_gate(&cx, &fx.host()).await.unwrap_err()
        });
        assert!(uds_admission_is_unproven(&error), "{error}");
        assert!(error.contains("version_invoke="), "{error}");
        assert!(!error.contains("UDS_TARGET_GATE_UNWIRED"), "{error}");
    }

    #[test]
    fn observe_uds_target_gate_fake_executable_records_fixed_argv_in_order() {
        let mut fx = ObserveFixture::new();
        let root = fx.repo.display().to_string();
        std::fs::write(
            fx.repo.join("uds-version.json"),
            uds_version_json(true, true, "0.1.0"),
        )
        .expect("version fixture");
        std::fs::write(
            fx.repo.join("uds-gate.json"),
            uds_gate_json(
                "live-trigger",
                "repo",
                &root,
                "repo",
                &["alpha", "beta"],
                &["alpha", "beta"],
                2,
                "touch ./uds-command-executed",
            ),
        )
        .expect("gate fixture");
        let binary = fx.repo.join("uds.bin");
        std::fs::write(
            &binary,
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> ./uds-argv.log\ncase \"$1\" in\nversion) cat ./uds-version.json; exit 0 ;;\ntarget-gate) cat ./uds-gate.json; exit 0 ;;\n*) exit 2 ;;\nesac\n",
        )
        .expect("fake uds");
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).expect("mode");
        fx.binary = Some(binary);
        let registry = fx.temp.path().join("registry.toml");
        std::fs::write(&registry, "").expect("registry");
        fx.registry = Some(registry.clone());
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        let observed = runtime
            .block_on(async {
                let cx = Cx::current().expect("runtime context");
                observe_uds_target_gate(&cx, &fx.host()).await
            })
            .expect("fake UDS must succeed");
        let argv = std::fs::read_to_string(fx.repo.join("uds-argv.log")).expect("argv log");
        let expected_argv = format!(
            "version --json\ntarget-gate {} {} --json\n",
            registry.display(),
            fx.repo.display()
        );
        assert_eq!(argv, expected_argv, "{argv}");
        assert!(
            !argv.contains("--target"),
            "49365ad target-gate is positional; --target must not be emitted: {argv}"
        );
        assert_eq!(observed.trigger, "live-trigger");
        assert_eq!(observed.requirements.len(), 2);
        assert_eq!(observed.requirements[0].id, "alpha");
        assert!(
            !fx.repo.join("uds-command-executed").exists(),
            "projection commands must not execute"
        );
    }

    #[test]
    fn observe_uds_target_gate_rejects_unproven_version_envelope() {
        let mut fx = ObserveFixture::new();
        std::fs::write(
            fx.repo.join("uds-version.json"),
            json!({
                "schema": "uds/v2",
                "verb": "version",
                "code": "EC-RED",
                "exit_status": 1,
                "version": "0.1.0",
                "commit_verified": true,
                "commands": [{"verb": "target-gate"}],
            })
            .to_string(),
        )
        .expect("version fixture");
        let binary = fx.repo.join("uds-unproven.bin");
        std::fs::write(&binary, "#!/bin/sh\ncat ./uds-version.json\nexit 1\n")
            .expect("unproven uds");
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).expect("mode");
        fx.binary = Some(binary);
        fx.registry = Some(fx.temp.path().join("registry.toml"));
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        let error = runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            observe_uds_target_gate(&cx, &fx.host()).await.unwrap_err()
        });
        assert_unproven_blocks_observer(error);
    }

    #[test]
    fn observe_uds_target_gate_rejects_unwired_projection_envelope() {
        let mut fx = ObserveFixture::new();
        std::fs::write(
            fx.repo.join("uds-version.json"),
            uds_version_json(true, true, "0.1.0"),
        )
        .expect("version fixture");
        std::fs::write(
            fx.repo.join("uds-gate.json"),
            json!({
                "schema": "uds/v2",
                "verb": "target-gate",
                "code": "EC-PASS",
                "exit_status": 0,
            })
            .to_string(),
        )
        .expect("gate fixture");
        let binary = fx.repo.join("uds-unwired.bin");
        std::fs::write(
            &binary,
            "#!/bin/sh\ncase \"$1\" in\nversion) cat ./uds-version.json; exit 0 ;;\ntarget-gate) cat ./uds-gate.json; exit 0 ;;\n*) exit 2 ;;\nesac\n",
        )
        .expect("unwired uds");
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).expect("mode");
        fx.binary = Some(binary);
        fx.registry = Some(fx.temp.path().join("registry.toml"));
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        let error = runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            observe_uds_target_gate(&cx, &fx.host()).await.unwrap_err()
        });
        assert_unwired_blocks_observer(error);
    }

    #[test]
    fn uds_cycle_admission_blocks_new_claim_or_dispatch_unless_observed() {
        let expected = uds_expected("repo", "/tmp/repo", "repo");
        let ok = parse_uds_target_gate(
            uds_process(
                Some(0),
                &uds_gate_json(
                    "live-trigger",
                    "repo",
                    "/tmp/repo",
                    "repo",
                    &["alpha"],
                    &["alpha"],
                    1,
                    "echo",
                ),
            ),
            &expected,
        )
        .expect("ok projection");
        assert!(uds_cycle_allows_new_claim_or_dispatch(
            &uds_cycle_admission(Ok(ok))
        ));
        assert!(!uds_cycle_allows_new_claim_or_dispatch(
            &uds_cycle_admission(Err(uds_target_gate_unproven("config_missing=uds_binary")))
        ));
        assert!(!uds_cycle_allows_new_claim_or_dispatch(
            &uds_cycle_admission(Err(uds_target_gate_unwired("empty_trigger")))
        ));
    }
}

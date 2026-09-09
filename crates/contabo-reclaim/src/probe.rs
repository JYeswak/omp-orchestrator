#![forbid(unsafe_code)]

use crate::model::{
    decide_guards, parse_listing, validate_candidate, ActiveBuild, CandidateSet, ControlSnapshot,
    EntryKind, GuardDecision, ReclaimError, ReclaimMode, ReclaimRefusal, ReclaimReport,
    RemoteProcessObservation, RunOutcome, WorkerSpec,
};
use asupersync::process::Command;
use asupersync::time::timeout;
use asupersync::Cx;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;
use subprocess_contract::{run_output, RunError};

const PROBE_DEADLINE: Duration = Duration::from_secs(30);
const RCH_STATUS_ARGS: &[&str] = &["status", "--json"];
const PGPREP_PATTERNS: &[&str] = &["[c]argo", "[r]ustc"];

#[derive(Debug, Clone)]
pub struct Config {
    pub worker: WorkerSpec,
    pub base: PathBuf,
    pub mode: ReclaimMode,
}

pub async fn run(cx: &Cx, config: &Config) -> Result<ReclaimReport, ReclaimError> {
    validate_base(&config.base)?;
    cx.checkpoint().map_err(|_| ReclaimError::Runtime {
        detail: "cancelled before control probe".to_owned(),
    })?;

    let mut report = ReclaimReport::new(config.worker, config.mode);
    let control = match control_snapshot(cx, config.worker).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            report.outcome = RunOutcome::Unknown;
            report.detail = error.to_string();
            return Ok(report);
        }
    };
    if !control.matches_worker(config.worker) {
        report.outcome = RunOutcome::Unknown;
        report.detail = format!(
            "CONTROL_HOST_MISMATCH worker={} selected_host={} control_host={}",
            config.worker.id, config.worker.host, control.worker_host
        );
        return Ok(report);
    }
    report.guards.push(format!(
        "control-plane worker={} host={} status={} active_builds={} used_slots={}",
        control.worker_id,
        control.worker_host,
        control.worker_status,
        control.active_builds.len(),
        control.used_slots
    ));
    let remote = remote_processes(cx, config.worker).await;
    append_process_guard(&mut report, &remote);
    match decide_guards(&control, &remote) {
        GuardDecision::Authorized => {}
        GuardDecision::SkippedLiveBuild { detail } => {
            report.outcome = RunOutcome::SkippedLiveBuild;
            report.detail = detail;
            return Ok(report);
        }
        GuardDecision::Unknown { detail } => {
            report.outcome = RunOutcome::Unknown;
            report.detail = detail;
            return Ok(report);
        }
        GuardDecision::Unreachable { detail } => {
            report.outcome = RunOutcome::Unreachable;
            report.detail = detail;
            return Ok(report);
        }
    }

    let listing = match list_candidates(cx, config.worker, &config.base).await {
        Ok(listing) => listing,
        Err(error) => {
            report.outcome = RunOutcome::Unreachable;
            report.detail = error.to_string();
            return Ok(report);
        }
    };
    let candidates = match parse_listing(&listing) {
        Ok(CandidateSet::Empty) => {
            report.outcome = RunOutcome::AlreadyClean;
            report.detail =
                "EMPTY_CANDIDATE_SET: no whitelisted artifact entries under base".to_owned();
            return Ok(report);
        }
        Ok(CandidateSet::NonEmpty(candidates)) => candidates,
        Err(error) => {
            report.outcome = RunOutcome::Unknown;
            report.detail = error.to_string();
            return Ok(report);
        }
    };

    let mut valid = Vec::with_capacity(candidates.len());
    let mut refusals = Vec::new();
    for candidate in candidates {
        cx.checkpoint().map_err(|_| ReclaimError::Runtime {
            detail: "cancelled during candidate validation".to_owned(),
        })?;
        let resolved = if candidate.kind == EntryKind::Symlink {
            match remote_realpath(cx, config.worker, &candidate.path).await {
                Ok(path) => Some(path),
                Err(error) => {
                    refusals.push(ReclaimRefusal {
                        path: candidate.path,
                        reason: crate::model::RefusalReason::SymlinkTargetUnreadable,
                        detail: error.to_string(),
                    });
                    continue;
                }
            }
        } else {
            None
        };
        match validate_candidate(&config.base, candidate, resolved.as_deref()) {
            Ok(candidate) => valid.push(candidate),
            Err(refusal) => refusals.push(refusal),
        }
    }
    report.candidates = valid
        .iter()
        .map(|candidate| candidate.candidate.path.display().to_string())
        .collect();
    report.refused = refusals.iter().map(ToString::to_string).collect();
    if !refusals.is_empty() {
        report.outcome = RunOutcome::Refused;
        report.detail = "one or more candidates failed whitelist or containment".to_owned();
        return Ok(report);
    }

    report.bytes = valid
        .iter()
        .map(|candidate| candidate.candidate.bytes)
        .sum();
    report.directories = valid.len();
    if config.mode == ReclaimMode::DryRun {
        report.outcome = RunOutcome::Planned;
        report.detail =
            "DRY_RUN: no deletion requested; every candidate passed both guards".to_owned();
        return Ok(report);
    }

    for candidate in &valid {
        cx.checkpoint().map_err(|_| ReclaimError::Runtime {
            detail: "cancelled before deletion".to_owned(),
        })?;
        let control = match control_snapshot(cx, config.worker).await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                report.outcome = RunOutcome::Unknown;
                report.detail = format!("before_delete={error}");
                return Ok(report);
            }
        };
        if !control.matches_worker(config.worker) {
            report.outcome = RunOutcome::Unknown;
            report.detail = format!(
                "before_delete=CONTROL_HOST_MISMATCH worker={} selected_host={} control_host={}",
                config.worker.id, config.worker.host, control.worker_host
            );
            return Ok(report);
        }
        let remote = remote_processes(cx, config.worker).await;
        match decide_guards(&control, &remote) {
            GuardDecision::Authorized => {}
            GuardDecision::SkippedLiveBuild { detail } => {
                report.outcome = RunOutcome::SkippedLiveBuild;
                report.detail = format!("before_delete={detail}");
                return Ok(report);
            }
            GuardDecision::Unknown { detail } => {
                report.outcome = RunOutcome::Unknown;
                report.detail = format!("before_delete={detail}");
                return Ok(report);
            }
            GuardDecision::Unreachable { detail } => {
                report.outcome = RunOutcome::Unreachable;
                report.detail = format!("before_delete={detail}");
                return Ok(report);
            }
        }
        delete_candidate(cx, config.worker, &candidate.candidate.path).await?;
    }
    report.outcome = RunOutcome::Reclaimed;
    report.detail = "APPLY: all candidates re-authorized immediately before deletion".to_owned();
    Ok(report)
}

fn validate_base(base: &Path) -> Result<(), ReclaimError> {
    if !base.is_absolute() {
        return Err(ReclaimError::InvalidBase {
            detail: format!("base={} must be absolute", base.display()),
        });
    }
    if base
        .components()
        .any(|component| component == std::path::Component::ParentDir)
    {
        return Err(ReclaimError::InvalidBase {
            detail: "base contains parent traversal".to_owned(),
        });
    }
    if base.to_string_lossy().chars().any(|character| {
        character.is_whitespace() || matches!(character, ';' | '&' | '|' | '`' | '$' | '>' | '<')
    }) {
        return Err(ReclaimError::InvalidBase {
            detail: "base contains shell-significant whitespace or metacharacters".to_owned(),
        });
    }
    Ok(())
}

#[derive(Debug)]
struct CapturedOutput {
    success: bool,
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

async fn run_command(
    cx: &Cx,
    command: Command,
    worker: &str,
    operation: &'static str,
) -> Result<CapturedOutput, ReclaimError> {
    cx.checkpoint().map_err(|_| ReclaimError::Runtime {
        detail: format!("cancelled before {operation}"),
    })?;
    match timeout(
        cx.now_for_observability(),
        PROBE_DEADLINE,
        run_output(cx, command),
    )
    .await
    {
        Ok(Ok(output)) => {
            cx.checkpoint().map_err(|_| ReclaimError::Runtime {
                detail: format!("cancelled after {operation}"),
            })?;
            Ok(CapturedOutput {
                success: output.status.success(),
                code: output.status.code(),
                stdout: output.stdout,
                stderr: output.stderr,
            })
        }
        Ok(Err(RunError::Timeout)) | Err(_) => Err(ReclaimError::Timeout {
            worker: worker.to_owned(),
            operation,
        }),
        Ok(Err(error)) => Err(ReclaimError::Probe {
            worker: worker.to_owned(),
            detail: format!("operation={operation} {error}"),
        }),
    }
}

fn target(worker: WorkerSpec) -> String {
    format!("root@{}", worker.host)
}

fn ssh(worker: WorkerSpec, args: &[&str]) -> Command {
    let mut command = Command::new("ssh");
    command.args([
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=5",
        &target(worker),
    ]);
    command.args(args);
    command
}

async fn control_snapshot(cx: &Cx, worker: WorkerSpec) -> Result<ControlSnapshot, ReclaimError> {
    let mut command = Command::new("rch");
    command.args(RCH_STATUS_ARGS);
    let output = run_command(cx, command, worker.id, "rch-status").await?;
    if !output.success {
        return Err(ReclaimError::Output {
            worker: worker.id.to_owned(),
            operation: "rch-status",
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    parse_control_snapshot(&output.stdout, worker)
}

pub fn parse_control_snapshot(
    bytes: &[u8],
    worker: WorkerSpec,
) -> Result<ControlSnapshot, ReclaimError> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|error| ReclaimError::MalformedStatus {
            detail: format!("invalid JSON: {error}"),
        })?;
    if value.get("success").and_then(Value::as_bool) != Some(true) {
        return Err(ReclaimError::MalformedStatus {
            detail: "control-plane status did not report success=true".to_owned(),
        });
    }
    let daemon = value
        .get("data")
        .and_then(|data| data.get("daemon"))
        .ok_or_else(|| ReclaimError::MalformedStatus {
            detail: "missing data.daemon".to_owned(),
        })?;
    let workers = daemon
        .get("workers")
        .and_then(Value::as_array)
        .ok_or_else(|| ReclaimError::MalformedStatus {
            detail: "missing data.daemon.workers array".to_owned(),
        })?;
    let worker_value = workers
        .iter()
        .find(|value| value.get("id").and_then(Value::as_str) == Some(worker.id))
        .ok_or_else(|| ReclaimError::MalformedStatus {
            detail: format!("worker {} absent from control-plane status", worker.id),
        })?;
    let worker_status = worker_value
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| ReclaimError::MalformedStatus {
            detail: format!("worker {} missing status", worker.id),
        })?
        .to_owned();
    let worker_host = worker_value
        .get("host")
        .and_then(Value::as_str)
        .ok_or_else(|| ReclaimError::MalformedStatus {
            detail: format!("worker {} missing host", worker.id),
        })?
        .to_owned();
    let used_slots = worker_value
        .get("used_slots")
        .and_then(Value::as_u64)
        .ok_or_else(|| ReclaimError::MalformedStatus {
            detail: format!("worker {} missing used_slots", worker.id),
        })?;
    let active_values = daemon
        .get("active_builds")
        .and_then(Value::as_array)
        .ok_or_else(|| ReclaimError::MalformedStatus {
            detail: "missing data.daemon.active_builds array".to_owned(),
        })?;
    let mut active_builds = Vec::new();
    for value in active_values {
        let worker_id = value
            .get("worker_id")
            .and_then(Value::as_str)
            .ok_or_else(|| ReclaimError::MalformedStatus {
                detail: "active build missing worker_id".to_owned(),
            })?;
        if worker_id != worker.id {
            continue;
        }
        let id = value
            .get("id")
            .map(|id| id.to_string())
            .unwrap_or_else(|| "<missing-id>".to_owned());
        active_builds.push(ActiveBuild {
            id,
            worker_id: worker_id.to_owned(),
        });
    }
    Ok(ControlSnapshot {
        worker_id: worker.id.to_owned(),
        worker_host,
        worker_status,
        used_slots,
        active_builds,
    })
}

pub fn parse_process_probe(
    code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
    pattern: &str,
) -> RemoteProcessObservation {
    let lines: Vec<String> = String::from_utf8_lossy(stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_owned)
        .collect();
    match code {
        Some(0) if !lines.is_empty() => RemoteProcessObservation::Present { lines },
        Some(0) => RemoteProcessObservation::Unavailable {
            detail: format!("pgrep pattern={pattern} exited 0 without process rows"),
        },
        Some(1) if lines.is_empty() => RemoteProcessObservation::Empty,
        Some(1) => RemoteProcessObservation::Present { lines },
        Some(code) => RemoteProcessObservation::Unavailable {
            detail: format!(
                "pgrep pattern={pattern} exit={code} stderr={}",
                String::from_utf8_lossy(stderr).trim()
            ),
        },
        None => RemoteProcessObservation::Unavailable {
            detail: format!("pgrep pattern={pattern} terminated by signal"),
        },
    }
}

async fn remote_processes(cx: &Cx, worker: WorkerSpec) -> RemoteProcessObservation {
    let mut lines = Vec::new();
    for pattern in PGPREP_PATTERNS {
        let output = match run_command(
            cx,
            ssh(worker, &["pgrep", "-a", "-f", pattern]),
            worker.id,
            "remote-process-probe",
        )
        .await
        {
            Ok(output) => output,
            Err(error) => {
                return RemoteProcessObservation::Unavailable {
                    detail: error.to_string(),
                }
            }
        };
        match parse_process_probe(output.code, &output.stdout, &output.stderr, pattern) {
            RemoteProcessObservation::Empty => {}
            RemoteProcessObservation::Present { lines: found } => lines.extend(found),
            RemoteProcessObservation::Unavailable { detail } => {
                return RemoteProcessObservation::Unavailable { detail }
            }
        }
    }
    lines.sort();
    lines.dedup();
    if lines.is_empty() {
        RemoteProcessObservation::Empty
    } else {
        RemoteProcessObservation::Present { lines }
    }
}

fn append_process_guard(report: &mut ReclaimReport, remote: &RemoteProcessObservation) {
    match remote {
        RemoteProcessObservation::Empty => report
            .guards
            .push("remote cargo/rustc processes=0".to_owned()),
        RemoteProcessObservation::Present { lines } => report
            .guards
            .push(format!("remote cargo/rustc processes={}", lines.len())),
        RemoteProcessObservation::Unavailable { detail } => report
            .guards
            .push(format!("remote process probe=UNKNOWN {detail}")),
    }
}

async fn list_candidates(cx: &Cx, worker: WorkerSpec, base: &Path) -> Result<String, ReclaimError> {
    let base = base.to_string_lossy().into_owned();
    let output = run_command(
        cx,
        ssh(
            worker,
            &[
                "find",
                &base,
                "-mindepth",
                "1",
                "-maxdepth",
                "1",
                "\\(",
                "-name",
                ".rch-target",
                "-o",
                "-name",
                ".rch-target-\\*",
                "-o",
                "-name",
                ".rch-tmp",
                "-o",
                "-name",
                "\\*-mut",
                "-o",
                "-name",
                "grade-\\*",
                "-o",
                "-name",
                ".grade-\\*",
                "\\)",
                "-printf",
                "'%p\\t%y\\t%s\\n'",
            ],
        ),
        worker.id,
        "candidate-list",
    )
    .await?;
    if !output.success {
        return Err(ReclaimError::Output {
            worker: worker.id.to_owned(),
            operation: "candidate-list",
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

async fn remote_realpath(
    cx: &Cx,
    worker: WorkerSpec,
    path: &Path,
) -> Result<PathBuf, ReclaimError> {
    let path = path.to_string_lossy().into_owned();
    let output = run_command(
        cx,
        ssh(worker, &["realpath", "-e", "--", &path]),
        worker.id,
        "symlink-realpath",
    )
    .await?;
    if !output.success {
        return Err(ReclaimError::Output {
            worker: worker.id.to_owned(),
            operation: "symlink-realpath",
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    let resolved = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if resolved.is_empty() {
        return Err(ReclaimError::Probe {
            worker: worker.id.to_owned(),
            detail: "realpath returned an empty target".to_owned(),
        });
    }
    Ok(PathBuf::from(resolved))
}

async fn delete_candidate(cx: &Cx, worker: WorkerSpec, path: &Path) -> Result<(), ReclaimError> {
    let path = path.to_string_lossy().into_owned();
    let output = run_command(
        cx,
        ssh(worker, &["rm", "-rf", "--", &path]),
        worker.id,
        "delete-candidate",
    )
    .await?;
    if output.success {
        Ok(())
    } else {
        Err(ReclaimError::Output {
            worker: worker.id.to_owned(),
            operation: "delete-candidate",
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worker() -> WorkerSpec {
        WorkerSpec {
            id: "contabo-2",
            host: "94.72.121.46",
        }
    }

    #[test]
    fn control_status_requires_both_authority_fields() {
        let json = br#"{
            "success": true,
            "data": {
                "daemon": {
                    "workers": [{"id":"contabo-2","host":"94.72.121.46","status":"healthy","used_slots":0}],
                    "active_builds": []
                }
            }
        }"#;
        let snapshot = parse_control_snapshot(json, worker()).expect("valid status");
        assert!(snapshot.is_clear());
        let missing = br#"{"success":true,"data":{"daemon":{"workers":[]}}}"#;
        let error = parse_control_snapshot(missing, worker()).expect_err("missing fields");
        assert!(error.to_string().contains("worker contabo-2 absent"));
    }
    #[test]
    fn process_probe_distinguishes_empty_present_and_unknown() {
        assert_eq!(
            parse_process_probe(Some(1), b"", b"", "[c]argo"),
            RemoteProcessObservation::Empty
        );
        assert_eq!(
            parse_process_probe(Some(0), b"123 cargo --build\n", b"", "[c]argo"),
            RemoteProcessObservation::Present {
                lines: vec!["123 cargo --build".to_owned()]
            }
        );
        assert!(matches!(
            parse_process_probe(Some(2), b"", b"permission denied", "[c]argo"),
            RemoteProcessObservation::Unavailable { .. }
        ));
    }
}

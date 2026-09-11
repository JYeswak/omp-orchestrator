#![forbid(unsafe_code)]

//! S1 `ompo doctor` probe loop and lifecycle-event writer.
//!
//! Every probe decision is emitted in declaration order with a non-empty reason
//! code and a reference to the report artifact that later L1 beads populate.
//! The shared lifecycle writer performs fsync plus readback; this crate refuses
//! a missing decision instead of silently accepting a partial event set.

use lifecycle_event::{
    default_repo_journal, emit_host, DurableJournal, EmitError, EmitOutcome, Layer, LifecycleEvent,
    Readback, ReasonCode,
};
use serde::Serialize;
use std::fmt;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use ompo_start::inception;
use subprocess_contract::{bounded_output, BoundedOutcome};

pub mod adapter_exec;
pub mod health_repair;
pub(crate) mod revision_env;
pub mod liveness;
pub mod omp_messages;
pub mod omp_process;
pub mod omp_state;
pub mod omp_stats;
pub mod provenance;
pub mod selfdoc;
pub mod state_triad;
pub mod umbrella;
pub mod upstream_report;

pub const ARTIFACT_REFERENCE: &str = ".omp-orchestrator/doctor/report.json";
const PROBE_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeSpec {
    pub name: &'static str,
    pub command: &'static str,
    pub args: &'static [&'static str],
}

/// L1's declared system probes. The list is stable so event ordering is stable.
pub const PROBES: &[ProbeSpec] = &[
    ProbeSpec {
        name: "tmux",
        command: "tmux",
        args: &["-V"],
    },
    ProbeSpec {
        name: "ntm",
        command: "ntm",
        args: &["--version"],
    },
    ProbeSpec {
        name: "br",
        command: "br",
        args: &["--version"],
    },
    ProbeSpec {
        name: "bv",
        command: "bv",
        args: &["--version"],
    },
    ProbeSpec {
        name: "agent-mail",
        command: "am",
        args: &["--version"],
    },
    ProbeSpec {
        name: "socraticode",
        command: "socraticode",
        args: &["--version"],
    },
    ProbeSpec {
        name: "rch",
        command: "rch",
        args: &["--version"],
    },
    ProbeSpec {
        name: "git",
        command: "git",
        args: &["--version"],
    },
    ProbeSpec {
        name: "disk",
        command: "df",
        args: &["-P", "."],
    },
    ProbeSpec {
        name: "frankenmermaid",
        command: "frankenmermaid",
        args: &["--version"],
    },
    ProbeSpec {
        name: "toolchain",
        command: "rustc",
        args: &["--version"],
    },
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProbeDecision {
    pub name: String,
    pub status: String,
    pub reason_code: String,
    pub detail: String,
    /// Independent PATH evidence. A present executable is not proof that its probe answered.
    pub presence: Option<String>,
    /// The successful probe's first non-empty version/identity line. None is not a version.
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DoctorSummary {
    pub schema: &'static str,
    pub run_id: String,
    pub scope: String,
    pub status: &'static str,
    pub exit_code: u8,
    pub artifact: &'static str,
    pub lifecycle_journal: PathBuf,
    pub probe_count: usize,
    pub event_count: usize,
    pub readback_lines: usize,
    pub probes: Vec<ProbeDecision>,
    pub remediation: Vec<String>,
    pub next_action: String,
    /// Verified readback of the on-disk artifact this report names. `None` is
    /// the pre-write state and the value serialized INTO the artifact; a
    /// returned summary always carries `Some`, because `next_action` promises a
    /// readback and an unverified write is not one.
    pub report: Option<ReportReadback>,
    /// L1's probe-answer metric WITH a verdict. `None` for scopes that run no
    /// probe loop, so an absent measurement is not a zero-valued green one.
    pub metric: Option<ProbeAnswerMetric>,
}

#[derive(Debug)]
pub enum DoctorError {
    EmptyProbeSet,
    MissingProbeEvent { probe: &'static str },
    UnsupportedScope(String),
    CurrentDirectory(std::io::Error),
    Emit(EmitError),
    ReportWrite { path: PathBuf, detail: String },
    ReportReadbackAbsent { path: PathBuf },
    ReportReadbackMismatch { path: PathBuf, detail: String },
}

impl fmt::Display for DoctorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProbeSet => f.write_str(
                "L1_DOCTOR_EMPTY_PROBE_SET — no probe decisions; lifecycle emission is refused",
            ),
            Self::MissingProbeEvent { probe } => write!(
                f,
                "L1_DOCTOR_MISSING_LIFECYCLE_EVENT probe={probe} — partial probe output is refused"
            ),
            Self::UnsupportedScope(scope) => {
                write!(f, "L1_DOCTOR_UNSUPPORTED_SCOPE scope={scope}")
            }
            Self::CurrentDirectory(error) => write!(f, "L1_DOCTOR_CURRENT_DIRECTORY error={error}"),
            Self::Emit(error) => write!(f, "L1_DOCTOR_LIFECYCLE_EMIT error={error}"),
            Self::ReportWrite { path, detail } => write!(
                f,
                "L1_DOCTOR_REPORT_WRITE path={} detail={detail}",
                path.display()
            ),
            Self::ReportReadbackAbsent { path } => write!(
                f,
                "L1_DOCTOR_REPORT_READBACK_ABSENT path={} — the write reported success but the artifact is not readable; a promised readback is not a readback",
                path.display()
            ),
            Self::ReportReadbackMismatch { path, detail } => write!(
                f,
                "L1_DOCTOR_REPORT_READBACK_MISMATCH path={} detail={detail}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for DoctorError {}

impl From<EmitError> for DoctorError {
    fn from(error: EmitError) -> Self {
        Self::Emit(error)
    }
}

fn first_output_line(output: &std::process::Output) -> Option<String> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    stdout
        .lines()
        .chain(stderr.lines())
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().chars().take(240).collect())
}

fn detail_from_output(output: &std::process::Output) -> String {
    first_output_line(output).unwrap_or_else(|| "no version output".to_owned())
}

fn reason_code(name: &str, status: &str) -> String {
    format!(
        "L1_PROBE_{}_{}",
        name.replace('-', "_").to_ascii_uppercase(),
        status
    )
}

fn run_probe(spec: &ProbeSpec) -> ProbeDecision {
    let presence = crate::adapter_exec::resolve_on_path(spec.command)
        .map(|path| path.display().to_string());
    let mut command = Command::new(spec.command);
    command.args(spec.args);
    let (status, detail, version) = match bounded_output(&mut command, PROBE_DEADLINE) {
        BoundedOutcome::Completed(output) => {
            let version = output
                .status
                .success()
                .then(|| first_output_line(&output))
                .flatten();
            let detail = format!("exit={} {}", output.status, detail_from_output(&output));
            if presence.is_some() {
                match version {
                    Some(version) => ("OK", version.clone(), Some(version)),
                    None => ("UNPROBEABLE", detail, None),
                }
            } else {
                ("ABSENT_SPECIFIC", detail, None)
            }
        }
        BoundedOutcome::TimedOut => ("UNMEASURED", "probe timed out".to_owned(), None),
        BoundedOutcome::Unspawned(error) => (
            if presence.is_some() { "UNPROBEABLE" } else { "ABSENT_SPECIFIC" },
            error.to_string(),
            None,
        ),
    };
    ProbeDecision {
        name: spec.name.to_owned(),
        status: status.to_owned(),
        reason_code: reason_code(spec.name, status),
        detail,
        presence,
        version,
    }
}
/// Map the completed probe set onto the doctor's two subject-result bands.
///
/// 0 means every probe established both independent signals. 1 means the doctor ran but at
/// least one subject was absent, unprobeable, stale, or otherwise not OK. An empty set is an
/// instrument error, never a healthy result.
pub fn doctor_exit_code(decisions: &[ProbeDecision]) -> Result<u8, DoctorError> {
    if decisions.is_empty() {
        return Err(DoctorError::EmptyProbeSet);
    }
    Ok(if decisions.iter().all(|decision| {
        decision.status == "OK"
            && decision.presence.as_ref().is_some_and(|value| !value.is_empty())
            && decision.version.as_ref().is_some_and(|value| !value.is_empty())
    }) {
        0
    } else {
        1
    })
}

/// L1's probe-answer metric, emitted WITH a verdict.
///
/// WHY THIS SHAPE AND NOT A SEVENTH `METRICS.toml` ROW: `load_metrics` refuses
/// any row count other than `EXPECTED_METRIC_COUNT` (= 6,
/// lifecycle-monitor/src/lib.rs:128,173), so adding a row would break every
/// caller of it. L1's row (`MET-L1-REPAIR-ACTION-RATE`) already exists; what
/// was missing was an EMISSION carrying the ratio, its denominator, the named
/// UNPROBEABLE set, and a verdict. Naming a metric is not an expectation, and a
/// metric with no denominator greens forever.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProbeAnswerMetric {
    pub id: &'static str,
    /// How many probes L1 DECLARES. The denominator is published, never implied.
    pub probes_declared: usize,
    /// How many declared probes actually ANSWERED — presence AND version.
    pub probes_answered: usize,
    /// `None` whenever the metric is UNMEASURED. A zero ratio is a real
    /// measurement; a missing one must never be rendered as 0.0.
    pub ratio: Option<f64>,
    pub floor: f64,
    /// The UNPROBEABLE subjects BY NAME. A count alone cannot be acted on.
    pub unprobeable: Vec<String>,
    /// Subjects whose version is present but outside the required floor
    /// (contract L1P-WRONG-VERSION-STALE). `None` while no version floor is
    /// declared for any probe: reporting 0 there would be a green-by-
    /// construction zero for a band that cannot currently be entered.
    pub stale_count: Option<usize>,
    pub stale_status: String,
    pub verdict: String,
    pub reason_code: String,
}

pub const PROBE_ANSWER_METRIC_ID: &str = "MET-L1-PROBE-ANSWER-RATE";

/// Names the STALE band's measurability. `PROBES` declares no version floor, so
/// the band is unreachable and its count is UNMEASURED rather than 0.
fn stale_band(decisions: &[ProbeDecision]) -> (Option<usize>, String) {
    let observed = decisions
        .iter()
        .filter(|decision| decision.status == "STALE")
        .count();
    if observed > 0 {
        (Some(observed), "MEASURED".to_owned())
    } else {
        (
            None,
            "UNMEASURED_NO_VERSION_FLOOR_DECLARED".to_owned(),
        )
    }
}

/// Compute the probe-answer metric from the declared set and the results.
///
/// A missing declared result or an empty result set is UNMEASURED with a reason
/// — never a zero-valued green metric.
pub fn probe_answer_metric(
    expected: &[ProbeSpec],
    decisions: &[ProbeDecision],
) -> ProbeAnswerMetric {
    let declared = expected.len();
    let unprobeable: Vec<String> = decisions
        .iter()
        .filter(|decision| decision.status == "UNPROBEABLE")
        .map(|decision| decision.name.clone())
        .collect();
    let (stale_count, stale_status) = stale_band(decisions);
    let mut metric = ProbeAnswerMetric {
        id: PROBE_ANSWER_METRIC_ID,
        probes_declared: declared,
        probes_answered: 0,
        ratio: None,
        floor: 1.0,
        unprobeable,
        stale_count,
        stale_status,
        verdict: String::new(),
        reason_code: String::new(),
    };
    if declared == 0 {
        metric.verdict = "ERROR".to_owned();
        metric.reason_code = "L1_METRIC_NO_DECLARED_PROBES".to_owned();
        return metric;
    }
    if decisions.is_empty() {
        // Contract LAW-L1-UNKNOWN: no authoritative observation is UNMEASURED
        // with UNKNOWN_NO_RECORD, not a 0/N green.
        metric.verdict = "UNMEASURED".to_owned();
        metric.reason_code = "UNKNOWN_NO_RECORD".to_owned();
        return metric;
    }
    let missing: Vec<&str> = expected
        .iter()
        .filter(|spec| {
            !decisions
                .iter()
                .any(|decision| decision.name == spec.name)
        })
        .map(|spec| spec.name)
        .collect();
    if !missing.is_empty() {
        metric.verdict = "UNMEASURED".to_owned();
        metric.reason_code = format!("L1_METRIC_PARTIAL_PROBE_SET missing={}", missing.join(","));
        return metric;
    }
    let answered = expected
        .iter()
        .filter(|spec| {
            decisions.iter().any(|decision| {
                decision.name == spec.name
                    && decision.status == "OK"
                    && decision.presence.as_ref().is_some_and(|v| !v.is_empty())
                    && decision.version.as_ref().is_some_and(|v| !v.is_empty())
            })
        })
        .count();
    metric.probes_answered = answered;
    let ratio = answered as f64 / declared as f64;
    metric.ratio = Some(ratio);
    if ratio >= metric.floor {
        metric.verdict = "MEASURED_OK".to_owned();
        metric.reason_code = "L1_METRIC_ALL_DECLARED_PROBES_ANSWERED".to_owned();
    } else {
        metric.verdict = "MEASURED_BELOW_FLOOR".to_owned();
        metric.reason_code = format!("L1_METRIC_BELOW_FLOOR answered={answered} declared={declared}");
    }
    metric
}

/// Build the one-event-per-decision lifecycle batch in declaration order.
pub fn lifecycle_events(
    expected: &[ProbeSpec],
    decisions: &[ProbeDecision],
) -> Result<Vec<LifecycleEvent>, DoctorError> {
    if decisions.is_empty() {
        return Err(DoctorError::EmptyProbeSet);
    }
    let mut events = Vec::with_capacity(expected.len());
    for spec in expected {
        let decision = decisions
            .iter()
            .find(|decision| decision.name == spec.name)
            .ok_or(DoctorError::MissingProbeEvent { probe: spec.name })?;
        let reason = ReasonCode::new(decision.reason_code.clone()).map_err(DoctorError::Emit)?;
        let blocker = if decision.status == "OK" {
            String::new()
        } else {
            decision.detail.clone()
        };
        let event = LifecycleEvent::new(
            Layer::L1,
            "HUMAN",
            "S1.L1",
            "ompo",
            EmitOutcome::Emitted,
            reason,
        )
        .with_blocker(blocker)
        .with_step(
            decision.name.clone(),
            decision.status.clone(),
            format!("readback={ARTIFACT_REFERENCE}"),
        );
        events.push(event);
    }
    Ok(events)
}

/// Verified evidence that the DoctorReport artifact exists and reads back with
/// the fields it was written with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReportReadback {
    pub path: PathBuf,
    pub bytes: usize,
    pub run_id: String,
    /// `required_tools` as READ BACK from the inception artifact, not restated
    /// from this crate's own constants.
    pub required_tools: Vec<String>,
    /// `READ_BACK` when the inception artifact supplied the set; otherwise a
    /// named UNMEASURED reason. Never a silent empty list.
    pub required_tools_status: String,
}

fn inception_artifact(repo: &Path) -> PathBuf {
    repo.join(".omp-orchestrator").join("inception.json")
}

/// Read the declared tool set from the inception artifact. A missing or
/// unreadable artifact is UNMEASURED with a reason, never an empty green set.
fn required_tools_readback(repo: &Path) -> (Vec<String>, String) {
    let artifact = inception_artifact(repo);
    match inception::read_required_tools(&artifact) {
        Ok(tools) => (tools, "READ_BACK".to_owned()),
        Err(error) => (
            Vec::new(),
            format!(
                "UNMEASURED_INCEPTION_REQUIRED_TOOLS reason={}",
                error
                    .to_string()
                    .split_whitespace()
                    .next()
                    .unwrap_or("INCEPTION_UNKNOWN")
            ),
        ),
    }
}

fn report_document(summary: &DoctorSummary, tools: &[String], tools_status: &str) -> String {
    let document = serde_json::json!({
        "schema": summary.schema,
        "run_id": summary.run_id,
        "scope": summary.scope,
        "status": summary.status,
        "exit_code": summary.exit_code,
        "probe_count": summary.probe_count,
        "event_count": summary.event_count,
        "readback_lines": summary.readback_lines,
        "probes": summary.probes,
        "remediation": summary.remediation,
        "required_tools": tools,
        "required_tools_status": tools_status,
        "metric": summary.metric,
    });
    // to_string_pretty on a json! value cannot fail: every leaf is already a Value.
    serde_json::to_string_pretty(&document).unwrap_or_else(|_| "{}".to_owned())
}

/// Durably write the DoctorReport artifact: temp file, fsync, rename, parent
/// fsync. Returns nothing on its own — the caller MUST pair it with
/// [`read_report_back`], because exit 0 from a write is not evidence.
fn write_report_bytes(path: &Path, bytes: &[u8]) -> Result<(), DoctorError> {
    let parent = path.parent().ok_or_else(|| DoctorError::ReportWrite {
        path: path.to_path_buf(),
        detail: "artifact path has no parent directory".to_owned(),
    })?;
    let fail = |detail: String| DoctorError::ReportWrite {
        path: path.to_path_buf(),
        detail,
    };
    fs::create_dir_all(parent).map_err(|error| fail(error.to_string()))?;
    let temp = parent.join(format!(
        ".report.json.{}.tmp",
        std::process::id()
    ));
    {
        let mut file = fs::File::create(&temp).map_err(|error| fail(error.to_string()))?;
        file.write_all(bytes).map_err(|error| fail(error.to_string()))?;
        file.sync_all().map_err(|error| fail(error.to_string()))?;
    }
    fs::rename(&temp, path).map_err(|error| fail(error.to_string()))?;
    if let Ok(dir) = fs::File::open(parent) {
        let _ = dir.sync_all();
    }
    Ok(())
}

/// Re-open the artifact and verify it carries the run it claims. This is the
/// half that turns `next_action: readback=…` from a promise into a receipt.
pub fn read_report_back(
    path: &Path,
    expected_run_id: &str,
    expected_tools: &[String],
) -> Result<ReportReadback, DoctorError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(DoctorError::ReportReadbackAbsent {
                path: path.to_path_buf(),
            });
        }
        Err(error) => {
            return Err(DoctorError::ReportReadbackMismatch {
                path: path.to_path_buf(),
                detail: error.to_string(),
            });
        }
    };
    let mismatch = |detail: String| DoctorError::ReportReadbackMismatch {
        path: path.to_path_buf(),
        detail,
    };
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|error| mismatch(error.to_string()))?;
    let run_id = value
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| mismatch("run_id absent from artifact".to_owned()))?;
    if run_id != expected_run_id {
        return Err(mismatch(format!(
            "run_id expected={expected_run_id} found={run_id}"
        )));
    }
    let tools: Vec<String> = value
        .get("required_tools")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| mismatch("required_tools absent from artifact".to_owned()))?
        .iter()
        .filter_map(|entry| entry.as_str().map(str::to_owned))
        .collect();
    if tools != expected_tools {
        return Err(mismatch(format!(
            "required_tools expected={expected_tools:?} found={tools:?}"
        )));
    }
    let required_tools_status = value
        .get("required_tools_status")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| mismatch("required_tools_status absent from artifact".to_owned()))?
        .to_owned();
    Ok(ReportReadback {
        path: path.to_path_buf(),
        bytes: text.len(),
        run_id: run_id.to_owned(),
        required_tools: tools,
        required_tools_status,
    })
}

/// Write the DoctorReport artifact named by [`ARTIFACT_REFERENCE`] and read it
/// back. The write alone is refused as evidence: if the artifact is not
/// readable afterwards the caller gets a typed refusal, not a success.
pub fn write_report_with_readback(
    repo: &Path,
    summary: &DoctorSummary,
) -> Result<ReportReadback, DoctorError> {
    let path = repo.join(ARTIFACT_REFERENCE);
    let (tools, tools_status) = required_tools_readback(repo);
    let document = report_document(summary, &tools, &tools_status);
    write_report_bytes(&path, document.as_bytes())?;
    read_report_back(&path, &summary.run_id, &tools)
}

/// Run the real bounded L1 probe loop and durably emit its lifecycle rows.
fn doctor_run_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("ompo-doctor-{nanos}-{}", std::process::id())
}

fn remediation_for(decisions: &[ProbeDecision]) -> Vec<String> {
    decisions
        .iter()
        .filter(|decision| decision.status != "OK")
        .map(|decision| {
            format!(
                "rerun probe={} reason={}",
                decision.name, decision.reason_code
            )
        })
        .collect()
}
pub fn run_doctor(repo: &Path, scope: &str) -> Result<DoctorSummary, DoctorError> {
    let mut summary = match scope {
        // The health/repair axis advertises these scopes (health_repair::SCOPES);
        // a scope health names must not die here. "system" runs the probe loop;
        // "inception" verifies control files plus the inception artifact.
        "system" => run_doctor_system(repo, scope),
        "inception" => run_doctor_inception(repo),
        _ => Err(DoctorError::UnsupportedScope(scope.to_owned())),
    }?;
    // ARTIFACT_REFERENCE was declared and named in `next_action` long before
    // anything wrote it. Writing it here and READING IT BACK is what makes the
    // promise a receipt; a failed readback fails the run.
    summary.report = Some(write_report_with_readback(repo, &summary)?);
    Ok(summary)
}
fn run_doctor_system(repo: &Path, scope: &str) -> Result<DoctorSummary, DoctorError> {
    let run_id = doctor_run_id();
    let decisions: Vec<_> = PROBES.iter().map(run_probe).collect();
    let exit_code = doctor_exit_code(&decisions)?;
    let status = if exit_code == 0 { "OK" } else { "DEGRADED" };
    let remediation = remediation_for(&decisions);
    let next_action = format!("readback={ARTIFACT_REFERENCE}");
    let events = lifecycle_events(PROBES, &decisions)?;
    let journal = DurableJournal::open(default_repo_journal(repo))?;
    let readback: Readback = emit_host(&journal, &events)?;
    Ok(DoctorSummary {
        schema: "ompo.doctor.v1",
        run_id,
        scope: scope.to_owned(),
        status,
        exit_code,
        artifact: ARTIFACT_REFERENCE,
        lifecycle_journal: readback.path,
        probe_count: decisions.len(),
        event_count: events.len(),
        readback_lines: readback.lines,
        metric: Some(probe_answer_metric(PROBES, &decisions)),
        probes: decisions,
        remediation,
        next_action,
        report: None,
    })
}

/// Scoped doctor for the inception axis health advertises. Verifies the
/// control-file set plus the inception artifact and reports a verdict —
/// read-only: unlike the system scope it writes no journal rows, so there is
/// no readback to confuse with a passing system run.
fn run_doctor_inception(repo: &Path) -> Result<DoctorSummary, DoctorError> {
    let run_id = doctor_run_id();
    let presence = inception::control_file_presence(repo);
    let missing: Vec<&str> = presence
        .iter()
        .filter_map(|(path, present)| (!present).then_some(path.as_str()))
        .collect();
    let mut decisions = vec![ProbeDecision {
        name: "control_files".to_owned(),
        status: if missing.is_empty() { "OK".to_owned() } else { "MISSING".to_owned() },
        reason_code: if missing.is_empty() {
            "CONTROL_FILES_COMPLETE".to_owned()
        } else {
            "CONTROL_FILES_MISSING".to_owned()
        },
        detail: if missing.is_empty() {
            format!("{} present", presence.len())
        } else {
            format!("missing {}", missing.join(","))
        },
        presence: Some(repo.display().to_string()),
        version: None,
    }];
    let artifact = repo.join(".omp-orchestrator").join("inception.json");
    decisions.push(match inception::read_inception(&artifact) {
        Ok(_) => ProbeDecision {
            name: "inception_artifact".to_owned(),
            status: "OK".to_owned(),
            reason_code: "INCEPTION_READABLE".to_owned(),
            detail: artifact.display().to_string(),
            presence: Some(artifact.display().to_string()),
            version: None,
        },
        Err(error) => ProbeDecision {
            name: "inception_artifact".to_owned(),
            status: "UNREADABLE".to_owned(),
            // Mirrors health_repair::reason_code_of; unify the two when that
            // file is not under active peer edit.
            reason_code: error.to_string().split_whitespace().next().unwrap_or("INCEPTION_UNKNOWN").to_owned(),
            detail: error.to_string(),
            presence: Some(artifact.display().to_string()),
            version: None,
        },
    });
    let exit_code = if decisions.iter().all(|decision| decision.status == "OK") { 0 } else { 1 };
    let status = if exit_code == 0 { "OK" } else { "DEGRADED" };
    let remediation = remediation_for(&decisions);
    Ok(DoctorSummary {
        schema: "ompo.doctor.v1",
        run_id,
        scope: "inception".to_owned(),
        status,
        exit_code,
        artifact: ARTIFACT_REFERENCE,
        lifecycle_journal: default_repo_journal(repo),
        probe_count: decisions.len(),
        event_count: 0,
        readback_lines: 0,
        probes: decisions,
        remediation,
        next_action: format!("repair_scope=inception artifact={}", artifact.display()),
        report: None,
        metric: None,
    })
}

/// A DoctorSummary shaped for artifact tests: no probes are run, so the write
/// path is exercised without depending on the host's tool set.
#[cfg(test)]
fn artifact_fixture_summary(run_id: &str) -> DoctorSummary {
    DoctorSummary {
        schema: "ompo.doctor.v1",
        run_id: run_id.to_owned(),
        scope: "system".to_owned(),
        status: "OK",
        exit_code: 0,
        artifact: ARTIFACT_REFERENCE,
        lifecycle_journal: PathBuf::from("/fixture/lifecycle.jsonl"),
        probe_count: 0,
        event_count: 0,
        readback_lines: 0,
        probes: Vec::new(),
        remediation: Vec::new(),
        next_action: format!("readback={ARTIFACT_REFERENCE}"),
        report: None,
        metric: None,
    }
}

#[cfg(test)]
mod artifact_tests {
    use super::*;
    use tempfile::tempdir;

    /// KNOWN-GOOD: the artifact is written and reads back, so the gate is not
    /// merely refusing everything.
    #[test]
    fn doctor_report_artifact_is_written_and_read_back() {
        let dir = tempdir().expect("fixture repo");
        let summary = artifact_fixture_summary("fixture-run-good");
        let readback =
            write_report_with_readback(dir.path(), &summary).expect("artifact readback");
        assert_eq!(readback.path, dir.path().join(ARTIFACT_REFERENCE));
        assert_eq!(readback.run_id, "fixture-run-good");
        assert!(readback.bytes > 0, "an empty artifact is not a readback");
        // No inception artifact in the fixture repo, so the tool set is NAMED
        // UNMEASURED rather than reported as an empty green list.
        assert!(
            readback
                .required_tools_status
                .starts_with("UNMEASURED_INCEPTION_REQUIRED_TOOLS"),
            "status={}",
            readback.required_tools_status
        );
        assert!(readback.required_tools.is_empty());
    }

    /// KNOWN-BAD: the write reports success and the artifact is then absent.
    /// EXPECT a typed refusal, never a summary that still claims a readback.
    #[test]
    fn write_success_with_absent_artifact_is_refused() {
        let dir = tempdir().expect("fixture repo");
        let summary = artifact_fixture_summary("fixture-run-absent");
        let path = dir.path().join(ARTIFACT_REFERENCE);
        let (tools, status) = required_tools_readback(dir.path());
        let document = report_document(&summary, &tools, &status);
        write_report_bytes(&path, document.as_bytes()).expect("write leg succeeds");
        assert!(path.exists(), "positive control: the write really landed");
        std::fs::remove_file(&path).expect("simulate a write whose artifact is not there");
        let error = read_report_back(&path, &summary.run_id, &tools)
            .expect_err("an absent artifact must not read back");
        assert!(
            error
                .to_string()
                .starts_with("L1_DOCTOR_REPORT_READBACK_ABSENT"),
            "error={error}"
        );
    }

    /// KNOWN-BAD: the artifact exists but belongs to a different run.
    #[test]
    fn stale_artifact_from_another_run_is_refused() {
        let dir = tempdir().expect("fixture repo");
        let summary = artifact_fixture_summary("fixture-run-first");
        write_report_with_readback(dir.path(), &summary).expect("first run");
        let path = dir.path().join(ARTIFACT_REFERENCE);
        let (tools, _status) = required_tools_readback(dir.path());
        let error = read_report_back(&path, "fixture-run-second", &tools)
            .expect_err("a stale artifact must not satisfy a later run");
        assert!(
            error
                .to_string()
                .starts_with("L1_DOCTOR_REPORT_READBACK_MISMATCH"),
            "error={error}"
        );
    }

    /// The inception tool set is READ BACK from the artifact, not restated from
    /// this crate's constants.
    #[test]
    fn inception_required_tools_are_read_back_into_the_report() {
        let dir = tempdir().expect("fixture repo");
        std::fs::create_dir_all(dir.path().join(".omp-orchestrator")).expect("runtime dir");
        std::fs::write(
            inception_artifact(dir.path()),
            r#"{"required_tools":["git","cargo","br","bv","ntm","am","jq"]}"#,
        )
        .expect("fixture inception artifact");
        let summary = artifact_fixture_summary("fixture-run-tools");
        let readback =
            write_report_with_readback(dir.path(), &summary).expect("artifact readback");
        assert_eq!(readback.required_tools_status, "READ_BACK");
        assert_eq!(
            readback.required_tools,
            vec!["git", "cargo", "br", "bv", "ntm", "am", "jq"]
        );
    }

    /// END-TO-END: the artifact is written BY `run_doctor` itself, so the
    /// promise in `next_action` is discharged by the same call that makes it.
    /// Prints the artifact so the readback is visible evidence, not a claim.
    #[test]
    fn run_doctor_writes_and_reads_back_the_named_artifact() {
        let dir = tempdir().expect("fixture repo");
        let summary = run_doctor(dir.path(), "inception").expect("inception scope");
        let readback = summary
            .report
            .as_ref()
            .expect("run_doctor must attach a VERIFIED readback, not a promise");
        let path = dir.path().join(ARTIFACT_REFERENCE);
        let text = std::fs::read_to_string(&path).expect("artifact must exist on disk");
        println!(
            "ARTIFACT_READBACK path={} bytes={} run_id={}\n{text}",
            readback.path.display(),
            readback.bytes,
            readback.run_id
        );
        assert_eq!(readback.run_id, summary.run_id);
        assert_eq!(readback.bytes, text.len());
        assert!(text.contains("\"run_id\""), "artifact text={text}");
    }
}

#[cfg(test)]
mod metric_tests {
    use super::*;

    fn answering(name: &str) -> ProbeDecision {
        ProbeDecision {
            name: name.to_owned(),
            status: "OK".to_owned(),
            reason_code: reason_code(name, "OK"),
            detail: "fixture".to_owned(),
            presence: Some(format!("/fixture/{name}")),
            version: Some("fixture-version".to_owned()),
        }
    }

    fn all_answering() -> Vec<ProbeDecision> {
        PROBES.iter().map(|spec| answering(spec.name)).collect()
    }

    /// POSITIVE CONTROL: every declared probe answers, and the metric is
    /// reported WITH its denominator.
    #[test]
    fn all_probes_answering_reports_the_ratio_and_its_denominator() {
        let metric = probe_answer_metric(PROBES, &all_answering());
        assert_eq!(metric.id, PROBE_ANSWER_METRIC_ID);
        assert_eq!(metric.probes_declared, PROBES.len());
        assert_eq!(metric.probes_answered, PROBES.len());
        assert_eq!(metric.ratio, Some(1.0));
        assert_eq!(metric.verdict, "MEASURED_OK");
        assert!(metric.unprobeable.is_empty());
    }

    /// KNOWN-BAD: omit ONE declared probe result. EXPECT UNMEASURED naming the
    /// missing probe — never a ratio computed over a short denominator.
    #[test]
    fn omitting_one_declared_probe_result_is_unmeasured_not_a_green_ratio() {
        let mut decisions = all_answering();
        let dropped = decisions.remove(2).name;
        let metric = probe_answer_metric(PROBES, &decisions);
        assert_eq!(metric.verdict, "UNMEASURED");
        assert!(
            metric.reason_code == format!("L1_METRIC_PARTIAL_PROBE_SET missing={dropped}"),
            "reason={}",
            metric.reason_code
        );
        assert_eq!(metric.ratio, None, "a partial set must not publish a ratio");
        assert_eq!(metric.probes_declared, PROBES.len());
    }

    /// KNOWN-BAD: no authoritative records at all. EXPECT UNMEASURED with
    /// UNKNOWN_NO_RECORD, never 0/N rendered as a measurement.
    #[test]
    fn no_authoritative_records_is_unmeasured_never_a_zero_valued_metric() {
        let metric = probe_answer_metric(PROBES, &[]);
        assert_eq!(metric.verdict, "UNMEASURED");
        assert_eq!(metric.reason_code, "UNKNOWN_NO_RECORD");
        assert_eq!(metric.ratio, None);
        assert_eq!(metric.probes_answered, 0);
    }

    /// A real shortfall IS a measurement: below the floor, with the
    /// UNPROBEABLE subject NAMED rather than merely counted.
    #[test]
    fn an_unprobeable_subject_is_named_and_drops_the_metric_below_floor() {
        let mut decisions = all_answering();
        decisions[1].status = "UNPROBEABLE".to_owned();
        decisions[1].version = None;
        let metric = probe_answer_metric(PROBES, &decisions);
        assert_eq!(metric.verdict, "MEASURED_BELOW_FLOOR");
        assert_eq!(metric.unprobeable, vec![PROBES[1].name.to_owned()]);
        assert_eq!(metric.probes_answered, PROBES.len() - 1);
        assert!(metric.ratio.is_some_and(|ratio| ratio < metric.floor));
    }

    /// The STALE band is UNMEASURED while no version floor is declared. A 0
    /// there would be green by construction for a band nothing can enter.
    #[test]
    fn stale_band_is_unmeasured_not_zero_while_no_version_floor_exists() {
        let metric = probe_answer_metric(PROBES, &all_answering());
        assert_eq!(metric.stale_count, None);
        assert_eq!(metric.stale_status, "UNMEASURED_NO_VERSION_FLOOR_DECLARED");
    }

    /// When a STALE subject IS observed the count becomes a real measurement.
    #[test]
    fn an_observed_stale_subject_is_counted() {
        let mut decisions = all_answering();
        decisions[0].status = "STALE".to_owned();
        let metric = probe_answer_metric(PROBES, &decisions);
        assert_eq!(metric.stale_count, Some(1));
        assert_eq!(metric.stale_status, "MEASURED");
    }

    /// The declared set cannot be empty: that is an instrument error, not a
    /// healthy 0/0.
    #[test]
    fn an_empty_declared_set_is_an_instrument_error() {
        let metric = probe_answer_metric(&[], &[]);
        assert_eq!(metric.verdict, "ERROR");
        assert_eq!(metric.reason_code, "L1_METRIC_NO_DECLARED_PROBES");
        assert_eq!(metric.ratio, None);
    }
}

pub fn current_repo() -> Result<PathBuf, DoctorError> {
    std::env::current_dir().map_err(DoctorError::CurrentDirectory)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn fixture_decisions() -> Vec<ProbeDecision> {
        PROBES
            .iter()
            .map(|spec| ProbeDecision {
                name: spec.name.to_owned(),
                status: "OK".to_owned(),
                reason_code: reason_code(spec.name, "OK"),
                detail: "fixture".to_owned(),
                presence: Some(format!("/fixture/{}", spec.command)),
                version: Some("fixture-version".to_owned()),
            })
            .collect()
    }

    #[test]
    fn writer_emits_one_event_per_probe_with_reason_and_artifact() {
        let dir = tempdir().expect("fixture directory");
        let journal = DurableJournal::open(dir.path().join("lifecycle.jsonl")).expect("journal");
        let decisions = fixture_decisions();
        let events = lifecycle_events(PROBES, &decisions).expect("events");
        let readback = emit_host(&journal, &events).expect("readback");
        assert_eq!(readback.lines, PROBES.len());
        let lines = std::fs::read_to_string(journal.path())
            .expect("journal text")
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), PROBES.len());
        for (line, decision) in lines.iter().zip(decisions.iter()) {
            let value: serde_json::Value = serde_json::from_str(line).expect("json event");
            assert_eq!(value["reason_code"], decision.reason_code);
            assert_eq!(value["step"], decision.name);
            assert_eq!(value["status"], "OK");
            assert_eq!(
                value["next_command"],
                format!("readback={ARTIFACT_REFERENCE}")
            );
        }
    }

    #[test]
    fn suppressing_one_probe_is_reported_not_silently_accepted() {
        let mut decisions = fixture_decisions();
        let suppressed = decisions.remove(3).name;
        let error = lifecycle_events(PROBES, &decisions).expect_err("missing event");
        assert_eq!(
            error.to_string(),
            format!("L1_DOCTOR_MISSING_LIFECYCLE_EVENT probe={suppressed} — partial probe output is refused")
        );
    }

    #[test]
    fn probe_requires_presence_and_version_signals() {
        const EMPTY_ARGS: &[&str] = &[];
        let good = run_probe(&ProbeSpec {
            name: "fixture-good",
            command: "printf",
            args: &["fixture-version"],
        });
        assert_eq!(good.status, "OK");
        assert!(good.presence.is_some(), "PATH presence must be recorded");
        assert_eq!(good.version.as_deref(), Some("fixture-version"));
        assert_eq!(doctor_exit_code(&[good]).expect("non-empty probe set"), 0);

        let no_version = run_probe(&ProbeSpec {
            name: "fixture-no-version",
            command: "printf",
            args: EMPTY_ARGS,
        });
        assert_eq!(no_version.status, "UNPROBEABLE");
        assert!(no_version.presence.is_some());
        assert!(no_version.version.is_none());
        assert_eq!(doctor_exit_code(&[no_version]).expect("non-empty probe set"), 1);

        let unprobeable = run_probe(&ProbeSpec {
            name: "fixture-unprobeable",
            command: "false",
            args: EMPTY_ARGS,
        });
        assert_eq!(unprobeable.status, "UNPROBEABLE");
        assert!(unprobeable.presence.is_some());
        assert!(unprobeable.version.is_none());

        let absent = run_probe(&ProbeSpec {
            name: "fixture-absent",
            command: "omp-l1-probe-command-that-is-absent",
            args: EMPTY_ARGS,
        });
        assert_eq!(absent.status, "ABSENT_SPECIFIC");
        assert!(absent.presence.is_none());
        assert!(absent.version.is_none());
    }

    #[test]
    fn two_band_exit_mutation_is_red_and_restores_green() {
        let mut decisions = fixture_decisions();
        assert_eq!(doctor_exit_code(&decisions).expect("non-empty probe set"), 0);

        decisions[0].status = "UNPROBEABLE".to_owned();
        decisions[0].reason_code = reason_code(&decisions[0].name, "UNPROBEABLE");
        decisions[0].version = None;
        assert_eq!(doctor_exit_code(&decisions).expect("mutated probe set"), 1);
        assert_ne!(decisions[0].status, "OK", "missing version cannot retain OK");

        decisions[0].status = "OK".to_owned();
        decisions[0].reason_code = reason_code(&decisions[0].name, "OK");
        decisions[0].version = Some("fixture-version".to_owned());
        assert_eq!(doctor_exit_code(&decisions).expect("restored probe set"), 0);
    }

    #[test]
    fn empty_probe_set_is_an_error() {
        let error = lifecycle_events(PROBES, &[]).expect_err("empty set");
        assert!(error.to_string().contains("L1_DOCTOR_EMPTY_PROBE_SET"));
        let error = doctor_exit_code(&[]).expect_err("empty exit set");
        assert!(error.to_string().contains("L1_DOCTOR_EMPTY_PROBE_SET"));
    }
}

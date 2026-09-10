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
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use ompo_start::inception;
use subprocess_contract::{bounded_output, BoundedOutcome};

pub mod adapter_exec;
pub mod health_repair;
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
}

#[derive(Debug)]
pub enum DoctorError {
    EmptyProbeSet,
    MissingProbeEvent { probe: &'static str },
    UnsupportedScope(String),
    CurrentDirectory(std::io::Error),
    Emit(EmitError),
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
    match scope {
        // The health/repair axis advertises these scopes (health_repair::SCOPES);
        // a scope health names must not die here. "system" runs the probe loop;
        // "inception" verifies control files plus the inception artifact.
        "system" => run_doctor_system(repo, scope),
        "inception" => run_doctor_inception(repo),
        _ => Err(DoctorError::UnsupportedScope(scope.to_owned())),
    }
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
        probes: decisions,
        remediation,
        next_action,
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
    })
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

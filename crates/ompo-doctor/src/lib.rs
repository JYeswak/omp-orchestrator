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
use std::time::Duration;
use subprocess_contract::{bounded_output, BoundedOutcome};

pub mod umbrella;
pub mod adapter_exec;
pub mod liveness;
pub mod provenance;
pub mod selfdoc;

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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorSummary {
    pub schema: &'static str,
    pub scope: String,
    pub artifact: &'static str,
    pub lifecycle_journal: PathBuf,
    pub probe_count: usize,
    pub event_count: usize,
    pub readback_lines: usize,
    pub probes: Vec<ProbeDecision>,
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

fn detail_from_output(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    stdout
        .lines()
        .chain(stderr.lines())
        .find(|line| !line.trim().is_empty())
        .unwrap_or("no version output")
        .trim()
        .chars()
        .take(240)
        .collect()
}

fn reason_code(name: &str, status: &str) -> String {
    format!(
        "L1_PROBE_{}_{}",
        name.replace('-', "_").to_ascii_uppercase(),
        status
    )
}

fn run_probe(spec: &ProbeSpec) -> ProbeDecision {
    let mut command = Command::new(spec.command);
    command.args(spec.args);
    let (status, detail) = match bounded_output(&mut command, PROBE_DEADLINE) {
        BoundedOutcome::Completed(output) if output.status.success() => {
            ("OK", detail_from_output(&output))
        }
        BoundedOutcome::Completed(output) => (
            "UNPROBEABLE",
            format!("exit={} {}", output.status, detail_from_output(&output)),
        ),
        BoundedOutcome::TimedOut => ("UNMEASURED", "probe timed out".to_owned()),
        BoundedOutcome::Unspawned(error) => ("ABSENT_SPECIFIC", error.to_string()),
    };
    ProbeDecision {
        name: spec.name.to_owned(),
        status: status.to_owned(),
        reason_code: reason_code(spec.name, status),
        detail,
    }
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
pub fn run_doctor(repo: &Path, scope: &str) -> Result<DoctorSummary, DoctorError> {
    if scope != "system" {
        return Err(DoctorError::UnsupportedScope(scope.to_owned()));
    }
    let decisions: Vec<_> = PROBES.iter().map(run_probe).collect();
    let events = lifecycle_events(PROBES, &decisions)?;
    let journal = DurableJournal::open(default_repo_journal(repo))?;
    let readback: Readback = emit_host(&journal, &events)?;
    Ok(DoctorSummary {
        schema: "ompo.doctor.v1",
        scope: scope.to_owned(),
        artifact: ARTIFACT_REFERENCE,
        lifecycle_journal: readback.path,
        probe_count: decisions.len(),
        event_count: events.len(),
        readback_lines: readback.lines,
        probes: decisions,
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
    fn empty_probe_set_is_an_error() {
        let error = lifecycle_events(PROBES, &[]).expect_err("empty set");
        assert!(error.to_string().contains("L1_DOCTOR_EMPTY_PROBE_SET"));
    }
}

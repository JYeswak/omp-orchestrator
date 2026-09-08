#![forbid(unsafe_code)]

//! Read-only typed access to `omp ps --json --dir <repo>`.
//!
//! The parser and classifier are deliberately independent of the subprocess boundary so
//! future CLI wiring can test protocol decisions without starting OMP. The probe records the
//! child's exit code separately from the parse/verdict: a process that exits successfully is
//! not thereby a valid JSON answer, and a non-zero child exit is never promoted to success.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use subprocess_contract::{bounded_output, BoundedOutcome};

const PROBE_DEADLINE: Duration = Duration::from_secs(10);

/// One project scope returned by `omp ps --json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessScope {
    pub kind: String,
    #[serde(rename = "projectDir")]
    pub project_dir: String,
    #[serde(rename = "runtimeDir")]
    pub runtime_dir: String,
    #[serde(rename = "brokerPid")]
    pub broker_pid: u32,
    pub daemons: Vec<DaemonProcess>,
}

/// One supervised daemon row returned inside a process scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonProcess {
    pub name: String,
    pub id: String,
    pub state: String,
    pub pid: u32,
    #[serde(rename = "createdAt")]
    pub created_at: u64,
    #[serde(rename = "startedAt")]
    pub started_at: u64,
    #[serde(rename = "readyAt")]
    pub ready_at: u64,
    #[serde(rename = "restartCount")]
    pub restart_count: u32,
    #[serde(rename = "outputBytes")]
    pub output_bytes: u64,
    #[serde(rename = "readyMatch")]
    pub ready_match: String,
    pub persist: bool,
    pub detached: bool,
    pub command: String,
    pub cwd: String,
    pub supervised: bool,
    #[serde(rename = "exitCode")]
    pub exit_code: Option<i32>,
    #[serde(rename = "exitedAt")]
    pub exited_at: Option<u64>,
}

/// Errors in the JSON document after the child has produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessParseError {
    /// The bytes are not a JSON document.
    InvalidJson { detail: String },
    /// The document is JSON but not the observed array-of-scope-objects shape.
    InvalidShape { detail: String },
}

impl fmt::Display for ProcessParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson { detail } => write!(formatter, "invalid JSON: {detail}"),
            Self::InvalidShape { detail } => write!(formatter, "invalid OMP ps shape: {detail}"),
        }
    }
}

impl std::error::Error for ProcessParseError {}

/// Parse the observed `omp ps --json` array without contacting OMP.
pub fn parse_ps_json(raw: &str) -> Result<Vec<ProcessScope>, ProcessParseError> {
    let value: Value =
        serde_json::from_str(raw).map_err(|error| ProcessParseError::InvalidJson {
            detail: error.to_string(),
        })?;
    if !value.is_array() {
        return Err(ProcessParseError::InvalidShape {
            detail: "top-level value must be an array of scope objects".to_owned(),
        });
    }
    serde_json::from_value(value).map_err(|error| ProcessParseError::InvalidShape {
        detail: error.to_string(),
    })
}

/// The pure verdict for one completed or bounded child observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessProbeVerdict {
    /// The child exited zero and returned a valid process list.
    Answered(Vec<ProcessScope>),
    /// The child exited without a zero status. Its exit code remains on the enclosing outcome.
    ChildFailed { detail: String },
    /// The bounded process contract terminated the child at its deadline.
    TimedOut,
    /// The process could not be spawned.
    SpawnFailed { detail: String },
    /// The child returned bytes that are not JSON.
    InvalidJson { detail: String },
    /// The child returned JSON with the wrong process-list shape.
    InvalidShape { detail: String },
}

/// Result of one read-only process probe. `exit_code` is intentionally independent of verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessProbeOutcome {
    pub exit_code: Option<i32>,
    pub verdict: ProcessProbeVerdict,
}

/// Classify captured `omp ps` output without spawning a process.
pub fn classify_ps_output(
    exit_code: Option<i32>,
    stdout: &str,
    stderr: &str,
) -> ProcessProbeOutcome {
    let verdict = match exit_code {
        Some(0) => match parse_ps_json(stdout) {
            Ok(scopes) => ProcessProbeVerdict::Answered(scopes),
            Err(ProcessParseError::InvalidJson { detail }) => {
                ProcessProbeVerdict::InvalidJson { detail }
            }
            Err(ProcessParseError::InvalidShape { detail }) => {
                ProcessProbeVerdict::InvalidShape { detail }
            }
        },
        Some(code) => ProcessProbeVerdict::ChildFailed {
            detail: if stderr.trim().is_empty() {
                format!("omp ps exited with code {code}")
            } else {
                stderr.trim().to_owned()
            },
        },
        None => ProcessProbeVerdict::ChildFailed {
            detail: if stderr.trim().is_empty() {
                "omp ps terminated without an exit code".to_owned()
            } else {
                stderr.trim().to_owned()
            },
        },
    };
    ProcessProbeOutcome { exit_code, verdict }
}

/// Cx-first, bounded, read-only execution of `omp ps --json --dir <repo>`.
///
/// This probe only starts `omp ps`; it never invokes a lifecycle mutation such as stop, kill,
/// or restart. The synchronous bounded helper is retained here because this path is also used
/// by non-runtime doctor callers; the Cx-first signature keeps later async wiring uniform.
pub async fn read_processes(cx: &asupersync::Cx, repo: &Path) -> ProcessProbeOutcome {
    let _ = cx;
    let mut command = Command::new("omp");
    command.args(["ps", "--json", "--dir"]);
    command.arg(repo);
    match bounded_output(&mut command, PROBE_DEADLINE) {
        BoundedOutcome::Completed(output) => match String::from_utf8(output.stdout) {
            Ok(stdout) => classify_ps_output(
                output.status.code(),
                &stdout,
                &String::from_utf8_lossy(&output.stderr),
            ),
            Err(error) => ProcessProbeOutcome {
                exit_code: output.status.code(),
                verdict: ProcessProbeVerdict::InvalidJson {
                    detail: format!("stdout is not UTF-8: {error}"),
                },
            },
        },
        BoundedOutcome::TimedOut => ProcessProbeOutcome {
            exit_code: None,
            verdict: ProcessProbeVerdict::TimedOut,
        },
        BoundedOutcome::Unspawned(error) => ProcessProbeOutcome {
            exit_code: None,
            verdict: ProcessProbeVerdict::SpawnFailed {
                detail: error.to_string(),
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn daemon_with_state(state: &str, extra: &str) -> String {
        format!(
            r#"{{"name":"omp.test","id":"daemon-1","state":"{state}","pid":123,"createdAt":1,"startedAt":2,"readyAt":3,"restartCount":0,"outputBytes":4,"readyMatch":"ready","persist":false,"detached":false,"command":"omp test","cwd":"/repo","supervised":true{extra}}}"#
        )
    }

    fn daemon(extra: &str) -> String {
        daemon_with_state("ready", extra)
    }

    fn scope(daemon: &str) -> String {
        format!(
            r#"{{"kind":"project","projectDir":"/repo","runtimeDir":"/run","brokerPid":99,"daemons":[{daemon}]}}"#
        )
    }

    #[test]
    fn parses_ready_row() {
        let parsed = parse_ps_json(&format!("[{}]", scope(&daemon("")))).expect("ready row");
        assert_eq!(parsed[0].daemons[0].state, "ready");
        assert_eq!(parsed[0].daemons[0].exit_code, None);
    }

    #[test]
    fn parses_exited_row() {
        let parsed = parse_ps_json(&format!(
            "[{}]",
            scope(&daemon_with_state(
                "exited",
                r#","exitCode":17,"exitedAt":8"#
            ))
        ))
        .expect("exited row");
        assert_eq!(parsed[0].daemons[0].exit_code, Some(17));
        assert_eq!(parsed[0].daemons[0].exited_at, Some(8));
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(matches!(
            parse_ps_json("[{"),
            Err(ProcessParseError::InvalidJson { .. })
        ));
    }

    #[test]
    fn rejects_wrong_top_level_shape() {
        assert!(matches!(
            parse_ps_json("{}"),
            Err(ProcessParseError::InvalidShape { .. })
        ));
    }

    #[test]
    fn rejects_non_object_daemon_entry() {
        let raw = r#"[{"kind":"project","projectDir":"/repo","runtimeDir":"/run","brokerPid":99,"daemons":[null]}]"#;
        assert!(matches!(
            parse_ps_json(raw),
            Err(ProcessParseError::InvalidShape { .. })
        ));
    }
}

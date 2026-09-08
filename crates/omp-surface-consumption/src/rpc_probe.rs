use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::process::Command;
use std::time::Duration;

const PROBE_DEADLINE: Duration = Duration::from_secs(75);
const OMP_DEADLINE_SECS: u64 = 60;
const EVENT_PROBE_SCRIPT: &str = r#"{ printf '%s\n%s\n' '{"id":"n","type":"negotiate_protocol","protocolVersion":2}' '{"id":"1","type":"prompt","message":"Reply with OK."}'; sleep 60; } | omp --mode=rpc --no-session --no-tools --max-time=60"#;

/// Event names on the extracted RPC dispatch seam.
pub const NOTIFICATION_TYPES: [&str; 6] = [
    "tool_stream_update",
    "message_update",
    "message_start",
    "message_end",
    "turn_end",
    "agent_end",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventReport {
    pub frame_count: usize,
    pub counts: BTreeMap<String, usize>,
}

#[derive(Debug)]
pub enum RpcProbeError {
    Process(String),
    EmptyOutput,
    InvalidJson(String),
}

impl fmt::Display for RpcProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Process(detail) => write!(formatter, "RPC_EVENT_PROCESS {detail}"),
            Self::EmptyOutput => formatter.write_str("RPC_EVENT_EMPTY_OUTPUT"),
            Self::InvalidJson(detail) => write!(formatter, "RPC_EVENT_INVALID_JSON {detail}"),
        }
    }
}

impl std::error::Error for RpcProbeError {}

/// Run a bounded, no-tools prompt and retain only event type counts.
///
/// The input pipe stays open through OMP's own deadline so asynchronous event frames are not
/// mistaken for absent output. The no-tools restriction deliberately leaves tool_stream_update
/// unexercised; the parser still consumes that event type when a tool-enabled caller supplies it.
pub fn probe() -> Result<EventReport, RpcProbeError> {
    let mut command = Command::new("sh");
    command.args(["-c", EVENT_PROBE_SCRIPT]);
    let output = match subprocess_contract::bounded_output(&mut command, PROBE_DEADLINE) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => output,
        subprocess_contract::BoundedOutcome::Completed(output) => {
            return Err(RpcProbeError::Process(format!(
                "exit={:?} stderr={}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        subprocess_contract::BoundedOutcome::TimedOut => {
            return Err(RpcProbeError::Process(format!(
                "probe_timeout_secs={}",
                PROBE_DEADLINE.as_secs()
            )));
        }
        subprocess_contract::BoundedOutcome::Unspawned(error) => {
            return Err(RpcProbeError::Process(error.to_string()));
        }
    };
    parse_output(&output.stdout)
}

pub fn parse_output(stdout: &[u8]) -> Result<EventReport, RpcProbeError> {
    let text = String::from_utf8_lossy(stdout);
    let mut counts = BTreeMap::new();
    let mut frame_count = 0;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let value = serde_json::from_str::<Value>(line.trim())
            .map_err(|error| RpcProbeError::InvalidJson(error.to_string()))?;
        frame_count += 1;
        if let Some(kind) = value.get("type").and_then(Value::as_str) {
            if NOTIFICATION_TYPES.contains(&kind) {
                *counts.entry(kind.to_owned()).or_insert(0) += 1;
            }
        }
    }
    if frame_count == 0 {
        return Err(RpcProbeError::EmptyOutput);
    }
    Ok(EventReport { frame_count, counts })
}

pub fn summary(report: &EventReport) -> String {
    let observed = NOTIFICATION_TYPES
        .iter()
        .filter_map(|name| report.counts.get(*name).map(|count| format!("{name}={count}")))
        .collect::<Vec<_>>();
    format!(
        "OMP_RPC_EVENT_CONSUMPTION deadline_secs={} frames={} observed={}",
        OMP_DEADLINE_SECS,
        report.frame_count,
        if observed.is_empty() {
            "NONE".to_owned()
        } else {
            observed.join(",")
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_dispatch_seam_events_without_retaining_payloads() {
        let output = br#"
            {"type":"ready"}
            {"type":"message_start","message":{}}
            {"type":"message_update","message":{},"assistantMessageEvent":{}}
            {"type":"message_end","message":{}}
            {"type":"turn_end","message":{},"toolResults":[],"turnIndex":0}
            {"type":"agent_end","messages":[],"isTerminal":true}
            {"type":"tool_stream_update","partialResult":{}}
        "#;
        let report = parse_output(output).expect("frames parse");
        assert_eq!(report.frame_count, 7);
        assert_eq!(report.counts.get("message_start"), Some(&1));
        assert_eq!(report.counts.get("message_update"), Some(&1));
        assert_eq!(report.counts.get("message_end"), Some(&1));
        assert_eq!(report.counts.get("turn_end"), Some(&1));
        assert_eq!(report.counts.get("agent_end"), Some(&1));
        assert_eq!(report.counts.get("tool_stream_update"), Some(&1));
    }

    #[test]
    fn empty_event_stream_is_a_typed_error() {
        assert!(matches!(parse_output(b"\n"), Err(RpcProbeError::EmptyOutput)));
    }
}

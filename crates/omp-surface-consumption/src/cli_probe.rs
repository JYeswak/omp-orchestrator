use serde_json::Value;
use std::fmt;
use std::process::Command;
use std::time::Duration;

const CLI_PROBE_DEADLINE: Duration = Duration::from_secs(60);

/// The OMP CLI data surfaces that have a real consumer in this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliProbe {
    Models,
    Stats,
    Usage,
}

impl CliProbe {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Models => "models",
            Self::Stats => "stats",
            Self::Usage => "usage",
        }
    }

    pub const fn required_keys(self) -> &'static [&'static str] {
        match self {
            Self::Models => &["models"],
            Self::Stats => &["overall", "byModel", "byFolder", "timeSeries"],
            Self::Usage => &["reports", "capacity", "accountsWithoutUsage"],
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "models" => Some(Self::Models),
            "stats" => Some(Self::Stats),
            "usage" => Some(Self::Usage),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum CliProbeError {
    Process(String),
    EmptyOutput,
    InvalidJson(String),
    WrongTopLevel,
    MissingKey(&'static str),
}

impl fmt::Display for CliProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Process(detail) => write!(formatter, "CLI_PROBE_PROCESS {detail}"),
            Self::EmptyOutput => formatter.write_str("CLI_PROBE_EMPTY_OUTPUT"),
            Self::InvalidJson(detail) => write!(formatter, "CLI_PROBE_INVALID_JSON {detail}"),
            Self::WrongTopLevel => formatter.write_str("CLI_PROBE_WRONG_TOP_LEVEL"),
            Self::MissingKey(key) => write!(formatter, "CLI_PROBE_MISSING_KEY key={key}"),
        }
    }
}

impl std::error::Error for CliProbeError {}

fn extract_json(stdout: &[u8]) -> Result<Value, CliProbeError> {
    let text = String::from_utf8_lossy(stdout);
    let start = text.find(['{', '[']).ok_or(CliProbeError::EmptyOutput)?;
    serde_json::from_str(&text[start..])
        .map_err(|error| CliProbeError::InvalidJson(error.to_string()))
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Run one OMP data command through the shared bounded process boundary.
///
/// The command is intentionally opt-in: the default surface scanner does not refresh
/// sessions, credentials, or model catalogs while checking the bundle.
pub fn probe_json(binary: &str, probe: CliProbe) -> Result<Value, CliProbeError> {
    let mut command = Command::new(binary);
    command.args([probe.name(), "--json"]);
    let output = match subprocess_contract::bounded_output(&mut command, CLI_PROBE_DEADLINE) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => output,
        subprocess_contract::BoundedOutcome::Completed(output) => {
            return Err(CliProbeError::Process(format!(
                "command={} exit={:?} stderr={}",
                probe.name(),
                output.status.code(),
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        subprocess_contract::BoundedOutcome::TimedOut => {
            return Err(CliProbeError::Process(format!(
                "command={} timed_out_secs={}",
                probe.name(),
                CLI_PROBE_DEADLINE.as_secs()
            )));
        }
        subprocess_contract::BoundedOutcome::Unspawned(error) => {
            return Err(CliProbeError::Process(format!(
                "command={} unavailable={error}",
                probe.name()
            )));
        }
    };

    let value = extract_json(&output.stdout)?;
    let object = value.as_object().ok_or(CliProbeError::WrongTopLevel)?;
    for key in probe.required_keys() {
        if !object.contains_key(*key) {
            return Err(CliProbeError::MissingKey(key));
        }
    }
    Ok(value)
}

/// Render a stable, non-sensitive summary suitable for a robot-facing receipt.
pub fn summary(probe: CliProbe, value: &Value) -> String {
    let object = value.as_object().expect("probe_json validates object output");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    let shape = match probe {
        CliProbe::Models => format!(
            "models_count={}",
            object
                .get("models")
                .and_then(Value::as_array)
                .map_or(0, Vec::len)
        ),
        CliProbe::Stats => format!(
            "overall_type={} by_model_count={} time_series_count={}",
            object.get("overall").map_or("missing", value_kind),
            object
                .get("byModel")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
            object
                .get("timeSeries")
                .and_then(Value::as_array)
                .map_or(0, Vec::len)
        ),
        CliProbe::Usage => format!(
            "reports_count={} capacity_type={}",
            object
                .get("reports")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
            object.get("capacity").map_or("missing", value_kind)
        ),
    };
    format!(
        "OMP_CLI_CONSUMPTION command={} top_keys={} {shape}",
        probe.name(),
        keys.join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_after_stats_status_prefix() {
        let value = extract_json(b"Syncing session files...\n{\"overall\":{}}\n").expect("JSON");
        assert_eq!(value["overall"], serde_json::json!({}));
    }

    #[test]
    fn refuses_missing_json_output() {
        assert!(matches!(extract_json(b"status only"), Err(CliProbeError::EmptyOutput)));
    }

}

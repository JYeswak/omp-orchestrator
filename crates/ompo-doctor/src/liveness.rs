#![forbid(unsafe_code)]

//! Runtime adapters for the L4 source algebra.
//!
//! Each external probe is bounded and its output is converted into the pure
//! \`ompo_start::liveness\` classifier before any portal or walkthrough renders it.

use ompo_start::liveness::{classify, LiveVerdict, SourceVerdict};
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use subprocess_contract::{bounded_output, BoundedOutcome};

const PROBE_DEADLINE: Duration = Duration::from_secs(10);
const MAX_FRESH_MS: u64 = 300_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub verdict: LiveVerdict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnReport {
    pub command: Vec<String>,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn digits(bytes: &[u8], start: usize, count: usize) -> Option<u32> {
    let end = start.checked_add(count)?;
    let slice = bytes.get(start..end)?;
    let mut value = 0u32;
    for byte in slice {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(u32::from(byte - b'0'))?;
    }
    Some(value)
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let adjusted_year = year - i64::from(month <= 2);
    let era = if adjusted_year >= 0 {
        adjusted_year / 400
    } else {
        (adjusted_year - 399) / 400
    };
    let year_of_era = adjusted_year - era * 400;
    let month_prime = i64::from(month) + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Parse the RFC3339 timestamp emitted by Agent Mail, retaining numeric timestamps as fallback.
fn parse_rfc3339_millis(raw: &str) -> Option<u64> {
    let bytes = raw.as_bytes();
    if bytes.len() < 20
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || bytes.get(10) != Some(&b'T')
        || bytes.get(13) != Some(&b':')
        || bytes.get(16) != Some(&b':')
    {
        return None;
    }
    let year = i64::from(digits(bytes, 0, 4)?);
    let month = digits(bytes, 5, 2)?;
    let day = digits(bytes, 8, 2)?;
    let hour = digits(bytes, 11, 2)?;
    let minute = digits(bytes, 14, 2)?;
    let second = digits(bytes, 17, 2)?;
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => return None,
    };
    if day == 0 || day > max_day || hour > 23 || minute > 59 || second > 60 {
        return None;
    }

    let mut cursor = 19;
    let mut millis = 0u32;
    if bytes.get(cursor) == Some(&b'.') {
        cursor += 1;
        let fraction_start = cursor;
        while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
            if cursor - fraction_start < 3 {
                millis = millis * 10 + u32::from(bytes[cursor] - b'0');
            }
            cursor += 1;
        }
        let fraction_len = cursor - fraction_start;
        if fraction_len == 0 {
            return None;
        }
        if fraction_len == 1 {
            millis *= 100;
        } else if fraction_len == 2 {
            millis *= 10;
        }
    }

    let offset_seconds = match bytes.get(cursor) {
        Some(b'Z') if cursor + 1 == bytes.len() => 0i64,
        Some(b'+' | b'-') if cursor + 6 == bytes.len() && bytes.get(cursor + 3) == Some(&b':') => {
            let offset_hours = digits(bytes, cursor + 1, 2)?;
            let offset_minutes = digits(bytes, cursor + 4, 2)?;
            if offset_hours > 23 || offset_minutes > 59 {
                return None;
            }
            let seconds = i64::from(offset_hours * 3_600 + offset_minutes * 60);
            if bytes[cursor] == b'+' { seconds } else { -seconds }
        }
        _ => return None,
    };
    let base_ms = days_from_civil(year, month, day)
        .checked_mul(86_400_000)?
        .checked_add(i64::from(hour * 3_600 + minute * 60 + second) * 1_000)?
        .checked_add(i64::from(millis))?;
    let timestamp_ms = base_ms.checked_sub(offset_seconds.checked_mul(1_000)?)?;
    u64::try_from(timestamp_ms).ok()
}

fn timestamp_ms(value: Option<&Value>) -> Option<u64> {
    match value {
        Some(Value::Number(number)) => {
            let raw = number.as_u64()?;
            if raw < 10_000_000_000 {
                raw.checked_mul(1_000)
            } else {
                Some(raw)
            }
        }
        Some(Value::String(raw)) => parse_rfc3339_millis(raw),
        _ => None,
    }
}

fn age_ms(value: Option<&Value>) -> Option<u64> {
    Some(now_ms().saturating_sub(timestamp_ms(value)?))
}

fn unavailable(name: &str, reason_code: &str, detail: String) -> SourceVerdict {
    SourceVerdict {
        name: name.to_owned(),
        available: false,
        fresh: false,
        reason_code: reason_code.to_owned(),
        age_ms: None,
        panes: Vec::new(),
    }
    .with_detail(detail)
}

trait SourceDetail {
    fn with_detail(self, detail: String) -> Self;
}

impl SourceDetail for SourceVerdict {
    fn with_detail(mut self, detail: String) -> Self {
        if !detail.is_empty() {
            self.reason_code.push_str(" detail=");
            self.reason_code.push_str(&detail);
        }
        self
    }
}

fn execute(binary: &str, args: &[&str]) -> Result<Output, String> {
    let mut command = Command::new(binary);
    command.args(args);
    match bounded_output(&mut command, PROBE_DEADLINE) {
        BoundedOutcome::Completed(output) => Ok(output),
        BoundedOutcome::TimedOut => Err(format!("{binary} timed out after {}s", PROBE_DEADLINE.as_secs())),
        BoundedOutcome::Unspawned(error) => Err(format!("{binary} unavailable: {error}")),
    }
}

fn parse_json(name: &str, output: Output) -> Result<Value, SourceVerdict> {
    if !output.status.success() {
        return Err(unavailable(
            name,
            "L4_SOURCE_EXIT",
            format!("exit={}", output.status),
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|error| {
        unavailable(name, "L4_SOURCE_INVALID_JSON", error.to_string())
    })
}

fn ntm_source() -> SourceVerdict {
    let output = match execute("ntm", &["--robot-snapshot", "--capability-compact"]) {
        Ok(output) => output,
        Err(error) => return unavailable("ntm", "L4_NTM_UNAVAILABLE", error),
    };
    let value = match parse_json("ntm", output) {
        Ok(value) => value,
        Err(source) => return source,
    };
    let schema_ok = value
        .get("schema_id")
        .and_then(Value::as_str)
        .is_some_and(|schema| schema == "ntm:robot:snapshot:v1");
    let source_rows = value
        .get("sources")
        .and_then(|sources| sources.get("sources"))
        .and_then(Value::as_object);
    let all_fresh = value
        .get("sources")
        .and_then(|sources| sources.get("all_fresh"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let age = source_rows.and_then(|rows| rows.values().filter_map(|row| age_ms(row.get("age_ms"))).min());
    let mut panes = value
        .get("sessions")
        .and_then(Value::as_array)
        .into_iter()
        .flat_map(|sessions| sessions.iter())
        .flat_map(|session| session.get("agents").and_then(Value::as_array).into_iter().flatten())
        .filter_map(|agent| agent.get("pane").and_then(Value::as_str))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    panes.sort();
    panes.dedup();
    let available = schema_ok && source_rows.is_some_and(|rows| !rows.is_empty());
    let fresh = available && all_fresh && age.is_some_and(|age| age <= MAX_FRESH_MS);
    SourceVerdict {
        name: "ntm".to_owned(),
        available,
        fresh,
        reason_code: if available { "L4_NTM_SOURCE" } else { "L4_NTM_SILENT" }.to_owned(),
        age_ms: age,
        panes,
    }
}

fn tick_source(session: &str) -> SourceVerdict {
    let output = match execute("tick-monitor", &["observe", "--session", session, "--no-save"]) {
        Ok(output) => output,
        Err(error) => return unavailable("tick-monitor", "L4_TICK_UNAVAILABLE", error),
    };
    let value = match parse_json("tick-monitor", output) {
        Ok(value) => value,
        Err(source) => return source,
    };
    classify_tick(&value)
}

/// Pure tick-observe classifier: gap_secs maps to age_ms, and a missing (or
/// unmeasurable) gap is SILENT, never a default age. Zero is a measured
/// freshness, not a default, so absence must not construct it.
fn classify_tick(value: &Value) -> SourceVerdict {
    let panes = value
        .get("omp_lifecycle")
        .and_then(|lifecycle| lifecycle.get("panes"))
        .and_then(Value::as_array)
        .map(|rows| {
            let mut panes = rows
                .iter()
                .filter_map(|row| row.get("pane").and_then(Value::as_str))
                .map(str::to_owned)
                .collect::<Vec<_>>();
            panes.sort();
            panes.dedup();
            panes
        })
        .unwrap_or_default();
    let age = value
        .get("gap_secs")
        .and_then(Value::as_f64)
        .filter(|gap| gap.is_finite() && *gap >= 0.0)
        .map(|gap| gap * 1_000.0)
        .filter(|ms| ms.is_finite())
        .and_then(|ms| {
            if ms < u64::MAX as f64 {
                Some(ms as u64)
            } else {
                None
            }
        });
    let available = value.get("omp_lifecycle").is_some() && age.is_some();
    let fresh = available && age.is_some_and(|age| age <= MAX_FRESH_MS);
    SourceVerdict {
        name: "tick-monitor".to_owned(),
        available,
        fresh,
        reason_code: if available { "L4_TICK_SOURCE" } else { "L4_TICK_SILENT" }.to_owned(),
        age_ms: age,
        panes,
    }
}

fn mail_source() -> SourceVerdict {
    let output = match execute("am", &["robot", "status", "--json"]) {
        Ok(output) => output,
        Err(error) => return unavailable("agent-mail", "L4_MAIL_UNAVAILABLE", error),
    };
    let value = match parse_json("agent-mail", output) {
        Ok(value) => value,
        Err(source) => return source,
    };
    let meta = value.get("_meta");
    let timestamp = meta.and_then(|meta| meta.get("timestamp"));
    let age = age_ms(timestamp);
    let timestamp_present = timestamp.is_some();
    let timestamp_unmeasured = timestamp_present && age.is_none();
    let mut panes = value
        .get("agents")
        .and_then(Value::as_array)
        .into_iter()
        .flat_map(|agents| agents.iter())
        .filter_map(|agent| agent.get("pane_id").or_else(|| agent.get("pane")))
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    panes.sort();
    panes.dedup();
    let available = meta.is_some();
    let fresh = available && age.is_some_and(|age| age <= MAX_FRESH_MS);
    SourceVerdict {
        name: "agent-mail".to_owned(),
        available,
        fresh,
        reason_code: if fresh {
            "L4_MAIL_SOURCE".to_owned()
        } else if timestamp_unmeasured {
            "L4_MAIL_TIMESTAMP_UNMEASURED".to_owned()
        } else if available && !timestamp_present {
            "L4_MAIL_TIMESTAMP_SILENT".to_owned()
        } else if available {
            "L4_MAIL_STALE".to_owned()
        } else {
            "L4_MAIL_SILENT".to_owned()
        },
        age_ms: age,
        panes,
    }
}

#[must_use]
pub fn observe(session: &str) -> Observation {
    let sources = vec![ntm_source(), tick_source(session), mail_source()];
    let verdict = classify(sources).unwrap_or_else(|error| LiveVerdict::NotLive {
        sources: Vec::new(),
        reason_code: error,
    });
    Observation { verdict }
}

/// Execute the explicitly requested NTM spawn after liveness and HD-0010 gates pass.
pub fn spawn(
    session: &str,
    repo: &Path,
    agents: &[(String, String)],
) -> Result<SpawnReport, String> {
    if agents.is_empty() {
        return Err("L4_SPAWN_AGENTS_REQUIRED — provide at least one --cc or --cod specification".to_owned());
    }
    let repo = repo
        .to_str()
        .ok_or_else(|| "L4_SPAWN_REPO_NOT_UTF8".to_owned())?;
    let mut args = vec!["spawn".to_owned(), session.to_owned()];
    for (flag, value) in agents {
        args.push(flag.clone());
        args.push(value.clone());
    }
    args.extend([
        "--assign".to_owned(),
        "--cass-context".to_owned(),
        repo.to_owned(),
    ]);
    // ONE owner of ntm invocation (io67x): the argv, the bounded spawn, the
    // fresh process group and the exit-first contract belong to `ntm-kernel`.
    // This site keeps its own typed report; what it no longer keeps is a
    // second hand-built Command.
    let run = match ntm_kernel::run_bounded(
        &ntm_kernel::NtmCall::subcommand(args.iter().map(String::as_str)),
        Duration::from_secs(120),
    ) {
        Ok(run) => run,
        Err(ntm_kernel::NtmOutcome::Unanswerable {
            reason_code: "NTM_TIMEOUT",
            ..
        }) => return Err("L4_SPAWN_TIMEOUT — ntm spawn exceeded 120s".to_owned()),
        Err(outcome) => {
            return Err(format!(
                "L4_SPAWN_UNAVAILABLE — {}",
                match &outcome {
                    ntm_kernel::NtmOutcome::Unanswerable { detail, .. } => detail.clone(),
                    other => other.state().to_owned(),
                }
            ))
        }
    };
    Ok(SpawnReport {
        command: args,
        exit_code: run.exit_code,
        stdout: run.stdout,
        stderr: run.stderr,
    })
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_agent_mail_rfc3339_timestamp() {
        assert_eq!(parse_rfc3339_millis("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_rfc3339_millis("1970-01-01T01:00:00+01:00"),
            Some(0)
        );
        assert_eq!(
            parse_rfc3339_millis("2026-09-08T01:11:31.653505+00:00").map(|value| value > 0),
            Some(true)
        );
        assert_eq!(parse_rfc3339_millis("not-a-timestamp"), None);
    }

    #[test]
    fn missing_gap_is_silent() {
        // A present lifecycle with no gap_secs is SILENT, never a default
        // age: zero is a measured freshness, not a default.
        let missing = classify_tick(&json!({"omp_lifecycle": {"panes": [{"pane": "%1"}]}}));
        assert!(!missing.available);
        assert!(!missing.fresh);
        assert_eq!(missing.reason_code, "L4_TICK_SILENT");
        assert_eq!(missing.age_ms, None);
        assert_eq!(missing.panes, vec!["%1".to_owned()]);
        // Positive control: a finite gap classifies as a source with the
        // exact millisecond age, proving the silence above is the missing
        // gap and not a broken classifier.
        let present = classify_tick(&json!({"omp_lifecycle": {"panes": []}, "gap_secs": 12.5}));
        assert!(present.available);
        assert!(present.fresh);
        assert_eq!(present.reason_code, "L4_TICK_SOURCE");
        assert_eq!(present.age_ms, Some(12_500));
    }

    #[test]
    fn timestamp_age_accepts_numeric_and_rfc3339_values() {
        assert!(age_ms(Some(&json!(0))).is_some());
        assert!(age_ms(Some(&json!("2026-09-08T01:11:31.653505+00:00"))).is_some());
        assert_eq!(age_ms(Some(&json!("unknown"))), None);
    }
}

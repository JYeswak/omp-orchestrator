//! Heartbeat (slice b): the supervisor's liveness proof, READABLE BY A THIRD PARTY.
//!
//! THE DEFECT THIS REPLACES: the heartbeat checker was a shell script that
//! wrote no typed row. The four-way identity proof (source == built artifact ==
//! installed artifact == heartbeat build_id) cited "heartbeat row #93 with
//! build_id=85828bf" — but a search of every path under ~/.local/state found
//! NOTHING containing build_id. The identity leg was unverifiable BY
//! CONSTRUCTION, not false.
//!
//! THE CONTRACT: the supervisor writes a HeartbeatRow as JSON to a KNOWN PATH
//! every cycle. A third party who did not write it can find the file, parse
//! the row, and verify build_id and pid. The path is named in the acceptance
//! and in the plist's OMP_ORCHESTRATOR_HEARTBEAT_PATH env var.

use serde_json::Value;
use std::fmt;
use std::path::{Path, PathBuf};

/// The default heartbeat path: the acceptance names it explicitly so a third
/// party can find it without asking the implementer.
pub const DEFAULT_HEARTBEAT_PATH: &str =
    "/Users/josh/.local/state/omp-orchestrator/heartbeat.json";

/// One heartbeat row: the supervisor's liveness proof.
///
/// Every field is required because "a row you can't verify" and "no row" are
/// different defects that currently look the same from outside. build_id and
/// pid are the identity proof legs; ts is the freshness proof; decision names
/// what the supervisor decided, not just that it is alive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatRow {
    pub build_id: String,
    pub pid: u32,
    pub session: String,
    pub repo: String,
    pub decision: String,
    pub ts: u64,
}

impl HeartbeatRow {
    pub fn to_json(&self) -> String {
        serde_json::json!({
            "build_id": self.build_id,
            "pid": self.pid,
            "session": self.session,
            "repo": self.repo,
            "decision": self.decision,
            "ts": self.ts,
        })
        .to_string()
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(text)
            .map_err(|error| format!("heartbeat JSON unparseable: {error}"))?;
        let get = |k: &str| {
            value
                .get(k)
                .and_then(Value::as_str)
                .map(String::from)
                .ok_or_else(|| format!("heartbeat row missing '{k}'"))
        };
        Ok(Self {
            build_id: get("build_id")?,
            pid: value
                .get("pid")
                .and_then(Value::as_u64)
                .ok_or_else(|| "heartbeat row missing 'pid'".to_owned())?
                as u32,
            session: get("session")?,
            repo: get("repo")?,
            decision: get("decision")?,
            ts: value
                .get("ts")
                .and_then(Value::as_u64)
                .ok_or_else(|| "heartbeat row missing 'ts'".to_owned())?,
        })
    }
}

impl fmt::Display for HeartbeatRow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "heartbeat: build_id={} pid={} session={} decision={} ts={}",
            self.build_id, self.pid, self.session, self.decision, self.ts
        )
    }
}

/// Write a heartbeat row to the given path. Creates the parent directory if needed.
pub fn write_heartbeat(path: &Path, row: &HeartbeatRow) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    std::fs::write(path, row.to_json())
        .map_err(|error| format!("cannot write {}: {error}", path.display()))
}

/// Read and validate a heartbeat row, checking freshness against now_unix.
///
/// A row older than max_age_secs is STALE, not missing: "the supervisor wrote
/// a heartbeat and then stopped" is a different defect from "the supervisor
/// never started."
pub fn read_heartbeat(path: &Path, now_unix: u64, max_age_secs: u64) -> Result<HeartbeatRow, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("heartbeat missing: cannot read {}: {error}", path.display()))?;
    let row = HeartbeatRow::from_json(&text)?;
    let age = now_unix.saturating_sub(row.ts);
    if age > max_age_secs {
        return Err(format!(
            "heartbeat STALE: ts={} is {age}s old (max {max_age_secs}s) — the supervisor may be dead or hung",
            row.ts
        ));
    }
    Ok(row)
}

/// Default heartbeat path, overridable via OMP_ORCHESTRATOR_HEARTBEAT_PATH.
pub fn heartbeat_path() -> PathBuf {
    std::env::var_os("OMP_ORCHESTRATOR_HEARTBEAT_PATH")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_HEARTBEAT_PATH))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_BUILD_ID: &str = "85828bf";
    const TEST_PID: u32 = 41826;
    const TEST_SESSION: &str = "omp-orchestrator";

    fn sample_row(ts: u64) -> HeartbeatRow {
        HeartbeatRow {
            build_id: TEST_BUILD_ID.to_owned(),
            pid: TEST_PID,
            session: TEST_SESSION.to_owned(),
            repo: "/Users/josh/Developer/omp-orchestrator".to_owned(),
            decision: "Dispatch".to_owned(),
            ts,
        }
    }

    /// ACCEPTANCE LEG: a fresh row carries build_id and pid — a third party who
    /// did not write it can find the file and verify both.
    #[test]
    fn fresh_row_carries_build_id_and_pid() {
        let path = std::env::temp_dir().join(format!("hb-{}", std::process::id()));
        let row = sample_row(1700000000);
        write_heartbeat(&path, &row).expect("write must succeed");

        let text = std::fs::read_to_string(&path).expect("read must succeed");
        assert!(text.contains(TEST_BUILD_ID), "build_id must be present: {text}");
        assert!(text.contains(&TEST_PID.to_string()), "pid must be present: {text}");

        // A third party reads it back.
        let read = read_heartbeat(&path, 1700000000, 180).expect("read must succeed");
        assert_eq!(read.build_id, TEST_BUILD_ID);
        assert_eq!(read.pid, TEST_PID);
        assert_eq!(read.session, TEST_SESSION);
        let _ = std::fs::remove_file(&path);
    }

    /// A stale heartbeat is a DIFFERENT defect from a missing one.
    #[test]
    fn stale_heartbeat_is_an_error_naming_the_age() {
        let path = std::env::temp_dir().join(format!("hb-stale-{}", std::process::id()));
        let row = sample_row(1000000000);
        write_heartbeat(&path, &row).expect("write must succeed");

        let error = read_heartbeat(&path, 1000000200, 180)
            .expect_err("a 200s-old row must be stale against a 180s max");
        assert!(error.contains("STALE"), "must name the staleness: {error}");
        assert!(error.contains("200"), "must name the age: {error}");
        let _ = std::fs::remove_file(&path);
    }

    /// A missing heartbeat is a DIFFERENT defect from a stale one.
    #[test]
    fn missing_heartbeat_is_an_error_naming_the_path() {
        let path = std::env::temp_dir().join(format!("hb-missing-{}", std::process::id()));
        let error = read_heartbeat(&path, 1700000000, 180)
            .expect_err("a missing file must be an error");
        assert!(error.contains("heartbeat missing"), "{error}");
        assert!(error.contains(&path.display().to_string()), "{error}");
        let _ = std::fs::remove_file(&path);
    }

    /// JSON round-trip: every field survives.
    #[test]
    fn json_round_trip_preserves_all_fields() {
        let original = sample_row(1700000000);
        let json = original.to_json();
        let parsed = HeartbeatRow::from_json(&json).expect("parse must succeed");
        assert_eq!(original, parsed);
    }

    /// A malformed heartbeat row is an error naming the missing field.
    #[test]
    fn malformed_row_names_the_missing_field() {
        let result = HeartbeatRow::from_json(r#"{"build_id": "x"}"#);
        assert!(result.is_err(), "a row without pid must fail");
        assert!(result.unwrap_err().contains("pid"), "must name the field");
    }
}

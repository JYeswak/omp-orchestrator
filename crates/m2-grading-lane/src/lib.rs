#![forbid(unsafe_code)]

//! Named consumer of `dispatch_saga::m2::route`. Decide-only.
//!
//! The live executor is launchd
//! `ai.zeststream.omp-orchestrator.m2-grading-lane` firing this crate's
//! bin with no `--apply`. Exit code is not evidence it ran; the oracle
//! at `$HOME/.local/state/flywheel/m2-grading-lane.jsonl` is.

pub use dispatch_saga::m2::{
    lineage_eligible, lineage_from_argv, route, GradingBead, ModelLineage, Pane, RouteDecision,
};

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const LANE: &str = "m2-grading-lane";
pub const ORACLE_SCHEMA: &str = "m2-grading-lane.tick.v1";
pub const DEFAULT_ORACLE_RELATIVE: &str = ".local/state/flywheel/m2-grading-lane.jsonl";
pub const DECISION_NO_FEED: &str = "DECIDE_ONLY_NO_FEED";
pub const DECISION_APPLY_REFUSED: &str = "APPLY_REFUSED_NO_FEED";

/// The lane entry: same decision as the kernel, so the kernel has a caller.
pub fn autoroute(
    bead: &GradingBead,
    panes: &[Pane],
) -> Result<RouteDecision, dispatch_saga::m2::ReportError> {
    route(bead, panes)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickRecord {
    pub ts_unix: u64,
    pub apply: bool,
    pub fired: bool,
    pub decision: String,
    pub bead: Option<String>,
    pub grader_pane: Option<String>,
    pub reason: String,
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn default_oracle_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("M2_ORACLE") {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME").ok_or_else(|| {
        "HOME_UNSET: set M2_ORACLE or HOME for the artifact oracle".to_owned()
    })?;
    Ok(PathBuf::from(home).join(DEFAULT_ORACLE_RELATIVE))
}

fn json_escape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out
}

fn opt_json(value: &Option<String>) -> String {
    match value {
        Some(v) => format!("\"{}\"", json_escape(v)),
        None => "null".to_owned(),
    }
}

pub fn render_tick_line(record: &TickRecord) -> String {
    format!(
        "{{\"schema\":\"{ORACLE_SCHEMA}\",\"ts\":{},\"lane\":\"{LANE}\",\"apply\":{},\"fired\":{},\"decision\":\"{}\",\"bead\":{},\"grader_pane\":{},\"reason\":\"{}\"}}\n",
        record.ts_unix,
        record.apply,
        record.fired,
        json_escape(&record.decision),
        opt_json(&record.bead),
        opt_json(&record.grader_pane),
        json_escape(&record.reason),
    )
}

/// Append one oracle row and fsync. Exit code is not this function.
pub fn append_oracle(path: &Path, record: &TickRecord) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!("ORACLE_PARENT_UNWRITABLE path={} error={error}", parent.display())
        })?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("ORACLE_UNWRITABLE path={} error={error}", path.display()))?;
    file.write_all(render_tick_line(record).as_bytes())
        .map_err(|error| format!("ORACLE_WRITE path={} error={error}", path.display()))?;
    file.sync_all()
        .map_err(|error| format!("ORACLE_FSYNC path={} error={error}", path.display()))?;
    Ok(())
}

pub fn decide_only_no_feed_record() -> TickRecord {
    TickRecord {
        ts_unix: now_unix(),
        apply: false,
        fired: true,
        decision: DECISION_NO_FEED.to_owned(),
        bead: None,
        grader_pane: None,
        reason: "default path is decide-only; no candidate feed; no ntm send; no br write"
            .to_owned(),
    }
}

pub fn apply_refused_record() -> TickRecord {
    TickRecord {
        ts_unix: now_unix(),
        apply: false,
        fired: true,
        decision: DECISION_APPLY_REFUSED.to_owned(),
        bead: None,
        grader_pane: None,
        reason: "--apply refused without an explicit candidate feed; send not executed"
            .to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consumer_calls_kernel_route() {
        let bead = GradingBead {
            id: "omp-orchestrator-exit-const-collision-cas".into(),
            status: "grading".into(),
            implementer_pane: "%9".into(),
            implementer_profile: ModelLineage::Grok,
        };
        let pane = Pane {
            pane: "%8".into(),
            argv: "omp --profile codex".into(),
            idle: true,
            safe_to_dispatch: true,
        };
        match autoroute(&bead, &[pane]).unwrap() {
            RouteDecision::Route { grader_profile, .. } => {
                assert_eq!(grader_profile, ModelLineage::Codex);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn oracle_line_names_schema_and_decide_only() {
        let line = render_tick_line(&decide_only_no_feed_record());
        assert!(line.contains(ORACLE_SCHEMA), "{line}");
        assert!(line.contains(DECISION_NO_FEED), "{line}");
        assert!(line.contains("\"apply\":false"), "{line}");
        assert!(!line.contains("\"apply\":true"), "{line}");
    }
}

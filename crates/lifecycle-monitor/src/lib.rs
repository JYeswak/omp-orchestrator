//! S1 observability monitor over `lifecycle-event` journals.
//!
//! # Failure this crate exists to prevent
//!
//! A monitor that reports the same verdict on a healthy log and an empty one.
//! Empty scan set is an ERROR, never a pass.
//!
//! One binary, `--layer L0..L5` (or all six). Two journal carriers, same as the
//! writer: repo JSONL and host JSONL. L0 host sink is a path, not a second crate.

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use lifecycle_event::{
    default_host_journal, default_repo_journal, readback_after_claimed_write, DurableJournal,
    EmitError, Layer, LifecycleEvent,
};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerState {
    Progressing,
    Silent,
    Refusing,
}

impl LayerState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Progressing => "progressing",
            Self::Silent => "silent",
            Self::Refusing => "refusing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerVerdict {
    pub layer: Layer,
    pub state: LayerState,
    pub row_count: usize,
    pub last_reason: String,
    pub age_ms: u64,
    pub fresh: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorError {
    EmptyScan { path: PathBuf, layer: Option<Layer> },
    Io { path: PathBuf, detail: String },
    Malformed { line: usize, detail: String },
    MissingReason { line: usize },
    MissingTimestamp { line: usize },
    MalformedTimestamp { line: usize, detail: String },
    ReadbackFailed { detail: String },
    MetricsMissing { path: PathBuf },
    MetricsIncomplete { found: usize },
    StaleLayers { layers: Vec<String> },
}

impl std::fmt::Display for MonitorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyScan { path, layer } => match layer {
                Some(layer) => write!(
                    f,
                    "LIFECYCLE_MONITOR_EMPTY_SCAN path={} layer={} — empty is ERROR, never a pass",
                    path.display(),
                    layer.as_str()
                ),
                None => write!(
                    f,
                    "LIFECYCLE_MONITOR_EMPTY_SCAN path={} — empty is ERROR, never a pass",
                    path.display()
                ),
            },
            Self::Io { path, detail } => {
                write!(f, "LIFECYCLE_MONITOR_IO path={} detail={detail}", path.display())
            }
            Self::Malformed { line, detail } => {
                write!(f, "LIFECYCLE_MONITOR_MALFORMED line={line} detail={detail}")
            }
            Self::MissingReason { line } => write!(
                f,
                "LIFECYCLE_MONITOR_MISSING_REASON line={line} — idle and refused must stay distinguishable"
            ),
            Self::MissingTimestamp { line } => {
                write!(f, "LIFECYCLE_MONITOR_MISSING_TIMESTAMP line={line}")
            }
            Self::MalformedTimestamp { line, detail } => write!(
                f,
                "LIFECYCLE_MONITOR_MALFORMED_TIMESTAMP line={line} detail={detail}"
            ),
            Self::ReadbackFailed { detail } => write!(
                f,
                "LIFECYCLE_MONITOR_READBACK_FAILED {detail} — the write succeeded is not evidence the artifact exists"
            ),
            Self::MetricsMissing { path } => {
                write!(f, "LIFECYCLE_MONITOR_METRICS_MISSING path={}", path.display())
            }
            Self::MetricsIncomplete { found } => write!(
                f,
                "LIFECYCLE_MONITOR_METRICS_INCOMPLETE found={found} expected=6"
            ),
            Self::StaleLayers { layers } => write!(
                f,
                "LIFECYCLE_MONITOR_STALE_LAYERS layers={} — stale is ERROR, never a pass",
                layers.join(",")
            ),
        }
    }
}

impl std::error::Error for MonitorError {}

#[derive(Debug, Clone, PartialEq)]
pub struct MetricSpec {
    pub id: String,
    pub layer: Layer,
    pub floor: f64,
    pub stall_after_ms: u64,
}

pub const EXPECTED_METRIC_COUNT: usize = 6;

/// Load `METRICS.toml`. Refuses fewer than six layer rows.
pub fn load_metrics(path: &Path) -> Result<Vec<MetricSpec>, MonitorError> {
    let text = fs::read_to_string(path).map_err(|_e| MonitorError::MetricsMissing {
        path: path.to_path_buf(),
    })?;
    let mut specs = Vec::new();
    let mut id = String::new();
    let mut layer = String::new();
    let mut floor = String::new();
    let mut stall = String::new();
    let mut in_metric = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if line == "[[metric]]" {
            if in_metric {
                specs.push(finish_metric(&id, &layer, &floor, &stall)?);
            }
            in_metric = true;
            id.clear();
            layer.clear();
            floor.clear();
            stall.clear();
            continue;
        }
        if !in_metric {
            continue;
        }
        if let Some(rest) = line.strip_prefix("id = ") {
            id = unquote(rest);
        } else if let Some(rest) = line.strip_prefix("layer = ") {
            layer = unquote(rest);
        } else if let Some(rest) = line.strip_prefix("floor = ") {
            floor = rest.trim().to_owned();
        } else if let Some(rest) = line.strip_prefix("stall_after_ms = ") {
            stall = rest.trim().to_owned();
        }
    }
    if in_metric {
        specs.push(finish_metric(&id, &layer, &floor, &stall)?);
    }
    if specs.len() != EXPECTED_METRIC_COUNT {
        return Err(MonitorError::MetricsIncomplete {
            found: specs.len(),
        });
    }
    Ok(specs)
}

fn unquote(raw: &str) -> String {
    raw.trim().trim_matches('"').to_owned()
}

fn finish_metric(
    id: &str,
    layer: &str,
    floor: &str,
    stall: &str,
) -> Result<MetricSpec, MonitorError> {
    let layer = Layer::parse(layer).map_err(|e| MonitorError::Malformed {
        line: 0,
        detail: e.to_string(),
    })?;
    let floor: f64 = floor.parse().map_err(|_| MonitorError::Malformed {
        line: 0,
        detail: format!("bad floor {floor}"),
    })?;
    let stall_after_ms: u64 = stall.parse().map_err(|_| MonitorError::Malformed {
        line: 0,
        detail: format!("bad stall_after_ms {stall}"),
    })?;
    if id.is_empty() {
        return Err(MonitorError::Malformed {
            line: 0,
            detail: "metric missing id".to_owned(),
        });
    }
    Ok(MetricSpec {
        id: id.to_owned(),
        layer,
        floor,
        stall_after_ms,
    })
}

#[derive(Debug, Clone)]
struct JournalRow {
    layer: Layer,
    outcome: String,
    reason: String,
    ts_unix: u64,
}

fn parse_rows(path: &Path) -> Result<Vec<JournalRow>, MonitorError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(MonitorError::EmptyScan {
                path: path.to_path_buf(),
                layer: None,
            })
        }
        Err(err) => {
            return Err(MonitorError::Io {
                path: path.to_path_buf(),
                detail: err.to_string(),
            })
        }
    };
    if text.trim().is_empty() {
        return Err(MonitorError::EmptyScan {
            path: path.to_path_buf(),
            layer: None,
        });
    }
    let mut rows = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line).map_err(|e| MonitorError::Malformed {
            line: i + 1,
            detail: e.to_string(),
        })?;
        let reason = value
            .get("reason_code")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        if reason.trim().is_empty() {
            return Err(MonitorError::MissingReason { line: i + 1 });
        }
        let layer_raw = value
            .get("layer")
            .and_then(Value::as_str)
            .unwrap_or("");
        let layer = Layer::parse(layer_raw).map_err(|e| MonitorError::Malformed {
            line: i + 1,
            detail: e.to_string(),
        })?;
        let outcome = value
            .get("outcome")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let timestamp = value.get("ts_unix").ok_or(MonitorError::MissingTimestamp { line: i + 1 })?;
        let ts_unix = timestamp
            .as_u64()
            .ok_or_else(|| MonitorError::MalformedTimestamp {
                line: i + 1,
                detail: timestamp.to_string(),
            })?;
        rows.push(JournalRow {
            layer,
            outcome,
            reason,
            ts_unix,
        });
    }
    if rows.is_empty() {
        return Err(MonitorError::EmptyScan {
            path: path.to_path_buf(),
            layer: None,
        });
    }
    Ok(rows)
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

/// Observe one layer. Empty set for that layer is ERROR.
pub fn observe_layer(
    journal: &Path,
    layer: Layer,
    stall_after_ms: u64,
) -> Result<LayerVerdict, MonitorError> {
    let rows = parse_rows(journal)?;
    let filtered: Vec<&JournalRow> = rows.iter().filter(|r| r.layer == layer).collect();
    if filtered.is_empty() {
        return Err(MonitorError::EmptyScan {
            path: journal.to_path_buf(),
            layer: Some(layer),
        });
    }
    let last = filtered.last().expect("non-empty");
    let age_ms = now_unix_ms().saturating_sub(last.ts_unix.saturating_mul(1000));
    let fresh = age_ms <= stall_after_ms;
    let state = if last.outcome == "refused" {
        LayerState::Refusing
    } else if !fresh {
        LayerState::Silent
    } else {
        LayerState::Progressing
    };
    Ok(LayerVerdict {
        layer,
        state,
        row_count: filtered.len(),
        last_reason: last.reason.clone(),
        age_ms,
        fresh,
    })
}

/// Observe every layer named in `metrics`. Any empty layer is ERROR.
pub fn observe_all(
    journal: &Path,
    metrics: &[MetricSpec],
) -> Result<Vec<LayerVerdict>, MonitorError> {
    let mut out = Vec::new();
    for spec in metrics {
        out.push(observe_layer(journal, spec.layer, spec.stall_after_ms)?);
    }
    Ok(out)
}

/// Freshness gate over observed verdicts (3s6a): every observed layer must be
/// progressing. One silent or refusing layer fails the gate with the stale set
/// named -- a single stale layer vetoes a fleet-wide clean, never the reverse.
pub fn gate_freshness_verdict(verdicts: &[LayerVerdict]) -> Result<(), MonitorError> {
    let stale: Vec<String> = verdicts
        .iter()
        .filter(|verdict| verdict.state != LayerState::Progressing)
        .map(|verdict| {
            format!(
                "{}:{}",
                verdict.layer.as_str(),
                verdict.state.as_str()
            )
        })
        .collect();
    if stale.is_empty() {
        return Ok(());
    }
    Err(MonitorError::StaleLayers { layers: stale })
}

/// Independent post-fact read of the journal. Does not go through `emit_host`.
///
/// The writer's readback proves the write path. This proves the artifact still
/// exists when a *different* crate opens the file. Empty is ERROR.
pub fn verify_artifact(path: &Path) -> Result<usize, MonitorError> {
    let rows = parse_rows(path)?;
    Ok(rows.len())
}


/// Gate: claimed write whose readback fails MUST refuse.
pub fn gate_claimed_write_readback(
    journal: &DurableJournal,
    claimed: &[LifecycleEvent],
) -> Result<(), MonitorError> {
    match readback_after_claimed_write(journal, claimed) {
        Ok(_) => Ok(()),
        Err(EmitError::ReadbackFailed { expected, found }) => Err(MonitorError::ReadbackFailed {
            detail: format!("expected={expected} found={found}"),
        }),
        Err(other) => Err(MonitorError::ReadbackFailed {
            detail: other.to_string(),
        }),
    }
}

pub fn journal_for_repo(repo: &Path) -> PathBuf {
    default_repo_journal(repo)
}

pub fn journal_for_host() -> PathBuf {
    default_host_journal()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifecycle_event::{emit_one_host, EmitOutcome, ReasonCode};
    use tempfile::tempdir;

    fn event(layer: Layer, reason: &str, outcome: EmitOutcome) -> LifecycleEvent {
        LifecycleEvent::new(
            layer,
            "HUMAN",
            format!("S1.{}", layer.as_str()),
            "test",
            outcome,
            ReasonCode::new(reason).unwrap(),
        )
    }

    fn metrics_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("METRICS.toml")
    }

    #[test]
    fn empty_journal_is_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("lifecycle.jsonl");
        let err = observe_layer(&path, Layer::L0, 60_000).expect_err("empty");
        assert!(matches!(err, MonitorError::EmptyScan { .. }));
    }

    #[test]
    fn empty_layer_filter_is_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("lifecycle.jsonl");
        let journal = DurableJournal::open(&path).unwrap();
        emit_one_host(&journal, event(Layer::L4, "OBSERVE_OK", EmitOutcome::Emitted)).unwrap();
        let err = observe_layer(&path, Layer::L0, 60_000).expect_err("no L0");
        match err {
            MonitorError::EmptyScan { layer: Some(Layer::L0), .. } => {}
            other => panic!("expected L0 empty, got {other:?}"),
        }
    }

    #[test]
    fn progressing_on_fresh_emit() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("lifecycle.jsonl");
        let journal = DurableJournal::open(&path).unwrap();
        emit_one_host(&journal, event(Layer::L4, "OBSERVE_OK", EmitOutcome::Emitted)).unwrap();
        let v = observe_layer(&path, Layer::L4, 60_000).expect("observe");
        assert_eq!(v.state, LayerState::Progressing);
        assert_eq!(v.row_count, 1);
        assert_eq!(v.last_reason, "OBSERVE_OK");
    }

    #[test]
    fn refusing_on_refused_outcome() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("lifecycle.jsonl");
        let journal = DurableJournal::open(&path).unwrap();
        emit_one_host(&journal, event(Layer::L1, "IDENTITY_DRIFT", EmitOutcome::Refused)).unwrap();
        let v = observe_layer(&path, Layer::L1, 60_000).expect("observe");
        assert_eq!(v.state, LayerState::Refusing);
    }

    #[test]
    fn stalled_when_age_exceeds_threshold() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("lifecycle.jsonl");
        std::fs::write(
            &path,
            r#"{"schema":"omp.lifecycle_event.v1","layer":"L3","stage_from":"S1.L2","stage_to":"S1.L3","actor":"test","outcome":"emitted","reason_code":"DECIDE","ts_unix":1}"#,
        )
        .unwrap();
        let v = observe_layer(&path, Layer::L3, 1000).expect("observe");
        assert_eq!(v.state, LayerState::Silent);
    }

    #[test]
    fn gate_fires_when_claimed_write_has_no_readback() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("lifecycle.jsonl");
        let journal = DurableJournal::open(&path).unwrap();
        let claimed = event(Layer::L5, "PORTAL_ROW", EmitOutcome::Emitted);
        let err = gate_claimed_write_readback(&journal, &[claimed]).expect_err("missing");
        assert!(matches!(err, MonitorError::ReadbackFailed { .. }));
    }

    #[test]
    fn gate_fires_after_truncating_a_successful_write() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("lifecycle.jsonl");
        let journal = DurableJournal::open(&path).unwrap();
        let claimed = event(Layer::L0, "INSTALL_VERIFIED", EmitOutcome::Emitted);
        emit_one_host(&journal, claimed.clone()).unwrap();
        std::fs::write(&path, "").unwrap();
        let err = gate_claimed_write_readback(&journal, &[claimed]).expect_err("truncated");
        assert!(matches!(err, MonitorError::ReadbackFailed { .. }));
    }

    #[test]
    fn metrics_toml_homes_six_layers() {
        let specs = load_metrics(&metrics_path()).expect("METRICS.toml");
        assert_eq!(specs.len(), 6);
        let layers: Vec<&str> = specs.iter().map(|s| s.layer.as_str()).collect();
        assert_eq!(layers, ["L0", "L1", "L2", "L3", "L4", "L5"]);
        assert!(specs.iter().all(|s| s.stall_after_ms > 0));
    }

    #[test]
    fn missing_reason_code_line_is_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("lifecycle.jsonl");
        std::fs::write(
            &path,
            r#"{"schema":"omp.lifecycle_event.v1","layer":"L2","outcome":"emitted"}"#,
        )
        .unwrap();
        let err = observe_layer(&path, Layer::L2, 60_000).expect_err("no reason");
        assert!(matches!(err, MonitorError::MissingReason { .. }));
    }

    #[test]
    fn verify_artifact_reads_without_the_writer() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("lifecycle.jsonl");
        std::fs::write(
            &path,
            r#"{"schema":"omp.lifecycle_event.v1","layer":"L5","stage_from":"S1.L4","stage_to":"S1.L5","actor":"test","outcome":"emitted","reason_code":"PORTAL_ROW","ts_unix":1}
"#,
        )
        .unwrap();
        let n = verify_artifact(&path).expect("independent read");
        assert_eq!(n, 1);
        std::fs::write(&path, "").unwrap();
        let err = verify_artifact(&path).expect_err("empty after truncate");
        assert!(matches!(err, MonitorError::EmptyScan { .. }));
    }
}



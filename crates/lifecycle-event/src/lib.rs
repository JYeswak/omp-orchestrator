//! S1 `LifecycleEvent` writer.
//!
//! # Failure this crate exists to prevent
//!
//! `S1.toml:21` and `SCHEMAS.toml` declare a `LifecycleEvent` row. Zero instances
//! existed on disk. A schema without a writer is a description. This crate is the
//! writer: construction refuses a missing `reason_code`, an empty emit set is an
//! error, and a write is not done until readback succeeds.
//!
//! # Two carriers, one type
//!
//! L1–L5 run inside a repo with `&Cx`. L0 (installer) has **zero Cx** — measured,
//! not aesthetic. Forcing one file + one runtime would invent a runtime the install
//! path cannot own. The row type is shared. The sinks differ:
//!
//! - [`emit_host`] — no Cx; L0 and any crate that cannot take asupersync
//! - [`emit`] — `&Cx` first, checkpoints around the durable write (feature `cx`)
//!
//! Same fsync-file-and-parent, same readback.

#![forbid(unsafe_code)]

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

pub const SCHEMA: &str = "omp.lifecycle_event.v1";
pub const RELATIVE_JOURNAL: &str = ".omp-orchestrator/work/s1/lifecycle.jsonl";

/// S1 layer the row belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    L0,
    L1,
    L2,
    L3,
    L4,
    L5,
}

impl Layer {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::L0 => "L0",
            Self::L1 => "L1",
            Self::L2 => "L2",
            Self::L3 => "L3",
            Self::L4 => "L4",
            Self::L5 => "L5",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, EmitError> {
        match raw {
            "L0" => Ok(Self::L0),
            "L1" => Ok(Self::L1),
            "L2" => Ok(Self::L2),
            "L3" => Ok(Self::L3),
            "L4" => Ok(Self::L4),
            "L5" => Ok(Self::L5),
            _ => Err(EmitError::UnknownLayer {
                got: raw.to_owned(),
            }),
        }
    }
}

/// Why the row exists. Empty is unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReasonCode(String);

impl ReasonCode {
    /// Refuses empty / whitespace so idle and refused stay distinguishable.
    pub fn new(raw: impl Into<String>) -> Result<Self, EmitError> {
        let value = raw.into();
        if value.trim().is_empty() {
            return Err(EmitError::MissingReasonCode);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Emitted,
    Refused,
    Idle,
}

impl Outcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Emitted => "emitted",
            Self::Refused => "refused",
            Self::Idle => "idle",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, EmitError> {
        match raw {
            "emitted" => Ok(Self::Emitted),
            "refused" => Ok(Self::Refused),
            "idle" => Ok(Self::Idle),
            _ => Err(EmitError::UnknownOutcome {
                got: raw.to_owned(),
            }),
        }
    }
}

/// One S1 lifecycle row. Extra L3 fields are optional, never a second type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleEvent {
    layer: Layer,
    stage_from: String,
    stage_to: String,
    actor: String,
    pane: String,
    incarnation: String,
    outcome: Outcome,
    reason_code: ReasonCode,
    blocker: String,
    step: String,
    status: String,
    next_command: String,
}

impl LifecycleEvent {
    pub fn new(
        layer: Layer,
        stage_from: impl Into<String>,
        stage_to: impl Into<String>,
        actor: impl Into<String>,
        outcome: Outcome,
        reason_code: ReasonCode,
    ) -> Self {
        Self {
            layer,
            stage_from: stage_from.into(),
            stage_to: stage_to.into(),
            actor: actor.into(),
            pane: String::new(),
            incarnation: String::new(),
            outcome,
            reason_code,
            blocker: String::new(),
            step: String::new(),
            status: String::new(),
            next_command: String::new(),
        }
    }

    pub fn with_pane(mut self, pane: impl Into<String>) -> Self {
        self.pane = pane.into();
        self
    }

    pub fn with_incarnation(mut self, incarnation: impl Into<String>) -> Self {
        self.incarnation = incarnation.into();
        self
    }

    pub fn with_blocker(mut self, blocker: impl Into<String>) -> Self {
        self.blocker = blocker.into();
        self
    }

    pub fn with_step(
        mut self,
        step: impl Into<String>,
        status: impl Into<String>,
        next_command: impl Into<String>,
    ) -> Self {
        self.step = step.into();
        self.status = status.into();
        self.next_command = next_command.into();
        self
    }

    pub fn layer(&self) -> Layer {
        self.layer
    }

    pub fn reason_code(&self) -> &str {
        self.reason_code.as_str()
    }

    pub fn to_json_line(&self) -> String {
        let mut map = serde_json::Map::new();
        map.insert("schema".into(), json!(SCHEMA));
        map.insert("layer".into(), json!(self.layer.as_str()));
        map.insert("stage_from".into(), json!(self.stage_from));
        map.insert("stage_to".into(), json!(self.stage_to));
        map.insert("actor".into(), json!(self.actor));
        map.insert("outcome".into(), json!(self.outcome.as_str()));
        map.insert("reason_code".into(), json!(self.reason_code.as_str()));
        if !self.pane.is_empty() {
            map.insert("pane".into(), json!(self.pane));
        }
        if !self.incarnation.is_empty() {
            map.insert("incarnation".into(), json!(self.incarnation));
        }
        if !self.blocker.is_empty() {
            map.insert("blocker".into(), json!(self.blocker));
        }
        if !self.step.is_empty() {
            map.insert("step".into(), json!(self.step));
            map.insert("status".into(), json!(self.status));
            map.insert("next_command".into(), json!(self.next_command));
        }
        Value::Object(map).to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmitError {
    MissingReasonCode,
    EmptyBatch,
    UnknownLayer { got: String },
    UnknownOutcome { got: String },
    Cancelled,
    Io { op: &'static str, detail: String },
    ReadbackFailed { expected: String, found: String },
}

impl std::fmt::Display for EmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingReasonCode => f.write_str(
                "LIFECYCLE_EVENT_MISSING_REASON_CODE — empty reason_code is refused at construction, not at read",
            ),
            Self::EmptyBatch => f.write_str(
                "LIFECYCLE_EVENT_EMPTY_BATCH — an empty emit set is an ERROR, never a pass",
            ),
            Self::UnknownLayer { got } => {
                write!(f, "LIFECYCLE_EVENT_UNKNOWN_LAYER got={got}")
            }
            Self::UnknownOutcome { got } => {
                write!(f, "LIFECYCLE_EVENT_UNKNOWN_OUTCOME got={got}")
            }
            Self::Cancelled => f.write_str("LIFECYCLE_EVENT_CANCELLED"),
            Self::Io { op, detail } => {
                write!(f, "LIFECYCLE_EVENT_IO op={op} detail={detail}")
            }
            Self::ReadbackFailed { expected, found } => write!(
                f,
                "LIFECYCLE_EVENT_READBACK_FAILED expected={expected} found={found} — a write that returns zero is not evidence the artifact exists"
            ),
        }
    }
}

impl std::error::Error for EmitError {}

fn io(op: &'static str, err: impl ToString) -> EmitError {
    EmitError::Io {
        op,
        detail: err.to_string(),
    }
}

/// Durable JSONL journal. Fsyncs the file **and** its parent directory.
pub struct DurableJournal {
    path: PathBuf,
}

impl DurableJournal {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, EmitError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| io("create_dir_all", e))?;
        }
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn append_lines(&self, lines: &[String]) -> Result<(), EmitError> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| io("open_append", e))?;
        for line in lines {
            file.write_all(line.as_bytes())
                .map_err(|e| io("write", e))?;
            file.write_all(b"\n").map_err(|e| io("write_newline", e))?;
        }
        file.sync_all().map_err(|e| io("fsync_file", e))?;
        fsync_parent(&self.path)?;
        Ok(())
    }

    fn read_text(&self) -> Result<String, EmitError> {
        match fs::read_to_string(&self.path) {
            Ok(text) => Ok(text),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(String::new()),
            Err(err) => Err(io("read", err)),
        }
    }
}

fn fsync_parent(path: &Path) -> Result<(), EmitError> {
    let parent = path.parent().ok_or_else(|| EmitError::Io {
        op: "parent",
        detail: "journal path has no parent directory".to_owned(),
    })?;
    let dir = File::open(parent).map_err(|e| io("open_parent", e))?;
    dir.sync_all().map_err(|e| io("fsync_parent", e))?;
    Ok(())
}

/// Proof that the last emitted lines exist on disk as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Readback {
    pub path: PathBuf,
    pub lines: usize,
}

fn require_readback(journal: &DurableJournal, lines: &[String]) -> Result<Readback, EmitError> {
    let text = journal.read_text()?;
    if text.trim().is_empty() {
        return Err(EmitError::ReadbackFailed {
            expected: lines.join("\n"),
            found: String::new(),
        });
    }
    let on_disk: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
    if on_disk.len() < lines.len() {
        return Err(EmitError::ReadbackFailed {
            expected: lines.join("\n"),
            found: text,
        });
    }
    let tail = &on_disk[on_disk.len() - lines.len()..];
    for (want, got) in lines.iter().zip(tail.iter()) {
        if want != got {
            return Err(EmitError::ReadbackFailed {
                expected: want.clone(),
                found: (*got).to_owned(),
            });
        }
    }
    Ok(Readback {
        path: journal.path().to_path_buf(),
        lines: on_disk.len(),
    })
}

fn encoded_batch(events: &[LifecycleEvent]) -> Result<Vec<String>, EmitError> {
    if events.is_empty() {
        return Err(EmitError::EmptyBatch);
    }
    Ok(events.iter().map(LifecycleEvent::to_json_line).collect())
}

/// Host-path emit: no Cx. Used by L0 installer and tick-monitor (no asupersync).
pub fn emit_host(
    journal: &DurableJournal,
    events: &[LifecycleEvent],
) -> Result<Readback, EmitError> {
    let lines = encoded_batch(events)?;
    journal.append_lines(&lines)?;
    require_readback(journal, &lines)
}

/// Convenience: one event, host sink.
pub fn emit_one_host(
    journal: &DurableJournal,
    event: LifecycleEvent,
) -> Result<Readback, EmitError> {
    emit_host(journal, std::slice::from_ref(&event))
}

pub fn default_repo_journal(repo: &Path) -> PathBuf {
    repo.join(RELATIVE_JOURNAL)
}

pub fn default_host_journal() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_owned());
    PathBuf::from(home)
        .join(".local/state/zeststream/scratch/s1-lifecycle/lifecycle.jsonl")
}

/// Test-only: pretend the write returned success, then prove readback refuses.
///
/// Production [`emit_host`] cannot skip readback. This helper exists so the
/// known-bad leg can construct the "write succeeded, artifact missing" state.
pub fn readback_after_claimed_write(
    journal: &DurableJournal,
    claimed: &[LifecycleEvent],
) -> Result<Readback, EmitError> {
    let lines = encoded_batch(claimed)?;
    require_readback(journal, &lines)
}

#[cfg(feature = "cx")]
pub async fn emit(
    cx: &asupersync::Cx,
    journal: &DurableJournal,
    events: &[LifecycleEvent],
) -> Result<Readback, EmitError> {
    if cx.checkpoint().is_err() {
        return Err(EmitError::Cancelled);
    }
    let result = emit_host(journal, events);
    if cx.checkpoint().is_err() {
        return Err(EmitError::Cancelled);
    }
    result
}

#[cfg(feature = "cx")]
pub async fn emit_one(
    cx: &asupersync::Cx,
    journal: &DurableJournal,
    event: LifecycleEvent,
) -> Result<Readback, EmitError> {
    emit(cx, journal, std::slice::from_ref(&event)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn code(raw: &str) -> ReasonCode {
        ReasonCode::new(raw).expect("reason")
    }

    fn row(layer: Layer, reason: &str) -> LifecycleEvent {
        LifecycleEvent::new(
            layer,
            "HUMAN",
            format!("S1.{}", layer.as_str()),
            "test",
            Outcome::Emitted,
            code(reason),
        )
    }

    #[test]
    fn rejects_empty_reason_code() {
        let err = ReasonCode::new("").expect_err("empty");
        assert_eq!(err, EmitError::MissingReasonCode);
        let err = ReasonCode::new("   ").expect_err("whitespace");
        assert_eq!(err, EmitError::MissingReasonCode);
    }

    #[test]
    fn rejects_empty_batch() {
        let dir = tempdir().unwrap();
        let journal = DurableJournal::open(dir.path().join("lifecycle.jsonl")).unwrap();
        let err = emit_host(&journal, &[]).expect_err("empty");
        assert_eq!(err, EmitError::EmptyBatch);
    }

    #[test]
    fn host_emit_roundtrips_and_readback() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("lifecycle.jsonl");
        let journal = DurableJournal::open(&path).unwrap();
        let event = row(Layer::L0, "INSTALL_VERIFIED");
        let back = emit_one_host(&journal, event.clone()).expect("emit");
        assert_eq!(back.lines, 1);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("INSTALL_VERIFIED"));
        assert!(text.contains(SCHEMA));
        assert!(path.parent().unwrap().is_dir());
    }

    #[test]
    fn refuses_when_readback_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("lifecycle.jsonl");
        let journal = DurableJournal::open(&path).unwrap();
        let event = row(Layer::L4, "THREE_FRESH");
        // Claimed write: nothing on disk.
        let err = readback_after_claimed_write(&journal, &[event]).expect_err("missing");
        match err {
            EmitError::ReadbackFailed { found, .. } => assert!(found.is_empty()),
            other => panic!("expected ReadbackFailed, got {other:?}"),
        }
    }

    #[test]
    fn refuses_when_readback_mismatches_after_truncation() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("lifecycle.jsonl");
        let journal = DurableJournal::open(&path).unwrap();
        let event = row(Layer::L1, "DOCTOR_OK");
        emit_one_host(&journal, event.clone()).unwrap();
        std::fs::write(&path, "").unwrap();
        let err = readback_after_claimed_write(&journal, &[event]).expect_err("truncated");
        assert!(matches!(err, EmitError::ReadbackFailed { .. }));
    }

    #[test]
    fn six_layers_encode() {
        for layer in [Layer::L0, Layer::L1, Layer::L2, Layer::L3, Layer::L4, Layer::L5] {
            let line = row(layer, layer.as_str()).to_json_line();
            assert!(line.contains(&format!("\"layer\":\"{}\"", layer.as_str())));
            assert!(line.contains("reason_code"));
        }
    }

    #[test]
    fn l3_step_fields_are_on_the_same_row() {
        let event = row(Layer::L3, "HD-0009").with_step("L3-HD0009", "Blocked", "await HD-0009");
        let line = event.to_json_line();
        assert!(line.contains("\"step\":\"L3-HD0009\""));
        assert!(line.contains("\"next_command\":\"await HD-0009\""));
    }

    #[cfg(feature = "cx")]
    #[test]
    fn cx_emit_roundtrips() {
        use asupersync::runtime::RuntimeBuilder;
        let dir = tempdir().unwrap();
        let journal = DurableJournal::open(dir.path().join("lifecycle.jsonl")).unwrap();
        let event = row(Layer::L5, "PORTAL_ROW");
        RuntimeBuilder::current_thread()
            .build()
            .expect("runtime")
            .block_on(async move {
                let cx = asupersync::Cx::current().expect("cx");
                emit_one(&cx, &journal, event).await.expect("cx emit");
            });
    }
}

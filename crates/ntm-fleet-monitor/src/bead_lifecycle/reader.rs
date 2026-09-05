#![forbid(unsafe_code)]

//! Bounded, cursor-backed reads from the bead lifecycle ledger.
//!
//! The ledger is an append-only file, so this reader cannot receive a native
//! filesystem notification without adding another service. It therefore uses
//! an honest bounded poll: each scan reads the append-only suffix, advances a
//! durable line cursor, and the wait sleeps for the configured interval before
//! scanning again. A timeout is a typed outcome, never an empty read.

use super::ledger::BEAD_LIFECYCLE_LEDGER_SCHEMA;
use asupersync::Cx;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Default interval between file-backed suffix scans.
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(10);

/// Maximum wait accepted by the reader. Callers must choose a bounded wait.
pub const MAX_WAIT: Duration = Duration::from_secs(300);

/// Last consumed one-based JSONL line number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LifecycleCursor(u64);

impl LifecycleCursor {
    pub const ZERO: Self = Self(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Durable cursor file for one reader identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleCursorStore {
    path: PathBuf,
}

impl LifecycleCursorStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<LifecycleCursor, ReaderError> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(LifecycleCursor::ZERO)
            }
            Err(error) => return Err(cursor_io("read", &self.path, error)),
        };
        let raw = text.trim();
        if raw.is_empty() {
            return Err(ReaderError::InvalidCursor {
                path: self.path.clone(),
                value: raw.to_owned(),
            });
        }
        raw.parse::<u64>()
            .map(LifecycleCursor::new)
            .map_err(|_| ReaderError::InvalidCursor {
                path: self.path.clone(),
                value: raw.to_owned(),
            })
    }

    pub fn persist(&self, cursor: LifecycleCursor) -> Result<(), ReaderError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| cursor_io("create_parent", parent, error))?;
        }
        let temp_path = self.path.with_extension("tmp");
        let mut file = File::create(&temp_path)
            .map_err(|error| cursor_io("create_temp", &temp_path, error))?;
        file.write_all(cursor.value().to_string().as_bytes())
            .and_then(|_| file.write_all(b"\n"))
            .map_err(|error| cursor_io("write_temp", &temp_path, error))?;
        file.sync_all()
            .map_err(|error| cursor_io("fsync_temp", &temp_path, error))?;
        fs::rename(&temp_path, &self.path)
            .map_err(|error| cursor_io("rename", &self.path, error))?;
        fsync_parent(&self.path)
    }
}

/// One validated lifecycle row with its append-only ledger cursor.
#[derive(Debug, Clone, PartialEq)]
pub struct LifecycleRow {
    pub cursor: LifecycleCursor,
    pub bead: String,
    pub event: String,
    pub status: String,
    pub idempotency_key: String,
    pub observed_at_ms: u64,
    pub raw: Value,
}

/// Result of one nonblocking suffix scan.
#[derive(Debug, Clone, PartialEq)]
pub enum ReadOutcome {
    Rows {
        cursor: LifecycleCursor,
        rows: Vec<LifecycleRow>,
    },
    Empty {
        cursor: LifecycleCursor,
    },
}

impl ReadOutcome {
    pub const fn cursor(&self) -> LifecycleCursor {
        match self {
            Self::Rows { cursor, .. } | Self::Empty { cursor } => *cursor,
        }
    }

    pub const fn is_empty(&self) -> bool {
        matches!(self, Self::Empty { .. })
    }
}

/// Result of a bounded wait. `TimedOut` is not an empty read.
#[derive(Debug, Clone, PartialEq)]
pub enum WaitOutcome {
    Rows {
        cursor: LifecycleCursor,
        rows: Vec<LifecycleRow>,
    },
    TimedOut {
        cursor: LifecycleCursor,
        waited: Duration,
    },
}

impl WaitOutcome {
    pub const fn cursor(&self) -> LifecycleCursor {
        match self {
            Self::Rows { cursor, .. } | Self::TimedOut { cursor, .. } => *cursor,
        }
    }

    pub const fn timed_out(&self) -> bool {
        matches!(self, Self::TimedOut { .. })
    }
}

/// Errors from cursor loading, row validation, cancellation, or I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReaderError {
    EmptyBeadSet,
    InvalidPollInterval,
    WaitExceedsBound {
        requested: Duration,
        maximum: Duration,
    },
    InvalidCursor {
        path: PathBuf,
        value: String,
    },
    CursorAhead {
        cursor: LifecycleCursor,
        available: LifecycleCursor,
    },
    MalformedRow {
        line: u64,
        detail: String,
    },
    Cancelled,
    Io {
        operation: &'static str,
        path: PathBuf,
        detail: String,
    },
}

impl std::fmt::Display for ReaderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyBeadSet => formatter.write_str("LIFECYCLE_READER_EMPTY_BEAD_SET"),
            Self::InvalidPollInterval => {
                formatter.write_str("LIFECYCLE_READER_INVALID_POLL_INTERVAL")
            }
            Self::WaitExceedsBound { requested, maximum } => write!(
                formatter,
                "LIFECYCLE_READER_WAIT_EXCEEDS_BOUND requested_secs={} maximum_secs={}",
                requested.as_secs(),
                maximum.as_secs()
            ),
            Self::InvalidCursor { path, value } => write!(
                formatter,
                "LIFECYCLE_READER_CURSOR_INVALID path={} value={value:?}",
                path.display()
            ),
            Self::CursorAhead { cursor, available } => write!(
                formatter,
                "LIFECYCLE_READER_CURSOR_AHEAD cursor={} available={}",
                cursor.value(),
                available.value()
            ),
            Self::MalformedRow { line, detail } => {
                write!(
                    formatter,
                    "LIFECYCLE_READER_ROW_MALFORMED line={line} detail={detail}"
                )
            }
            Self::Cancelled => formatter.write_str("LIFECYCLE_READER_CANCELLED"),
            Self::Io {
                operation,
                path,
                detail,
            } => write!(
                formatter,
                "LIFECYCLE_READER_IO operation={operation} path={} detail={detail}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ReaderError {}

/// Append-only lifecycle reader scoped to a nonempty bead set.
#[derive(Debug, Clone)]
pub struct LifecycleReader {
    ledger_path: PathBuf,
    cursor_store: LifecycleCursorStore,
    beads: BTreeSet<String>,
    cursor: LifecycleCursor,
    poll_interval: Duration,
}

impl LifecycleReader {
    pub fn new<I, S>(
        ledger_path: impl Into<PathBuf>,
        cursor_path: impl Into<PathBuf>,
        beads: I,
    ) -> Result<Self, ReaderError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let beads = beads
            .into_iter()
            .map(Into::into)
            .filter(|bead| !bead.trim().is_empty())
            .collect::<BTreeSet<_>>();
        if beads.is_empty() {
            return Err(ReaderError::EmptyBeadSet);
        }
        let cursor_store = LifecycleCursorStore::new(cursor_path);
        let cursor = cursor_store.load()?;
        Ok(Self {
            ledger_path: ledger_path.into(),
            cursor_store,
            beads,
            cursor,
            poll_interval: DEFAULT_POLL_INTERVAL,
        })
    }

    pub fn with_poll_interval(mut self, poll_interval: Duration) -> Result<Self, ReaderError> {
        if poll_interval.is_zero() {
            return Err(ReaderError::InvalidPollInterval);
        }
        self.poll_interval = poll_interval;
        Ok(self)
    }

    pub fn ledger_path(&self) -> &Path {
        &self.ledger_path
    }

    pub fn cursor_path(&self) -> &Path {
        self.cursor_store.path()
    }

    pub const fn cursor(&self) -> LifecycleCursor {
        self.cursor
    }

    pub const fn poll_interval(&self) -> Duration {
        self.poll_interval
    }

    /// Read rows newer than the persisted cursor without waiting.
    pub fn read_available(&mut self) -> Result<ReadOutcome, ReaderError> {
        let text = match fs::read_to_string(&self.ledger_path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(ReadOutcome::Empty {
                    cursor: self.cursor,
                });
            }
            Err(error) => return Err(cursor_io("read_ledger", &self.ledger_path, error)),
        };
        let mut available = LifecycleCursor::ZERO;
        let mut rows = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let line_cursor = LifecycleCursor::new(index as u64 + 1);
            available = line_cursor;
            if line_cursor <= self.cursor {
                continue;
            }
            if line.trim().is_empty() {
                continue;
            }
            let value: Value =
                serde_json::from_str(line).map_err(|error| ReaderError::MalformedRow {
                    line: line_cursor.value(),
                    detail: error.to_string(),
                })?;
            let row = decode_row(line_cursor, value)?;
            if self.beads.contains(&row.bead) {
                rows.push(row);
            }
        }
        if available < self.cursor {
            return Err(ReaderError::CursorAhead {
                cursor: self.cursor,
                available,
            });
        }
        self.advance_cursor(available)?;
        if rows.is_empty() {
            Ok(ReadOutcome::Empty {
                cursor: self.cursor,
            })
        } else {
            Ok(ReadOutcome::Rows {
                cursor: self.cursor,
                rows,
            })
        }
    }

    /// Wait for a matching row with an explicit wall-clock bound.
    ///
    /// The file format has no native notification channel, so this is a
    /// documented 10-second default poll rather than a falsely named block.
    /// The loop is cancel-aware and never spawns a detached task.
    pub async fn wait_for_rows(
        cx: &Cx,
        reader: &mut Self,
        timeout: Duration,
    ) -> Result<WaitOutcome, ReaderError> {
        if timeout > MAX_WAIT {
            return Err(ReaderError::WaitExceedsBound {
                requested: timeout,
                maximum: MAX_WAIT,
            });
        }
        let started = Instant::now();
        let deadline = started + timeout;
        loop {
            cx.checkpoint().map_err(|_| ReaderError::Cancelled)?;
            match reader.read_available()? {
                ReadOutcome::Rows { cursor, rows } => {
                    return Ok(WaitOutcome::Rows { cursor, rows })
                }
                ReadOutcome::Empty { .. } => {}
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(WaitOutcome::TimedOut {
                    cursor: reader.cursor,
                    waited: now.saturating_duration_since(started),
                });
            }
            let delay = reader
                .poll_interval
                .min(deadline.saturating_duration_since(now));
            asupersync::time::sleep(asupersync::time::wall_now(), delay).await;
            cx.checkpoint().map_err(|_| ReaderError::Cancelled)?;
        }
    }

    fn advance_cursor(&mut self, next: LifecycleCursor) -> Result<(), ReaderError> {
        if next > self.cursor {
            self.cursor_store.persist(next)?;
            self.cursor = next;
        }
        Ok(())
    }
}

fn decode_row(cursor: LifecycleCursor, value: Value) -> Result<LifecycleRow, ReaderError> {
    let schema = required_string(&value, "schema", cursor)?;
    if schema != BEAD_LIFECYCLE_LEDGER_SCHEMA {
        return Err(malformed(cursor, format!("schema={schema:?}")));
    }
    let event = required_string(&value, "event", cursor)?;
    let status = required_string(&value, "status", cursor)?;
    let bead = required_string(&value, "bead", cursor)?;
    let idempotency_key = required_string(&value, "idempotency_key", cursor)?;
    for key in [
        "repo",
        "session",
        "pane",
        "packet_digest",
        "objective",
        "invoker",
        "evidence",
    ] {
        let _ = required_value(&value, key, cursor)?;
    }
    let observed_at_ms = value
        .get("freshness")
        .and_then(|freshness| freshness.get("observed_at_ms"))
        .and_then(Value::as_u64)
        .ok_or_else(|| malformed(cursor, "freshness.observed_at_ms missing"))?;
    Ok(LifecycleRow {
        cursor,
        bead,
        event,
        status,
        idempotency_key,
        observed_at_ms,
        raw: value,
    })
}

fn required_string(
    value: &Value,
    key: &str,
    cursor: LifecycleCursor,
) -> Result<String, ReaderError> {
    required_value(value, key, cursor)?
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| malformed(cursor, format!("{key} is not a nonempty string")))
}

fn required_value<'a>(
    value: &'a Value,
    key: &str,
    cursor: LifecycleCursor,
) -> Result<&'a Value, ReaderError> {
    value
        .get(key)
        .filter(|candidate| !candidate.is_null())
        .ok_or_else(|| malformed(cursor, format!("{key} missing")))
}

fn malformed(cursor: LifecycleCursor, detail: impl Into<String>) -> ReaderError {
    ReaderError::MalformedRow {
        line: cursor.value(),
        detail: detail.into(),
    }
}

fn cursor_io(operation: &'static str, path: &Path, error: impl ToString) -> ReaderError {
    ReaderError::Io {
        operation,
        path: path.to_path_buf(),
        detail: error.to_string(),
    }
}

fn fsync_parent(path: &Path) -> Result<(), ReaderError> {
    let parent = path.parent().ok_or_else(|| ReaderError::Io {
        operation: "parent",
        path: path.to_path_buf(),
        detail: "cursor path has no parent".to_owned(),
    })?;
    let directory = File::open(parent).map_err(|error| cursor_io("open_parent", parent, error))?;
    directory
        .sync_all()
        .map_err(|error| cursor_io("fsync_parent", parent, error))
}

#![forbid(unsafe_code)]

//! Typed, bounded transport for one OMP `--mode=rpc` process.
//!
//! The adapter owns exactly one child process, sends a fixed request sequence,
//! drains both output pipes, and closes the child before returning. It never
//! invokes a shell and never treats a missing, malformed, unknown, rejected, or
//! timed-out frame as success.
//!
//! # OMP surface
//!
//! This crate covers the OMP `--mode=rpc` single-session transport and these
//! request methods: `negotiate_protocol` v2, `get_state`, `get_session_stats`,
//! and `get_messages`. Session continuity flags are observations only; this
//! crate does not claim continuity across processes or sessions.
//!
//! # No-claim boundary
//!
//! [`NO_CLAIM_BOUNDARY`] is deliberately part of the public API. This adapter
//! cannot observe or dispatch third-party panes. It does not talk to tmux or
//! NTM, and it has no delivery receipt for another process, pane, or session.

use asupersync::io::{AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader, LineReader};
use asupersync::process::{
    Child, ChildStdin, Command, ProcessError, ProcessGroupMode, ProcessSignalTarget, Stdio,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// JSON schema version emitted by the robot-facing binary and report helpers.
pub const OMP_RPC_SCHEMA_VERSION: u32 = 1;

/// The exact OMP surface implemented by this crate.
pub const OMP_SURFACE: &str = "--mode=rpc single-session transport";

/// What this crate explicitly does not prove or operate.
pub const NO_CLAIM_BOUNDARY: &str = "This adapter drives one configured OMP --mode=rpc child only. It cannot observe or dispatch third-party panes, tmux sessions, NTM packets, or cross-pane delivery receipts; session continuity flags are context observations, not continuity proof.";

const DEFAULT_MAX_FRAME_BYTES: usize = 1_048_576;
const DEFAULT_MAX_CAPTURE_BYTES: usize = 4 * 1_048_576;
const DEFAULT_BINARY: &str = "omp";
const REQUEST_IDS: [&str; 4] = ["negotiate-2", "state", "stats", "messages"];

/// A bounded phase timeout. No phase, including shutdown, is unbounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadlines {
    pub startup: Duration,
    pub request: Duration,
    pub shutdown: Duration,
}

impl Default for Deadlines {
    fn default() -> Self {
        Self {
            startup: Duration::from_secs(15),
            request: Duration::from_secs(15),
            shutdown: Duration::from_secs(5),
        }
    }
}

/// Which bounded phase elapsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutPhase {
    Startup,
    Request,
    Shutdown,
}

impl TimeoutPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::Request => "request",
            Self::Shutdown => "shutdown",
        }
    }
}

/// A direct executable invocation. The program is passed to
/// `asupersync::process::Command` without a shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OmpCommand {
    binary: PathBuf,
    args: Vec<OsString>,
    current_dir: Option<PathBuf>,
    environment: BTreeMap<OsString, OsString>,
}

impl OmpCommand {
    /// Build an invocation with the required single-session mode argument.
    pub fn new(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
            args: vec![OsString::from("--mode=rpc")],
            current_dir: None,
            environment: BTreeMap::new(),
        }
    }

    /// The conventional local OMP path used by the quick robot command.
    pub fn default_binary() -> Self {
        Self::new(DEFAULT_BINARY)
    }

    pub fn binary(&self) -> &Path {
        &self.binary
    }

    pub fn args(&self) -> &[OsString] {
        &self.args
    }

    /// Add one literal argument. It is never parsed as shell syntax.
    pub fn arg(mut self, value: impl AsRef<OsStr>) -> Self {
        self.args.push(value.as_ref().to_os_string());
        self
    }

    pub fn current_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.current_dir = Some(path.into());
        self
    }

    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.environment
            .insert(key.as_ref().to_os_string(), value.as_ref().to_os_string());
        self
    }

    fn process_command(&self, max_capture_bytes: usize) -> Command {
        let mut command = Command::new("omp");
        command.args(&self.args);
        if let Some(path) = &self.current_dir {
            command.current_dir(path);
        }
        for (key, value) in &self.environment {
            command.env(key, value);
        }
        if let Some(parent) = self
            .binary
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
        {
            let mut search_path = vec![parent.to_path_buf()];
            if let Some(existing) = std::env::var_os("PATH") {
                search_path.extend(std::env::split_paths(&existing));
            }
            if let Ok(joined) = std::env::join_paths(search_path) {
                command.env("PATH", joined);
            }
        }
        command
            .stdin(Stdio::Pipe)
            .stdout(Stdio::Pipe)
            .stderr(Stdio::Pipe)
            .process_group_mode(ProcessGroupMode::NewProcessGroup)
            .signal_target(ProcessSignalTarget::ProcessGroup)
            .kill_on_drop(true);
        let _ = max_capture_bytes;
        command
    }
}

/// Configuration for one bounded session run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcSessionConfig {
    pub command: OmpCommand,
    pub deadlines: Deadlines,
    pub max_frame_bytes: usize,
    pub max_capture_bytes: usize,
}

impl RpcSessionConfig {
    pub fn new(binary: impl Into<PathBuf>) -> Self {
        Self {
            command: OmpCommand::new(binary),
            deadlines: Deadlines::default(),
            max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
            max_capture_bytes: DEFAULT_MAX_CAPTURE_BYTES,
        }
    }

    pub fn with_command(command: OmpCommand) -> Self {
        Self {
            command,
            deadlines: Deadlines::default(),
            max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
            max_capture_bytes: DEFAULT_MAX_CAPTURE_BYTES,
        }
    }

    pub fn deadlines(mut self, deadlines: Deadlines) -> Self {
        self.deadlines = deadlines;
        self
    }

    pub fn max_frame_bytes(mut self, limit: usize) -> Self {
        self.max_frame_bytes = limit.max(1);
        self
    }

    pub fn max_capture_bytes(mut self, limit: usize) -> Self {
        self.max_capture_bytes = limit.max(1);
        self
    }
}

/// Protocol version advertised or negotiated by OMP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProtocolVersion(pub u32);

impl ProtocolVersion {
    pub const V2: Self = Self(2);
}

/// Correlation identifier for one fixed request.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(String);

impl RequestId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The closed request vocabulary. There is intentionally no raw request arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcRequest {
    NegotiateProtocol,
    GetState,
    GetSessionStats,
    GetMessages,
}

impl RpcRequest {
    pub const fn id(self) -> &'static str {
        match self {
            Self::NegotiateProtocol => "negotiate-2",
            Self::GetState => "state",
            Self::GetSessionStats => "stats",
            Self::GetMessages => "messages",
        }
    }

    pub const fn command(self) -> &'static str {
        match self {
            Self::NegotiateProtocol => "negotiate_protocol",
            Self::GetState => "get_state",
            Self::GetSessionStats => "get_session_stats",
            Self::GetMessages => "get_messages",
        }
    }

    /// The one-shot request order sent after a valid `ready` frame.
    pub const fn sequence() -> [Self; 4] {
        [
            Self::NegotiateProtocol,
            Self::GetState,
            Self::GetSessionStats,
            Self::GetMessages,
        ]
    }

    pub fn to_frame(self) -> String {
        let frame = match self {
            Self::NegotiateProtocol => json!({
                "id": self.id(),
                "type": self.command(),
                "protocolVersion": 2
            }),
            Self::GetState | Self::GetSessionStats | Self::GetMessages => json!({
                "id": self.id(),
                "type": self.command()
            }),
        };
        format!("{frame}\n")
    }
}

/// Known response command, with an explicit unknown arm for forward protocol changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcCommand {
    NegotiateProtocol,
    GetState,
    GetSessionStats,
    GetMessages,
    Unknown(String),
}

impl RpcCommand {
    pub fn parse(value: &str) -> Self {
        match value {
            "negotiate_protocol" => Self::NegotiateProtocol,
            "get_state" => Self::GetState,
            "get_session_stats" => Self::GetSessionStats,
            "get_messages" => Self::GetMessages,
            other => Self::Unknown(other.to_owned()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::NegotiateProtocol => "negotiate_protocol",
            Self::GetState => "get_state",
            Self::GetSessionStats => "get_session_stats",
            Self::GetMessages => "get_messages",
            Self::Unknown(value) => value,
        }
    }
}

/// The OMP ready frame. A missing or malformed versions field is not a ready frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadyFrame {
    pub protocol_version: Option<ProtocolVersion>,
    pub supported_protocol_versions: Vec<ProtocolVersion>,
}

/// A syntactically valid response, including unsuccessful/error responses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseFrame {
    pub id: Option<RequestId>,
    pub command: RpcCommand,
    pub success: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
    pub raw: String,
}

impl ResponseFrame {
    pub fn is_error(&self) -> bool {
        !self.success
    }
}

/// A frame type not recognized by this adapter. Its raw JSON is retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownFrame {
    pub frame_type: String,
    pub raw: String,
}

/// Why a line could not become a typed known frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MalformedReason {
    EmptyLine,
    InvalidJson(String),
    JsonNotObject,
    MissingType,
    TypeNotString,
    ReadyVersionsMissing,
    ReadyVersionNotInteger,
    ResponseIdNotString,
    ResponseCommandMissing,
    ResponseCommandNotString,
    ResponseSuccessMissing,
    ResponseSuccessNotBoolean,
    ResponseErrorNotString,
    FrameTooLarge { bytes: usize, limit: usize },
}

impl fmt::Display for MalformedReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLine => formatter.write_str("empty line"),
            Self::InvalidJson(error) => write!(formatter, "invalid JSON: {error}"),
            Self::JsonNotObject => formatter.write_str("JSON frame is not an object"),
            Self::MissingType => formatter.write_str("frame has no type"),
            Self::TypeNotString => formatter.write_str("frame type is not a string"),
            Self::ReadyVersionsMissing => {
                formatter.write_str("ready.supportedProtocolVersions is missing or not an array")
            }
            Self::ReadyVersionNotInteger => {
                formatter.write_str("ready protocol version is not an unsigned integer")
            }
            Self::ResponseIdNotString => formatter.write_str("response id is not a string"),
            Self::ResponseCommandMissing => formatter.write_str("response command is missing"),
            Self::ResponseCommandNotString => {
                formatter.write_str("response command is not a string")
            }
            Self::ResponseSuccessMissing => formatter.write_str("response success is missing"),
            Self::ResponseSuccessNotBoolean => {
                formatter.write_str("response success is not a boolean")
            }
            Self::ResponseErrorNotString => formatter.write_str("response error is not a string"),
            Self::FrameTooLarge { bytes, limit } => {
                write!(formatter, "frame is {bytes} bytes, limit is {limit}")
            }
        }
    }
}

/// A malformed line, retained instead of discarded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MalformedFrame {
    pub line_no: usize,
    pub reason: MalformedReason,
    pub raw: String,
}

/// Every line observed on stdout has one explicit representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcFrame {
    Ready(ReadyFrame),
    Response(ResponseFrame),
    Unknown(UnknownFrame),
    Malformed(MalformedFrame),
}

impl RpcFrame {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Ready(_) => "ready",
            Self::Response(_) => "response",
            Self::Unknown(_) => "unknown",
            Self::Malformed(_) => "malformed",
        }
    }
}

/// Parse exactly one newline-delimited stdout frame.
#[must_use]
pub fn parse_frame(line_no: usize, line: &str) -> RpcFrame {
    let raw = line.trim_end_matches(&['\r', '\n'][..]).to_owned();
    if raw.trim().is_empty() {
        return RpcFrame::Malformed(MalformedFrame {
            line_no,
            reason: MalformedReason::EmptyLine,
            raw,
        });
    }
    let value = match serde_json::from_str::<Value>(&raw) {
        Ok(value) => value,
        Err(error) => {
            return RpcFrame::Malformed(MalformedFrame {
                line_no,
                reason: MalformedReason::InvalidJson(error.to_string()),
                raw,
            });
        }
    };
    let Some(object) = value.as_object() else {
        return RpcFrame::Malformed(MalformedFrame {
            line_no,
            reason: MalformedReason::JsonNotObject,
            raw,
        });
    };
    let Some(type_value) = object.get("type") else {
        return RpcFrame::Malformed(MalformedFrame {
            line_no,
            reason: MalformedReason::MissingType,
            raw,
        });
    };
    let Some(frame_type) = type_value.as_str() else {
        return RpcFrame::Malformed(MalformedFrame {
            line_no,
            reason: MalformedReason::TypeNotString,
            raw,
        });
    };

    match frame_type {
        "ready" => parse_ready(line_no, raw, object),
        "response" => parse_response(line_no, raw, object),
        other => RpcFrame::Unknown(UnknownFrame {
            frame_type: other.to_owned(),
            raw,
        }),
    }
}

fn parse_ready(line_no: usize, raw: String, object: &serde_json::Map<String, Value>) -> RpcFrame {
    let Some(values) = object
        .get("supportedProtocolVersions")
        .and_then(Value::as_array)
    else {
        return RpcFrame::Malformed(MalformedFrame {
            line_no,
            reason: MalformedReason::ReadyVersionsMissing,
            raw,
        });
    };
    let mut supported = Vec::with_capacity(values.len());
    for value in values {
        let Some(version) = value
            .as_u64()
            .and_then(|version| u32::try_from(version).ok())
        else {
            return RpcFrame::Malformed(MalformedFrame {
                line_no,
                reason: MalformedReason::ReadyVersionNotInteger,
                raw,
            });
        };
        supported.push(ProtocolVersion(version));
    }
    let protocol_version = object
        .get("protocolVersion")
        .and_then(Value::as_u64)
        .and_then(|version| u32::try_from(version).ok())
        .map(ProtocolVersion);
    RpcFrame::Ready(ReadyFrame {
        protocol_version,
        supported_protocol_versions: supported,
    })
}

fn parse_response(
    line_no: usize,
    raw: String,
    object: &serde_json::Map<String, Value>,
) -> RpcFrame {
    let id = match object.get("id") {
        None => None,
        Some(value) => match value.as_str() {
            Some(value) => Some(RequestId::new(value)),
            None => {
                return RpcFrame::Malformed(MalformedFrame {
                    line_no,
                    reason: MalformedReason::ResponseIdNotString,
                    raw,
                });
            }
        },
    };
    let Some(command_value) = object.get("command") else {
        return RpcFrame::Malformed(MalformedFrame {
            line_no,
            reason: MalformedReason::ResponseCommandMissing,
            raw,
        });
    };
    let Some(command) = command_value.as_str() else {
        return RpcFrame::Malformed(MalformedFrame {
            line_no,
            reason: MalformedReason::ResponseCommandNotString,
            raw,
        });
    };
    let Some(success_value) = object.get("success") else {
        return RpcFrame::Malformed(MalformedFrame {
            line_no,
            reason: MalformedReason::ResponseSuccessMissing,
            raw,
        });
    };
    let Some(success) = success_value.as_bool() else {
        return RpcFrame::Malformed(MalformedFrame {
            line_no,
            reason: MalformedReason::ResponseSuccessNotBoolean,
            raw,
        });
    };
    let error = match object.get("error") {
        None => None,
        Some(value) => match value.as_str() {
            Some(value) => Some(value.to_owned()),
            None => {
                return RpcFrame::Malformed(MalformedFrame {
                    line_no,
                    reason: MalformedReason::ResponseErrorNotString,
                    raw,
                });
            }
        },
    };
    RpcFrame::Response(ResponseFrame {
        id,
        command: RpcCommand::parse(command),
        success,
        data: object.get("data").cloned(),
        error,
        raw,
    })
}

/// The selected successful response values. The raw `data` JSON remains intact
/// because OMP may extend each object without changing this transport contract.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SelectedResponses {
    pub negotiate_protocol: Option<Value>,
    pub state: Option<Value>,
    pub session_stats: Option<Value>,
    pub messages: Option<Value>,
}

impl SelectedResponses {
    fn record(&mut self, response: &ResponseFrame) {
        match &response.command {
            RpcCommand::NegotiateProtocol => self.negotiate_protocol = response.data.clone(),
            RpcCommand::GetState => self.state = response.data.clone(),
            RpcCommand::GetSessionStats => self.session_stats = response.data.clone(),
            RpcCommand::GetMessages => self.messages = response.data.clone(),
            RpcCommand::Unknown(_) => {}
        }
    }
}

/// Captured bytes from one child stream, bounded in retained memory while the
/// reader continues to drain to EOF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedOutput {
    pub bytes: Vec<u8>,
    pub total_bytes: usize,
    pub truncated: bool,
}

/// A successful one-shot report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcSessionReport {
    pub schema: u32,
    pub lifecycle: Lifecycle,
    pub ready: ReadyFrame,
    pub negotiated: ProtocolVersion,
    pub frames: Vec<RpcFrame>,
    pub responses: Vec<ResponseFrame>,
    pub selected: SelectedResponses,
    pub stderr: CapturedOutput,
    pub exit_code: Option<i32>,
}

impl RpcSessionReport {
    pub fn ok(&self) -> bool {
        self.lifecycle == Lifecycle::Stopped
            && self.negotiated == ProtocolVersion::V2
            && self.exit_code == Some(0)
            && self.responses.len() == REQUEST_IDS.len()
            && self.responses.iter().all(|response| response.success)
    }

    /// Versioned robot-facing JSON without relying on serde implementation details.
    pub fn to_json(&self) -> Value {
        json!({
            "schema": self.schema,
            "surface": OMP_SURFACE,
            "lifecycle": self.lifecycle.as_str(),
            "ok": self.ok(),
            "ready": {
                "protocolVersion": self.ready.protocol_version.map(|v| v.0),
                "supportedProtocolVersions": self.ready.supported_protocol_versions.iter().map(|v| v.0).collect::<Vec<_>>()
            },
            "negotiated": self.negotiated.0,
            "responseCount": self.responses.len(),
            "frameCount": self.frames.len(),
            "unknownFrames": self.frames.iter().filter(|frame| matches!(frame, RpcFrame::Unknown(_))).count(),
            "malformedFrames": self.frames.iter().filter(|frame| matches!(frame, RpcFrame::Malformed(_))).count(),
            "stderrBytes": self.stderr.total_bytes,
            "stderrTruncated": self.stderr.truncated,
            "exitCode": self.exit_code,
            "noClaim": NO_CLAIM_BOUNDARY
        })
    }
}

/// Lifecycle states are intentionally distinct from the process exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    Spawned,
    Ready,
    Negotiated,
    Active,
    Stopping,
    Stopped,
    Failed,
    TimedOut,
}

impl Lifecycle {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Spawned => "spawned",
            Self::Ready => "ready",
            Self::Negotiated => "negotiated",
            Self::Active => "active",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
            Self::TimedOut => "timed-out",
        }
    }
}

/// Protocol and framing failures are typed and fail closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    UnsupportedProtocol {
        required: ProtocolVersion,
        advertised: Vec<ProtocolVersion>,
    },
    DuplicateReady,
    Malformed(MalformedFrame),
    UnexpectedFrame(RpcFrame),
    UnexpectedResponseId(String),
    ResponseCommandMismatch {
        id: String,
        expected: String,
        received: String,
    },
    ResponseRejected {
        id: String,
        command: String,
        error: Option<String>,
    },
    MissingData {
        id: String,
        command: String,
    },
    MissingResponses(Vec<String>),
    FrameTooLarge {
        bytes: usize,
        limit: usize,
    },
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProtocol {
                required,
                advertised,
            } => {
                write!(
                    formatter,
                    "required protocol v{}, advertised {advertised:?}",
                    required.0
                )
            }
            Self::DuplicateReady => formatter.write_str("duplicate ready frame"),
            Self::Malformed(frame) => write!(
                formatter,
                "malformed frame at line {}: {}",
                frame.line_no, frame.reason
            ),
            Self::UnexpectedFrame(frame) => write!(formatter, "unexpected {} frame", frame.kind()),
            Self::UnexpectedResponseId(id) => write!(formatter, "unexpected response id {id}"),
            Self::ResponseCommandMismatch {
                id,
                expected,
                received,
            } => write!(
                formatter,
                "response {id} expected {expected}, received {received}"
            ),
            Self::ResponseRejected { id, command, error } => {
                write!(formatter, "response {id} ({command}) rejected: {error:?}")
            }
            Self::MissingData { id, command } => {
                write!(formatter, "response {id} ({command}) did not include data")
            }
            Self::MissingResponses(ids) => write!(formatter, "missing responses: {ids:?}"),
            Self::FrameTooLarge { bytes, limit } => {
                write!(formatter, "frame is {bytes} bytes, limit is {limit}")
            }
        }
    }
}

/// All transport outcomes, including process, cancellation, timeout, and protocol errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcError {
    Process {
        operation: String,
        detail: String,
    },
    ProcessExited {
        code: Option<i32>,
    },
    Cancelled {
        detail: String,
    },
    Timeout {
        phase: TimeoutPhase,
    },
    Protocol(ProtocolError),
    Io {
        stream: &'static str,
        detail: String,
    },
    Cleanup {
        primary: Box<RpcError>,
        detail: String,
    },
}

impl fmt::Display for RpcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Process { operation, detail } => {
                write!(formatter, "process {operation}: {detail}")
            }
            Self::ProcessExited { code } => {
                write!(formatter, "omp exited unsuccessfully: {code:?}")
            }
            Self::Cancelled { detail } => write!(formatter, "cancelled: {detail}"),
            Self::Timeout { phase } => write!(formatter, "timeout during {}", phase.as_str()),
            Self::Protocol(error) => write!(formatter, "protocol error: {error}"),
            Self::Io { stream, detail } => write!(formatter, "{stream} I/O: {detail}"),
            Self::Cleanup { primary, detail } => write!(formatter, "{primary}; cleanup: {detail}"),
        }
    }
}

impl std::error::Error for RpcError {}

fn process_error(operation: &'static str, error: ProcessError) -> RpcError {
    RpcError::Process {
        operation: operation.to_owned(),
        detail: error.to_string(),
    }
}

fn checkpoint(cx: &asupersync::Cx) -> Result<(), RpcError> {
    cx.checkpoint().map_err(|_| RpcError::Cancelled {
        detail: format!("context cancellation: {:?}", cx.cancel_reason()),
    })
}

fn map_io(stream: &'static str, error: std::io::Error) -> RpcError {
    RpcError::Io {
        stream,
        detail: error.to_string(),
    }
}

fn validate_binary(binary: &Path) -> Result<(), RpcError> {
    let valid_name = binary
        .file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name == "omp");
    if valid_name {
        Ok(())
    } else {
        Err(RpcError::Process {
            operation: "validate executable".to_owned(),
            detail: format!(
                "only an executable named omp is allowed: {}",
                binary.display()
            ),
        })
    }
}

/// Run one bounded OMP RPC session. The caller's `&Cx` owns cancellation.
pub async fn run_session(
    cx: &asupersync::Cx,
    config: &RpcSessionConfig,
) -> Result<RpcSessionReport, RpcError> {
    validate_binary(config.command.binary())?;
    checkpoint(cx)?;
    let mut child = config
        .command
        .process_command(config.max_capture_bytes)
        .spawn()
        .map_err(|error| process_error("spawn", error))?;
    let stdin = child.stdin().ok_or_else(|| RpcError::Process {
        operation: "spawn".to_owned(),
        detail: "configured stdin pipe was unavailable".to_owned(),
    })?;
    let stdout = child.stdout().ok_or_else(|| RpcError::Process {
        operation: "spawn".to_owned(),
        detail: "configured stdout pipe was unavailable".to_owned(),
    })?;
    let stderr = child.stderr().ok_or_else(|| RpcError::Process {
        operation: "spawn".to_owned(),
        detail: "configured stderr pipe was unavailable".to_owned(),
    })?;
    let reader = LineReader::new(BufReader::with_capacity(
        config.max_frame_bytes.min(8192).max(1),
        stdout,
    ));

    let protocol_future = drive_protocol(cx, &mut child, reader, stdin, config);
    let stderr_future = drain_bounded(cx, stderr, config.max_capture_bytes);
    let (protocol_result, stderr_result) = asupersync::join!(protocol_future, stderr_future);
    let stderr = stderr_result.map_err(|error| map_io("stderr", error))?;
    let mut report = protocol_result?;
    report.stderr = stderr;
    if report.exit_code != Some(0) {
        return Err(RpcError::ProcessExited {
            code: report.exit_code,
        });
    }
    checkpoint(cx)?;
    Ok(report)
}

async fn drive_protocol(
    cx: &asupersync::Cx,
    child: &mut Child,
    mut reader: LineReader<BufReader<asupersync::process::ChildStdout>>,
    mut stdin: ChildStdin,
    config: &RpcSessionConfig,
) -> Result<RpcSessionReport, RpcError> {
    let startup = asupersync::time::timeout(
        asupersync::time::wall_now(),
        config.deadlines.startup,
        await_ready(cx, &mut reader, config.max_frame_bytes),
    )
    .await;
    let (ready, mut frames) = match startup {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => {
            let cleanup = cleanup_child(cx, child, reader, stdin, config).await;
            return Err(with_cleanup(error, cleanup));
        }
        Err(_) => {
            let cleanup = cleanup_child(cx, child, reader, stdin, config).await;
            return Err(with_cleanup(
                RpcError::Timeout {
                    phase: TimeoutPhase::Startup,
                },
                cleanup,
            ));
        }
    };
    if !ready
        .supported_protocol_versions
        .contains(&ProtocolVersion::V2)
    {
        let error = RpcError::Protocol(ProtocolError::UnsupportedProtocol {
            required: ProtocolVersion::V2,
            advertised: ready.supported_protocol_versions.clone(),
        });
        let cleanup = cleanup_child(cx, child, reader, stdin, config).await;
        return Err(with_cleanup(error, cleanup));
    }
    frames.push(RpcFrame::Ready(ready.clone()));

    let request_phase = asupersync::time::timeout(
        asupersync::time::wall_now(),
        config.deadlines.request,
        exchange_requests(cx, &mut reader, &mut stdin, frames, config.max_frame_bytes),
    )
    .await;
    let (mut frames, responses, selected) = match request_phase {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => {
            let cleanup = cleanup_child(cx, child, reader, stdin, config).await;
            return Err(with_cleanup(error, cleanup));
        }
        Err(_) => {
            let cleanup = cleanup_child(cx, child, reader, stdin, config).await;
            return Err(with_cleanup(
                RpcError::Timeout {
                    phase: TimeoutPhase::Request,
                },
                cleanup,
            ));
        }
    };
    if responses.len() != REQUEST_IDS.len() {
        let outstanding = REQUEST_IDS
            .iter()
            .filter(|id| {
                !responses.iter().any(|response| {
                    response
                        .id
                        .as_ref()
                        .is_some_and(|actual| actual.as_str() == **id)
                })
            })
            .map(|id| (*id).to_owned())
            .collect();
        let error = RpcError::Protocol(ProtocolError::MissingResponses(outstanding));
        let cleanup = cleanup_child(cx, child, reader, stdin, config).await;
        return Err(with_cleanup(error, cleanup));
    }

    drop(stdin);
    let (mut buffered, _) = reader.into_parts();
    let tail = match asupersync::time::timeout(
        asupersync::time::wall_now(),
        config.deadlines.shutdown,
        drain_bounded(cx, &mut buffered, config.max_capture_bytes),
    )
    .await
    {
        Ok(Ok(tail)) => tail,
        Ok(Err(error)) => return Err(map_io("stdout", error)),
        Err(_) => {
            let _ = child.kill();
            let _ = asupersync::time::timeout(
                asupersync::time::wall_now(),
                config.deadlines.shutdown,
                child.wait_async(cx),
            )
            .await;
            return Err(RpcError::Timeout {
                phase: TimeoutPhase::Shutdown,
            });
        }
    };
    let status = match asupersync::time::timeout(
        asupersync::time::wall_now(),
        config.deadlines.shutdown,
        child.wait_async(cx),
    )
    .await
    {
        Ok(Ok(status)) => status,
        Ok(Err(error)) => return Err(process_error("wait", error)),
        Err(_) => {
            let _ = child.kill();
            let _ = asupersync::time::timeout(
                asupersync::time::wall_now(),
                config.deadlines.shutdown,
                child.wait_async(cx),
            )
            .await;
            return Err(RpcError::Timeout {
                phase: TimeoutPhase::Shutdown,
            });
        }
    };
    if tail.truncated {
        return Err(RpcError::Protocol(ProtocolError::FrameTooLarge {
            bytes: tail.total_bytes,
            limit: config.max_capture_bytes,
        }));
    }
    append_tail_frames(&mut frames, &tail.bytes);
    if frames
        .iter()
        .any(|frame| matches!(frame, RpcFrame::Malformed(_)))
    {
        let malformed = frames
            .iter()
            .find_map(|frame| match frame {
                RpcFrame::Malformed(frame) => Some(frame.clone()),
                _ => None,
            })
            .expect("malformed frame exists");
        return Err(RpcError::Protocol(ProtocolError::Malformed(malformed)));
    }
    let lifecycle = Lifecycle::Stopped;
    let negotiated = ProtocolVersion::V2;
    Ok(RpcSessionReport {
        schema: OMP_RPC_SCHEMA_VERSION,
        lifecycle,
        ready,
        negotiated,
        frames,
        responses,
        selected,
        stderr: CapturedOutput {
            bytes: Vec::new(),
            total_bytes: 0,
            truncated: false,
        },
        exit_code: Some(status.code().unwrap_or(-1)),
    })
}

async fn await_ready(
    cx: &asupersync::Cx,
    reader: &mut LineReader<BufReader<asupersync::process::ChildStdout>>,
    max_frame_bytes: usize,
) -> Result<(ReadyFrame, Vec<RpcFrame>), RpcError> {
    let mut frames = Vec::new();
    let mut line_no = 0;
    let mut ready = None;
    let mut startup_complete = false;
    loop {
        checkpoint(cx)?;
        let mut line = String::new();
        let bytes = reader
            .read_line(&mut line)
            .await
            .map_err(|error| map_io("stdout", error))?;
        if bytes == 0 {
            return Err(RpcError::Protocol(ProtocolError::MissingResponses(vec![
                "ready".to_owned(),
            ])));
        }
        line_no += 1;
        if bytes > max_frame_bytes {
            return Err(RpcError::Protocol(ProtocolError::FrameTooLarge {
                bytes,
                limit: max_frame_bytes,
            }));
        }
        match parse_frame(line_no, &line) {
            RpcFrame::Ready(candidate) => {
                if ready.is_some() {
                    return Err(RpcError::Protocol(ProtocolError::DuplicateReady));
                }
                ready = Some(candidate);
            }
            RpcFrame::Unknown(unknown) => {
                startup_complete |= unknown.frame_type == "available_commands_update";
                frames.push(RpcFrame::Unknown(unknown));
            }
            RpcFrame::Response(response) => frames.push(RpcFrame::Response(response)),
            RpcFrame::Malformed(malformed) => {
                return Err(RpcError::Protocol(ProtocolError::Malformed(malformed)));
            }
        }
        if startup_complete {
            if let Some(ready) = ready.take() {
                return Ok((ready, frames));
            }
        }
    }
}

async fn exchange_requests(
    cx: &asupersync::Cx,
    reader: &mut LineReader<BufReader<asupersync::process::ChildStdout>>,
    stdin: &mut ChildStdin,
    mut frames: Vec<RpcFrame>,
    max_frame_bytes: usize,
) -> Result<(Vec<RpcFrame>, Vec<ResponseFrame>, SelectedResponses), RpcError> {
    let sequence = RpcRequest::sequence();
    let mut pending = sequence
        .iter()
        .map(|request| request.id().to_owned())
        .collect::<Vec<_>>();
    let mut responses = Vec::with_capacity(sequence.len());
    let mut selected = SelectedResponses::default();
    let mut line_no = frames.len();

    for request in sequence {
        checkpoint(cx)?;
        stdin
            .write_all(request.to_frame().as_bytes())
            .await
            .map_err(|error| map_io("stdin", error))?;
        stdin
            .flush()
            .await
            .map_err(|error| map_io("stdin", error))?;

        loop {
            checkpoint(cx)?;
            let mut line = String::new();
            let bytes = reader
                .read_line(&mut line)
                .await
                .map_err(|error| map_io("stdout", error))?;
            if bytes == 0 {
                return Err(RpcError::Protocol(ProtocolError::MissingResponses(pending)));
            }
            line_no += 1;
            if bytes > max_frame_bytes {
                return Err(RpcError::Protocol(ProtocolError::FrameTooLarge {
                    bytes,
                    limit: max_frame_bytes,
                }));
            }
            match parse_frame(line_no, &line) {
                RpcFrame::Ready(_) => {
                    return Err(RpcError::Protocol(ProtocolError::DuplicateReady));
                }
                RpcFrame::Malformed(malformed) => {
                    return Err(RpcError::Protocol(ProtocolError::Malformed(malformed)));
                }
                RpcFrame::Unknown(unknown) => frames.push(RpcFrame::Unknown(unknown)),
                RpcFrame::Response(response) => {
                    let Some(id) = response.id.as_ref() else {
                        return Err(RpcError::Protocol(ProtocolError::UnexpectedResponseId(
                            "<missing>".to_owned(),
                        )));
                    };
                    let Some(position) = pending.iter().position(|pending| pending == id.as_str())
                    else {
                        return Err(RpcError::Protocol(ProtocolError::UnexpectedResponseId(
                            id.as_str().to_owned(),
                        )));
                    };
                    let expected = sequence
                        .iter()
                        .copied()
                        .find(|candidate| candidate.id() == id.as_str())
                        .ok_or_else(|| {
                            RpcError::Protocol(ProtocolError::UnexpectedResponseId(
                                id.as_str().to_owned(),
                            ))
                        })?;
                    if response.command.as_str() != expected.command() {
                        return Err(RpcError::Protocol(ProtocolError::ResponseCommandMismatch {
                            id: id.as_str().to_owned(),
                            expected: expected.command().to_owned(),
                            received: response.command.as_str().to_owned(),
                        }));
                    }
                    if !response.success {
                        return Err(RpcError::Protocol(ProtocolError::ResponseRejected {
                            id: id.as_str().to_owned(),
                            command: response.command.as_str().to_owned(),
                            error: response.error.clone(),
                        }));
                    }
                    if !matches!(expected, RpcRequest::NegotiateProtocol) && response.data.is_none()
                    {
                        return Err(RpcError::Protocol(ProtocolError::MissingData {
                            id: id.as_str().to_owned(),
                            command: response.command.as_str().to_owned(),
                        }));
                    }
                    let is_current = id.as_str() == request.id();
                    pending.remove(position);
                    selected.record(&response);
                    responses.push(response.clone());
                    frames.push(RpcFrame::Response(response));
                    if is_current {
                        break;
                    }
                }
            }
        }
    }
    Ok((frames, responses, selected))
}

fn append_tail_frames(frames: &mut Vec<RpcFrame>, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    let mut line_no = frames.len();
    for line in String::from_utf8_lossy(bytes).split_inclusive('\n') {
        line_no += 1;
        frames.push(parse_frame(line_no, line));
    }
}

fn with_cleanup(primary: RpcError, cleanup: Result<(), RpcError>) -> RpcError {
    match cleanup {
        Ok(()) => primary,
        Err(error) => RpcError::Cleanup {
            primary: Box::new(primary),
            detail: error.to_string(),
        },
    }
}

async fn cleanup_child(
    cx: &asupersync::Cx,
    child: &mut Child,
    reader: LineReader<BufReader<asupersync::process::ChildStdout>>,
    stdin: ChildStdin,
    config: &RpcSessionConfig,
) -> Result<(), RpcError> {
    drop(stdin);
    let _ = child.kill();
    let (mut buffered, _) = reader.into_parts();
    let drain = asupersync::time::timeout(
        asupersync::time::wall_now(),
        config.deadlines.shutdown,
        drain_bounded(cx, &mut buffered, config.max_capture_bytes),
    )
    .await;
    match drain {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => return Err(map_io("stdout", error)),
        Err(_) => {
            let _ = child.kill();
            let _ = asupersync::time::timeout(
                asupersync::time::wall_now(),
                config.deadlines.shutdown,
                child.wait_async(cx),
            )
            .await;
            return Err(RpcError::Timeout {
                phase: TimeoutPhase::Shutdown,
            });
        }
    }
    match asupersync::time::timeout(
        asupersync::time::wall_now(),
        config.deadlines.shutdown,
        child.wait_async(cx),
    )
    .await
    {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(error)) => Err(process_error("cleanup wait", error)),
        Err(_) => {
            let _ = child.kill();
            Err(RpcError::Timeout {
                phase: TimeoutPhase::Shutdown,
            })
        }
    }
}

async fn drain_bounded<R>(
    cx: &asupersync::Cx,
    mut reader: R,
    limit: usize,
) -> std::io::Result<CapturedOutput>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = Vec::with_capacity(limit.min(8192));
    let mut total_bytes = 0usize;
    let mut truncated = false;
    let mut chunk = [0u8; 8192];
    loop {
        cx.checkpoint().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::Interrupted, "context cancellation")
        })?;
        let read = reader.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        total_bytes = total_bytes.saturating_add(read);
        if bytes.len() < limit {
            let retain = (limit - bytes.len()).min(read);
            bytes.extend_from_slice(&chunk[..retain]);
            truncated |= retain < read;
        } else {
            truncated = true;
        }
    }
    Ok(CapturedOutput {
        bytes,
        total_bytes,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_sequence_is_closed_and_bounded() {
        let sequence = RpcRequest::sequence();
        assert_eq!(sequence.len(), 4);
        assert_eq!(sequence[0].command(), "negotiate_protocol");
        assert_eq!(sequence[3].command(), "get_messages");
        assert!(
            sequence
                .iter()
                .all(|request| request.to_frame().len() < 256)
        );
    }

    #[test]
    fn unknown_and_malformed_frames_are_retained_as_types() {
        assert!(matches!(
            parse_frame(1, r#"{"type":"future_frame"}"#),
            RpcFrame::Unknown(_)
        ));
        assert!(matches!(
            parse_frame(2, "not-json"),
            RpcFrame::Malformed(MalformedFrame {
                reason: MalformedReason::InvalidJson(_),
                ..
            })
        ));
    }
    #[test]
    fn executable_name_is_allowlisted() {
        assert!(validate_binary(Path::new("omp")).is_ok());
        assert!(validate_binary(Path::new("/tmp/fake-omp")).is_err());
    }
}

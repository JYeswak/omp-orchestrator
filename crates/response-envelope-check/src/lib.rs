#![forbid(unsafe_code)]

//! Fail-closed validation for the response from "ntm --robot-send".
//!
//! A zero exit code is not delivery proof. The response must be an object with the
//! four per-target fields, and those fields must describe exactly the request that
//! was sent. An empty request is the only healthy no-op; a non-empty, validated
//! request is a typed nonzero observation.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::fmt;
use std::process::ExitStatus;

/// The target set supplied to one ntm send invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NtmSendRequest {
    pub targets: Vec<String>,
}

impl NtmSendRequest {
    pub fn new(targets: Vec<String>) -> Self {
        Self { targets }
    }

    pub fn from_targets<I, S>(targets: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            targets: targets.into_iter().map(Into::into).collect(),
        }
    }

    pub fn is_noop(&self) -> bool {
        self.targets.is_empty()
    }
}

/// The required, typed per-target portion of an ntm send response.
///
/// The optional root success flag is retained when present, but never substitutes
/// for the required per-target fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NtmSendEnvelope {
    pub targets: Vec<String>,
    pub successful: Vec<String>,
    pub failed: Vec<String>,
    pub blocked: bool,
    pub success: Option<bool>,
}

impl NtmSendEnvelope {
    /// Parse an envelope while enforcing its shape (including all required fields).
    pub fn parse(input: impl AsRef<[u8]>) -> Result<Self, EnvelopeError> {
        let bytes = input.as_ref();
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|error| EnvelopeError::InvalidJson(error.to_string()))?;
        let object = value.as_object().ok_or(EnvelopeError::NotAnObject)?;
        let targets = string_array(object, "targets")?;
        let successful = string_array(object, "successful")?;
        let failed = string_array(object, "failed")?;
        let blocked = object
            .get("blocked")
            .ok_or(EnvelopeError::MissingField("blocked"))?
            .as_bool()
            .ok_or(EnvelopeError::WrongFieldType("blocked"))?;
        let success = match object.get("success") {
            None => None,
            Some(value) => Some(
                value
                    .as_bool()
                    .ok_or(EnvelopeError::WrongFieldType("success"))?,
            ),
        };
        if success == Some(false) {
            return Err(EnvelopeError::RootSuccessFalse);
        }
        reject_duplicates(&targets, "targets")?;
        reject_duplicates(&successful, "successful")?;
        reject_duplicates(&failed, "failed")?;
        Ok(Self {
            targets,
            successful,
            failed,
            blocked,
            success,
        })
    }

    pub fn from_json(input: impl AsRef<[u8]>) -> Result<Self, EnvelopeError> {
        Self::parse(input)
    }

    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
            && self.successful.is_empty()
            && self.failed.is_empty()
            && !self.blocked
    }
}

fn string_array(
    object: &Map<String, Value>,
    field: &'static str,
) -> Result<Vec<String>, EnvelopeError> {
    let values = object
        .get(field)
        .ok_or(EnvelopeError::MissingField(field))?
        .as_array()
        .ok_or(EnvelopeError::WrongFieldType(field))?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or(EnvelopeError::WrongFieldType(field))
        })
        .collect()
}

fn reject_duplicates(values: &[String], field: &'static str) -> Result<(), EnvelopeError> {
    let mut seen = HashSet::with_capacity(values.len());
    if let Some(value) = values.iter().find(|value| !seen.insert(value.as_str())) {
        return Err(EnvelopeError::DuplicateValue {
            field,
            value: value.clone(),
        });
    }
    Ok(())
}

/// Why an envelope could not be interpreted as a typed ntm response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvelopeError {
    InvalidJson(String),
    NotAnObject,
    MissingField(&'static str),
    WrongFieldType(&'static str),
    DuplicateValue { field: &'static str, value: String },
    RootSuccessFalse,
}

impl fmt::Display for EnvelopeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(error) => write!(formatter, "invalid ntm send JSON: {error}"),
            Self::NotAnObject => formatter.write_str("ntm send response is not a JSON object"),
            Self::MissingField(field) => {
                write!(formatter, "ntm send response missing field {field}")
            }
            Self::WrongFieldType(field) => {
                write!(
                    formatter,
                    "ntm send response field {field} has the wrong type"
                )
            }
            Self::DuplicateValue { field, value } => {
                write!(
                    formatter,
                    "ntm send response field {field} repeats target {value:?}"
                )
            }
            Self::RootSuccessFalse => formatter.write_str("ntm send root success is false"),
        }
    }
}

impl std::error::Error for EnvelopeError {}

/// A process-status input accepted by check.
pub trait ProcessStatusLike {
    fn succeeded(&self) -> bool;
}

impl ProcessStatusLike for bool {
    fn succeeded(&self) -> bool {
        *self
    }
}

/// A portable status for callers that already reduced a process result to code/signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessStatus {
    pub code: Option<i32>,
    pub success: bool,
}

impl ProcessStatus {
    pub const fn success() -> Self {
        Self {
            code: Some(0),
            success: true,
        }
    }

    pub const fn failure(code: Option<i32>) -> Self {
        Self {
            code,
            success: false,
        }
    }
}

impl ProcessStatusLike for ProcessStatus {
    fn succeeded(&self) -> bool {
        self.success
    }
}

impl ProcessStatusLike for ExitStatus {
    fn succeeded(&self) -> bool {
        self.success()
    }
}

impl ProcessStatusLike for &ExitStatus {
    fn succeeded(&self) -> bool {
        self.success()
    }
}

/// Why a typed send could not be claimed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckError {
    Envelope(EnvelopeError),
    InvalidRequest {
        field: &'static str,
        value: String,
    },
    TargetSetMismatch {
        requested: Vec<String>,
        reported: Vec<String>,
    },
    SuccessfulSetMismatch {
        requested: Vec<String>,
        reported: Vec<String>,
    },
    FailedTargets(Vec<String>),
    Blocked,
}

impl fmt::Display for CheckError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Envelope(error) => error.fmt(formatter),
            Self::InvalidRequest { field, value } => {
                write!(formatter, "invalid request {field}: {value}")
            }
            Self::TargetSetMismatch {
                requested,
                reported,
            } => {
                write!(
                    formatter,
                    "target set mismatch: requested={requested:?} reported={reported:?}"
                )
            }
            Self::SuccessfulSetMismatch {
                requested,
                reported,
            } => {
                write!(
                    formatter,
                    "successful set mismatch: requested={requested:?} reported={reported:?}"
                )
            }
            Self::FailedTargets(targets) => write!(formatter, "failed targets: {targets:?}"),
            Self::Blocked => formatter.write_str("ntm send was blocked"),
        }
    }
}

impl std::error::Error for CheckError {}

impl From<EnvelopeError> for CheckError {
    fn from(error: EnvelopeError) -> Self {
        Self::Envelope(error)
    }
}

/// The externally visible classification of one send attempt.
///
/// HealthyNoOp is reserved for a successful process and an empty request with a
/// complete empty envelope. A validated non-empty send is TypedNonzero; all
/// malformed or contradictory observations are Error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckResult {
    HealthyNoOp,
    TypedNonzero,
    Error(CheckError),
}

pub type NtmSendCheckResult = CheckResult;

impl CheckResult {
    pub fn error(&self) -> Option<&CheckError> {
        match self {
            Self::Error(error) => Some(error),
            Self::HealthyNoOp | Self::TypedNonzero => None,
        }
    }

    pub fn is_healthy_noop(&self) -> bool {
        matches!(self, Self::HealthyNoOp)
    }

    pub fn is_typed_nonzero(&self) -> bool {
        matches!(self, Self::TypedNonzero)
    }
}

/// Check one process status and its raw ntm response against the requested targets.
///
/// Process success is checked before either healthy classification is returned. A
/// failed process with a well-formed response is therefore TypedNonzero, never a
/// healthy no-op. Invalid JSON/shape and any target/result contradiction are Error.
pub fn check<S>(request: &NtmSendRequest, status: S, stdout: impl AsRef<[u8]>) -> CheckResult
where
    S: ProcessStatusLike,
{
    let envelope = match NtmSendEnvelope::parse(stdout) {
        Ok(envelope) => envelope,
        Err(error) => return CheckResult::Error(error.into()),
    };
    if let Err(error) = validate_request(request) {
        return CheckResult::Error(error);
    }
    if !same_set(&request.targets, &envelope.targets) {
        return CheckResult::Error(CheckError::TargetSetMismatch {
            requested: request.targets.clone(),
            reported: envelope.targets,
        });
    }
    if !same_set(&request.targets, &envelope.successful) {
        return CheckResult::Error(CheckError::SuccessfulSetMismatch {
            requested: request.targets.clone(),
            reported: envelope.successful,
        });
    }
    if !envelope.failed.is_empty() {
        return CheckResult::Error(CheckError::FailedTargets(envelope.failed));
    }
    if envelope.blocked {
        return CheckResult::Error(CheckError::Blocked);
    }
    if !status.succeeded() {
        return CheckResult::TypedNonzero;
    }
    if request.is_noop() {
        CheckResult::HealthyNoOp
    } else {
        CheckResult::TypedNonzero
    }
}

pub fn check_success(
    request: &NtmSendRequest,
    process_succeeded: bool,
    stdout: impl AsRef<[u8]>,
) -> CheckResult {
    check(request, process_succeeded, stdout)
}

pub fn check_process(
    request: &NtmSendRequest,
    status: &ExitStatus,
    stdout: impl AsRef<[u8]>,
) -> CheckResult {
    check(request, status, stdout)
}

fn validate_request(request: &NtmSendRequest) -> Result<(), CheckError> {
    reject_duplicates(&request.targets, "targets").map_err(CheckError::Envelope)
}

fn same_set(left: &[String], right: &[String]) -> bool {
    left.len() == right.len()
        && left.iter().collect::<HashSet<_>>() == right.iter().collect::<HashSet<_>>()
}

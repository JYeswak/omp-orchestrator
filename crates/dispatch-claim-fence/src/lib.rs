#![forbid(unsafe_code)]

//! Typed, fail-closed authorization for bead dispatch packets.
//!
//! This crate authorizes a packet from a point-in-time `br show --json`
//! projection. A [`DispatchPermit`] does not attest that transport occurred;
//! the dispatch ledger remains the authority for that separate claim.

use serde::Deserialize;

/// The tracker status relevant to dispatch admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BeadStatus {
    Open,
    InProgress,
    Closed,
    Blocked,
    Deferred,
    Unknown(String),
}

impl BeadStatus {
    fn parse(value: &str) -> Self {
        match value.trim() {
            "open" => Self::Open,
            "in_progress" => Self::InProgress,
            "closed" => Self::Closed,
            "blocked" => Self::Blocked,
            "deferred" => Self::Deferred,
            other => Self::Unknown(other.to_owned()),
        }
    }

    fn label(&self) -> &str {
        match self {
            Self::Open => "open",
            Self::InProgress => "in_progress",
            Self::Closed => "closed",
            Self::Blocked => "blocked",
            Self::Deferred => "deferred",
            Self::Unknown(value) => value,
        }
    }
}

/// The typed fields needed to authorize one bead packet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BeadSnapshot {
    id: String,
    title: String,
    description: String,
    status: BeadStatus,
    assignee: Option<String>,
}

impl BeadSnapshot {
    /// Builds a snapshot from already-separated tracker fields.
    pub fn new(
        id: &str,
        title: &str,
        description: &str,
        status: &str,
        assignee: Option<&str>,
    ) -> Self {
        Self {
            id: id.trim().to_owned(),
            title: title.to_owned(),
            description: description.to_owned(),
            status: BeadStatus::parse(status),
            assignee: normalize_optional(assignee),
        }
    }

    /// Tracker identifier used in the packet.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Human-readable bead title used in the packet.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Tracker description used in the packet.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Canonical tracker status label.
    pub fn status_label(&self) -> &str {
        self.status.label()
    }

    /// Tracker assignee, if one is recorded.
    pub fn assignee(&self) -> Option<&str> {
        self.assignee.as_deref()
    }
}

/// A dispatch operation. Broadcasts and corrections are intentionally not
/// represented as bead dispatches, so they cannot bypass the bead fence by
/// supplying an empty bead identifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispatchIntent {
    Bead {
        bead_id: String,
        receiver_agent: String,
    },
    Broadcast {
        operation: String,
        receiver_agent: String,
    },
    Correction {
        operation: String,
        receiver_agent: String,
    },
}

impl DispatchIntent {
    pub fn bead(bead_id: &str, receiver_agent: &str) -> Self {
        Self::Bead {
            bead_id: bead_id.trim().to_owned(),
            receiver_agent: receiver_agent.trim().to_owned(),
        }
    }

    pub fn broadcast(operation: &str, receiver_agent: &str) -> Self {
        Self::Broadcast {
            operation: operation.trim().to_owned(),
            receiver_agent: receiver_agent.trim().to_owned(),
        }
    }

    pub fn correction(operation: &str, receiver_agent: &str) -> Self {
        Self::Correction {
            operation: operation.trim().to_owned(),
            receiver_agent: receiver_agent.trim().to_owned(),
        }
    }
}

/// Authorization result. This is a permission to proceed with packet
/// construction, not proof that a transport call or ledger write happened.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispatchPermit {
    Bead {
        bead_id: String,
        receiver_agent: String,
    },
    Broadcast {
        operation: String,
        receiver_agent: String,
    },
    Correction {
        operation: String,
        receiver_agent: String,
    },
}

/// Claim-fence rejection. Every rejected bead includes the observed status and
/// a command that would make the claim explicit and actionable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClaimFenceError {
    MissingBeadId,
    MissingReceiverAgent,
    MissingOperation,
    MissingSnapshot {
        bead_id: String,
    },
    SnapshotIdMismatch {
        requested: String,
        observed: String,
    },
    ClaimRequired {
        bead_id: String,
        actual_status: String,
        actual_assignee: Option<String>,
        expected_agent: String,
        command: String,
    },
    AssignedElsewhere {
        bead_id: String,
        actual_status: String,
        actual_assignee: String,
        expected_agent: String,
        command: String,
    },
    UnknownStatus {
        bead_id: String,
        actual_status: String,
    },
}

impl ClaimFenceError {
    pub fn command(&self) -> Option<&str> {
        match self {
            Self::ClaimRequired { command, .. } | Self::AssignedElsewhere { command, .. } => {
                Some(command)
            }
            _ => None,
        }
    }
}

impl std::fmt::Display for ClaimFenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingBeadId => formatter.write_str(
                "DISPATCH_ERROR bead id is missing; use a named broadcast or correction operation",
            ),
            Self::MissingReceiverAgent => {
                formatter.write_str("DISPATCH_BLOCKED receiver agent is missing")
            }
            Self::MissingOperation => formatter.write_str("DISPATCH_ERROR named operation is missing"),
            Self::MissingSnapshot { bead_id } => write!(
                formatter,
                "DISPATCH_BLOCKED bead={bead_id} tracker snapshot is missing"
            ),
            Self::SnapshotIdMismatch { requested, observed } => write!(
                formatter,
                "DISPATCH_BLOCKED requested bead={requested} but tracker returned bead={observed}"
            ),
            Self::ClaimRequired {
                bead_id,
                actual_status,
                actual_assignee,
                expected_agent,
                command,
            } => write!(
                formatter,
                "DISPATCH_BLOCKED bead={bead_id} status={actual_status} assignee={} receiver={expected_agent}; claim it first: {command}",
                actual_assignee.as_deref().unwrap_or("unassigned")
            ),
            Self::AssignedElsewhere {
                bead_id,
                actual_status,
                actual_assignee,
                expected_agent,
                command,
            } => write!(
                formatter,
                "DISPATCH_BLOCKED bead={bead_id} status={actual_status} assignee={actual_assignee} receiver={expected_agent}; claim it first: {command}"
            ),
            Self::UnknownStatus {
                bead_id,
                actual_status,
            } => write!(
                formatter,
                "DISPATCH_BLOCKED bead={bead_id} tracker status={actual_status} is not recognized"
            ),
        }
    }
}

impl std::error::Error for ClaimFenceError {}

/// Authorizes an operation against the tracker snapshot immediately before
/// packet construction. Bead dispatch requires `in_progress` and exact
/// receiver ownership; other named operations have their own explicit lane.
pub fn authorize(
    intent: &DispatchIntent,
    snapshot: Option<&BeadSnapshot>,
) -> Result<DispatchPermit, ClaimFenceError> {
    match intent {
        DispatchIntent::Bead {
            bead_id,
            receiver_agent,
        } => {
            if bead_id.trim().is_empty() {
                return Err(ClaimFenceError::MissingBeadId);
            }
            if receiver_agent.trim().is_empty() {
                return Err(ClaimFenceError::MissingReceiverAgent);
            }
            let snapshot = snapshot.ok_or_else(|| ClaimFenceError::MissingSnapshot {
                bead_id: bead_id.clone(),
            })?;
            if snapshot.id != *bead_id {
                return Err(ClaimFenceError::SnapshotIdMismatch {
                    requested: bead_id.clone(),
                    observed: snapshot.id.clone(),
                });
            }
            let command = claim_command(bead_id, receiver_agent);
            if matches!(snapshot.status, BeadStatus::Unknown(_)) {
                return Err(ClaimFenceError::UnknownStatus {
                    bead_id: bead_id.clone(),
                    actual_status: snapshot.status_label().to_owned(),
                });
            }
            if snapshot.status != BeadStatus::InProgress {
                return Err(ClaimFenceError::ClaimRequired {
                    bead_id: bead_id.clone(),
                    actual_status: snapshot.status_label().to_owned(),
                    actual_assignee: snapshot.assignee.clone(),
                    expected_agent: receiver_agent.clone(),
                    command,
                });
            }
            match snapshot.assignee.as_deref() {
                Some(actual) if actual == receiver_agent => Ok(DispatchPermit::Bead {
                    bead_id: bead_id.clone(),
                    receiver_agent: receiver_agent.clone(),
                }),
                Some(actual) => Err(ClaimFenceError::AssignedElsewhere {
                    bead_id: bead_id.clone(),
                    actual_status: snapshot.status_label().to_owned(),
                    actual_assignee: actual.to_owned(),
                    expected_agent: receiver_agent.clone(),
                    command,
                }),
                None => Err(ClaimFenceError::ClaimRequired {
                    bead_id: bead_id.clone(),
                    actual_status: snapshot.status_label().to_owned(),
                    actual_assignee: None,
                    expected_agent: receiver_agent.clone(),
                    command,
                }),
            }
        }
        DispatchIntent::Broadcast {
            operation,
            receiver_agent,
        } => {
            require_named_operation(operation, receiver_agent)?;
            Ok(DispatchPermit::Broadcast {
                operation: operation.clone(),
                receiver_agent: receiver_agent.clone(),
            })
        }
        DispatchIntent::Correction {
            operation,
            receiver_agent,
        } => {
            require_named_operation(operation, receiver_agent)?;
            Ok(DispatchPermit::Correction {
                operation: operation.clone(),
                receiver_agent: receiver_agent.clone(),
            })
        }
    }
}

fn require_named_operation(operation: &str, receiver_agent: &str) -> Result<(), ClaimFenceError> {
    if operation.trim().is_empty() {
        return Err(ClaimFenceError::MissingOperation);
    }
    if receiver_agent.trim().is_empty() {
        return Err(ClaimFenceError::MissingReceiverAgent);
    }
    Ok(())
}

fn claim_command(bead_id: &str, receiver_agent: &str) -> String {
    format!("br update {bead_id} --assignee {receiver_agent} --status in_progress")
}

fn normalize_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[derive(Debug, Deserialize)]
struct BrShowRow {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
    status: String,
    #[serde(default)]
    assignee: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BrShowPayload {
    Rows(Vec<BrShowRow>),
    Row(BrShowRow),
    Envelope { issues: Vec<BrShowRow> },
}

/// Parses the JSON emitted by `br show <id> --json` without exposing an
/// untyped JSON value to callers.
pub fn parse_br_show_json(bytes: &[u8]) -> Result<BeadSnapshot, SnapshotParseError> {
    let payload: BrShowPayload = serde_json::from_slice(bytes)
        .map_err(|error| SnapshotParseError::Malformed(error.to_string()))?;
    let row = match payload {
        BrShowPayload::Rows(mut rows) => rows.drain(..).next(),
        BrShowPayload::Row(row) => Some(row),
        BrShowPayload::Envelope { mut issues } => issues.drain(..).next(),
    }
    .ok_or(SnapshotParseError::Empty)?;
    if row.id.trim().is_empty() {
        return Err(SnapshotParseError::MissingField("id"));
    }
    if row.status.trim().is_empty() {
        return Err(SnapshotParseError::MissingField("status"));
    }
    Ok(BeadSnapshot::new(
        &row.id,
        &row.title,
        &row.description,
        &row.status,
        row.assignee.as_deref(),
    ))
}

/// Failure while converting tracker output into a typed snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SnapshotParseError {
    Malformed(String),
    Empty,
    MissingField(&'static str),
}

impl std::fmt::Display for SnapshotParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(detail) => write!(formatter, "malformed br show JSON: {detail}"),
            Self::Empty => formatter.write_str("br show returned no bead row"),
            Self::MissingField(field) => write!(formatter, "br show row is missing {field}"),
        }
    }
}

impl std::error::Error for SnapshotParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_status_is_not_admitted() {
        let error = authorize(
            &DispatchIntent::bead("5rh", "BlueLantern"),
            Some(&BeadSnapshot::new("5rh", "title", "body", "mystery", None)),
        )
        .expect_err("unknown tracker status must fail closed");
        assert!(matches!(error, ClaimFenceError::UnknownStatus { .. }));
    }

    #[test]
    fn envelope_json_is_supported() {
        let snapshot =
            parse_br_show_json(br#"{"issues":[{"id":"5rh","status":"open","assignee":null}]}"#)
                .expect("envelope output");
        assert_eq!(snapshot.id(), "5rh");
        assert_eq!(snapshot.assignee(), None);
    }
}

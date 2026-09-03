#![forbid(unsafe_code)]

//! Typed, fail-closed authorization for bead dispatch packets.
//!
//! This crate authorizes a packet from a point-in-time `br show --json`
//! projection. A [`DispatchPermit`] does not attest that transport occurred;
//! the dispatch ledger remains the authority for that separate claim.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
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

/// The two independent places from which an assignee may be known.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum IdentityNamespace {
    AgentMail,
    Subagent,
}

impl std::fmt::Display for IdentityNamespace {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::AgentMail => "agent_mail",
            Self::Subagent => "subagents",
        })
    }
}

/// One identity entry from one namespace, with the actor key that lets a
/// diagnostic recognize aliases without treating them as a refusal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityRecord {
    namespace: IdentityNamespace,
    name: String,
    actor_id: String,
}

impl IdentityRecord {
    pub fn agent_mail(name: &str, actor_id: &str) -> Self {
        Self::new(IdentityNamespace::AgentMail, name, actor_id)
    }

    pub fn subagent(name: &str, actor_id: &str) -> Self {
        Self::new(IdentityNamespace::Subagent, name, actor_id)
    }

    fn new(namespace: IdentityNamespace, name: &str, actor_id: &str) -> Self {
        Self {
            namespace,
            name: name.trim().to_owned(),
            actor_id: actor_id.trim().to_owned(),
        }
    }

    pub fn namespace(&self) -> IdentityNamespace {
        self.namespace
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn actor_id(&self) -> &str {
        &self.actor_id
    }
}

/// A resolved assignee. Resolution is by name, while actor_id is retained for
/// duplicate-name reporting and later lifecycle reconciliation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedIdentity {
    assignee: String,
    actor_id: String,
    namespaces: Vec<IdentityNamespace>,
}

impl ResolvedIdentity {
    pub fn assignee(&self) -> &str {
        &self.assignee
    }

    pub fn actor_id(&self) -> &str {
        &self.actor_id
    }

    pub fn namespaces(&self) -> &[IdentityNamespace] {
        &self.namespaces
    }
}

/// An alias group is diagnostic data, not an identity failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DuplicateAlias {
    actor_id: String,
    names: Vec<String>,
}

impl DuplicateAlias {
    pub fn actor_id(&self) -> &str {
        &self.actor_id
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }
}

/// A refusal or operational failure from the dual identity namespace check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssigneeIdentityError {
    RegistryUnavailable {
        missing: Vec<IdentityNamespace>,
    },
    UnknownAssignee {
        assignee: String,
        agent_mail: Vec<String>,
        subagents: Vec<String>,
    },
    AmbiguousAssignee {
        assignee: String,
        matches: Vec<ResolvedIdentity>,
    },
}

impl AssigneeIdentityError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::RegistryUnavailable { .. } => "IDENTITY_REGISTRY_UNAVAILABLE",
            Self::UnknownAssignee { .. } => "ASSIGNEE_NOT_REGISTERED",
            Self::AmbiguousAssignee { .. } => "ASSIGNEE_AMBIGUOUS",
        }
    }
}

impl std::fmt::Display for AssigneeIdentityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RegistryUnavailable { missing } => write!(
                formatter,
                "ASSIGNEE_IDENTITY_ERROR reason=identity_registry_unavailable missing={} checked=agent_mail,subagents",
                missing.iter().map(ToString::to_string).collect::<Vec<_>>().join(",")
            ),
            Self::UnknownAssignee {
                assignee,
                agent_mail,
                subagents,
            } => write!(
                formatter,
                "ASSIGNEE_IDENTITY_REFUSED assignee={assignee} reason=unknown_assignee checked=agent_mail names=[{}] checked=subagents names=[{}]",
                agent_mail.join(","),
                subagents.join(",")
            ),
            Self::AmbiguousAssignee { assignee, matches } => write!(
                formatter,
                "ASSIGNEE_IDENTITY_ERROR assignee={assignee} reason=ambiguous_assignee matches={}",
                matches
                    .iter()
                    .map(|identity| format!("{}:{}", identity.actor_id, identity.namespaces.iter().map(ToString::to_string).collect::<Vec<_>>().join("+")))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}

impl std::error::Error for AssigneeIdentityError {}

/// Snapshots of the Agent Mail roster and the parent-owned subagent registry.
/// Both must be present and non-empty before any assignee is accepted.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IdentityRegistries {
    agent_mail: Vec<IdentityRecord>,
    subagents: Vec<IdentityRecord>,
}

impl IdentityRegistries {
    pub fn new(agent_mail: Vec<IdentityRecord>, subagents: Vec<IdentityRecord>) -> Self {
        Self {
            agent_mail,
            subagents,
        }
    }

    pub fn agent_mail(&self) -> &[IdentityRecord] {
        &self.agent_mail
    }

    pub fn subagents(&self) -> &[IdentityRecord] {
        &self.subagents
    }

    /// Resolve an assignee only when both registry snapshots are available.
    pub fn resolve(&self, assignee: &str) -> Result<ResolvedIdentity, AssigneeIdentityError> {
        let mut missing = Vec::new();
        if self.agent_mail.is_empty() {
            missing.push(IdentityNamespace::AgentMail);
        }
        if self.subagents.is_empty() {
            missing.push(IdentityNamespace::Subagent);
        }
        if !missing.is_empty() {
            return Err(AssigneeIdentityError::RegistryUnavailable { missing });
        }

        let assignee = assignee.trim().to_owned();
        let matches: Vec<&IdentityRecord> = self
            .agent_mail
            .iter()
            .chain(self.subagents.iter())
            .filter(|record| record.name == assignee)
            .collect();
        let actor_ids: BTreeSet<&str> = matches
            .iter()
            .map(|record| record.actor_id.as_str())
            .collect();
        if actor_ids.is_empty() {
            let mut agent_mail: Vec<String> = self
                .agent_mail
                .iter()
                .map(|record| record.name.clone())
                .collect();
            let mut subagents: Vec<String> = self
                .subagents
                .iter()
                .map(|record| record.name.clone())
                .collect();
            agent_mail.sort();
            subagents.sort();
            return Err(AssigneeIdentityError::UnknownAssignee {
                assignee,
                agent_mail,
                subagents,
            });
        }
        if actor_ids.len() > 1 {
            return Err(AssigneeIdentityError::AmbiguousAssignee {
                assignee,
                matches: matches
                    .iter()
                    .map(|record| ResolvedIdentity {
                        assignee: record.name.clone(),
                        actor_id: record.actor_id.clone(),
                        namespaces: vec![record.namespace],
                    })
                    .collect(),
            });
        }

        let actor_id = (*actor_ids.iter().next().expect("nonempty actor ids")).to_owned();
        let mut namespaces: Vec<IdentityNamespace> =
            matches.iter().map(|record| record.namespace).collect();
        namespaces.sort_unstable();
        namespaces.dedup();
        Ok(ResolvedIdentity {
            assignee,
            actor_id,
            namespaces,
        })
    }

    /// Return every actor represented by more than one distinct assignee name.
    pub fn duplicate_aliases(&self) -> Vec<DuplicateAlias> {
        let mut names_by_actor: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for record in self.agent_mail.iter().chain(self.subagents.iter()) {
            if !record.actor_id.is_empty() && !record.name.is_empty() {
                names_by_actor
                    .entry(record.actor_id.as_str())
                    .or_default()
                    .insert(record.name.as_str());
            }
        }
        names_by_actor
            .into_iter()
            .filter_map(|(actor_id, names)| {
                (names.len() > 1).then(|| DuplicateAlias {
                    actor_id: actor_id.to_owned(),
                    names: names.into_iter().map(ToOwned::to_owned).collect(),
                })
            })
            .collect()
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

/// Evidence from the durable dispatch ledger for the claimed bead.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispatchLedgerEvidence {
    Present,
    Absent,
    Unavailable { reason: String },
}

impl std::fmt::Display for DispatchLedgerEvidence {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Present => formatter.write_str("PRESENT"),
            Self::Absent => formatter.write_str("ABSENT"),
            Self::Unavailable { reason } => write!(formatter, "UNAVAILABLE reason={reason}"),
        }
    }
}

/// The mutually exclusive next action for an `open` + assigned refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimRemedy {
    CompleteClaim,
    ReleaseFully,
    EvidenceUnavailable,
}

impl std::fmt::Display for ClaimRemedy {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::CompleteClaim => "COMPLETE_CLAIM",
            Self::ReleaseFully => "RELEASE_FULLY",
            Self::EvidenceUnavailable => "EVIDENCE_UNAVAILABLE",
        })
    }
}

/// An actionable refusal that retains both mutually exclusive remedies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimRequiredAdvice {
    bead_id: String,
    assignee: String,
    evidence: DispatchLedgerEvidence,
    remedy: ClaimRemedy,
    claim_command: String,
    release_command: String,
}

impl ClaimRequiredAdvice {
    pub fn remedy(&self) -> ClaimRemedy {
        self.remedy
    }

    pub fn evidence(&self) -> &DispatchLedgerEvidence {
        &self.evidence
    }
}

impl std::fmt::Display for ClaimRequiredAdvice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "CLAIM_REQUIRED_ACTIONABLE bead={} assignee={} dispatch_ledger={} remedy={} mutually_exclusive=true complete_claim={} release_fully={} next_action={}",
            self.bead_id,
            self.assignee,
            self.evidence,
            self.remedy,
            self.claim_command,
            self.release_command,
            match self.remedy {
                ClaimRemedy::CompleteClaim => "complete_claim",
                ClaimRemedy::ReleaseFully => "release_fully",
                ClaimRemedy::EvidenceUnavailable => "inspect_dispatch_ledger",
            }
        )
    }
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
    AssigneeIdentity(AssigneeIdentityError),
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

    /// Attach dispatch-ledger evidence to an `open` + assigned refusal.
    /// `Present` selects completing the claim; `Absent` selects a full release;
    /// `Unavailable` preserves both choices without guessing.
    pub fn claim_required_advice(
        &self,
        evidence: DispatchLedgerEvidence,
    ) -> Option<ClaimRequiredAdvice> {
        let Self::ClaimRequired {
            bead_id,
            actual_assignee,
            expected_agent,
            ..
        } = self
        else {
            return None;
        };
        let assignee = actual_assignee
            .as_deref()
            .unwrap_or("unassigned")
            .to_owned();
        let remedy = match evidence {
            DispatchLedgerEvidence::Present => ClaimRemedy::CompleteClaim,
            DispatchLedgerEvidence::Absent => ClaimRemedy::ReleaseFully,
            DispatchLedgerEvidence::Unavailable { .. } => ClaimRemedy::EvidenceUnavailable,
        };
        Some(ClaimRequiredAdvice {
            bead_id: bead_id.clone(),
            assignee,
            evidence,
            remedy,
            claim_command: claim_command(bead_id, expected_agent),
            release_command: release_command(bead_id),
        })
    }

    /// Stable machine-readable reason label.
    ///
    /// The operator-facing refusal names this instead of a prose fragment, so a
    /// refusal can be counted by cause without parsing English. Measured
    /// 2026-09-01: 135 re-dispatches of one unclaimed bead were invisible
    /// because the only per-tick trace was a transport label.
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingBeadId => "MISSING_BEAD_ID",
            Self::MissingReceiverAgent => "MISSING_RECEIVER_AGENT",
            Self::MissingOperation => "MISSING_OPERATION",
            Self::MissingSnapshot { .. } => "MISSING_SNAPSHOT",
            Self::SnapshotIdMismatch { .. } => "SNAPSHOT_ID_MISMATCH",
            Self::ClaimRequired { .. } => "CLAIM_REQUIRED",
            Self::AssignedElsewhere { .. } => "ASSIGNED_ELSEWHERE",
            Self::UnknownStatus { .. } => "UNKNOWN_STATUS",
            Self::AssigneeIdentity(error) => error.code(),
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
            Self::AssigneeIdentity(error) => std::fmt::Display::fmt(error, formatter),
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

/// Authorizes a bead dispatch and verifies that its receiver exists in both
/// the Agent Mail and parent-owned subagent registry snapshots. The legacy
/// `authorize` function remains the claim/status-only boundary for callers that
/// have not yet supplied identity snapshots.
pub fn authorize_with_identities(
    intent: &DispatchIntent,
    snapshot: Option<&BeadSnapshot>,
    identities: &IdentityRegistries,
) -> Result<DispatchPermit, ClaimFenceError> {
    let permit = authorize(intent, snapshot)?;
    if let DispatchIntent::Bead { receiver_agent, .. } = intent {
        identities
            .resolve(receiver_agent)
            .map_err(ClaimFenceError::AssigneeIdentity)?;
    }
    Ok(permit)
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
fn release_command(bead_id: &str) -> String {
    format!("br update {bead_id} --assignee \"\" --status open")
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

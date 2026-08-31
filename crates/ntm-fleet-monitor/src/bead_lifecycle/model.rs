#![forbid(unsafe_code)]

//! Input, evidence, status, and error types for the bead lifecycle.

use crate::NotApproved;
use serde_json::json;
use std::fmt;

pub const BEAD_LIFECYCLE_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BeadId(String);
impl BeadId {
    pub fn new(raw: impl Into<String>) -> Result<Self, LifecycleError> {
        let raw = raw.into();
        if raw.trim().is_empty() {
            return Err(LifecycleError::InvalidInput { field: "bead" });
        }
        Ok(Self(raw))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId(String);
impl EventId {
    pub fn new(raw: impl Into<String>) -> Result<Self, LifecycleError> {
        let raw = raw.into();
        if raw.trim().is_empty() {
            return Err(LifecycleError::InvalidInput { field: "event_id" });
        }
        Ok(Self(raw))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DispatchTarget {
    pub session: String,
    pub pane: String,
}
impl DispatchTarget {
    pub fn new(
        session: impl Into<String>,
        pane: impl Into<String>,
    ) -> Result<Self, LifecycleError> {
        let session = session.into();
        let pane = pane.into();
        if session.trim().is_empty() {
            return Err(LifecycleError::InvalidInput { field: "session" });
        }
        if pane.trim().is_empty() {
            return Err(LifecycleError::InvalidInput { field: "pane" });
        }
        Ok(Self { session, pane })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidencePolicy {
    pub now_ms: u64,
    pub max_age_ms: u64,
}
impl EvidencePolicy {
    pub const fn new(now_ms: u64, max_age_ms: u64) -> Self {
        Self { now_ms, max_age_ms }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LifecycleStatus {
    Selected,
    Dispatched,
    ReceiverVerified,
    Ungraded,
    Grading,
    GradedPass,
    GradedFix,
    Closed,
    RedispatchRequired,
    Blocked,
}
impl LifecycleStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Selected => "selected",
            Self::Dispatched => "dispatched",
            Self::ReceiverVerified => "receiver_verified",
            Self::Ungraded => "ungraded",
            Self::Grading => "grading",
            Self::GradedPass => "graded_pass",
            Self::GradedFix => "graded_fix",
            Self::Closed => "closed",
            Self::RedispatchRequired => "redispatch_required",
            Self::Blocked => "blocked",
        }
    }
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Closed | Self::Blocked)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LifecycleEventKind {
    Selected,
    Dispatched,
    ReceiverVerified,
    MarkedUngraded,
    GradingStarted,
    GradedPass,
    GradedFix,
    Closed,
    RedispatchRequired,
    Blocked,
}
impl LifecycleEventKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Selected => "selected",
            Self::Dispatched => "dispatched",
            Self::ReceiverVerified => "receiver_verified",
            Self::MarkedUngraded => "marked_ungraded",
            Self::GradingStarted => "grading_started",
            Self::GradedPass => "graded_pass",
            Self::GradedFix => "graded_fix",
            Self::Closed => "closed",
            Self::RedispatchRequired => "redispatch_required",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BlockerKind {
    ExternalDependency,
    HumanApproval,
    LocalCondition,
}
impl BlockerKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExternalDependency => "external-dependency",
            Self::HumanApproval => "human-approval",
            Self::LocalCondition => "local-condition",
        }
    }
    pub const fn is_external(self) -> bool {
        matches!(self, Self::ExternalDependency | Self::HumanApproval)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EscalationRef(String);
impl EscalationRef {
    pub fn new(raw: impl Into<String>) -> Result<Self, LifecycleError> {
        let raw = raw.into();
        if raw.trim().is_empty() {
            return Err(LifecycleError::InvalidInput {
                field: "escalation_ref",
            });
        }
        Ok(Self(raw))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockerEvidence {
    kind: BlockerKind,
    reason: BlockReason,
    escalation_ref: EscalationRef,
    observed_at_ms: u64,
}
impl BlockerEvidence {
    pub fn new(
        kind: BlockerKind,
        reason: BlockReason,
        escalation_ref: Option<EscalationRef>,
        observed_at_ms: u64,
    ) -> Result<Self, LifecycleError> {
        let Some(escalation_ref) = escalation_ref else {
            return Err(LifecycleError::MissingEscalationReference);
        };
        Ok(Self {
            kind,
            reason,
            escalation_ref,
            observed_at_ms,
        })
    }
    pub const fn kind(&self) -> BlockerKind {
        self.kind
    }
    pub fn reason(&self) -> &BlockReason {
        &self.reason
    }
    pub fn escalation_ref(&self) -> &EscalationRef {
        &self.escalation_ref
    }
    pub const fn observed_at_ms(&self) -> u64 {
        self.observed_at_ms
    }
    fn serialized(&self) -> serde_json::Value {
        json!({
            "kind": self.kind.as_str(),
            "reason": self.reason.as_str(),
            "escalation_ref": self.escalation_ref.as_str(),
            "observed_at_ms": self.observed_at_ms,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleEvent {
    pub id: EventId,
    pub kind: LifecycleEventKind,
    pub status: LifecycleStatus,
    pub blocker: Option<BlockerEvidence>,
}
impl LifecycleEvent {
    pub fn serialized(&self) -> String {
        let mut value = json!({
            "schema": BEAD_LIFECYCLE_SCHEMA_VERSION,
            "event": self.kind.as_str(),
            "event_id": self.id.as_str(),
            "status": self.status.as_str(),
        });
        if let Some(blocker) = self.blocker.as_ref() {
            value["blocker"] = blocker.serialized();
        }
        value.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchReceipt {
    pub id: EventId,
    pub bead: BeadId,
    pub target: DispatchTarget,
    pub objective: String,
    pub dispatched_at_ms: u64,
}
impl DispatchReceipt {
    pub fn new(
        id: EventId,
        bead: BeadId,
        target: DispatchTarget,
        objective: impl Into<String>,
        dispatched_at_ms: u64,
    ) -> Result<Self, LifecycleError> {
        let objective = objective.into();
        if objective.trim().is_empty() {
            return Err(LifecycleError::InvalidInput { field: "objective" });
        }
        Ok(Self {
            id,
            bead,
            target,
            objective,
            dispatched_at_ms,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiverEvidence {
    pub id: EventId,
    pub bead: BeadId,
    pub target: DispatchTarget,
    pub objective: String,
    pub observed_at_ms: u64,
}
impl ReceiverEvidence {
    pub fn new(
        id: EventId,
        bead: BeadId,
        target: DispatchTarget,
        objective: impl Into<String>,
        observed_at_ms: u64,
    ) -> Result<Self, LifecycleError> {
        let objective = objective.into();
        if objective.trim().is_empty() {
            return Err(LifecycleError::InvalidInput {
                field: "receiver_objective",
            });
        }
        Ok(Self {
            id,
            bead,
            target,
            objective,
            observed_at_ms,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GradeResult {
    Pass,
    Fix { name: String },
}
impl GradeResult {
    pub fn fix(name: impl Into<String>) -> Result<Self, LifecycleError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(LifecycleError::InvalidInput { field: "fix_name" });
        }
        Ok(Self::Fix { name })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GradeReceipt {
    pub id: EventId,
    pub bead: BeadId,
    pub target: DispatchTarget,
    pub receiver_event_id: EventId,
    pub result: GradeResult,
    pub graded_at_ms: u64,
}
impl GradeReceipt {
    pub fn pass(
        id: EventId,
        bead: BeadId,
        target: DispatchTarget,
        receiver_event_id: EventId,
        graded_at_ms: u64,
    ) -> Self {
        Self {
            id,
            bead,
            target,
            receiver_event_id,
            result: GradeResult::Pass,
            graded_at_ms,
        }
    }
    pub fn fix(
        id: EventId,
        bead: BeadId,
        target: DispatchTarget,
        receiver_event_id: EventId,
        name: impl Into<String>,
        graded_at_ms: u64,
    ) -> Result<Self, LifecycleError> {
        Ok(Self {
            id,
            bead,
            target,
            receiver_event_id,
            result: GradeResult::fix(name)?,
            graded_at_ms,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedispatchPlan {
    pub id: EventId,
    pub bead: BeadId,
    pub target: DispatchTarget,
    pub fix_name: String,
}
impl RedispatchPlan {
    pub fn new(
        id: EventId,
        bead: BeadId,
        target: DispatchTarget,
        fix_name: impl Into<String>,
    ) -> Result<Self, LifecycleError> {
        let fix_name = fix_name.into();
        if fix_name.trim().is_empty() {
            return Err(LifecycleError::InvalidInput { field: "fix_name" });
        }
        Ok(Self {
            id,
            bead,
            target,
            fix_name,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockReason(String);
impl BlockReason {
    pub fn new(raw: impl Into<String>) -> Result<Self, LifecycleError> {
        let raw = raw.into();
        if raw.trim().is_empty() {
            return Err(LifecycleError::InvalidInput {
                field: "block_reason",
            });
        }
        Ok(Self(raw))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleError {
    InvalidInput {
        field: &'static str,
    },
    InvalidTransition {
        from: LifecycleStatus,
        event: LifecycleEventKind,
    },
    DuplicateEvent(EventId),
    NotApproved(NotApproved),
    WrongBead,
    WrongTarget,
    WrongObjective,
    MissingEscalationReference,
    LocalBlockerNotAllowed,
    EvidenceInFuture {
        observed_at_ms: u64,
        now_ms: u64,
    },
    StaleEvidence {
        age_ms: u64,
        max_age_ms: u64,
    },
    MissingReceiverEvidence,
    WrongReceiverEvent,
    GradeMustBePass,
}
impl fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput { field } => write!(f, "invalid empty input: {field}"),
            Self::InvalidTransition { from, event } => write!(
                f,
                "invalid transition {} -> {}",
                from.as_str(),
                event.as_str()
            ),
            Self::DuplicateEvent(id) => write!(f, "duplicate non-idempotent event {}", id.as_str()),
            Self::NotApproved(_) => f.write_str("wave is not autonomous"),
            Self::WrongBead => f.write_str("evidence bead does not match lifecycle bead"),
            Self::WrongTarget => f.write_str("evidence target does not match lifecycle target"),
            Self::WrongObjective => {
                f.write_str("evidence objective does not match lifecycle objective")
            }
            Self::MissingEscalationReference => {
                f.write_str("external blocker requires an escalation reference")
            }
            Self::LocalBlockerNotAllowed => {
                f.write_str("lifecycle blocking requires an external blocker")
            }
            Self::EvidenceInFuture { .. } => f.write_str("evidence timestamp is in the future"),
            Self::StaleEvidence { .. } => f.write_str("evidence is stale"),
            Self::MissingReceiverEvidence => {
                f.write_str("independent receiver evidence is required")
            }
            Self::WrongReceiverEvent => {
                f.write_str("grade does not name the stored receiver evidence")
            }
            Self::GradeMustBePass => f.write_str("close requires an independent passing grade"),
        }
    }
}

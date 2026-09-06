#![forbid(unsafe_code)]

//! Durable, identity-bearing rows for the typed bead lifecycle.
//!
//! [`BeadLifecycle`](super::BeadLifecycle) remains the sole transition authority.
//! This module supplies its durable carrier: every accepted transition is encoded
//! as JSONL, fsynced, and read back. The resident supervisor emits the dispatch and
//! receiver half; an independent control-plane grader can continue the same ledger
//! with grading, pass/fix, close, and redispatch rows.

use super::{
    BeadId, BeadLifecycle, BlockerEvidence, DispatchReceipt, DispatchTarget, EvidencePolicy,
    EventId, GradeReceipt, GradeResult, LifecycleError, LifecycleStatus, ReceiverEvidence,
    RedispatchPlan,
};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const BEAD_LIFECYCLE_LEDGER_SCHEMA: &str = "omp.bead.lifecycle.v1";

/// Provenance of the process that emitted a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvokerClass {
    Scheduled,
    Manual,
}

impl InvokerClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scheduled => "SCHEDULED",
            Self::Manual => "MANUAL",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "SCHEDULED" => Some(Self::Scheduled),
            "MANUAL" => Some(Self::Manual),
            _ => None,
        }
    }

    /// A tty is the only local fact separating an operator invocation from a
    /// scheduler invocation. Tests use this constructor with an explicit fact.
    pub const fn from_tty(has_tty: bool) -> Self {
        if has_tty {
            Self::Manual
        } else {
            Self::Scheduled
        }
    }

    pub fn detect_current() -> Self {
        let has_tty = io::stdin().is_terminal()
            || io::stdout().is_terminal()
            || io::stderr().is_terminal();
        Self::from_tty(has_tty)
    }
}

/// Identity required on every lifecycle row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleIdentity {
    pub bead: BeadId,
    pub repo: String,
    pub target: DispatchTarget,
    pub packet_digest: String,
    pub invoker: InvokerClass,
}

impl LifecycleIdentity {
    pub fn new(
        bead: BeadId,
        repo: impl Into<String>,
        target: DispatchTarget,
        packet_digest: impl Into<String>,
        invoker: InvokerClass,
    ) -> Result<Self, LedgerError> {
        let repo = repo.into();
        let packet_digest = packet_digest.into();
        if repo.trim().is_empty() {
            return Err(LedgerError::InvalidIdentity { field: "repo" });
        }
        if packet_digest.trim().is_empty() {
            return Err(LedgerError::InvalidIdentity {
                field: "packet_digest",
            });
        }
        Ok(Self {
            bead,
            repo,
            target,
            packet_digest,
            invoker,
        })
    }
}

/// Evidence attached to one transition. Empty evidence is unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEvidence {
    pub id: EventId,
    pub observed_at_ms: u64,
    pub policy: EvidencePolicy,
    pub fields: BTreeMap<String, String>,
}

impl LedgerEvidence {
    pub fn new<I, K, V>(
        id: EventId,
        observed_at_ms: u64,
        policy: EvidencePolicy,
        fields: I,
    ) -> Result<Self, LedgerError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let fields = fields
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect::<BTreeMap<_, _>>();
        if fields.is_empty() {
            return Err(LedgerError::EmptyEvidence);
        }
        validate_freshness(observed_at_ms, policy)?;
        Ok(Self {
            id,
            observed_at_ms,
            policy,
            fields,
        })
    }

    pub fn single(
        id: EventId,
        observed_at_ms: u64,
        policy: EvidencePolicy,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, LedgerError> {
        Self::new(id, observed_at_ms, policy, [(key.into(), value.into())])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppendOutcome {
    Appended { event: String },
    Replayed { event: String },
}

impl AppendOutcome {
    pub const fn replayed(&self) -> bool {
        matches!(self, Self::Replayed { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    InvalidIdentity { field: &'static str },
    EmptyEvidence,
    EvidenceInFuture { observed_at_ms: u64, now_ms: u64 },
    StaleEvidence { age_ms: u64, max_age_ms: u64 },
    Lifecycle(LifecycleError),
    Io { op: &'static str, detail: String },
    MalformedExistingRow { line: String },
    NoMatchingLifecycle { bead: String, target: String },
    UnsupportedReplay { event: String },
    ConflictingIdempotencyKey { key: String },
    PartialIdempotency { key: String },
    ReadbackMismatch,
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidIdentity { field } => {
                write!(f, "LIFECYCLE_LEDGER_INVALID_IDENTITY field={field}")
            }
            Self::EmptyEvidence => {
                f.write_str("LIFECYCLE_LEDGER_EMPTY_EVIDENCE — empty evidence is an ERROR")
            }
            Self::EvidenceInFuture {
                observed_at_ms,
                now_ms,
            } => write!(
                f,
                "LIFECYCLE_LEDGER_EVIDENCE_IN_FUTURE observed_at_ms={observed_at_ms} now_ms={now_ms}"
            ),
            Self::StaleEvidence { age_ms, max_age_ms } => write!(
                f,
                "LIFECYCLE_LEDGER_STALE_EVIDENCE age_ms={age_ms} max_age_ms={max_age_ms}"
            ),
            Self::Lifecycle(error) => write!(f, "LIFECYCLE_LEDGER_TRANSITION_REFUSED {error}"),
            Self::Io { op, detail } => write!(f, "LIFECYCLE_LEDGER_IO op={op} detail={detail}"),
            Self::MalformedExistingRow { line } => {
                write!(f, "LIFECYCLE_LEDGER_EXISTING_ROW_MALFORMED line={line}")
            }
            Self::NoMatchingLifecycle { bead, target } => write!(
                f,
                "LIFECYCLE_LEDGER_NO_MATCHING_LIFECYCLE bead={bead} target={target}"
            ),
            Self::UnsupportedReplay { event } => {
                write!(f, "LIFECYCLE_LEDGER_UNSUPPORTED_REPLAY event={event}")
            }
            Self::ConflictingIdempotencyKey { key } => {
                write!(f, "LIFECYCLE_LEDGER_IDEMPOTENCY_CONFLICT key={key}")
            }
            Self::PartialIdempotency { key } => {
                write!(f, "LIFECYCLE_LEDGER_IDEMPOTENCY_PARTIAL key={key}")
            }
            Self::ReadbackMismatch => f.write_str("LIFECYCLE_LEDGER_READBACK_MISMATCH"),
        }
    }
}

impl std::error::Error for LedgerError {}

impl From<LifecycleError> for LedgerError {
    fn from(error: LifecycleError) -> Self {
        Self::Lifecycle(error)
    }
}

fn validate_freshness(observed_at_ms: u64, policy: EvidencePolicy) -> Result<(), LedgerError> {
    if observed_at_ms > policy.now_ms {
        return Err(LedgerError::EvidenceInFuture {
            observed_at_ms,
            now_ms: policy.now_ms,
        });
    }
    let age_ms = policy.now_ms - observed_at_ms;
    if age_ms > policy.max_age_ms {
        return Err(LedgerError::StaleEvidence {
            age_ms,
            max_age_ms: policy.max_age_ms,
        });
    }
    Ok(())
}

/// Hash the exact packet bytes sent to the receiving pane.
pub fn packet_digest(packet: &[u8]) -> String {
    let digest = Sha256::digest(packet);
    let mut encoded = String::with_capacity(71);
    encoded.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

/// Default per-repository lifecycle ledger location.
pub fn default_ledger_path(repo: &Path, session: &str) -> PathBuf {
    repo.join(".flywheel")
        .join(format!("bead-lifecycle-{session}.jsonl"))
}

/// Durable writer around the existing [`BeadLifecycle`] state machine.
pub struct LifecycleLedger {
    path: PathBuf,
    identity: LifecycleIdentity,
    objective: String,
    lifecycle: BeadLifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiverVerifiedCandidate {
    pub identity: LifecycleIdentity,
    pub objective: String,
    pub receiver_event_id: EventId,
}

impl std::fmt::Debug for LifecycleLedger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LifecycleLedger")
            .field("path", &self.path)
            .field("identity", &self.identity)
            .field("status", &self.lifecycle.status())
            .finish()
    }
}

impl LifecycleLedger {
    /// Start a lifecycle and persist its selected row.
    pub fn start(
        path: impl Into<PathBuf>,
        identity: LifecycleIdentity,
        objective: impl Into<String>,
        approval: crate::Approved,
        selected: LedgerEvidence,
    ) -> Result<Self, LedgerError> {
        let objective = objective.into();
        let lifecycle = BeadLifecycle::select(
            identity.bead.clone(),
            identity.target.clone(),
            objective.clone(),
            approval,
            selected.id.clone(),
        )?;
        let ledger = Self {
            path: path.into(),
            identity,
            objective,
            lifecycle,
        };
        let row = ledger.row("selected", LifecycleStatus::Selected, &selected, Map::new());
        ledger.append_rows(vec![(selected.id.as_str().to_owned(), row)])?;
        Ok(ledger)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn identity(&self) -> &LifecycleIdentity {
        &self.identity
    }

    pub fn status(&self) -> LifecycleStatus {
        self.lifecycle.status()
    }
    pub fn receiver_verified_candidates(
        path: impl AsRef<Path>,
        tracker_status_path: impl AsRef<Path>,
    ) -> Result<Vec<ReceiverVerifiedCandidate>, LedgerError> {
        let statuses = read_tracker_statuses(tracker_status_path.as_ref())?;
        let mut latest = BTreeMap::new();
        for row in read_rows(path.as_ref())? {
            let identity = row_identity(&row)?;
            if !matches!(
                statuses.get(identity.bead.as_str()).map(String::as_str),
                Some("open" | "in_progress")
            ) {
                continue;
            }
            latest.insert(identity_key(&identity), row);
        }
        let mut candidates = Vec::new();
        for row in latest.values() {
            if row.get("event").and_then(Value::as_str) != Some("receiver_verified")
                || row.get("status").and_then(Value::as_str)
                    != Some(LifecycleStatus::ReceiverVerified.as_str())
            {
                continue;
            }
            let identity = row_identity(row)?;
            let objective = row_string(row, "objective")?;
            let receiver_event_id = row_event_id(row)?;
            candidates.push(ReceiverVerifiedCandidate {
                identity,
                objective,
                receiver_event_id,
            });
        }
        candidates.sort_by(|left, right| {
            left.identity
                .bead
                .as_str()
                .cmp(right.identity.bead.as_str())
                .then_with(|| left.identity.target.pane.cmp(&right.identity.target.pane))
        });
        Ok(candidates)
    }

    pub fn active_grading_panes(path: impl AsRef<Path>) -> Result<BTreeSet<String>, LedgerError> {
        let mut latest = BTreeMap::new();
        for row in read_rows(path.as_ref())? {
            let identity = row_identity(&row)?;
            latest.insert(identity_key(&identity), row);
        }
        let mut panes = BTreeSet::new();
        for row in latest.values() {
            if row.get("status").and_then(Value::as_str) != Some(LifecycleStatus::Grading.as_str()) {
                continue;
            }
            panes.insert(row_string(row, "grader_pane")?);
        }
        Ok(panes)
    }

    pub fn resume(
        path: impl Into<PathBuf>,
        identity: LifecycleIdentity,
        objective: impl Into<String>,
        approval: crate::Approved,
    ) -> Result<Self, LedgerError> {
        let path = path.into();
        let objective = objective.into();
        let mut rows = Vec::new();
        for row in read_rows(&path)? {
            if row_identity(&row)? == identity {
                rows.push(row);
            }
        }
        let Some(selected) = rows.first() else {
            return Err(LedgerError::NoMatchingLifecycle {
                bead: identity.bead.as_str().to_owned(),
                target: identity.target.pane.clone(),
            });
        };
        if selected.get("event").and_then(Value::as_str) != Some("selected") {
            return Err(LedgerError::UnsupportedReplay {
                event: selected
                    .get("event")
                    .and_then(Value::as_str)
                    .unwrap_or("missing")
                    .to_owned(),
            });
        }
        let selected_id = row_event_id(selected)?;
        let mut lifecycle = BeadLifecycle::select(
            identity.bead.clone(),
            identity.target.clone(),
            objective.clone(),
            approval,
            selected_id,
        )?;
        for row in rows.into_iter().skip(1) {
            let event = row
                .get("event")
                .and_then(Value::as_str)
                .ok_or_else(|| LedgerError::MalformedExistingRow { line: row.to_string() })?;
            let event_id = row_event_id(&row)?;
            let observed_at_ms = row_observed_at(&row)?;
            match event {
                "dispatch_receipt" => lifecycle.dispatch(DispatchReceipt::new(
                    event_id,
                    identity.bead.clone(),
                    identity.target.clone(),
                    objective.clone(),
                    observed_at_ms,
                )?)?,
                "ungraded" => lifecycle.mark_ungraded(event_id)?,
                "receiver_verified" => lifecycle.verify_receiver(
                    ReceiverEvidence::new(
                        event_id,
                        identity.bead.clone(),
                        identity.target.clone(),
                        objective.clone(),
                        observed_at_ms,
                    )?,
                    EvidencePolicy::new(observed_at_ms, 0),
                )?,
                "grading_started" => {
                    let grader = row_string(&row, "grader_pane")?;
                    lifecycle.start_grading(event_id, grader)?;
                }
                "grade_receipt" => {}
                "graded_pass" | "graded_fix" => {
                    let grader_pane = row_string(&row, "grader_pane")?;
                    let receiver_event_id = EventId::new(row_string(&row, "receiver_event_id")?)?;
                    let result = if event == "graded_pass" {
                        GradeResult::Pass
                    } else {
                        GradeResult::fix(row_string(&row, "named_acceptance")?)?
                    };
                    lifecycle.grade(
                        GradeReceipt {
                            id: event_id,
                            bead: identity.bead.clone(),
                            target: identity.target.clone(),
                            grader_pane,
                            receiver_event_id,
                            result,
                            graded_at_ms: observed_at_ms,
                        },
                        EvidencePolicy::new(observed_at_ms, 0),
                    )?;
                }
                "closed" => lifecycle.close(event_id)?,
                "redispatch_required" => lifecycle.require_redispatch(RedispatchPlan::new(
                    event_id,
                    identity.bead.clone(),
                    identity.target.clone(),
                    row_string(&row, "fix_name")?,
                )?)?,
                other => {
                    return Err(LedgerError::UnsupportedReplay {
                        event: other.to_owned(),
                    })
                }
            }
        }
        Ok(Self {
            path,
            identity,
            objective,
            lifecycle,
        })
    }

    pub fn claim_peer_grading(
        path: impl Into<PathBuf>,
        candidate: ReceiverVerifiedCandidate,
        approval: crate::Approved,
        grader_pane: impl Into<String>,
        evidence: LedgerEvidence,
    ) -> Result<AppendOutcome, LedgerError> {
        let mut ledger = Self::resume(
            path,
            candidate.identity,
            candidate.objective,
            approval,
        )?;
        ledger.start_grading(evidence, grader_pane)
    }

    fn ensure_identity(
        &self,
        bead: &BeadId,
        target: &DispatchTarget,
        objective: Option<&str>,
    ) -> Result<(), LedgerError> {
        if bead != &self.identity.bead {
            return Err(LedgerError::InvalidIdentity { field: "bead" });
        }
        if target != &self.identity.target {
            return Err(LedgerError::InvalidIdentity { field: "target" });
        }
        if let Some(objective) = objective {
            if objective != self.objective {
                return Err(LedgerError::InvalidIdentity { field: "objective" });
            }
        }
        Ok(())
    }

    pub fn dispatch(
        &mut self,
        receipt: DispatchReceipt,
        evidence: LedgerEvidence,
    ) -> Result<AppendOutcome, LedgerError> {
        require_key(&receipt.id, &evidence.id)?;
        self.ensure_identity(&receipt.bead, &receipt.target, Some(&receipt.objective))?;
        if let Some(replayed) = self.preflight_one(
            "dispatch_receipt",
            LifecycleStatus::Dispatched,
            &evidence,
            Map::new(),
        )? {
            return Ok(replayed);
        }
        let backup = self.lifecycle.clone();
        self.lifecycle.dispatch(receipt)?;
        let result = self.append_one(
            "dispatch_receipt",
            LifecycleStatus::Dispatched,
            evidence,
            Map::new(),
        );
        if result.is_err() {
            self.lifecycle = backup;
        }
        result
    }

    pub fn mark_ungraded(
        &mut self,
        evidence: LedgerEvidence,
    ) -> Result<AppendOutcome, LedgerError> {
        self.apply_one("ungraded", LifecycleStatus::Ungraded, evidence, |life, id| {
            life.mark_ungraded(id)
        })
    }

    pub fn verify_receiver(
        &mut self,
        receiver: ReceiverEvidence,
        evidence: LedgerEvidence,
    ) -> Result<AppendOutcome, LedgerError> {
        require_key(&receiver.id, &evidence.id)?;
        self.ensure_identity(&receiver.bead, &receiver.target, Some(&receiver.objective))?;
        let policy = evidence.policy;
        self.apply_one(
            "receiver_verified",
            LifecycleStatus::ReceiverVerified,
            evidence,
            |life, _id| life.verify_receiver(receiver, policy),
        )
    }


    pub fn start_grading(
        &mut self,
        evidence: LedgerEvidence,
        grader_pane: impl Into<String>,
    ) -> Result<AppendOutcome, LedgerError> {
        let grader_pane = grader_pane.into();
        let mut extra = Map::new();
        extra.insert("grader_pane".to_owned(), json!(&grader_pane));
        if let Some(replayed) = self.preflight_one(
            "grading_started",
            LifecycleStatus::Grading,
            &evidence,
            extra.clone(),
        )? {
            return Ok(replayed);
        }
        let backup = self.lifecycle.clone();
        self.lifecycle.start_grading(evidence.id.clone(), grader_pane)?;
        let result = self.append_one("grading_started", LifecycleStatus::Grading, evidence, extra);
        if result.is_err() {
            self.lifecycle = backup;
        }
        result
    }

    pub fn grade(
        &mut self,
        receipt: GradeReceipt,
        evidence: LedgerEvidence,
    ) -> Result<Vec<AppendOutcome>, LedgerError> {
        require_key(&receipt.id, &evidence.id)?;
        self.ensure_identity(&receipt.bead, &receipt.target, None)?;
        let grader_pane = receipt.grader_pane.clone();
        let receiver_event_id = receipt.receiver_event_id.as_str().to_owned();
        let (result_event, result_status, mut result_extra) = match &receipt.result {
            GradeResult::Pass => ("graded_pass", LifecycleStatus::GradedPass, Map::new()),
            GradeResult::Fix { name } => {
                let mut extra = Map::new();
                extra.insert("result".to_owned(), json!("fix"));
                extra.insert("named_acceptance".to_owned(), json!(name));
                ("graded_fix", LifecycleStatus::GradedFix, extra)
            }
        };
        result_extra.insert("grader_pane".to_owned(), json!(&grader_pane));
        result_extra.insert("receiver_event_id".to_owned(), json!(receiver_event_id));
        let receipt_id = EventId::new(format!("{}:receipt", evidence.id.as_str()))?;
        let receipt_evidence = LedgerEvidence::new(
            receipt_id.clone(),
            evidence.observed_at_ms,
            evidence.policy,
            evidence.fields.clone().into_iter(),
        )?;
        let receipt_row = self.row(
            "grade_receipt",
            result_status,
            &receipt_evidence,
            result_extra.clone(),
        );
        let result_row = self.row(result_event, result_status, &evidence, result_extra);
        let rows = vec![
            (receipt_id.as_str().to_owned(), receipt_row),
            (evidence.id.as_str().to_owned(), result_row),
        ];
        if let Some(replayed) = self.preflight_rows(&rows)? {
            return Ok(replayed);
        }
        let backup = self.lifecycle.clone();
        self.lifecycle.grade(receipt, evidence.policy)?;
        let result = self.append_rows(rows);
        if result.is_err() {
            self.lifecycle = backup;
        }
        result
    }

    pub fn close(&mut self, evidence: LedgerEvidence) -> Result<AppendOutcome, LedgerError> {
        self.apply_one("closed", LifecycleStatus::Closed, evidence, |life, id| {
            life.close(id)
        })
    }

    pub fn require_redispatch(
        &mut self,
        plan: RedispatchPlan,
        evidence: LedgerEvidence,
    ) -> Result<AppendOutcome, LedgerError> {
        require_key(&plan.id, &evidence.id)?;
        self.ensure_identity(&plan.bead, &plan.target, None)?;
        let mut extra = Map::new();
        extra.insert("named_acceptance".to_owned(), json!(&plan.fix_name));
        if let Some(replayed) = self.preflight_one(
            "redispatch_required",
            LifecycleStatus::RedispatchRequired,
            &evidence,
            extra.clone(),
        )? {
            return Ok(replayed);
        }
        let backup = self.lifecycle.clone();
        self.lifecycle.require_redispatch(plan)?;
        let result = self.append_one(
            "redispatch_required",
            LifecycleStatus::RedispatchRequired,
            evidence,
            extra,
        );
        if result.is_err() {
            self.lifecycle = backup;
        }
        result
    }

    pub fn block(
        &mut self,
        evidence: LedgerEvidence,
        blocker: BlockerEvidence,
    ) -> Result<AppendOutcome, LedgerError> {
        let mut blocker_row = Map::new();
        blocker_row.insert(
            "blocker".to_owned(),
            json!({
                "kind": blocker.kind().as_str(),
                "reason": blocker.reason().as_str(),
                "escalation_ref": blocker.escalation_ref().as_str(),
                "observed_at_ms": blocker.observed_at_ms(),
            }),
        );
        if let Some(replayed) = self.preflight_one(
            "blocked",
            LifecycleStatus::Blocked,
            &evidence,
            blocker_row.clone(),
        )? {
            return Ok(replayed);
        }
        let backup = self.lifecycle.clone();
        self.lifecycle
            .block(evidence.id.clone(), blocker, evidence.policy)?;
        let result = self.append_one("blocked", LifecycleStatus::Blocked, evidence, blocker_row);
        if result.is_err() {
            self.lifecycle = backup;
        }
        result
    }

    fn apply_one<F>(
        &mut self,
        event: &str,
        status: LifecycleStatus,
        evidence: LedgerEvidence,
        transition: F,
    ) -> Result<AppendOutcome, LedgerError>
    where
        F: FnOnce(&mut BeadLifecycle, EventId) -> Result<(), LifecycleError>,
    {
        if let Some(replayed) = self.preflight_one(event, status, &evidence, Map::new())? {
            return Ok(replayed);
        }
        let backup = self.lifecycle.clone();
        transition(&mut self.lifecycle, evidence.id.clone())?;
        let result = self.append_one(event, status, evidence, Map::new());
        if result.is_err() {
            self.lifecycle = backup;
        }
        result
    }

    fn preflight_one(
        &self,
        event: &str,
        status: LifecycleStatus,
        evidence: &LedgerEvidence,
        extra: Map<String, Value>,
    ) -> Result<Option<AppendOutcome>, LedgerError> {
        let row = self.row(event, status, evidence, extra);
        let rows = vec![(evidence.id.as_str().to_owned(), row)];
        Ok(self.preflight_rows(&rows)?.map(|mut values| values.remove(0)))
    }

    fn append_one(
        &self,
        event: &str,
        status: LifecycleStatus,
        evidence: LedgerEvidence,
        extra: Map<String, Value>,
    ) -> Result<AppendOutcome, LedgerError> {
        let row = self.row(event, status, &evidence, extra);
        self.append_rows(vec![(evidence.id.as_str().to_owned(), row)])
            .map(|mut values| values.remove(0))
    }

    fn row(
        &self,
        event: &str,
        status: LifecycleStatus,
        evidence: &LedgerEvidence,
        extra: Map<String, Value>,
    ) -> Value {
        let mut row = json!({
            "schema": BEAD_LIFECYCLE_LEDGER_SCHEMA,
            "event": event,
            "status": status.as_str(),
            "bead": self.identity.bead.as_str(),
            "repo": &self.identity.repo,
            "session": &self.identity.target.session,
            "pane": &self.identity.target.pane,
            "packet_digest": &self.identity.packet_digest,
            "objective": &self.objective,
            "idempotency_key": evidence.id.as_str(),
            "freshness": {
                "observed_at_ms": evidence.observed_at_ms,
                "now_ms": evidence.policy.now_ms,
                "max_age_ms": evidence.policy.max_age_ms,
                "age_ms": evidence.policy.now_ms - evidence.observed_at_ms,
                "within_window": true,
            },
            "invoker": self.identity.invoker.as_str(),
            "evidence": &evidence.fields,
            "written_at_unix": unix_seconds(),
        });
        if let Some(object) = row.as_object_mut() {
            object.extend(extra);
        }
        row
    }

    fn preflight_rows(
        &self,
        rows: &[(String, Value)],
    ) -> Result<Option<Vec<AppendOutcome>>, LedgerError> {
        if rows.is_empty() {
            return Err(LedgerError::EmptyEvidence);
        }
        let existing = self.existing_rows()?;
        let mut outcomes = Vec::with_capacity(rows.len());
        let mut new_count = 0usize;
        for (key, row) in rows {
            match existing.get(key) {
                Some(old) if same_identity_row(old, row) => outcomes.push(AppendOutcome::Replayed {
                    event: row["event"].as_str().unwrap_or("unknown").to_owned(),
                }),
                Some(_) => {
                    return Err(LedgerError::ConflictingIdempotencyKey { key: key.clone() })
                }
                None => {
                    new_count += 1;
                    outcomes.push(AppendOutcome::Appended {
                        event: row["event"].as_str().unwrap_or("unknown").to_owned(),
                    });
                }
            }
        }
        if new_count == 0 {
            Ok(Some(outcomes))
        } else if new_count != rows.len() {
            Err(LedgerError::PartialIdempotency {
                key: rows
                    .iter()
                    .find(|(key, _)| existing.contains_key(key))
                    .map(|(key, _)| key.clone())
                    .unwrap_or_else(|| "multiple".to_owned()),
            })
        } else {
            Ok(None)
        }
    }

    fn append_rows(&self, rows: Vec<(String, Value)>) -> Result<Vec<AppendOutcome>, LedgerError> {
        if rows.is_empty() {
            return Err(LedgerError::EmptyEvidence);
        }
        if let Some(outcomes) = self.preflight_rows(&rows)? {
            return Ok(outcomes);
        }

        let lines = rows
            .iter()
            .map(|(_, row)| row.to_string())
            .collect::<Vec<_>>();
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| io_error("create_parent", error))?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| io_error("open_append", error))?;
        for line in &lines {
            file.write_all(line.as_bytes())
                .and_then(|_| file.write_all(b"\n"))
                .map_err(|error| io_error("write", error))?;
        }
        file.sync_all().map_err(|error| io_error("fsync_file", error))?;
        fsync_parent(&self.path)?;
        let text = fs::read_to_string(&self.path).map_err(|error| io_error("readback", error))?;
        if lines.iter().any(|line| !text.lines().any(|on_disk| on_disk == line)) {
            return Err(LedgerError::ReadbackMismatch);
        }
        Ok(rows
            .into_iter()
            .map(|(_, row)| AppendOutcome::Appended {
                event: row["event"].as_str().unwrap_or("unknown").to_owned(),
            })
            .collect())
    }

    fn existing_rows(&self) -> Result<BTreeMap<String, Value>, LedgerError> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
            Err(error) => return Err(io_error("read_existing", error)),
        };
        let mut rows = BTreeMap::new();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let row: Value = serde_json::from_str(line).map_err(|_| {
                LedgerError::MalformedExistingRow {
                    line: line.to_owned(),
                }
            })?;
            let Some(key) = row.get("idempotency_key").and_then(Value::as_str) else {
                return Err(LedgerError::MalformedExistingRow {
                    line: line.to_owned(),
                });
            };
            rows.insert(key.to_owned(), row);
        }
        Ok(rows)
    }
}

fn read_rows(path: &Path) -> Result<Vec<Value>, LedgerError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error("read_rows", error)),
    };
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .map_err(|_| LedgerError::MalformedExistingRow { line: line.to_owned() })
        })
        .collect()
}

/// Read current tracker statuses once, then let candidate production apply
/// the status gate. The br list surface is deliberately not involved: it omits closed
/// rows, which would manufacture a false "no closed candidates" result.
fn read_tracker_statuses(path: &Path) -> Result<BTreeMap<String, String>, LedgerError> {
    let text = fs::read_to_string(path).map_err(|error| io_error("read_tracker_statuses", error))?;
    let mut statuses = BTreeMap::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let row: Value = serde_json::from_str(line).map_err(|_| LedgerError::MalformedExistingRow {
            line: line.to_owned(),
        })?;
        statuses.insert(row_string(&row, "id")?, row_string(&row, "status")?);
    }
    Ok(statuses)
}

fn row_string(row: &Value, key: &str) -> Result<String, LedgerError> {
    row.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| LedgerError::MalformedExistingRow { line: row.to_string() })
}

fn row_event_id(row: &Value) -> Result<EventId, LedgerError> {
    Ok(EventId::new(row_string(row, "idempotency_key")?)?)
}

fn row_observed_at(row: &Value) -> Result<u64, LedgerError> {
    row.get("freshness")
        .and_then(|freshness| freshness.get("observed_at_ms"))
        .and_then(Value::as_u64)
        .ok_or_else(|| LedgerError::MalformedExistingRow { line: row.to_string() })
}

fn row_identity(row: &Value) -> Result<LifecycleIdentity, LedgerError> {
    let invoker = InvokerClass::parse(&row_string(row, "invoker")?).ok_or_else(|| {
        LedgerError::MalformedExistingRow { line: row.to_string() }
    })?;
    Ok(LifecycleIdentity::new(
        BeadId::new(row_string(row, "bead")?)?,
        row_string(row, "repo")?,
        DispatchTarget::new(row_string(row, "session")?, row_string(row, "pane")?)?,
        row_string(row, "packet_digest")?,
        invoker,
    )?)
}

fn identity_key(identity: &LifecycleIdentity) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}",
        identity.bead.as_str(),
        identity.repo,
        identity.target.session,
        identity.target.pane,
        identity.packet_digest,
        identity.invoker.as_str(),
    )
}


fn require_key(expected: &EventId, supplied: &EventId) -> Result<(), LedgerError> {
    if expected == supplied {
        Ok(())
    } else {
        Err(LedgerError::InvalidIdentity {
            field: "idempotency_key",
        })
    }
}


fn same_identity_row(left: &Value, right: &Value) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    if let Some(object) = left.as_object_mut() {
        object.remove("written_at_unix");
    }
    if let Some(object) = right.as_object_mut() {
        object.remove("written_at_unix");
    }
    left == right
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn io_error(op: &'static str, error: impl ToString) -> LedgerError {
    LedgerError::Io {
        op,
        detail: error.to_string(),
    }
}

fn fsync_parent(path: &Path) -> Result<(), LedgerError> {
    let parent = path.parent().ok_or_else(|| LedgerError::Io {
        op: "parent",
        detail: "ledger path has no parent".to_owned(),
    })?;
    let directory = File::open(parent).map_err(|error| io_error("open_parent", error))?;
    directory
        .sync_all()
        .map_err(|error| io_error("fsync_parent", error))
}

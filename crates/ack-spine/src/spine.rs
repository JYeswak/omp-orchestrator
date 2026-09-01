#![forbid(unsafe_code)]

//! The executable ack-spine coordinator.
//!
//! Each public step takes an owning [`Cx`] and passes through the canonical
//! [`crate::ledger::step`] primitive. The coordinator records transport,
//! receiver, and tracker facts independently; none is inferred from another.
//! A pending marker is written before the first step and retained until all
//! three authorities agree, so cancellation cannot turn an uncertain send into
//! a retryable clean state.
//!
//! NO-CLAIM: the worker is a separate tmux process. The spine owns the durable
//! ledger and marker, while receiver delivery remains observational and the ack
//! remains tracker read-back.

use crate::ack::{detect_ack, AckVerdict};
use crate::authorities::{AckAuthority, AckEvidence, DeliveryAuthority, TransportAuthority};
use crate::ledger::{self, StepError, StepKind, StepLedger};
use receiver_receipt::ReceiptVerdict;
use asupersync::Cx;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use subprocess_contract::RunError;

/// The durable identity shared by all three authorities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchIntent {
    pub bead_id: String,
    pub pane_id: String,
    pub session: String,
}

impl DispatchIntent {
    pub fn new(bead_id: &str, pane_id: &str, session: &str) -> Self {
        Self {
            bead_id: bead_id.to_owned(),
            pane_id: pane_id.to_owned(),
            session: session.to_owned(),
        }
    }
}

/// Durable intent retained when a dispatch cannot prove its final state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingDispatch {
    pub intent: DispatchIntent,
    pub last_step: Option<String>,
    pub reason: String,
}

/// Errors that prevent the spine from making a positive claim.
#[derive(Debug)]
pub enum SpineError {
    Cancelled,
    EmptyLedger,
    InconsistentLedger(String),
    MissingAuthority(&'static str),
    Io(std::io::Error),
    MarkerParse(serde_json::Error),
    Process(RunError),
}

impl fmt::Display for SpineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("ACK_SPINE_CANCELLED"),
            Self::EmptyLedger => f.write_str("ACK_SPINE_EMPTY_LEDGER"),
            Self::InconsistentLedger(detail) => {
                write!(f, "ACK_SPINE_INCONSISTENT_LEDGER: {detail}")
            }
            Self::MissingAuthority(name) => write!(f, "ACK_SPINE_MISSING_AUTHORITY: {name}"),
            Self::Io(error) => write!(f, "ACK_SPINE_MARKER_IO: {error}"),
            Self::MarkerParse(error) => write!(f, "ACK_SPINE_MARKER_PARSE: {error}"),
            Self::Process(error) => write!(f, "ACK_SPINE_PROCESS: {error}"),
        }
    }
}

impl std::error::Error for SpineError {}

impl From<std::io::Error> for SpineError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for SpineError {
    fn from(error: serde_json::Error) -> Self {
        Self::MarkerParse(error)
    }
}

impl From<RunError> for SpineError {
    fn from(error: RunError) -> Self {
        Self::Process(error)
    }
}

/// Atomic, fail-closed persistence for an uncertain dispatch.
#[derive(Debug, Clone)]
pub struct PendingDispatchStore {
    path: PathBuf,
}

impl PendingDispatchStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn persist(&self, pending: &PendingDispatch) -> Result<(), SpineError> {
        let parent = self.path.parent().filter(|path| !path.as_os_str().is_empty());
        if let Some(parent) = parent {
            fs::create_dir_all(parent)?;
        }
        let temporary = self.path.with_extension(format!(
            "pending-{}.tmp",
            std::process::id()
        ));
        let encoded = serde_json::to_vec_pretty(pending)?;
        fs::write(&temporary, encoded)?;
        fs::rename(temporary, &self.path)?;
        Ok(())
    }

    pub fn load(&self) -> Result<Option<PendingDispatch>, SpineError> {
        match fs::read(&self.path) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn clear(&self) -> Result<(), SpineError> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Existing uncertainty blocks automatic retry until an operator resolves it.
    pub fn retry_allowed(&self) -> bool {
        !self.path.exists()
    }
}

fn step_error(error: StepError) -> SpineError {
    match error {
        StepError::Cancelled { .. } => SpineError::Cancelled,
        StepError::Ledger(error) => SpineError::InconsistentLedger(error.to_string()),
    }
}

/// The one ledger-writing primitive used by every spine step.
pub async fn emit_step(
    cx: &Cx,
    ledger: &mut StepLedger,
    intent: &DispatchIntent,
    kind: StepKind,
    detail: &str,
) -> Result<(), SpineError> {
    ledger::step(
        cx,
        ledger,
        kind,
        &intent.bead_id,
        &intent.pane_id,
        &intent.session,
        detail,
        |_| async {},
    )
    .await
    .map_err(step_error)
}

/// Coordinates the three independent authorities and durable recovery state.
#[derive(Debug)]
pub struct AckSpine {
    intent: DispatchIntent,
    ledger: StepLedger,
    pending: PendingDispatchStore,
    transport: Option<TransportAuthority>,
    delivery: Option<DeliveryAuthority>,
    acknowledgement: Option<AckAuthority>,
}

impl AckSpine {
    pub fn new(intent: DispatchIntent, pending_path: impl Into<PathBuf>) -> Self {
        Self {
            intent,
            ledger: StepLedger::new(),
            pending: PendingDispatchStore::new(pending_path),
            transport: None,
            delivery: None,
            acknowledgement: None,
        }
    }

    pub fn intent(&self) -> &DispatchIntent {
        &self.intent
    }

    pub fn ledger(&self) -> &StepLedger {
        &self.ledger
    }

    pub fn pending_store(&self) -> &PendingDispatchStore {
        &self.pending
    }

    pub fn retry_allowed(&self) -> bool {
        self.pending.retry_allowed()
    }

    pub fn evidence(&self) -> Result<AckEvidence, SpineError> {
        Ok(AckEvidence::new(
            self.transport
                .clone()
                .ok_or(SpineError::MissingAuthority("transport"))?,
            self.delivery
                .clone()
                .ok_or(SpineError::MissingAuthority("delivery"))?,
            self.acknowledgement
                .clone()
                .ok_or(SpineError::MissingAuthority("ack"))?,
        ))
    }

    fn pending_snapshot(&self, reason: &str) -> PendingDispatch {
        PendingDispatch {
            intent: self.intent.clone(),
            last_step: self.ledger.last_kind().map(|kind| kind.as_str().to_owned()),
            reason: reason.to_owned(),
        }
    }

    fn persist_pending(&self, reason: &str) -> Result<(), SpineError> {
        self.pending.persist(&self.pending_snapshot(reason))
    }

    /// Establish the intent before any external send can occur.
    pub async fn begin(&mut self, cx: &Cx) -> Result<(), SpineError> {
        checkpoint(cx)?;
        self.persist_pending("dispatch_started")?;
        emit_step(
            cx,
            &mut self.ledger,
            &self.intent,
            StepKind::BeadSelected,
            "intent recorded",
        )
        .await?;
        self.persist_pending("bead_selected")
    }

    pub async fn packet_rendered(&mut self, cx: &Cx) -> Result<(), SpineError> {
        emit_step(
            cx,
            &mut self.ledger,
            &self.intent,
            StepKind::PacketRendered,
            "packet rendered from bead",
        )
        .await?;
        self.persist_pending("packet_rendered")
    }

    /// Record only transport truth. It never establishes delivery.
    pub async fn record_transport(
        &mut self,
        cx: &Cx,
        authority: TransportAuthority,
    ) -> Result<(), SpineError> {
        let detail = match &authority {
            TransportAuthority::Succeeded { receipt } => format!("transport succeeded: {receipt}"),
            TransportAuthority::Failed { detail } => format!("transport failed: {detail}"),
        };
        emit_step(
            cx,
            &mut self.ledger,
            &self.intent,
            StepKind::PacketSent,
            &detail,
        )
        .await?;
        self.transport = Some(authority);
        self.persist_pending("transport_recorded")
    }

    /// Record only observational receiver truth. It never establishes ack.
    pub async fn record_delivery(
        &mut self,
        cx: &Cx,
        authority: DeliveryAuthority,
    ) -> Result<(), SpineError> {
        let (kind, detail) = match &authority {
            DeliveryAuthority::Observed {
                receipt: ReceiptVerdict::ReceiptConfirmed {
                    pane_id,
                    timer_before_secs,
                    timer_after_secs,
                    stable_content_changed,
                },
            } => (
                StepKind::ReceiverVerified,
                format!(
                    "delivery receipt confirmed pane={pane_id} timer_before_secs={timer_before_secs:?} timer_after_secs={timer_after_secs} stable_content_changed={stable_content_changed}"
                ),
            ),
            DeliveryAuthority::Observed {
                receipt: ReceiptVerdict::NoReceipt { pane_id, reason },
            } => (
                StepKind::ReceiverTimedOut,
                format!("delivery no receipt pane={pane_id} reason={reason}"),
            ),
            DeliveryAuthority::Observed {
                receipt: ReceiptVerdict::Dead { pane_id },
            } => (
                StepKind::ReceiverTimedOut,
                format!("delivery dead pane={pane_id}"),
            ),
            DeliveryAuthority::Observed {
                receipt: ReceiptVerdict::Indeterminate { pane_id, reason },
            } => (
                StepKind::ReceiverTimedOut,
                format!("delivery indeterminate pane={pane_id} reason={reason}"),
            ),
            DeliveryAuthority::NotObserved { reason } => {
                (StepKind::ReceiverTimedOut, format!("delivery absent: {reason}"))
            }
        };
        emit_step(cx, &mut self.ledger, &self.intent, kind, &detail).await?;
        self.delivery = Some(authority);
        self.persist_pending("delivery_recorded")
    }

    /// Record only durable bead-comment read-back truth.
    pub async fn record_ack(
        &mut self,
        cx: &Cx,
        authority: AckAuthority,
    ) -> Result<(), SpineError> {
        let detail = match &authority {
            AckAuthority::ReadBack { comment_id, .. } => {
                format!("ack read back: {comment_id}")
            }
            AckAuthority::NotReadBack { reason } => format!("ack absent: {reason}"),
        };
        emit_step(
            cx,
            &mut self.ledger,
            &self.intent,
            StepKind::AckReadBack,
            &detail,
        )
        .await?;
        self.acknowledgement = Some(authority);
        self.persist_pending("ack_recorded")
    }

    /// Read the durable tracker comment through the cancel-correct process boundary.
    pub async fn read_ack(&mut self, cx: &Cx, marker: &str) -> Result<AckVerdict, SpineError> {
        let verdict = detect_ack(cx, &self.intent.bead_id, marker).await;
        let authority = match &verdict {
            AckVerdict::Confirmed { marker, .. } => AckAuthority::ReadBack {
                bead_id: self.intent.bead_id.clone(),
                comment_id: marker.clone(),
            },
            AckVerdict::Missing { .. } => AckAuthority::NotReadBack {
                reason: "marker absent from br comments list".to_owned(),
            },
            AckVerdict::Unverifiable { detail, .. } => AckAuthority::NotReadBack {
                reason: format!("tracker unreadable: {detail}"),
            },
        };
        self.record_ack(cx, authority).await?;
        Ok(verdict)
    }

    /// Persist uncertainty even when the owning context is already cancelled.
    pub async fn cancel(&mut self, _cx: &Cx, reason: &str) -> Result<(), SpineError> {
        self.persist_pending(reason)?;
        self.ledger
            .assert_step_count()
            .map_err(|error| SpineError::InconsistentLedger(error.to_string()))
    }

    /// Finish only after the three authorities and the ledger are independently valid.
    pub async fn finish(&mut self, cx: &Cx) -> Result<AckEvidence, SpineError> {
        checkpoint(cx)?;
        self.ledger
            .assert_non_empty()
            .map_err(|_| SpineError::EmptyLedger)?;
        self.ledger
            .assert_step_count()
            .map_err(|error| SpineError::InconsistentLedger(error.to_string()))?;
        let evidence = self.evidence()?;
        if evidence.fully_acknowledged() {
            self.pending.clear()?;
        } else {
            self.persist_pending("incomplete_authorities")?;
        }
        Ok(evidence)
    }

    /// Resume information is durable and does not authorize an automatic retry.
    pub fn recoverable_intent(&self) -> Result<Option<PendingDispatch>, SpineError> {
        self.pending.load()
    }
}

fn checkpoint(cx: &Cx) -> Result<(), SpineError> {
    cx.checkpoint().map_err(|_| SpineError::Cancelled)
}

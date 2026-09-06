#![forbid(unsafe_code)]

//! The ACK stage joins transport, receiver, and tracker evidence.
//!
//! Transport output is captured before any later observation can discard it. Receiver
//! delivery is assessed from the caller-supplied pre-send observation. A record action
//! requires the ntm transport, both receiver signals, and a verbatim matching br comment.

use receiver_receipt::{
    assess_receiver_receipt, PostSendObservation, ReceiptReason, ReceiptVerdict,
};
use serde_json::Value;
use std::fmt;
use tick_monitor::Observation;

/// Maximum number of retry actions for one dispatch attempt sequence.
pub const MAX_RETRY_ATTEMPTS: u32 = 3;

/// Transport used to submit a packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    /// The only transport with a retained per-target JSON receipt.
    NtmRobotSend,
    /// Codex fallback: tmux literal input with no equivalent measured receipt.
    TmuxSendKeysLiteral,
}

impl TransportKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::NtmRobotSend => "ntm_robot_send",
            Self::TmuxSendKeysLiteral => "tmux_send_keys_literal",
        }
    }

    /// Only ntm has the measured transport contract needed for a receipt claim.
    pub const fn supports_delivery_claim(self) -> bool {
        matches!(self, Self::NtmRobotSend)
    }
}

/// A complete ntm per-target transport receipt, retained verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NtmRobotSendReceipt {
    pub raw_json: String,
    pub targets: Vec<String>,
    pub successful: Vec<String>,
    pub failed: Vec<String>,
    pub blocked: bool,
}

/// A retained measurement for the unproven Codex tmux fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmuxSendKeysMeasurement {
    pub raw_json: String,
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

/// Transport evidence retained at the instant the send completes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportReceipt {
    NtmRobotSend(NtmRobotSendReceipt),
    TmuxSendKeysLiteral(TmuxSendKeysMeasurement),
}

/// Why a transport receipt could not be captured as typed evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportReceiptError {
    InvalidUtf8,
    InvalidJson(String),
    NotAnObject,
    MissingField(&'static str),
    WrongFieldType(&'static str),
}

impl fmt::Display for TransportReceiptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUtf8 => f.write_str("transport stdout is not UTF-8"),
            Self::InvalidJson(error) => write!(f, "transport stdout is invalid JSON: {error}"),
            Self::NotAnObject => f.write_str("transport receipt is not a JSON object"),
            Self::MissingField(field) => write!(f, "transport receipt missing field {field}"),
            Self::WrongFieldType(field) => {
                write!(f, "transport receipt field {field} has the wrong type")
            }
        }
    }
}

fn string_array(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<Vec<String>, TransportReceiptError> {
    let values = object
        .get(field)
        .ok_or(TransportReceiptError::MissingField(field))?
        .as_array()
        .ok_or(TransportReceiptError::WrongFieldType(field))?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or(TransportReceiptError::WrongFieldType(field))
        })
        .collect()
}

impl TransportReceipt {
    /// Parse and retain the complete stdout from `ntm --robot-send`.
    pub fn capture_ntm(stdout: &[u8]) -> Result<Self, TransportReceiptError> {
        let raw_json =
            String::from_utf8(stdout.to_vec()).map_err(|_| TransportReceiptError::InvalidUtf8)?;
        let value: Value = serde_json::from_slice(stdout)
            .map_err(|error| TransportReceiptError::InvalidJson(error.to_string()))?;
        let object = value
            .as_object()
            .ok_or(TransportReceiptError::NotAnObject)?;
        let blocked = object
            .get("blocked")
            .ok_or(TransportReceiptError::MissingField("blocked"))?
            .as_bool()
            .ok_or(TransportReceiptError::WrongFieldType("blocked"))?;
        Ok(Self::NtmRobotSend(NtmRobotSendReceipt {
            raw_json,
            targets: string_array(object, "targets")?,
            successful: string_array(object, "successful")?,
            failed: string_array(object, "failed")?,
            blocked,
        }))
    }

    /// Retain the Codex fallback measurement without upgrading it to delivery proof.
    pub fn capture_codex(
        command: impl Into<String>,
        stdout: &[u8],
        stderr: &[u8],
        exit_code: Option<i32>,
    ) -> Self {
        let command = command.into();
        let stdout = String::from_utf8_lossy(stdout).into_owned();
        let stderr = String::from_utf8_lossy(stderr).into_owned();
        let raw_json = serde_json::json!({
            "transport": TransportKind::TmuxSendKeysLiteral.label(),
            "command": command,
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": exit_code,
        })
        .to_string();
        Self::TmuxSendKeysLiteral(TmuxSendKeysMeasurement {
            raw_json,
            command,
            stdout,
            stderr,
            exit_code,
        })
    }

    pub const fn kind(&self) -> TransportKind {
        match self {
            Self::NtmRobotSend(_) => TransportKind::NtmRobotSend,
            Self::TmuxSendKeysLiteral(_) => TransportKind::TmuxSendKeysLiteral,
        }
    }

    pub fn raw_json(&self) -> &str {
        match self {
            Self::NtmRobotSend(receipt) => &receipt.raw_json,
            Self::TmuxSendKeysLiteral(measurement) => &measurement.raw_json,
        }
    }

    pub const fn supports_delivery_claim(&self) -> bool {
        self.kind().supports_delivery_claim()
    }
}

/// One tracker comment, text verbatim, timestamp from the tracker row.
///
/// `created_at` is the comment's own time, never the clock at read-back.
/// Missing or unparseable time is `None` and fails closed under recency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckComment {
    pub text: String,
    pub created_at: Option<u64>,
}

/// Read-back of the tracker comments; each text is preserved verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckReadback {
    pub bead_id: String,
    pub pane_id: String,
    pub comments: Vec<AckComment>,
    /// Unix seconds from the pending-dispatch MARKER, never wall clock.
    /// `None` disables recency (the mutation that lets a stale ACK pass).
    pub dispatch_issued_at: Option<u64>,
}

/// Why the authoritative comment read-back could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AckReadbackError {
    InvalidJson(String),
    NotAnArray,
    MissingText(usize),
    WrongTextType(usize),
    /// The read-back contained zero ACK rows; an empty census is an ERROR.
    EmptyAckCensus,
}

impl fmt::Display for AckReadbackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(error) => write!(f, "comments JSON is invalid: {error}"),
            Self::NotAnArray => f.write_str("comments JSON is not an array"),
            Self::MissingText(index) => write!(f, "comment row {index} has no text"),
            Self::WrongTextType(index) => write!(f, "comment row {index} text is not a string"),
            Self::EmptyAckCensus => f.write_str("ACK_CENSUS_EMPTY"),
        }
    }
}

fn ack_token(bead_id: &str) -> &str {
    bead_id.rsplit('-').next().unwrap_or(bead_id)
}

fn ack_prefix(bead_id: &str, pane_id: &str) -> String {
    format!("ACK {} on {pane_id} -- ", ack_token(bead_id))
}

/// Parse a `br comments` `created_at` cell. Unix number or RFC3339 `…Z`.
///
/// Sourced from the tracker row, never from the process clock.
pub fn parse_comment_created_at(value: &Value) -> Option<u64> {
    if let Some(n) = value.as_u64() {
        return Some(n);
    }
    let s = value.as_str()?;
    parse_rfc3339_z(s)
}

fn parse_rfc3339_z(s: &str) -> Option<u64> {
    let s = s.strip_suffix('Z')?;
    let (date, time) = s.split_once('T')?;
    let mut date = date.split('-');
    let y: i32 = date.next()?.parse().ok()?;
    let m: u32 = date.next()?.parse().ok()?;
    let d: u32 = date.next()?.parse().ok()?;
    if date.next().is_some() {
        return None;
    }
    let time = time.split('.').next()?;
    let mut time = time.split(':');
    let hh: u32 = time.next()?.parse().ok()?;
    let mm: u32 = time.next()?.parse().ok()?;
    let ss: u32 = time.next()?.parse().ok()?;
    if time.next().is_some() {
        return None;
    }
    unix_from_civil(y, m, d, hh, mm, ss)
}

fn unix_from_civil(y: i32, m: u32, d: u32, hh: u32, mm: u32, ss: u32) -> Option<u64> {
    if !(1..=12).contains(&m) || d == 0 || d > 31 || hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    let mut y = y;
    if m <= 2 {
        y -= 1;
    }
    let era = y.div_euclid(400);
    let yoe = (y - era * 400) as u32;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era as i64 * 146097 + doe as i64 - 719468;
    let secs = days
        .checked_mul(86400)?
        .checked_add(i64::from(hh) * 3600)?
        .checked_add(i64::from(mm) * 60)?
        .checked_add(i64::from(ss))?;
    u64::try_from(secs).ok()
}

/// Render the single ACK instruction shared by the parser and receiver packets.
///
/// The receiver resolves both the Agent Mail name and `$TMUX_PANE`; the sender
/// supplies neither value. The output is instruction text, not an executed command.
#[must_use]
pub fn ack_instruction(bead_id: &str) -> String {
    format!(
        "br comments add {bead_id} --actor \"$(agent name you resolve yourself)\" \"ACK {} on $TMUX_PANE -- agent=<same name> title=$(tmux display-message -p -t \"$TMUX_PANE\" '#{{pane_title}}')\"",
        ack_token(bead_id)
    )
}

/// What the ACK parser learned about one bead/pane read-back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AckReadbackVerdict {
    /// A strict ACK line matched the expected bead and pane.
    Matched { comment: String },
    /// No parseable ACK line was present for this bead.
    Missing,
    /// An ACK for this bead named a different pane than the dispatch target.
    AckPaneMismatch { expected: String, got: String },
}

impl AckReadbackVerdict {
    /// A stable label for logs and refusal routing.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Matched { .. } => "ACK_MATCHED",
            Self::Missing => "ACK_MISSING",
            Self::AckPaneMismatch { .. } => "ACK_PANE_MISMATCH",
        }
    }
}

impl fmt::Display for AckReadbackVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Matched { comment } => write!(f, "ACK_MATCHED comment={comment}"),
            Self::Missing => f.write_str("ACK_MISSING"),
            Self::AckPaneMismatch { expected, got } => {
                write!(f, "ACK_PANE_MISMATCH expected={expected} got={got}")
            }
        }
    }
}
impl AckReadback {
    /// Parse `br comments list <id> --json` without trimming comment text.
    pub fn from_comments_json(
        bead_id: impl Into<String>,
        pane_id: impl Into<String>,
        bytes: &[u8],
    ) -> Result<Self, AckReadbackError> {
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|error| AckReadbackError::InvalidJson(error.to_string()))?;
        let rows = value.as_array().ok_or(AckReadbackError::NotAnArray)?;
        if rows.is_empty() {
            return Err(AckReadbackError::EmptyAckCensus);
        }
        let mut comments = Vec::with_capacity(rows.len());
        for (index, row) in rows.iter().enumerate() {
            let text = row
                .get("text")
                .ok_or(AckReadbackError::MissingText(index))?
                .as_str()
                .ok_or(AckReadbackError::WrongTextType(index))?;
            comments.push(AckComment {
                text: text.to_owned(),
                created_at: row.get("created_at").and_then(parse_comment_created_at),
            });
        }
        Ok(Self {
            bead_id: bead_id.into(),
            pane_id: pane_id.into(),
            comments,
            dispatch_issued_at: None,
        })
    }

    /// Bind this read-back to a dispatch. `issued_at` is the MARKER field.
    #[must_use]
    pub fn with_dispatch_issued_at(mut self, issued_at: u64) -> Self {
        self.dispatch_issued_at = Some(issued_at);
        self
    }

    fn is_fresh(&self, created_at: Option<u64>) -> bool {
        match self.dispatch_issued_at {
            None => true,
            Some(issued) => created_at.map(|t| t > issued).unwrap_or(false),
        }
    }

    /// Return the exact matching comment body, if the required ACK exists.
    pub fn matching_comment(&self) -> Option<&str> {
        self.matching_comment_for(&self.bead_id, &self.pane_id)
    }

    /// Classify a read-back ACK against the dispatched bead and pane.
    ///
    /// `session_pane_ids` are the tmux pane ids that belong to this dispatch
    /// session. An ACK naming a pane outside that set is ignored (ABSENT), not
    /// `ACK_PANE_MISMATCH`. A mismatch *inside* the set stays a typed refusal.
    /// Empty `session_pane_ids` means only `pane_id` is in-session.
    ///
    /// An ACK older than `dispatch_issued_at` is ABSENT, not a mismatch.
    /// Recency is skipped when `dispatch_issued_at` is `None`.
    pub fn match_verdict_for(&self, bead_id: &str, pane_id: &str) -> AckReadbackVerdict {
        self.match_verdict_in_session(bead_id, pane_id, &[])
    }

    pub fn match_verdict_in_session(
        &self,
        bead_id: &str,
        pane_id: &str,
        session_pane_ids: &[String],
    ) -> AckReadbackVerdict {
        if self.bead_id != bead_id {
            return AckReadbackVerdict::Missing;
        }
        let prefix = format!("ACK {} on ", ack_token(bead_id));
        for comment in &self.comments {
            let Some(rest) = comment.text.strip_prefix(&prefix) else {
                continue;
            };
            let Some((got, _)) = rest.split_once(" -- ") else {
                continue;
            };
            if !self.is_fresh(comment.created_at) {
                continue;
            }
            if got == pane_id {
                return AckReadbackVerdict::Matched {
                    comment: comment.text.clone(),
                };
            }
            let in_session = session_pane_ids.iter().any(|id| id == got);
            if in_session {
                return AckReadbackVerdict::AckPaneMismatch {
                    expected: pane_id.to_owned(),
                    got: got.to_owned(),
                };
            }
        }
        AckReadbackVerdict::Missing
    }

    /// Match only a read-back bound to the stage's bead and pane.
    pub fn matching_comment_for(&self, bead_id: &str, pane_id: &str) -> Option<&str> {
        if self.bead_id != bead_id || self.pane_id != pane_id {
            return None;
        }
        let prefix = ack_prefix(bead_id, pane_id);
        self.comments
            .iter()
            .find(|comment| comment.text.starts_with(&prefix) && self.is_fresh(comment.created_at))
            .map(|comment| comment.text.as_str())
    }
}

/// Input captured around exactly one transport attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckStageInput {
    pub bead_id: String,
    pub pane_id: String,
    pub transport: TransportReceipt,
    /// Captured before transport execution; never reconstructed from post-state.
    pub pre_send: Observation,
    pub post_send: PostSendObservation,
    pub ack: AckReadback,
    pub attempts_so_far: u32,
    /// Tmux pane ids belonging to this dispatch session. Empty: only `pane_id`
    /// is in-session, so a foreign ACK is ABSENT not ACK_PANE_MISMATCH.
    pub session_pane_ids: Vec<String>,
}

/// One typed action plus all evidence that led to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckStageResult {
    pub action: AckAction,
    pub delivery: ReceiptVerdict,
    pub transport: TransportReceipt,
    pub ack_comment: Option<String>,
    pub ack_verdict: AckReadbackVerdict,
}

impl AckStageResult {
    pub fn is_confirmed(&self) -> bool {
        matches!(self.action, AckAction::RecordReceipt { .. })
            && self.ack_comment.is_some()
            && matches!(self.delivery, ReceiptVerdict::ReceiptConfirmed { .. } | ReceiptVerdict::AckConfirmed { .. })
            && self.transport.supports_delivery_claim()
    }
}

/// Assess transport, independent receiver signals, and authoritative ACK read-back.
///
/// A successful sender field is retained as evidence but is never consulted for delivery.
/// Codex's tmux fallback is always INDETERMINATE, even if the receiver classifier sees a
/// timer reset and hash change; that combination is not a proven uniform transport.
pub fn assess(input: &AckStageInput) -> AckStageResult {
    let receiver =
        assess_receiver_receipt(&input.pane_id, &input.pre_send, input.post_send.clone());
    let delivery = match (&input.transport, receiver) {
        (transport, ReceiptVerdict::ReceiptConfirmed { pane_id, .. })
            if !transport.supports_delivery_claim() =>
        {
            ReceiptVerdict::Indeterminate {
                pane_id,
                reason: ReceiptReason::UnprovenTransport {
                    transport: transport.kind().label(),
                },
            }
        }
        (_, receiver) => receiver,
    };
    let ack_verdict =
        input
            .ack
            .match_verdict_in_session(&input.bead_id, &input.pane_id, &input.session_pane_ids);
    let ack_comment = match &ack_verdict {
        AckReadbackVerdict::Matched { comment } => Some(comment.clone()),
        AckReadbackVerdict::Missing | AckReadbackVerdict::AckPaneMismatch { .. } => None,
    };
    let delivery = if let AckReadbackVerdict::AckPaneMismatch { expected, got } = &ack_verdict {
        ReceiptVerdict::Indeterminate {
            pane_id: expected.clone(),
            reason: ReceiptReason::AckPaneMismatch {
                expected: expected.clone(),
                got: got.clone(),
            },
        }
    } else if let Some(comment) = &ack_comment {
        if input.transport.supports_delivery_claim() {
            // A fresh, prefix-correct tracker ACK is durable receiver evidence. It remains
            // true after the pane starts work, so do not route this arm through liveness.
            ReceiptVerdict::AckConfirmed {
                pane_id: input.pane_id.clone(),
                comment: comment.clone(),
            }
        } else {
            // The tmux literal path cannot make a uniform delivery claim, even with a
            // comment; its transport heuristic remains restrictive by construction.
            delivery
        }
    } else if matches!(&delivery, ReceiptVerdict::ReceiptConfirmed { .. }) {
        ReceiptVerdict::Indeterminate {
            pane_id: input.pane_id.clone(),
            reason: ReceiptReason::AckReadbackMissing,
        }
    } else {
        delivery
    };
    let action = decide(&delivery, input.attempts_so_far);
    AckStageResult {
        action,
        delivery,
        transport: input.transport.clone(),
        ack_comment,
        ack_verdict,
    }
}

/// The one action selected from one receiver verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AckAction {
    /// Record that the receiver-side receipt was confirmed.
    RecordReceipt { pane_id: String },
    /// Retry a packet that has no receiver evidence, while recording its bounded ordinal.
    Retry {
        pane_id: String,
        attempt: u32,
        max_attempts: u32,
    },
    /// Stop sending and unstick a pane whose queued packet was never submitted.
    Unstick {
        pane_id: String,
        reason: ReceiptReason,
    },
    /// Stop sending and await a human response to the dialog.
    AwaitHuman {
        pane_id: String,
        reason: ReceiptReason,
    },
    /// Stop scheduling work for a pane absent from a non-empty census.
    AbandonDeadPane { pane_id: String },
    /// The bounded retry budget is exhausted; no resend is authorized.
    RetryExhausted { pane_id: String, attempts: u32 },
}

impl AckAction {
    /// Stable machine-readable action label.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::RecordReceipt { .. } => "RECORD_RECEIPT",
            Self::Retry { .. } => "RETRY",
            Self::Unstick { .. } => "UNSTICK",
            Self::AwaitHuman { .. } => "AWAIT_HUMAN",
            Self::AbandonDeadPane { .. } => "ABANDON_DEAD_PANE",
            Self::RetryExhausted { .. } => "RETRY_EXHAUSTED",
        }
    }

    /// Whether this action injects another packet. Only bounded Retry does.
    pub const fn is_retry(&self) -> bool {
        matches!(self, Self::Retry { .. })
    }
}

/// Consume exactly one receiver verdict and choose exactly one follow-up action.
///
/// `attempts_so_far` is durable caller state. A `NO_RECEIPT` can retry only while it is
/// below [`MAX_RETRY_ATTEMPTS`]. `WEDGED_UNSUBMITTED` is deliberately checked first and
/// never becomes a retry, even when its retry budget is unused. `INDETERMINATE` always
/// waits rather than burying a human dialog with another packet.
pub fn decide(verdict: &ReceiptVerdict, attempts_so_far: u32) -> AckAction {
    match verdict {
        ReceiptVerdict::ReceiptConfirmed { pane_id, .. } | ReceiptVerdict::AckConfirmed { pane_id, .. } => AckAction::RecordReceipt {
            pane_id: pane_id.clone(),
        },
        ReceiptVerdict::Dead { pane_id } => AckAction::AbandonDeadPane {
            pane_id: pane_id.clone(),
        },
        ReceiptVerdict::Indeterminate { pane_id, reason } => AckAction::AwaitHuman {
            pane_id: pane_id.clone(),
            reason: reason.clone(),
        },
        ReceiptVerdict::NoReceipt { pane_id, reason } => {
            if matches!(reason, ReceiptReason::WedgedUnsubmitted) {
                return AckAction::Unstick {
                    pane_id: pane_id.clone(),
                    reason: reason.clone(),
                };
            }
            if attempts_so_far < MAX_RETRY_ATTEMPTS {
                AckAction::Retry {
                    pane_id: pane_id.clone(),
                    attempt: attempts_so_far + 1,
                    max_attempts: MAX_RETRY_ATTEMPTS,
                }
            } else {
                AckAction::RetryExhausted {
                    pane_id: pane_id.clone(),
                    attempts: attempts_so_far,
                }
            }
        }
    }
}

/// **What the supervisor should PUBLISH about one dispatch.**
///
/// # The measured collapse, `zlyy`
///
/// `send_and_verify` returns `Result<AckStageResult, String>` and the ledger renders every
/// `Err` as `status=DISPATCH_FAILED` (`omp-orchestrator/src/main.rs:3293`). Two of the three
/// non-delivered outcomes are NOT failures:
///
/// - `ACK_STAGE_INDETERMINATE` — the codex/tmux transport is indeterminate BY CONSTRUCTION
///   (see `assess` above: a confirmed receiver signal on a transport that cannot support a
///   delivery claim is downgraded to `Indeterminate`). The packet very likely arrived.
/// - `ACK_STAGE_RETRY_BLOCKED` — the 30s window elapsed. `main.rs`'s own comment at that
///   site says *"the ack is late, not absent"*, and sets `owes_human=false`.
///
/// Measured on `io3h -> %1414`, 2026-09-02 23:55:08Z: `DISPATCH_FAILED
/// detail=ACK_STAGE_INDETERMINATE reason=unproven_transport`, four seconds before the
/// worker's own `ACK io3h on %1414 -- starting...` landed in the tracker. The word was
/// false, and it is the loop's public status word.
///
/// # This type invents NOTHING
///
/// Every variant is a projection of fields `AckStageResult` already carries —
/// `is_confirmed()`, `transport.supports_delivery_claim()`, `action`, `ack_verdict`. It
/// lives HERE, in the crate that owns the ack vocabulary, rather than in a new crate,
/// because `omp-types`' header names the alternative as the measured defect: *"Six
/// independent `Verdict` types … none composable, none countable."* A seventh would be the
/// same mistake with a better excuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchVerdict {
    /// Delivery is PROVEN: a matched ACK on a transport that can carry a delivery claim.
    Delivered(AckStageResult),
    /// The transport cannot prove delivery. Not a failure — an unprovable success.
    Indeterminate {
        /// Why proof is unavailable, from `ReceiptReason`.
        reason: String,
        /// Which transport could not carry the claim.
        transport: TransportKind,
    },
    /// The ACK window elapsed without a matched read-back. Late, not absent.
    AckPending {
        /// How long the window was.
        after_secs: u64,
        /// What distinguishes this pane's state, for the next reader.
        discriminator: String,
    },
    /// The send itself was refused or never ran: `TIMEOUT program=tmux`, a spawn error.
    Failed(String),
}

impl DispatchVerdict {
    /// The status word the ledger publishes.
    ///
    /// `DISPATCH_FAILED` appears for `Failed` and NOWHERE ELSE. That single property is
    /// this type's reason to exist; a test asserts it across every variant.
    pub const fn status_word(&self) -> &'static str {
        match self {
            Self::Delivered(_) => "DISPATCH_DELIVERED",
            Self::Indeterminate { .. } => "DISPATCH_INDETERMINATE",
            Self::AckPending { .. } => "ACK_PENDING",
            Self::Failed(_) => "DISPATCH_FAILED",
        }
    }

    /// Is this a refused send — the only case a human should read as a failure?
    pub const fn is_failure(&self) -> bool {
        matches!(self, Self::Failed(_))
    }

    /// May the supervisor treat the packet as having reached the pane?
    ///
    /// TRUE for `Delivered` and `AckPending` (the pane holds it; the ACK is late) and
    /// FALSE for `Indeterminate` — which is the conservative direction, because an
    /// unprovable transport must not authorize a delivery claim. Deliberately NOT the
    /// negation of [`Self::is_failure`]: three states, not two.
    pub const fn packet_is_with_the_receiver(&self) -> bool {
        matches!(self, Self::Delivered(_) | Self::AckPending { .. })
    }

    /// The detail field, for the ledger row.
    pub fn detail(&self) -> String {
        match self {
            Self::Delivered(result) => format!(
                "action={} verdict={} transport={}",
                result.action.label(),
                result.delivery.label(),
                result.transport.kind().label()
            ),
            Self::Indeterminate { reason, transport } => {
                format!("reason={reason} transport={}", transport.label())
            }
            Self::AckPending {
                after_secs,
                discriminator,
            } => format!("after={after_secs}s {discriminator}"),
            Self::Failed(detail) => detail.clone(),
        }
    }
}

/// Project one assessed stage into the verdict the ledger should publish.
///
/// # Order of the arms is the whole contract
///
/// 1. `is_confirmed()` — ack-stage's OWN definition of proven delivery. Consulted first so
///    this function can never be more generous than the crate it projects.
/// 2. A transport that cannot support a delivery claim is INDETERMINATE, whatever the
///    receiver classifier saw. This mirrors `assess`'s downgrade rather than re-deriving
///    it; the two must not be able to disagree.
/// 3. A retry that is out of budget after a bounded window is `AckPending` — late, not
///    absent — and carries the window so a reader can see what was waited for.
/// 4. Everything left is a genuine no-receipt outcome.
///
/// A transport-level refusal never reaches here: the caller has no `AckStageResult` to
/// project, and constructs [`DispatchVerdict::Failed`] directly.
pub fn classify_dispatch(result: &AckStageResult, window_secs: u64) -> DispatchVerdict {
    if result.is_confirmed() {
        return DispatchVerdict::Delivered(result.clone());
    }
    let transport = result.transport.kind();
    if !result.transport.supports_delivery_claim() {
        return DispatchVerdict::Indeterminate {
            reason: result.delivery.label().to_owned(),
            transport,
        };
    }
    match &result.action {
        AckAction::Retry { attempt, .. } => DispatchVerdict::AckPending {
            after_secs: window_secs,
            discriminator: format!("attempt={attempt}"),
        },
        AckAction::RetryExhausted { attempts, .. } => DispatchVerdict::AckPending {
            after_secs: window_secs,
            discriminator: format!("attempts={attempts}"),
        },
        AckAction::RecordReceipt { .. } => DispatchVerdict::Indeterminate {
            reason: result.delivery.label().to_owned(),
            transport,
        },
        AckAction::Unstick { reason, .. } | AckAction::AwaitHuman { reason, .. } => {
            DispatchVerdict::Failed(format!(
                "action={} reason={reason:?}",
                result.action.label()
            ))
        }
        AckAction::AbandonDeadPane { pane_id } => {
            DispatchVerdict::Failed(format!("action=abandon_dead_pane pane={pane_id}"))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn confirmed() -> ReceiptVerdict {
        ReceiptVerdict::ReceiptConfirmed {
            pane_id: "%p".into(),
            timer_before_secs: None,
            timer_after_secs: 1,
            stable_content_changed: true,
        }
    }

    fn no_receipt(reason: ReceiptReason) -> ReceiptVerdict {
        ReceiptVerdict::NoReceipt {
            pane_id: "%p".into(),
            reason,
        }
    }

    fn ntm() -> TransportReceipt {
        TransportReceipt::capture_ntm(
            br#"{"targets":["5"],"successful":["5"],"failed":[],"blocked":false}"#,
        )
        .unwrap()
    }

    fn ack(comments: &[&str]) -> AckReadback {
        AckReadback {
            bead_id: "omp-orchestrator-ack-stage-qhl".into(),
            pane_id: "%1413".into(),
            comments: comments
                .iter()
                .map(|comment| AckComment {
                    text: (*comment).into(),
                    created_at: None,
                })
                .collect(),
            dispatch_issued_at: None,
        }
    }

    fn ack_at(pane_id: &str, comments: &[(&str, u64)]) -> AckReadback {
        AckReadback {
            bead_id: "omp-orchestrator-ack-stage-qhl".into(),
            pane_id: pane_id.into(),
            comments: comments
                .iter()
                .map(|(comment, at)| AckComment {
                    text: (*comment).into(),
                    created_at: Some(*at),
                })
                .collect(),
            dispatch_issued_at: None,
        }
    }

    fn identity(at: u64) -> receiver_receipt::ObservationIdentity {
        receiver_receipt::ObservationIdentity {
            epoch: "ack-stage-test".into(),
            sequence: at,
            changed_at: at.to_string(),
        }
    }

    fn idle() -> Observation {
        receiver_receipt::observe_capture(
            "%1413",
            "prompt\nπ . GPT-5.6 . /tmp/receiver",
            100,
            identity(100),
        )
    }

    fn working() -> Observation {
        receiver_receipt::observe_capture(
            "%1413",
            "accepted packet\n⠙ 1s . GPT-5.6 . /tmp/receiver",
            101,
            identity(101),
        )
    }
    #[test]
    fn ntm_capture_retains_full_json_and_durable_ack_confirms() {
        let raw = br#"{"targets":["5"],"successful":["5"],"failed":[],"blocked":false}"#;
        let receipt = TransportReceipt::capture_ntm(raw).unwrap();
        let TransportReceipt::NtmRobotSend(receipt) = receipt else {
            panic!("expected ntm receipt");
        };
        assert_eq!(receipt.raw_json, String::from_utf8(raw.to_vec()).unwrap());
        assert_eq!(receipt.targets, vec!["5"]);
        assert_eq!(receipt.successful, vec!["5"]);
        assert!(receipt.failed.is_empty());
        assert!(!receipt.blocked);

        let result = assess(&AckStageInput {
            bead_id: "omp-orchestrator-ack-stage-qhl".into(),
            pane_id: "%1413".into(),
            transport: TransportReceipt::NtmRobotSend(receipt),
            pre_send: idle(),
            post_send: PostSendObservation::Present(idle()),
            ack: ack(&["ACK qhl on %1413 -- first step"]),
            attempts_so_far: 0,
            session_pane_ids: Vec::new(),
        });
        assert_eq!(result.action.label(), "RECORD_RECEIPT");
        assert!(result.is_confirmed());
    }

    #[test]
    fn authoritative_ack_is_verbatim_and_uses_short_bead_token() {
        let readback = AckReadback::from_comments_json(
            "omp-orchestrator-ack-stage-qhl",
            "%1413",
            br#"[{"id":1,"text":"ACK qhl on %1413 -- first step"}]"#,
        )
        .unwrap();
        assert_eq!(
            readback.matching_comment(),
            Some("ACK qhl on %1413 -- first step")
        );
    }

    #[test]
    fn mismatched_ack_readback_cannot_confirm_delivery() {
        let result = assess(&AckStageInput {
            bead_id: "different-bead".into(),
            pane_id: "%1413".into(),
            transport: ntm(),
            pre_send: idle(),
            post_send: PostSendObservation::Present(working()),
            ack: ack(&["ACK qhl on %1413 -- first step"]),
            attempts_so_far: 0,
            session_pane_ids: Vec::new(),
        });
        assert_eq!(result.action.label(), "AWAIT_HUMAN");
        assert!(matches!(
            result.delivery,
            ReceiptVerdict::Indeterminate {
                reason: ReceiptReason::AckReadbackMissing,
                ..
            }
        ));
    }

    #[test]
    fn ntm_delivery_requires_receiver_signals_and_ack_readback() {
        let result = assess(&AckStageInput {
            bead_id: "omp-orchestrator-ack-stage-qhl".into(),
            pane_id: "%1413".into(),
            transport: ntm(),
            pre_send: idle(),
            post_send: PostSendObservation::Present(working()),
            ack: ack(&["ACK qhl on %1413 -- first step"]),
            attempts_so_far: 0,
            session_pane_ids: Vec::new(),
        });
        assert_eq!(result.action.label(), "RECORD_RECEIPT");
        assert!(result.is_confirmed());
        assert_eq!(
            result.ack_comment.as_deref(),
            Some("ACK qhl on %1413 -- first step")
        );
    }

    #[test]
    fn missing_ack_is_indeterminate_and_never_retries() {
        let result = assess(&AckStageInput {
            bead_id: "omp-orchestrator-ack-stage-qhl".into(),
            pane_id: "%1413".into(),
            transport: ntm(),
            pre_send: idle(),
            post_send: PostSendObservation::Present(working()),
            ack: ack(&[]),
            attempts_so_far: 0,
            session_pane_ids: Vec::new(),
        });
        assert_eq!(result.action.label(), "AWAIT_HUMAN");
        assert!(!result.action.is_retry());
        assert!(matches!(
            result.delivery,
            ReceiptVerdict::Indeterminate {
                reason: ReceiptReason::AckReadbackMissing,
                ..
            }
        ));
    }

    #[test]
    fn codex_delivery_candidate_is_indeterminate_with_measurement() {
        let result = assess(&AckStageInput {
            bead_id: "omp-orchestrator-ack-stage-qhl".into(),
            pane_id: "%1413".into(),
            transport: TransportReceipt::capture_codex(
                "tmux send-keys -l; tmux send-keys Enter",
                b"",
                b"",
                Some(0),
            ),
            pre_send: idle(),
            post_send: PostSendObservation::Present(working()),
            ack: ack(&["ACK qhl on %1413 -- first step"]),
            attempts_so_far: 0,
            session_pane_ids: Vec::new(),
        });
        assert_eq!(result.action.label(), "AWAIT_HUMAN");
        assert!(!result.action.is_retry());
        assert!(matches!(
            result.delivery,
            ReceiptVerdict::Indeterminate {
                reason: ReceiptReason::UnprovenTransport {
                    transport: "tmux_send_keys_literal"
                },
                ..
            }
        ));
    }

    #[test]
    fn each_verdict_has_one_typed_action() {
        assert_eq!(decide(&confirmed(), 0).label(), "RECORD_RECEIPT");
        assert_eq!(
            decide(&no_receipt(ReceiptReason::IdleUnchanged), 0).label(),
            "RETRY"
        );
        assert_eq!(
            decide(&no_receipt(ReceiptReason::WedgedUnsubmitted), 0).label(),
            "UNSTICK"
        );
        assert_eq!(
            decide(
                &ReceiptVerdict::Indeterminate {
                    pane_id: "%p".into(),
                    reason: ReceiptReason::DialogOpen,
                },
                0
            )
            .label(),
            "AWAIT_HUMAN"
        );
        assert_eq!(
            decide(
                &ReceiptVerdict::Dead {
                    pane_id: "%p".into()
                },
                0
            )
            .label(),
            "ABANDON_DEAD_PANE"
        );
    }

    #[test]
    fn no_receipt_retry_records_bounded_attempt_number() {
        let action = decide(&no_receipt(ReceiptReason::IdleUnchanged), 1);
        assert_eq!(
            action,
            AckAction::Retry {
                pane_id: "%p".into(),
                attempt: 2,
                max_attempts: MAX_RETRY_ATTEMPTS,
            }
        );
        assert!(action.is_retry());
    }

    #[test]
    fn retry_budget_is_hard_and_exhaustion_does_not_retry() {
        let action = decide(
            &no_receipt(ReceiptReason::IdleUnchanged),
            MAX_RETRY_ATTEMPTS,
        );
        assert_eq!(
            action,
            AckAction::RetryExhausted {
                pane_id: "%p".into(),
                attempts: MAX_RETRY_ATTEMPTS,
            }
        );
        assert!(!action.is_retry());
    }

    #[test]
    fn wedged_unsubmitted_never_retries() {
        let action = decide(&no_receipt(ReceiptReason::WedgedUnsubmitted), 0);
        assert_eq!(action.label(), "UNSTICK");
        assert!(!action.is_retry());
    }

    #[test]
    fn indeterminate_dialog_never_retries() {
        let action = decide(
            &ReceiptVerdict::Indeterminate {
                pane_id: "%p".into(),
                reason: ReceiptReason::DialogOpen,
            },
            0,
        );
        assert_eq!(action.label(), "AWAIT_HUMAN");
        assert!(!action.is_retry());
    }
    #[test]
    fn ack_instruction_is_parser_compatible_and_receiver_derived() {
        let instruction = ack_instruction("omp-orchestrator-ack-stage-qhl");
        assert_eq!(
            instruction,
            r#"br comments add omp-orchestrator-ack-stage-qhl --actor "$(agent name you resolve yourself)" "ACK qhl on $TMUX_PANE -- agent=<same name> title=$(tmux display-message -p -t "$TMUX_PANE" '#{pane_title}')""#
        );
        assert!(!instruction.contains("%1413"));
        assert!(!instruction.contains("GreenFrog"));
        assert!(instruction.contains(" -- "));
        assert!(instruction.contains(r#"-t "$TMUX_PANE" '#{pane_title}'"#));
    }

    #[test]
    fn rendered_ack_round_trips_through_matching_comment_for() {
        let instruction = ack_instruction("omp-orchestrator-ack-stage-qhl")
            .replace("$(agent name you resolve yourself)", "GreenFrog")
            .replace("<same name>", "GreenFrog")
            .replace("$TMUX_PANE", "%1413");
        assert!(instruction.contains("ACK qhl on %1413 -- agent=GreenFrog"));
        let rendered_comment = "ACK qhl on %1413 -- agent=GreenFrog title=omp-orchestrator__cod_1";
        let readback = AckReadback {
            bead_id: "omp-orchestrator-ack-stage-qhl".into(),
            pane_id: "%1413".into(),
            comments: vec![AckComment {
                text: rendered_comment.into(),
                created_at: None,
            }],
            dispatch_issued_at: None,
        };
        assert_eq!(
            readback.matching_comment_for("omp-orchestrator-ack-stage-qhl", "%1413"),
            Some(rendered_comment)
        );
    }

    #[test]
    fn dropping_ack_separator_breaks_the_round_trip() {
        let malformed = "ACK qhl on %1413 agent=GreenFrog title=omp-orchestrator__cod_1";
        let readback = AckReadback {
            bead_id: "omp-orchestrator-ack-stage-qhl".into(),
            pane_id: "%1413".into(),
            comments: vec![AckComment {
                text: malformed.into(),
                created_at: None,
            }],
            dispatch_issued_at: None,
        };
        assert_eq!(
            readback.matching_comment_for("omp-orchestrator-ack-stage-qhl", "%1413"),
            None,
            "the parser and instruction must share the required separator"
        );
    }

    #[test]
    fn foreign_session_ack_pane_is_absent_not_a_mismatch() {
        let readback = ack(&["ACK qhl on %1414 -- agent=Other title=wrong-repo"]);
        assert_eq!(
            readback.match_verdict_for("omp-orchestrator-ack-stage-qhl", "%8"),
            AckReadbackVerdict::Missing,
            "a control-plane pane must not satisfy or refute this session's readback"
        );
        let result = assess(&AckStageInput {
            bead_id: "omp-orchestrator-ack-stage-qhl".into(),
            pane_id: "%8".into(),
            transport: ntm(),
            pre_send: idle(),
            post_send: PostSendObservation::Present(working()),
            ack: readback,
            attempts_so_far: 0,
            session_pane_ids: vec!["%5".into(), "%8".into(), "%9".into()],
        });
        assert_eq!(result.ack_verdict, AckReadbackVerdict::Missing);
        assert!(
            !matches!(
                result.delivery,
                ReceiptVerdict::Indeterminate {
                    reason: ReceiptReason::AckPaneMismatch { .. },
                    ..
                }
            ),
            "foreign pane must not surface as ACK_PANE_MISMATCH: {:?}",
            result.delivery
        );
    }

    #[test]
    fn in_session_ack_pane_mismatch_is_still_a_typed_refusal() {
        let readback = ack(&["ACK qhl on %7 -- agent=Other title=wrong-pane"]);
        assert_eq!(
            readback.match_verdict_in_session(
                "omp-orchestrator-ack-stage-qhl",
                "%8",
                &["%7".into(), "%8".into()]
            ),
            AckReadbackVerdict::AckPaneMismatch {
                expected: "%8".into(),
                got: "%7".into(),
            }
        );
        let result = assess(&AckStageInput {
            bead_id: "omp-orchestrator-ack-stage-qhl".into(),
            pane_id: "%8".into(),
            transport: ntm(),
            pre_send: idle(),
            post_send: PostSendObservation::Present(working()),
            ack: readback,
            attempts_so_far: 0,
            session_pane_ids: vec!["%7".into(), "%8".into()],
        });
        assert_eq!(
            result.ack_verdict,
            AckReadbackVerdict::AckPaneMismatch {
                expected: "%8".into(),
                got: "%7".into(),
            }
        );
        assert!(matches!(
            result.delivery,
            ReceiptVerdict::Indeterminate {
                reason: ReceiptReason::AckPaneMismatch { .. },
                ..
            }
        ));
    }

    #[test]
    fn an_empty_ack_census_is_an_error() {
        let error =
            AckReadback::from_comments_json("omp-orchestrator-ack-stage-qhl", "%1413", br#"[]"#)
                .expect_err("zero ACK rows must be loud");
        assert_eq!(error, AckReadbackError::EmptyAckCensus);
        assert!(error.to_string().contains("ACK_CENSUS_EMPTY"));
    }

    #[test]
    fn rfc3339_z_created_at_is_unix_from_the_tracker_row() {
        let value = serde_json::json!("2026-09-05T23:33:12Z");
        let parsed = parse_comment_created_at(&value).expect("RFC3339 Z must parse");
        assert_eq!(parsed, 1_788_651_192);
        assert_eq!(
            parse_comment_created_at(&serde_json::json!(1_788_651_192)),
            Some(1_788_651_192)
        );
        assert_eq!(unix_from_civil(1970, 1, 1, 0, 0, 0), Some(0));
    }

    #[test]
    fn stale_same_pane_ack_is_absent_not_a_mismatch() {
        const ISSUED: u64 = 1_788_650_431;
        let readback = ack_at(
            "%8",
            &[(
                "ACK qhl on %8 -- agent=WildStone title=stale-same-pane",
                ISSUED - 3_600,
            )],
        )
        .with_dispatch_issued_at(ISSUED);
        assert_eq!(
            readback.match_verdict_for("omp-orchestrator-ack-stage-qhl", "%8"),
            AckReadbackVerdict::Missing,
            "KNOWN-BAD: an ACK predating issued_at must not confirm this dispatch"
        );
        let result = assess(&AckStageInput {
            bead_id: "omp-orchestrator-ack-stage-qhl".into(),
            pane_id: "%8".into(),
            transport: ntm(),
            pre_send: idle(),
            post_send: PostSendObservation::Present(working()),
            ack: readback,
            attempts_so_far: 0,
            session_pane_ids: vec!["%8".into()],
        });
        assert_eq!(result.ack_verdict, AckReadbackVerdict::Missing);
        assert!(!result.is_confirmed(), "stale ACK must keep waiting");
        assert!(
            !matches!(
                result.delivery,
                ReceiptVerdict::Indeterminate {
                    reason: ReceiptReason::AckPaneMismatch { .. },
                    ..
                }
            ),
            "stale is ABSENT not mismatch: {:?}",
            result.delivery
        );
    }

    #[test]
    fn fresh_same_pane_ack_is_accepted() {
        const ISSUED: u64 = 1_788_650_431;
        let readback = ack_at(
            "%1413",
            &[(
                "ACK qhl on %1413 -- agent=WildStone title=fresh-same-pane",
                ISSUED + 12,
            )],
        )
        .with_dispatch_issued_at(ISSUED);
        assert_eq!(
            readback.match_verdict_for("omp-orchestrator-ack-stage-qhl", "%1413"),
            AckReadbackVerdict::Matched {
                comment: "ACK qhl on %1413 -- agent=WildStone title=fresh-same-pane".into(),
            }
        );
        let result = assess(&AckStageInput {
            bead_id: "omp-orchestrator-ack-stage-qhl".into(),
            pane_id: "%1413".into(),
            transport: ntm(),
            pre_send: idle(),
            post_send: PostSendObservation::Present(working()),
            ack: readback,
            attempts_so_far: 0,
            session_pane_ids: vec!["%1413".into()],
        });
        assert!(
            result.is_confirmed(),
            "KNOWN-GOOD: fresh ACK on the expected pane"
        );
    }

    #[test]
    fn comments_without_ack_prefix_are_absent_never_a_pass() {
        let readback = ack_at("%8", &[("progress note, not an ACK", 1_788_650_500)])
            .with_dispatch_issued_at(1_788_650_431);
        assert_eq!(
            readback.match_verdict_for("omp-orchestrator-ack-stage-qhl", "%8"),
            AckReadbackVerdict::Missing,
            "ANTI-VACUITY: zero ACK comments is ABSENT, never a pass"
        );
        assert!(readback.matching_comment().is_none());
    }

    #[test]
    fn disabling_recency_lets_a_stale_ack_pass() {
        const ISSUED: u64 = 1_788_650_431;
        let stale = ack_at(
            "%8",
            &[(
                "ACK qhl on %8 -- agent=WildStone title=stale-same-pane",
                ISSUED - 3_600,
            )],
        );
        assert_eq!(
            stale.match_verdict_for("omp-orchestrator-ack-stage-qhl", "%8"),
            AckReadbackVerdict::Matched {
                comment: "ACK qhl on %8 -- agent=WildStone title=stale-same-pane".into(),
            },
            "MUTATION: issued_at=None must go RED against the recency invariant"
        );
        let gated = stale.clone().with_dispatch_issued_at(ISSUED);
        assert_eq!(
            gated.match_verdict_for("omp-orchestrator-ack-stage-qhl", "%8"),
            AckReadbackVerdict::Missing
        );
    }

    #[test]
    fn fresh_in_session_mismatch_is_still_ack_pane_mismatch() {
        const ISSUED: u64 = 1_788_650_431;
        let readback = ack_at(
            "%8",
            &[(
                "ACK qhl on %7 -- agent=Other title=fresh-wrong-pane",
                ISSUED + 5,
            )],
        )
        .with_dispatch_issued_at(ISSUED);
        assert_eq!(
            readback.match_verdict_in_session(
                "omp-orchestrator-ack-stage-qhl",
                "%8",
                &["%7".into(), "%8".into()]
            ),
            AckReadbackVerdict::AckPaneMismatch {
                expected: "%8".into(),
                got: "%7".into(),
            },
            "FRESH in-session mismatch must stay ACK_PANE_MISMATCH"
        );
    }

    #[test]
    fn stale_in_session_mismatch_is_absent() {
        const ISSUED: u64 = 1_788_650_431;
        let readback = ack_at(
            "%8",
            &[(
                "ACK qhl on %7 -- agent=Other title=stale-wrong-pane",
                ISSUED - 10,
            )],
        )
        .with_dispatch_issued_at(ISSUED);
        assert_eq!(
            readback.match_verdict_in_session(
                "omp-orchestrator-ack-stage-qhl",
                "%8",
                &["%7".into(), "%8".into()]
            ),
            AckReadbackVerdict::Missing,
            "a stale wrong-pane ACK is ABSENT, not a mismatch"
        );
    }

    #[test]
    fn equal_created_at_is_not_strictly_newer() {
        const ISSUED: u64 = 1_788_650_431;
        let readback = ack_at(
            "%8",
            &[("ACK qhl on %8 -- agent=WildStone title=equal-clock", ISSUED)],
        )
        .with_dispatch_issued_at(ISSUED);
        assert_eq!(
            readback.match_verdict_for("omp-orchestrator-ack-stage-qhl", "%8"),
            AckReadbackVerdict::Missing
        );
    }

    #[test]
    fn undatable_comment_fails_closed_under_recency() {
        let readback = ack(&["ACK qhl on %8 -- agent=WildStone title=no-time"])
            .with_dispatch_issued_at(1_788_650_431);
        assert_eq!(
            readback.match_verdict_for("omp-orchestrator-ack-stage-qhl", "%8"),
            AckReadbackVerdict::Missing,
            "missing created_at under recency is ABSENT"
        );
    }
}

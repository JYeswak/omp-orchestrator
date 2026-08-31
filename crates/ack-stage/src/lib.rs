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

/// Read-back of the tracker comments; each text is preserved verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckReadback {
    pub bead_id: String,
    pub pane_id: String,
    pub comments: Vec<String>,
}

/// Why the authoritative comment read-back could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AckReadbackError {
    InvalidJson(String),
    NotAnArray,
    MissingText(usize),
    WrongTextType(usize),
}

impl fmt::Display for AckReadbackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(error) => write!(f, "comments JSON is invalid: {error}"),
            Self::NotAnArray => f.write_str("comments JSON is not an array"),
            Self::MissingText(index) => write!(f, "comment row {index} has no text"),
            Self::WrongTextType(index) => write!(f, "comment row {index} text is not a string"),
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
        let mut comments = Vec::with_capacity(rows.len());
        for (index, row) in rows.iter().enumerate() {
            let text = row
                .get("text")
                .ok_or(AckReadbackError::MissingText(index))?
                .as_str()
                .ok_or(AckReadbackError::WrongTextType(index))?;
            comments.push(text.to_owned());
        }
        Ok(Self {
            bead_id: bead_id.into(),
            pane_id: pane_id.into(),
            comments,
        })
    }

    /// Return the exact matching comment body, if the required ACK exists.
    pub fn matching_comment(&self) -> Option<&str> {
        self.matching_comment_for(&self.bead_id, &self.pane_id)
    }

    /// Match only a read-back bound to the stage's bead and pane.
    pub fn matching_comment_for(&self, bead_id: &str, pane_id: &str) -> Option<&str> {
        if self.bead_id != bead_id || self.pane_id != pane_id {
            return None;
        }
        let token = bead_id.rsplit('-').next().unwrap_or(bead_id);
        let prefix = format!("ACK {token} on {pane_id} -- ");
        self.comments
            .iter()
            .find(|comment| comment.starts_with(&prefix))
            .map(String::as_str)
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
}

/// One typed action plus all evidence that led to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckStageResult {
    pub action: AckAction,
    pub delivery: ReceiptVerdict,
    pub transport: TransportReceipt,
    pub ack_comment: Option<String>,
}

impl AckStageResult {
    pub fn is_confirmed(&self) -> bool {
        matches!(self.action, AckAction::RecordReceipt { .. })
            && self.ack_comment.is_some()
            && matches!(self.delivery, ReceiptVerdict::ReceiptConfirmed { .. })
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
    let ack_comment = input
        .ack
        .matching_comment_for(&input.bead_id, &input.pane_id)
        .map(ToOwned::to_owned);
    let delivery =
        if matches!(delivery, ReceiptVerdict::ReceiptConfirmed { .. }) && ack_comment.is_none() {
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
        ReceiptVerdict::ReceiptConfirmed { pane_id, .. } => AckAction::RecordReceipt {
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
            comments: comments.iter().map(|comment| (*comment).into()).collect(),
        }
    }

    fn idle() -> Observation {
        receiver_receipt::observe_capture("%1413", "prompt\nπ . GPT-5.6 . /tmp/receiver", 100)
    }

    fn working() -> Observation {
        receiver_receipt::observe_capture(
            "%1413",
            "accepted packet\n⠙ 1s . GPT-5.6 . /tmp/receiver",
            101,
        )
    }

    #[test]
    fn ntm_capture_retains_full_json_and_successful_is_not_delivery() {
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
        });
        assert_eq!(result.action.label(), "RETRY");
        assert!(!result.is_confirmed());
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
}

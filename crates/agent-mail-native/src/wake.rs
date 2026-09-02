//! A typed wrapper over NTM's blocking mail wake.
//!
//! **This is a live-notification surface, deliberately separate from Agent
//! Mail.** Agent Mail is durable truth: what was delivered, read, and
//! acknowledged. NTM is the wake: a blocking call that returns when something
//! arrives. Keeping them apart means a notification defect can never corrupt
//! the durable record, and a durable read never depends on a notifier being
//! healthy.
//!
//! The wrapper exists rather than a hand-rolled poller because a blocking wake
//! already exists in a kernel. Per `docs/contracts/kernel_only_policy.md`,
//! re-implementing one as a sleep loop would be a handroll of a capability we
//! already own. There is **no polling loop in this module.**
//!
//! # The measured terminal shapes, and what actually discriminates them
//!
//! `ntm --robot-wait --wait-until=mail_pending` has two terminal shapes. The
//! fleet initially concluded an explicit `--timeout` selected between them,
//! and that conclusion was WRONG. Full measurement, 2026-09-02, four readers:
//!
//! | invocation | outer ceiling | `error_code` | `cursor_info` | `waited_seconds` |
//! |---|---|---|---|---|
//! | `--timeout=15s` | none | `TIMEOUT` | present | 16.03 |
//! | `--timeout=20s` | none | `TIMEOUT` | present | 20.04 |
//! | `--timeout=30s` | none | `TIMEOUT` | present | 30.05 |
//! | no flag | `timeout 75` | `CANCELED` | **absent** | 71.34 |
//! | no flag | `timeout 90` | `CANCELED` | **absent** | 83.47 |
//! | no flag | `timeout 180` | `CANCELED` | **absent** | 176.03 |
//! | no flag | `timeout 340` | **`TIMEOUT`** | **present** | **300.34** |
//!
//! The last row settles it. **The documented 5-minute default is real and it
//! produces the good terminal shape**: at 300.34s the no-flag path returned
//! `error_code=TIMEOUT` with `cursor_info{observed_cursor: 298904,
//! next_cursor: 298904, oldest_cursor: ...}`. The default path is NOT
//! unbounded and does NOT block forever.
//!
//! What produced every `CANCELED` was the OBSERVER: each of the 75s/90s/180s
//! runs was killed by the caller's own `timeout` before NTM's internal
//! deadline, which is why `waited_seconds` tracked the outer ceiling linearly.
//! Three readers each mistook their own SIGTERM for the kernel's behaviour.
//!
//! So the real defect is narrower, and it is a genuine one: **when the process
//! is killed by an external signal, NTM reports `CANCELED` with `cursor_info`
//! ABSENT** — dropping the resume point that the very same kernel emits on its
//! timeout path. A supervisor that bounds a wait therefore costs its caller
//! the cursor.
//!
//! # Why [`WakeRequest::timeout`] is still mandatory
//!
//! Not because the default never returns — it does. Because:
//!
//! 1. A 5-minute block is far too long for a dispatch loop, and inheriting a
//!    default ceiling means the loop's latency is set by a kernel default
//!    rather than by the caller.
//! 2. Owning the ceiling is what keeps the resume cursor. If our own deadline
//!    fires we get `TIMEOUT` with the resumable triple; if someone else's
//!    signal lands first we get `CANCELED` with nothing. Bounding the wait
//!    ourselves makes the good path the default path.
//!
//! The `CANCELED`-without-cursor shape is still modelled, as
//! [`MailWakeOutcome::CancelledWithoutCursor`], because a supervisor can
//! always out-race our ceiling. It is a distinct variant and never means
//! "no mail".

use crate::error::MailError;
use asupersync::Cx;
use asupersync::process::Command;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::time::Duration;
use subprocess_contract::{RunError, run_output};

/// A position in NTM's attention feed.
///
/// A DIFFERENT SEQUENCE from [`DeliveryCursor`](crate::DeliveryCursor).
/// Measured simultaneously on 2026-09-02: the Agent Mail delivery cursor was
/// ~5,160 while the NTM attention cursor was ~298,700. Passing one where the
/// other belongs is silently accepted by both tools because both are integers,
/// so they are separate types here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AttentionCursor(u64);

impl AttentionCursor {
    /// Wrap a raw attention-feed position.
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// The raw position, for passing back to NTM only.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for AttentionCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "attention:{}", self.0)
    }
}

/// Which arrival condition to wait for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeCondition {
    /// Mail is pending for a target pane.
    MailPending,
    /// Mail requiring acknowledgement is pending.
    MailAckRequired,
}

impl WakeCondition {
    /// The `--wait-until` value.
    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::MailPending => "mail_pending",
            Self::MailAckRequired => "mail_ack_required",
        }
    }
}

/// A blocking wait for mail arrival.
#[derive(Debug, Clone)]
pub struct WakeRequest {
    /// The NTM session to watch.
    pub session: String,
    /// The arrival condition.
    pub condition: WakeCondition,
    /// Where in the attention feed to wait from.
    pub attention_cursor: Option<AttentionCursor>,
    /// How long to block. **Mandatory**: the no-flag path never returns.
    pub timeout: Duration,
}

impl WakeRequest {
    /// Build a bounded wait for pending mail.
    #[must_use]
    pub fn mail_pending(session: impl Into<String>, timeout: Duration) -> Self {
        Self {
            session: session.into(),
            condition: WakeCondition::MailPending,
            attention_cursor: None,
            timeout,
        }
    }

    /// Resume the wait from a known attention position.
    #[must_use]
    pub fn from_cursor(mut self, cursor: AttentionCursor) -> Self {
        self.attention_cursor = Some(cursor);
        self
    }

    /// The exact argv this request becomes.
    ///
    /// Public so a caller can log or assert the convention, and so the
    /// mandatory `--timeout` is testable without spawning anything.
    #[must_use]
    pub fn argv(&self) -> Vec<String> {
        let mut argv = vec![
            format!("--robot-wait={}", self.session),
            format!("--wait-until={}", self.condition.as_wire()),
            // ALWAYS present. Omitting it is the unbounded path.
            format!("--timeout={}s", self.timeout.as_secs().max(1)),
        ];
        if let Some(cursor) = self.attention_cursor {
            argv.push(format!("--attention-cursor={}", cursor.get()));
        }
        argv
    }
}

/// How a bounded wake ended.
///
/// Matched exhaustively everywhere: this enum's name ends in `outcome`, which
/// the repo's `state-wildcard-lint` treats as state-like, so a wildcard arm
/// on it is a lint violation. That is the right rule here — a new terminal
/// shape from NTM must force every call site to decide what it means rather
/// than silently falling into a catch-all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailWakeOutcome {
    /// The condition fired.
    Woke {
        /// NTM's `wake_reason`.
        reason: String,
        /// NTM's `matched_condition`.
        matched: String,
        /// The triggering event's attention position, when reported.
        trigger_cursor: Option<AttentionCursor>,
        /// Where to resume the feed.
        next_cursor: Option<AttentionCursor>,
    },
    /// Our own ceiling elapsed. Carries NO verdict about the mailbox.
    ///
    /// This is the shape that makes the wake usable: it arrives with the
    /// resumable triple, so a monitor loses nothing by timing out.
    TimedOut {
        /// What NTM reported it waited.
        waited: Duration,
        /// The feed position observed at timeout.
        observed_cursor: Option<AttentionCursor>,
        /// Where to resume.
        next_cursor: Option<AttentionCursor>,
        /// The oldest position still available in the feed.
        oldest_cursor: Option<AttentionCursor>,
    },
    /// The wait was cancelled by a signal and returned NO resume cursor.
    ///
    /// The measured defect shape. Distinct from
    /// [`MailWakeOutcome::TimedOut`] because the caller has LOST ITS PLACE and
    /// must re-baseline; and distinct from any notion of "no mail", because
    /// nothing was learned about the mailbox at all.
    CancelledWithoutCursor {
        /// What NTM reported it waited before dying.
        waited: Duration,
    },
}

impl MailWakeOutcome {
    /// True when the outcome says nothing about whether mail exists.
    #[must_use]
    pub fn is_inconclusive(&self) -> bool {
        match self {
            Self::Woke { .. } => false,
            Self::TimedOut { .. } | Self::CancelledWithoutCursor { .. } => true,
        }
    }

    /// The position to resume the attention feed from, when one survived.
    #[must_use]
    pub fn resume_cursor(&self) -> Option<AttentionCursor> {
        match self {
            Self::Woke { next_cursor, .. } => *next_cursor,
            Self::TimedOut { next_cursor, .. } => *next_cursor,
            Self::CancelledWithoutCursor { .. } => None,
        }
    }
}

/// Block until mail arrives, our ceiling elapses, or we are signalled.
///
/// Spawns `ntm` exactly once through
/// [`subprocess_contract::run_output`], which owns the child in its own
/// process group, drains stdout and stderr concurrently, observes the caller's
/// `Cx`, and escalates group termination on cancellation. There is no detached
/// task and no second pipe left unread.
pub async fn wait_for_mail(
    cx: &Cx,
    request: &WakeRequest,
) -> Result<MailWakeOutcome, MailError> {
    cx.checkpoint()
        .map_err(|_| MailError::from_cancelled(cx, "wait_for_mail"))?;

    let mut command = Command::new("ntm");
    command.args(request.argv());

    let output = match run_output(cx, command).await {
        Ok(output) => output,
        Err(RunError::Timeout) => {
            return Err(MailError::TimedOut {
                operation: "wait_for_mail".to_owned(),
            });
        }
        Err(RunError::Cancelled(kind)) => return Err(MailError::Cancelled(kind)),
        Err(RunError::Process(error)) => {
            return Err(MailError::Unreachable {
                endpoint: "ntm".to_owned(),
                detail: format!("spawn: {error}"),
            });
        }
    };

    // NTM exits non-zero on both TIMEOUT and CANCELED, so the exit status is
    // not the discriminator — the JSON body is. Parsing stdout regardless of
    // status is deliberate.
    parse_wake(&output.stdout)
}

/// Classify NTM's robot JSON into a terminal outcome.
///
/// Separated from the spawn so every shape below is testable against the exact
/// bodies captured from the live kernel, with no process involved.
pub fn parse_wake(stdout: &[u8]) -> Result<MailWakeOutcome, MailError> {
    let text = String::from_utf8_lossy(stdout);
    let body: Value = serde_json::from_str(text.trim()).map_err(|error| MailError::Protocol {
        detail: format!("ntm robot output was not JSON: {error}"),
    })?;

    let cursor = |parent: &Value, field: &str| -> Option<AttentionCursor> {
        parent
            .get(field)
            .and_then(Value::as_u64)
            .map(AttentionCursor::new)
    };
    let waited = Duration::from_secs_f64(
        body.get("waited_seconds")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            .max(0.0),
    );

    if body.get("success").and_then(Value::as_bool).unwrap_or(false) {
        let trigger = body.get("trigger_event");
        return Ok(MailWakeOutcome::Woke {
            reason: body
                .get("wake_reason")
                .and_then(Value::as_str)
                .unwrap_or("unspecified")
                .to_owned(),
            matched: body
                .get("matched_condition")
                .and_then(Value::as_str)
                .unwrap_or("unspecified")
                .to_owned(),
            trigger_cursor: trigger.and_then(|event| cursor(event, "cursor")),
            next_cursor: body
                .get("cursor_info")
                .and_then(|info| cursor(info, "next_cursor")),
        });
    }

    let code = body
        .get("error_code")
        .and_then(Value::as_str)
        .unwrap_or("UNSPECIFIED");

    match body.get("cursor_info") {
        // The good path: a ceiling elapsed and the resumable triple survived.
        Some(info) if code == "TIMEOUT" => Ok(MailWakeOutcome::TimedOut {
            waited,
            observed_cursor: cursor(info, "observed_cursor"),
            next_cursor: cursor(info, "next_cursor"),
            oldest_cursor: cursor(info, "oldest_cursor"),
        }),
        // TIMEOUT without the triple would be a NEW shape. Refuse rather than
        // synthesise a resume point we did not receive.
        None if code == "TIMEOUT" => Err(MailError::Protocol {
            detail: "ntm reported TIMEOUT with no cursor_info; resume point unavailable"
                .to_owned(),
        }),
        // The measured defect: cancelled, cursor stripped.
        Some(_) | None => {
            if code == "CANCELED" {
                Ok(MailWakeOutcome::CancelledWithoutCursor { waited })
            } else {
                Err(MailError::Protocol {
                    detail: format!(
                        "ntm reported unrecognised error_code={code}: {}",
                        body.get("error")
                            .and_then(Value::as_str)
                            .unwrap_or("<no error text>")
                    ),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Captured live: `--timeout=15s`, rc=1.
    const LIVE_TIMEOUT: &str = r#"{"success":false,"error":"timeout after 15s","error_code":"TIMEOUT","session":"omp-orchestrator","condition":"mail_pending","waited_seconds":16.031305292,"agents":[],"cursor_info":{"observed_cursor":298724,"next_cursor":298724,"oldest_cursor":297786}}"#;

    /// Captured live: no `--timeout`, killed by an outer 90s ceiling.
    const LIVE_CANCELED: &str = r#"{"success":false,"error":"context canceled","error_code":"CANCELED","hint":"Start a new --robot-wait when ready","session":"omp-orchestrator","condition":"mail_pending","waited_seconds":83.469077833,"agents":[]}"#;

    /// Captured live from `--robot-attention`, the working wake surface.
    const LIVE_WOKE: &str = r#"{"success":true,"wake_reason":"attention","matched_condition":"attention","trigger_event":{"cursor":298316,"category":"bead","type":"bead.updated"},"cursor_info":{"next_cursor":298317}}"#;

    #[test]
    fn argv_always_carries_an_explicit_timeout() {
        // The single most important assertion in this module: the unbounded
        // path must be unreachable through this type.
        let request = WakeRequest::mail_pending("omp-orchestrator", Duration::from_secs(30));
        let argv = request.argv();
        assert!(
            argv.iter().any(|arg| arg.starts_with("--timeout=")),
            "a request without --timeout never returns: {argv:?}"
        );
        assert!(argv.contains(&"--timeout=30s".to_owned()), "{argv:?}");
        assert!(
            argv.contains(&"--wait-until=mail_pending".to_owned()),
            "{argv:?}"
        );
        assert!(
            argv.contains(&"--robot-wait=omp-orchestrator".to_owned()),
            "{argv:?}"
        );
    }

    #[test]
    fn a_sub_second_timeout_still_sends_a_nonzero_ceiling() {
        // `--timeout=0s` would be indistinguishable from no ceiling.
        let request = WakeRequest::mail_pending("s", Duration::from_millis(10));
        assert!(request.argv().contains(&"--timeout=1s".to_owned()));
    }

    #[test]
    fn attention_cursor_is_only_sent_when_supplied() {
        let bare = WakeRequest::mail_pending("s", Duration::from_secs(5));
        assert!(
            !bare.argv().iter().any(|a| a.contains("attention-cursor")),
            "must not invent a cursor"
        );
        let resumed = bare.from_cursor(AttentionCursor::new(298_315));
        assert!(
            resumed
                .argv()
                .contains(&"--attention-cursor=298315".to_owned())
        );
    }

    #[test]
    fn timeout_shape_yields_the_resumable_triple() {
        match parse_wake(LIVE_TIMEOUT.as_bytes()).expect("parse") {
            MailWakeOutcome::TimedOut {
                waited,
                observed_cursor,
                next_cursor,
                oldest_cursor,
            } => {
                assert_eq!(observed_cursor, Some(AttentionCursor::new(298_724)));
                assert_eq!(next_cursor, Some(AttentionCursor::new(298_724)));
                assert_eq!(oldest_cursor, Some(AttentionCursor::new(297_786)));
                assert!((waited.as_secs_f64() - 16.031).abs() < 0.01);
            }
            other => panic!("a bounded timeout must not read as {other:?}"),
        }
    }

    #[test]
    fn canceled_shape_is_distinct_and_loses_its_place() {
        match parse_wake(LIVE_CANCELED.as_bytes()).expect("parse") {
            MailWakeOutcome::CancelledWithoutCursor { waited } => {
                assert!((waited.as_secs_f64() - 83.469).abs() < 0.01);
            }
            other => panic!("the cancelled shape must not read as {other:?}"),
        }
    }

    #[test]
    fn canceled_is_not_a_timeout_and_offers_no_resume_point() {
        // The whole reason these are separate variants: a timeout keeps the
        // caller's place, a cancel does not. Collapsing them would silently
        // resume from a cursor that was never returned.
        let cancelled = parse_wake(LIVE_CANCELED.as_bytes()).expect("parse");
        let timed_out = parse_wake(LIVE_TIMEOUT.as_bytes()).expect("parse");
        assert_ne!(cancelled, timed_out);
        assert_eq!(cancelled.resume_cursor(), None);
        assert_eq!(
            timed_out.resume_cursor(),
            Some(AttentionCursor::new(298_724))
        );
    }

    #[test]
    fn no_terminal_shape_ever_means_no_mail() {
        for raw in [LIVE_TIMEOUT, LIVE_CANCELED] {
            let outcome = parse_wake(raw.as_bytes()).expect("parse");
            assert!(
                outcome.is_inconclusive(),
                "{outcome:?} must not imply an empty mailbox"
            );
        }
    }

    #[test]
    fn a_successful_wake_is_conclusive_and_carries_the_trigger() {
        match parse_wake(LIVE_WOKE.as_bytes()).expect("parse") {
            MailWakeOutcome::Woke {
                reason,
                matched,
                trigger_cursor,
                next_cursor,
            } => {
                assert_eq!(reason, "attention");
                assert_eq!(matched, "attention");
                assert_eq!(trigger_cursor, Some(AttentionCursor::new(298_316)));
                assert_eq!(next_cursor, Some(AttentionCursor::new(298_317)));
            }
            other => panic!("a fired condition must not read as {other:?}"),
        }
        assert!(
            !parse_wake(LIVE_WOKE.as_bytes())
                .expect("parse")
                .is_inconclusive()
        );
    }

    #[test]
    fn a_timeout_without_cursor_info_is_refused_not_invented() {
        let raw = r#"{"success":false,"error_code":"TIMEOUT","waited_seconds":5.0}"#;
        match parse_wake(raw.as_bytes()).expect_err("must refuse") {
            MailError::Protocol { detail } => {
                assert!(detail.contains("resume point unavailable"), "{detail}");
            }
            other => panic!("expected a protocol refusal, got {other}"),
        }
    }

    #[test]
    fn an_unrecognised_error_code_is_refused_rather_than_guessed() {
        let raw = r#"{"success":false,"error_code":"WAT","error":"something new","waited_seconds":1.0}"#;
        match parse_wake(raw.as_bytes()).expect_err("must refuse") {
            MailError::Protocol { detail } => assert!(detail.contains("WAT"), "{detail}"),
            other => panic!("expected a protocol refusal, got {other}"),
        }
    }

    #[test]
    fn non_json_output_is_a_protocol_error_not_a_silent_empty() {
        match parse_wake(b"ntm: command not found").expect_err("must refuse") {
            MailError::Protocol { detail } => assert!(detail.contains("not JSON"), "{detail}"),
            other => panic!("expected a protocol refusal, got {other}"),
        }
    }

    #[test]
    fn attention_and_delivery_cursors_are_different_types() {
        // Both are integers on the wire and ~5,160 vs ~298,700 in practice.
        let attention = AttentionCursor::new(298_724);
        let delivery = crate::DeliveryCursor::new(5161);
        assert_eq!(attention.to_string(), "attention:298724");
        assert_eq!(delivery.to_string(), "cursor:5161");
    }
}

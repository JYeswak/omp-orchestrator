//! The `am` CLI as a differential oracle for the daemon binding.
//!
//! This is deliberately NOT a second implementation of the journey. It is one
//! narrow read, used to cross-check the daemon binding against an independent
//! reader.
//!
//! The oracle is meaningful precisely because the two paths do not share code
//! or transport. Measured 2026-09-02:
//!
//! - the **daemon** answers authenticated MCP JSON-RPC on `127.0.0.1:8765`;
//! - the **CLI** carries no bearer token and reads `storage.sqlite3` directly.
//!
//! So agreement between them is evidence about the STORE, not evidence that
//! one layer is self-consistent. Their first measured agreement: recipient
//! `GreenFrog` reported `tail_cursor` 5142 via both paths at the same instant.
//!
//! Keeping the CLI in this role rather than as a fallback matters. A fallback
//! would silently paper over a daemon outage with a direct SQLite read, which
//! is exactly the fail-open the CLI itself already does — and exactly how an
//! auth failure came to be reported as absence.

use crate::cursor::DeliveryCursor;
use crate::error::MailError;
use crate::journey::{AgentName, EventPage, ProjectKey};
use asupersync::Cx;
use asupersync::process::Command;
use serde::Deserialize;
use serde_json::Value;
use subprocess_contract::{RunError, run_output};

/// One `am inbox-events --position-now` reading.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OracleReading {
    /// The cursor to resume from.
    pub next_cursor: DeliveryCursor,
    /// This recipient's high-water mark.
    pub tail_cursor: DeliveryCursor,
    /// The oldest position still retained for this recipient.
    #[serde(default)]
    pub oldest_available_cursor: Option<DeliveryCursor>,
}

/// Whether the two independent readers agree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorAgreement {
    /// Both readers reported the same tail.
    Agree {
        /// The agreed position.
        tail: DeliveryCursor,
    },
    /// The readers disagree.
    ///
    /// Not necessarily a defect: the delivery log is append-only and live, so
    /// a message arriving between the two reads moves the tail forward. A
    /// disagreement where the CLI is AHEAD by a little is ordinary skew; the
    /// CLI reading BEHIND the daemon, or a large gap, is not.
    Disagree {
        /// What the daemon binding read.
        daemon: DeliveryCursor,
        /// What the CLI read.
        cli: DeliveryCursor,
    },
}

impl CursorAgreement {
    /// True when both readers matched exactly.
    #[must_use]
    pub fn is_agreement(&self) -> bool {
        match self {
            Self::Agree { .. } => true,
            Self::Disagree { .. } => false,
        }
    }

    /// How far apart the readers are, as a signed count of positions.
    ///
    /// Positive means the CLI is ahead of the daemon binding.
    #[must_use]
    pub fn skew(&self) -> i128 {
        match self {
            Self::Agree { .. } => 0,
            Self::Disagree { daemon, cli } => i128::from(cli.get()) - i128::from(daemon.get()),
        }
    }
}

/// Compare a daemon page against a CLI reading.
#[must_use]
pub fn compare(daemon: &EventPage, cli: &OracleReading) -> CursorAgreement {
    if daemon.tail_cursor == cli.tail_cursor {
        CursorAgreement::Agree {
            tail: daemon.tail_cursor,
        }
    } else {
        CursorAgreement::Disagree {
            daemon: daemon.tail_cursor,
            cli: cli.tail_cursor,
        }
    }
}

/// Read the tail position through the CLI.
///
/// Spawns `am` once via [`subprocess_contract::run_output`]: own process
/// group, both pipes drained concurrently, bounded by the caller's `Cx`.
pub async fn cli_position_now(
    cx: &Cx,
    project: &ProjectKey,
    agent: &AgentName,
) -> Result<OracleReading, MailError> {
    cx.checkpoint()
        .map_err(|_| MailError::from_cancelled(cx, "cli_position_now"))?;

    let mut command = Command::new("am");
    command.args([
        "inbox-events",
        "--project",
        project.as_str(),
        "--agent",
        agent.as_str(),
        "--position-now",
        "--json",
    ]);

    let output = match run_output(cx, command).await {
        Ok(output) => output,
        Err(RunError::Timeout) => {
            return Err(MailError::TimedOut {
                operation: "cli_position_now".to_owned(),
            });
        }
        Err(RunError::Cancelled(kind)) => return Err(MailError::Cancelled(kind)),
        Err(RunError::Process(error)) => {
            return Err(MailError::Unreachable {
                endpoint: "am".to_owned(),
                detail: format!("spawn: {error}"),
            });
        }
    };

    parse_oracle(&output.stdout, &output.stderr, output.status.code())
}

/// Parse a CLI reading, honouring its fail-closed refusals.
///
/// Kept separate from the spawn so the refusal shapes are testable. Note the
/// exit-status discipline: the CLI **fails closed** for an unregistered agent
/// with `{"status":"error","code":"inbox_events_unavailable"}` and a nonzero
/// status, which is correct behaviour and must not be read as an empty
/// mailbox.
pub fn parse_oracle(
    stdout: &[u8],
    stderr: &[u8],
    status: Option<i32>,
) -> Result<OracleReading, MailError> {
    let text = String::from_utf8_lossy(stdout);
    // The CLI prefixes unrelated migration chatter on some invocations, so the
    // JSON object is located rather than assumed to start at byte zero.
    let body = text
        .find('{')
        .map(|start| &text[start..])
        .unwrap_or(text.as_ref())
        .trim();

    if body.is_empty() {
        return Err(MailError::Unreachable {
            endpoint: "am".to_owned(),
            detail: format!(
                "CLI produced no JSON (status {:?}): {}",
                status,
                String::from_utf8_lossy(stderr).trim()
            ),
        });
    }

    let parsed: Value = serde_json::from_str(body).map_err(|error| MailError::Protocol {
        detail: format!("CLI output was not JSON: {error}"),
    })?;

    if let Some(code) = parsed.get("code").and_then(Value::as_str) {
        return Err(MailError::ToolRefused {
            tool: "am inbox-events".to_owned(),
            kind: code.to_owned(),
            message: parsed
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("CLI refused")
                .to_owned(),
            recoverable: false,
        });
    }

    serde_json::from_value(parsed).map_err(|error| MailError::Codec {
        detail: format!("decoding CLI reading: {error}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Captured live: `am inbox-events --agent GreenFrog --position-now --json`.
    const LIVE_CLI: &str = r#"{"events":[],"next_cursor":5161,"has_more":false,"oldest_available_cursor":2108,"tail_cursor":5161}"#;

    fn page(tail: u64, oldest: Option<u64>) -> EventPage {
        EventPage {
            events: Vec::new(),
            next_cursor: DeliveryCursor::new(tail),
            has_more: false,
            oldest_available_cursor: oldest.map(DeliveryCursor::new),
            tail_cursor: DeliveryCursor::new(tail),
        }
    }

    #[test]
    fn cli_reading_decodes_the_measured_shape() {
        let reading = parse_oracle(LIVE_CLI.as_bytes(), b"", Some(0)).expect("parse");
        assert_eq!(reading.tail_cursor, DeliveryCursor::new(5161));
        assert_eq!(
            reading.oldest_available_cursor,
            Some(DeliveryCursor::new(2108))
        );
    }

    #[test]
    fn migration_chatter_before_the_json_is_tolerated() {
        // Measured: the CLI prints "fsqlite: applied migration repairs..."
        // ahead of its JSON on some invocations.
        let noisy = format!("fsqlite: applied migration repairs (took 0.1s)\n{LIVE_CLI}");
        let reading = parse_oracle(noisy.as_bytes(), b"", Some(0)).expect("parse");
        assert_eq!(reading.tail_cursor, DeliveryCursor::new(5161));
    }

    #[test]
    fn the_two_readers_agreeing_is_an_agreement() {
        let reading = parse_oracle(LIVE_CLI.as_bytes(), b"", Some(0)).expect("parse");
        let verdict = compare(&page(5161, Some(2108)), &reading);
        assert!(verdict.is_agreement());
        assert_eq!(verdict.skew(), 0);
        assert_eq!(
            verdict,
            CursorAgreement::Agree {
                tail: DeliveryCursor::new(5161)
            }
        );
    }

    #[test]
    fn live_append_only_skew_is_reported_signed_not_hidden() {
        let reading = parse_oracle(LIVE_CLI.as_bytes(), b"", Some(0)).expect("parse");
        // Daemon read first, at 5159; two events landed before the CLI read.
        let verdict = compare(&page(5159, Some(2108)), &reading);
        assert!(!verdict.is_agreement());
        assert_eq!(verdict.skew(), 2, "the CLI is two positions ahead");
        // And the reverse direction is negative, which is the suspicious one.
        assert_eq!(compare(&page(5163, Some(2108)), &reading).skew(), -2);
    }

    #[test]
    fn a_fail_closed_cli_refusal_is_not_an_empty_mailbox() {
        // The measured unregistered-agent shape. Correct CLI behaviour, and it
        // must surface as a refusal rather than a zero reading.
        let raw = r#"{"status":"error","code":"inbox_events_unavailable","message":"agent not registered"}"#;
        let error = parse_oracle(raw.as_bytes(), b"", Some(1)).expect_err("must refuse");
        match error {
            MailError::ToolRefused { kind, .. } => {
                assert_eq!(kind, "inbox_events_unavailable");
            }
            other => panic!("expected a refusal, got {other}"),
        }
        assert!(
            parse_oracle(raw.as_bytes(), b"", Some(1))
                .expect_err("must refuse")
                .is_inconclusive_about_mail()
        );
    }

    #[test]
    fn empty_cli_output_is_unreachable_not_zero() {
        let error = parse_oracle(b"", b"am: not found", Some(127)).expect_err("must fail");
        match error {
            MailError::Unreachable { detail, .. } => {
                assert!(detail.contains("no JSON"), "{detail}");
                assert!(detail.contains("am: not found"), "{detail}");
            }
            other => panic!("expected unreachable, got {other}"),
        }
    }
}

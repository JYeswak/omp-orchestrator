//! Every way this binding can fail, as a distinct variant.
//!
//! The governing rule, and the reason this file is long: **an unreachable
//! daemon is an ERROR, never "no mail".** Both conditions produce "zero
//! messages" at a naive call site, and only one of them means the mailbox is
//! empty. Collapsing them is how a dispatch loop goes quiet for hours while
//! reporting healthy.
//!
//! Measured 2026-09-02, the false negative this exists to prevent: `am agent
//! start` reported "no listener on 127.0.0.1:8765" while `curl /health`
//! returned `{"status":"ready",...}` and authenticated tool calls returned
//! live data. Root cause found the same night: the CLI carries no bearer
//! token, `/mcp/` answers 401 to an unauthenticated caller, and the CLI
//! reports that authentication failure as ABSENCE. An auth refusal rendered as
//! "nothing there" is exactly this bug class.
//!
//! Second governing rule: **a timeout is not a verdict.** A bounded wait that
//! elapses maps to [`MailError::TimedOut`], which carries no claim about the
//! mailbox, the message, or the daemon's health. It never maps to the token a
//! genuinely failing subject produces.

use crate::cursor::DeliveryCursor;
use asupersync::Cx;
use asupersync::http::ClientError;
use asupersync::types::CancelKind;
use std::fmt;

/// A failure from the Agent Mail binding.
#[derive(Debug)]
pub enum MailError {
    /// The daemon could not be reached at all: connection refused, DNS
    /// failure, or a transport-level I/O error before any HTTP status.
    ///
    /// This is NEVER an empty inbox, an empty event page, or a zero cursor.
    /// A caller that wants "is there mail?" must handle this variant
    /// separately from an empty success.
    Unreachable {
        /// The endpoint that could not be reached, token redacted.
        endpoint: String,
        /// What the transport reported.
        detail: String,
    },
    /// The daemon answered, but rejected our credential (401/403).
    ///
    /// Distinct from [`MailError::Unreachable`] because the remedy differs
    /// entirely: a token to find versus a daemon to start. Conflating them is
    /// the measured CLI defect described in this module's docs.
    Unauthorized {
        /// The HTTP status the daemon returned.
        status: u16,
    },
    /// No bearer token was discovered, so the request was never attempted.
    ///
    /// Reported before any I/O: guessing an empty token would produce an
    /// `Unauthorized` that looks like a wrong credential rather than a
    /// missing one.
    MissingCredential {
        /// The sources that were searched, in order. Never contains a token.
        searched: Vec<String>,
    },
    /// The bounded wait elapsed. Carries no verdict about the subject.
    TimedOut {
        /// The operation that was bounded.
        operation: String,
    },
    /// The owning region was cancelled for a reason other than a deadline.
    Cancelled(CancelKind),
    /// The daemon answered with an unexpected HTTP status.
    UnexpectedStatus {
        /// The status returned.
        status: u16,
        /// A bounded prefix of the body, for triage.
        body_prefix: String,
    },
    /// The response was not a well-formed JSON-RPC envelope, or the MCP
    /// `content` payload was missing or not the expected shape.
    ///
    /// Measured envelope shape: the tool payload arrives as a JSON-encoded
    /// STRING at `result.content[0].text` and requires a second decode. A
    /// caller that decodes only the outer envelope sees a string where it
    /// expected an object, which is this variant.
    Protocol {
        /// What was expected and what arrived.
        detail: String,
    },
    /// The daemon returned a JSON-RPC error object.
    Rpc {
        /// JSON-RPC error code.
        code: i64,
        /// JSON-RPC error message.
        message: String,
    },
    /// The tool executed and refused, with `isError: true` on the envelope.
    ///
    /// Measured shape: `{"error":{"type":"CURSOR_AHEAD","message":"...",
    /// "recoverable":true,"data":{...}}}`.
    ToolRefused {
        /// The tool that refused.
        tool: String,
        /// The server's own error type string, e.g. `INVALID_AGENT_NAME`.
        kind: String,
        /// The server's human-readable message.
        message: String,
        /// Whether the server marked the condition recoverable.
        recoverable: bool,
    },
    /// A cursor was requested that is beyond the durable tail.
    ///
    /// Promoted out of [`MailError::ToolRefused`] because a monitor must be
    /// able to react to it specifically: the remedy is to re-baseline, not to
    /// retry the same cursor.
    CursorAhead {
        /// The cursor that was requested.
        requested: DeliveryCursor,
        /// The daemon's current durable tail.
        tail: DeliveryCursor,
    },
    /// A cursor was requested that has fallen below retained history, so the
    /// events between it and the oldest retained event are gone.
    ///
    /// A monitor that receives this has a REAL GAP and must say so rather than
    /// silently resuming from the oldest retained position.
    CursorExpired {
        /// The cursor that was requested.
        requested: DeliveryCursor,
        /// The oldest position still retained, when the daemon reported one.
        oldest_available: Option<DeliveryCursor>,
    },
    /// A payload could not be encoded for, or decoded from, the daemon.
    Codec {
        /// Which direction failed and on what field.
        detail: String,
    },
    /// The tool catalogue came back empty.
    ///
    /// Anti-vacuity: a binding that enumerates zero endpoints has NOT verified
    /// the surface, it has failed to. Reporting that as success lets a broken
    /// daemon and a healthy one produce the same green result.
    EmptyCatalogue,
}

impl MailError {
    /// Map a cancelled `Cx` to the right terminal, preserving the
    /// timeout/other split rather than flattening it.
    pub(crate) fn from_cancelled(cx: &Cx, operation: &str) -> Self {
        if cx.any_cause_is(CancelKind::Timeout) || cx.any_cause_is(CancelKind::Deadline) {
            Self::TimedOut {
                operation: operation.to_owned(),
            }
        } else {
            Self::Cancelled(
                cx.cancel_reason()
                    .map_or(CancelKind::User, |reason| reason.kind),
            )
        }
    }

    /// Classify an asupersync HTTP client failure.
    ///
    /// `ConnectError`/`DnsError` become [`MailError::Unreachable`];
    /// `DeadlineExceeded` becomes [`MailError::TimedOut`]. Nothing here can
    /// produce an empty-success.
    pub(crate) fn from_client(error: ClientError, endpoint: &str, operation: &str) -> Self {
        match error {
            ClientError::ConnectError(io) => Self::Unreachable {
                endpoint: endpoint.to_owned(),
                detail: format!("connect: {io}"),
            },
            ClientError::DnsError(io) => Self::Unreachable {
                endpoint: endpoint.to_owned(),
                detail: format!("dns: {io}"),
            },
            ClientError::Io(io) => Self::Unreachable {
                endpoint: endpoint.to_owned(),
                detail: format!("io: {io}"),
            },
            ClientError::DeadlineExceeded => Self::TimedOut {
                operation: operation.to_owned(),
            },
            other => Self::Protocol {
                detail: format!("http client: {other:?}"),
            },
        }
    }

    /// True when the failure says nothing about the mailbox's contents.
    ///
    /// The predicate a dispatch loop needs in order to refuse to report
    /// "no mail" on a transport failure.
    #[must_use]
    pub fn is_inconclusive_about_mail(&self) -> bool {
        match self {
            Self::Unreachable { .. }
            | Self::Unauthorized { .. }
            | Self::MissingCredential { .. }
            | Self::TimedOut { .. }
            | Self::Cancelled(_)
            | Self::UnexpectedStatus { .. }
            | Self::Protocol { .. }
            | Self::Rpc { .. }
            | Self::Codec { .. }
            | Self::EmptyCatalogue
            | Self::CursorExpired { .. }
            | Self::CursorAhead { .. }
            | Self::ToolRefused { .. } => true,
        }
    }
}

impl fmt::Display for MailError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreachable { endpoint, detail } => write!(
                formatter,
                "UNREACHABLE endpoint={endpoint} detail={detail} \
                 (this is NOT an empty inbox)"
            ),
            Self::Unauthorized { status } => write!(
                formatter,
                "UNAUTHORIZED status={status} \
                 (daemon answered and refused the credential; it is running)"
            ),
            Self::MissingCredential { searched } => write!(
                formatter,
                "MISSING_CREDENTIAL searched={} (no request was attempted)",
                searched.join(",")
            ),
            Self::TimedOut { operation } => write!(
                formatter,
                "TIMED_OUT operation={operation} (carries no verdict)"
            ),
            Self::Cancelled(kind) => write!(formatter, "CANCELLED kind={kind:?}"),
            Self::UnexpectedStatus {
                status,
                body_prefix,
            } => write!(
                formatter,
                "UNEXPECTED_STATUS status={status} body={body_prefix}"
            ),
            Self::Protocol { detail } => write!(formatter, "PROTOCOL detail={detail}"),
            Self::Rpc { code, message } => {
                write!(formatter, "RPC_ERROR code={code} message={message}")
            }
            Self::ToolRefused {
                tool,
                kind,
                message,
                recoverable,
            } => write!(
                formatter,
                "TOOL_REFUSED tool={tool} kind={kind} recoverable={recoverable} message={message}"
            ),
            Self::CursorAhead { requested, tail } => write!(
                formatter,
                "CURSOR_AHEAD requested={requested} tail={tail} (re-baseline required)"
            ),
            Self::CursorExpired {
                requested,
                oldest_available,
            } => match oldest_available {
                Some(oldest) => write!(
                    formatter,
                    "CURSOR_EXPIRED requested={requested} oldest_available={oldest} (REAL GAP)"
                ),
                None => write!(
                    formatter,
                    "CURSOR_EXPIRED requested={requested} oldest_available=none (REAL GAP)"
                ),
            },
            Self::Codec { detail } => write!(formatter, "CODEC detail={detail}"),
            Self::EmptyCatalogue => formatter.write_str(
                "EMPTY_CATALOGUE the daemon advertised zero tools \
                 (surface NOT verified; this is a failure, not a pass)",
            ),
        }
    }
}

impl std::error::Error for MailError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_failure_is_inconclusive_about_mail() {
        // The whole point: no failure variant may ever be read as "the
        // mailbox is empty". Enumerated explicitly so a NEW variant that
        // forgets this rule fails to compile in `is_inconclusive_about_mail`.
        let cases = vec![
            MailError::Unreachable {
                endpoint: "http://127.0.0.1:9/mcp/".to_owned(),
                detail: "connect: refused".to_owned(),
            },
            MailError::Unauthorized { status: 401 },
            MailError::MissingCredential { searched: vec![] },
            MailError::TimedOut {
                operation: "fetch_inbox".to_owned(),
            },
            MailError::Cancelled(CancelKind::User),
            MailError::EmptyCatalogue,
        ];
        for case in cases {
            assert!(
                case.is_inconclusive_about_mail(),
                "{case} must not imply an empty mailbox"
            );
        }
    }

    #[test]
    fn unreachable_display_denies_the_empty_inbox_reading() {
        let error = MailError::Unreachable {
            endpoint: "http://127.0.0.1:9/mcp/".to_owned(),
            detail: "connect: Connection refused".to_owned(),
        };
        let rendered = error.to_string();
        assert!(rendered.starts_with("UNREACHABLE"), "got {rendered}");
        assert!(
            rendered.contains("NOT an empty inbox"),
            "the refusal must say what it is not: {rendered}"
        );
    }

    #[test]
    fn timeout_display_carries_no_verdict() {
        let rendered = MailError::TimedOut {
            operation: "fetch_inbox_events".to_owned(),
        }
        .to_string();
        assert!(rendered.contains("carries no verdict"), "got {rendered}");
    }

    #[test]
    fn unauthorized_is_not_unreachable() {
        // The measured CLI defect: an auth failure reported as absence.
        let unauthorized = MailError::Unauthorized { status: 401 };
        let rendered = unauthorized.to_string();
        assert!(
            rendered.contains("it is running"),
            "an auth refusal proves the daemon is UP: {rendered}"
        );
        assert!(
            !rendered.contains("UNREACHABLE"),
            "must not read as unreachable: {rendered}"
        );
    }

    #[test]
    fn empty_catalogue_names_itself_a_failure() {
        let rendered = MailError::EmptyCatalogue.to_string();
        assert!(
            rendered.contains("not a pass"),
            "anti-vacuity must be explicit: {rendered}"
        );
    }
}

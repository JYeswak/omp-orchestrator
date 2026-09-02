//! The JSON-RPC-over-HTTP transport to the Agent Mail MCP daemon.
//!
//! Measured protocol facts, all verified live on 2026-09-02 against
//! `am serve-http` v0.3.31 at `http://127.0.0.1:8765/mcp/`:
//!
//! 1. **The daemon is stateless for tool calls.** A bare `tools/call` succeeds
//!    with no prior `initialize` and no `Mcp-Session-Id` header; `initialize`
//!    returns no session header at all. So there is no session to establish,
//!    keep alive, or lose.
//! 2. **Responses are plain `application/json`**, not SSE, despite the
//!    streamable-HTTP transport advertising `text/event-stream`.
//! 3. **The tool payload is double-encoded.** It arrives as a JSON string at
//!    `result.content[0].text` and needs a second `serde_json` pass. Decoding
//!    only the outer envelope yields a string where an object was expected.
//! 4. **Tool-level refusals ride a 200.** The HTTP status is 200 and the
//!    envelope carries `isError: true` with
//!    `{"error":{"type":...,"message":...,"recoverable":...,"data":{...}}}`
//!    inside the same double-encoded text field. A client that only checks the
//!    HTTP status treats every refusal as a success.

use crate::endpoint::Endpoint;
use crate::error::MailError;
use asupersync::Cx;
use asupersync::http::HttpClient;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Default bound on a single daemon call.
///
/// Every request is bounded. An unbounded call to a local daemon still hangs
/// forever when the daemon is wedged rather than down, which is the failure
/// mode a dispatch loop cannot survive.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

/// The daemon's unauthenticated health report.
#[derive(Debug, Clone, Deserialize)]
pub struct DaemonHealth {
    /// Reported readiness string, e.g. `ready`.
    pub status: String,
    /// Daemon version, e.g. `0.3.31`.
    #[serde(default)]
    pub version: Option<String>,
    /// Registered project count.
    #[serde(default)]
    pub project_count: Option<u64>,
    /// Total message count in the store.
    #[serde(default)]
    pub message_count: Option<u64>,
}

impl DaemonHealth {
    /// True when the daemon reported itself ready.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.status.eq_ignore_ascii_case("ready")
    }
}

/// A native client for the Agent Mail MCP surface.
pub struct MailClient {
    endpoint: Endpoint,
    http: HttpClient,
    next_id: AtomicU64,
    request_timeout: Duration,
}

impl std::fmt::Debug for MailClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MailClient")
            .field("endpoint", &self.endpoint)
            .field("request_timeout", &self.request_timeout)
            .finish_non_exhaustive()
    }
}

impl MailClient {
    /// Build a client for an explicit endpoint.
    #[must_use]
    pub fn new(endpoint: Endpoint) -> Self {
        Self {
            endpoint,
            http: HttpClient::new(),
            next_id: AtomicU64::new(1),
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
        }
    }

    /// Build a client by discovering the endpoint and credential.
    #[must_use]
    pub fn discover() -> Self {
        Self::new(Endpoint::discover())
    }

    /// Override the per-request bound.
    #[must_use]
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// The endpoint this client targets.
    #[must_use]
    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    /// Read the daemon's unauthenticated health route.
    ///
    /// This is the only probe that works without a credential, and it is the
    /// correct way to separate "daemon down" from "credential missing": a
    /// healthy `/health` plus a 401 on `/mcp/` is an AUTH problem, and
    /// reporting it as absence is the measured `am agent start` defect.
    pub async fn health(&self, cx: &Cx) -> Result<DaemonHealth, MailError> {
        cx.checkpoint()
            .map_err(|_| MailError::from_cancelled(cx, "health"))?;
        let url = self.endpoint.health_url();
        let response = self
            .http
            .post(&url)
            .method(asupersync::http::Method::Get)
            .timeout(self.request_timeout)
            .send(cx)
            .await
            .map_err(|error| MailError::from_client(error, &url, "health"))?;

        if response.status != 200 {
            return Err(MailError::UnexpectedStatus {
                status: response.status,
                body_prefix: body_prefix(&response.body),
            });
        }
        serde_json::from_slice(&response.body).map_err(|error| MailError::Codec {
            detail: format!("health response: {error}"),
        })
    }

    /// Enumerate the tools the daemon advertises.
    ///
    /// Refuses an empty catalogue with [`MailError::EmptyCatalogue`]. An
    /// enumeration that returns zero names has not verified the surface; a
    /// caller that accepts it cannot distinguish a healthy daemon from a
    /// broken one, and a test asserting "0 tools" would pass vacuously
    /// forever.
    pub async fn list_tools(&self, cx: &Cx) -> Result<Vec<String>, MailError> {
        let raw = self
            .rpc(cx, "tools/list", json!({}), "tools/list")
            .await?;
        let tools = raw
            .get("tools")
            .and_then(Value::as_array)
            .ok_or_else(|| MailError::Protocol {
                detail: "tools/list result has no `tools` array".to_owned(),
            })?;

        let mut names: Vec<String> = tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .map(str::to_owned)
            .collect();

        if names.is_empty() {
            return Err(MailError::EmptyCatalogue);
        }
        names.sort();
        Ok(names)
    }

    /// Invoke one MCP tool and return its decoded payload.
    ///
    /// Handles the double decode and promotes tool-level refusals — including
    /// the two cursor conditions — into typed variants.
    pub async fn call_tool(
        &self,
        cx: &Cx,
        tool: &str,
        arguments: Value,
    ) -> Result<Value, MailError> {
        let params = json!({ "name": tool, "arguments": arguments });
        let result = self.rpc(cx, "tools/call", params, tool).await?;
        decode_tool_payload(tool, &result)
    }

    /// One JSON-RPC round trip, returning the `result` object.
    async fn rpc(
        &self,
        cx: &Cx,
        method: &str,
        params: Value,
        operation: &str,
    ) -> Result<Value, MailError> {
        cx.checkpoint()
            .map_err(|_| MailError::from_cancelled(cx, operation))?;

        let Some(token) = self.endpoint.token() else {
            return Err(MailError::MissingCredential {
                searched: Endpoint::discovery_sources(),
            });
        };

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let envelope = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let body = serde_json::to_vec(&envelope).map_err(|error| MailError::Codec {
            detail: format!("encoding {method} request: {error}"),
        })?;

        let url = self.endpoint.url();
        let response = self
            .http
            .post(url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {token}"))
            .body(body)
            .timeout(self.request_timeout)
            .send(cx)
            .await
            .map_err(|error| MailError::from_client(error, url, operation))?;

        cx.checkpoint()
            .map_err(|_| MailError::from_cancelled(cx, operation))?;

        match response.status {
            200 => {}
            401 | 403 => {
                return Err(MailError::Unauthorized {
                    status: response.status,
                });
            }
            status => {
                return Err(MailError::UnexpectedStatus {
                    status,
                    body_prefix: body_prefix(&response.body),
                });
            }
        }

        let parsed: Value =
            serde_json::from_slice(&response.body).map_err(|error| MailError::Protocol {
                detail: format!(
                    "{method} response was not JSON: {error} (body starts: {})",
                    body_prefix(&response.body)
                ),
            })?;

        if let Some(error) = parsed.get("error") {
            let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("<no message>")
                .to_owned();
            return Err(MailError::Rpc { code, message });
        }

        parsed
            .get("result")
            .cloned()
            .ok_or_else(|| MailError::Protocol {
                detail: format!("{method} response had neither `result` nor `error`"),
            })
    }
}

/// Decode the double-encoded MCP tool payload and classify refusals.
///
/// Split out of [`MailClient::call_tool`] so the wire-shape handling is
/// testable without a daemon: these are the exact envelopes captured live.
pub(crate) fn decode_tool_payload(tool: &str, result: &Value) -> Result<Value, MailError> {
    let text = result
        .get("content")
        .and_then(Value::as_array)
        .and_then(|content| content.first())
        .and_then(|entry| entry.get("text"))
        .and_then(Value::as_str)
        .ok_or_else(|| MailError::Protocol {
            detail: format!("{tool}: result.content[0].text missing or not a string"),
        })?;

    let payload: Value = serde_json::from_str(text).map_err(|error| MailError::Protocol {
        detail: format!("{tool}: inner payload was not JSON: {error}"),
    })?;

    let is_error = result
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    // A refusal can be signalled by `isError` OR by the payload simply
    // carrying an `error` object. Both shapes were observed, so neither alone
    // is a sufficient check.
    let refusal = payload.get("error").filter(|value| value.is_object());
    match refusal {
        Some(error) => Err(classify_refusal(tool, error)),
        None if is_error => Err(MailError::ToolRefused {
            tool: tool.to_owned(),
            kind: "UNSPECIFIED".to_owned(),
            message: format!("isError was set with no error object: {payload}"),
            recoverable: false,
        }),
        None => Ok(payload),
    }
}

/// Promote the server's error object into the narrowest variant available.
fn classify_refusal(tool: &str, error: &Value) -> MailError {
    let kind = error
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("UNSPECIFIED");
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("<no message>")
        .to_owned();
    let recoverable = error
        .get("recoverable")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let data = error.get("data");

    let cursor_field = |name: &str| -> Option<crate::cursor::DeliveryCursor> {
        data.and_then(|data| data.get(name))
            .and_then(Value::as_u64)
            .map(crate::cursor::DeliveryCursor::new)
    };

    match kind {
        "CURSOR_AHEAD" => MailError::CursorAhead {
            requested: cursor_field("after").unwrap_or(crate::cursor::DeliveryCursor::ORIGIN),
            tail: cursor_field("tail_cursor").unwrap_or(crate::cursor::DeliveryCursor::ORIGIN),
        },
        "CURSOR_EXPIRED" => MailError::CursorExpired {
            requested: cursor_field("after").unwrap_or(crate::cursor::DeliveryCursor::ORIGIN),
            oldest_available: cursor_field("oldest_available_cursor"),
        },
        other => MailError::ToolRefused {
            tool: tool.to_owned(),
            kind: other.to_owned(),
            message,
            recoverable,
        },
    }
}

/// A bounded, single-line prefix of a response body for triage messages.
fn body_prefix(body: &[u8]) -> String {
    const LIMIT: usize = 180;
    let text = String::from_utf8_lossy(body);
    let flattened = text.replace(['\n', '\r'], " ");
    if flattened.chars().count() <= LIMIT {
        return flattened;
    }
    let truncated: String = flattened.chars().take(LIMIT).collect();
    format!("{truncated}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cursor::DeliveryCursor;

    /// The exact envelope captured from the live daemon for a successful
    /// `fetch_inbox_events --position-now` against recipient `GreenFrog`.
    const LIVE_SUCCESS: &str = r#"{"content":[{"type":"text","text":"{\"events\":[],\"next_cursor\":5142,\"has_more\":false,\"oldest_available_cursor\":2108,\"tail_cursor\":5142}"}]}"#;

    /// The exact envelope captured for `after=99999999`.
    const LIVE_CURSOR_AHEAD: &str = r#"{"content":[{"type":"text","text":"{\"error\":{\"type\":\"CURSOR_AHEAD\",\"message\":\"cursor_ahead after=99999999 tail_cursor=5142\",\"recoverable\":true,\"data\":{\"after\":99999999,\"tail_cursor\":5142}}}"}],"isError":true}"#;

    /// The exact envelope captured for a descriptive agent name.
    const LIVE_INVALID_NAME: &str = r#"{"content":[{"type":"text","text":"{\"error\":{\"type\":\"INVALID_AGENT_NAME\",\"message\":\"Invalid agent name format\",\"recoverable\":true,\"data\":{\"provided\":\"AmNative\"}}}"}],"isError":true}"#;

    fn envelope(raw: &str) -> Value {
        serde_json::from_str(raw).expect("fixture must parse")
    }

    #[test]
    fn double_encoded_success_payload_decodes() {
        let payload =
            decode_tool_payload("fetch_inbox_events", &envelope(LIVE_SUCCESS)).expect("success");
        assert_eq!(payload["tail_cursor"], 5142);
        assert_eq!(payload["oldest_available_cursor"], 2108);
        assert!(payload["events"].as_array().expect("events").is_empty());
    }

    #[test]
    fn cursor_ahead_becomes_its_own_variant_not_a_generic_refusal() {
        let error = decode_tool_payload("fetch_inbox_events", &envelope(LIVE_CURSOR_AHEAD))
            .expect_err("must refuse");
        match error {
            MailError::CursorAhead { requested, tail } => {
                assert_eq!(requested, DeliveryCursor::new(99_999_999));
                assert_eq!(tail, DeliveryCursor::new(5142));
            }
            other => panic!("CURSOR_AHEAD must not flatten into {other}"),
        }
    }

    #[test]
    fn unknown_refusal_keeps_the_servers_own_type_string() {
        let error = decode_tool_payload("register_agent", &envelope(LIVE_INVALID_NAME))
            .expect_err("must refuse");
        match error {
            MailError::ToolRefused {
                tool,
                kind,
                recoverable,
                ..
            } => {
                assert_eq!(tool, "register_agent");
                assert_eq!(kind, "INVALID_AGENT_NAME");
                assert!(recoverable);
            }
            other => panic!("expected a tool refusal, got {other}"),
        }
    }

    #[test]
    fn a_refusal_is_never_reported_as_an_empty_success() {
        // The trap this guards: `isError` rides a 200, and the refusal payload
        // has no `events` key. A client that ignored `isError` and defaulted a
        // missing `events` to `[]` would report "no mail" on a hard refusal.
        let error = decode_tool_payload("fetch_inbox_events", &envelope(LIVE_CURSOR_AHEAD))
            .expect_err("must refuse");
        assert!(error.is_inconclusive_about_mail());
    }

    #[test]
    fn missing_content_field_is_a_protocol_error() {
        let error = decode_tool_payload("send_message", &envelope(r#"{"nope":true}"#))
            .expect_err("must fail");
        match error {
            MailError::Protocol { detail } => assert!(detail.contains("content[0].text")),
            other => panic!("expected a protocol error, got {other}"),
        }
    }

    #[test]
    fn single_encoded_payload_is_rejected_rather_than_silently_accepted() {
        // If the daemon ever stops double-encoding, we must notice loudly
        // instead of decoding a string as a record.
        let raw = r#"{"content":[{"type":"text","text":"not json at all"}]}"#;
        let error = decode_tool_payload("health_check", &envelope(raw)).expect_err("must fail");
        match error {
            MailError::Protocol { detail } => assert!(detail.contains("not JSON")),
            other => panic!("expected a protocol error, got {other}"),
        }
    }

    #[test]
    fn health_readiness_matches_the_measured_status_string() {
        let health: DaemonHealth = serde_json::from_str(
            r#"{"status":"ready","version":"0.3.31","project_count":133,"message_count":4982}"#,
        )
        .expect("decode");
        assert!(health.is_ready());
        assert_eq!(health.version.as_deref(), Some("0.3.31"));
        assert_eq!(health.message_count, Some(4982));
    }

    #[test]
    fn body_prefix_is_bounded_and_single_line() {
        let long = vec![b'x'; 4096];
        let prefix = body_prefix(&long);
        assert!(prefix.chars().count() <= 181, "unbounded: {}", prefix.len());
        assert!(!body_prefix(b"a\nb").contains('\n'));
    }
}

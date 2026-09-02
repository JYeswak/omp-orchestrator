//! The dispatch/ack/comms journey as typed operations.
//!
//! Every function takes `&Cx` first, checkpoints before it does work, and is
//! bounded by the client's per-request deadline. Every field name and response
//! shape here was captured from the live daemon on 2026-09-02 rather than
//! inferred from the tool schemas, because several shapes are not what a
//! schema reader would guess:
//!
//! - `fetch_inbox` returns a **bare JSON array**, not an object with a
//!   `messages` key.
//! - `fetch_inbox` **marks messages read by default**, so `mark_read` is an
//!   explicit field here and defaults to `false`. A read that mutates state
//!   is not a read.
//! - `send_message` reports `verified_sender: false` unless a `sender_token`
//!   is supplied, and the token is the `registration_token` handed back at
//!   registration.
//! - `get_message_delivery_receipt` distinguishes **persisted / signaled /
//!   acknowledged** per recipient. Measured live: message 40786 came back
//!   `persisted: true, signaled: false, acknowledged: true` — delivered and
//!   acknowledged while never having been signalled. Collapsing those three
//!   into one boolean destroys exactly the distinction that makes a silent
//!   delivery failure visible.

use crate::client::MailClient;
use crate::cursor::{CursorQuery, DeliveryCursor, MessageId};
use crate::error::MailError;
use asupersync::Cx;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::fmt;

/// A project namespace key. Agent Mail keys projects by absolute path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectKey(String);

impl ProjectKey {
    /// Wrap a project key.
    #[must_use]
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// The raw key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProjectKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// An agent identity within a project.
///
/// Agent Mail refuses descriptive names: measured refusal
/// `INVALID_AGENT_NAME` for `"AmNative"`, with the message "Agent names MUST
/// be randomly generated adjective+noun combinations". So a caller cannot
/// choose a meaningful name, and [`register`] omits the name to let the daemon
/// generate one.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentName(String);

impl AgentName {
    /// Wrap an agent name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// The raw name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The credential that proves a message really came from this agent.
///
/// Handed back once, at registration, as `registration_token`. Redacted in
/// `Debug` for the same reason as the bearer token: a sender credential in a
/// log is a forged-message capability for anyone reading the log.
#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SenderToken(String);

impl SenderToken {
    /// Wrap a sender token.
    #[must_use]
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    /// Expose the token for transmission to the daemon only.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SenderToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SenderToken(<redacted>)")
    }
}

/// An agent identity as the daemon read it back after registration.
///
/// This is the read-back half of "typed register with read-back": the fields
/// are what the daemon actually stored, not what we asked it to store.
#[derive(Debug, Clone, Deserialize)]
pub struct RegisteredAgent {
    /// The daemon's numeric agent id.
    pub id: i64,
    /// The generated agent name.
    pub name: AgentName,
    /// The program string recorded for this agent.
    pub program: String,
    /// The model string recorded for this agent.
    pub model: String,
    /// Whatever task description was stored.
    #[serde(default)]
    pub task_description: Option<String>,
    /// The daemon's project id for the namespace.
    #[serde(default)]
    pub project_id: Option<i64>,
    /// Capabilities the daemon granted this identity.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// The sender credential, present only on a fresh registration.
    #[serde(default)]
    pub registration_token: Option<SenderToken>,
}

/// A request to register an identity.
#[derive(Debug, Clone)]
pub struct RegisterRequest {
    /// Project namespace.
    pub project: ProjectKey,
    /// Program string, e.g. `omp`.
    pub program: String,
    /// Model string.
    pub model: String,
    /// Optional human-facing description, conventionally carrying the
    /// signed FROM / REPLY VIA routing block.
    pub task_description: Option<String>,
}

/// Register an identity and return the daemon's read-back.
///
/// The name is deliberately not settable: the daemon refuses descriptive
/// names, so offering the parameter would only let a caller construct a
/// request that is guaranteed to be refused.
pub async fn register(
    cx: &Cx,
    client: &MailClient,
    request: &RegisterRequest,
) -> Result<RegisteredAgent, MailError> {
    let mut arguments = Map::new();
    arguments.insert(
        "project_key".to_owned(),
        json!(request.project.as_str()),
    );
    arguments.insert("program".to_owned(), json!(request.program));
    arguments.insert("model".to_owned(), json!(request.model));
    if let Some(description) = &request.task_description {
        arguments.insert("task_description".to_owned(), json!(description));
    }

    let payload = client
        .call_tool(cx, "register_agent", Value::Object(arguments))
        .await?;
    decode(&payload, "register_agent")
}

/// Look up one agent's stored profile.
pub async fn whois(
    cx: &Cx,
    client: &MailClient,
    project: &ProjectKey,
    agent: &AgentName,
) -> Result<Value, MailError> {
    client
        .call_tool(
            cx,
            "whois",
            json!({ "project_key": project.as_str(), "agent_name": agent.as_str() }),
        )
        .await
}

/// List the identities registered in a project namespace.
pub async fn list_agents(
    cx: &Cx,
    client: &MailClient,
    project: &ProjectKey,
) -> Result<Vec<AgentName>, MailError> {
    let payload = client
        .call_tool(
            cx,
            "list_agents",
            json!({ "project_key": project.as_str() }),
        )
        .await?;

    let entries = payload
        .as_array()
        .or_else(|| payload.get("agents").and_then(Value::as_array))
        .ok_or_else(|| MailError::Protocol {
            detail: "list_agents returned neither an array nor an `agents` array".to_owned(),
        })?;

    Ok(entries
        .iter()
        .filter_map(|entry| entry.get("name").and_then(Value::as_str))
        .map(AgentName::new)
        .collect())
}

/// How important a message is. Mirrors the daemon's accepted strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Importance {
    /// Routine traffic.
    Normal,
    /// Elevated attention.
    High,
    /// Requires immediate attention.
    Urgent,
}

impl Importance {
    /// The wire string the daemon expects.
    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::High => "high",
            Self::Urgent => "urgent",
        }
    }
}

/// A message to send.
#[derive(Debug, Clone)]
pub struct SendRequest {
    /// Project namespace.
    pub project: ProjectKey,
    /// The sending identity.
    pub sender: AgentName,
    /// Primary recipients.
    pub to: Vec<AgentName>,
    /// Subject line.
    pub subject: String,
    /// Markdown body. Conventionally opens with the signed FROM / REPLY VIA
    /// block so a recipient can answer without guessing a transport.
    pub body_md: String,
    /// Whether the recipient must acknowledge.
    pub ack_required: bool,
    /// Message importance.
    pub importance: Importance,
    /// Optional thread to attach to.
    pub thread_id: Option<i64>,
    /// The sender credential. Supplying it is what makes the daemon report
    /// `verified_sender: true`; omitting it yields an unverified message that
    /// anyone could have forged.
    pub sender_token: Option<SenderToken>,
}

impl SendRequest {
    /// Build a minimal, normal-importance, ack-required message.
    #[must_use]
    pub fn new(
        project: ProjectKey,
        sender: AgentName,
        to: Vec<AgentName>,
        subject: impl Into<String>,
        body_md: impl Into<String>,
    ) -> Self {
        Self {
            project,
            sender,
            to,
            subject: subject.into(),
            body_md: body_md.into(),
            ack_required: true,
            importance: Importance::Normal,
            thread_id: None,
            sender_token: None,
        }
    }

    /// Attach a sender credential so the daemon can verify the sender.
    #[must_use]
    pub fn signed_by(mut self, token: SenderToken) -> Self {
        self.sender_token = Some(token);
        self
    }
}

/// What the daemon recorded for one delivered copy.
#[derive(Debug, Clone, Deserialize)]
pub struct DeliveredPayload {
    /// The message's durable id.
    pub id: MessageId,
    /// Sending agent name.
    #[serde(default, rename = "from")]
    pub from_agent: Option<String>,
    /// Recipients.
    #[serde(default)]
    pub to: Vec<String>,
    /// Subject as stored.
    #[serde(default)]
    pub subject: Option<String>,
    /// Creation timestamp.
    #[serde(default)]
    pub created_ts: Option<String>,
    /// Whether an acknowledgement is required.
    #[serde(default)]
    pub ack_required: bool,
}

/// The receipt for a send.
#[derive(Debug, Clone, Deserialize)]
pub struct SendReceipt {
    /// One entry per delivered copy.
    #[serde(default)]
    pub deliveries: Vec<Delivery>,
    /// How many copies were delivered.
    #[serde(default)]
    pub count: u64,
    /// Whether the daemon could verify the sender's identity.
    ///
    /// `false` means the message was accepted but its `from` is unproven.
    #[serde(default)]
    pub verified_sender: bool,
}

/// One delivered copy.
#[derive(Debug, Clone, Deserialize)]
pub struct Delivery {
    /// The project the copy landed in.
    #[serde(default)]
    pub project: Option<String>,
    /// The stored message.
    pub payload: DeliveredPayload,
}

impl SendReceipt {
    /// The id of the first delivered copy, which is the id a caller acks.
    #[must_use]
    pub fn message_id(&self) -> Option<MessageId> {
        self.deliveries.first().map(|entry| entry.payload.id)
    }
}

/// Send a message.
pub async fn send(
    cx: &Cx,
    client: &MailClient,
    request: &SendRequest,
) -> Result<SendReceipt, MailError> {
    let recipients: Vec<&str> = request.to.iter().map(AgentName::as_str).collect();
    let mut arguments = Map::new();
    arguments.insert("project_key".to_owned(), json!(request.project.as_str()));
    arguments.insert("sender_name".to_owned(), json!(request.sender.as_str()));
    arguments.insert("to".to_owned(), json!(recipients));
    arguments.insert("subject".to_owned(), json!(request.subject));
    arguments.insert("body_md".to_owned(), json!(request.body_md));
    arguments.insert("ack_required".to_owned(), json!(request.ack_required));
    arguments.insert(
        "importance".to_owned(),
        json!(request.importance.as_wire()),
    );
    if let Some(thread) = request.thread_id {
        arguments.insert("thread_id".to_owned(), json!(thread));
    }
    if let Some(token) = &request.sender_token {
        arguments.insert("sender_token".to_owned(), json!(token.expose()));
    }

    let payload = client
        .call_tool(cx, "send_message", Value::Object(arguments))
        .await?;
    decode(&payload, "send_message")
}

/// Reply to a message, preserving or establishing its thread.
pub async fn reply(
    cx: &Cx,
    client: &MailClient,
    project: &ProjectKey,
    sender: &AgentName,
    message_id: MessageId,
    body_md: &str,
    ack_required: bool,
) -> Result<SendReceipt, MailError> {
    let payload = client
        .call_tool(
            cx,
            "reply_message",
            json!({
                "project_key": project.as_str(),
                "sender_name": sender.as_str(),
                "message_id": message_id.get(),
                "body_md": body_md,
                "ack_required": ack_required,
            }),
        )
        .await?;
    decode(&payload, "reply_message")
}

/// A request to read an inbox.
#[derive(Debug, Clone)]
pub struct InboxRequest {
    /// Project namespace.
    pub project: ProjectKey,
    /// Whose inbox.
    pub agent: AgentName,
    /// Include message bodies.
    pub include_bodies: bool,
    /// Maximum messages to return.
    pub limit: Option<u32>,
    /// Only unread messages.
    pub unread_only: bool,
    /// Whether reading should MARK the returned messages read.
    ///
    /// Explicit and defaulting to `false`, because the daemon's own default is
    /// `true` and a monitor that polls an inbox would otherwise silently
    /// consume the unread state other tooling depends on.
    pub mark_read: bool,
}

impl InboxRequest {
    /// A non-mutating read with bodies.
    #[must_use]
    pub fn read_only(project: ProjectKey, agent: AgentName) -> Self {
        Self {
            project,
            agent,
            include_bodies: true,
            limit: None,
            unread_only: false,
            mark_read: false,
        }
    }
}

/// One message as stored in an inbox.
#[derive(Debug, Clone, Deserialize)]
pub struct InboxMessage {
    /// Durable message id.
    pub id: MessageId,
    /// Subject line.
    #[serde(default)]
    pub subject: Option<String>,
    /// Sending agent.
    #[serde(default, rename = "from")]
    pub from_agent: Option<String>,
    /// Delivery kind, e.g. `to` or `cc`.
    #[serde(default)]
    pub kind: Option<String>,
    /// Whether an acknowledgement is required.
    #[serde(default)]
    pub ack_required: bool,
    /// Creation timestamp.
    #[serde(default)]
    pub created_ts: Option<String>,
    /// When this recipient read the message, if ever.
    ///
    /// The reconciliation key: a delivery event exists for every delivered
    /// message, but only a read sets `read_ts`.
    #[serde(default)]
    pub read_ts: Option<String>,
    /// Body markdown, when requested.
    #[serde(default)]
    pub body_md: Option<String>,
}

impl InboxMessage {
    /// True when this recipient has never read the message.
    #[must_use]
    pub fn is_unread(&self) -> bool {
        self.read_ts.is_none()
    }
}

/// Read an inbox.
pub async fn fetch_inbox(
    cx: &Cx,
    client: &MailClient,
    request: &InboxRequest,
) -> Result<Vec<InboxMessage>, MailError> {
    let mut arguments = Map::new();
    arguments.insert("project_key".to_owned(), json!(request.project.as_str()));
    arguments.insert("agent_name".to_owned(), json!(request.agent.as_str()));
    arguments.insert("include_bodies".to_owned(), json!(request.include_bodies));
    arguments.insert("unread_only".to_owned(), json!(request.unread_only));
    arguments.insert("mark_read".to_owned(), json!(request.mark_read));
    if let Some(limit) = request.limit {
        arguments.insert("limit".to_owned(), json!(limit));
    }

    let payload = client
        .call_tool(cx, "fetch_inbox", Value::Object(arguments))
        .await?;

    // Measured: a bare array, not `{"messages": [...]}`. Both are accepted
    // here so a daemon that grows an envelope does not break the binding.
    let entries = payload
        .as_array()
        .cloned()
        .or_else(|| {
            payload
                .get("messages")
                .and_then(Value::as_array)
                .cloned()
        })
        .ok_or_else(|| MailError::Protocol {
            detail: "fetch_inbox returned neither an array nor a `messages` array".to_owned(),
        })?;

    serde_json::from_value(Value::Array(entries)).map_err(|error| MailError::Codec {
        detail: format!("decoding inbox messages: {error}"),
    })
}

/// One durable delivery event.
#[derive(Debug, Clone, Deserialize)]
pub struct DeliveryEvent {
    /// This event's position in the delivery sequence.
    pub cursor: DeliveryCursor,
    /// The message the event refers to. NOT the cursor.
    pub message_id: MessageId,
    /// Delivery kind, e.g. `to` or `cc`.
    #[serde(default)]
    pub kind: Option<String>,
    /// When the copy was delivered.
    #[serde(default)]
    pub delivered_ts: Option<String>,
    /// Subject, carried for triage without a body fetch.
    #[serde(default)]
    pub subject: Option<String>,
    /// Sending agent.
    #[serde(default, rename = "from")]
    pub from_agent: Option<String>,
    /// Whether an acknowledgement is required.
    #[serde(default)]
    pub ack_required: bool,
}

/// One page of the durable delivery log.
#[derive(Debug, Clone, Deserialize)]
pub struct EventPage {
    /// Events in this page, oldest first.
    #[serde(default)]
    pub events: Vec<DeliveryEvent>,
    /// The cursor to persist AFTER processing every event in this page.
    pub next_cursor: DeliveryCursor,
    /// Whether more events are already available.
    #[serde(default)]
    pub has_more: bool,
    /// The oldest position still retained, when the daemon reported one.
    #[serde(default)]
    pub oldest_available_cursor: Option<DeliveryCursor>,
    /// This recipient's high-water mark in the sequence.
    pub tail_cursor: DeliveryCursor,
}

/// Read the durable delivery log for one recipient.
///
/// The [`CursorQuery`] enum makes the daemon's "`position_now` cannot be
/// combined with `after`" rule unrepresentable rather than merely documented.
pub async fn fetch_inbox_events(
    cx: &Cx,
    client: &MailClient,
    project: &ProjectKey,
    agent: &AgentName,
    query: CursorQuery,
    limit: Option<u32>,
) -> Result<EventPage, MailError> {
    let mut arguments = Map::new();
    arguments.insert("project_key".to_owned(), json!(project.as_str()));
    arguments.insert("agent_name".to_owned(), json!(agent.as_str()));
    if let Some(limit) = limit {
        arguments.insert("limit".to_owned(), json!(limit));
    }
    match query {
        CursorQuery::PositionNow => {
            arguments.insert("position_now".to_owned(), json!(true));
        }
        CursorQuery::After(cursor) => {
            arguments.insert("after".to_owned(), json!(cursor.get()));
        }
        CursorQuery::FromOldestRetained => {}
    }

    let payload = client
        .call_tool(cx, "fetch_inbox_events", Value::Object(arguments))
        .await?;
    decode(&payload, "fetch_inbox_events")
}

/// Verify that a page fetched with `after: stored` really contains every
/// event the caller had not yet processed.
///
/// **This guard exists because the daemon does not enforce its own documented
/// contract.** The `fetch_inbox_events` description states that "a cursor
/// below retained history produces `CURSOR_EXPIRED`". Measured 2026-09-02
/// against recipient `GreenFrog`, whose oldest retained position was 2108:
/// requesting `after: 1` did NOT refuse. It returned a normal success page
/// beginning at cursor 2108, silently skipping everything between. A caller
/// that trusted the refusal contract would have recorded a clean read over a
/// gap it never saw.
///
/// So continuity is checked client-side. When `stored` sits below the oldest
/// retained position, this refuses with
/// [`MailError::CursorExpired`](crate::MailError::CursorExpired).
///
/// Note the honest limit of what this can prove: because the delivery
/// sequence is GLOBAL and a single recipient's events are sparse within it
/// (measured: `GreenFrog` events at 2108, 2109, 2126 — non-contiguous),
/// a cursor below `oldest_available_cursor` cannot be distinguished from a
/// recipient whose first event simply arrived later. Both readings mean the
/// same thing for a resuming monitor: **continuity is unprovable, so do not
/// claim it.** Refusing is correct in both cases.
pub fn verify_resume_continuity(
    stored: DeliveryCursor,
    page: &EventPage,
) -> Result<(), MailError> {
    match page.oldest_available_cursor {
        Some(oldest) if oldest.advanced_beyond(stored) => Err(MailError::CursorExpired {
            requested: stored,
            oldest_available: Some(oldest),
        }),
        Some(_) | None => Ok(()),
    }
}

/// A delivery position bound to the recipient it was read for.
///
/// **A cursor without its recipient is half a value.** Measured 2026-09-02,
/// both true at the same instant: the circulated baseline `5105` is
/// unresumable for `SnowyCanyon` (whose floor is 5147) and perfectly
/// resumable for `GreenFrog` (whose floor is 2108). Four recipients measured
/// together had floors spread across 2058-5147. So "the delivery cursor" is
/// not a quantity the system has one of, and circulating a bare integer as
/// one is what produced an hour of contradictory readings across five agents.
///
/// This type makes that mistake unrepresentable: [`resume_from`] takes a
/// `ResumePoint`, so the project and recipient come FROM the stored position
/// and cannot be paired with someone else's cursor by a caller who has only
/// an integer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumePoint {
    project: ProjectKey,
    recipient: AgentName,
    cursor: DeliveryCursor,
}

impl ResumePoint {
    /// Capture the position to resume from after processing a page.
    ///
    /// The ordinary constructor, and the safe one: the recipient is the one
    /// the page was read for, so the pairing is correct by construction. Call
    /// it only once every event in `page` has been processed — `next_cursor`
    /// is a commitment that everything up to it is done.
    #[must_use]
    pub fn after_processing(
        project: &ProjectKey,
        recipient: &AgentName,
        page: &EventPage,
    ) -> Self {
        Self {
            project: project.clone(),
            recipient: recipient.clone(),
            cursor: page.next_cursor,
        }
    }

    /// Rebuild a resume point loaded from persistence.
    ///
    /// Named to make the provenance obvious at the call site: the caller is
    /// asserting this integer was stored for THIS recipient. That assertion
    /// is still checked at read time by [`verify_resume_continuity`], which
    /// refuses a position below the recipient's floor.
    #[must_use]
    pub fn restored(project: ProjectKey, recipient: AgentName, cursor: DeliveryCursor) -> Self {
        Self {
            project,
            recipient,
            cursor,
        }
    }

    /// The project namespace.
    #[must_use]
    pub fn project(&self) -> &ProjectKey {
        &self.project
    }

    /// The recipient this position belongs to.
    #[must_use]
    pub fn recipient(&self) -> &AgentName {
        &self.recipient
    }

    /// The bare position, for persistence only.
    #[must_use]
    pub fn cursor(&self) -> DeliveryCursor {
        self.cursor
    }
}

/// Resume a durable read from a persisted position, refusing a silent gap.
///
/// The restart-safe entry point: fetch everything after the stored position,
/// then prove the page is continuous with it. Prefer this over
/// [`fetch_inbox_events`] with [`CursorQuery::After`] in any monitor that
/// persists its position across restarts.
pub async fn resume_from(
    cx: &Cx,
    client: &MailClient,
    from: &ResumePoint,
    limit: Option<u32>,
) -> Result<EventPage, MailError> {
    let page = fetch_inbox_events(
        cx,
        client,
        from.project(),
        from.recipient(),
        CursorQuery::After(from.cursor()),
        limit,
    )
    .await?;
    verify_resume_continuity(from.cursor(), &page)?;
    Ok(page)
}

/// The receipt for an acknowledgement.
#[derive(Debug, Clone, Deserialize)]
pub struct AckReceipt {
    /// The message that was acknowledged.
    pub message_id: MessageId,
    /// Whether the daemon recorded the acknowledgement.
    #[serde(default)]
    pub acknowledged: bool,
    /// When it was acknowledged.
    #[serde(default)]
    pub acknowledged_at: Option<String>,
    /// When it had been read, if ever.
    #[serde(default)]
    pub read_at: Option<String>,
}

/// Acknowledge a message.
pub async fn acknowledge(
    cx: &Cx,
    client: &MailClient,
    project: &ProjectKey,
    agent: &AgentName,
    message_id: MessageId,
) -> Result<AckReceipt, MailError> {
    let payload = client
        .call_tool(
            cx,
            "acknowledge_message",
            json!({
                "project_key": project.as_str(),
                "agent_name": agent.as_str(),
                "message_id": message_id.get(),
            }),
        )
        .await?;
    decode(&payload, "acknowledge_message")
}

/// Per-recipient delivery facts for one message.
#[derive(Debug, Clone, Deserialize)]
pub struct RecipientReceipt {
    /// The recipient's agent name.
    pub recipient: String,
    /// Delivery kind.
    #[serde(default)]
    pub kind: Option<String>,
    /// The copy exists durably.
    #[serde(default)]
    pub persisted: bool,
    /// A message-id-bound signal receipt was appended after a successful
    /// signal write. A debounced or failed signal stays persisted but
    /// unsignalled — which is the silent-failure shape.
    #[serde(default)]
    pub signaled: bool,
    /// The recipient acknowledged.
    #[serde(default)]
    pub acknowledged: bool,
    /// When it was acknowledged.
    #[serde(default)]
    pub acknowledged_at: Option<String>,
}

impl RecipientReceipt {
    /// True when the copy is durable but was never signalled.
    ///
    /// The measured case: message 40786 was `persisted` and `acknowledged`
    /// with `signaled: false`. A monitor that treats "persisted" as
    /// "notified" cannot see this.
    #[must_use]
    pub fn is_persisted_but_unsignalled(&self) -> bool {
        self.persisted && !self.signaled
    }
}

/// Durable delivery facts for one message.
#[derive(Debug, Clone, Deserialize)]
pub struct DeliveryReceipt {
    /// The message.
    pub message_id: MessageId,
    /// Whether the message itself is durable.
    #[serde(default)]
    pub persisted: bool,
    /// When it was persisted.
    #[serde(default)]
    pub persisted_at: Option<String>,
    /// Per-recipient facts.
    #[serde(default)]
    pub recipients: Vec<RecipientReceipt>,
}

/// Read durable delivery facts for one message: the failure/queue status
/// surface.
pub async fn delivery_receipt(
    cx: &Cx,
    client: &MailClient,
    project: &ProjectKey,
    message_id: MessageId,
) -> Result<DeliveryReceipt, MailError> {
    let payload = client
        .call_tool(
            cx,
            "get_message_delivery_receipt",
            json!({
                "project_key": project.as_str(),
                "message_id": message_id.get(),
            }),
        )
        .await?;
    decode(&payload, "get_message_delivery_receipt")
}

/// A granted file reservation.
#[derive(Debug, Clone, Deserialize)]
pub struct ReservationGrant {
    /// The reservation's id, used to renew or release it precisely.
    pub id: i64,
    /// The path pattern reserved.
    pub path_pattern: String,
    /// Whether the reservation is exclusive.
    #[serde(default)]
    pub exclusive: bool,
    /// Why it was taken.
    #[serde(default)]
    pub reason: Option<String>,
    /// When it lapses.
    #[serde(default)]
    pub expires_ts: Option<String>,
}

/// The result of a reservation request.
#[derive(Debug, Clone, Deserialize)]
pub struct ReservationResult {
    /// Reservations that were granted.
    #[serde(default)]
    pub granted: Vec<ReservationGrant>,
    /// Conflicting reservations held by others.
    #[serde(default)]
    pub conflicts: Vec<Value>,
}

impl ReservationResult {
    /// True when nothing conflicted.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.conflicts.is_empty()
    }
}

/// Reserve paths for exclusive or shared work.
pub async fn reserve_paths(
    cx: &Cx,
    client: &MailClient,
    project: &ProjectKey,
    agent: &AgentName,
    paths: &[String],
    exclusive: bool,
    ttl_seconds: u64,
    reason: &str,
) -> Result<ReservationResult, MailError> {
    let payload = client
        .call_tool(
            cx,
            "file_reservation_paths",
            json!({
                "project_key": project.as_str(),
                "agent_name": agent.as_str(),
                "paths": paths,
                "exclusive": exclusive,
                "ttl_seconds": ttl_seconds,
                "reason": reason,
            }),
        )
        .await?;
    decode(&payload, "file_reservation_paths")
}

/// Extend reservations this agent already holds.
pub async fn renew_reservations(
    cx: &Cx,
    client: &MailClient,
    project: &ProjectKey,
    agent: &AgentName,
    extend_seconds: u64,
) -> Result<Value, MailError> {
    client
        .call_tool(
            cx,
            "renew_file_reservations",
            json!({
                "project_key": project.as_str(),
                "agent_name": agent.as_str(),
                "extend_seconds": extend_seconds,
            }),
        )
        .await
}

/// Release reservations, optionally narrowed to specific paths.
pub async fn release_reservations(
    cx: &Cx,
    client: &MailClient,
    project: &ProjectKey,
    agent: &AgentName,
    paths: Option<&[String]>,
) -> Result<Value, MailError> {
    let mut arguments = Map::new();
    arguments.insert("project_key".to_owned(), json!(project.as_str()));
    arguments.insert("agent_name".to_owned(), json!(agent.as_str()));
    if let Some(paths) = paths {
        arguments.insert("paths".to_owned(), json!(paths));
    }
    client
        .call_tool(cx, "release_file_reservations", Value::Object(arguments))
        .await
}

/// Check whether paths would conflict, without taking a reservation.
pub async fn check_conflicts(
    cx: &Cx,
    client: &MailClient,
    project: &ProjectKey,
    agent: &AgentName,
    paths: &[String],
) -> Result<Value, MailError> {
    client
        .call_tool(
            cx,
            "check_file_reservation_conflicts",
            json!({
                "project_key": project.as_str(),
                "agent_name": agent.as_str(),
                "paths": paths,
            }),
        )
        .await
}

/// One delivered message whose inbox state disagrees with its delivery event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unreconciled {
    /// The message.
    pub message_id: MessageId,
    /// Where its delivery event sits in the sequence.
    pub cursor: DeliveryCursor,
    /// Whether the inbox has a `read_ts` for it.
    pub read: bool,
    /// Whether the message demands an acknowledgement.
    pub ack_required: bool,
    /// Whether the inbox row was found at all.
    pub present_in_inbox: bool,
}

/// Reconcile the durable delivery log against inbox read state.
///
/// The two surfaces answer different questions and can disagree: a delivery
/// event proves a copy was WRITTEN, while `read_ts` proves it was SEEN. This
/// returns the events whose messages are either absent from the inbox read or
/// present and unread, which is the backlog a dispatch loop actually owes work
/// on.
///
/// Pure, so the reconciliation rule is testable without a daemon.
#[must_use]
pub fn reconcile(events: &[DeliveryEvent], inbox: &[InboxMessage]) -> Vec<Unreconciled> {
    events
        .iter()
        .filter_map(|event| {
            let found = inbox
                .iter()
                .find(|message| message.id == event.message_id);
            match found {
                Some(message) if !message.is_unread() => None,
                Some(message) => Some(Unreconciled {
                    message_id: event.message_id,
                    cursor: event.cursor,
                    read: false,
                    ack_required: message.ack_required,
                    present_in_inbox: true,
                }),
                None => Some(Unreconciled {
                    message_id: event.message_id,
                    cursor: event.cursor,
                    read: false,
                    ack_required: event.ack_required,
                    present_in_inbox: false,
                }),
            }
        })
        .collect()
}

/// Decode a payload into a concrete type with a naming error message.
fn decode<T: serde::de::DeserializeOwned>(payload: &Value, tool: &str) -> Result<T, MailError> {
    serde_json::from_value(payload.clone()).map_err(|error| MailError::Codec {
        detail: format!("decoding {tool} payload: {error}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(cursor: u64, message: i64, ack: bool) -> DeliveryEvent {
        DeliveryEvent {
            cursor: DeliveryCursor::new(cursor),
            message_id: MessageId::new(message),
            kind: Some("to".to_owned()),
            delivered_ts: None,
            subject: None,
            from_agent: None,
            ack_required: ack,
        }
    }

    fn message(id: i64, read: Option<&str>, ack: bool) -> InboxMessage {
        InboxMessage {
            id: MessageId::new(id),
            subject: None,
            from_agent: None,
            kind: None,
            ack_required: ack,
            created_ts: None,
            read_ts: read.map(str::to_owned),
            body_md: None,
        }
    }

    #[test]
    fn a_read_message_is_reconciled() {
        let events = vec![event(5157, 40786, true)];
        let inbox = vec![message(40786, Some("2026-09-02T05:10:25Z"), true)];
        assert!(reconcile(&events, &inbox).is_empty());
    }

    #[test]
    fn an_unread_delivered_message_is_surfaced() {
        let events = vec![event(5157, 40786, true)];
        let inbox = vec![message(40786, None, true)];
        let pending = reconcile(&events, &inbox);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].message_id, MessageId::new(40786));
        assert_eq!(pending[0].cursor, DeliveryCursor::new(5157));
        assert!(pending[0].present_in_inbox);
    }

    #[test]
    fn a_delivered_message_absent_from_the_inbox_is_surfaced_distinctly() {
        // The silent-loss shape: the delivery log says a copy exists, the
        // inbox read does not return it. That is NOT the same as unread and
        // must not be reported as reconciled.
        let events = vec![event(5157, 40786, true)];
        let pending = reconcile(&events, &[]);
        assert_eq!(pending.len(), 1);
        assert!(
            !pending[0].present_in_inbox,
            "absence from the inbox must be distinguishable from unread"
        );
    }

    #[test]
    fn reconciliation_matches_on_message_id_not_cursor() {
        // Cursor 5157 and message 40786 are different integers for the same
        // event. Matching on the wrong one silently reconciles nothing.
        let events = vec![event(40786, 5157, true)];
        let inbox = vec![message(40786, Some("t"), true)];
        let pending = reconcile(&events, &inbox);
        assert_eq!(
            pending.len(),
            1,
            "message 5157 is unread; matching by cursor would wrongly clear it"
        );
    }

    #[test]
    fn inbox_read_defaults_to_non_mutating() {
        let request = InboxRequest::read_only(
            ProjectKey::new("/tmp/project"),
            AgentName::new("BrightGorge"),
        );
        assert!(
            !request.mark_read,
            "a read must not consume unread state by default"
        );
    }

    #[test]
    fn sender_token_is_redacted_in_debug() {
        let token = SenderToken::new("JCPK3Kii4-rXpN7C4RZ47us8LS1RSdlwQFhtelWyIUg");
        let rendered = format!("{token:?}");
        assert!(
            !rendered.contains("JCPK3Kii4"),
            "sender credential leaked: {rendered}"
        );
        assert!(rendered.contains("<redacted>"));
    }

    #[test]
    fn send_request_is_unsigned_until_signed_by() {
        let request = SendRequest::new(
            ProjectKey::new("/tmp/project"),
            AgentName::new("BrightGorge"),
            vec![AgentName::new("BrightGorge")],
            "subject",
            "body",
        );
        assert!(request.sender_token.is_none());
        let signed = request.signed_by(SenderToken::new("tok"));
        assert!(signed.sender_token.is_some());
    }

    #[test]
    fn send_receipt_decodes_the_measured_shape() {
        // Captured live from send_message.
        let raw = r#"{"deliveries":[{"project":"/example/project","payload":{"id":40786,"subject":"shape probe","ack_required":true,"from":"BrightGorge","to":["BrightGorge"],"created_ts":"2026-09-02T05:10:08.742609Z"}}],"count":1,"verified_sender":false}"#;
        let receipt: SendReceipt = serde_json::from_str(raw).expect("decode");
        assert_eq!(receipt.count, 1);
        assert_eq!(receipt.message_id(), Some(MessageId::new(40786)));
        assert!(
            !receipt.verified_sender,
            "an unsigned send must report verified_sender false"
        );
    }

    #[test]
    fn delivery_receipt_separates_persisted_from_signaled() {
        // Captured live for message 40786: durable and acknowledged, never
        // signalled.
        let raw = r#"{"message_id":40786,"persisted":true,"persisted_at":"2026-09-02T05:10:08Z","recipients":[{"recipient":"BrightGorge","kind":"to","persisted":true,"signaled":false,"acknowledged":true,"acknowledged_at":"2026-09-02T05:10:41Z"}]}"#;
        let receipt: DeliveryReceipt = serde_json::from_str(raw).expect("decode");
        assert!(receipt.persisted);
        let recipient = &receipt.recipients[0];
        assert!(recipient.acknowledged, "it was acknowledged");
        assert!(!recipient.signaled, "and never signalled");
        assert!(
            recipient.is_persisted_but_unsignalled(),
            "the silent-notification shape must be detectable"
        );
    }

    #[test]
    fn event_page_decodes_the_measured_shape_including_null_oldest() {
        let raw = r#"{"events":[],"next_cursor":0,"has_more":false,"oldest_available_cursor":null,"tail_cursor":0}"#;
        let page: EventPage = serde_json::from_str(raw).expect("decode");
        assert_eq!(page.tail_cursor, DeliveryCursor::ORIGIN);
        assert!(
            page.oldest_available_cursor.is_none(),
            "a never-delivered recipient reports a null oldest cursor"
        );
    }

    #[test]
    fn importance_wire_strings_are_lowercase() {
        assert_eq!(Importance::Normal.as_wire(), "normal");
        assert_eq!(Importance::Urgent.as_wire(), "urgent");
    }

    fn page_with(tail: u64, oldest: Option<u64>) -> EventPage {
        EventPage {
            events: Vec::new(),
            next_cursor: DeliveryCursor::new(tail),
            has_more: false,
            oldest_available_cursor: oldest.map(DeliveryCursor::new),
            tail_cursor: DeliveryCursor::new(tail),
        }
    }

    #[test]
    fn a_cursor_at_or_above_the_floor_is_continuous() {
        // GreenFrog's measured floor is 2108. The circulated 5105 baseline is
        // perfectly resumable for that recipient.
        let page = page_with(5161, Some(2108));
        assert!(verify_resume_continuity(DeliveryCursor::new(5105), &page).is_ok());
        // Exactly at the floor is still continuous: nothing below it was owed.
        assert!(verify_resume_continuity(DeliveryCursor::new(2108), &page).is_ok());
    }

    #[test]
    fn a_cursor_below_the_floor_refuses_instead_of_silently_clamping() {
        // The measured daemon defect: `after: 1` against GreenFrog returned a
        // success page starting at 2108, skipping everything between, despite
        // its own docs promising CURSOR_EXPIRED. The guard restores the
        // documented refusal.
        let page = page_with(5161, Some(2108));
        match verify_resume_continuity(DeliveryCursor::new(1), &page).expect_err("must refuse") {
            MailError::CursorExpired {
                requested,
                oldest_available,
            } => {
                assert_eq!(requested, DeliveryCursor::new(1));
                assert_eq!(oldest_available, Some(DeliveryCursor::new(2108)));
            }
            other => panic!("expected CursorExpired, got {other}"),
        }
    }

    #[test]
    fn the_5105_cross_recipient_case_refuses_for_the_recipient_that_started_later() {
        // Both true at the same instant, which is why a bare integer is not a
        // resume point: 5105 resumes GreenFrog (floor 2108) and cannot resume
        // SnowyCanyon (floor 5147).
        let green = page_with(5161, Some(2108));
        let snowy = page_with(5164, Some(5147));
        let stored = DeliveryCursor::new(5105);
        assert!(verify_resume_continuity(stored, &green).is_ok());
        assert!(verify_resume_continuity(stored, &snowy).is_err());
    }

    #[test]
    fn a_null_floor_cannot_disprove_continuity() {
        // A never-delivered recipient reports `oldest_available_cursor: null`
        // (measured: SnowyCanyon at 05:02, tail 0). With no floor there is
        // nothing to contradict, so the read is accepted.
        let page = page_with(0, None);
        assert!(verify_resume_continuity(DeliveryCursor::ORIGIN, &page).is_ok());
    }

    #[test]
    fn a_resume_point_carries_its_recipient() {
        let project = ProjectKey::new("/example/project");
        let recipient = AgentName::new("GreenFrog");
        let point =
            ResumePoint::after_processing(&project, &recipient, &page_with(5161, Some(2108)));
        assert_eq!(point.cursor(), DeliveryCursor::new(5161));
        assert_eq!(point.recipient().as_str(), "GreenFrog");
        assert_eq!(point.project().as_str(), project.as_str());
    }

    #[test]
    fn resume_points_for_different_recipients_are_not_equal_at_the_same_cursor() {
        // The class this type kills: one integer, two recipients, two
        // different meanings.
        let project = ProjectKey::new("/p");
        let cursor = DeliveryCursor::new(5105);
        let green = ResumePoint::restored(project.clone(), AgentName::new("GreenFrog"), cursor);
        let snowy = ResumePoint::restored(project, AgentName::new("SnowyCanyon"), cursor);
        assert_ne!(
            green, snowy,
            "the same position for two recipients must not compare equal"
        );
    }

    #[test]
    fn after_processing_commits_to_next_cursor_not_the_last_event() {
        // `next_cursor` is the daemon's "everything up to here is yours";
        // persisting an event's own cursor instead would replay that event.
        let project = ProjectKey::new("/p");
        let recipient = AgentName::new("GreenFrog");
        let mut page = page_with(5161, Some(2108));
        page.events.push(event(5160, 40_000, false));
        page.next_cursor = DeliveryCursor::new(5161);
        let point = ResumePoint::after_processing(&project, &recipient, &page);
        assert_eq!(point.cursor(), DeliveryCursor::new(5161));
        assert!(
            point.cursor().advanced_beyond(page.events[0].cursor),
            "the persisted position must be past every processed event"
        );
    }
}

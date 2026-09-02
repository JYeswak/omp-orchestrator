#![forbid(unsafe_code)]

//! Native typed binding to Agent Mail's authenticated MCP HTTP surface, for
//! the dispatch / ack / comms journey.
//!
//! # Why this crate exists
//!
//! Measured 2026-09-02 in this repository: **zero crates touched Agent Mail.**
//! `grep -rn 'Command::new("am")' crates/` returned 0 files, and there was no
//! HTTP or MCP binding either — only comments referencing it. Across the 20
//! contracts in `docs/contracts/`, the comms/mail leg was the documented hole.
//! Meanwhile the daemon held 4,982 messages across 133 projects.
//!
//! # Two authorities over one store
//!
//! This is the single most important thing to understand before trusting any
//! Agent Mail figure, and it was measured, not assumed:
//!
//! - **The daemon** (`am serve-http` on `127.0.0.1:8765`) serves an
//!   authenticated MCP surface at `/mcp/`. Every MCP client — every agent in
//!   the fleet — talks to this.
//! - **The `am` CLI** does NOT. It carries no bearer token, `/mcp/` answers
//!   401 to an unauthenticated caller, and the CLI reads `storage.sqlite3`
//!   directly. `am health` builds a throwaway probe database and never
//!   consults the daemon at all.
//!
//! That split is the explanation for a contradiction that cost the fleet
//! hours: `am agent start` reported "no listener on 127.0.0.1:8765" while
//! `curl /health` returned `{"status":"ready",...}`. It was an **auth failure
//! reported as absence**. Every CLI-derived figure reads the SQLite file, not
//! the daemon.
//!
//! This crate therefore treats the **daemon as primary** and keeps the CLI as
//! a **differential oracle** (see [`oracle`]) rather than duplicating it.
//!
//! # Design rules this crate holds
//!
//! - `&Cx` first in every async API, `cx.checkpoint()` before and after I/O,
//!   every request bounded, no detached tasks, and subprocess work delegated
//!   to [`subprocess_contract`] so both pipes are drained by a kernel we
//!   already own.
//! - **An unreachable daemon is an ERROR, never "no mail".** Every failure is
//!   a distinct [`MailError`] variant, and
//!   [`MailError::is_inconclusive_about_mail`] exists so a dispatch loop can
//!   refuse to report an empty mailbox on a transport failure.
//! - **A timeout is not a verdict.** A bounded wait that elapses maps to
//!   [`MailError::TimedOut`], never to the token a failing subject produces.
//! - **Anti-vacuity.** [`MailClient::list_tools`] refuses an empty catalogue
//!   with [`MailError::EmptyCatalogue`]: an enumeration returning zero
//!   endpoints has failed to verify the surface, not passed.
//! - Identifiers that are interchangeable integers on the wire are distinct
//!   types here: [`DeliveryCursor`], [`MessageId`], and
//!   [`wake::AttentionCursor`]. A delivery cursor also cannot be used as a
//!   resume point without its recipient — see [`journey::ResumePoint`].
//! - Secrets ([`endpoint::Endpoint`]'s bearer token,
//!   [`journey::SenderToken`]) are redacted in `Debug` and never literals in
//!   this source.
//!
//! # Defects defended here
//!
//! Each is a measured Agent Mail or NTM defect that this crate refuses in a
//! typed way rather than inheriting:
//!
//! | defect | defence |
//! |---|---|
//! | `fetch_inbox_events` promises `CURSOR_EXPIRED` below retained history but silently clamps to the floor and returns success | [`journey::verify_resume_continuity`] |
//! | `fetch_inbox` marks messages read by default, so a read mutates state | [`journey::InboxRequest::mark_read`], explicit and defaulting to `false` |
//! | tool refusals ride HTTP 200 with `isError` inside a double-encoded payload | decoded and promoted to typed variants in [`client`] |
//! | `ntm --robot-wait --wait-until=mail_pending` never returns without an explicit `--timeout` | [`wake::WakeRequest::timeout`] is mandatory, not optional |
//!
//! # Example
//!
//! ```no_run
//! use agent_mail_native::{CursorQuery, MailClient, journey};
//! use agent_mail_native::journey::{AgentName, ProjectKey};
//! use asupersync::Cx;
//!
//! async fn baseline(cx: &Cx) -> Result<(), agent_mail_native::MailError> {
//!     let client = MailClient::discover();
//!     let project = ProjectKey::new(env!("CARGO_MANIFEST_DIR"));
//!     let me = AgentName::new("BrightGorge");
//!
//!     // Establish a durable baseline without consuming backlog.
//!     let page = journey::fetch_inbox_events(
//!         cx, &client, &project, &me, CursorQuery::PositionNow, None,
//!     ).await?;
//!     println!("tail is {}", page.tail_cursor);
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod cursor;
pub mod endpoint;
pub mod error;
pub mod journey;
pub mod oracle;
pub mod wake;

pub use client::{DaemonHealth, MailClient};
pub use cursor::{CursorQuery, DeliveryCursor, MessageId};
pub use endpoint::{Endpoint, TokenSource};
pub use error::MailError;
pub use journey::{AgentName, ProjectKey, ResumePoint, SenderToken};
pub use wake::{AttentionCursor, MailWakeOutcome, WakeRequest};

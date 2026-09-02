//! K6 — the canonical dispatch-packet byte contract.
//!
//! One row per dispatch step, emitted as field-ordered UTF-8 JSONL, with the
//! SHA-256 taken over **the exact bytes emitted**. Every design choice below is
//! a measured defect refused in a typed way, not a preference.
//!
//! # Why "hash the bytes you emitted" and not the struct
//!
//! A hash computed over a struct before serialisation certifies a value that no
//! reader will ever see. The auditable artifact is the LINE. Measured
//! counterexample from this repository: `build.rs` emitted no `rerun-if-changed`
//! at all, so a git-derived `build_id` never re-derived and shipped as
//! `unversioned` while its own comment blamed a cached artifact. **A derived id
//! that never re-derives IS a cached id.** [`PacketRow::emit`] therefore returns
//! the line and the digest of that line together, and the digest is taken from
//! the emitted `String`, never from the value it came from.
//!
//! The row deliberately carries **no field for its own hash**. A self-describing
//! digest inside the bytes it covers is either a lie or a fixpoint computation;
//! `input_sha256` and `output_sha256` stay what their names say — digests of the
//! payloads, not of the row.
//!
//! # Why pane index AND pane_id are both required
//!
//! Indices shifted twice in one session, and two panes changed MODEL mid-session
//! (`%1408` GLM 5.3 -> Opus 5, `%1409` GLM 5.3 -> GPT-5.6-Luna) while their AGENT
//! names held. `notified pane 1` is undiagnosable later; `notified %1397,
//! resolved from index 1` is evidence.
//!
//! Measured 2026-09-02 06:2xZ, and it is the sharper reason: the fleet's own
//! observation tool **cannot supply a pane_id at all**. The raw keys of
//! `ntm --robot-activity=omp-orchestrator` -> `agents[0]` are exactly
//! `agent_type, capture_collected_at, capture_provenance, confidence,
//! detected_patterns, observation_confidence, observation_freshness,
//! observation_state, output_sequence, pane, pane_idx, pane_pid,
//! safe_to_dispatch, state, state_since, velocity`. It carries the index TWICE
//! (`pane`, `pane_idx`) and a PID, and **no `%NNNN` handle**. So a row built from
//! that surface must either refuse or default, and defaulting is what produces an
//! index-only row nobody can resolve six hours later. The pane_id comes from the
//! caller's own `TMUX_PANE`, or from K0's
//! [`crate::identity::resolve_pane_identity`] — that is the K0/K6 seam.
//!
//! # Why `authority` is required
//!
//! Two authorities exist over one store: the authenticated daemon (MCP HTTP) and
//! the `am` CLI reading `storage.sqlite3` directly. They agreed with skew 0 on
//! tail cursor at 5172 and 5191 — so they concur on CONTENT and diverge on
//! liveness and read-state reporting. A row that does not say which one it came
//! from cannot be reconciled against the other later.
//!
//! # Why the row carries no derived continuity verdict
//!
//! **A layer must not re-derive a judgement whose deciding input it cannot
//! observe.** Earned expensively: a client-side guard tried to decide cursor
//! expiry, but the delivery page returns only `events, next_cursor, has_more,
//! oldest_available_cursor, tail_cursor` — `global_oldest` never leaves the
//! daemon. The guard decided with strictly less information than the decider has
//! and reproduced a bug upstream had already fixed. So this contract has no
//! `resumable` field: continuity is the emitter's observation, recorded as data,
//! never the consumer's inference.
//!
//! # What is REUSED rather than re-derived
//!
//! - [`crate::journey::ResumePoint`] already makes a bare integer unrepresentable
//!   as a resume point. [`CursorPoint`] is a serialisable projection of it and has
//!   no constructor taking a raw cursor.
//! - [`crate::journey::AgentName`], [`crate::journey::ProjectKey`],
//!   [`crate::cursor::DeliveryCursor`], and K0's
//!   [`crate::identity::PaneIdentity`] / [`crate::identity::BindingStatus`].
//! - `serde_json` for escaping. Hand-rolled JSON is a measured defect elsewhere
//!   in this workspace: `crates/ack-spine/src/ledger.rs:203-219` escapes only
//!   `"`, so a newline in a field emits a literal newline inside a JSON string —
//!   invalid JSON, and it breaks the one-object-per-line invariant that makes the
//!   format JSONL at all.
//!
//! This module is synchronous and performs no I/O, so it takes no `&Cx`: there is
//! nothing to cancel and no pipe to drain. Writing the emitted line to a file is
//! the caller's bounded, cancellable work.

use crate::identity::{BindingStatus, PaneIdentity};
use crate::journey::ResumePoint;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

/// The schema identifier every row carries.
pub const SCHEMA: &str = "omp.dispatch.packet";

/// The schema version every row carries. A reader refuses any other value; see
/// [`parse_line`].
pub const SCHEMA_VERSION: u32 = 1;

/// The FIXED top-level field order, in emission order.
///
/// This is the contract that makes the digest stable across writers: two
/// emitters that agree on values but not on order produce different bytes and
/// therefore different digests, which would make every cross-writer comparison
/// a false mismatch. Asserted against the emitted line by the invariant suite,
/// read back with an independent scanner rather than trusted from the struct.
pub const FIELD_ORDER: [&str; 14] = [
    "schema",
    "version",
    "ts_unix",
    "authority",
    "stage",
    "actor",
    "bead",
    "attempt",
    "cursor_before",
    "cursor_after",
    "input_sha256",
    "output_sha256",
    "outcome",
    "error",
];

/// The FIXED field order inside the nested `actor` object.
pub const ACTOR_FIELD_ORDER: [&str; 4] = ["agent_name", "pane_id", "pane_index", "pane_binding"];

/// Which of the two authorities over one store produced this row.
///
/// # What this does NOT mean
///
/// It does not rank them. The daemon is primary for liveness and read state; the
/// CLI is a differential oracle over the same content. A row saying `cli` is not
/// a lower-quality row — it is a row whose liveness claims must not be compared
/// directly against a `daemon` row's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Authority {
    /// The authenticated MCP HTTP surface on the daemon.
    Daemon,
    /// The `am` CLI, which reads `storage.sqlite3` directly and carries no token.
    Cli,
    /// A tmux/ntm observation of a pane, which is neither mail authority.
    PaneObservation,
    /// The bead tracker, a third and separate authority.
    Tracker,
}

impl Authority {
    /// The wire token, for callers that must log it outside a row.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Daemon => "daemon",
            Self::Cli => "cli",
            Self::PaneObservation => "pane-observation",
            Self::Tracker => "tracker",
        }
    }
}

/// The dispatch lifecycle stage this row records.
///
/// # What this does NOT mean
///
/// A stage is what was ATTEMPTED, never what was achieved — the [`Outcome`] on
/// the same row carries that. `Dispatched` with `outcome: refused` is a
/// well-formed row and a common one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    /// The work was filed in the tracker.
    Filed,
    /// The work was claimed and projected into tracker state.
    Claimed,
    /// A packet was sent to a pane.
    Dispatched,
    /// The receiver was independently observed after the send.
    Observed,
    /// The tracker was read back.
    Verified,
    /// The work was closed with cited evidence.
    Closed,
}

impl Stage {
    /// The wire token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Filed => "filed",
            Self::Claimed => "claimed",
            Self::Dispatched => "dispatched",
            Self::Observed => "observed",
            Self::Verified => "verified",
            Self::Closed => "closed",
        }
    }
}

/// What the attempt produced.
///
/// # What this does NOT mean
///
/// `Delivered` means the transport returned success for the send. It does NOT
/// mean the receiver observed the packet: measured on this fleet, a send returned
/// `successful: ["4"]` while the packet never arrived, and the inverse also
/// fired the same session. Receiver observation is K8 and is a separate
/// authority by design. `TimedOut` is never a verdict about the subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// The transport returned success. Not a receipt.
    Delivered,
    /// A working gate or policy refused, which is a functioning refusal.
    Refused,
    /// A bounded wait elapsed. Not a verdict about the subject.
    TimedOut,
    /// The authority could not be reached, so the verdict is ABSENT, not negative.
    Unreachable,
    /// The tool itself broke, distinct from a refusal.
    ToolError,
    /// Nothing was eligible. Distinct from success, per this repo's anti-vacuity rule.
    NothingToDo,
}

impl Outcome {
    /// The wire token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Delivered => "delivered",
            Self::Refused => "refused",
            Self::TimedOut => "timed-out",
            Self::Unreachable => "unreachable",
            Self::ToolError => "tool-error",
            Self::NothingToDo => "nothing-to-do",
        }
    }

    /// True when this outcome must not be read as the work having happened.
    ///
    /// The restrictive set, mirroring the RPC lifecycle's restrictive terminals:
    /// a caller that treats any of these as success is asserting something the
    /// row does not say.
    #[must_use]
    pub const fn is_restrictive(self) -> bool {
        matches!(
            self,
            Self::Refused | Self::TimedOut | Self::Unreachable | Self::ToolError
        )
    }
}

/// A K6 serialisation or validation failure.
///
/// Every variant is a WRITE-time refusal. There is no lenient path: a row that
/// cannot be fully populated is not emitted at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PacketError {
    /// A required field was absent at the source, typically an `Option` on an
    /// upstream type that this contract requires.
    MissingField {
        /// The field that could not be populated.
        field: &'static str,
    },
    /// A required field was present but empty, which is the same defect wearing
    /// a value.
    EmptyField {
        /// The field that was blank.
        field: &'static str,
    },
    /// A digest field was not 64 lowercase hex characters.
    MalformedSha256 {
        /// Which digest field.
        field: &'static str,
        /// The length observed, so the caller can tell truncation from garbage.
        observed_len: usize,
    },
    /// `attempt` was zero. Attempts are 1-based: a zeroth attempt did not happen.
    ZeroAttempt,
    /// The row was not valid JSON, or not an object.
    Malformed {
        /// What the parser said.
        detail: String,
    },
    /// The schema identifier was not [`SCHEMA`].
    UnsupportedSchema {
        /// What the row claimed.
        found: String,
    },
    /// The schema version was not [`SCHEMA_VERSION`]. This is the mechanism that
    /// makes a version bump DETECTABLE rather than silently tolerated.
    UnsupportedVersion {
        /// What the row claimed, as the RAW value off the wire. A `u64` rather
        /// than a `u32` deliberately: a narrowing conversion would report a
        /// version the row never carried.
        found: u64,
        /// What this reader supports.
        expected: u32,
    },
    /// The emitted key order did not match [`FIELD_ORDER`].
    FieldOrderViolation {
        /// The position of the first disagreement.
        index: usize,
        /// The key expected there.
        expected: &'static str,
        /// The key found there.
        found: String,
    },
    /// ANTI-VACUITY: a journal with no rows is an ERROR, never a clean run. A
    /// dispatch that emitted nothing did not verify that nothing happened.
    EmptyJournal,
}

impl fmt::Display for PacketError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField { field } => write!(
                formatter,
                "PACKET_MISSING_FIELD: required field `{field}` was absent at the source; \
                 a row is not emitted with a defaulted required field"
            ),
            Self::EmptyField { field } => write!(
                formatter,
                "PACKET_EMPTY_FIELD: required field `{field}` was blank; \
                 an empty required value is the same defect wearing a value"
            ),
            Self::MalformedSha256 {
                field,
                observed_len,
            } => write!(
                formatter,
                "PACKET_MALFORMED_SHA256: `{field}` must be 64 lowercase hex characters, \
                 observed {observed_len}"
            ),
            Self::ZeroAttempt => write!(
                formatter,
                "PACKET_ZERO_ATTEMPT: attempts are 1-based; a zeroth attempt did not happen"
            ),
            Self::Malformed { detail } => {
                write!(formatter, "PACKET_MALFORMED: {detail}")
            }
            Self::UnsupportedSchema { found } => write!(
                formatter,
                "PACKET_UNSUPPORTED_SCHEMA: found `{found}`, this reader speaks `{SCHEMA}`"
            ),
            Self::UnsupportedVersion { found, expected } => write!(
                formatter,
                "PACKET_UNSUPPORTED_VERSION: row is version {found}, this reader supports \
                 {expected}; a bump is detected here rather than silently tolerated"
            ),
            Self::FieldOrderViolation {
                index,
                expected,
                found,
            } => write!(
                formatter,
                "PACKET_FIELD_ORDER: position {index} must be `{expected}`, found `{found}`; \
                 the fixed order is what makes the digest stable across writers"
            ),
            Self::EmptyJournal => write!(
                formatter,
                "PACKET_EMPTY_JOURNAL: zero rows is an ERROR, never a clean run"
            ),
        }
    }
}

impl std::error::Error for PacketError {}

/// The SHA-256 of `bytes`, as 64 lowercase hex characters.
///
/// Exposed so callers hash payloads with the same function the row contract
/// uses. Two digest implementations in one pipeline is how a comparison becomes
/// a false mismatch.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use fmt::Write as _;
        // Writing to a String cannot fail; the result is discarded deliberately.
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

fn validate_sha256(field: &'static str, value: &str) -> Result<(), PacketError> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(PacketError::MalformedSha256 {
            field,
            observed_len: value.len(),
        });
    }
    Ok(())
}

fn require_non_empty(field: &'static str, value: &str) -> Result<(), PacketError> {
    if value.trim().is_empty() {
        return Err(PacketError::EmptyField { field });
    }
    Ok(())
}

/// A serialisable cursor position, always paired with the recipient it was read
/// for and the project it belongs to.
///
/// The delivery sequence is GLOBAL while each recipient's events are sparse and
/// non-contiguous within it, so a bare integer is not a resume point. Measured:
/// cursor 5105 was circulated fleet-wide as "the cursor" while it sat below one
/// recipient's floor of 5147 and was perfectly resumable for another whose floor
/// was 2108.
///
/// There is deliberately **no constructor taking a raw cursor**. The only way to
/// build one is [`CursorPoint::from_resume_point`], so the pairing comes from a
/// [`ResumePoint`] that already established it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorPoint {
    /// The project the sequence belongs to.
    pub project: String,
    /// The recipient this position was read for.
    pub recipient: String,
    /// The raw sequence position.
    pub cursor: u64,
}

impl CursorPoint {
    /// Project a [`ResumePoint`] onto the wire.
    ///
    /// The only constructor. A caller holding just an integer cannot reach this
    /// type, which is the point.
    #[must_use]
    pub fn from_resume_point(point: &ResumePoint) -> Self {
        Self {
            project: point.project().as_str().to_owned(),
            recipient: point.recipient().as_str().to_owned(),
            cursor: point.cursor().get(),
        }
    }
}

/// Whether this row touched a cursor, and if so both ends of the movement.
///
/// Acceptance 3 says cursor before AND after must be present on every row that
/// touches a cursor. Expressing that as two `Option`s would make "touched and
/// recorded only one end" REPRESENTABLE and leave it to a validator. Here it is
/// unrepresentable: the touched variant carries both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorTouch {
    /// This row did not read or advance a cursor.
    Untouched,
    /// This row moved a cursor, and both ends are recorded.
    Touched {
        /// The position before the step.
        before: CursorPoint,
        /// The position after the step.
        after: CursorPoint,
    },
}

impl CursorTouch {
    fn before(&self) -> Option<&CursorPoint> {
        match self {
            Self::Untouched => None,
            Self::Touched { before, .. } => Some(before),
        }
    }

    fn after(&self) -> Option<&CursorPoint> {
        match self {
            Self::Untouched => None,
            Self::Touched { after, .. } => Some(after),
        }
    }
}

/// Who acted: the agent name, the tmux pane handle, AND the pane index.
///
/// All four fields are required. See the module docs for why defaulting any of
/// them produces a row nobody can resolve later.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorIdentity {
    /// The durable Agent Mail identity. Names held across the session while
    /// indices and models did not.
    pub agent_name: String,
    /// The tmux pane handle, e.g. `%1408`.
    pub pane_id: String,
    /// The pane index at emission time. Indices shifted twice in one session, so
    /// this is a timestamped observation, not an identifier.
    pub pane_index: u32,
    /// How the pane binding was established, so a later reader knows whether the
    /// identity was verified against the live pane.
    pub pane_binding: BindingStatus,
}

impl ActorIdentity {
    /// Build an actor from explicit parts, refusing blanks.
    pub fn new(
        agent_name: impl Into<String>,
        pane_id: impl Into<String>,
        pane_index: u32,
        pane_binding: BindingStatus,
    ) -> Result<Self, PacketError> {
        let agent_name = agent_name.into();
        let pane_id = pane_id.into();
        require_non_empty("actor.agent_name", &agent_name)?;
        require_non_empty("actor.pane_id", &pane_id)?;
        Ok(Self {
            agent_name,
            pane_id,
            pane_index,
            pane_binding,
        })
    }

    /// Build an actor from K0's [`PaneIdentity`], REFUSING its optionality.
    ///
    /// `PaneIdentity::agent_name` and `PaneIdentity::pane_index` are `Option`
    /// because the resolver may not return them. This contract requires both, so
    /// an absent value is a typed [`PacketError::MissingField`] here rather than
    /// an `unwrap_or_default` — which is the precise defect K6 exists to prevent.
    /// A row is not emitted at all rather than emitted with a zero index or an
    /// empty name.
    pub fn from_pane_identity(identity: &PaneIdentity) -> Result<Self, PacketError> {
        let agent_name = identity
            .agent_name
            .as_ref()
            .ok_or(PacketError::MissingField {
                field: "actor.agent_name",
            })?;
        let pane_index = identity.pane_index.ok_or(PacketError::MissingField {
            field: "actor.pane_index",
        })?;
        Self::new(
            agent_name.as_str(),
            identity.pane_id.as_str(),
            pane_index,
            identity.binding,
        )
    }
}

/// A monotonic wall-clock stamp in whole seconds since the Unix epoch.
///
/// Every figure carries its timestamp or it is not a figure: the mail store moved
/// from 133 projects / 4,982 messages at 05:00 to 136 / 5,025 at 05:29 in one
/// session, and reconciling two agents' counts required pinning each to its read
/// point.
///
/// Integer seconds rather than a rendered RFC 3339 string, deliberately: a
/// rendering varies by formatter and would make two writers' bytes differ on
/// identical facts, which is exactly what the fixed field order exists to
/// prevent. Rendering is a reader's job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Read the host clock.
    #[must_use]
    pub fn now() -> Self {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        Self(seconds)
    }

    /// Wrap an explicit epoch second, for deterministic emission in tests and
    /// for stamping a row with the time an observation was COLLECTED rather than
    /// the time it was serialised.
    #[must_use]
    pub const fn from_unix(seconds: i64) -> Self {
        Self(seconds)
    }

    /// The raw epoch second.
    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }
}

/// Every value a row needs, with no optional required fields.
///
/// This is a struct rather than a builder on purpose. Every required field is
/// non-`Option`, so **forgetting one is a compile error**, not a runtime
/// default — the strongest available form of acceptance 5. The runtime
/// validation in [`PacketRow::new`] then covers what the type system cannot:
/// blanks, malformed digests, and a zeroth attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowSpec {
    /// Which authority produced this row.
    pub authority: Authority,
    /// The lifecycle stage attempted.
    pub stage: Stage,
    /// Who acted.
    pub actor: ActorIdentity,
    /// The bead this row is about.
    pub bead: String,
    /// 1-based attempt number.
    pub attempt: u32,
    /// Whether a cursor moved, and both ends if so.
    pub cursor: CursorTouch,
    /// SHA-256 of the exact input payload bytes.
    pub input_sha256: String,
    /// SHA-256 of the exact output payload bytes.
    pub output_sha256: String,
    /// What the attempt produced.
    pub outcome: Outcome,
    /// The error text when the outcome is restrictive; `None` otherwise.
    pub error: Option<String>,
    /// When the fact this row records was observed.
    pub ts: Timestamp,
}

/// One validated packet row.
///
/// Construct with [`PacketRow::new`], emit with [`PacketRow::emit`]. There is no
/// path from an unvalidated value to emitted bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketRow {
    spec: RowSpec,
}

/// The private wire projection. Field DECLARATION ORDER here is the emitted key
/// order — `serde`'s derive calls `serialize_field` in declaration order and the
/// JSON serialiser writes them in call order.
///
/// A `serde_json::Map` is deliberately NOT used to build rows: without the
/// `preserve_order` feature it is a `BTreeMap` and would silently alphabetise
/// the contract into `actor, attempt, authority, bead, ...`.
/// Serialize-only by construction: it borrows, so it cannot round-trip. Reads go
/// through [`ParsedRow`], which owns its values. That split is deliberate — a
/// single type doing both would have to own its fields and would then copy every
/// value on every emission.
#[derive(Serialize)]
struct Wire<'row> {
    schema: &'row str,
    version: u32,
    ts_unix: i64,
    authority: Authority,
    stage: Stage,
    actor: &'row ActorIdentity,
    bead: &'row str,
    attempt: u32,
    cursor_before: Option<&'row CursorPoint>,
    cursor_after: Option<&'row CursorPoint>,
    input_sha256: &'row str,
    output_sha256: &'row str,
    outcome: Outcome,
    error: Option<&'row str>,
}

/// An owned parse of a row, for readers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedRow {
    /// The schema identifier, already checked against [`SCHEMA`].
    pub schema: String,
    /// The version, already checked against [`SCHEMA_VERSION`].
    pub version: u32,
    /// Observation time in epoch seconds.
    pub ts_unix: i64,
    /// Which authority produced the row.
    pub authority: Authority,
    /// The stage attempted.
    pub stage: Stage,
    /// Who acted.
    pub actor: ActorIdentity,
    /// The bead.
    pub bead: String,
    /// 1-based attempt.
    pub attempt: u32,
    /// Position before the step, when a cursor was touched.
    pub cursor_before: Option<CursorPoint>,
    /// Position after the step, when a cursor was touched.
    pub cursor_after: Option<CursorPoint>,
    /// Digest of the input payload.
    pub input_sha256: String,
    /// Digest of the output payload.
    pub output_sha256: String,
    /// What the attempt produced.
    pub outcome: Outcome,
    /// Error text on a restrictive outcome.
    pub error: Option<String>,
}

/// An emitted row: the exact bytes, and the digest OF those bytes.
///
/// The digest is computed from [`EmittedRow::line`] after serialisation, so
/// re-reading the line from disk and re-hashing reproduces it. That round trip is
/// the only thing that makes a receipt auditable later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedRow {
    /// The exact JSONL line, without a trailing newline.
    pub line: String,
    /// SHA-256 of `line.as_bytes()`, 64 lowercase hex characters.
    pub sha256: String,
}

impl PacketRow {
    /// Validate a spec into a row, refusing at WRITE time.
    ///
    /// Refuses a blank bead, a blank actor name or pane id, a digest that is not
    /// 64 lowercase hex characters, and a zeroth attempt. There is no lenient
    /// branch and no defaulted field.
    pub fn new(spec: RowSpec) -> Result<Self, PacketError> {
        require_non_empty("bead", &spec.bead)?;
        require_non_empty("actor.agent_name", &spec.actor.agent_name)?;
        require_non_empty("actor.pane_id", &spec.actor.pane_id)?;
        validate_sha256("input_sha256", &spec.input_sha256)?;
        validate_sha256("output_sha256", &spec.output_sha256)?;
        if spec.attempt == 0 {
            return Err(PacketError::ZeroAttempt);
        }
        if let Some(error) = spec.error.as_deref() {
            require_non_empty("error", error)?;
        }
        Ok(Self { spec })
    }

    /// The validated spec.
    #[must_use]
    pub const fn spec(&self) -> &RowSpec {
        &self.spec
    }

    /// Serialise to one JSONL line and hash THE EMITTED BYTES.
    ///
    /// Optional fields are emitted as explicit `null`, never omitted: skipping
    /// would break the fixed field order and make "absent" indistinguishable
    /// from "present and empty", which is the same indistinguishability defect
    /// as folding a working refusal into a crash's exit code.
    pub fn emit(&self) -> Result<EmittedRow, PacketError> {
        let wire = Wire {
            schema: SCHEMA,
            version: SCHEMA_VERSION,
            ts_unix: self.spec.ts.get(),
            authority: self.spec.authority,
            stage: self.spec.stage,
            actor: &self.spec.actor,
            bead: &self.spec.bead,
            attempt: self.spec.attempt,
            cursor_before: self.spec.cursor.before(),
            cursor_after: self.spec.cursor.after(),
            input_sha256: &self.spec.input_sha256,
            output_sha256: &self.spec.output_sha256,
            outcome: self.spec.outcome,
            error: self.spec.error.as_deref(),
        };
        let line = serde_json::to_string(&wire).map_err(|error| PacketError::Malformed {
            detail: error.to_string(),
        })?;
        let sha256 = sha256_hex(line.as_bytes());
        Ok(EmittedRow { line, sha256 })
    }
}

/// The top-level keys of one emitted line, in emission order.
///
/// A deliberately independent scanner: it does not deserialise, so it cannot
/// inherit the emitter's own ordering assumption.
///
/// # The instrument defect this function had, and why it is recorded here
///
/// The first version tracked only DEPTH and treated any string at depth 1 as a
/// key. It therefore returned values as keys — the observed output was
/// `["schema", "omp.dispatch.packet", "version", ...]`, 21 entries instead of
/// 14 — and five legs failed on one bug. Depth says WHERE a string is; it does
/// not say whether it is in KEY POSITION. That needs the last significant
/// delimiter: a string is a key only when the previous one was `{` or a
/// depth-1 `,`, never after a `:`.
///
/// The failure was loud because the legs compare against the full expected
/// sequence rather than a length or a subset. A leg asserting only
/// `keys.len() >= 14` would have passed on the broken scanner.
#[must_use]
pub fn top_level_keys(line: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut escaped = false;
    let mut current = String::new();
    // True when the next string encountered at depth 1 occupies key position.
    let mut awaiting_key = false;
    // Whether the string currently being read is a key rather than a value.
    let mut reading_key = false;
    for character in line.chars() {
        if in_string {
            if escaped {
                escaped = false;
                current.push(character);
                continue;
            }
            match character {
                '\\' => escaped = true,
                '"' => {
                    in_string = false;
                    if reading_key {
                        keys.push(current.clone());
                        awaiting_key = false;
                        reading_key = false;
                    }
                    current.clear();
                }
                other => current.push(other),
            }
            continue;
        }
        match character {
            '"' => {
                in_string = true;
                current.clear();
                reading_key = depth == 1 && awaiting_key;
            }
            '{' => {
                depth += 1;
                if depth == 1 {
                    awaiting_key = true;
                }
            }
            '[' => depth += 1,
            '}' | ']' => depth -= 1,
            ',' => {
                if depth == 1 {
                    awaiting_key = true;
                }
            }
            ':' => awaiting_key = false,
            _ => {}
        }
    }
    keys
}

/// Assert the emitted key order matches [`FIELD_ORDER`] exactly.
///
/// Length is compared as well as content: a row with the right prefix and an
/// extra trailing key is a contract change, not a compatible row.
pub fn assert_field_order(line: &str) -> Result<(), PacketError> {
    let keys = top_level_keys(line);
    for (index, expected) in FIELD_ORDER.iter().enumerate() {
        match keys.get(index) {
            Some(found) if found == expected => {}
            Some(found) => {
                return Err(PacketError::FieldOrderViolation {
                    index,
                    expected,
                    found: found.clone(),
                });
            }
            None => {
                return Err(PacketError::FieldOrderViolation {
                    index,
                    expected,
                    found: String::new(),
                });
            }
        }
    }
    if keys.len() != FIELD_ORDER.len() {
        return Err(PacketError::FieldOrderViolation {
            index: FIELD_ORDER.len(),
            expected: "<end of row>",
            found: keys
                .get(FIELD_ORDER.len())
                .cloned()
                .unwrap_or_else(|| format!("{} keys", keys.len())),
        });
    }
    Ok(())
}

/// Parse one emitted line, refusing a foreign schema or an unsupported version.
///
/// The version check is what makes a bump DETECTABLE: a reader built for
/// version 1 refuses a version 2 row with a typed error naming both, rather than
/// deserialising the fields it happens to recognise.
pub fn parse_line(line: &str) -> Result<ParsedRow, PacketError> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|error| PacketError::Malformed {
            detail: error.to_string(),
        })?;
    let object = value.as_object().ok_or(PacketError::Malformed {
        detail: "row is not a JSON object".to_owned(),
    })?;
    let schema = object
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .ok_or(PacketError::MissingField { field: "schema" })?;
    if schema != SCHEMA {
        return Err(PacketError::UnsupportedSchema {
            found: schema.to_owned(),
        });
    }
    let version = object
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .ok_or(PacketError::MissingField { field: "version" })?;
    // NOT `u32::try_from(version).unwrap_or(u32::MAX)`. That clamp was here in
    // the first draft and it is the same defect this module refuses elsewhere: a
    // row claiming version 5_000_000_000 would be REPORTED as 4294967295, so the
    // refusal would name a version the row never carried. `found` is the raw
    // `u64` off the wire, unmodified, because a diagnostic that alters the value
    // it is diagnosing is worse than no diagnostic.
    if version != u64::from(SCHEMA_VERSION) {
        return Err(PacketError::UnsupportedVersion {
            found: version,
            expected: SCHEMA_VERSION,
        });
    }
    serde_json::from_value(value).map_err(|error| PacketError::Malformed {
        detail: error.to_string(),
    })
}

/// An append-only journal of emitted rows.
#[derive(Debug, Clone, Default)]
pub struct PacketJournal {
    rows: Vec<EmittedRow>,
}

impl PacketJournal {
    /// An empty journal. It is an ERROR until something is appended; see
    /// [`PacketJournal::assert_non_empty`].
    #[must_use]
    pub const fn new() -> Self {
        Self { rows: Vec::new() }
    }

    /// Validate, emit, and append one row. The single emission path.
    pub fn append(&mut self, spec: RowSpec) -> Result<&EmittedRow, PacketError> {
        let emitted = PacketRow::new(spec)?.emit()?;
        self.rows.push(emitted);
        Ok(self
            .rows
            .last()
            .expect("a row was just pushed, so last() is Some"))
    }

    /// The emitted rows.
    #[must_use]
    pub fn rows(&self) -> &[EmittedRow] {
        &self.rows
    }

    /// ANTI-VACUITY: zero rows is an ERROR, never a clean run.
    ///
    /// A journal that recorded nothing has not established that nothing
    /// happened — it is indistinguishable from a journal whose writer never ran.
    pub fn assert_non_empty(&self) -> Result<(), PacketError> {
        if self.rows.is_empty() {
            return Err(PacketError::EmptyJournal);
        }
        Ok(())
    }

    /// The journal as JSONL: one row per line, newline-separated, with a
    /// trailing newline so appending another row cannot join two rows.
    #[must_use]
    pub fn to_jsonl(&self) -> String {
        let mut out = String::new();
        for row in &self.rows {
            out.push_str(&row.line);
            out.push('\n');
        }
        out
    }

    /// The digest of the whole journal's emitted bytes.
    #[must_use]
    pub fn jsonl_sha256(&self) -> String {
        sha256_hex(self.to_jsonl().as_bytes())
    }
}

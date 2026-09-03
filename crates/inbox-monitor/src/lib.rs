#![forbid(unsafe_code)]

//! `inbox-monitor` — turn an ARRIVED Agent Mail message into a LOUD typed outcome.
//!
//! # Why this crate exists, measured
//!
//! The only signal measured to reach a human in this fleet is a typed NONZERO outcome the
//! operator must answer. `ATTENTION.txt` took **178 consecutive ticks from one writer with
//! zero readers** — a log line is not a monitor. This crate therefore does exactly one
//! thing: it reads the two Agent Mail surfaces, classifies, and *exits with a code a
//! supervisor cannot ignore*. The JSON row and the JSONL ledger are evidence, not the
//! notification; the exit code is the notification.
//!
//! It is NOT a second messaging system. `ntm send` remains the work transport and Agent
//! Mail remains the durable record. This adds **notification** and **attribution** to what
//! already exists.
//!
//! # The two-surface split, and why a design that reads one is wrong
//!
//! * The **cursor** comes from `am inbox-events`. That feed is restart-safe and `--after`
//!   is a *delivery* cursor, explicitly "not a message id". It carries **no read state**.
//! * The **unread-ness** comes from `am inbox`. Those rows are the authority for whether a
//!   human still owes an answer.
//!
//! Any design that tries to derive unread-ness from the event feed is reading a delivery
//! log as a mailbox and will report a confident false zero the moment a message is
//! delivered twice or acknowledged out of band.
//!
//! # MEASURED CONTRACT DRIFT — `am inbox` rows, 2026-09-02
//!
//! The shared task contract claimed `am inbox --json` rows carry
//! `read_ts` / `created_ts` / `ack_required` / `kind` / `thread_id`. They do **not**. The
//! installed `am` emits, as the union over 109 live rows:
//!
//! ```text
//! ack_status, age, from, id, importance, priority, subject, thread, topic
//! ```
//!
//! wrapped in an envelope whose row array is keyed `inbox` (alongside `_meta`, `_alerts`,
//! `_actions`, `_diagnostics`, `count`). `topic` is absent on some rows, so it is optional.
//! Read state is carried by `priority`, whose measured value set is
//! `{read, unread, urgent, ack-overdue}`; `--unread` returns everything that is not `read`
//! (17 + 1 + 2 = 20 of 109 on the measuring run). Acknowledgement state is `ack_status`,
//! measured `{none, required, overdue}`.
//!
//! [`InboxRow`] keeps the contract's shape because that is the type this crate is judged
//! against, and [`parse_inbox_rows`] adapts the measured wire shape onto it — honouring a
//! literal `read_ts` key if a future `am` ever ships one. Had the crate hard-required
//! `read_ts` off the wire it would have parsed **nothing** in production while every test
//! stayed green: a crate that is correct against a document and broken against the machine.
//!
//! `am inbox-events` needed no adaptation — its five top-level keys and eight event keys
//! were confirmed live, byte for byte with the contract.
//!
//! # `--direct` is not free
//!
//! `--direct` ("allow a direct SQLite read only when no daemon is reachable") produces
//! byte-identical output and does not announce itself. A monitor that silently falls back
//! is a monitor whose provenance is unrecoverable, so the emitted row carries
//! `direct_read` and the human-readable line says so out loud. Measured asymmetry:
//! `am inbox-events` accepts `--direct`; **`am inbox` has no such flag** (`am inbox --help`,
//! 2026-09-02), so the flag is passed to exactly one of the two reads.

use serde::Deserialize;
use std::fmt;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------------------
// Exit codes
// ---------------------------------------------------------------------------------------

/// `MailWaiting` — a human owes an answer.
///
/// From the UNALLOCATED `5..=63` band of `docs/error_codes/exit_code_registry.md` §5, which
/// that table names as "the correct home for any new distinct meaning". It is deliberately
/// NOT 1: `XC-001` already carries three meanings (gate refused / tool broke / caller
/// misused), and folding "you have mail" into it makes the one actionable outcome in this
/// crate indistinguishable from a crash.
pub const EXIT_MAIL_WAITING: u8 = 12;

/// `Unreachable` — the monitor could not observe Agent Mail at all.
///
/// A DIFFERENT number from [`EXIT_MAIL_WAITING`] because it is not the same fact: the
/// verdict is ABSENT, not negative. This is the `XC-077` UNPROVEN shape ("the checker could
/// not establish its verdict"), given its own number in the unallocated band rather than
/// reusing 77, which four other crates already spend on their own deadman semantics.
///
/// The measured false negative this defends against: `am agent start` reported
/// "no listener on 127.0.0.1:8765" while `/health` returned `{"status":"ready"}` and two
/// robot calls returned live data. A monitor that reads "cannot see" as "nothing to see"
/// reports silence loudest exactly when it is blindest.
pub const EXIT_UNREACHABLE: u8 = 13;

/// `CursorRegressed` — the persisted cursor is AHEAD of the server's tail.
///
/// A third distinct number because the remedy is different from both neighbours: nothing is
/// wrong with the mailbox and nothing is wrong with the daemon — our own state file is
/// describing a future that the durable feed has not reached, so every subsequent run would
/// silently SKIP events. Repair the state; do not read mail and do not retry.
pub const EXIT_CURSOR_REGRESSED: u8 = 14;

/// `CursorBelowFloor` — the persisted cursor is BELOW this recipient's oldest retained event.
///
/// A FOURTH distinct number, and the dangerous one, because it is the only fault in this
/// crate that is silent by construction. [`EXIT_CURSOR_REGRESSED`] means our state is
/// AHEAD of the durable tail, so every further run SKIPS events. This one means our state
/// is BELOW the recipient's oldest retained event, so the position we would resume from
/// cannot be shown to be continuous with what we have already seen. Same family, opposite
/// direction, and — decisively — two different repairs, so two codes.
///
/// # Why this is unconditional, and why NOTHING here is about data loss
///
/// `oldest_available_cursor` is the oldest event still retained **for that recipient**; for
/// a mailbox that started receiving recently it simply equals its FIRST EVENT. Nothing is
/// being evicted, and no fleet-wide retention window exists — measured 2026-09-02, spans
/// per recipient were GreenFrog `oldest=2108 tail=5161` (3053), AmberGate
/// `oldest=2058 tail=5162` (3104), SnowyCanyon `oldest=5147 tail=5165` (18), BrightGorge 0.
/// A small "eviction window" cannot coexist with a recipient retaining 3053 positions.
///
/// The real reason is structural: a recipient's events are **sparse and non-contiguous
/// within one GLOBAL monotonic sequence** — GreenFrog holds 2108, 2109, 2126. So a stored
/// cursor below `oldest_available_cursor` **cannot be distinguished** from a recipient that
/// merely started receiving later. Continuity is unprovable in both cases, so refusing is
/// correct either way, which is exactly why this guard does not need to know the cause.
///
/// # The measured defect this defends against: a documented refusal that silently CLAMPS
///
/// The daemon documents that a cursor below retained history yields `CURSOR_EXPIRED`. It
/// does not. Measured 2026-09-02, isolated with a redirect and no pipe:
///
/// ```text
/// am inbox-events --agent GreenFrog --after 1 --limit 3 --json  ->  rc=0
/// first event cursor served = 2108, oldest_available = 2108
/// keys present: events, has_more, next_cursor, oldest_available_cursor, tail_cursor
/// error/code/status keys present: NONE
/// ```
///
/// Asked from 1, served from 2108: **2107 positions silently skipped behind a normal
/// success shape**, with no refusal token of any kind. That is why the check must be
/// CLIENT-SIDE and must run BEFORE the mail check — the server will hand back a
/// healthy-looking page from a position the caller never asked for.
///
/// # How we measurably get here
///
/// Not eviction: a borrowed cursor. The sequence is global while the floor is per
/// recipient, and the orchestrator circulated cursor 5105 fleet-wide. 5105 is below
/// SnowyCanyon's floor of 5147 — and perfectly resumable for GreenFrog. **Cursors are not
/// portable across recipients**; a monitor that accepts a borrowed one goes blind while
/// every surface it emits reports healthy.
pub const EXIT_CURSOR_BELOW_FLOOR: u8 = 15;

/// `AuthoritiesDisagree` — the daemon and the CLI report different unread counts.
///
/// A FIFTH distinct number because it is not a mail fact and not a state fault: both reads
/// SUCCEEDED and they contradict each other, so the monitor has no unread count to report.
/// Folding this into [`EXIT_MAIL_WAITING`] with one arm's number would be the exact defect
/// this crate exists to remove — a monitor that reports a number without saying which
/// authority answered is not evidence.
///
/// # Measured, on my own mailbox, 2026-09-02T23:5x Z
///
/// | surface | unread | total rows |
/// |---|---|---|
/// | daemon, `fetch_inbox` over authenticated MCP, `mark_read=false` | **96** | 128, of which 32 carry a non-null `read_ts` |
/// | `am inbox --unread` (CLI, direct `storage.sqlite3`) | **20** | `count: 20` in the envelope |
///
/// The bead that owns this recorded the daemon at **0** unread with "every row carries a
/// `read_ts`". That is REFUTED by a non-mutating read, and the refutation names its own
/// cause: `fetch_inbox`'s `mark_read` **defaults to true**, so a read that omits it CONSUMES
/// the unread state it was measuring and a second read then honestly reports zero. This
/// crate's daemon arm therefore sets `mark_read: false` explicitly, which is also why
/// [`agent_mail_native::journey::InboxRequest`] carries that field with an explicit `false`
/// default rather than inheriting the daemon's.
///
/// Which number is TRUE is not decided here and is not this crate's question. 96 and 20 are
/// both reported, each attributed to the authority that said it.
pub const EXIT_AUTHORITIES_DISAGREE: u8 = 16;

/// `Clear` — read both surfaces, nothing is owed.
pub const EXIT_CLEAR: u8 = 0;

// ---------------------------------------------------------------------------------------
// Verdict
// ---------------------------------------------------------------------------------------

/// WHY the monitor could not observe. A 401 is not an absent listener.
///
/// Acceptance 3 of `omp-orchestrator-monitor-reads-oracle-y256` requires these two to be
/// distinguishable. They are distinguishable HERE because `agent-mail-native` already
/// separates them upstream — `MailError::Unauthorized { status }` for a daemon that answered
/// and rejected the credential, `MailError::Unreachable` for a transport failure before any
/// HTTP status. This enum consumes that distinction rather than re-deriving it from a
/// message string, which is the only way a caller can branch on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnreachableReason {
    /// Nothing answered: connection refused, DNS failure, transport I/O error.
    /// The remedy is to START something.
    Absent,
    /// The daemon answered and rejected our credential. The remedy is to SUPPLY one.
    /// `status` is carried because 401 and 403 are different answers to that question.
    Unauthorized {
        /// The HTTP status the daemon returned, 401 or 403.
        status: u16,
    },
    /// Reached, authorized, and still unusable — a parse failure, a refusal we do not
    /// classify, or a timeout. Named rather than folded into `Absent`, because "we could
    /// not read the answer" is not "there was no answer".
    Indeterminate,
}

impl UnreachableReason {
    /// The stable machine label emitted in the JSON row and the ledger.
    pub fn label(&self) -> &'static str {
        match self {
            UnreachableReason::Absent => "absent",
            UnreachableReason::Unauthorized { .. } => "unauthorized",
            UnreachableReason::Indeterminate => "indeterminate",
        }
    }
}

/// What one observation of the two Agent Mail surfaces means, and what its integer says.
///
/// # Recovering the meaning from the integer alone
///
/// A reader holding only the number recovers it from
/// `docs/error_codes/exit_code_registry.md` — the `XC-*` table, band `5..=63`
/// ("UNALLOCATED, ours to claim"), rows for `inbox-monitor`:
///
/// | code | variant | MEANS | does **NOT** mean |
/// |---|---|---|---|
/// | 0 | [`MonitorVerdict::Clear`] | both surfaces read; no unread mail; cursor sane | that mail was delivered and ignored — `Clear` is only reachable after BOTH reads succeeded |
/// | 12 | [`MonitorVerdict::MailWaiting`] | a human owes an answer to a named sender | that the message is urgent; importance is a separate field on the row |
/// | 13 | [`MonitorVerdict::Unreachable`] | the monitor could not observe; the verdict is ABSENT | that there is no mail. This is the fail-closed unknown |
/// | 14 | [`MonitorVerdict::CursorRegressed`] | our persisted cursor is ahead of the durable tail | that mail is waiting. Our STATE is wrong, not the mailbox |
/// | 15 | [`MonitorVerdict::CursorBelowFloor`] | our persisted cursor is below THIS recipient's oldest retained event, so the resume position cannot be proven continuous | that mail is waiting, and NOT that the mailbox lost anything. A recipient's events are sparse in a global sequence, so a position below the floor is indistinguishable from a recipient that started later — and the daemon will not say so, it CLAMPS to the floor and returns success |
///
/// | 16 | [`MonitorVerdict::AuthoritiesDisagree`] | both mailbox authorities answered and gave DIFFERENT unread counts, so no count is reported | that anything failed, and NOT that mail is or is not waiting. Both reads SUCCEEDED; the disagreement is the finding. Measured live: daemon 96, CLI 20 |
///
/// The load-bearing column is **does NOT mean**, per that registry's §1: a refusal and a
/// failure are indistinguishable from the outside when the only signal is a small integer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorVerdict {
    /// Both surfaces read successfully and nothing is owed.
    Clear,
    /// At least one unread row is waiting; the oldest is named so the notification is
    /// answerable without a second lookup.
    MailWaiting {
        /// How many unread rows the mailbox surface reported.
        unread: usize,
        /// `from` of the oldest unread row (lowest message id).
        oldest_from: String,
        /// `subject` of the oldest unread row.
        oldest_subject: String,
    },
    /// The monitor could not establish a verdict. Restrictive: never fold into `Clear`.
    Unreachable {
        /// Why — the spawn outcome, the exit status, or the parse failure.
        detail: String,
        /// WHICH KIND of not-observable, as a type rather than a substring.
        ///
        /// A free-text `detail` cannot be asserted on, and the distinction it was hiding is
        /// the one that cost the fleet an hour: `am agent start` reported "no listener on
        /// 127.0.0.1:8765" while `/health` answered `status: ready` and two robot calls
        /// returned live data — an AUTH FAILURE REPORTED AS ABSENCE. The remedies are
        /// opposite (supply a credential vs start a process), so they are separate values.
        reason: UnreachableReason,
    },
    /// The persisted cursor is strictly ahead of the durable tail cursor.
    CursorRegressed {
        /// What our state file said.
        persisted: u64,
        /// What the durable feed said its tail was.
        tail: u64,
    },
    /// The persisted cursor is strictly BELOW this recipient's oldest retained event, so
    /// the position we would resume from cannot be shown to be continuous.
    ///
    /// Deliberately NOT the same variant as [`MonitorVerdict::CursorRegressed`], because it
    /// is not the same fact and not the same repair. Regressed: our state is AHEAD of the
    /// tail, so runs SKIP. Below floor: a recipient's events are sparse and non-contiguous
    /// in one global sequence, so a stored position under the floor cannot be distinguished
    /// from a recipient that simply started receiving later — continuity is unprovable
    /// either way, and the daemon answers such a read by silently CLAMPING to the floor
    /// behind a success shape (measured: asked from 1, served from 2108, no error keys).
    /// All three numbers are carried so the row is diagnosable from the artifact alone.
    CursorBelowFloor {
        /// What our state file said.
        persisted: u64,
        /// This recipient's oldest replayable cursor — the floor `persisted` fell under.
        oldest_available: u64,
        /// The durable tail, so the size of the unreachable gap is readable directly.
        tail: u64,
    },
    /// Both mailbox authorities answered and they DISAGREE, so there is no unread count to
    /// report. Both numbers and both source labels are carried, because a disagreement that
    /// does not name its sides is not actionable.
    ///
    /// Deliberately NOT [`MonitorVerdict::Unreachable`]: nothing failed. Two successful
    /// reads contradict each other, which is a different fact with a different remedy —
    /// reconcile the surfaces, do not restart anything.
    AuthoritiesDisagree {
        /// The authenticated daemon's unread count — the designated PRIMARY.
        daemon_unread: usize,
        /// The `am` CLI's unread count — the designated differential ORACLE.
        cli_unread: usize,
    },
}

impl MonitorVerdict {
    /// The process exit code for this verdict. See the table on [`MonitorVerdict`].
    pub fn exit_code(&self) -> u8 {
        match self {
            MonitorVerdict::Clear => EXIT_CLEAR,
            MonitorVerdict::MailWaiting { .. } => EXIT_MAIL_WAITING,
            MonitorVerdict::Unreachable { .. } => EXIT_UNREACHABLE,
            MonitorVerdict::CursorRegressed { .. } => EXIT_CURSOR_REGRESSED,
            MonitorVerdict::CursorBelowFloor { .. } => EXIT_CURSOR_BELOW_FLOOR,
            MonitorVerdict::AuthoritiesDisagree { .. } => EXIT_AUTHORITIES_DISAGREE,
        }
    }

    /// The stable machine label emitted in the JSON row and the ledger.
    pub fn label(&self) -> &'static str {
        match self {
            MonitorVerdict::Clear => "clear",
            MonitorVerdict::MailWaiting { .. } => "mail_waiting",
            MonitorVerdict::Unreachable { .. } => "unreachable",
            MonitorVerdict::CursorRegressed { .. } => "cursor_regressed",
            MonitorVerdict::CursorBelowFloor { .. } => "cursor_below_floor",
            MonitorVerdict::AuthoritiesDisagree { .. } => "authorities_disagree",
        }
    }

    /// The line a human reads. Every nonzero verdict names its own remedy.
    pub fn human_line(&self) -> String {
        match self {
            MonitorVerdict::Clear => "inbox-monitor: CLEAR — no unread mail, cursor sane".to_string(),
            MonitorVerdict::MailWaiting {
                unread,
                oldest_from,
                oldest_subject,
            } => format!(
                "inbox-monitor: MAIL WAITING — {unread} unread; oldest from {oldest_from}: {oldest_subject}"
            ),
            MonitorVerdict::Unreachable { detail, reason } => {
                // The remedy differs by reason, so the line names it. An operator handed
                // "could not observe" alone restarts a daemon that is already running.
                let remedy = match reason {
                    UnreachableReason::Absent => {
                        "nothing answered — START the daemon (`am agent start`)"
                    }
                    UnreachableReason::Unauthorized { .. } => {
                        "the daemon ANSWERED and refused the credential — supply a token \
                         (AGENT_MAIL_TOKEN); do NOT restart anything"
                    }
                    UnreachableReason::Indeterminate => {
                        "reached but unreadable — diagnose the payload; this is not absence"
                    }
                };
                format!(
                    "inbox-monitor: UNREACHABLE [{}] — could not observe Agent Mail \
                     ({detail}). {remedy}. This is NOT 'no mail'; the verdict is absent",
                    reason.label()
                )
            }
            MonitorVerdict::CursorRegressed { persisted, tail } => format!(
                "inbox-monitor: CURSOR REGRESSED — persisted {persisted} is ahead of tail {tail}; \
                 repair state, do not read mail"
            ),
            MonitorVerdict::CursorBelowFloor {
                persisted,
                oldest_available,
                tail,
            } => format!(
                "inbox-monitor: CURSOR BELOW FLOOR — persisted {persisted} is below this \
                 recipient's oldest retained event {oldest_available} (tail {tail}); the resume \
                 position cannot be proven continuous and the daemon CLAMPS to the floor behind \
                 a success shape, so the page would be one the server chose. Repair the cursor \
                 file named on stderr, or re-baseline with --position-now; the cursor was NOT \
                 advanced and no mail claim was made"
            ),
            MonitorVerdict::AuthoritiesDisagree {
                daemon_unread,
                cli_unread,
            } => format!(
                "inbox-monitor: AUTHORITIES DISAGREE — daemon (authenticated MCP fetch_inbox) \
                 says {daemon_unread} unread, am CLI (direct storage.sqlite3) says \
                 {cli_unread}. Both reads SUCCEEDED; nothing is unreachable. No unread count \
                 is reported because reporting one without naming its authority is not \
                 evidence. Reconcile the surfaces; do not restart anything"
            ),
        }
    }

    /// May a run ending in this verdict advance the persisted cursor?
    ///
    /// # Why this is a typed predicate and not an inline `matches!`
    ///
    /// This is the single most dangerous boolean in the crate, and it used to be spelled
    /// `!matches!(verdict, Unreachable { .. })` at its one call site in `main.rs` — a
    /// NEGATIVE test, which silently opts every future variant IN. `CursorBelowFloor` was
    /// added and would have inherited "yes, advance" by default.
    ///
    /// That default is precisely self-defeating: a run that detects an unprovable position
    /// and then persists a new one has repaired the SYMPTOM and kept the blindness. The
    /// operator sees one exit 15, the cursor silently re-bases to whatever position the
    /// server clamped to, and the next run reports `Clear` — the evidence that positions
    /// were skipped is gone, which is the exact failure the verdict exists to announce.
    ///
    /// So the predicate is POSITIVE and EXHAUSTIVE: only a verdict that made a trustworthy
    /// claim about a proven position may move the cursor, and a new variant must choose.
    ///
    /// * `Clear` / `MailWaiting` — the position was proven and the page is trustworthy.
    /// * `Unreachable` — nothing was observed; advancing would skip unseen events.
    /// * `CursorRegressed` / `CursorBelowFloor` — the POSITION itself is unproven, so the
    ///   page is not ours to consume. Both abstain; the state file is the operator's repair.
    pub fn advances_cursor(&self) -> bool {
        match self {
            MonitorVerdict::Clear | MonitorVerdict::MailWaiting { .. } => true,
            // The POSITION was proven — both cursor guards passed and the page is
            // trustworthy. What is unresolved is the mailbox, and re-reading the same events
            // cannot resolve it, so withholding the advance would only imply the position is
            // suspect when it is not. The verdict repeats on every run until the surfaces are
            // reconciled, which is intended for a monitor that must not self-clear.
            MonitorVerdict::AuthoritiesDisagree { .. } => true,
            MonitorVerdict::Unreachable { .. }
            | MonitorVerdict::CursorRegressed { .. }
            | MonitorVerdict::CursorBelowFloor { .. } => false,
        }
    }
}

impl fmt::Display for MonitorVerdict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

// ---------------------------------------------------------------------------------------
// The event feed — `am inbox-events --json`
// ---------------------------------------------------------------------------------------

/// One delivery event. Key set confirmed live 2026-09-02 against the installed `am`.
///
/// Every field is REQUIRED. `serde` errors on a missing key here rather than defaulting,
/// which is deliberate: a defaulted `cursor` of 0 or a defaulted empty `from` is exactly how
/// a false zero is manufactured, and a false zero from a monitor is worse than no monitor.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Event {
    /// Delivery cursor. Not a message id.
    pub cursor: u64,
    /// The message this delivery refers to.
    pub message_id: u64,
    /// Delivery kind, e.g. `to`.
    pub kind: String,
    /// RFC3339 delivery timestamp as reported by `am`.
    pub delivered_ts: String,
    /// Sending agent.
    pub from: String,
    /// Message subject.
    pub subject: String,
    /// Importance as reported by `am`.
    pub importance: String,
    /// Whether the message demands an acknowledgement.
    pub ack_required: bool,
}

/// One page of the durable delivery feed.
///
/// Unknown keys are tolerated on purpose — a future `am` adding a field must not turn this
/// monitor blind. A MISSING key is the hard error, because that is the direction that
/// fabricates data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventPage {
    /// The events in this page, oldest first.
    pub events: Vec<Event>,
    /// Whether more events exist beyond this page.
    pub has_more: bool,
    /// The cursor to pass as `--after` on the next call.
    pub next_cursor: u64,
    /// The oldest cursor the durable feed can still replay from, or `None` when this
    /// recipient has NO retained events at all.
    ///
    /// # MEASURED, 2026-09-03: the key is present and EXPLICITLY NULL for a fresh recipient
    ///
    /// ```text
    /// am inbox-events --agent SwiftBeacon --limit 200 --json   (registered seconds earlier)
    /// -> {"events":[],"next_cursor":0,"has_more":false,
    ///     "oldest_available_cursor":null,"tail_cursor":0}
    /// ```
    ///
    /// This field was `u64` and the page therefore FAILED TO PARSE, which the binary
    /// reported as `UNREACHABLE [indeterminate] — invalid type: null, expected u64`. Every
    /// newly registered agent was in that state until its first delivery: a monitor
    /// announcing that it could not observe Agent Mail, about a daemon that answered
    /// correctly. That is a FALSE RESTRICTIVE, and an over-strict gate is worse than none
    /// because it gets routed around.
    ///
    /// `None` is therefore a first-class measured value, not a defaulted absence, and
    /// [`classify`] treats it as "there is no floor to be below" — see the guard there.
    pub oldest_available_cursor: Option<u64>,
    /// The current durable tail.
    pub tail_cursor: u64,
}

impl EventPage {
    /// The delivery position this page PROVES, or `None` when it proves none.
    ///
    /// # The measured defect this exists to prevent
    ///
    /// A recipient with no retained events answers `next_cursor: 0` beside
    /// `oldest_available_cursor: null`. Zero is not a position in this feed — every real
    /// cursor measured is `>= 1` (2058, 5741, 6016) — it is the ABSENCE of one, wearing an
    /// integer.
    ///
    /// Persisting it is how a fresh recipient's very FIRST message becomes a cursor fault.
    /// Measured end to end, 2026-09-03, on an agent registered minutes earlier:
    ///
    /// ```text
    /// t0  inbox-monitor --agent SwiftBeacon        -> clear,  next_cursor 0, cursor file := 0
    /// t1  am mail send --to SwiftBeacon (id 41433) -> first delivery lands at cursor 6016
    /// t2  inbox-monitor --watch                    -> CURSOR BELOW FLOOR (exit 15):
    ///                                                 persisted 0 < oldest_available 6016
    /// ```
    ///
    /// The verdict was correct about its own inputs and useless to its reader: the watch
    /// woke on the right event, 10 s after arming, and then reported a state fault instead
    /// of MAIL WAITING. Every newly registered agent's first message would have done this.
    /// An over-strict guard that fires on the known-good path is worse than no guard,
    /// because it is the one that gets routed around.
    ///
    /// The repair is upstream of the guard, not inside it: never persist a position the feed
    /// did not have. `None` here leaves the cursor file absent, and an absent cursor is the
    /// BASELINE [`classify`] already treats as neither ahead of the tail nor below the floor.
    #[must_use]
    pub fn provable_position(&self) -> Option<u64> {
        // Two conditions, and they are one fact seen from both ends: an empty feed has no
        // floor AND no next position. Either alone is enough to refuse, and requiring both
        // to be present is what makes a future `am` that reports only one of them safe.
        if self.oldest_available_cursor.is_none() || self.next_cursor == 0 {
            None
        } else {
            Some(self.next_cursor)
        }
    }
}

/// The wire shape, before the one nullable field is interpreted.
///
/// `Option<Option<u64>>` is the double-option, and it is here for one reason: it keeps BOTH
/// halves of the contract above. A key that is ABSENT deserializes to the outer `None` and
/// is refused as a hard error; a key that is present and `null` deserializes to `Some(None)`
/// and is the measured empty-feed state. A plain `Option<u64>` would have collapsed those
/// two into each other, quietly re-admitting the missing-key fabrication this struct's doc
/// forbids.
///
/// `deserialize_with` is REQUIRED and is not decoration: `#[serde(default)]` alone does not
/// produce this behaviour. Derived `Option<T>` maps an explicit `null` straight to `None` at
/// the outer level, so the first attempt at this fix reported
/// `missing field \`oldest_available_cursor\`` about a payload that carried the key — the
/// same false restrictive in a new costume, caught by running the binary against the live
/// daemon rather than by reasoning about serde. [`double_option`] forces the inner
/// `Option<u64>` to absorb the null so only true absence reaches the outer layer.
#[derive(Deserialize)]
struct EventPageWire {
    events: Vec<Event>,
    has_more: bool,
    next_cursor: u64,
    #[serde(default, deserialize_with = "double_option")]
    oldest_available_cursor: Option<Option<u64>>,
    tail_cursor: u64,
}

/// Deserialize a present-but-nullable field into `Some(inner)`, leaving the outer `None` to
/// mean "the key was absent" — which `#[serde(default)]` supplies and nothing else can.
fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// A typed parse failure. Never silently a default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// Which surface failed to parse.
    pub what: &'static str,
    /// The underlying detail, kept verbatim so `Unreachable { detail }` stays diagnosable.
    pub detail: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.what, self.detail)
    }
}

impl std::error::Error for ParseError {}

/// Parse `am inbox-events --json` output.
pub fn parse_event_page(json: &str) -> Result<EventPage, ParseError> {
    let wire: EventPageWire = serde_json::from_str(json).map_err(|error| ParseError {
        what: "am inbox-events",
        detail: error.to_string(),
    })?;
    // ABSENT is still fatal. Only an explicit null is a value.
    let oldest_available_cursor = wire.oldest_available_cursor.ok_or_else(|| ParseError {
        what: "am inbox-events",
        detail: "missing field `oldest_available_cursor`".to_string(),
    })?;
    Ok(EventPage {
        events: wire.events,
        has_more: wire.has_more,
        next_cursor: wire.next_cursor,
        oldest_available_cursor,
        tail_cursor: wire.tail_cursor,
    })
}

// ---------------------------------------------------------------------------------------
// The mailbox surface — `am inbox --json`
// ---------------------------------------------------------------------------------------

/// One mailbox row, in this crate's typed shape.
///
/// `read_ts` is `None` exactly when a human still owes an answer. See the MEASURED CONTRACT
/// DRIFT block at the top of this module for how the wire shape maps onto it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxRow {
    /// Message id. Also the ordering key for "oldest": `am` reports `age` as a relative
    /// string, so the monotonic id is the only sortable field on the measured surface.
    pub id: u64,
    /// Sending agent.
    pub from: String,
    /// Message subject.
    pub subject: String,
    /// `None` when unread. `Some(..)` carries whatever read marker the surface exposed.
    pub read_ts: Option<String>,
    /// Importance as reported by `am`.
    pub importance: String,
    /// Whether the message demands an acknowledgement.
    pub ack_required: bool,
}

/// The unread subset — the rows a human still owes an answer to.
pub fn unread(rows: &[InboxRow]) -> Vec<&InboxRow> {
    rows.iter().filter(|row| row.read_ts.is_none()).collect()
}

/// Parse `am inbox --json` output into typed rows.
///
/// Accepts the measured envelope (`{"inbox": [...]}`, alongside `_meta`/`count`/…) and a
/// bare array, in that order. The row adapter is documented in the MEASURED CONTRACT DRIFT
/// block above; `id`, `from` and `subject` are REQUIRED and a row missing any of them is a
/// hard error, never a row with an empty sender.
pub fn parse_inbox_rows(json: &str) -> Result<Vec<InboxRow>, ParseError> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|error| ParseError {
        what: "am inbox",
        detail: error.to_string(),
    })?;

    let array = match &value {
        serde_json::Value::Array(items) => items,
        serde_json::Value::Object(map) => ["inbox", "messages", "rows", "items"]
            .iter()
            .find_map(|key| map.get(*key).and_then(serde_json::Value::as_array))
            .ok_or_else(|| ParseError {
                what: "am inbox",
                detail: format!(
                    "no row array under inbox/messages/rows/items; top-level keys were [{}]",
                    map.keys().cloned().collect::<Vec<_>>().join(", ")
                ),
            })?,
        other => {
            return Err(ParseError {
                what: "am inbox",
                detail: format!("expected an object or array, found {other}"),
            })
        }
    };

    array.iter().map(inbox_row_from_wire).collect()
}

fn required_str(row: &serde_json::Value, key: &'static str) -> Result<String, ParseError> {
    row.get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ParseError {
            what: "am inbox",
            detail: format!("row is missing required string key `{key}`: {row}"),
        })
}

/// Map one measured wire row onto [`InboxRow`].
fn inbox_row_from_wire(row: &serde_json::Value) -> Result<InboxRow, ParseError> {
    let id = row
        .get("id")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| ParseError {
            what: "am inbox",
            detail: format!("row is missing required integer key `id`: {row}"),
        })?;

    // Read state. A literal `read_ts` wins if a future `am` ships one; otherwise the
    // measured authority is `priority`, where every value except `read` means unread.
    let read_ts = match row.get("read_ts") {
        Some(serde_json::Value::String(stamp)) => Some(stamp.clone()),
        Some(serde_json::Value::Null) => None,
        _ => match row.get("priority").and_then(serde_json::Value::as_str) {
            Some("read") => Some(
                row.get("age")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("read")
                    .to_string(),
            ),
            Some(_) => None,
            None => {
                return Err(ParseError {
                    what: "am inbox",
                    detail: format!(
                        "row carries neither `read_ts` nor `priority`, so read state is \
                         unknowable and must not be guessed: {row}"
                    ),
                })
            }
        },
    };

    // Acknowledgement state. Measured `ack_status` set: {none, required, overdue}.
    let ack_required = match row.get("ack_required") {
        Some(serde_json::Value::Bool(flag)) => *flag,
        _ => match row.get("ack_status").and_then(serde_json::Value::as_str) {
            Some(status) => status != "none",
            None => false,
        },
    };

    Ok(InboxRow {
        id,
        from: required_str(row, "from")?,
        subject: required_str(row, "subject")?,
        read_ts,
        importance: row
            .get("importance")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        ack_required,
    })
}

// ---------------------------------------------------------------------------------------
// Classification — pure, no I/O
// ---------------------------------------------------------------------------------------

/// The PRIMARY authority's answer, or an explicit statement that it was not consulted.
///
/// An `Option<usize>` would have done the same job and is exactly what this must not be: a
/// `None` reads as "zero unread" to the next person who writes `unwrap_or_default()`, and a
/// false zero from a monitor is the failure mode this whole crate exists to prevent. The
/// two states are therefore NAMED.
///
/// `NotConsulted` is not a production state on the notification path — there, a daemon that
/// cannot be read is [`MonitorVerdict::Unreachable`], never a skipped arm. It exists for
/// callers that deliberately decline the second authority, and every existing single-arm
/// test passes it so those legs keep testing exactly what they tested before.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonArm {
    /// The daemon answered with this unread count.
    Unread(usize),
    /// The daemon was not asked. The CLI's answer stands alone and no cross-check is made.
    NotConsulted,
}

/// Classify one observation.
///
/// `rows` is expected to be the mailbox surface's rows; the unread filter is re-applied here
/// on purpose. A caller passing the FULL mailbox must not be able to manufacture a
/// `MailWaiting` out of already-read rows, and a caller passing a pre-filtered slice gets
/// the same answer — one function, one definition of unread.
///
/// # ORDER: both cursor-state faults are checked BEFORE the mail check, and that is load-bearing
///
/// An earlier revision put mail first, on the reasoning that mail is the outcome a human can
/// act on immediately while a state repair does not expire. That reasoning is wrong here,
/// for a mechanical reason: a cursor-state fault means the `events` page is untrustworthy,
/// and `MailWaiting`/`Clear` are exactly the verdicts whose emission ADVANCES THE CURSOR
/// (see `main.rs` — the cursor moves for every verdict except `Unreachable`).
///
/// So mail-first does not merely deprioritise the fault, it DESTROYS THE EVIDENCE of it:
/// the run reports a healthy, actionable verdict, writes `page.next_cursor`, and the
/// unresumable position is gone with no one ever learning that positions were skipped. And
/// the daemon will not object — measured 2026-09-02, a read from a cursor below the floor
/// was served from the floor with rc=0 and no error key, so the page arrives from a
/// position the caller never asked for, wearing a normal success shape.
///
/// Reporting `MailWaiting` or `Clear` from an unresumable position is therefore the exact
/// silent blindness this guards against. Both faults are terminal for the observation.
///
/// # ORDER: the two-authority check sits BETWEEN the cursor guards and the mail check
///
/// After the cursor guards, because a disagreement about read state says nothing about the
/// delivery position and must not pre-empt a fault that destroys evidence. Before the mail
/// check, because that is the whole point: with the arms in conflict there is no unread
/// count, and reporting one arm's number would be picking the convenient side silently —
/// which is the defect this check was added to remove.
pub fn classify(
    page: &EventPage,
    rows: &[InboxRow],
    persisted_cursor: Option<u64>,
    daemon: DaemonArm,
) -> MonitorVerdict {
    // `None` is a BASELINE, not a fault: a first run has never been positioned, so it is
    // neither ahead of the tail nor below the floor. Folding `None` into either fault would
    // fire this guard on every fresh install, and a guard that fires on the known-good path
    // is a guard that gets routed around.
    if let Some(persisted) = persisted_cursor {
        // Unchanged condition, unchanged code: our state describes a future the durable
        // feed has not reached, so every further `--after` would SKIP.
        if persisted > page.tail_cursor {
            return MonitorVerdict::CursorRegressed {
                persisted,
                tail: page.tail_cursor,
            };
        }

        // Strictly below this recipient's oldest retained event. Equal to the floor is a
        // legitimate full replay, so the comparison is `<` and never `<=`.
        //
        // A `None` floor is NOT a fault and must never be coerced into one. It means the
        // recipient has no retained events at all (measured: `oldest_available_cursor: null`
        // beside `tail_cursor: 0` and `events: []` for an agent registered seconds earlier),
        // so there is no position a stored cursor could be below — the set of positions the
        // guard compares against is empty. Reaching here with a `Some(persisted)` over an
        // empty feed also means `persisted <= tail_cursor == 0`, which the regression guard
        // above already accepted, so the only stored value that survives to this point is 0
        // itself. Nothing to refuse.
        if let Some(floor) = page.oldest_available_cursor {
            if persisted < floor {
                return MonitorVerdict::CursorBelowFloor {
                    persisted,
                    oldest_available: floor,
                    tail: page.tail_cursor,
                };
            }
        }
    }

    let pending = unread(rows);

    // The two-authority reconciliation. `unread(rows)` is re-applied above, so both sides of
    // this comparison are counted by THIS crate's single definition of unread — the CLI's
    // own `count` envelope field is deliberately not trusted as the second number, because
    // then a disagreement could be manufactured by the envelope alone.
    if let DaemonArm::Unread(daemon_unread) = daemon {
        if daemon_unread != pending.len() {
            return MonitorVerdict::AuthoritiesDisagree {
                daemon_unread,
                cli_unread: pending.len(),
            };
        }
    }

    if let Some(oldest) = pending.iter().min_by_key(|row| row.id) {
        return MonitorVerdict::MailWaiting {
            unread: pending.len(),
            oldest_from: oldest.from.clone(),
            oldest_subject: oldest.subject.clone(),
        };
    }

    MonitorVerdict::Clear
}

// ---------------------------------------------------------------------------------------
// The blocking watch — the wake this crate exists to provide
// ---------------------------------------------------------------------------------------

/// `WatchTimedOut` — a bounded wake was asked for and the bound elapsed with NO arrival.
///
/// A SIXTH distinct number, and the reason it is not [`EXIT_CLEAR`] is the whole point of
/// the mode. A one-shot run that reads both surfaces and finds nothing owed has answered
/// its question: "is anything waiting right now?" — no. That is a legitimate zero and it
/// exits 0. A `--watch` run asks a DIFFERENT question: "block until something arrives."
/// When its ceiling elapses, that question was never answered. Nothing arrived *within the
/// window we were willing to wait*, which is not the same claim as "the mailbox is empty"
/// and is emphatically not success.
///
/// The concrete supervisor shape this protects: `inbox-monitor --watch && drain_mail`. With
/// a clean timeout exiting 0, that composition runs `drain_mail` every time the wake DOESN'T
/// fire — the exact inversion. And a supervisor that loops on 17 to re-arm the wait can do
/// so without ever confusing it with 13, which says the monitor could not look at all.
///
/// **A timeout is not a verdict.** 17 carries no claim about the mailbox beyond "no arrival
/// was observed inside the bound, and every poll in that bound could see".
pub const EXIT_WATCH_TIMED_OUT: u8 = 17;

/// How many CONSECUTIVE unobservable polls establish blindness in a watch.
///
/// Not 1, and not "never". At 1, a single transient refusal — a daemon restart, one dropped
/// connection — aborts a five-minute wake, and an operator whose wake dies on every daemon
/// blip stops using the wake. At "never", a permanently dead daemon is indistinguishable
/// from a quiet mailbox for the whole window, which is the "quietest when blindest" defect
/// this crate was built to remove. Three consecutive failures at the poll cadence is a
/// condition, not a blip, and it is reported through the SAME
/// [`MonitorVerdict::Unreachable`] the one-shot uses, so the exit code and the remedy line
/// are identical whichever mode found it.
pub const WATCH_BLIND_STREAK: u32 = 3;

/// What one poll of a watch decided about whether to keep waiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchStep {
    /// Report this poll's verdict now and stop waiting.
    Stop,
    /// Keep waiting. `consecutive_blind` is the updated unobservable streak, which the
    /// caller must carry into the next call — it is the state this decision is made against.
    Wait {
        /// Unobservable polls in a row, as of this poll. Zero after any successful read.
        consecutive_blind: u32,
    },
}

/// Decide, for one poll of a watch, whether the wait is over.
///
/// Pure and exhaustive over [`MonitorVerdict`], so the watch loop's whole policy is
/// testable with no daemon, no clock, and no subprocess. The wildcard that a `_ =>` arm
/// would introduce is exactly what must not exist here: a newly added verdict silently
/// inheriting "keep waiting" would make a future loud fact invisible for the whole window.
///
/// * [`MonitorVerdict::Clear`] — the ONLY verdict that keeps a watch waiting, and it resets
///   the blind streak: a poll that read both surfaces is proof the monitor can see.
/// * [`MonitorVerdict::MailWaiting`] — the wake. Stop.
/// * [`MonitorVerdict::CursorRegressed`] / [`MonitorVerdict::CursorBelowFloor`] /
///   [`MonitorVerdict::AuthoritiesDisagree`] — loud facts that polling harder cannot
///   resolve. Each would simply repeat on every remaining poll, so continuing would spend
///   the whole window re-deriving a conclusion already reached. Stop.
/// * [`MonitorVerdict::Unreachable`] — tolerate up to `blind_streak_limit - 1` in a row,
///   then stop. See [`WATCH_BLIND_STREAK`].
#[must_use]
pub fn watch_step(
    verdict: &MonitorVerdict,
    consecutive_blind: u32,
    blind_streak_limit: u32,
) -> WatchStep {
    match verdict {
        MonitorVerdict::Clear => WatchStep::Wait {
            consecutive_blind: 0,
        },
        MonitorVerdict::MailWaiting { .. }
        | MonitorVerdict::CursorRegressed { .. }
        | MonitorVerdict::CursorBelowFloor { .. }
        | MonitorVerdict::AuthoritiesDisagree { .. } => WatchStep::Stop,
        MonitorVerdict::Unreachable { .. } => {
            let streak = consecutive_blind.saturating_add(1);
            // `.max(1)` so a caller passing 0 cannot build a watch that tolerates blindness
            // forever — the degenerate limit collapses to "stop on the first blind poll",
            // which is restrictive, rather than to "never stop", which is silent.
            if streak >= blind_streak_limit.max(1) {
                WatchStep::Stop
            } else {
                WatchStep::Wait {
                    consecutive_blind: streak,
                }
            }
        }
    }
}

/// How a bounded watch ended.
///
/// Three terminals, and the split between the last two is the one that matters: a bound
/// that elapsed while the monitor COULD see is a different fact from a bound that elapsed
/// while it could not, and folding them together would let a dead daemon masquerade as a
/// quiet mailbox for exactly as long as the operator was willing to wait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchOutcome {
    /// A poll produced a verdict the operator must answer, inside the bound. Carries it
    /// verbatim, so a watch and a one-shot report the SAME verdict with the SAME code.
    Settled {
        /// The verdict that ended the wait.
        verdict: MonitorVerdict,
        /// How many polls it took, so a wake's latency is readable from the artifact.
        polls: u32,
        /// Seconds from the first poll to the settling one.
        waited_secs: u64,
    },
    /// The bound elapsed, every poll was REACHABLE, and every one said `Clear`.
    ///
    /// The legitimate zero of a watch — and still NOT [`MonitorVerdict::Clear`]. See
    /// [`EXIT_WATCH_TIMED_OUT`].
    TimedOut {
        /// Polls completed inside the bound. Zero would mean the bound was shorter than one
        /// read, which is a caller error and is reported as such rather than as a quiet pass.
        polls: u32,
        /// Seconds actually waited.
        waited_secs: u64,
        /// The ceiling the caller asked for, beside what was actually spent.
        bound_secs: u64,
    },
    /// The bound elapsed with the TRAILING polls unobservable — blind, but never for
    /// [`WATCH_BLIND_STREAK`] in a row, so the streak rule never fired.
    ///
    /// This is the shape that would otherwise be reported as a clean timeout, and it is the
    /// one that must not be: the window ended without a wake AND the last thing the monitor
    /// knew was that it could not see. It exits [`EXIT_UNREACHABLE`], identically to a
    /// one-shot that could not look, because that is the same fact.
    TimedOutBlind {
        /// The last unobservable verdict, so the remedy line is the specific one.
        verdict: MonitorVerdict,
        /// Polls completed inside the bound.
        polls: u32,
        /// Seconds actually waited.
        waited_secs: u64,
        /// The ceiling the caller asked for.
        bound_secs: u64,
        /// How many unobservable polls the window ended on.
        trailing_blind: u32,
    },
}

impl WatchOutcome {
    /// The process exit code. The ONLY place a watch terminal becomes an integer.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            // Identical to the one-shot's code for the same verdict — a wake that found mail
            // and a poll that found mail are the same fact and must not differ by mode.
            WatchOutcome::Settled { verdict, .. } => verdict.exit_code(),
            WatchOutcome::TimedOut { .. } => EXIT_WATCH_TIMED_OUT,
            WatchOutcome::TimedOutBlind { .. } => EXIT_UNREACHABLE,
        }
    }

    /// The stable machine label for the watch TERMINAL, emitted beside the verdict label.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            WatchOutcome::Settled { .. } => "watch_settled",
            WatchOutcome::TimedOut { .. } => "watch_timed_out",
            WatchOutcome::TimedOutBlind { .. } => "watch_timed_out_blind",
        }
    }

    /// The label that goes in the row's `verdict` field.
    ///
    /// A clean timeout deliberately does NOT report `clear` here. The row is the artifact an
    /// operator greps months later, and `verdict: "clear"` on a run that was asked to block
    /// for a wake is a false negative written to disk.
    #[must_use]
    pub fn verdict_label(&self) -> &'static str {
        match self {
            WatchOutcome::Settled { verdict, .. } | WatchOutcome::TimedOutBlind { verdict, .. } => {
                verdict.label()
            }
            WatchOutcome::TimedOut { .. } => "watch_timed_out",
        }
    }

    /// The verdict that ended the wait, when a poll produced one.
    #[must_use]
    pub fn verdict(&self) -> Option<&MonitorVerdict> {
        match self {
            WatchOutcome::Settled { verdict, .. } | WatchOutcome::TimedOutBlind { verdict, .. } => {
                Some(verdict)
            }
            WatchOutcome::TimedOut { .. } => None,
        }
    }

    /// May a run ending in this watch terminal advance the persisted cursor?
    ///
    /// Positive and exhaustive, for the same reason [`MonitorVerdict::advances_cursor`] is.
    #[must_use]
    pub fn advances_cursor(&self) -> bool {
        match self {
            WatchOutcome::Settled { verdict, .. } => verdict.advances_cursor(),
            // Every poll read both surfaces and every one was clear, so the position is as
            // proven as any one-shot `Clear`.
            WatchOutcome::TimedOut { .. } => true,
            // The window ended blind. Advancing here would move the cursor past events no
            // successful read ever saw.
            WatchOutcome::TimedOutBlind { .. } => false,
        }
    }

    /// The line a human reads.
    #[must_use]
    pub fn human_line(&self) -> String {
        match self {
            WatchOutcome::Settled {
                verdict,
                polls,
                waited_secs,
            } => format!(
                "{} [watch settled after {polls} poll(s), {waited_secs}s]",
                verdict.human_line()
            ),
            WatchOutcome::TimedOut {
                polls,
                waited_secs,
                bound_secs,
            } => format!(
                "inbox-monitor: WATCH TIMED OUT — no arrival in {waited_secs}s of a \
                 {bound_secs}s bound across {polls} reachable poll(s). This is NOT 'the \
                 inbox is empty' (that is exit {EXIT_CLEAR}, from a one-shot run) and NOT \
                 'could not look' (exit {EXIT_UNREACHABLE}). The wake did not fire inside \
                 the window; re-arm the wait or raise --timeout"
            ),
            WatchOutcome::TimedOutBlind {
                verdict,
                polls,
                waited_secs,
                bound_secs,
                trailing_blind,
            } => format!(
                "inbox-monitor: WATCH ENDED BLIND — the {bound_secs}s bound elapsed after \
                 {waited_secs}s and {polls} poll(s), and the last {trailing_blind} could not \
                 observe. NOT a clean timeout: no wake fired AND the monitor was blind when \
                 the window closed, so nothing may be concluded about the mailbox. \
                 Underlying: {}",
                verdict.human_line()
            ),
        }
    }
}

impl fmt::Display for WatchOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

// ---------------------------------------------------------------------------------------
// State: cursor + ledger paths, and durable cursor persistence
// ---------------------------------------------------------------------------------------

/// Why a state path could not be resolved. Never a home-path literal — a hardcoded
/// checkout COMPILES fine after a move and then silently reads the WRONG state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// `HOME` is unset or not valid UTF-8. There is no fallback, by design.
    HomeUnset,
    /// The agent name cannot be used as a filename component.
    InvalidAgent {
        /// The rejected name.
        agent: String,
        /// Why it was rejected.
        reason: &'static str,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::HomeUnset => formatter.write_str(
                "HOME is unset; refusing to guess a state directory (a guessed path reads the \
                 wrong state silently)",
            ),
            ConfigError::InvalidAgent { agent, reason } => {
                write!(
                    formatter,
                    "agent name {agent:?} is unusable as a path component: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for ConfigError {}

/// A cursor read or write failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorError {
    /// The filesystem refused.
    Io {
        /// What was being attempted.
        op: &'static str,
        /// The path involved.
        path: PathBuf,
        /// The OS-level detail.
        detail: String,
    },
    /// The file exists but does not hold a cursor.
    Malformed {
        /// The path involved.
        path: PathBuf,
        /// The bytes we could not read as a cursor.
        contents: String,
    },
}

impl fmt::Display for CursorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CursorError::Io { op, path, detail } => {
                write!(formatter, "{op} {}: {detail}", path.display())
            }
            CursorError::Malformed { path, contents } => write!(
                formatter,
                "{} does not hold a cursor: {contents:?}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for CursorError {}

/// The state directory, given an explicit home. Pure — this is the testable seam.
pub fn state_dir_in(home: &Path) -> PathBuf {
    home.join(".local")
        .join("state")
        .join("flywheel")
        .join("inbox-monitor")
}

/// `HOME` resolved at runtime, or [`ConfigError::HomeUnset`]. Never a literal.
pub fn home_dir() -> Result<PathBuf, ConfigError> {
    match std::env::var_os("HOME") {
        Some(value) if !value.is_empty() => Ok(PathBuf::from(value)),
        _ => Err(ConfigError::HomeUnset),
    }
}

/// The state directory for this fleet.
pub fn state_dir() -> Result<PathBuf, ConfigError> {
    Ok(state_dir_in(&home_dir()?))
}

/// Reject an agent name that cannot be a single filename component.
///
/// `..` or a separator would escape the state directory; an empty name would collide with
/// the directory itself. This is the one place a caller-supplied string reaches the
/// filesystem, so it is validated here rather than at each call site.
fn validate_agent(agent: &str) -> Result<(), ConfigError> {
    if agent.is_empty() {
        return Err(ConfigError::InvalidAgent {
            agent: agent.to_string(),
            reason: "empty",
        });
    }
    if agent.contains('/') || agent.contains('\\') || agent.contains('\0') {
        return Err(ConfigError::InvalidAgent {
            agent: agent.to_string(),
            reason: "contains a path separator or NUL",
        });
    }
    if agent == "." || agent == ".." {
        return Err(ConfigError::InvalidAgent {
            agent: agent.to_string(),
            reason: "is a directory traversal component",
        });
    }
    Ok(())
}

/// The cursor file for one agent, given an explicit home.
pub fn cursor_path_in(home: &Path, agent: &str) -> Result<PathBuf, ConfigError> {
    validate_agent(agent)?;
    Ok(state_dir_in(home).join(format!("{agent}.cursor")))
}

/// The cursor file for one agent: `$HOME/.local/state/flywheel/inbox-monitor/<agent>.cursor`.
pub fn cursor_path(agent: &str) -> Result<PathBuf, ConfigError> {
    cursor_path_in(&home_dir()?, agent)
}

/// The append-only notification ledger, given an explicit home.
pub fn ledger_path_in(home: &Path) -> PathBuf {
    state_dir_in(home).join("ledger.jsonl")
}

/// The append-only notification ledger.
pub fn ledger_path() -> Result<PathBuf, ConfigError> {
    Ok(ledger_path_in(&home_dir()?))
}

/// Read the persisted cursor. `Ok(None)` means "never positioned", which is a legitimate
/// first-run state and NOT an error — but a file that exists and holds garbage IS an error,
/// because treating it as zero would replay the entire retained feed.
pub fn read_cursor(path: &Path) -> Result<Option<u64>, CursorError> {
    let raw = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(CursorError::Io {
                op: "read cursor",
                path: path.to_path_buf(),
                detail: error.to_string(),
            })
        }
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(CursorError::Malformed {
            path: path.to_path_buf(),
            contents: raw,
        });
    }
    trimmed
        .parse::<u64>()
        .map(Some)
        .map_err(|_| CursorError::Malformed {
            path: path.to_path_buf(),
            contents: raw,
        })
}

/// Write the cursor durably: temp file in the SAME directory, fsync, then rename.
///
/// A plain truncating write can leave a half-written cursor if the process dies mid-write,
/// and a truncated cursor parses as a *smaller* number — which replays, or worse, parses as
/// a larger prefix and skips. Rename within one directory is atomic, so a reader sees either
/// the old cursor or the new one and never a prefix of either.
pub fn write_cursor_atomic(path: &Path, cursor: u64) -> Result<(), CursorError> {
    use std::io::Write as _;

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| CursorError::Io {
        op: "create state dir",
        path: parent.to_path_buf(),
        detail: error.to_string(),
    })?;

    let temp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("cursor"),
        std::process::id()
    ));

    {
        let mut file = std::fs::File::create(&temp).map_err(|error| CursorError::Io {
            op: "create temp cursor",
            path: temp.clone(),
            detail: error.to_string(),
        })?;
        file.write_all(cursor.to_string().as_bytes())
            .map_err(|error| CursorError::Io {
                op: "write temp cursor",
                path: temp.clone(),
                detail: error.to_string(),
            })?;
        file.sync_all().map_err(|error| CursorError::Io {
            op: "fsync temp cursor",
            path: temp.clone(),
            detail: error.to_string(),
        })?;
    }

    std::fs::rename(&temp, path).map_err(|error| {
        let _ = std::fs::remove_file(&temp);
        CursorError::Io {
            op: "rename cursor into place",
            path: path.to_path_buf(),
            detail: error.to_string(),
        }
    })
}

/// Append one JSON row to the ledger, durably, creating the directory if needed.
///
/// Returns only after `fsync`, because the cursor may only advance AFTER this returns: a
/// crash between the read and the append must REPLAY, never skip.
pub fn append_ledger(path: &Path, row: &str) -> Result<(), CursorError> {
    use std::io::Write as _;

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| CursorError::Io {
        op: "create state dir",
        path: parent.to_path_buf(),
        detail: error.to_string(),
    })?;

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| CursorError::Io {
            op: "open ledger",
            path: path.to_path_buf(),
            detail: error.to_string(),
        })?;
    let mut line = row.trim_end().to_string();
    line.push('\n');
    file.write_all(line.as_bytes())
        .map_err(|error| CursorError::Io {
            op: "append ledger",
            path: path.to_path_buf(),
            detail: error.to_string(),
        })?;
    file.sync_all().map_err(|error| CursorError::Io {
        op: "fsync ledger",
        path: path.to_path_buf(),
        detail: error.to_string(),
    })
}

// ---------------------------------------------------------------------------------------
// Pane attribution
// ---------------------------------------------------------------------------------------

/// Resolve a `pane_id` from `tmux list-panes -a -F '#{pane_id} #{session_name}:#{window_index}.#{pane_index}'`.
///
/// Pure over a `&str` so pane attribution is testable without tmux.
///
/// # Why not `tmux display-message -p`
///
/// Measured self-identity trap: `tmux display-message -p` WITHOUT `-t` answers for the
/// ACTIVE pane, not the calling one. A monitor running in a background pane would therefore
/// attribute its notification to whatever pane the operator happened to be looking at.
///
/// # Why ambiguity returns `None`
///
/// Pane indices are not stable handles — they shifted twice in one evening. A
/// `session_name` plus `pane_index` can match panes in several WINDOWS of the same session,
/// and there is no correct guess among them. Returning `None` for both "absent" and
/// "ambiguous" is deliberate: the emitted row records the index the resolution was attempted
/// FROM, so a null pane is diagnosable, whereas a guessed pane is not.
pub fn resolve_pane(listing: &str, session: &str, index: u32) -> Option<String> {
    let suffix = format!(".{index}");
    let prefix = format!("{session}:");

    let mut matches = listing.lines().filter_map(|line| {
        let mut parts = line.split_whitespace();
        let pane_id = parts.next()?;
        let location = parts.next()?;
        if parts.next().is_some() {
            return None;
        }
        if !pane_id.starts_with('%') {
            return None;
        }
        if location.starts_with(&prefix) && location.ends_with(&suffix) {
            Some(pane_id.to_string())
        } else {
            None
        }
    });

    let first = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(first)
}

// ---------------------------------------------------------------------------------------
// Timestamps
// ---------------------------------------------------------------------------------------

/// Civil date from a days-since-epoch count (Howard Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    (year, month as u32, day)
}

/// Render a UNIX epoch second count as RFC3339 UTC, so the ledger is legible without a
/// converter. Pure, and unit-tested against known epochs.
pub fn iso8601_utc(epoch_secs: u64) -> String {
    let days = (epoch_secs / 86_400) as i64;
    let rem = epoch_secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Wall-clock now, in epoch seconds. `0` when the clock is before the epoch, which is not a
/// condition worth a typed error in a notifier.
pub fn now_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|delta| delta.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso8601_matches_known_epochs() {
        assert_eq!(iso8601_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso8601_utc(1_000_000_000), "2001-09-09T01:46:40Z");
        assert_eq!(iso8601_utc(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn every_verdict_has_a_distinct_code_except_clear() {
        let codes = [
            MonitorVerdict::Clear.exit_code(),
            MonitorVerdict::MailWaiting {
                unread: 1,
                oldest_from: "a".into(),
                oldest_subject: "b".into(),
            }
            .exit_code(),
            MonitorVerdict::Unreachable { detail: "x".into(), reason: UnreachableReason::Indeterminate }.exit_code(),
            MonitorVerdict::CursorRegressed {
                persisted: 2,
                tail: 1,
            }
            .exit_code(),
            MonitorVerdict::CursorBelowFloor {
                persisted: 5105,
                oldest_available: 5147,
                tail: 5165,
            }
            .exit_code(),
        ];
        let mut sorted = codes.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), codes.len(), "verdict codes must be distinct");
        for code in codes.iter().skip(1) {
            assert!(
                (5..=63).contains(code),
                "nonzero codes must come from the unallocated band, got {code}"
            );
        }
    }

    #[test]
    fn an_agent_name_cannot_escape_the_state_directory() {
        let home = Path::new("/tmp/does-not-need-to-exist");
        assert!(cursor_path_in(home, "../../etc/passwd").is_err());
        assert!(cursor_path_in(home, "").is_err());
        assert!(cursor_path_in(home, "..").is_err());
        // POSITIVE CONTROL: a normal agent name resolves.
        let good = cursor_path_in(home, "AmberGate").expect("normal agent name must resolve");
        assert!(good.ends_with("AmberGate.cursor"));
    }
}

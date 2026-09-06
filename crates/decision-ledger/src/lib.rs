#![forbid(unsafe_code)]

//! **S9: the human-decision ledger, which had a reader and no writer.**
//!
//! # The measured loss, 2026-09-02
//!
//! `docs/decisions.jsonl` received ZERO machine-written rows while the fleet emitted
//! hundreds of rows addressed to a human. Measured on the heartbeat ledger for the day:
//!
//! ```text
//! human-addressed rows (detail matches owner=josh|AWAIT_HUMAN), by status:
//!   101 DISPATCH_RESULT_RECORDED
//!   188 GATE_UNWIRED
//!   305 SUPERVISOR_REFUSED
//!
//! the same rows, collapsed by shape:
//!   168  unwired=ack-spine[UNWIRED->repair-gate-trigger] owner=josh
//!    22  DISPATCH_BLOCKED bead=<id> receiver agent is missing owner=josh next_action=claim-bead
//!    16  MONITOR_BLIND owner=josh next_action=repair-monitor detail=zero panes observed
//! ```
//!
//! The only code that touched the file READ it, to cite HD-0001. Every decision the loop
//! actually needed lived in pane scrollback, a bead comment, or a progress file.
//!
//! # The falsifier the bead offered, and why it is too weak
//!
//! The bead said any row with `ts >= 1788328800` refutes "reader and no writer". One
//! appeared: `HD-0008` at 22:55:37Z, `recorded_by: SnowyCanyon`, landed by hand inside
//! skill-loop commit `3b34887`. **A row entered by hand proves rows can exist, not that a
//! writer exists.** The load-bearing half of that falsifier is the second clause — a
//! non-comment `decisions.jsonl` append site under `crates/*/src` — and it was still empty
//! when this crate was written.
//!
//! # What this crate does NOT decide
//!
//! Rule Zero: the decision is the human's. This appends the QUESTION so it stops being
//! lost, and records the ANSWER when a human gives one. It never invents a `decision`
//! field, and `record_decision` refuses a row whose decider is empty.

use std::collections::BTreeSet;
use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use serde_json::{json, Map, Value};

pub mod execution;


/// A question the loop needs a human to answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// What is being asked, in one sentence a human can answer without context.
    pub question: String,
    /// Which component asked. Never a pid — a pid dies with the process that wrote it.
    pub asked_by: String,
    /// What is halted: `gate:<name>` or `bead:<id>`.
    pub blocking: String,
    /// Unix seconds.
    pub ts: u64,
}

/// A human's answer to an existing request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// The `HD-` id of the request being answered.
    pub id: String,
    /// What was decided, in the decider's own words where possible.
    pub decision: String,
    /// Who decided. An empty decider is REFUSED: an unattributed decision is a rumour.
    pub decider: String,
    /// Which agent transcribed it.
    pub recorded_by: String,
    /// Where the decision can be read back — a bead id, a commit, a transcript.
    pub transcript_ref: String,
}

/// What `append_request` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppendOutcome {
    /// A new `HD-` row was written.
    Appended { id: String },
    /// An open request with the same question already exists.
    ///
    /// This is the whole point of the dedupe key: 188 identical `GATE_UNWIRED` rows are
    /// ONE question asked 188 times, and a ledger that records it 188 times is as
    /// unreadable as one that records it never.
    Deduped { id: String },
}

impl AppendOutcome {
    /// The row's id, whether it was written now or already present.
    pub fn id(&self) -> &str {
        match self {
            Self::Appended { id } | Self::Deduped { id } => id,
        }
    }

    /// Did this call write to the file?
    pub fn wrote(&self) -> bool {
        matches!(self, Self::Appended { .. })
    }
}

/// Why a ledger operation was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    /// The ledger could not be read. Fail closed: an unreadable ledger must not be
    /// treated as an empty one, or the next append starts renumbering from HD-0001 and
    /// silently forks the id space.
    Unreadable { path: String, detail: String },
    /// A line was not JSON. Named with its line number rather than skipped.
    Malformed { line: usize, detail: String },
    /// The ledger could not be appended to.
    NotWritable { path: String, detail: String },
    /// `record_decision` was given a request id that is not in the ledger.
    NoSuchRequest { id: String },
    /// A field a row cannot be honest without was empty.
    MissingField { field: &'static str },
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { path, detail } => write!(
                f,
                "DECISION_LEDGER_UNREADABLE path={path} detail={detail} \
                 next_action=repair-or-name-the-path -- an unreadable ledger is NOT an \
                 empty one; appending would fork the HD- id space"
            ),
            Self::Malformed { line, detail } => write!(
                f,
                "DECISION_LEDGER_MALFORMED line={line} detail={detail} \
                 next_action=fix-the-row -- a skipped row is a lost decision"
            ),
            Self::NotWritable { path, detail } => {
                write!(
                    f,
                    "DECISION_LEDGER_NOT_WRITABLE path={path} detail={detail}"
                )
            }
            Self::NoSuchRequest { id } => write!(
                f,
                "DECISION_LEDGER_NO_SUCH_REQUEST id={id} \
                 next_action=append-the-request-first -- a decision with no question is \
                 an answer nobody can audit"
            ),
            Self::MissingField { field } => write!(
                f,
                "DECISION_LEDGER_MISSING_FIELD field={field} \
                 next_action=supply-it -- Rule Zero: the decision is the human's, so an \
                 unattributed one is refused rather than recorded"
            ),
        }
    }
}

/// One parsed ledger row, kept as a `Value` so the schema stays ADDITIVE.
///
/// The existing file carries 12-14 keys across two shapes already
/// (`residual_risk_stated_at_decision_time` appears on one row and `condition` on
/// another). A rigid struct would have to choose, and choosing would rewrite rows this
/// crate did not author.
#[derive(Debug, Clone)]
pub struct Row {
    /// The raw object, preserved byte-for-byte on read.
    pub value: Value,
}

impl Row {
    /// The row's `HD-` id, if it has one.
    pub fn id(&self) -> Option<&str> {
        self.value.get("id").and_then(Value::as_str)
    }

    /// The question text, if any.
    pub fn question(&self) -> Option<&str> {
        self.value.get("question").and_then(Value::as_str)
    }

    /// Has a human answered this row?
    ///
    /// Keyed on a NON-EMPTY `decision`, so a request row and an answered row are
    /// distinguishable without a new field — which is what keeps the extension additive.
    pub fn is_answered(&self) -> bool {
        self.value
            .get("decision")
            .and_then(Value::as_str)
            .is_some_and(|decision| !decision.trim().is_empty())
    }

    /// The numeric part of an `HD-NNNN` id.
    pub fn sequence(&self) -> Option<u32> {
        self.id()?.strip_prefix("HD-")?.parse().ok()
    }
}

/// A stable, printable dedupe key for a question.
///
/// FNV-1a over the normalized question. Not a cryptographic hash and does not need to be:
/// it distinguishes questions within one file, and being reproducible in one line of code
/// matters more here than collision resistance. Normalization is whitespace-and-case only
/// — deliberately NOT digit-stripping, because `bead=…-oj6.3` and `bead=…-oj6.4` are
/// DIFFERENT decisions and a digit-blind key would merge them into one.
pub fn question_key(question: &str) -> String {
    let normalized = question.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in normalized.to_lowercase().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Read every row, refusing rather than skipping.
///
/// An absent file is an EMPTY ledger and not an error — a repo that has never recorded a
/// decision is a real state. An unreadable one IS an error, because it is
/// indistinguishable from empty exactly when it matters.
pub fn read_rows(path: &Path) -> Result<Vec<Row>, LedgerError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(LedgerError::Unreadable {
                path: path.display().to_string(),
                detail: error.to_string(),
            })
        }
    };
    let mut rows = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line).map_err(|error| LedgerError::Malformed {
            line: index + 1,
            detail: error.to_string(),
        })?;
        rows.push(Row { value });
    }
    Ok(rows)
}

/// The next free `HD-` id, one past the highest present.
///
/// Reads the MAX rather than counting rows, so a deleted row cannot cause a reused id.
pub fn next_id(rows: &[Row]) -> String {
    let highest = rows.iter().filter_map(Row::sequence).max().unwrap_or(0);
    format!("HD-{:04}", highest + 1)
}

/// Every question already in the ledger, by dedupe key.
pub fn existing_keys(rows: &[Row]) -> BTreeSet<String> {
    rows.iter()
        .filter_map(Row::question)
        .map(question_key)
        .collect()
}

/// Render a request as a ledger row.
///
/// Every key the existing schema uses is present, empty where a request cannot yet know
/// it. An absent key and an empty one read differently to `jq`, and the reader
/// (`main.rs`) indexes by name.
pub fn request_row(id: &str, request: &Request) -> Value {
    json!({
        "id": id,
        "ts": request.ts,
        "question": request.question,
        "asked_by": request.asked_by,
        "blocking": request.blocking,
        "question_key": question_key(&request.question),
        "options_considered": Value::Array(Vec::new()),
        "decision": "",
        "decider": "",
        "recorded_by": request.asked_by,
        "binds_stages": json!(["S9"]),
        "transcript_ref": "",
        "review_after": "",
        "supersedes": "",
    })
}

/// Append a request unless its question is already open in the ledger.
pub fn append_request(path: &Path, request: &Request) -> Result<AppendOutcome, LedgerError> {
    if request.question.trim().is_empty() {
        return Err(LedgerError::MissingField { field: "question" });
    }
    if request.blocking.trim().is_empty() {
        return Err(LedgerError::MissingField { field: "blocking" });
    }
    let rows = read_rows(path)?;
    let key = question_key(&request.question);
    if let Some(existing) = rows
        .iter()
        .find(|row| row.question().map(question_key).as_deref() == Some(key.as_str()))
    {
        return Ok(AppendOutcome::Deduped {
            id: existing.id().unwrap_or("HD-????").to_owned(),
        });
    }
    let id = next_id(&rows);
    append_line(path, &request_row(&id, request))?;
    Ok(AppendOutcome::Appended { id })
}

/// Record a human's answer to an existing request.
///
/// Appends rather than rewriting the request row: the ledger is an append-only log, and a
/// question that was open for two hours is itself the finding. Rewriting would erase it.
pub fn record_decision(path: &Path, decision: &Decision) -> Result<(), LedgerError> {
    if decision.decider.trim().is_empty() {
        return Err(LedgerError::MissingField { field: "decider" });
    }
    if decision.decision.trim().is_empty() {
        return Err(LedgerError::MissingField { field: "decision" });
    }
    let rows = read_rows(path)?;
    let request = rows
        .iter()
        .find(|row| row.id() == Some(decision.id.as_str()))
        .ok_or_else(|| LedgerError::NoSuchRequest {
            id: decision.id.clone(),
        })?;
    let mut row = Map::new();
    row.insert("id".into(), json!(decision.id));
    row.insert("ts".into(), json!(now_unix()));
    row.insert(
        "question".into(),
        json!(request.question().unwrap_or_default()),
    );
    row.insert("answers".into(), json!(decision.id));
    row.insert("decision".into(), json!(decision.decision));
    row.insert("decider".into(), json!(decision.decider));
    row.insert("recorded_by".into(), json!(decision.recorded_by));
    row.insert("transcript_ref".into(), json!(decision.transcript_ref));
    row.insert("binds_stages".into(), json!(["S9"]));
    row.insert("options_considered".into(), Value::Array(Vec::new()));
    row.insert("review_after".into(), json!(""));
    row.insert("supersedes".into(), json!(""));
    append_line(path, &Value::Object(row))
}

fn append_line(path: &Path, row: &Value) -> Result<(), LedgerError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| LedgerError::NotWritable {
            path: parent.display().to_string(),
            detail: error.to_string(),
        })?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| LedgerError::NotWritable {
            path: path.display().to_string(),
            detail: error.to_string(),
        })?;
    // ONE write of one line: a partial row is worse than a missing one, because a reader
    // refuses the whole file on a malformed line.
    let line = format!("{}\n", serde_json::to_string(row).unwrap_or_default());
    file.write_all(line.as_bytes())
        .map_err(|error| LedgerError::NotWritable {
            path: path.display().to_string(),
            detail: error.to_string(),
        })
}

/// Wall-clock seconds. Isolated so tests can reason about the rest without a clock.
pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// Turn one heartbeat row into a human-decision request, or `None`.
///
/// # Keyed on the SUBJECT, not the text
///
/// The 188 `GATE_UNWIRED` rows carry a byte-identical detail, but the `DISPATCH_BLOCKED`
/// rows carry a pane id and a tick number that change every time. A textual key would
/// emit one row per tick for those; a digit-stripped key would merge two different beads.
/// So the question is COMPOSED from the extracted subject — `gate:ack-spine`,
/// `bead:omp-orchestrator-x` — and the dedupe key falls out of that.
///
/// # Unclassified is not discarded
///
/// A row that says `owner=josh` and matches no known shape still produces a request, with
/// its detail as the question. A classifier that silently drops the case it did not
/// anticipate is how S9 got a reader and no writer in the first place.
pub fn classify_heartbeat(status: &str, detail: &str, ts: u64) -> Option<Request> {
    let addressed = detail.contains("owner=josh") || detail.contains("AWAIT_HUMAN");
    if !addressed {
        return None;
    }
    let asked_by = "omp-orchestrator-supervisor".to_owned();

    if let Some(gate) = field_after(detail, "unwired=") {
        let gate = gate.split(['[', ' ']).next().unwrap_or(gate);
        return Some(Request {
            question: format!(
                "Gate {gate} has no reachable trigger. Wire it, retire it, or declare it \
                 advisory with a named reason?"
            ),
            asked_by,
            blocking: format!("gate:{gate}"),
            ts,
        });
    }
    if status == "MONITOR_BLIND" || detail.starts_with("MONITOR_BLIND") {
        return Some(Request {
            question: "tick-monitor observed zero panes, so the supervisor is blind. \
                       Repair the monitor or authorise dispatch without a census?"
                .to_owned(),
            asked_by,
            blocking: "gate:tick-monitor".to_owned(),
            ts,
        });
    }
    if let Some(bead) = field_after(detail, "bead=") {
        let bead = bead.split_whitespace().next().unwrap_or(bead);
        let subject = if detail.contains("AWAIT_HUMAN") {
            format!("Bead {bead} has exhausted its ack retries and is awaiting a human.")
        } else {
            format!("Bead {bead} cannot be dispatched: its receiver agent is missing.")
        };
        return Some(Request {
            question: format!("{subject} Assign a holder, re-scope it, or park it?"),
            asked_by,
            blocking: format!("bead:{bead}"),
            ts,
        });
    }
    // The anticipated-shape list ends here, and the row still names a human. Carry it.
    Some(Request {
        question: format!(
            "UNCLASSIFIED human-addressed refusal from status {status}: {}",
            detail.split_whitespace().collect::<Vec<_>>().join(" ")
        ),
        asked_by,
        blocking: format!("status:{status}"),
        ts,
    })
}

/// The value following `needle`, up to the next whitespace.
fn field_after<'a>(haystack: &'a str, needle: &str) -> Option<&'a str> {
    let start = haystack.find(needle)? + needle.len();
    let rest = &haystack[start..];
    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let value = &rest[..end];
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

/// One distinct question found by a replay, with how many rows produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayEntry {
    /// The composed question.
    pub question: String,
    /// What it blocks.
    pub blocking: String,
    /// How many heartbeat rows collapsed into this one question.
    pub occurrences: usize,
    /// Earliest occurrence.
    pub first_ts: u64,
}

/// What a replay found, including the arithmetic that makes a zero readable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replay {
    /// Heartbeat rows read.
    pub rows_read: usize,
    /// Rows whose detail addressed a human.
    pub human_addressed: usize,
    /// Distinct questions, sorted by first occurrence.
    pub distinct: Vec<ReplayEntry>,
}

impl Replay {
    /// ANTI-VACUITY: a zero must be readable as either "nothing to record" or "the
    /// replay is broken", and those are different states.
    ///
    /// Returns `Err` with the arithmetic when the input carried no rows at all, because
    /// an empty scan reports exactly like a clean one.
    pub fn require_nonvacuous(&self) -> Result<(), String> {
        if self.rows_read == 0 {
            return Err(format!(
                "REPLAY_VACUOUS rows_read=0 -- nothing was read, which is not the same as \
                 nothing to record. Check the ledger path and the --since window before \
                 believing human_addressed={} distinct={}",
                self.human_addressed,
                self.distinct.len()
            ));
        }
        Ok(())
    }
}

/// Replay heartbeat rows into the distinct requests they should have produced.
///
/// Pure: takes `(status, detail, ts)` triples and writes nothing, so the dry-run and the
/// real run share one classifier and cannot disagree.
pub fn replay(rows: &[(String, String, u64)]) -> Replay {
    let mut order: Vec<ReplayEntry> = Vec::new();
    let mut human_addressed = 0usize;
    for (status, detail, ts) in rows {
        let Some(request) = classify_heartbeat(status, detail, *ts) else {
            continue;
        };
        human_addressed += 1;
        let key = question_key(&request.question);
        if let Some(entry) = order
            .iter_mut()
            .find(|entry| question_key(&entry.question) == key)
        {
            entry.occurrences += 1;
            entry.first_ts = entry.first_ts.min(*ts);
        } else {
            order.push(ReplayEntry {
                question: request.question,
                blocking: request.blocking,
                occurrences: 1,
                first_ts: *ts,
            });
        }
    }
    order.sort_by_key(|entry| entry.first_ts);
    Replay {
        rows_read: rows.len(),
        human_addressed,
        distinct: order,
    }
}

/// Extract an `HD-` id from a bead close reason, for the decision half.
///
/// Acceptance 1's second clause: a close carrying `--reason "HD-…"` records the answer, a
/// close without one records nothing. Returning `None` for the second case is the point —
/// a close is not automatically a decision.
pub fn hd_reference(reason: &str) -> Option<String> {
    let start = reason.find("HD-")?;
    let rest = &reason[start + 3..];
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    Some(format!("HD-{digits}"))
}

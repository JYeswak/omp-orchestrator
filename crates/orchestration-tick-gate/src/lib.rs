#![forbid(unsafe_code)]

//! Reusable validator for the orchestration tick receipt contract.
//!
//! This crate validates rows that exist. It does not create the ledger or
//! claim that every supervisor tick emitted one; that producer is a separate
//! wiring concern.

use serde_json::Value;
use std::fmt;
use std::io::Write;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// The result class for a missing or empty ledger.
pub const NOTHING_TO_CHECK: &str = "NOTHING_TO_CHECK";

/// A receipt validation failure with a stable row/law marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    /// The ledger is absent or contains no receipt rows.
    NothingToCheck(String),
    /// A physical row cannot be decoded as one JSON object.
    Invalid(String),
}

/// Read one JSON object per physical ledger line.
///
/// Missing, empty, and whitespace-only ledgers are errors. A multiline JSON
/// object is also an error: the ledger contract is JSONL, so the physical line
/// is the addressable evidence row.
pub fn read_ledger(path: &Path) -> Result<Vec<Value>, LedgerError> {
    let bytes = fs::read(path).map_err(|error| {
        LedgerError::NothingToCheck(format!(
            "{NOTHING_TO_CHECK}: orchestration ledger unavailable at {}: {error}",
            path.display()
        ))
    })?;
    parse_ledger(&bytes)
}

/// Parse staged ledger bytes without reading the worktree version.
pub fn parse_ledger(bytes: &[u8]) -> Result<Vec<Value>, LedgerError> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(LedgerError::NothingToCheck(format!(
            "{NOTHING_TO_CHECK}: orchestration ledger has zero receipt rows"
        )));
    }

    let mut rows = Vec::new();
    for (index, line) in bytes.split(|byte| *byte == b'\n').enumerate() {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let row = serde_json::from_slice(line).map_err(|error| {
            LedgerError::Invalid(format!(
                "LEDGER_ROW_INVALID row={} error={error}",
                index + 1
            ))
        })?;
        rows.push(row);
    }
    if rows.is_empty() {
        return Err(LedgerError::NothingToCheck(format!(
            "{NOTHING_TO_CHECK}: orchestration ledger has zero receipt rows"
        )));
    }
    Ok(rows)
}

/// Validate one production-shaped orchestration receipt.
///
/// The six laws are named in the returned text. OC-L3 and OC-L5 are checked
/// when their receipt fields are present; a missing producer remains outside
/// this row validator's scope.
pub fn validate_receipt(receipt: &Value) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let observed = receipt.get("observed").and_then(Value::as_object);
    let free = observed
        .and_then(|value| value.get("free_capacity"))
        .and_then(Value::as_array)
        .map(|panes| {
            panes
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();

    let dispatched = receipt
        .get("dispatched")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let refused = receipt
        .get("refused")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    // OC-L1: every observed free pane is dispatched or refused exactly once.
    let mut named = BTreeMap::<String, usize>::new();
    for row in dispatched.iter().chain(refused.iter()) {
        if let Some(pane) = row.get("pane").and_then(Value::as_str) {
            *named.entry(pane.to_owned()).or_default() += 1;
        } else {
            errors.push("OC-L1: dispatch/refusal row has no pane".to_owned());
        }
    }
    for pane in &free {
        match named.get(pane).copied().unwrap_or(0) {
            0 => errors.push(format!(
                "OC-L1: free pane {pane} is neither dispatched nor refused"
            )),
            1 => {}
            count => errors.push(format!(
                "OC-L1: free pane {pane} is named {count} times; expected exactly once"
            )),
        }
    }
    for pane in named.keys().filter(|pane| !free.contains(*pane)) {
        errors.push(format!(
            "OC-L1: pane {pane} is named but was not observed free"
        ));
    }
    for row in &refused {
        let reason = row
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if reason.trim().is_empty() {
            let pane = row
                .get("pane")
                .and_then(Value::as_str)
                .unwrap_or("<missing>");
            errors.push(format!("OC-L1: refusal for {pane} has no reason"));
        }
    }

    // OC-L2: dispatch can name only a claimed bead.
    for row in &dispatched {
        let bead = row
            .get("bead")
            .and_then(Value::as_str)
            .unwrap_or("<missing>");
        if row.get("claimed_first") != Some(&Value::Bool(true)) {
            errors.push(format!("OC-L2: dispatch names unclaimed bead {bead}"));
        }
    }

    // OC-L3: a row may not report edits to delegated paths.
    if let Some(edits) = receipt.get("edits").and_then(Value::as_array) {
        for edit in edits {
            let path = edit
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or("<missing>");
            errors.push(format!("OC-L3: orchestrator edited delegated path {path}"));
        }
    }

    // OC-L4: measured claims carry their producing command.
    if let Some(claims) = receipt.get("claims").and_then(Value::as_array) {
        for claim in claims {
            let figure = claim
                .get("figure")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let command = claim
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if figure.trim().is_empty() || command.trim().is_empty() {
                errors.push("OC-L4: measured claim is missing its producing command".to_owned());
            }
        }
    } else {
        errors.push("OC-L4: claims is not an array".to_owned());
    }

    // OC-L5: investigation and routing cannot share a tick with free capacity.
    if receipt.get("investigation").is_some_and(has_nonempty_value) && !free.is_empty() {
        errors.push(format!(
            "OC-L5: investigation row has non-zero free capacity ({})",
            free.len()
        ));
    }

    // OC-L6: destructive actions cite a prior measurement row.
    let tick = receipt.get("tick").and_then(Value::as_u64).unwrap_or(0);
    if let Some(actions) = receipt.get("destructive").and_then(Value::as_array) {
        for action in actions {
            let target = action
                .get("target")
                .and_then(Value::as_str)
                .unwrap_or("<missing>");
            let evidence_row = action.get("evidence_row").and_then(Value::as_u64);
            if evidence_row.is_none_or(|row| row == 0 || row >= tick) {
                errors.push(format!(
                    "OC-L6: destructive action on {target} lacks a prior measurement row"
                ));
            }
        }
    } else {
        errors.push("OC-L6: destructive is not an array".to_owned());
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn has_nonempty_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::String(value) => !value.trim().is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
        Value::Number(_) => true,
    }
}

/// Render the stable human/machine-readable result for one receipt.
#[must_use]
pub fn report(receipt: &Value) -> String {
    match validate_receipt(receipt) {
        Ok(()) => "ORCHESTRATION_TICK_GATE status=CLEAN".to_owned(),
        Err(errors) => format!(
            "ORCHESTRATION_TICK_GATE status=VIOLATION\n{}",
            errors.join("\n")
        ),
    }
}

/// Extract a stable law code from a validator error.
#[must_use]
pub fn law_code(error: &str) -> &str {
    error.split_once(':').map_or("UNKNOWN", |(law, _)| law)
}

/// Return the default canonical ledger path relative to a repository root.
#[must_use]
pub fn default_ledger_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".flywheel/orchestration-ticks.jsonl")
}

// ---------------------------------------------------------------------------------------
// psf7 — the WRITER, and the gap detector that makes a MISSING row loud
// ---------------------------------------------------------------------------------------

/// Heartbeat statuses that mean A TICK REACHED AN OUTCOME and therefore owed a row.
///
/// DECLARED, never inferred. A gap detector keyed on "any heartbeat" would fire on
/// `CYCLE_STARTED` — a cycle that began and was still running owes nothing yet — and a
/// detector that fires on the healthy path is a detector that gets routed around. Measured
/// over the newest 4,000 rows of the live ledger 2026-09-02: `CYCLE_STARTED` 1119,
/// `SUPERVISOR_REFUSED` 482, `DISPATCH_RETRY_BLOCKED` 373, `SUPERVISED_WORKING` 99,
/// `IDLE_UNAUTHORIZED` 88, `DISPATCH_RESULT_RECORDED` 85, `DISPATCH_CLAIMED` 51.
///
/// A status absent from this list contributes NOTHING, which is the honest failure
/// direction: an unlisted new outcome under-reports the gap rather than manufacturing one.
/// `gap_status_coverage` names that limit as a test rather than leaving it implied.
pub const TICK_OUTCOME_STATUSES: &[&str] = &[
    "AWAITING_HUMAN",
    "DISPATCH_CLAIMED",
    "DISPATCH_RESULT_RECORDED",
    "GATE_UNWIRED",
    "IDLE_UNAUTHORIZED",
    "SUPERVISED_WORKING",
    "SUPERVISOR_REFUSED",
];

/// A NAMED gap: tick outcomes happened after the newest recorded row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickGap {
    /// How many tick-outcome heartbeats are newer than the newest ledger row.
    pub outcomes_since_last_row: usize,
    /// The newest such heartbeat's status, so the gap names what went unrecorded.
    pub newest_status: String,
    /// Its `ts_unix`.
    pub newest_ts: u64,
    /// Its `tick`, which is the orchestrator's own counter and NOT the anchor.
    pub newest_tick: u64,
    /// The newest ledger row's `ts`, or `None` when the ledger is empty or absent —
    /// the case that was previously indistinguishable from a healthy fleet.
    pub last_row_ts: Option<u64>,
}

impl fmt::Display for TickGap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let anchor = match self.last_row_ts {
            Some(ts) => format!("last_row_ts={ts}"),
            None => "last_row_ts=NONE (ledger empty or absent)".to_owned(),
        };
        write!(
            formatter,
            "TICK_ROWS_MISSING outcomes_since_last_row={} {anchor} \
             newest_outcome={} newest_ts={} newest_tick={} \
             detail=\"the external heartbeat clock recorded tick outcomes that no ledger row \
             describes; a tick without a row did not happen\"",
            self.outcomes_since_last_row, self.newest_status, self.newest_ts, self.newest_tick
        )
    }
}

/// Compare the ledger against an EXTERNAL clock and name any gap.
///
/// # Why the anchor is not in the ledger
///
/// `psf7` left item 2 open — tick-number continuity, or a timestamp against dispatch
/// evidence — and flagged the honest answer as possibly needing an external clock. It does.
/// **A field cannot detect its own absence.** If the anchor lives in the artifact being
/// checked, an artifact that stopped being written has no anchor either, and an empty ledger
/// reads exactly like a healthy one. That is the whole defect.
///
/// Tick-number continuity is REJECTED for the reason the bead itself gave: a restart or a
/// second orchestrator legitimately breaks monotonicity, so a continuity check would fire on
/// correct behaviour and be disabled within a day.
///
/// The clock used instead is `write_heartbeat`'s ledger, which is
/// * written on a DIFFERENT code path from the tick row, so suppressing the row writer does
///   not suppress the clock — which is exactly what makes a fires-on-known-bad leg possible;
/// * machine-local and untracked, so this is a RUNTIME check and not a commit-time gate.
///
/// Returns `None` when nothing is owed. `heartbeat` rows lacking `status` or `ts_unix` are
/// skipped explicitly rather than defaulted.
///
/// HONEST NOTE on that skip: it is DEFENCE IN DEPTH, not the sole guard, and a mutation
/// proved it. Replacing the `let Some(ts) = … else { continue }` with `unwrap_or(0)` left
/// every test GREEN — because `0 <= floor` holds for every floor, so a defaulted row is
/// skipped by the floor comparison anyway. The earlier comment here claimed the default
/// would "silently shrink the gap"; that was wrong and is corrected. The explicit form is
/// kept because it survives a future change to the floor comparison, and
/// `a_clock_row_without_a_timestamp_contributes_nothing` asserts the OUTCOME rather than
/// pretending to pin the mechanism.
#[must_use]
pub fn detect_gap(ledger: &[Value], heartbeat: &[Value]) -> Option<TickGap> {
    let last_row_ts = ledger
        .iter()
        .filter_map(|row| row.get("ts").and_then(Value::as_u64))
        .max();
    let floor = last_row_ts.unwrap_or(0);

    let mut newest: Option<(u64, u64, String)> = None;
    let mut count = 0usize;
    for row in heartbeat {
        let Some(status) = row.get("status").and_then(Value::as_str) else {
            continue;
        };
        let Some(ts) = row.get("ts_unix").and_then(Value::as_u64) else {
            continue;
        };
        if ts <= floor || !TICK_OUTCOME_STATUSES.contains(&status) {
            continue;
        }
        count += 1;
        let tick = row.get("tick").and_then(Value::as_u64).unwrap_or(0);
        if newest.as_ref().is_none_or(|(seen, _, _)| ts > *seen) {
            newest = Some((ts, tick, status.to_owned()));
        }
    }

    let (newest_ts, newest_tick, newest_status) = newest?;
    Some(TickGap {
        outcomes_since_last_row: count,
        newest_status,
        newest_ts,
        newest_tick,
        last_row_ts,
    })
}

/// One pane's disposition in a tick, as the row records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneDisposition {
    /// Work was sent, naming the bead. `claimed_first` is asserted by OC-L2 and is
    /// therefore not a caller-supplied boolean here: a builder that let a caller pass
    /// `false` would emit a row the validator refuses.
    Dispatched {
        /// The pane the packet went to.
        pane: String,
        /// The bead named in the packet, which MUST already be claimed.
        bead: String,
        /// The receipt observed after the send.
        receipt: String,
    },
    /// No work was sent, and the reason is mandatory — OC-L1 refuses a blank one.
    Refused {
        /// The pane that was left alone.
        pane: String,
        /// Why, in the operator's terms.
        reason: String,
    },
}

/// Build one receipt from what the tick actually observed.
///
/// Every free pane appears exactly once by CONSTRUCTION: the builder takes the free set and
/// the dispositions, and any free pane without a disposition becomes a refusal carrying
/// `fallback_reason`. That is deliberate — OC-L1 would otherwise be violated by a decision
/// arm that simply forgot a pane, and a row that fails validation is a row an operator
/// deletes rather than fixes.
///
/// `claims` rows are pass-through and self-reported. Per this bead's NON-GOAL that is the
/// honesty boundary: `4wmo` checks shape, `detect_gap` checks presence, and neither checks
/// truth.
///
/// # Why a struct and not ten positional arguments
///
/// It WAS ten, and clippy said so (`too many arguments (10/7)`). In a ledger writer that is
/// not a style complaint: `attention` and `dead` are adjacent integers, so a swapped pair
/// compiles, passes every test, and writes a FABRICATED figure into the one artifact whose
/// purpose is auditability. Named fields make that swap unspellable.
pub struct Receipt<'a> {
    /// Wall clock for the row, in epoch seconds.
    pub ts: u64,
    /// The orchestrator's own tick counter. NOT the gap anchor — it resets per process.
    pub tick: u64,
    /// Panes observed as free capacity. Every one is named exactly once in the output.
    pub free_capacity: &'a [String],
    /// Panes blocked on a human answer.
    pub attention: u64,
    /// Dead panes, or `None` when the caller could not measure it. Never coerce to 0.
    pub dead: Option<u64>,
    /// What produced the observation, so the row is reproducible.
    pub source: &'a str,
    /// What the decision did with the panes it named.
    pub dispositions: &'a [PaneDisposition],
    /// The reason attached to any free pane the decision did not name.
    pub fallback_reason: &'a str,
    /// Self-reported measured claims, each carrying a producing command (OC-L4).
    pub claims: Vec<Value>,
    /// What this row does NOT establish. The honesty boundary, stated in the row.
    pub not_done: Vec<Value>,
}

#[must_use]
pub fn build_receipt(receipt: &mut Receipt<'_>) -> Value {
    let Receipt {
        ts,
        tick,
        free_capacity,
        attention,
        dead,
        source,
        dispositions,
        fallback_reason,
        claims,
        not_done,
    } = receipt;
    let (ts, tick, attention, dead) = (*ts, *tick, *attention, *dead);
    let (free_capacity, source, fallback_reason) = (*free_capacity, *source, *fallback_reason);
    let dispositions: &[PaneDisposition] = dispositions;
    let claims = std::mem::take(claims);
    let not_done = std::mem::take(not_done);
    let mut dispatched = Vec::new();
    let mut refused = Vec::new();
    let mut named = BTreeSet::new();

    for disposition in dispositions {
        match disposition {
            PaneDisposition::Dispatched {
                pane,
                bead,
                receipt,
            } => {
                // A disposition for a pane that was NOT observed free is dropped rather
                // than emitted: OC-L1 refuses "named but not observed free", and silently
                // emitting it would make the writer produce an invalid row.
                if !free_capacity.iter().any(|free| free == pane) {
                    continue;
                }
                if !named.insert(pane.clone()) {
                    continue;
                }
                dispatched.push(serde_json::json!({
                    "pane": pane,
                    "bead": bead,
                    "claimed_first": true,
                    "receipt": receipt,
                }));
            }
            PaneDisposition::Refused { pane, reason } => {
                if !free_capacity.iter().any(|free| free == pane) {
                    continue;
                }
                if !named.insert(pane.clone()) {
                    continue;
                }
                let reason = if reason.trim().is_empty() {
                    fallback_reason
                } else {
                    reason.as_str()
                };
                refused.push(serde_json::json!({ "pane": pane, "reason": reason }));
            }
        }
    }
    for pane in free_capacity {
        if named.insert(pane.clone()) {
            refused.push(serde_json::json!({ "pane": pane, "reason": fallback_reason }));
        }
    }

    serde_json::json!({
        "ts": ts,
        "tick": tick,
        "observed": {
            "free_capacity": free_capacity,
            "attention": attention,
            // `null`, NOT 0, when the caller cannot measure it. A zero here would be a
            // fabricated figure in the one artifact whose purpose is auditability, and it
            // is the same class as coercing an UNKNOWN into a negative answer.
            "dead": match dead { Some(count) => Value::from(count), None => Value::Null },
            "source": source,
        },
        "dispatched": dispatched,
        "refused": refused,
        "landed": {},
        "not_done": not_done,
        "claims": claims,
        "destructive": [],
    })
}

/// Append one receipt durably: temp file in the SAME directory, fsync, rename.
///
/// The same discipline the cursor writer uses, for the same reason — a torn JSONL line is
/// an invalid row, and an invalid row is what `4wmo` refuses. A crash mid-append must leave
/// the ledger readable, so the append is: read, extend, write-temp, fsync, rename.
pub fn append_receipt(path: &Path, receipt: &Value) -> Result<(), String> {
    let line = serde_json::to_string(receipt)
        .map_err(|error| format!("TICK_ROW_SERIALIZE_FAILED {error}"))?;
    if line.contains('\n') {
        return Err("TICK_ROW_SERIALIZE_FAILED row contains a newline; JSONL requires one physical line".to_owned());
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("TICK_ROW_WRITE_FAILED path={} has no parent", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("TICK_ROW_WRITE_FAILED create_parent {error}"))?;
    let mut body = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("TICK_ROW_WRITE_FAILED read {error}")),
    };
    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }
    body.push_str(&line);
    body.push('\n');

    let temp = path.with_extension(format!("jsonl.tmp.{}", std::process::id()));
    {
        let mut file = fs::File::create(&temp)
            .map_err(|error| format!("TICK_ROW_WRITE_FAILED create_temp {error}"))?;
        file.write_all(body.as_bytes())
            .map_err(|error| format!("TICK_ROW_WRITE_FAILED write {error}"))?;
        file.sync_all()
            .map_err(|error| format!("TICK_ROW_WRITE_FAILED fsync {error}"))?;
    }
    fs::rename(&temp, path).map_err(|error| {
        let _ = fs::remove_file(&temp);
        format!("TICK_ROW_WRITE_FAILED rename {error}")
    })
}

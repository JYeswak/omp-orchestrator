#![forbid(unsafe_code)]

//! Grader attribution must be a tracker field, not English inside `close_reason`.
//!
//! Detection is proven against a planted unattributed close. Production is a
//! shrinking ceiling: live count MAY fall (remediation, including a clean ledger)
//! and MUST NOT rise. Measured 2026-09-06 against `.beads/issues.jsonl`: 37
//! unattributed closed beads. The 23 in the bead body was 2026-09-02; a ceiling
//! set from that figure is already breached.
//!
//! Tracked caller: `.github/workflows/gate.yml` `cargo test -p grader-attribution-gate`.
//! Neighbouring hole, cited not solved: `gfm6`.

use std::fmt;

/// Git/`$USER` identity `br` writes when `--actor` is omitted.
///
/// Case-folded comparison is load-bearing (`Josh` vs `josh`, measured in
/// `bead-holder`).
pub const DEFAULT_AUTHORS: &[&str] = &["josh"];
/// The instant after which every bead must carry explicit br --actor provenance.
///
/// This is a source constant rather than a CLI default, so the detector has one
/// reviewable boundary. Rows before it are legacy data and are not retro-attributed.
pub const ACTOR_PROVENANCE_CUTOFF: &str = "2026-09-07T18:42:29Z";

/// Why the actor cutoff exists: legacy created_by values are not recoverable
/// without inventing provenance; only future writes can be required explicitly.
pub const ACTOR_PROVENANCE_CUTOFF_REASON: &str =
    "legacy created_by values are not retro-attributed; require explicit --actor going forward";

pub const ACTOR_PROVENANCE_EXIT_OK: u8 = 0;
pub const ACTOR_PROVENANCE_EXIT_VIOLATION: u8 = 1;
pub const ACTOR_PROVENANCE_EXIT_EMPTY: u8 = 2;
pub const ACTOR_PROVENANCE_EXIT_INVALID: u8 = 3;

/// One bead row reduced to the fields needed by the future-actor gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorProvenanceRow {
    pub id: String,
    pub created_at: String,
    pub created_by: Option<String>,
}

/// A post-cutoff row attributed to the shared git identity or no identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorProvenanceViolation {
    pub bead_id: String,
    pub field: &'static str,
    pub value: Option<String>,
}

impl fmt::Display for ActorProvenanceViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = self.value.as_deref().unwrap_or("<missing>");
        write!(
            f,
            "ACTOR_PROVENANCE_RED code=ACTOR_PROVENANCE_VIOLATION bead={} field={} value={} cutoff={}",
            self.bead_id, self.field, value, ACTOR_PROVENANCE_CUTOFF
        )
    }
}

/// Errors while reading actor-provenance input. A refused read is not green.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorProvenanceError {
    EmptyScan,
    MalformedLine { line: usize },
    MissingField { line: usize, field: &'static str },
    InvalidTimestamp { line: usize, value: String },
}

impl fmt::Display for ActorProvenanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyScan => f.write_str(
                "ACTOR_PROVENANCE_SCAN_EMPTY code=ACTOR_PROVENANCE_EMPTY -- bead ledger contained no rows",
            ),
            Self::MalformedLine { line } => write!(
                f,
                "ACTOR_PROVENANCE_LEDGER_INVALID code=ACTOR_PROVENANCE_MALFORMED line={line}"
            ),
            Self::MissingField { line, field } => write!(
                f,
                "ACTOR_PROVENANCE_LEDGER_INVALID code=ACTOR_PROVENANCE_MISSING_FIELD line={line} field={field}"
            ),
            Self::InvalidTimestamp { line, value } => write!(
                f,
                "ACTOR_PROVENANCE_LEDGER_INVALID code=ACTOR_PROVENANCE_INVALID_TIMESTAMP line={line} value={value}"
            ),
        }
    }
}

impl std::error::Error for ActorProvenanceError {}

impl ActorProvenanceError {
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::EmptyScan => ACTOR_PROVENANCE_EXIT_EMPTY,
            Self::MalformedLine { .. }
            | Self::MissingField { .. }
            | Self::InvalidTimestamp { .. } => ACTOR_PROVENANCE_EXIT_INVALID,
        }
    }
}

fn canonical_utc_timestamp(value: &str) -> Option<String> {
    let (date, time_with_zone) = value.split_once('T')?;
    let date_bytes = date.as_bytes();
    if date_bytes.len() != 10
        || date_bytes[4] != b'-'
        || date_bytes[7] != b'-'
        || !date_bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        return None;
    }
    let time = time_with_zone.strip_suffix('Z')?;
    let (clock, fraction) = time.split_once('.').unwrap_or((time, ""));
    let clock_bytes = clock.as_bytes();
    if clock_bytes.len() != 8
        || clock_bytes[2] != b':'
        || clock_bytes[5] != b':'
        || !clock_bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 2 | 5) || byte.is_ascii_digit())
        || fraction.len() > 9
        || !fraction.is_empty() && !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let mut normalized = String::with_capacity(30);
    normalized.push_str(date);
    normalized.push('T');
    normalized.push_str(clock);
    normalized.push('.');
    normalized.push_str(fraction);
    for _ in fraction.len()..9 {
        normalized.push('0');
    }
    normalized.push('Z');
    Some(normalized)
}

fn is_utc_timestamp(value: &str) -> bool {
    canonical_utc_timestamp(value).is_some()
}

fn timestamp_after_cutoff(value: &str) -> bool {
    let Some(current) = canonical_utc_timestamp(value) else {
        return false;
    };
    let Some(cutoff) = canonical_utc_timestamp(ACTOR_PROVENANCE_CUTOFF) else {
        return false;
    };
    current > cutoff
}

/// Parse every bead row needed by the cutoff gate, refusing malformed input.
pub fn parse_actor_provenance(jsonl: &str) -> Result<Vec<ActorProvenanceRow>, ActorProvenanceError> {
    let mut rows = Vec::new();
    for (line_index, line) in jsonl.lines().enumerate() {
        let line_number = line_index + 1;
        if line.trim().is_empty() {
            continue;
        }
        let value = serde_json::from_str::<serde_json::Value>(line)
            .map_err(|_| ActorProvenanceError::MalformedLine { line: line_number })?;
        let object = value
            .as_object()
            .ok_or(ActorProvenanceError::MalformedLine { line: line_number })?;
        let id = object
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .ok_or(ActorProvenanceError::MissingField {
                line: line_number,
                field: "id",
            })?
            .to_owned();
        let created_at = object
            .get("created_at")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|created_at| !created_at.is_empty())
            .ok_or(ActorProvenanceError::MissingField {
                line: line_number,
                field: "created_at",
            })?
            .to_owned();
        if !is_utc_timestamp(&created_at) {
            return Err(ActorProvenanceError::InvalidTimestamp {
                line: line_number,
                value: created_at,
            });
        }
        rows.push(ActorProvenanceRow {
            id,
            created_at,
            created_by: object
                .get("created_by")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|author| !author.is_empty())
                .map(str::to_owned),
        });
    }
    if rows.is_empty() {
        Err(ActorProvenanceError::EmptyScan)
    } else {
        Ok(rows)
    }
}

/// Return only post-cutoff rows lacking an explicit non-default actor.
pub fn actor_provenance_violations(
    rows: &[ActorProvenanceRow],
) -> Result<Vec<ActorProvenanceViolation>, ActorProvenanceError> {
    if rows.is_empty() {
        return Err(ActorProvenanceError::EmptyScan);
    }
    Ok(rows
        .iter()
        .filter(|row| timestamp_after_cutoff(&row.created_at))
        .filter(|row| {
            row.created_by
                .as_deref()
                .is_none_or(|author| is_default_author(author, DEFAULT_AUTHORS))
        })
        .map(|row| ActorProvenanceViolation {
            bead_id: row.id.clone(),
            field: "created_by",
            value: row.created_by.clone(),
        })
        .collect())
}

#[must_use]
pub fn actor_provenance_gate_exit(violations: &[ActorProvenanceViolation]) -> u8 {
    if violations.is_empty() {
        ACTOR_PROVENANCE_EXIT_OK
    } else {
        ACTOR_PROVENANCE_EXIT_VIOLATION
    }
}

/// Shrinking ratchet over unattributed closed beads in `.beads/issues.jsonl`.
///
/// Source command (2026-09-06):
/// `unattributed_close_ids(parse_closed_beads(.beads/issues.jsonl))` → 37.
/// May only be lowered. Raising it hides a regression.
pub const UNATTRIBUTED_CLOSE_CEILING: usize = 37;

/// One attempted close, as the tracker will record it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloseAttempt {
    pub bead_id: String,
    /// Who implemented. Not the current assignee (`gfm6`: that field is free text).
    pub implementer: Option<String>,
    /// `--actor` on `br close` / `br comments add`. `None` means the flag was omitted.
    pub actor: Option<String>,
    pub close_reason: String,
    pub comment_authors: Vec<String>,
    /// `Some` when `$TMUX_PANE` is set — a pane close without `--actor` is the defect.
    pub tmux_pane: Option<String>,
}

/// Why a close is not attributable.
///
/// Two variants, two messages. Collapsing them would make "forgot `--actor`"
/// indistinguishable from "graded your own work".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributionRefusal {
    ActorAbsent { pane: Option<String> },
    SelfGrade { actor: String, implementer: String },
    GraderProseMismatch { prose_name: String, actor: String },
}

impl fmt::Display for AttributionRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ActorAbsent { pane } => match pane {
                Some(pane) => write!(
                    f,
                    "ACTOR_REQUIRED pane={pane} -- br close without --actor from a tmux pane \
                     attributes the closer as josh; pass --actor <AgentMail name>"
                ),
                None => write!(
                    f,
                    "ACTOR_REQUIRED pane=none -- br close without --actor attributes the closer \
                     as josh; pass --actor <AgentMail name>"
                ),
            },
            Self::SelfGrade { actor, implementer } => write!(
                f,
                "SELF_GRADE_REFUSED actor={actor} implementer={implementer} -- grader must be a \
                 different identity than the implementer"
            ),
            Self::GraderProseMismatch { prose_name, actor } => write!(
                f,
                "GRADER_PROSE_MISMATCH prose={prose_name} actor={actor} -- close_reason names a \
                 grader the CLI actor is not"
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributionPass {
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributionVerdict {
    Pass(AttributionPass),
    Refused(AttributionRefusal),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmptyScan;

impl fmt::Display for EmptyScan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            "ATTRIBUTION_SCAN_EMPTY -- an empty scan set is an ERROR, never a pass; \
             a deliverable never checked reports identically to one that passed",
        )
    }
}

pub fn is_default_author(name: &str, defaults: &[&str]) -> bool {
    let folded = name.trim().to_lowercase();
    defaults
        .iter()
        .any(|default| default.eq_ignore_ascii_case(&folded))
}

/// Names a close_reason that says "by AmberGate" / "grader WildStone" without `--actor`.
pub fn prose_grader(close_reason: &str) -> Option<String> {
    for (needle, skip) in [("grader ", 7), ("by ", 3)] {
        if let Some(idx) = close_reason.to_ascii_lowercase().find(needle) {
            let rest = close_reason[idx + skip..].trim_start();
            let token = rest
                .split(|c: char| c.is_whitespace() || c == '(' || c == ',' || c == ';')
                .next()
                .unwrap_or("");
            if token
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_uppercase())
                && token.len() > 2
            {
                return Some(token.to_owned());
            }
        }
    }
    None
}

/// Non-default comment authors plus a prose grader, if any.
///
/// A zero from this matcher on a record known to carry attribution is a broken matcher.
pub fn attribution_hits(authors: &[String], close_reason: &str, defaults: &[&str]) -> Vec<String> {
    let mut hits: Vec<String> = authors
        .iter()
        .filter(|author| !is_default_author(author, defaults))
        .cloned()
        .collect();
    if let Some(prose) = prose_grader(close_reason) {
        if !hits.iter().any(|hit| hit.eq_ignore_ascii_case(&prose)) {
            hits.push(prose);
        }
    }
    hits
}

pub fn assess_close(input: &CloseAttempt, defaults: &[&str]) -> AttributionVerdict {
    let actor = input
        .actor
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty());
    let actor = match actor {
        None => {
            return AttributionVerdict::Refused(AttributionRefusal::ActorAbsent {
                pane: input.tmux_pane.clone(),
            });
        }
        Some(name) if is_default_author(name, defaults) => {
            return AttributionVerdict::Refused(AttributionRefusal::ActorAbsent {
                pane: input.tmux_pane.clone(),
            });
        }
        Some(name) => name,
    };
    if let Some(implementer) = input
        .implementer
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        if actor.eq_ignore_ascii_case(implementer) {
            return AttributionVerdict::Refused(AttributionRefusal::SelfGrade {
                actor: actor.to_owned(),
                implementer: implementer.to_owned(),
            });
        }
    }
    if let Some(prose) = prose_grader(&input.close_reason) {
        if !is_default_author(&prose, defaults) && !prose.eq_ignore_ascii_case(actor) {
            return AttributionVerdict::Refused(AttributionRefusal::GraderProseMismatch {
                prose_name: prose,
                actor: actor.to_owned(),
            });
        }
    }
    AttributionVerdict::Pass(AttributionPass {
        actor: actor.to_owned(),
    })
}

/// Anti-vacuity: scanning nothing is an error.
pub fn scan_closes(
    rows: &[CloseAttempt],
    defaults: &[&str],
) -> Result<Vec<AttributionVerdict>, EmptyScan> {
    if rows.is_empty() {
        return Err(EmptyScan);
    }
    Ok(rows
        .iter()
        .map(|row| assess_close(row, defaults))
        .collect())
}

/// One closed bead as recorded in `.beads/issues.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedBead {
    pub id: String,
    pub closed_at: String,
    pub close_reason: String,
    pub comment_authors: Vec<String>,
}

/// Parse closed rows from the production JSONL. Zero closed rows is EmptyScan.
pub fn parse_closed_beads(jsonl: &str) -> Result<Vec<ClosedBead>, EmptyScan> {
    let mut rows = Vec::new();
    for line in jsonl.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("status").and_then(|s| s.as_str()) != Some("closed") {
            continue;
        }
        let id = value
            .get("id")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_owned();
        if id.is_empty() {
            continue;
        }
        let authors = value
            .get("comments")
            .and_then(|c| c.as_array())
            .map(|comments| {
                comments
                    .iter()
                    .filter_map(|comment| {
                        comment
                            .get("author")
                            .and_then(|author| author.as_str())
                            .map(str::to_owned)
                    })
                    .collect()
            })
            .unwrap_or_default();
        rows.push(ClosedBead {
            id,
            closed_at: value
                .get("closed_at")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_owned(),
            close_reason: value
                .get("close_reason")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_owned(),
            comment_authors: authors,
        });
    }
    if rows.is_empty() {
        Err(EmptyScan)
    } else {
        Ok(rows)
    }
}

/// Closed beads whose comments are only the git default author (or have none).
pub fn unattributed_close_ids(rows: &[ClosedBead], defaults: &[&str]) -> Vec<String> {
    rows.iter()
        .filter(|row| {
            !row
                .comment_authors
                .iter()
                .any(|author| !is_default_author(author, defaults))
        })
        .map(|row| row.id.clone())
        .collect()
}

/// Production ratchet. Live MAY fall. Live MUST NOT exceed the ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CeilingVerdict {
    Exact { live: usize },
    Slack { live: usize, ceiling: usize },
    Breached { live: usize, ceiling: usize },
}

impl CeilingVerdict {
    pub fn from_counts(live: usize, ceiling: usize) -> Self {
        if live > ceiling {
            Self::Breached { live, ceiling }
        } else if live < ceiling {
            Self::Slack { live, ceiling }
        } else {
            Self::Exact { live }
        }
    }

    /// Slack is green (remediation). Breach is red (regression).
    pub fn refuses(self) -> bool {
        matches!(self, Self::Breached { .. })
    }
}

impl fmt::Display for CeilingVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exact { live } => write!(
                f,
                "ATTRIBUTION_CEILING live={live} ceiling={live} verdict=EXACT"
            ),
            Self::Slack { live, ceiling } => write!(
                f,
                "ATTRIBUTION_CEILING live={live} ceiling={ceiling} verdict=SLACK \
                 -- lower UNATTRIBUTED_CLOSE_CEILING to {live}; shrink-only"
            ),
            Self::Breached { live, ceiling } => write!(
                f,
                "CEILING_BREACHED live={live} ceiling={ceiling} -- a new unattributed \
                 close widened a known gap"
            ),
        }
    }
}

/// Exit 1 only on a ceiling breach. Slack and exact stay green so a clean
/// ledger does not fail CI. Empty scan is 2, from the parser, not here.
///
/// Returns `u8`, the width `ExitCode::from` actually takes. It returned `i32` and the caller
/// wrote `ExitCode::from(ledger_exit as u8)`, which is an exit-path narrowing cast over a
/// value whose range is `{0,1}` — a cast that can never be exercised and still has to be
/// justified to `exit_codes::every_narrowing_exit_cast_has_a_reasoned_allowance`. Removing
/// the cast is cheaper and stronger than a reasoned allowance row for it.
pub fn ledger_gate_exit(unattributed: &[String]) -> u8 {
    if CeilingVerdict::from_counts(unattributed.len(), UNATTRIBUTED_CLOSE_CEILING).refuses() {
        1
    } else {
        0
    }
}

/// Correction obligations (bead `omp-orchestrator-6le78`): the route for a
/// grader quoted into doctrine with no correction path.
///
/// A grader subagent has no write tool -- the anti-self-certification
/// property that makes its verdict admissible, and it MUST NOT be relaxed.
/// The consequence is structural: a grader can detect a misquote of its own
/// finding and cannot repair it. The route is a bead convention plus a
/// discoverable query, not a workflow engine and not write access:
///
/// - A correction is a bead titled `CORRECTION: <what is misquoted>`, whose
///   body names the misquoting location, the correct reading, and the
///   evidence, and whose ASSIGNEE is the discharging party (the file or
///   lane owner who can edit the misquote). An unassigned obligation is an
///   unrouted grade with extra steps.
/// - THE QUERY is `br list --title-contains 'CORRECTION:'`: the default
///   listing excludes closed beads, so every row it returns is outstanding
///   by construction -- the same shape that makes `br list --status
///   grading` the owed-grade query. No human or conductor needs to have
///   read IRC for the obligation to be found.
/// - The live `--title-contains` match is case-insensitive (measured
///   2026-09-12: `reclaim` and `RECLAIM` return the same 7 rows), so live
///   discovery is a SUPERSET of the canonical-prefix match below: it can
///   surface more, never hide an outstanding obligation.
///
/// The `473e62a` misquote that motivated this is routed to its file owner
/// separately and is NOT the subject here; this is the route, and its legs
/// below run it against fixture ledgers, not that instance.
pub const CORRECTION_TITLE_PREFIX: &str = "CORRECTION: ";

/// One bead row reduced to the fields the correction query reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrectionRow {
    pub id: String,
    pub title: String,
    pub status: String,
    pub assignee: Option<String>,
}

/// One outstanding correction obligation: a non-closed `CORRECTION:` bead.
/// The assignee, when present, is the discharging party.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrectionObligation {
    pub bead_id: String,
    pub title: String,
    pub assignee: Option<String>,
}

/// Why a correction query has no answer. An unreadable source is UNKNOWN,
/// never "no outstanding corrections": zero obligations and an unreadable
/// ledger must not share a verdict, and the compliant verdict is a positive
/// arm (a parsed ledger with no open correction rows), never the fallthrough.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrectionQueryError {
    EmptyScan,
    MalformedLine { line: usize },
    MissingField { line: usize, field: &'static str },
}

impl fmt::Display for CorrectionQueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyScan => f.write_str(
                "CORRECTION_SCAN_EMPTY code=CORRECTION_EMPTY -- ledger contained no rows",
            ),
            Self::MalformedLine { line } => write!(
                f,
                "CORRECTION_LEDGER_INVALID code=CORRECTION_MALFORMED line={line}"
            ),
            Self::MissingField { line, field } => write!(
                f,
                "CORRECTION_LEDGER_INVALID code=CORRECTION_MISSING_FIELD line={line} field={field}"
            ),
        }
    }
}

impl std::error::Error for CorrectionQueryError {}

/// Parse every bead row the correction query needs, refusing malformed input.
/// Mirrors `parse_actor_provenance`: an empty ledger is an error (a query
/// over nothing read is not a clean bill), and a missing field names its
/// line rather than defaulting.
pub fn parse_correction_rows(
    jsonl: &str,
) -> Result<Vec<CorrectionRow>, CorrectionQueryError> {
    let mut rows = Vec::new();
    for (line_index, line) in jsonl.lines().enumerate() {
        let line_number = line_index + 1;
        if line.trim().is_empty() {
            continue;
        }
        let value = serde_json::from_str::<serde_json::Value>(line)
            .map_err(|_| CorrectionQueryError::MalformedLine { line: line_number })?;
        let object = value
            .as_object()
            .ok_or(CorrectionQueryError::MalformedLine { line: line_number })?;
        let required = |field: &'static str| {
            object
                .get(field)
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
                .ok_or(CorrectionQueryError::MissingField {
                    line: line_number,
                    field,
                })
        };
        rows.push(CorrectionRow {
            id: required("id")?,
            title: required("title")?,
            status: required("status")?,
            assignee: object
                .get("assignee")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_owned),
        });
    }
    if rows.is_empty() {
        Err(CorrectionQueryError::EmptyScan)
    } else {
        Ok(rows)
    }
}

/// The discovery predicate, stated as code so legs can run it: outstanding
/// means titled `CORRECTION: ` AND not closed. A correction being worked
/// (assigned, in progress) is still outstanding until its close lands.
#[must_use]
pub fn outstanding_corrections(rows: &[CorrectionRow]) -> Vec<CorrectionObligation> {
    rows.iter()
        .filter(|row| row.title.starts_with(CORRECTION_TITLE_PREFIX))
        .filter(|row| row.status != "closed")
        .map(|row| CorrectionObligation {
            bead_id: row.id.clone(),
            title: row.title.clone(),
            assignee: row.assignee.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attempt(
        actor: Option<&str>,
        implementer: Option<&str>,
        reason: &str,
        pane: Option<&str>,
    ) -> CloseAttempt {
        CloseAttempt {
            bead_id: "omp-orchestrator-gcyf".into(),
            implementer: implementer.map(str::to_owned),
            actor: actor.map(str::to_owned),
            close_reason: reason.into(),
            comment_authors: actor.into_iter().map(str::to_owned).collect(),
            tmux_pane: pane.map(str::to_owned),
        }
    }

    #[test]
    fn actor_absent_from_a_pane_is_actor_required() {
        let verdict = assess_close(
            &attempt(None, Some("AmberGate"), "MUTATION-VERIFIED prose grader", Some("%9")),
            DEFAULT_AUTHORS,
        );
        let AttributionVerdict::Refused(refusal) = verdict else {
            panic!("absent actor must refuse, got {verdict:?}");
        };
        let text = refusal.to_string();
        assert!(
            text.starts_with("ACTOR_REQUIRED pane=%9"),
            "KNOWN-BAD absent --actor must name the pane: {text}"
        );
        assert!(!text.contains("SELF_GRADE_REFUSED"));
    }

    #[test]
    fn default_josh_actor_is_actor_required_not_self_grade() {
        let verdict = assess_close(
            &attempt(
                Some("josh"),
                Some("AmberGate"),
                "MUTATION-VERIFIED by SnowyCanyon",
                Some("%1397"),
            ),
            DEFAULT_AUTHORS,
        );
        let AttributionVerdict::Refused(refusal) = verdict else {
            panic!("josh actor is omitted --actor, got {verdict:?}");
        };
        assert!(
            refusal.to_string().starts_with("ACTOR_REQUIRED"),
            "{}",
            refusal
        );
    }

    #[test]
    fn actor_equal_implementer_is_self_grade() {
        let verdict = assess_close(
            &attempt(
                Some("BlueLantern"),
                Some("BlueLantern"),
                "MUTATION-VERIFIED self",
                Some("%1414"),
            ),
            DEFAULT_AUTHORS,
        );
        let AttributionVerdict::Refused(refusal) = verdict else {
            panic!("self-grade must refuse, got {verdict:?}");
        };
        let text = refusal.to_string();
        assert!(
            text.starts_with("SELF_GRADE_REFUSED actor=BlueLantern implementer=BlueLantern"),
            "KNOWN-BAD self-grade must name both identities: {text}"
        );
        assert!(!text.contains("ACTOR_REQUIRED"));
    }

    #[test]
    fn the_two_refusals_do_not_share_a_message() {
        let absent = AttributionRefusal::ActorAbsent {
            pane: Some("%9".into()),
        }
        .to_string();
        let self_grade = AttributionRefusal::SelfGrade {
            actor: "BlueLantern".into(),
            implementer: "BlueLantern".into(),
        }
        .to_string();
        assert_ne!(
            absent, self_grade,
            "two distinct failures, two distinct messages"
        );
        assert!(absent.contains("ACTOR_REQUIRED"));
        assert!(self_grade.contains("SELF_GRADE_REFUSED"));
        assert!(!absent.contains("SELF_GRADE_REFUSED"));
        assert!(!self_grade.contains("ACTOR_REQUIRED"));
    }

    #[test]
    fn iis6_close_passes() {
        let iis6 = CloseAttempt {
            bead_id: "omp-orchestrator-iis6".into(),
            implementer: Some("AmberGate".into()),
            actor: Some("WildStone".into()),
            close_reason: "MUTATION-VERIFIED grader WildStone %9 re-executed cat-file 4d8c784"
                .into(),
            comment_authors: vec!["WildStone".into()],
            tmux_pane: Some("%9".into()),
        };
        match assess_close(&iis6, DEFAULT_AUTHORS) {
            AttributionVerdict::Pass(pass) => assert_eq!(pass.actor, "WildStone"),
            other => panic!("KNOWN-GOOD iis6 must PASS, got {other:?}"),
        }
    }

    #[test]
    fn prose_grader_mismatch_is_refused() {
        let verdict = assess_close(
            &attempt(
                Some("WildStone"),
                Some("AmberGate"),
                "MUTATION-VERIFIED by SnowyCanyon (orchestrator)",
                Some("%9"),
            ),
            DEFAULT_AUTHORS,
        );
        let AttributionVerdict::Refused(AttributionRefusal::GraderProseMismatch { prose_name, actor }) =
            verdict
        else {
            panic!("prose/actor split must refuse, got {verdict:?}");
        };
        assert_eq!(prose_name, "SnowyCanyon");
        assert_eq!(actor, "WildStone");
    }

    #[test]
    fn empty_scan_is_an_error() {
        let error = scan_closes(&[], DEFAULT_AUTHORS).expect_err("empty scan must not pass");
        assert!(error.to_string().contains("ATTRIBUTION_SCAN_EMPTY"));
    }

    #[test]
    fn leht_and_mj8w_are_positive_controls() {
        let leht_authors = vec![
            "josh".into(),
            "AmberGate".into(),
            "GreenFrog".into(),
            "BlueLantern".into(),
        ];
        let leht_hits = attribution_hits(
            &leht_authors,
            "APPROVED: independent regrade verified derived membership",
            DEFAULT_AUTHORS,
        );
        assert!(
            leht_hits.iter().any(|h| h == "GreenFrog"),
            "POSITIVE CONTROL leht: matcher must fire on GreenFrog, got {leht_hits:?}"
        );

        let mj8w_authors = vec!["josh".into(), "BlueLantern".into(), "Pass7Grader".into()];
        let mj8w_hits = attribution_hits(
            &mj8w_authors,
            "APPROVED by AmberGate (pane 4, non-implementer grade of BlueLantern's 4893b36).",
            DEFAULT_AUTHORS,
        );
        assert!(
            mj8w_hits.iter().any(|h| h == "AmberGate"),
            "POSITIVE CONTROL mj8w: matcher must fire on AmberGate, got {mj8w_hits:?}"
        );
        assert!(
            mj8w_hits.iter().any(|h| h == "BlueLantern"),
            "POSITIVE CONTROL mj8w: matcher must fire on BlueLantern, got {mj8w_hits:?}"
        );
        assert!(
            !leht_hits.is_empty() && !mj8w_hits.is_empty(),
            "a zero from a pattern that cannot match is not evidence"
        );
    }

    #[test]
    fn mutation_collapsing_messages_is_detectable() {
        let absent = AttributionRefusal::ActorAbsent {
            pane: Some("%9".into()),
        }
        .to_string();
        let self_grade = AttributionRefusal::SelfGrade {
            actor: "BlueLantern".into(),
            implementer: "BlueLantern".into(),
        }
        .to_string();
        assert_ne!(absent, self_grade);
        assert!(absent.contains("ACTOR_REQUIRED"));
        assert!(self_grade.contains("SELF_GRADE_REFUSED"));
    }

    #[test]
    fn slack_and_exact_do_not_refuse_a_clean_or_improving_ledger() {
        assert!(!CeilingVerdict::from_counts(0, UNATTRIBUTED_CLOSE_CEILING).refuses());
        assert!(!CeilingVerdict::from_counts(36, 37).refuses());
        assert!(!CeilingVerdict::from_counts(37, 37).refuses());
        assert_eq!(ledger_gate_exit(&[]), 0);
    }

    #[test]
    fn a_count_above_the_ceiling_is_a_regression() {
        let verdict = CeilingVerdict::from_counts(38, 37);
        assert!(verdict.refuses());
        assert!(verdict.to_string().contains("CEILING_BREACHED"));
        let mut ids = Vec::new();
        ids.resize(UNATTRIBUTED_CLOSE_CEILING + 1, "x".into());
        assert_eq!(ledger_gate_exit(&ids), 1);
    }
}

#[cfg(test)]
mod correction_tests {
    use super::*;

    const FIXTURE: &str = r#"{"id":"omp-orchestrator-open-1","title":"CORRECTION: exit_codes misquotes GradePairAdm","status":"open","assignee":"BackstopFix"}
{"id":"omp-orchestrator-closed-1","title":"CORRECTION: stale label claim","status":"closed","assignee":"pane-1"}
{"id":"omp-orchestrator-plain-1","title":"Some ordinary bead","status":"open","assignee":"pane-2"}
"#;

    const DISCHARGED: &str = r#"{"id":"omp-orchestrator-open-1","title":"CORRECTION: exit_codes misquotes GradePairAdm","status":"closed","assignee":"BackstopFix"}
{"id":"omp-orchestrator-closed-1","title":"CORRECTION: stale label claim","status":"closed","assignee":"pane-1"}
{"id":"omp-orchestrator-plain-1","title":"Some ordinary bead","status":"open","assignee":"pane-2"}
"#;

    /// KNOWN-BAD (item 5, first half): an outstanding correction obligation
    /// is surfaced WITH its discharging party, and nothing else is.
    #[test]
    fn outstanding_correction_is_surfaced_with_its_discharger() {
        let rows = parse_correction_rows(FIXTURE).expect("fixture parses");
        let obligations = outstanding_corrections(&rows);
        assert_eq!(obligations.len(), 1, "exactly one row is outstanding: {obligations:?}");
        assert_eq!(obligations[0].bead_id, "omp-orchestrator-open-1");
        assert_eq!(
            obligations[0].assignee.as_deref(),
            Some("BackstopFix"),
            "the obligation must name its discharging party: {:?}",
            obligations[0]
        );
    }

    /// KNOWN-BAD companion for the title arm: an open bead WITHOUT the
    /// prefix is never an obligation, however live. Dropping the title
    /// check surfaces it (SUPERSET) and reddens exactly this leg.
    #[test]
    fn open_non_correction_is_not_an_obligation() {
        let rows = parse_correction_rows(FIXTURE).expect("fixture parses");
        let obligations = outstanding_corrections(&rows);
        assert!(
            obligations.iter().all(|obligation| obligation.bead_id != "omp-orchestrator-plain-1"),
            "a live non-correction bead must not surface: {obligations:?}"
        );
    }

    /// KNOWN-BAD companion for the status arm: a discharged correction is
    /// gone. Dropping the status check resurfaces it (SUPERSET) and reddens
    /// exactly this leg -- disjoint from the title arm's set.
    #[test]
    fn closed_correction_is_not_an_obligation() {
        let rows = parse_correction_rows(DISCHARGED).expect("fixture parses");
        let obligations = outstanding_corrections(&rows);
        assert!(
            obligations.is_empty(),
            "discharge must clear the query -- same query, opposite result: {obligations:?}"
        );
    }

    /// ANTI-VACUITY (item 6): an unreadable ledger is UNKNOWN, never clean.
    /// Malformed input and an empty scan refuse; a compliant empty set is a
    /// positive arm (parsed rows, zero obligations), never the fallthrough.
    #[test]
    fn unreadable_ledger_is_unknown_never_clean() {
        assert!(matches!(
            parse_correction_rows("not json\n"),
            Err(CorrectionQueryError::MalformedLine { line: 1 })
        ));
        assert!(matches!(
            parse_correction_rows(""),
            Err(CorrectionQueryError::EmptyScan)
        ));
        assert!(matches!(
            parse_correction_rows("{\"id\":\"x\"}\n"),
            Err(CorrectionQueryError::MissingField { .. })
        ));
    }

    #[test]
    fn compliant_empty_is_a_parsed_ledger_with_no_open_corrections() {
        let rows = parse_correction_rows(DISCHARGED).expect("discharge parses");
        assert_eq!(rows.len(), 3, "the compliant verdict must rest on rows read, not on absence");
        assert!(outstanding_corrections(&rows).is_empty());
    }
}

#[cfg(test)]
mod correction_status_tests {
    use super::*;

    const TWO_ROW: &str = r#"{"id":"omp-orchestrator-open-1","title":"CORRECTION: exit_codes misquotes GradePairAdm","status":"open","assignee":"BackstopFix"}
{"id":"omp-orchestrator-closed-1","title":"CORRECTION: stale label claim","status":"closed","assignee":"pane-1"}
"#;

    /// Status-arm discriminator (rule 7b): with no open plain bead in the
    /// fixture, dropping the TITLE arm changes nothing here (the closed row
    /// stays hidden by status alone), while dropping the STATUS arm
    /// resurfaces it. Paired with `open_non_correction_is_not_an_obligation`
    /// (reddens only when the title arm drops), the two mutations have
    /// DISJOINT reddened sets -- each arm proved load-bearing alone.
    #[test]
    fn status_arm_alone_hides_closed_corrections() {
        let rows = parse_correction_rows(TWO_ROW).expect("fixture parses");
        let obligations = outstanding_corrections(&rows);
        assert_eq!(obligations.len(), 1, "only the open row surfaces: {obligations:?}");
        assert_eq!(obligations[0].bead_id, "omp-orchestrator-open-1");
    }
}

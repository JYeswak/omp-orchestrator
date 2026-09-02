#![forbid(unsafe_code)]
//! THE WORKER SAYS DONE — `omp-orchestrator-omp-coverage-mission-ipg.19`.
//!
//! Josh: *"why isn't pane 4 letting you know when they are done."*
//!
//! # There was no worker → conductor completion path at all
//!
//! Every completion this session was discovered by the conductor happening to look,
//! or by a human saying so in chat. The follow-up stage detects SILENCE past a
//! deadline — *"this pane went quiet and owes a verdict"* — which is the
//! pathological case. **A healthy finish and a hang look identical to a silence
//! detector until the deadline expires**, so every clean completion costs a full
//! deadline of idle time before anything notices.
//!
//! # TWO DEFECTS THE BEAD DOES NOT NAME, BOTH MEASURED 2026-09-02
//!
//! **1. The existing push path is unreachable for every compliant worker.**
//! `FollowUpVerdict::Finished` is gated on `bead_closed`, and its doc calls a closed
//! row *"a finish the worker asserted"*. But `AGENTS.md:1268` states **"a bead is
//! closed by an agent who did NOT implement it"**. So a worker that follows the
//! grading rule can never produce the artifact that triggers `Finished`, and one
//! that does trigger it has broken the rule. The push exists and the population it
//! serves cannot reach it.
//!
//! **2. The ACK protocol silently disabled the silence detector.** `comments_present`
//! is checked BEFORE the deadline, and the ACK protocol adopted today posts a
//! comment on **every** dispatch. So any ACKed bead returns `VerdictPosted`
//! forever and `SilentPastDeadline` can never fire for it. A mechanism I helped
//! adopt turned off the watchdog for the 63 beads currently `in_progress`.
//!
//! Acceptance 5 — *"a worker that dies mid-task produces NO completion row, and the
//! silence detector fires"* — was **impossible** before this module, and not because
//! completion was missing.
//!
//! # Why the signal is a tracker row and not a message
//!
//! Acceptance 2: the only artifact that survives a pane dying, a context
//! compaction, and a daemon restart is a durable tracker row. The pane may be gone
//! by the time anyone reads it, so the signal is a **bead comment** with a strict,
//! byte-exact prefix — the same shape as the ACK protocol, whose parser is the
//! authority for its own format (`ack-stage/src/lib.rs:243-253`).

use std::fmt;

/// The prefix every completion row starts with. Byte-exact, like `ACK`.
const DONE_PREFIX: &str = "DONE ";

/// A worker's assertion that it finished, parsed from a durable tracker row.
///
/// Every field is something only the worker knows, which is why nothing can infer
/// this. `frees_pane` is the capacity claim: the conductor's whole reason for
/// wanting the signal early is that the pane is available before the grade lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionSignal {
    pub bead_token: String,
    pub pane_id: String,
    /// The worker's terminal verdict — its own words, unverified. See NO-CLAIM 1 on
    /// the bead: a completion signal makes a finish VISIBLE, not VERIFIED.
    pub verdict: String,
    /// Where the evidence lives: a commit, a ledger path, a test name.
    pub evidence: String,
    /// The pane the worker claims it is freeing. Normally its own.
    pub frees_pane: String,
}

impl fmt::Display for CompletionSignal {
    /// The canonical wire form. **This is the emitter** — a worker never
    /// hand-formats the row, because a hand-formatted row is a row that drifts from
    /// the parser, and the ACK protocol already paid for that lesson.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{} on {} -- verdict={} evidence={} frees={}",
            DONE_PREFIX, self.bead_token, self.pane_id, self.verdict, self.evidence, self.frees_pane
        )
    }
}

/// Why a candidate row is not a completion signal.
///
/// A typed refusal rather than `None`: *"this comment is not a completion"* and
/// *"this comment tried to be a completion and is malformed"* are different facts,
/// and a worker whose row is silently ignored believes it reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionParseError {
    /// Not a completion row at all — an ACK, a grade, prose.
    NotACompletion,
    /// Starts with the prefix and is missing a required field.
    Malformed { missing: &'static str },
    /// The token does not match the bead the caller asked about.
    WrongBead { found: String },
    /// A required field was present and empty. An empty verdict is not a verdict.
    EmptyField { field: &'static str },
}

/// Build the completion row a worker should post for a bead it just finished.
///
/// `bead_id` is the full id; the token is its LAST hyphen-segment, matching the ACK
/// protocol exactly so a worker learns one rule rather than two.
pub fn completion_row(
    bead_id: &str,
    pane_id: &str,
    verdict: &str,
    evidence: &str,
    frees_pane: &str,
) -> CompletionSignal {
    CompletionSignal {
        bead_token: bead_token(bead_id),
        pane_id: pane_id.to_owned(),
        verdict: verdict.to_owned(),
        evidence: evidence.to_owned(),
        frees_pane: frees_pane.to_owned(),
    }
}

/// The last hyphen-segment of a bead id — the ACK protocol's token rule, restated
/// here so the two formats cannot drift apart.
pub fn bead_token(bead_id: &str) -> String {
    bead_id.rsplit('-').next().unwrap_or(bead_id).to_owned()
}

/// Parse one tracker comment as a completion signal for `bead_id`.
pub fn parse_completion(comment: &str, bead_id: &str) -> Result<CompletionSignal, CompletionParseError> {
    let rest = comment
        .strip_prefix(DONE_PREFIX)
        .ok_or(CompletionParseError::NotACompletion)?;
    let (token, rest) = rest
        .split_once(" on ")
        .ok_or(CompletionParseError::Malformed { missing: "pane" })?;
    let expected = bead_token(bead_id);
    if token != expected {
        return Err(CompletionParseError::WrongBead {
            found: token.to_owned(),
        });
    }
    let (pane, fields) = rest
        .split_once(" -- ")
        .ok_or(CompletionParseError::Malformed { missing: "fields" })?;
    if pane.trim().is_empty() {
        return Err(CompletionParseError::EmptyField { field: "pane" });
    }
    let verdict = field(fields, "verdict=").ok_or(CompletionParseError::Malformed {
        missing: "verdict",
    })?;
    let evidence = field(fields, "evidence=").ok_or(CompletionParseError::Malformed {
        missing: "evidence",
    })?;
    let frees = field(fields, "frees=").ok_or(CompletionParseError::Malformed { missing: "frees" })?;
    for (name, value) in [("verdict", &verdict), ("evidence", &evidence), ("frees", &frees)] {
        if value.trim().is_empty() {
            return Err(CompletionParseError::EmptyField { field: leak(name) });
        }
    }
    Ok(CompletionSignal {
        bead_token: token.to_owned(),
        pane_id: pane.to_owned(),
        verdict,
        evidence,
        frees_pane: frees,
    })
}

/// `key=value` up to the next ` key=` boundary, so a verdict may contain spaces.
fn field(fields: &str, key: &str) -> Option<String> {
    let start = fields.find(key)? + key.len();
    let rest = &fields[start..];
    let end = ["verdict=", "evidence=", "frees="]
        .iter()
        .filter_map(|other| rest.find(&format!(" {other}")))
        .min()
        .unwrap_or(rest.len());
    Some(rest[..end].trim().to_owned())
}

fn leak(name: &str) -> &'static str {
    match name {
        "verdict" => "verdict",
        "evidence" => "evidence",
        "frees" => "frees",
        _ => "field",
    }
}

/// Is this comment the ACK for `bead_id`, rather than a substantive verdict?
///
/// **This is the function `classify_followup` needed and did not have.** With
/// `comments_present: bool` an ACK is indistinguishable from a verdict, and since
/// the ACK protocol posts one on every dispatch, the silence detector was dead for
/// every ACKed bead.
pub fn is_ack_row(comment: &str, bead_id: &str) -> bool {
    let prefix = format!("ACK {} on ", bead_token(bead_id));
    comment.starts_with(&prefix)
}

/// What the conductor should conclude about one dispatched bead.
///
/// # Acceptance 3: completion and silence are DISTINCT and cannot collapse
///
/// `Finished` and `SilentPastDeadline` are answered differently — refill versus
/// investigate — and a single "not working" state is the `free_capacity` defect one
/// layer up. The arms are mutually exclusive **by construction**: `Finished`
/// requires a parsed completion row, and `SilentPastDeadline` requires there to be
/// none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionOutcome {
    /// A worker posted a typed completion row. **No close required** — this is the
    /// arm that makes the push reachable for a compliant worker.
    Finished(CompletionSignal),
    /// A completion row exists for a DIFFERENT bead, or is malformed. Loud, because
    /// a worker that posted a broken row believes it reported.
    MalformedCompletion(CompletionParseError),
    /// Acknowledged and working: the ACK is present, no completion yet, and the
    /// deadline has not passed. **Distinct from `Finished` and from silence.**
    AckedWorking { minutes_elapsed: u64 },
    /// A substantive comment that is not an ACK and not a completion.
    VerdictPosted,
    /// No completion, no substantive comment, past the deadline. The silence
    /// detector's case, and the ONLY arm that means investigate.
    SilentPastDeadline { minutes_elapsed: u64 },
    /// Not yet acknowledged and not yet due. Nothing to do.
    Dispatched { minutes_elapsed: u64 },
}

impl CompletionOutcome {
    /// Does the conductor get capacity back?
    pub fn frees_capacity(&self) -> bool {
        matches!(self, CompletionOutcome::Finished(_))
    }

    /// Does a human owe this bead attention?
    pub fn owes_investigation(&self) -> bool {
        matches!(
            self,
            CompletionOutcome::SilentPastDeadline { .. } | CompletionOutcome::MalformedCompletion(_)
        )
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Finished(_) => "FINISHED",
            Self::MalformedCompletion(_) => "COMPLETION_MALFORMED",
            Self::AckedWorking { .. } => "ACKED_WORKING",
            Self::VerdictPosted => "VERDICT_POSTED",
            Self::SilentPastDeadline { .. } => "SILENT_PAST_DEADLINE",
            Self::Dispatched { .. } => "DISPATCHED",
        }
    }
}

/// Classify one dispatched bead from its durable comment rows.
///
/// Ordering is deliberate and is the fix for defect 2: a **completion** is looked
/// for first, an **ACK** is recognised as not-a-verdict, and only then does the
/// deadline decide. The old order let any comment pre-empt the deadline.
pub fn classify_completion(
    bead_id: &str,
    comments: &[String],
    minutes_elapsed: u64,
    deadline_minutes: u64,
) -> CompletionOutcome {
    let mut malformed: Option<CompletionParseError> = None;
    for comment in comments {
        match parse_completion(comment, bead_id) {
            Ok(signal) => return CompletionOutcome::Finished(signal),
            Err(CompletionParseError::NotACompletion) => {}
            Err(other) => malformed = Some(other),
        }
    }
    if let Some(error) = malformed {
        return CompletionOutcome::MalformedCompletion(error);
    }
    let acked = comments.iter().any(|c| is_ack_row(c, bead_id));
    let substantive = comments
        .iter()
        .any(|c| !is_ack_row(c, bead_id) && !c.trim().is_empty());
    if substantive {
        return CompletionOutcome::VerdictPosted;
    }
    if minutes_elapsed >= deadline_minutes {
        // An ACK does NOT suppress this any more. That is the whole of defect 2.
        return CompletionOutcome::SilentPastDeadline { minutes_elapsed };
    }
    if acked {
        CompletionOutcome::AckedWorking { minutes_elapsed }
    } else {
        CompletionOutcome::Dispatched { minutes_elapsed }
    }
}

/// ANTI-VACUITY, acceptance 6: zero completions observed in a window where a bead
/// moved to closed is an ERROR.
///
/// **A completion mechanism that never fires is indistinguishable from a fleet that
/// never finishes anything** — and that is the exact shape of the 178-tick capacity
/// failure, where every watchdog reported healthy over nothing.
pub fn assert_completions_not_vacuous(
    completions_observed: usize,
    beads_closed_in_window: usize,
) -> Result<(), String> {
    if beads_closed_in_window > 0 && completions_observed == 0 {
        return Err(format!(
            "COMPLETION_SIGNAL_VACUOUS: {beads_closed_in_window} bead(s) closed in this window \
             and ZERO completion rows were observed. Either no worker is emitting, or the parser \
             stopped matching -- and a mechanism that never fires reads exactly like a fleet that \
             finishes nothing."
        ));
    }
    Ok(())
}

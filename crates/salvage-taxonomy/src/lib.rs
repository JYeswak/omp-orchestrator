//! What to DO when a dispatched turn dies.
//!
//! # Why this crate exists
//!
//! `dispatch-silence-watch`, `dispatcher-deadman`, `receiver-receipt` and `ack-stage` all detect
//! that something is WRONG. **None of them says what to do next**, so the orchestrator
//! hand-decides every time — which is why one pane hand-routed an entire session.
//!
//! The taxonomy is distilled from `loktar00/agent-skills/omp-orchestration` (~90 real turns, 10
//! models, 5 providers). **Two of its five rows are counter-intuitive, which is exactly why a
//! typed classifier beats judgement:**
//!
//! - a **deadline** means the work MOSTLY LANDED and must be SALVAGED, not relaunched;
//! - an **`EXIT 0` with no work** means RELAUNCH UNCHANGED rather than investigate.
//!
//! Judgement gets those two backwards under time pressure. A type does not.
//!
//! # This crate CLASSIFIES and RECOMMENDS. It does not actuate.
//!
//! Same split as `ntm-fleet-monitor` — *"classifies; does not send"* — because a recommendation you
//! can read is auditable and a relaunch you cannot see is not.
//!
//! # `EXIT 1` IS NOT A DISCRIMINATOR
//!
//! A provider error, a deadline, and a workspace-load outage all produce a nonzero exit. This is
//! the same trap as `cargo`'s generic `101`, which cost this repository a false mutation verdict
//! within the last two hours: a run returned `exit=101` with no test output because a peer's
//! `Cargo.toml` had a stray character, and only the ABSENCE of a `test result:` line distinguished
//! it from a real RED. So [`classify`] REQUIRES accompanying text and refuses to guess from an
//! exit code alone.
//!
//! # NO-CLAIM
//!
//! This makes the decision EXPLICIT and REVIEWABLE. It does not make it correct, does not observe
//! any turn by itself — every field of [`TurnEvidence`] is supplied by a caller that did the
//! observing — and does not relaunch, salvage or write anything. An [`TurnOutcome::Unknown`] is a
//! refusal to recommend, not a diagnosis.

#![forbid(unsafe_code)]

use std::fmt;

use asupersync::Cx;

/// The five observed end-states of a dispatched turn, plus the honest sixth.
///
/// Ordering is not significance; the variants follow the source table's rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnOutcome {
    /// `EXIT 0` with work present. The only outcome that means "done".
    Finished,
    /// `EXIT 1` plus deadline text. **The work usually MOSTLY landed.**
    DeadlineExceeded,
    /// `EXIT 1` plus provider text: upstream died mid-generation.
    ProviderError,
    /// No exit observed and the session log has gone stale past the threshold.
    SilentProcessDeath,
    /// `EXIT 0` with zero work: the provider returned an empty completion.
    EmptyCompletion,
    /// Not classifiable from the evidence supplied. **Never a relaunch.**
    Unknown,
}

/// What the orchestrator should DO. One variant per row, plus [`Self::Hold`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SalvageDecision {
    /// Check the claimed work actually exists.
    Verify,
    /// Inspect files, run the checks, and write the missing checkpoint YOURSELF —
    /// **labelled orchestrator-written**. Relaunch only if essentials are missing.
    Salvage,
    /// Send the identical packet again. Correct for an empty completion and for nothing else.
    RelaunchAsIs,
    /// Relaunch with a preamble naming the repo state and the REMAINING scope, so the
    /// worker does not rewrite what landed.
    RelaunchWithContinuePreamble,
    /// The process is gone. Salvage or continue, but stop waiting on it.
    TreatAsDead,
    /// **The refusal.** An unclassified turn is precisely the case where relaunching
    /// DUPLICATES landed work, so `Unknown` must never map to any relaunch.
    Hold,
}

impl SalvageDecision {
    /// Does this decision cause a worker to run the packet again?
    ///
    /// Exists so the `Unknown`-never-relaunches invariant is a property a test can assert over
    /// EVERY variant, rather than a claim about the three arms someone remembered to check.
    #[must_use]
    pub fn is_relaunch(self) -> bool {
        matches!(
            self,
            Self::RelaunchAsIs | Self::RelaunchWithContinuePreamble
        )
    }
}

/// Everything a caller observed about one dispatched turn.
///
/// # The liveness oracle IS available here, contrary to the dispatch premise
///
/// The dispatch that commissioned this crate said the upstream liveness oracle is unavailable
/// because `~/.omp/agent/sessions` is stale. **That is the wrong subtree.** Measured 2026-09-07:
/// 0 files touched under `~/.omp/agent/sessions` in 30 minutes, and **26 session files touched
/// under `~/.omp/profiles/<profile>/agent/sessions/` in 10 minutes.** The live path carries a
/// `<profile>` segment.
///
/// And it is PANE-ATTRIBUTABLE, by correspondence rather than by assumption:
///
/// ```text
/// claude  3 live session files  <->  3 live claude panes
/// codex   2 live session files  <->  2 live codex panes
/// grok    0 live session files  <->  1 grok pane, DEAD (provider credits exhausted)
/// ```
///
/// Five live files against five live panes, and the dead lineage at zero. That is why
/// [`Self::session_stale_secs`] is a first-class input instead of an unavailable one.
///
/// **Its residual, stated rather than hidden:** the correspondence was observed once, at one
/// moment, on one host. Two panes sharing a profile could share a session file, in which case the
/// oracle degrades from pane-level to profile-level. A caller that cannot establish which file
/// belongs to which pane MUST pass `None` rather than a profile-wide figure.
#[derive(Debug, Clone, Default)]
pub struct TurnEvidence {
    /// Process exit status. `None` means **never observed**, which is a different fact from `0`.
    pub exit: Option<i32>,
    /// Captured tail — pane footer, stderr, or log. The discriminator when `exit` is nonzero.
    pub text: String,
    /// Age of this pane's session log. `None` when the caller could not attribute one.
    pub session_stale_secs: Option<u64>,
    /// Did the dispatched bead receive a comment after the packet was sent?
    pub bead_comment_since_dispatch: bool,
    /// How many of the dispatched bead's paths have commits since the packet.
    pub landed_paths: usize,
    /// Was a matching `ACK <token> on <pane> --` comment observed?
    pub ack_present: bool,
}

impl TurnEvidence {
    /// Did the caller observe ANYTHING? Used by the anti-vacuity arm.
    ///
    /// "I could not look" and "there is nothing there" are opposite conditions, and this
    /// repository blames conflating them for twelve confident zeros in one session.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.exit.is_none()
            && self.text.trim().is_empty()
            && self.session_stale_secs.is_none()
            && !self.bead_comment_since_dispatch
            && self.landed_paths == 0
            && !self.ack_present
    }
}

/// Threshold from the source table: a session log older than this with no exit is a dead process.
pub const SILENT_DEATH_STALE_SECS: u64 = 20 * 60;

/// A classification, its recommended move, and the reason a human can audit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classification {
    pub outcome: TurnOutcome,
    pub decision: SalvageDecision,
    /// Human-readable, and it NAMES THE EVIDENCE that drove the arm — not just the arm.
    pub reason: String,
}

impl Classification {
    /// Distinct code per outcome.
    ///
    /// Both the code and the message are asserted by every known-bad leg. A message-only
    /// assertion is defeasible: on `omp-orchestrator-2sx1` a mutation collapsed
    /// `EXIT_PUBLISH -> EXIT_MISSING_FIELD` while `FINDING_PUBLISH_FAILED` stayed printed, so the
    /// token was still correct and the code was wrong. `AGENTS.md` rule 7 read as "message INSTEAD
    /// OF code" is defeated by that mutation; the defensible form is BOTH.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self.outcome {
            TurnOutcome::Finished => 0,
            TurnOutcome::DeadlineExceeded => 10,
            TurnOutcome::ProviderError => 11,
            TurnOutcome::SilentProcessDeath => 12,
            TurnOutcome::EmptyCompletion => 13,
            // Distinct from every classified outcome, so "could not classify" can never be
            // mistaken for "finished" by a caller reading the code alone.
            TurnOutcome::Unknown => 20,
        }
    }
}

/// Refusals, kept separate from outcomes so a broken instrument cannot masquerade as a verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaxonomyError {
    /// A nonzero exit with no accompanying text. Refused rather than guessed: a deadline, a
    /// provider error and a workspace-load outage are indistinguishable by code alone.
    NonzeroExitWithoutText { exit: i32 },
    /// Cancellation observed at a checkpoint.
    Cancelled,
}

impl fmt::Display for TaxonomyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonzeroExitWithoutText { exit } => write!(
                f,
                "SALVAGE_EXIT_WITHOUT_TEXT exit={exit} — a nonzero exit is NOT a discriminator: \
                 a deadline, a provider error and a workspace-load outage all produce one. \
                 Supply the accompanying text."
            ),
            Self::Cancelled => write!(f, "SALVAGE_CANCELLED"),
        }
    }
}

impl std::error::Error for TaxonomyError {}

impl TaxonomyError {
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::NonzeroExitWithoutText { .. } => 21,
            Self::Cancelled => 22,
        }
    }
}

/// Needles for the deadline row. Matched case-insensitively against the captured text.
pub const DEADLINE_NEEDLES: &[&str] = &["deadline exceeded", "deadline_exceeded", "timed out"];
/// Needles for the provider row.
pub const PROVIDER_NEEDLES: &[&str] = &[
    "provider error",
    "upstream error",
    "credit",
    "rate limit",
    "429",
    "502",
    "503",
];

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    let lowered = haystack.to_lowercase();
    needles.iter().any(|needle| lowered.contains(needle))
}

/// Classify ONE turn.
///
/// `&Cx` first per the asupersync contract, with the checkpoint before the decision so a cancelled
/// classification yields [`TaxonomyError::Cancelled`] rather than a recommendation nobody asked for.
///
/// # Errors
///
/// [`TaxonomyError::NonzeroExitWithoutText`] when an exit code arrives without its text, and
/// [`TaxonomyError::Cancelled`] on cancellation.
pub fn classify(cx: &Cx, evidence: &TurnEvidence) -> Result<Classification, TaxonomyError> {
    if cx.checkpoint().is_err() {
        return Err(TaxonomyError::Cancelled);
    }

    // ANTI-VACUITY FIRST, and deliberately ahead of every other arm: an unobserved turn and a
    // finished turn must not report identically. `Unknown` is the residual, never the default —
    // a classifier whose compliant verdict is the fallthrough branch re-acquires that defect
    // every time someone adds a case.
    if evidence.is_empty() {
        return Ok(Classification {
            outcome: TurnOutcome::Unknown,
            decision: SalvageDecision::Hold,
            reason: "SALVAGE_UNKNOWN no evidence supplied — HOLD. Relaunching an unclassified \
                     turn is exactly how landed work gets duplicated."
                .to_owned(),
        });
    }

    let has_work = evidence.landed_paths > 0;

    match evidence.exit {
        Some(0) if has_work => Ok(Classification {
            outcome: TurnOutcome::Finished,
            decision: SalvageDecision::Verify,
            reason: format!(
                "SALVAGE_FINISHED exit=0 landed_paths={} — VERIFY the claimed work; an exit code \
                 is a claim, not an artifact.",
                evidence.landed_paths
            ),
        }),
        // COUNTER-INTUITIVE ROW: exit 0 and nothing landed is an EMPTY COMPLETION, and the move
        // is to send the identical packet again rather than investigate a worker that did nothing.
        Some(0) => Ok(Classification {
            outcome: TurnOutcome::EmptyCompletion,
            decision: SalvageDecision::RelaunchAsIs,
            reason: "SALVAGE_EMPTY_COMPLETION exit=0 landed_paths=0 — the provider returned \
                     nothing. RELAUNCH AS-IS; there is no partial state to preserve."
                .to_owned(),
        }),
        Some(exit) => {
            if evidence.text.trim().is_empty() {
                return Err(TaxonomyError::NonzeroExitWithoutText { exit });
            }
            if contains_any(&evidence.text, DEADLINE_NEEDLES) {
                // COUNTER-INTUITIVE ROW: a deadline means the work MOSTLY LANDED.
                return Ok(Classification {
                    outcome: TurnOutcome::DeadlineExceeded,
                    decision: SalvageDecision::Salvage,
                    reason: format!(
                        "SALVAGE_DEADLINE exit={exit} landed_paths={} — the work usually MOSTLY \
                         landed. SALVAGE: inspect the files, run the checks, and write the missing \
                         checkpoint yourself LABELLED orchestrator-written. Relaunch only if \
                         essentials are absent.",
                        evidence.landed_paths
                    ),
                });
            }
            if contains_any(&evidence.text, PROVIDER_NEEDLES) {
                return Ok(Classification {
                    outcome: TurnOutcome::ProviderError,
                    decision: SalvageDecision::RelaunchWithContinuePreamble,
                    reason: format!(
                        "SALVAGE_PROVIDER_ERROR exit={exit} landed_paths={} — upstream died \
                         mid-generation. RELAUNCH WITH A CONTINUE PREAMBLE naming the repo state \
                         and the REMAINING scope, so the worker does not rewrite what landed.",
                        evidence.landed_paths
                    ),
                });
            }
            // Nonzero, text present, and it matches no row. Refusing to recommend is the point:
            // this is where a guess would relaunch over landed work.
            Ok(Classification {
                outcome: TurnOutcome::Unknown,
                decision: SalvageDecision::Hold,
                reason: format!(
                    "SALVAGE_UNKNOWN exit={exit} text present but matches no known row — HOLD. \
                     A nonzero exit alone is not a discriminator."
                ),
            })
        }
        None => {
            // No exit was ever observed. The session log is the only remaining witness.
            match evidence.session_stale_secs {
                Some(stale) if stale >= SILENT_DEATH_STALE_SECS => Ok(Classification {
                    outcome: TurnOutcome::SilentProcessDeath,
                    decision: SalvageDecision::TreatAsDead,
                    reason: format!(
                        "SALVAGE_SILENT_DEATH no exit observed, session stale {stale}s >= \
                         {SILENT_DEATH_STALE_SECS}s, landed_paths={} — TREAT AS DEAD and salvage \
                         or continue. Landed code was substantially complete every time this was \
                         seen upstream.",
                        evidence.landed_paths
                    ),
                }),
                Some(stale) => Ok(Classification {
                    outcome: TurnOutcome::Unknown,
                    decision: SalvageDecision::Hold,
                    reason: format!(
                        "SALVAGE_UNKNOWN no exit observed and the session is only {stale}s stale \
                         (< {SILENT_DEATH_STALE_SECS}s) — HOLD. The turn may still be running, \
                         and a relaunch would duplicate it."
                    ),
                }),
                None => Ok(Classification {
                    outcome: TurnOutcome::Unknown,
                    decision: SalvageDecision::Hold,
                    reason: "SALVAGE_UNKNOWN no exit observed and no attributable session log — \
                             HOLD. A profile-wide staleness figure MUST NOT be substituted for a \
                             pane-attributable one."
                        .to_owned(),
                }),
            }
        }
    }
}

/// Classify a batch, checkpointing per row.
///
/// # Errors
///
/// Propagates [`classify`]'s errors, and [`TaxonomyError::Cancelled`] if cancellation lands
/// between rows — so a cancelled sweep does not return a partial set that reads as complete.
pub fn classify_all(
    cx: &Cx,
    rows: &[TurnEvidence],
) -> Result<Vec<Classification>, TaxonomyError> {
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        // Per-iteration checkpoint: the loop is the cancellation surface, and
        // `omp-orchestrator-fj08` records what an uncheckpointed loop over unbounded input costs.
        if cx.checkpoint().is_err() {
            return Err(TaxonomyError::Cancelled);
        }
        out.push(classify(cx, row)?);
    }
    Ok(out)
}

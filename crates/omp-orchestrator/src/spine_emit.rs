#![forbid(unsafe_code)]
//! THE CLOSE HALF OF THE LOOP — `omp-orchestrator-eg0m`.
//!
//! # The fact that decided this
//!
//! MEASURED across all 6,305 heartbeat rows: five of `ack-spine`'s eleven
//! `StepKind` variants appear in **zero** of them, and **four of the five are the
//! close half** — `Closed`, `GradeReceived`, `Redispatched`, `FenceChecked`,
//! `PacketRendered`. The heartbeat records dispatch richly and records completion
//! not at all, so the ledger can answer *"what did we dispatch"* in detail and
//! **cannot answer "did anything finish"** at all.
//!
//! Every kind that IS carried is carried as a **substring of a free-text `detail`**.
//! There is no `ACK_STAGE_*` value in `status` anywhere; `ACK_STAGE_INDETERMINATE`,
//! `ACK_STAGE_RETRY_BLOCKED` and `ack_readback_missing` all live inside prose. So
//! the evidence the ACK protocol just made authoritative is matchable by substring
//! and by nothing else, while `StepKind` is an enum a consumer can match
//! exhaustively with compiler checking.
//!
//! # What was REFUTED, recorded so nobody rebuilds it
//!
//! The proposed reason for wiring was an asupersync per-step
//! cancellation/checkpoint ledger. That is refuted:
//! `StepRecord = { kind, bead_id, pane_id, session, ts_unix, detail }` carries
//! **zero** cancel/checkpoint fields, and field for field the heartbeat is
//! **richer** — it has `build_id`, `pid`, `tick` and `repo`, none of which
//! `StepRecord` has. `Cx` runs the steps and is never recorded about them.
//! **The wiring is about step COVERAGE and TYPING, never about cancellation.**
//!
//! # What this module is, and what it deliberately is not
//!
//! It is the **classifier**: pure functions that turn facts the supervisor already
//! reads — a bead snapshot, a prior-dispatch record — into the typed completion
//! kinds nothing has ever emitted. Emission itself goes through
//! `ack_spine::ledger::step`, which is `&Cx`-first and checkpoints on both sides of
//! the effect, so this module never spawns, never awaits, and never writes.
//!
//! It is NOT a second ledger. The heartbeat keeps its rows; this adds the typed
//! projection that a grader can match exhaustively.

use ack_spine::ledger::StepKind;
use std::path::{Path, PathBuf};

/// The close reasons this repo's policy admits. A prose reason is REFUSED by `br`,
/// the refusal scrolls past in-pane, and the agent believes the close landed — so
/// the presence of one of these prefixes is what distinguishes a graded close from
/// an attempted one.
pub const GRADED_CLOSE_PREFIXES: &[&str] = &["MUTATION-VERIFIED", "DONE", "APPROVED", "WONTFIX"];

/// What the supervisor knows about a bead it dispatched earlier.
///
/// Deliberately the smallest shape that answers the completion question, and every
/// field is something `br show --json` already returns — this module invents no
/// state and reads no file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriorDispatch {
    pub bead_id: String,
    pub pane_id: String,
    /// `open` / `in_progress` / `blocked` / `closed`, verbatim from `br`.
    pub status: String,
    /// The close reason, when the bead is closed.
    pub close_reason: Option<String>,
    /// Whether this bead has already been dispatched before the packet under
    /// consideration. Distinguishes `PacketSent` from `Redispatched`.
    pub prior_dispatch_count: usize,
    /// Whether a completion row for this bead is ALREADY in the spine ledger.
    /// Without this the reconcile pass re-emits `Closed` every tick forever and the
    /// ledger stops meaning "this happened once".
    pub already_recorded: bool,
}

/// The typed completion kinds owed for one previously-dispatched bead.
///
/// # Why a Vec and not an Option
///
/// A graded close owes **two** facts: the bead closed, and a grade was received.
/// They are different observations — a close with a prose reason is a close that
/// was never graded — and collapsing them into one value is the same coercion that
/// made `busy` and `dead` the same verdict in `iis6`.
pub fn completion_kinds(prior: &PriorDispatch) -> Vec<StepKind> {
    if prior.already_recorded {
        return Vec::new();
    }
    if prior.status != "closed" {
        return Vec::new();
    }
    let reason = prior.close_reason.as_deref().unwrap_or("").trim_start();
    let graded = GRADED_CLOSE_PREFIXES
        .iter()
        .any(|prefix| reason.starts_with(prefix));
    if graded {
        // Order matters for a reader replaying the ledger: the grade is the cause,
        // the close is the effect.
        vec![StepKind::GradeReceived, StepKind::Closed]
    } else {
        // `mcq2` acceptance 6. This was a bare `Closed`, which made an ungraded
        // close distinguishable only by the ABSENCE of a sibling `GradeReceived`
        // row — and absence cannot separate "closed without a grade" from "the
        // grade row was lost". MEASURED: 18 of 102 closes carry a first token
        // outside the four documented prefixes, so this is 18% of the population,
        // not an edge case.
        vec![StepKind::ClosedWithoutGrade]
    }
}

/// Which kind a packet about to be sent should carry.
///
/// `Redispatched` existed in the enum and in zero rows, because nothing ever asked
/// the question. A bead that has been dispatched before and is being dispatched
/// again is a **distinct fact** from a first dispatch: it is the signal that a prior
/// attempt produced no completion, which is precisely what the missing close half
/// made invisible.
pub fn send_kind(prior_dispatch_count: usize) -> StepKind {
    if prior_dispatch_count > 0 {
        StepKind::Redispatched
    } else {
        StepKind::PacketSent
    }
}

/// How many prior sends this bead already has, from heartbeat `DISPATCHED` rows
/// plus spine `packet_sent`/`redispatched` rows.
///
/// Heartbeat-only counting made `Redispatched` unreachable for beads the
/// supervisor had already sent on the spine path: measured, `eg0m` has spine
/// `packet_sent` and zero heartbeat `DISPATCHED` lines, so every later cycle
/// still asked `send_kind(0)`.
pub fn prior_send_count(heartbeat_jsonl: &str, spine_jsonl: &str, bead: &str) -> usize {
    let needle = format!("bead={bead} ");
    let needle_q = format!("bead={bead}\"");
    let from_heartbeat = heartbeat_jsonl
        .lines()
        .filter(|line| line.contains("\"DISPATCHED\""))
        .filter(|line| line.contains(&needle) || line.contains(&needle_q))
        .count();
    let from_spine = spine_jsonl.lines().filter(|line| {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line.trim()) else {
            return false;
        };
        let kind = value.get("kind").and_then(|k| k.as_str()).unwrap_or("");
        if kind != StepKind::PacketSent.as_str() && kind != StepKind::Redispatched.as_str() {
            return false;
        }
        value.get("bead").and_then(|b| b.as_str()) == Some(bead)
    }).count();
    from_heartbeat + from_spine
}


/// Where the supervisor's spine ledger lives, derived from the heartbeat path.
///
/// Derived rather than configured for the reason `8nuh` records: a fact about the
/// deployment carried in a separate environment variable is a fact that can go
/// missing from the plist and stall the fleet with no file evidence. One path, one
/// source.
pub fn spine_ledger_path(heartbeat_ledger: &Path) -> PathBuf {
    heartbeat_ledger.with_file_name("omp-orchestrator.spine.jsonl")
}

/// Which bead ids already carry a `Closed` row in a spine ledger's JSONL.
///
/// Reads the persisted ledger as text and looks for the typed kind together with
/// the bead id. **A parse failure yields an EMPTY set, never a silent "everything
/// is recorded"**: the conservative direction here is to re-emit, because a missing
/// completion row is the defect this bead exists to fix and a duplicate is merely
/// noise. The `already_recorded` flag then suppresses the duplicate on the next
/// tick once the row is genuinely on disk.
pub fn recorded_closures(jsonl: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in jsonl.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let kind = value.get("kind").and_then(|k| k.as_str()).unwrap_or("");
        if kind != StepKind::Closed.as_str() {
            continue;
        }
        if let Some(bead) = value.get("bead").and_then(|b| b.as_str()) {
            if !bead.is_empty() {
                out.push(bead.to_owned());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// ANTI-VACUITY. A dispatch cycle that emitted no steps is an ERROR, not a clean
/// tick.
///
/// `StepLedger::assert_non_empty` already refuses an empty ledger; this states the
/// same requirement at the supervisor's boundary, where the count is a decision
/// rather than an invariant. **An empty ledger after a dispatch reads identically to
/// a dispatch that never happened**, which is the failure shape this whole file is
/// about.
pub fn assert_cycle_emitted(steps: usize, dispatched: bool) -> Result<(), String> {
    if dispatched && steps == 0 {
        return Err(
            "SPINE_LEDGER_EMPTY: a dispatch cycle emitted zero StepRecords; an empty ledger \
             is indistinguishable from a cycle that never ran"
                .to_owned(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prior(status: &str, reason: Option<&str>) -> PriorDispatch {
        PriorDispatch {
            bead_id: "omp-orchestrator-eg0m".to_owned(),
            pane_id: "%1408".to_owned(),
            status: status.to_owned(),
            close_reason: reason.map(ToOwned::to_owned),
            prior_dispatch_count: 0,
            already_recorded: false,
        }
    }

    /// THE THREE KINDS THE RULING TURNED ON. `Closed` and `GradeReceived` appear in
    /// zero of 6,305 heartbeat rows; this is the function that produces them.
    #[test]
    fn a_graded_close_owes_both_the_grade_and_the_close() {
        let kinds = completion_kinds(&prior("closed", Some("MUTATION-VERIFIED all six legs")));
        assert_eq!(kinds, vec![StepKind::GradeReceived, StepKind::Closed]);
    }

    /// A close with a PROSE reason is a close that was never graded. Collapsing the
    /// two would be the `iis6` coercion again: two distinct conditions, one value.
    #[test]
    fn an_ungraded_close_is_its_own_kind_not_a_bare_close() {
        let kinds = completion_kinds(&prior("closed", Some("looks good to me")));
        // `mcq2` acceptance 6: TYPED and DISTINCT. A bare `Closed` here would make
        // an ungraded close readable only as the ABSENCE of a `GradeReceived`
        // sibling, and absence cannot separate "never graded" from "row lost".
        assert_eq!(kinds, vec![StepKind::ClosedWithoutGrade]);
        assert!(!kinds.contains(&StepKind::GradeReceived));
        assert!(
            !kinds.contains(&StepKind::Closed),
            "an ungraded close must NOT also claim the graded-close kind: a reader \
             counting `closed` rows would then count it as graded"
        );
        // The two kinds must be distinguishable ON THE WIRE, or the typing is
        // decoration and a JSONL reader is back to inference.
        assert_ne!(
            StepKind::Closed.as_str(),
            StepKind::ClosedWithoutGrade.as_str()
        );
    }

    /// Every approved prefix must work, or the classifier silently downgrades a
    /// real grade to a bare close for three of the four verbs.
    #[test]
    fn every_policy_prefix_counts_as_a_grade() {
        for prefix in GRADED_CLOSE_PREFIXES {
            let reason = format!("{prefix}: evidence on the bead");
            let kinds = completion_kinds(&prior("closed", Some(&reason)));
            assert!(
                kinds.contains(&StepKind::GradeReceived),
                "prefix {prefix} did not register as a grade"
            );
        }
        // And a near-miss must NOT: a reason that merely CONTAINS the word is not a
        // reason that starts with it. This is the substring-matching defect the
        // heartbeat already has, and the reason for moving to typed kinds.
        let kinds = completion_kinds(&prior("closed", Some("this is DONE in spirit")));
        assert_eq!(kinds, vec![StepKind::ClosedWithoutGrade]);
    }

    /// An open bead owes nothing. Emitting `Closed` for live work would make the
    /// ledger's completion answer worse than absent.
    #[test]
    fn an_unfinished_bead_owes_no_completion_row() {
        for status in ["open", "in_progress", "blocked"] {
            assert!(
                completion_kinds(&prior(status, None)).is_empty(),
                "{status}"
            );
        }
    }

    /// IDEMPOTENCE. Without this the reconcile pass re-emits `Closed` every 90
    /// seconds forever, and a ledger that says a bead closed 400 times cannot be
    /// read as "this happened once".
    #[test]
    fn an_already_recorded_closure_is_not_re_emitted() {
        let mut p = prior("closed", Some("APPROVED"));
        p.already_recorded = true;
        assert!(completion_kinds(&p).is_empty());
    }

    /// `Redispatched` vs `PacketSent`: the fact that a prior attempt produced no
    /// completion is exactly what the missing close half made invisible.
    #[test]
    fn a_second_packet_for_one_bead_is_a_redispatch() {
        assert_eq!(send_kind(0), StepKind::PacketSent);
        assert_eq!(send_kind(1), StepKind::Redispatched);
        assert_eq!(send_kind(7), StepKind::Redispatched);
    }

    /// `Redispatched` is selected by `send_kind`; the supervisor emit site in
    /// `main.rs` is the production caller. A `ledger::step(` call here would keep
    /// the census GREEN after deleting that emit site (eg0m acceptance 4).
    #[test]
    fn a_redispatch_is_a_distinct_kind_on_the_wire() {
        assert_eq!(StepKind::Redispatched.as_str(), "redispatched");
        assert_ne!(StepKind::Redispatched.as_str(), StepKind::PacketSent.as_str());
    }

    /// Spine `packet_sent` is a prior send even when heartbeat has no DISPATCHED
    /// row — the measured eg0m hole.
    #[test]
    fn a_spine_packet_sent_makes_the_next_kind_redispatched() {
        let spine = r#"{"kind":"packet_sent","bead":"omp-orchestrator-eg0m","pane":"%8","session":"s","ts":1,"detail":"prior_dispatches=0"}"#;
        let n = prior_send_count("", spine, "omp-orchestrator-eg0m");
        assert_eq!(n, 1);
        assert_eq!(send_kind(n), StepKind::Redispatched);
        assert_eq!(prior_send_count("", spine, "omp-orchestrator-eg0m.1"), 0);
        let hb = r#"{"status":"DISPATCHED","detail":"bead=omp-orchestrator-eg0m pane=%8"}"#;
        assert_eq!(prior_send_count(hb, "", "omp-orchestrator-eg0m"), 1);
        assert_eq!(prior_send_count(hb, spine, "omp-orchestrator-eg0m"), 2);
    }


    /// The persisted-ledger reader, both directions.
    #[test]
    fn recorded_closures_reads_typed_rows_and_ignores_everything_else() {
        let jsonl = format!(
            "{}\n{}\n{}\nnot json at all\n\n",
            r#"{"kind":"closed","bead":"a","pane":"%1","session":"s","ts":1,"detail":""}"#,
            r#"{"kind":"packet_sent","bead":"b","pane":"%1","session":"s","ts":1,"detail":""}"#,
            r#"{"kind":"closed","bead":"a","pane":"%1","session":"s","ts":2,"detail":""}"#,
        );
        // The literal must match the enum, or this test proves nothing about the
        // reader — it would pass on a typo in both places.
        assert_eq!(StepKind::Closed.as_str(), "closed");
        let found = recorded_closures(&jsonl);
        assert_eq!(found, vec!["a".to_owned()], "deduped, closed-only");
        // A malformed ledger must yield EMPTY, never "everything is recorded":
        // re-emitting a duplicate is noise, suppressing a real closure is the bug.
        assert!(recorded_closures("{{{ broken").is_empty());
        assert!(recorded_closures("").is_empty());
    }

    /// ANTI-VACUITY, both directions. A dispatch with zero steps is an error; a
    /// tick that dispatched nothing is not.
    #[test]
    fn an_empty_ledger_after_a_dispatch_is_an_error_but_an_idle_tick_is_not() {
        assert!(assert_cycle_emitted(0, true).is_err());
        assert!(assert_cycle_emitted(0, false).is_ok());
        assert!(assert_cycle_emitted(4, true).is_ok());
    }

    /// The ledger path is DERIVED from the heartbeat path, per `8nuh`: a second
    /// environment variable is a second thing that can go missing from a plist.
    #[test]
    fn the_ledger_path_is_derived_from_the_heartbeat_path() {
        let p = spine_ledger_path(Path::new("/tmp/x/omp-orchestrator.heartbeat.jsonl"));
        assert_eq!(p, PathBuf::from("/tmp/x/omp-orchestrator.spine.jsonl"));
        assert_eq!(p.parent(), Path::new("/tmp/x/a").parent());
    }
}

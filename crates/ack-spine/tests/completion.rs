#![forbid(unsafe_code)]
//! THE WORKER SAYS DONE — acceptance legs for `omp-orchestrator-omp-coverage-mission-ipg.19`.
//!
//! Hermetic: no `br`, no tmux, no subprocess. Every leg is a pure classification
//! over comment rows, which is the point — the signal is a **durable tracker row**,
//! so the mechanism must be checkable without a live pane.

use ack_spine::completion::{
    assert_completions_not_vacuous, bead_token, classify_completion, completion_row,
    is_ack_row, parse_completion, CompletionOutcome, CompletionParseError,
};
use ack_spine::followup::{classify_followup, followup_action, FollowUpAction, FollowUpVerdict};

const BEAD: &str = "omp-orchestrator-omp-coverage-mission-ipg.19";
const PANE: &str = "%1408";

fn done_row() -> String {
    completion_row(
        BEAD,
        PANE,
        "acceptance 1-6 met, mutation-verified",
        "commit a3ab48d + tests/completion.rs",
        PANE,
    )
    .to_string()
}

fn ack_row() -> String {
    format!("ACK {} on {PANE} -- starting", bead_token(BEAD))
}

/// ACCEPTANCE 1: the signal is EMITTED by the worker and carries all four facts.
/// A watcher cannot infer any of them.
#[test]
fn the_completion_row_carries_bead_verdict_evidence_and_freed_capacity() {
    let row = done_row();
    let signal = parse_completion(&row, BEAD).expect("the emitter's own row must parse");
    assert_eq!(signal.bead_token, "ipg.19");
    assert_eq!(signal.pane_id, PANE);
    assert_eq!(signal.verdict, "acceptance 1-6 met, mutation-verified");
    assert_eq!(signal.evidence, "commit a3ab48d + tests/completion.rs");
    assert_eq!(signal.frees_pane, PANE);
    // ROUND TRIP: the emitter and the parser must not drift. The ACK protocol paid
    // for this lesson — a hand-formatted row is a row that drifts from its reader.
    assert_eq!(signal.to_string(), row);
}

/// The token rule is the ACK protocol's rule, not a second one. A worker learns one
/// thing. `ipg.19` -> `19`, matching `ack-stage`'s `rsplit('-')`.
#[test]
fn the_token_rule_matches_the_ack_protocol_exactly() {
    // The LAST hyphen-segment, which for this id is `ipg.19` and not `19` — the
    // dotted child keeps its parent stem. I asserted "19" first and the leg
    // caught me; the implementation matches the ACK I actually posted.
    assert_eq!(bead_token(BEAD), "ipg.19");
    assert_eq!(bead_token("omp-orchestrator-eg0m"), "eg0m");
    assert_eq!(bead_token("omp-orchestrator-kxe.5"), "kxe.5");
    // No hyphen at all: the whole id is the token rather than an empty string.
    assert_eq!(bead_token("solo"), "solo");
}

/// ACCEPTANCE 4, FIRES-ON-KNOWN-GOOD: a normal completion produces the row and the
/// conductor gets capacity back **without polling**. Nothing here observes a pane.
#[test]
fn a_normal_finish_frees_capacity_with_no_polling() {
    let outcome = classify_completion(BEAD, &[ack_row(), done_row()], 5, 90);
    match &outcome {
        CompletionOutcome::Finished(signal) => assert_eq!(signal.frees_pane, PANE),
        other => panic!("a posted completion must be Finished, got {other:?}"),
    }
    assert!(outcome.frees_capacity(), "a finish must return capacity");
    assert!(
        !outcome.owes_investigation(),
        "a finish must NOT send anyone to investigate — that is the whole distinction"
    );
    // And it wins even PAST the deadline: a worker that finished slowly is finished,
    // not silent. Ordering completion ahead of the deadline is what makes this true.
    assert!(
        classify_completion(BEAD, &[ack_row(), done_row()], 10_000, 90).frees_capacity(),
        "a slow finish is still a finish"
    );
}

/// ACCEPTANCE 5, FIRES-ON-KNOWN-BAD: a worker that dies mid-task produces NO
/// completion row, and the SILENCE path fires instead. **The two must not overlap.**
#[test]
fn a_dead_worker_produces_no_completion_and_the_silence_path_fires() {
    let outcome = classify_completion(BEAD, &[ack_row()], 120, 90);
    assert_eq!(
        outcome,
        CompletionOutcome::SilentPastDeadline {
            minutes_elapsed: 120
        }
    );
    assert!(!outcome.frees_capacity(), "silence must NOT free capacity");
    assert!(outcome.owes_investigation());
}

/// ACCEPTANCE 3 AND 5 TOGETHER, AS AN EXHAUSTIVE PROPERTY: no input produces both a
/// finish and a silence verdict. They are mutually exclusive **by construction** —
/// `Finished` requires a parsed row and `SilentPastDeadline` requires there to be
/// none — and this walks the whole input space that matters to prove it.
#[test]
fn no_input_is_both_finished_and_silent() {
    let rows: Vec<Vec<String>> = vec![
        vec![],
        vec![ack_row()],
        vec![done_row()],
        vec![ack_row(), done_row()],
        vec!["VERDICT: re-ran it, PASS".to_owned()],
        vec![ack_row(), "VERDICT: re-ran it, PASS".to_owned()],
        vec!["DONE ipg.19 on %1408 -- verdict= evidence=x frees=%1408".to_owned()],
    ];
    let mut finished = 0;
    let mut silent = 0;
    for comments in &rows {
        for minutes in [0u64, 89, 90, 100_000] {
            let outcome = classify_completion(BEAD, comments, minutes, 90);
            let is_finished = matches!(outcome, CompletionOutcome::Finished(_));
            let is_silent = matches!(outcome, CompletionOutcome::SilentPastDeadline { .. });
            assert!(
                !(is_finished && is_silent),
                "overlap at comments={comments:?} minutes={minutes}"
            );
            // And exactly one of the two capacity questions may be true.
            assert!(
                !(outcome.frees_capacity() && outcome.owes_investigation()),
                "a verdict cannot both free capacity and demand investigation: {outcome:?}"
            );
            finished += usize::from(is_finished);
            silent += usize::from(is_silent);
        }
    }
    // ANTI-VACUITY: the non-overlap above is trivially true if neither ever fires.
    assert!(finished > 0, "no input produced Finished: the leg is vacuous");
    assert!(silent > 0, "no input produced SilentPastDeadline: the leg is vacuous");
}

/// DEFECT 2, THE ONE THE BEAD DOES NOT NAME. An ACK is not a verdict, so it must NOT
/// suppress the deadline.
///
/// MEASURED: `classify_followup` checked `comments_present` before the deadline, and
/// the ACK protocol posts a comment on every dispatch — so `SilentPastDeadline` could
/// never fire for any ACKed bead. This asserts the fix in both directions.
#[test]
fn an_ack_does_not_suppress_the_silence_deadline() {
    assert!(is_ack_row(&ack_row(), BEAD));
    assert!(!is_ack_row(&done_row(), BEAD), "a completion is not an ACK");
    assert!(!is_ack_row("VERDICT: re-ran it", BEAD));

    // ACK only, past the deadline -> SILENT. This was VerdictPosted forever.
    assert!(matches!(
        classify_completion(BEAD, &[ack_row()], 120, 90),
        CompletionOutcome::SilentPastDeadline { .. }
    ));
    // ACK only, before the deadline -> acknowledged and working, its own arm.
    assert!(matches!(
        classify_completion(BEAD, &[ack_row()], 30, 90),
        CompletionOutcome::AckedWorking { .. }
    ));
    // No ACK at all, before the deadline -> dispatched, a DIFFERENT fact from acked.
    assert!(matches!(
        classify_completion(BEAD, &[], 30, 90),
        CompletionOutcome::Dispatched { .. }
    ));
    // A substantive comment DOES suppress the deadline, which is correct.
    assert_eq!(
        classify_completion(BEAD, &[ack_row(), "VERDICT: PASS".to_owned()], 120, 90),
        CompletionOutcome::VerdictPosted
    );
}

/// DEFECT 1: the existing push was unreachable for a compliant worker. A completion
/// row now yields `Finished` from `classify_followup` **with the bead still open**.
#[test]
fn a_completion_row_reaches_finished_without_closing_the_bead() {
    let verdict = classify_followup(
        BEAD,
        /* bead_closed = */ false,
        /* close_reason = */ None,
        "AmberGate",
        "AmberGate",
        &[ack_row(), done_row()],
        /* minutes = */ 5,
        /* deadline = */ 90,
        /* tracker_readable = */ true,
    );
    match &verdict {
        FollowUpVerdict::Finished { bead_id, .. } => assert_eq!(bead_id, BEAD),
        other => panic!("an open bead with a completion row must be Finished, got {other:?}"),
    }
    assert_eq!(followup_action(&verdict), FollowUpAction::Healthy);

    // KNOWN-BAD for the same call: strip the completion row and the SAME inputs past
    // the deadline must escalate. Without this the leg above could pass on a
    // classifier that returns Finished for everything.
    let silent = classify_followup(
        BEAD,
        false,
        None,
        "AmberGate",
        "AmberGate",
        &[ack_row()],
        120,
        90,
        true,
    );
    assert!(matches!(
        silent,
        FollowUpVerdict::SilentPastDeadline { .. }
    ));
    assert!(matches!(
        followup_action(&silent),
        FollowUpAction::NeedsFollowUp(_)
    ));
}

/// A malformed completion is LOUD. A worker that posted a broken row believes it
/// reported, and silence there is the worst of both mechanisms.
#[test]
fn a_malformed_completion_is_named_rather_than_ignored() {
    // Empty verdict.
    assert_eq!(
        parse_completion("DONE ipg.19 on %1408 -- verdict= evidence=x frees=%1408", BEAD),
        Err(CompletionParseError::EmptyField { field: "verdict" })
    );
    // Missing the fields section entirely.
    assert_eq!(
        parse_completion("DONE ipg.19 on %1408", BEAD),
        Err(CompletionParseError::Malformed { missing: "fields" })
    );
    // Right shape, wrong bead — a row pasted from another dispatch.
    assert_eq!(
        parse_completion(
            "DONE eg0m on %1408 -- verdict=v evidence=e frees=%1408",
            BEAD
        ),
        Err(CompletionParseError::WrongBead {
            found: "eg0m".to_owned()
        })
    );
    // Not a completion at all: a plain refusal, not an error.
    assert_eq!(
        parse_completion(&ack_row(), BEAD),
        Err(CompletionParseError::NotACompletion)
    );
    // And the classifier escalates a malformed row rather than treating the bead as
    // working normally.
    let outcome = classify_completion(
        BEAD,
        &["DONE ipg.19 on %1408 -- verdict= evidence=x frees=%1408".to_owned()],
        5,
        90,
    );
    assert!(matches!(outcome, CompletionOutcome::MalformedCompletion(_)));
    assert!(outcome.owes_investigation());
    assert!(!outcome.frees_capacity());
}

/// A verdict containing spaces must survive the field parser, or workers will write
/// terse useless verdicts to avoid breaking it.
#[test]
fn a_multi_word_verdict_survives_the_field_parser() {
    let signal = parse_completion(
        "DONE ipg.19 on %1408 -- verdict=all six acceptances met, mutation-verified \
         evidence=commit a3ab48d, tests/completion.rs frees=%1408",
        BEAD,
    )
    .expect("multi-word fields must parse");
    assert_eq!(signal.verdict, "all six acceptances met, mutation-verified");
    assert_eq!(signal.evidence, "commit a3ab48d, tests/completion.rs");
    assert_eq!(signal.frees_pane, "%1408");
}

/// ACCEPTANCE 6, ANTI-VACUITY, both directions. A completion mechanism that never
/// fires is indistinguishable from a fleet that never finishes anything — the exact
/// shape of the 178-tick capacity failure where every watchdog reported healthy.
#[test]
fn zero_completions_against_a_closed_bead_is_an_error() {
    assert!(assert_completions_not_vacuous(0, 3).is_err());
    // And the honest passes: nothing closed, or completions were seen.
    assert!(assert_completions_not_vacuous(0, 0).is_ok());
    assert!(assert_completions_not_vacuous(3, 3).is_ok());
    // The message must name the counts, or an operator cannot act on it.
    let error = assert_completions_not_vacuous(0, 7).unwrap_err();
    assert!(error.contains("COMPLETION_SIGNAL_VACUOUS"), "{error}");
    assert!(error.contains('7'), "{error}");
}

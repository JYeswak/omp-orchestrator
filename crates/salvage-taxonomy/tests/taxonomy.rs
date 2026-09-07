//! Acceptance for the salvage taxonomy: one fixture per row of the upstream table, plus the
//! legs this repository has paid for.
//!
//! NAMED TARGET per the mmt4 ruling: `cargo test -p salvage-taxonomy --test taxonomy`.
//!
//! EVERY negative leg asserts BOTH the message and the exit code. A message-only assertion is
//! defeasible and we have the mutation that defeats it: on `omp-orchestrator-2sx1`, collapsing
//! `EXIT_PUBLISH -> EXIT_MISSING_FIELD` left `FINDING_PUBLISH_FAILED` printed, so the token stayed
//! correct while the code was wrong.

use asupersync::runtime::RuntimeBuilder;
use asupersync::types::Budget;
use asupersync::Cx;
use salvage_taxonomy::{
    classify, classify_all, Classification, SalvageDecision, TaxonomyError, TurnEvidence,
    TurnOutcome, SILENT_DEATH_STALE_SECS,
};

/// Production `Cx` acquisition. `Cx::for_request` is `#[cfg(any(test, feature =
/// "test-internals"))]`-gated at asupersync `src/cx/cx.rs:5037` and is NOT available to a caller
/// that does not enable that feature, which this workspace deliberately does not — so the runtime
/// shape from `crates/pane-dispatch-fence/src/main.rs:193-201` is the one that works.
fn with_cx<T>(body: impl FnOnce(&Cx) -> T) -> T {
    let runtime = RuntimeBuilder::current_thread()
        .build()
        .expect("asupersync runtime");
    let cx = runtime.request_cx_with_budget(Budget::INFINITE);
    runtime.block_on(async move { body(&cx) })
}

fn classified(evidence: TurnEvidence) -> Classification {
    with_cx(|cx| classify(cx, &evidence).expect("classification"))
}

/// ROW 1 — KNOWN-GOOD, and mandatory. A clean exit with real work maps to `Verify` and to nothing
/// else. An attack-only suite ships an over-strict classifier, and an over-strict classifier gets
/// routed around, which is a slower death than no classifier.
#[test]
fn a_clean_exit_with_work_is_verify_and_nothing_else() {
    let result = classified(TurnEvidence {
        exit: Some(0),
        text: "done".to_owned(),
        landed_paths: 3,
        ack_present: true,
        ..Default::default()
    });
    assert_eq!(result.outcome, TurnOutcome::Finished);
    assert_eq!(result.decision, SalvageDecision::Verify);
    assert_eq!(result.exit_code(), 0, "a finished turn is the only zero code");
    assert!(
        result.reason.contains("SALVAGE_FINISHED") && result.reason.contains("landed_paths=3"),
        "the reason must name the evidence that drove the arm, got: {}",
        result.reason
    );
    assert!(
        !result.decision.is_relaunch(),
        "a finished turn must never be relaunched"
    );
}

/// ROW 2 — THE COUNTER-INTUITIVE ONE. A deadline means the work MOSTLY LANDED, so the move is
/// SALVAGE. A classifier that relaunches here destroys landed work, which is the single most
/// expensive mistake in the table.
#[test]
fn a_deadline_is_salvage_not_relaunch() {
    for text in [
        "Error: Deadline exceeded",
        "deadline_exceeded after 900s",
        "the request timed out",
    ] {
        let result = classified(TurnEvidence {
            exit: Some(1),
            text: text.to_owned(),
            landed_paths: 7,
            ..Default::default()
        });
        assert_eq!(
            result.outcome,
            TurnOutcome::DeadlineExceeded,
            "text {text:?} must classify as a deadline"
        );
        assert_eq!(result.decision, SalvageDecision::Salvage);
        assert_eq!(result.exit_code(), 10);
        assert!(
            !result.decision.is_relaunch(),
            "RELAUNCHING A DEADLINE DESTROYS LANDED WORK — text {text:?}"
        );
        assert!(
            result.reason.contains("orchestrator-written"),
            "the salvage reason must say the checkpoint is LABELLED orchestrator-written, got: {}",
            result.reason
        );
    }
}

/// ROW 3 — a provider error relaunches WITH a continue preamble, never as-is: a bare relaunch
/// rewrites what already landed.
#[test]
fn a_provider_error_relaunches_with_a_continue_preamble() {
    for text in ["provider error 502", "rate limit exceeded", "out of credit"] {
        let result = classified(TurnEvidence {
            exit: Some(1),
            text: text.to_owned(),
            landed_paths: 2,
            ..Default::default()
        });
        assert_eq!(result.outcome, TurnOutcome::ProviderError, "text {text:?}");
        assert_eq!(
            result.decision,
            SalvageDecision::RelaunchWithContinuePreamble
        );
        assert_eq!(result.exit_code(), 11);
        assert!(
            result.reason.contains("REMAINING scope"),
            "the preamble instruction must name the REMAINING scope, got: {}",
            result.reason
        );
    }
}

/// ROW 4 — no exit ever observed plus a stale session log is a dead process.
#[test]
fn a_stale_session_with_no_exit_is_treated_as_dead() {
    let result = classified(TurnEvidence {
        exit: None,
        session_stale_secs: Some(SILENT_DEATH_STALE_SECS),
        landed_paths: 5,
        ..Default::default()
    });
    assert_eq!(result.outcome, TurnOutcome::SilentProcessDeath);
    assert_eq!(result.decision, SalvageDecision::TreatAsDead);
    assert_eq!(result.exit_code(), 12);
    assert!(result.reason.contains("SALVAGE_SILENT_DEATH"));

    // BOUNDARY, in the safe direction: one second under the threshold must NOT be declared dead,
    // because the turn may still be running and a relaunch would duplicate it.
    let fresh = classified(TurnEvidence {
        exit: None,
        session_stale_secs: Some(SILENT_DEATH_STALE_SECS - 1),
        landed_paths: 5,
        ..Default::default()
    });
    assert_eq!(fresh.outcome, TurnOutcome::Unknown);
    assert_eq!(fresh.decision, SalvageDecision::Hold);
    assert!(!fresh.decision.is_relaunch());
}

/// ROW 5 — THE OTHER COUNTER-INTUITIVE ONE. `EXIT 0` with zero work is an empty completion, and
/// the move is to send the identical packet again rather than investigate.
#[test]
fn an_empty_completion_relaunches_as_is() {
    let result = classified(TurnEvidence {
        exit: Some(0),
        text: String::new(),
        landed_paths: 0,
        bead_comment_since_dispatch: true,
        ..Default::default()
    });
    assert_eq!(result.outcome, TurnOutcome::EmptyCompletion);
    assert_eq!(result.decision, SalvageDecision::RelaunchAsIs);
    assert_eq!(result.exit_code(), 13);
    assert!(result.reason.contains("no partial state to preserve"));
}

/// ANTI-VACUITY, **CODE HALF ONLY**. An unobserved turn and a finished turn must not report
/// identically to a caller reading the exit code.
///
/// PARTITIONED DELIBERATELY, and this is the correction of my own error. The single test that used
/// to live here was named `..._is_unknown_with_a_distinct_code_and_never_a_relaunch` — **two
/// `and`s joining two properties** — and it reddened under BOTH mutations, which I then published
/// as proof that the code and decision assertions were "provably independent". Overlapping failure
/// sets prove the opposite: at least one test conflates two properties. `%7` measured it.
///
/// A conjunctive test name is a conflated assertion advertising itself, and it is visible at write
/// time rather than at mutation time. This leg asserts the CODE and says nothing about the
/// decision, so the code mutation reddens it and the decision mutation must not.
#[test]
fn no_evidence_at_all_carries_a_code_distinct_from_finished() {
    let result = classified(TurnEvidence::default());
    assert_eq!(result.outcome, TurnOutcome::Unknown);
    assert_eq!(result.exit_code(), 20, "Unknown must not share Finished's 0");
    assert_ne!(
        result.exit_code(),
        Classification {
            outcome: TurnOutcome::Finished,
            decision: SalvageDecision::Verify,
            reason: String::new(),
        }
        .exit_code(),
        "'could not look' and 'done' must not be the same integer"
    );
}

/// ANTI-VACUITY, **DECISION HALF ONLY**, and it pins the EXACT reason text rather than a prefix.
///
/// A PREFIX IS NOT A MESSAGE. `AGENTS.md` rule 7 says pin the message AND the code because my
/// `2sx1` mutation left `FINDING_PUBLISH_FAILED` printed while the code collapsed `5 -> 3`. The
/// crate filed to demonstrate that rule then asserted `contains("SALVAGE_UNKNOWN")` — a prefix,
/// which SURVIVES the very mutation the commit body advertised as the headline: flipping this arm
/// to `RelaunchAsIs` leaves the word `HOLD` printed in the reason while the decision underneath it
/// is wrong. `%7` caught that the message half was unpinned. So the assertion below is the full
/// sentence, not its first token.
#[test]
fn no_evidence_at_all_holds_with_the_exact_reason_text() {
    let result = classified(TurnEvidence::default());
    assert_eq!(result.decision, SalvageDecision::Hold);
    assert!(
        !result.decision.is_relaunch(),
        "an unclassified turn is EXACTLY the case where a relaunch duplicates landed work"
    );
    assert_eq!(
        result.reason,
        "SALVAGE_UNKNOWN no evidence supplied — HOLD. Relaunching an unclassified turn is exactly \
         how landed work gets duplicated.",
        "the EXACT reason text, because a prefix assertion survives a decision mutation"
    );
}

/// THE `101` TRAP, and it is the leg this repository has paid for twice tonight. A nonzero exit is
/// not a discriminator: three different texts under the SAME exit code produce three different
/// outcomes, and the same code with NO text is a typed refusal rather than a guess.
#[test]
fn a_nonzero_exit_is_not_a_discriminator() {
    let outcomes: Vec<TurnOutcome> = ["Deadline exceeded", "provider error 503", "who knows"]
        .into_iter()
        .map(|text| {
            classified(TurnEvidence {
                exit: Some(1),
                text: text.to_owned(),
                landed_paths: 1,
                ..Default::default()
            })
            .outcome
        })
        .collect();
    assert_eq!(
        outcomes,
        vec![
            TurnOutcome::DeadlineExceeded,
            TurnOutcome::ProviderError,
            TurnOutcome::Unknown
        ],
        "one exit code, three outcomes — the TEXT is the discriminator"
    );

    let refusal = with_cx(|cx| {
        classify(
            cx,
            &TurnEvidence {
                exit: Some(1),
                text: "   ".to_owned(),
                landed_paths: 1,
                ..Default::default()
            },
        )
    })
    .expect_err("a nonzero exit with no text must be REFUSED, not guessed");
    assert_eq!(refusal, TaxonomyError::NonzeroExitWithoutText { exit: 1 });
    assert_eq!(refusal.exit_code(), 21, "a refusal code, distinct from every outcome");
    assert!(
        refusal.to_string().contains("workspace-load outage"),
        "the refusal must name why the code alone is insufficient, got: {refusal}"
    );
}

/// THE INVARIANT, asserted over EVERY variant rather than the arms someone remembered: no input
/// that yields `Unknown` may recommend a relaunch. A property beats a spot check because the next
/// person to add a case cannot quietly exempt it.
#[test]
fn unknown_never_recommends_a_relaunch_across_every_shape() {
    let shapes = vec![
        TurnEvidence::default(),
        TurnEvidence {
            exit: Some(1),
            text: "unrecognised failure".to_owned(),
            ..Default::default()
        },
        TurnEvidence {
            exit: None,
            session_stale_secs: Some(1),
            landed_paths: 9,
            ..Default::default()
        },
        TurnEvidence {
            exit: None,
            session_stale_secs: None,
            ack_present: true,
            ..Default::default()
        },
    ];
    let results = with_cx(|cx| classify_all(cx, &shapes).expect("batch"));
    assert_eq!(results.len(), shapes.len(), "one row in, one row out");
    for result in &results {
        assert_eq!(result.outcome, TurnOutcome::Unknown, "{}", result.reason);
        assert!(
            !result.decision.is_relaunch(),
            "Unknown recommended a relaunch: {}",
            result.reason
        );
        assert_eq!(result.decision, SalvageDecision::Hold);
    }
}

/// A profile-wide staleness figure must not be substituted for a pane-attributable one.
///
/// NAMED FOR WHAT IT ASSERTS. This leg used to be called
/// `an_unattributable_session_log_holds_rather_than_guessing` — "holds" is decision language, and
/// the body asserts the OUTCOME and the CODE. It reddened under the code mutation and stayed
/// green under the decision mutation, so the name promised a property the body never checked. The
/// decision half for this shape is covered by
/// `unknown_never_recommends_a_relaunch_across_every_shape`, which includes exactly this evidence
/// row, so nothing is lost by keeping this leg on the code side of the partition.
#[test]
fn an_unattributable_session_log_is_unknown_with_the_unknown_code() {
    let result = classified(TurnEvidence {
        exit: None,
        session_stale_secs: None,
        landed_paths: 4,
        bead_comment_since_dispatch: true,
        ..Default::default()
    });
    assert_eq!(result.outcome, TurnOutcome::Unknown);
    assert_eq!(result.exit_code(), 20);
    assert!(
        result.reason.contains("profile-wide"),
        "the reason must name the substitution it refuses, got: {}",
        result.reason
    );
}

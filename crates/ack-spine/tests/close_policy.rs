#![forbid(unsafe_code)]
//! Close-prefix policy reconciliation for `omp-orchestrator-mcq2`.
//!
//! The live tracker is an input to the census leg: a scan that sees no closed
//! rows is an error. The initial mcq2 census recorded 43 `MUTATION-VERIFIED`
//! rows as the positive control; concurrent independent regrades may reopen
//! some of those rows, so the assertion requires the control to remain live.
//! The test intentionally uses the classifier for row disposition so a future
//! prefix change cannot silently leave the census and classifier disagreeing.

use ack_spine::close_reason::{classify_close_reason, ClosePrefix, CloseReasonVerdict};
use ack_spine::ledger::StepKind;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

/// The four tokens documented by `AGENTS.md:1289`.
const DOCUMENTED_PREFIXES: &[&str] = &["MUTATION-VERIFIED", "DONE", "APPROVED", "WONTFIX"];

/// The initial mcq2 census recorded 43 closed positive-control rows.
const INITIAL_MUTATION_VERIFIED: usize = 43;

fn live_issue_rows() -> Vec<Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.beads/issues.jsonl");
    let contents = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "close-prefix census cannot read {}: {error}",
            path.display()
        )
    });
    contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .unwrap_or_else(|error| panic!("close-prefix census found invalid JSONL: {error}"))
        })
        .collect()
}

fn is_closed(row: &Value) -> bool {
    row.get("status").and_then(Value::as_str) == Some("closed")
}

fn close_reason(row: &Value) -> Option<&str> {
    row.get("close_reason").and_then(Value::as_str)
}

#[test]
fn documented_prefixes_equal_the_enforced_set() {
    let enforced: Vec<&str> = ClosePrefix::ALL
        .iter()
        .map(|prefix| prefix.as_str())
        .collect();
    assert_eq!(
        enforced.as_slice(),
        DOCUMENTED_PREFIXES,
        "the documented and classifier-enforced close-prefix sets diverged"
    );
}

#[test]
fn complete_and_partially_close_attempts_are_refused_with_admitted_prefixes() {
    for reason in ["COMPLETE: work claimed complete", "PARTIALLY confirmed"] {
        let verdict = classify_close_reason(Some(reason));
        assert_eq!(
            verdict,
            CloseReasonVerdict::PolicyRefused {
                leading: reason.split_whitespace().next().unwrap().to_owned()
            },
            "unadmitted close attempt must be refused: {reason}"
        );
        let rendered = verdict.to_string();
        for prefix in DOCUMENTED_PREFIXES {
            assert!(
                rendered.contains(prefix),
                "refusal must name admitted prefix {prefix}: {rendered}"
            );
        }
    }
}

#[test]
fn unadmitted_close_is_a_distinct_typed_ledger_kind() {
    assert_eq!(StepKind::Closed.as_str(), "closed");
    assert_eq!(
        StepKind::ClosedWithoutGrade.as_str(),
        "closed_without_grade",
        "an unadmitted close must not collapse into the ordinary closed kind"
    );
    assert_ne!(
        StepKind::Closed,
        StepKind::ClosedWithoutGrade,
        "ClosedWithoutGrade is the typed ack-spine outcome for an ungraded close"
    );
}

#[test]
fn live_closed_prefix_census_is_non_vacuous_and_has_no_unadmitted_rows() {
    let rows = live_issue_rows();
    let closed: Vec<&Value> = rows.iter().filter(|row| is_closed(row)).collect();
    assert!(
        !closed.is_empty(),
        "ANTI-VACUITY: zero closed rows is an ERROR"
    );

    let mutation_verified = closed
        .iter()
        .filter(|row| {
            matches!(
                classify_close_reason(close_reason(row)),
                CloseReasonVerdict::Verified {
                    prefix: ClosePrefix::MutationVerified
                }
            )
        })
        .count();
    assert!(
        mutation_verified > 0,
        "positive control disappeared: initial census had {INITIAL_MUTATION_VERIFIED} closed MUTATION-VERIFIED rows"
    );

    let unadmitted: Vec<String> = closed
        .iter()
        .filter_map(|row| {
            let reason = close_reason(row);
            (!matches!(
                classify_close_reason(reason),
                CloseReasonVerdict::Verified { .. }
            ))
            .then(|| {
                format!(
                    "{}: {}",
                    row.get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("<missing-id>"),
                    reason.unwrap_or("<missing-reason>")
                )
            })
        })
        .collect();
    assert!(
        unadmitted.is_empty(),
        "closed rows without an admitted prefix remain: {unadmitted:?}"
    );
}

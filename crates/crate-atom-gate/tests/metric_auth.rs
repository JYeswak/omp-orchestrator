//! 92zh — metric authorization. Known-bad uses a REAL checker name (C38).
#![forbid(unsafe_code)]

use crate_atom_gate::metric_auth::{
    adopted_subset, authorize_new_crate, authorize_new_crate_with, measurement_block_count,
    require_measurements, require_vector_text, AuthError, AuthVerdict, CrateProposal, Measurement,
    ADOPTED_HUMAN_INTERRUPTIONS, REQUIRE_METRIC_CLAIM,
};
use std::path::PathBuf;

fn ledger() -> crate_atom_gate::metric_auth::MetricVector {
    let mut v = adopted_subset();
    v.measurements.push(Measurement {
        metric: ADOPTED_HUMAN_INTERRUPTIONS.to_string(),
        numerator: 24,
        denominator_value: 42,
        source_artifact: "docs/decisions.jsonl".to_string(),
        quoted: "42 HD-* rows; 24 unanswered".to_string(),
    });
    v
}

/// Acc 5/7: empty ledger is an ERROR, never a pass.
#[test]
fn anti_vacuity_zero_measurements_is_an_error() {
    let empty = adopted_subset();
    let err = require_measurements(&empty).expect_err("empty ledger");
    assert!(matches!(err, AuthError::ZeroMeasurements), "{err:?}");
    let err = authorize_new_crate(
        &CrateProposal {
            crate_name: "path-literal-guard".to_string(),
            improves: Some(ADOPTED_HUMAN_INTERRUPTIONS.to_string()),
        },
        &empty,
    )
    .expect_err("known-good claim still errors on empty ledger");
    assert!(matches!(err, AuthError::ZeroMeasurements));
}

/// Acc 5: a REAL checker (path-literal-guard) naming no measure is REFUSED.
#[test]
fn known_bad_real_checker_without_a_measure_is_refused() {
    assert!(REQUIRE_METRIC_CLAIM);
    let verdict = authorize_new_crate(
        &CrateProposal {
            crate_name: "path-literal-guard".to_string(),
            improves: None,
        },
        &ledger(),
    )
    .expect("ledger nonempty");
    match verdict {
        AuthVerdict::Refused { crate_name, reason } => {
            assert_eq!(crate_name, "path-literal-guard");
            assert!(reason.contains("METRIC_UNNAMED"), "{reason}");
        }
        other => panic!("expected refuse, got {other:?}"),
    }
}

/// Acc 6: a crate that names an adopted measure is admitted. Gate must be passable.
#[test]
fn known_good_named_measure_is_admitted() {
    let verdict = authorize_new_crate(
        &CrateProposal {
            crate_name: "crate-atom-gate".to_string(),
            improves: Some(ADOPTED_HUMAN_INTERRUPTIONS.to_string()),
        },
        &ledger(),
    )
    .expect("ledger nonempty");
    assert!(verdict.admits(), "{verdict:?}");
}

/// Acc 8: mutate REQUIRE_METRIC_CLAIM's meaning via authorize path with unnamed
/// claim under a flipped policy — prove the unnamed crate would be admitted,
/// then restore the on-disk const byte-identically.
#[test]
fn mutation_admits_unnamed_then_restores() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/metric_auth.rs");
    let before = std::fs::read(&path).expect("read");
    let original = String::from_utf8(before.clone()).expect("utf8");
    struct Restore {
        path: PathBuf,
        original: Vec<u8>,
    }
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = std::fs::write(&self.path, &self.original);
        }
    }
    let restore = Restore {
        path: path.clone(),
        original: before.clone(),
    };
    assert!(original.contains("pub const REQUIRE_METRIC_CLAIM: bool = true;"));
    let mutated = original.replace(
        "pub const REQUIRE_METRIC_CLAIM: bool = true;",
        "pub const REQUIRE_METRIC_CLAIM: bool = false;",
    );
    std::fs::write(&path, mutated.as_bytes()).expect("mutate");

    // In-memory RED: the production function still has true (this process).
    // The file mutation is the checksum contract; the behavioral RED is the
    // unnamed-claim path when REQUIRE is treated as false, constructed here:
    let would_admit_if_flipped = CrateProposal {
        crate_name: "path-literal-guard".to_string(),
        improves: None,
    };
    let control = authorize_new_crate(&would_admit_if_flipped, &ledger()).expect("ledger");
    assert!(
        !control.admits(),
        "unflipped control must still refuse: {control:?}"
    );
    let mutated_verdict =
        authorize_new_crate_with(false, &would_admit_if_flipped, &ledger()).expect("ledger");
    assert!(
        mutated_verdict.admits(),
        "item1 RED: REQUIRE_METRIC_CLAIM=false must admit unnamed path-literal-guard, got {mutated_verdict:?}"
    );

    std::fs::write(&path, &restore.original).expect("restore");
    drop(restore);
    let after = std::fs::read(&path).expect("re-read");
    assert_eq!(before, after, "post-restore checksum diverged");
}

/// Acc 7 on the on-disk vector: the committed METRIC-VECTOR.toml has measurements.
#[test]
fn live_vector_file_is_not_vacuous() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/plan/METRIC-VECTOR.toml");
    let text = std::fs::read_to_string(&path).expect("METRIC-VECTOR.toml must exist");
    require_vector_text(&text).expect("live vector must have [[measurement]] blocks");
    assert!(
        measurement_block_count(&text) >= 2,
        "expected both adopted metrics to have a row, got {}",
        measurement_block_count(&text)
    );
    assert!(text.contains("human_interruptions"));
    assert!(text.contains("ready_frontier_utilization"));
    assert!(text.contains("docs/decisions.jsonl"));
    assert!(text.contains("tmux list-panes"));
}

/// Acc 1: the ten rejected WWJD names are named, not silently dropped.
#[test]
fn rejected_wwjd_names_are_declared() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/plan/METRIC-VECTOR.toml");
    let text = std::fs::read_to_string(&path).expect("vector");
    for id in [
        "T_first_ready",
        "T_beads_ready",
        "tokens_per_qualified_bead",
        "context_reconstruction_tokens",
        "false_ready_rate",
        "reopen_rate",
        "escaped_plan_defects",
        "contract_churn",
        "wasted_model_cost",
        "replay_success_rate",
    ] {
        assert!(
            text.contains(id),
            "rejected WWJD name {id} missing from the vector"
        );
    }
}

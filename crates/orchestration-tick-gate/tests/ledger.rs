use orchestration_tick_gate::{read_ledger, validate_receipt, LedgerError};
use serde_json::{json, Value};
use std::fs;

fn good_receipt() -> Value {
    json!({
        "ts": 1_700_000_000,
        "tick": 2,
        "observed": {
            "free_capacity": ["%1408"],
            "attention": 0,
            "dead": 0,
            "source": "tick-monitor observe"
        },
        "dispatched": [{
            "pane": "%1408",
            "bead": "omp-orchestrator-demo",
            "claimed_first": true,
            "receipt": "idle->working t=6"
        }],
        "refused": [],
        "landed": {
            "commits_since_last_tick": 0,
            "beads_closed": 0,
            "graded_by_non_implementer": 0
        },
        "claims": [{
            "figure": "free_capacity=1",
            "command": "tick-monitor observe --session omp-orchestrator"
        }],
        "destructive": [],
        "not_done": ["no edits to delegated paths"]
    })
}

#[test]
fn known_good_receipt_validates() {
    validate_receipt(&good_receipt()).expect("known-good production-shaped receipt");
}

#[test]
fn missing_free_pane_is_named_as_a_violation() {
    let mut receipt = good_receipt();
    receipt["dispatched"] = json!([]);
    let errors = validate_receipt(&receipt).expect_err("free pane omission must be red");
    assert!(errors.iter().any(|error| error.contains("OC-L1")));
    assert!(errors.iter().any(|error| error.contains("%1408")));
}

#[test]
fn investigation_with_free_capacity_is_named_as_a_violation() {
    let mut receipt = good_receipt();
    receipt["investigation"] = json!({"target": "pane-truth", "reason": "inspect"});
    let errors =
        validate_receipt(&receipt).expect_err("investigation must not route with free capacity");
    assert!(errors.iter().any(|error| error.contains("OC-L5")));
}

#[test]
fn edits_after_dispatch_are_named_as_a_violation() {
    let mut receipt = good_receipt();
    receipt["edits"] = json!([{"path": "crates/example/src/lib.rs"}]);
    let errors = validate_receipt(&receipt).expect_err("delegated edit must be red");
    assert!(errors.iter().any(|error| error.contains("OC-L3")));
}

#[test]
fn multiline_row_is_not_accepted_as_a_receipt() {
    let path = std::env::temp_dir().join(format!(
        "orchestration-tick-multiline-{}",
        std::process::id()
    ));
    fs::write(&path, b"{\"tick\": 1,\n\"observed\": {}}\n").unwrap();
    let error = read_ledger(&path).expect_err("a physical multiline row must be rejected");
    let _ = fs::remove_file(&path);
    assert!(matches!(error, LedgerError::Invalid(message) if message.contains("row=1")));
}

#[test]
fn empty_and_missing_ledgers_are_typed_errors() {
    let missing =
        std::env::temp_dir().join(format!("orchestration-tick-missing-{}", std::process::id()));
    assert!(matches!(
        read_ledger(&missing),
        Err(LedgerError::NothingToCheck(message)) if message.contains("NOTHING_TO_CHECK")
    ));

    let empty =
        std::env::temp_dir().join(format!("orchestration-tick-empty-{}", std::process::id()));
    fs::write(&empty, b"\n  \n").unwrap();
    let error = read_ledger(&empty).expect_err("empty ledger must be red");
    let _ = fs::remove_file(&empty);
    assert!(
        matches!(error, LedgerError::NothingToCheck(message) if message.contains("zero receipt rows"))
    );
}

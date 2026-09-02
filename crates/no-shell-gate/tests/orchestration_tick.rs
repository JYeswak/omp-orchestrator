//! Conformance gate for `docs/contracts/orchestration_contract.md`.
//!
//! This target gates the receipt shape and the six receipt-checkable laws. It
//! deliberately does not create or emit `.flywheel/orchestration-ticks.jsonl`:
//! the producer is a separate wiring concern and remains an explicit
//! BUILT != WIRED no-claim until a caller exists.

#![forbid(unsafe_code)]

use convergence_stamp::sha256_hex;
use orchestration_tick_gate::{read_ledger, validate_receipt, LedgerError};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const CONDUCT_NON_COVERAGE: &str =
    "NON-COVERAGE: OC-L3 and OC-L5 are operator-conduct laws; this receipt gate cannot enforce them";
const PRODUCER_NON_COVERAGE: &str =
    "NON-COVERAGE: no receipt producer is built or wired by this test target";

static FIXTURE_SEQ: AtomicU64 = AtomicU64::new(0);

fn good_receipt() -> Value {
    json!({
        "ts": 1_700_000_000,
        "tick": 2,
        "observed": {
            "free_capacity": ["%1408", "%1409"],
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
        "refused": [{
            "pane": "%1409",
            "reason": "admission=stale"
        }],
        "landed": {
            "commits_since_last_tick": 0,
            "beads_closed": 0,
            "graded_by_non_implementer": 0
        },
        "claims": [{
            "figure": "free_capacity=2",
            "command": "tick-monitor observe --session omp-orchestrator"
        }],
        "destructive": [],
        "not_done": ["no edits to delegated paths"]
    })
}

fn report(receipt: &Value) -> String {
    match validate_receipt(receipt) {
        Ok(()) => format!(
            "ORCHESTRATION_TICK_GATE status=CLEAN\n{CONDUCT_NON_COVERAGE}\n{PRODUCER_NON_COVERAGE}"
        ),
        Err(errors) => format!(
            "ORCHESTRATION_TICK_GATE status=VIOLATION\n{}\n{CONDUCT_NON_COVERAGE}\n{PRODUCER_NON_COVERAGE}",
            errors.join("\n")
        ),
    }
}


fn fixture_path(label: &str) -> PathBuf {
    let sequence = FIXTURE_SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "omp-orchestration-tick-{label}-{}-{sequence}.jsonl",
        std::process::id()
    ))
}

fn replace_once(bytes: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
    let position = bytes
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("mutation needle must exist in the fixture");
    let mut mutated = Vec::with_capacity(bytes.len() + replacement.len() - needle.len());
    mutated.extend_from_slice(&bytes[..position]);
    mutated.extend_from_slice(replacement);
    mutated.extend_from_slice(&bytes[position + needle.len()..]);
    mutated
}

#[test]
fn every_free_pane_is_dispatched_or_refused_with_a_reason() {
    let receipt = good_receipt();
    println!("{}", report(&receipt));
    assert!(validate_receipt(&receipt).is_ok());
}
#[test]
fn omitted_free_pane_is_red_and_names_the_pane() {
    let mut receipt = good_receipt();
    receipt["refused"] = json!([]);
    let error = validate_receipt(&receipt).expect_err("omitted free pane must be RED");
    println!(
        "ORCHESTRATION_TICK_GATE status=VIOLATION\n{}",
        error.join("\n")
    );
    assert!(error.iter().any(|line| line.contains("%1409")));
}

#[test]
fn no_dispatch_row_names_an_unclaimed_bead() {
    let mut receipt = good_receipt();
    receipt["dispatched"][0]["claimed_first"] = Value::Bool(false);
    let error = validate_receipt(&receipt).expect_err("unclaimed dispatch must be RED");
    println!(
        "ORCHESTRATION_TICK_GATE status=VIOLATION\n{}",
        error.join("\n")
    );
    assert!(error
        .iter()
        .any(|line| line.contains("omp-orchestrator-demo")));
}

#[test]
fn no_tick_row_edits_a_path_dispatched_in_an_earlier_row() {
    let output = report(&good_receipt());
    println!("{output}");
    assert!(output.contains("OC-L3"));
    assert!(output.contains("operator-conduct laws"));
}

#[test]
fn every_measured_field_carries_a_producing_command() {
    let mut receipt = good_receipt();
    receipt["claims"][0]["command"] = Value::String(String::new());
    let error = validate_receipt(&receipt).expect_err("claim without command must be RED");
    println!(
        "ORCHESTRATION_TICK_GATE status=VIOLATION\n{}",
        error.join("\n")
    );
    assert!(error.iter().any(|line| line.contains("OC-L4")));
}

#[test]
fn a_tick_row_with_an_investigation_field_also_shows_zero_free_capacity() {
    let output = report(&good_receipt());
    println!("{output}");
    assert!(output.contains("OC-L5"));
    assert!(output.contains("operator-conduct laws"));
}

#[test]
fn every_destructive_action_row_cites_a_prior_measurement_row() {
    let mut receipt = good_receipt();
    receipt["destructive"] = json!([{
        "action": "restart",
        "target": "omp-orchestrator",
        "evidence_row": 0
    }]);
    let error =
        validate_receipt(&receipt).expect_err("destructive action without evidence must be RED");
    println!(
        "ORCHESTRATION_TICK_GATE status=VIOLATION\n{}",
        error.join("\n")
    );
    assert!(error.iter().any(|line| line.contains("omp-orchestrator")));
}

#[test]
fn empty_or_missing_ledger_is_nothing_to_check_not_clean() {
    let missing = fixture_path("missing");
    let error = read_ledger(&missing).expect_err("missing ledger must not pass");
    println!("ORCHESTRATION_TICK_GATE status=NOTHING_TO_CHECK\n{error:?}");
    assert!(
        matches!(error, LedgerError::NothingToCheck(message) if message.contains("NOTHING_TO_CHECK"))
    );

    let empty = fixture_path("empty");
    fs::write(&empty, b"\n  \n").expect("write empty ledger fixture");
    let error = read_ledger(&empty).expect_err("empty ledger must not pass");
    let _ = fs::remove_file(&empty);
    println!("ORCHESTRATION_TICK_GATE status=NOTHING_TO_CHECK\n{error:?}");
    assert!(
        matches!(error, LedgerError::NothingToCheck(message) if message.contains("zero receipt rows"))
    );
}

#[test]
fn mutation_is_red_and_byte_identical_restore_is_green() {
    let path = fixture_path("mutation");
    let before = serde_json::to_vec(&good_receipt()).expect("serialize receipt fixture");
    let before_sha = sha256_hex(&before);
    fs::write(&path, &before).expect("write baseline receipt fixture");

    let mutated = replace_once(
        &before,
        b"\"claimed_first\":true",
        b"\"claimed_first\":false",
    );
    fs::write(&path, &mutated).expect("write mutated receipt fixture");
    let mutated_row: Value = serde_json::from_slice(&mutated).expect("parse mutated receipt");
    let mutation_error = validate_receipt(&mutated_row).expect_err("mutation must be RED");
    assert!(mutation_error.iter().any(|line| line.contains("OC-L2")));

    fs::write(&path, &before).expect("restore baseline receipt fixture");
    let after = fs::read(&path).expect("read restored receipt fixture");
    let after_sha = sha256_hex(&after);
    println!(
        "mutation sha256 before={before_sha} after={after_sha} byte_identical={}",
        before == after
    );
    assert_eq!(before, after, "mutation restore must be byte-identical");
    assert_eq!(before_sha, after_sha, "mutation restore digest must match");
    let restored: Value = serde_json::from_slice(&after).expect("parse restored receipt");
    assert!(validate_receipt(&restored).is_ok());
    fs::remove_file(path).expect("remove mutation fixture");
}

#[test]
fn gate_output_declares_unwired_receipt_producer() {
    let output = report(&good_receipt());
    println!("{output}");
    assert!(output.contains("no receipt producer is built or wired"));
}

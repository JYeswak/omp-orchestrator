#![forbid(unsafe_code)]

//! Thin-consumer legs for the committed control-plane `rch-reclaim-owner/v1`
//! contract. Every leg is behavioral: fixed argv, forwarded bytes, exit
//! codes, call order, and refusal reasons. No leg asserts on producer source
//! text. Cancellation legs run recovery on a FRESH request context, never the
//! cancelled caller one: a test that recovered on the caller Cx would refuse
//! here instead of passing.

use asupersync::runtime::RuntimeBuilder;
use asupersync::types::{Budget, CancelKind};
use asupersync::Cx;
use contabo_reclaim::{
    consume, consume_with, parse_args, recover_terminal_response_with, render_cli_error,
    request_json, ConsumerError, OwnerForwardedResponse, OwnerMachineryReason, OwnerMode,
    OwnerProcessOutput, CONSUMER_OUTER_BOUND, CONTROL_COMMAND_BOUND, MAX_OWNER_OUTPUT_BYTES,
    OWNER_BINARY, OWNER_MACHINERY_EXIT, OWNER_SCHEMA,
};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc::sync_channel;
use std::thread;

fn test_runtime() -> asupersync::runtime::Runtime {
    RuntimeBuilder::current_thread().build().expect("runtime")
}

fn ambient_cx() -> Cx {
    Cx::current().expect("runtime Cx")
}

fn recovery_cx_for(runtime: &asupersync::runtime::Runtime) -> Cx {
    runtime.request_cx_with_budget(Budget::INFINITE)
}

/// One full worker row in the exact owner shape. Every field the owner emits
/// is present; values are unjudged (no roster, no thresholds).
fn worker_row(worker_id: &str) -> Value {
    serde_json::json!({
        "worker": {"id": worker_id, "host": worker_id, "user": "root"},
        "prior_admission": "ENABLED",
        "drain": {"outcome": "SUCCEEDED", "observed_state": "ENABLED", "used_slots": 0, "evidence": []},
        "remote": null,
        "restore": {"outcome": "NOT_ATTEMPTED", "restored_state": null, "evidence": []},
        "outcome": "SUCCEEDED",
        "error": null
    })
}

/// Minimal valid owner answer with one full worker row. Sparse but complete:
/// the consumer forwards bytes it need not fully model, yet empty workers
/// are refused, so at least one row is required.
fn owner_response(request_id: &str, mode: &str, outcome: &str, exit_code: u8) -> Vec<u8> {
    serde_json::json!({
        "schema": OWNER_SCHEMA,
        "request_id": request_id,
        "mode": mode,
        "outcome": outcome,
        "workers": [worker_row("contabo-1")],
        "error": null,
        "exit_code": exit_code
    })
    .to_string()
    .into_bytes()
}

/// Owner answer carrying a top-level error, for the Error/Cancelled matrix
/// pairs and the pre-roster empty-workers shapes.
fn error_object(kind: &str, code: &str) -> Value {
    serde_json::json!({
        "kind": kind,
        "code": code,
        "message": "fixture",
        "evidence": []
    })
}

fn owner_response_with_error(
    request_id: &str,
    mode: &str,
    outcome: &str,
    exit_code: u8,
    error: Value,
) -> Vec<u8> {
    serde_json::json!({
        "schema": OWNER_SCHEMA,
        "request_id": request_id,
        "mode": mode,
        "outcome": outcome,
        "workers": [worker_row("contabo-1")],
        "error": error,
        "exit_code": exit_code
    })
    .to_string()
    .into_bytes()
}

/// Backed Partial: the folded WORKER_RECLAIM top still present on a Blocked
/// worker row, the shape the top/worker coupling gate admits. A SUCCEEDED
/// worker cannot source a WORKER_RECLAIM top.
fn owner_response_partial_backed(request_id: &str, mode: &str) -> Vec<u8> {
    let mut body: Value = serde_json::from_slice(&owner_response_with_error(
        request_id,
        mode,
        "PARTIAL",
        1,
        error_object("WORKER_RECLAIM", "WORKER_RECLAIM"),
    ))
    .expect("response");
    body["workers"][0]["outcome"] = Value::String("BLOCKED".to_owned());
    body["workers"][0]["error"] = error_object("WORKER_RECLAIM", "WORKER_RECLAIM");
    serde_json::to_vec(&body).expect("response JSON")
}

/// Backed BoundExceeded: the bound top still present on a BoundExceeded
/// worker row, the shape the top/worker backing rule admits.
fn owner_response_bound_backed(request_id: &str, mode: &str) -> Vec<u8> {
    let mut body: Value = serde_json::from_slice(&owner_response_with_error(
        request_id,
        mode,
        "BOUND_EXCEEDED",
        124,
        error_object("BOUND_EXCEEDED", "BOUND_EXCEEDED"),
    ))
    .expect("response");
    body["workers"][0]["outcome"] = Value::String("BOUND_EXCEEDED".to_owned());
    body["workers"][0]["error"] = error_object("BOUND_EXCEEDED", "BOUND_EXCEEDED");
    serde_json::to_vec(&body).expect("response JSON")
}

/// Release-folded Error/78 top: the prior error (kind/code) preserved from a
/// matching worker row, the top carrying the OWNER_LOCK_RELEASE_FAILED
/// marker the fold appends. `worker_outcome` is Some(Blocked/Error) for
/// worker-derived priors and None for pre-roster priors (empty workers).
fn owner_response_release_folded(
    request_id: &str,
    worker_outcome: Option<&str>,
    kind: &str,
    code: &str,
) -> Vec<u8> {
    let workers: Vec<Value> = match worker_outcome {
        Some(outcome) => {
            let mut row = worker_row("contabo-1");
            row.as_object_mut().expect("worker object").insert(
                "outcome".to_owned(),
                Value::String(outcome.to_owned()),
            );
            row.as_object_mut()
                .expect("worker object")
                .insert("error".to_owned(), error_object(kind, code));
            vec![row]
        }
        None => Vec::new(),
    };
    serde_json::json!({
        "schema": OWNER_SCHEMA,
        "request_id": request_id,
        "mode": "REPORT",
        "outcome": "ERROR",
        "workers": workers,
        "error": {
            "kind": kind,
            "code": code,
            "message": "fixture",
            "evidence": ["OWNER_LOCK_RELEASE_FAILED message=fixture evidence=[]"]
        },
        "exit_code": 78
    })
    .to_string()
    .into_bytes()
}

/// Valid pre-roster shape: NO worker rows, with the outcome naming the
/// failure and carrying its error. Anything implying visited workers with
/// none present refuses.
fn owner_response_empty_workers(
    request_id: &str,
    mode: &str,
    outcome: &str,
    exit_code: u8,
    error: Value,
) -> Vec<u8> {
    serde_json::json!({
        "schema": OWNER_SCHEMA,
        "request_id": request_id,
        "mode": mode,
        "outcome": outcome,
        "workers": [],
        "error": error,
        "exit_code": exit_code
    })
    .to_string()
    .into_bytes()
}

/// Populated owner answer in the exact owner wire shape: a nested remote
/// carrying QUARANTINE_INTENT effects and a pressure_mode the old consumer
/// could not parse. Must forward raw and unchanged.
fn owner_response_populated(request_id: &str, mode: &str) -> Vec<u8> {
    let mut worker = worker_row("contabo-1");
    worker
        .as_object_mut()
        .expect("worker object")
        .insert("outcome".to_owned(), Value::String("ERROR".to_owned()));
    worker
        .as_object_mut()
        .expect("worker object")
        .insert(
            "error".to_owned(),
            error_object("WORKER_RECLAIM", "WORKER_RECLAIM"),
        );
    worker
        .as_object_mut()
        .expect("worker object")
        .insert(
            "remote".to_owned(),
            serde_json::json!({
                "schema": OWNER_SCHEMA,
                "request_id": request_id,
                "mode": mode,
                "outcome": "ERROR",
                "owner_root": "/fixture/owner-root",
                "idle_threshold_secs": 86400,
                "pressure_mode": "CRITICAL",
                "candidates": [],
                "effects": [{
                    "kind": "QUARANTINE_INTENT",
                    "original": null,
                    "quarantine": null,
                    "bytes": 4096,
                    "evidence": "fixture"
                }],
                "error": {"kind": "WORKER_RECLAIM", "code": "WORKER_RECLAIM", "message": "fixture", "evidence": []},
                "exit_code": 1
            }),
        );
    serde_json::json!({
        "schema": OWNER_SCHEMA,
        "request_id": request_id,
        "mode": mode,
        "outcome": "PARTIAL",
        "workers": [worker],
        "error": {"kind": "WORKER_RECLAIM", "code": "WORKER_RECLAIM", "message": "fixture", "evidence": []},
        "exit_code": 1
    })
    .to_string()
    .into_bytes()
}

/// Durable status in the exact owner shape: a per-worker lease vector plus
/// the stored body exit. Stale flat fields ride along to prove lenient
/// tolerance of versioned extras. The stored exit must equal the subcommand
/// process exit the caller pairs it with.
fn durable_status(
    request_id: &str,
    mode: &str,
    leases: &[bool],
    terminal: Value,
    exit_code: u8,
) -> Vec<u8> {
    let workers: Vec<Value> = leases
        .iter()
        .enumerate()
        .map(|(index, lease)| {
            serde_json::json!({
                "worker_id": format!("contabo-{}", index + 1),
                "lease_active": lease,
                "phase": "WORKER",
                "restore": null,
                "effects": []
            })
        })
        .collect();
    serde_json::json!({
        "schema": OWNER_SCHEMA,
        "request_id": request_id,
        "state": {
            "schema": OWNER_SCHEMA,
            "request_id": request_id,
            "mode": mode,
            "workers": workers,
            "current_worker": "contabo-1",
            "phase": "COMPLETE"
        },
        "terminal": terminal,
        "kill_authorized": true,
        "exit_code": exit_code
    })
    .to_string()
    .into_bytes()
}

fn cancel_response(request_id: &str, requested: bool, exit_code: u8) -> Vec<u8> {
    serde_json::json!({
        "schema": OWNER_SCHEMA,
        "request_id": request_id,
        "cancellation_requested": requested,
        "exit_code": exit_code
    })
    .to_string()
    .into_bytes()
}

fn process_output(stdout: Vec<u8>, exit_code: u8) -> OwnerProcessOutput {
    OwnerProcessOutput {
        stdout,
        stderr: b"owner diagnostic".to_vec(),
        exit_code: Some(exit_code),
    }
}

fn consume_fake(
    request_id: &str,
    mode: OwnerMode,
    output: Result<OwnerProcessOutput, ConsumerError>,
) -> Result<OwnerForwardedResponse, ConsumerError> {
    let runtime = test_runtime();
    let recovery = recovery_cx_for(&runtime);
    runtime.block_on(async {
        let cx = ambient_cx();
        consume_with(&cx, &recovery, request_id, mode, move |_task_cx, _| {
            std::future::ready(output.clone())
        })
        .await
    })
}

fn assert_owner_refusal(
    result: Result<OwnerForwardedResponse, ConsumerError>,
    reason: OwnerMachineryReason,
) {
    let error = result.expect_err("owner consumer must refuse");
    assert_eq!(error.reason(), reason);
    assert_eq!(error.exit_code(), OWNER_MACHINERY_EXIT);
    assert!(error.to_string().contains("class=OWNER_MACHINERY"));
}

#[test]
fn exact_owner_contract_constants_are_stable() {
    assert_eq!(OWNER_SCHEMA, "control-plane.rch-reclaim-owner/v1");
    assert_eq!(OWNER_BINARY, "rch-reclaim-owner");
    assert_eq!(OWNER_MACHINERY_EXIT, 78);
    assert_eq!(CONTROL_COMMAND_BOUND.as_secs(), 15);
}

#[test]
fn outer_bound_is_the_only_owned_bound_and_covers_the_owner_worst_case() {
    // Pinned to owner wire v1: worst case 15 + 4*345 + 15 = 1410s. No worker
    // count, roster, threshold, or per-worker formula lives here.
    assert_eq!(CONSUMER_OUTER_BOUND.as_secs(), 1800);
    assert!(CONSUMER_OUTER_BOUND.as_secs() > 1410);
    assert_eq!(
        MAX_OWNER_OUTPUT_BYTES,
        17 * 1024 * 1024,
        "combined ceiling mirrors the owner 4MiB*4+1MiB response cap"
    );
}

#[test]
fn report_request_is_exactly_one_call_and_raw_response_is_unchanged() {
    let raw = owner_response("req-report", "REPORT", "SUCCEEDED", 0);
    let expected_raw = raw.clone();
    let calls = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
    let runtime = test_runtime();
    let recovery = recovery_cx_for(&runtime);
    let result = runtime.block_on(async {
        let cx = ambient_cx();
        let calls = Arc::clone(&calls);
        consume_with(
            &cx,
            &recovery,
            "req-report",
            OwnerMode::Report,
            move |_task_cx, args| {
                calls.lock().unwrap().push(args);
                std::future::ready(Ok(process_output(raw.clone(), 0)))
            },
        )
        .await
    });

    let forwarded = result.expect("valid owner response");
    // Exactly one call, fixed argv, and the adapter seam carries no stdin
    // parameter at all, so stdin smuggling cannot typecheck.
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(calls.lock().unwrap()[0], vec!["report", "req-report"]);
    assert_eq!(forwarded.raw_stdout, expected_raw);
    assert_eq!(forwarded.exit_code, 0);
    assert_eq!(forwarded.request_id, "req-report");
    assert!(!forwarded.recovered_from_status);
}

#[test]
fn apply_maps_to_apply_and_preserves_owner_partial_exit() {
    let raw = owner_response_populated("req-apply", "APPLY");
    let expected_raw = raw.clone();
    let calls = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
    let runtime = test_runtime();
    let recovery = recovery_cx_for(&runtime);
    let result = runtime
        .block_on(async {
            let cx = ambient_cx();
            let calls = Arc::clone(&calls);
            consume_with(
                &cx,
                &recovery,
                "req-apply",
                OwnerMode::Apply,
                move |_task_cx, args| {
                    calls.lock().unwrap().push(args);
                    std::future::ready(Ok(process_output(raw.clone(), 1)))
                },
            )
            .await
        })
        .expect("populated owner response with intents and pressure mode");

    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(calls.lock().unwrap()[0], vec!["apply", "req-apply"]);
    // Populated remote/effects forward raw and unchanged: the consumer never
    // needed to model them.
    assert_eq!(result.raw_stdout, expected_raw);
    assert_eq!(result.exit_code, 1);
}

#[test]
fn exact_owner_outcome_exit_matrix_forwards() {
    // The owner derives exits from outcomes (outcome.rs): each pair below is
    // an emittable combination. Pre-roster failures (77/78) carry no worker
    // rows; anything else does.
    let forward: Vec<(u8, &str, Value)> = vec![
        (0u8, "SUCCEEDED", Value::Null),
        (
            1u8,
            "PARTIAL",
            error_object("WORKER_RECLAIM", "WORKER_RECLAIM"),
        ),
        (
            77u8,
            "ERROR",
            error_object("ROSTER_UNMEASURABLE", "ROSTER_UNMEASURABLE"),
        ),
        (
            78u8,
            "CANCELLED",
            error_object("CANCELLED", "OWNER_CANCELLED"),
        ),
        (
            124u8,
            "BOUND_EXCEEDED",
            error_object("BOUND_EXCEEDED", "BOUND_EXCEEDED"),
        ),
    ];
    for (exit, outcome, error) in forward {
        // Pre-roster failures (77/78) carry no worker rows; anything else does.
        // The PARTIAL row carries a backing Blocked worker and the
        // BOUND_EXCEEDED row a backing BoundExceeded worker: a worker-severity
        // top must still be present on a compatible row.
        let raw = if exit == 77 || exit == 78 {
            owner_response_empty_workers("req-matrix", "REPORT", outcome, exit, error)
        } else if outcome == "PARTIAL" {
            owner_response_partial_backed("req-matrix", "REPORT")
        } else if outcome == "BOUND_EXCEEDED" {
            owner_response_bound_backed("req-matrix", "REPORT")
        } else if error.is_null() {
            owner_response("req-matrix", "REPORT", outcome, exit)
        } else {
            owner_response_with_error("req-matrix", "REPORT", outcome, exit, error)
        };
        let forwarded = consume_fake(
            "req-matrix",
            OwnerMode::Report,
            Ok(process_output(raw.clone(), exit)),
        )
        .unwrap_or_else(|_| panic!("matrix pair {outcome}/{exit} forwards"));
        assert_eq!(forwarded.raw_stdout, raw);
        assert_eq!(forwarded.exit_code, exit);
    }
}

#[test]
fn unknown_partial_effects_forwards_only_with_backing_evidence() {
    // Top-level UPE needs a nonzero exit AND a nested remote row carrying
    // UPE with nonempty effects; either half missing refuses.
    // Backed UPE: the nested remote carries UPE with nonempty effects, and
    // the row itself is UnknownPartialEffects with the remote's error — the
    // exact owner mapping (owner_result.rs UPE arm). A SUCCEEDED row cannot
    // wrap a failed remote, so that shape would refuse in worker validation.
    let mut backing = worker_row("contabo-1");
    backing
        .as_object_mut()
        .expect("worker object")
        .insert(
            "outcome".to_owned(),
            Value::String("UNKNOWN_PARTIAL_EFFECTS".to_owned()),
        );
    backing
        .as_object_mut()
        .expect("worker object")
        .insert(
            "error".to_owned(),
            error_object("WORKER_RECLAIM", "WORKER_RECLAIM"),
        );
    backing
        .as_object_mut()
        .expect("worker object")
        .insert(
            "remote".to_owned(),
            serde_json::json!({
                "schema": OWNER_SCHEMA,
                "request_id": "req-upe",
                "mode": "REPORT",
                "outcome": "UNKNOWN_PARTIAL_EFFECTS",
                "owner_root": "/fixture/owner-root",
                "idle_threshold_secs": 86400,
                "pressure_mode": "NORMAL",
                "candidates": [],
                "effects": [{
                    "kind": "PURGED",
                    "original": null,
                    "quarantine": null,
                    "bytes": 1,
                    "evidence": "backing"
                }],
                "error": {"kind": "WORKER_RECLAIM", "code": "WORKER_RECLAIM", "message": "fixture", "evidence": []},
                "exit_code": 1
            }),
        );
    let good = serde_json::json!({
        "schema": OWNER_SCHEMA,
        "request_id": "req-upe",
        "mode": "REPORT",
        "outcome": "UNKNOWN_PARTIAL_EFFECTS",
        "workers": [backing],
        "error": {"kind": "WORKER_RECLAIM", "code": "WORKER_RECLAIM", "message": "fixture", "evidence": []},
        "exit_code": 1
    })
    .to_string()
    .into_bytes();
    let forwarded = consume_fake(
        "req-upe",
        OwnerMode::Report,
        Ok(process_output(good.clone(), 1)),
    )
    .expect("backed UPE forwards");
    assert_eq!(forwarded.raw_stdout, good);
    // Same body with exit 0, and a UPE claim with no backing row, both refuse.
    let runtime = test_runtime();
    let recovery = recovery_cx_for(&runtime);
    let result = runtime.block_on(async {
        let cx = ambient_cx();
        consume_with(
            &cx,
            &recovery,
            "req-upe",
            OwnerMode::Report,
            move |_task_cx, args| match args.first().map(String::as_str) {
                Some("status") => {
                    std::future::ready(Ok(process_output(b"not-json".to_vec(), 0)))
                }
                _ => std::future::ready(Ok(process_output(
                    owner_response("req-upe", "REPORT", "UNKNOWN_PARTIAL_EFFECTS", 0),
                    0,
                ))),
            },
        )
        .await
    });
    let error = result.expect_err("unbacked zero-exit UPE must refuse");
    assert_eq!(error.reason(), OwnerMachineryReason::RecoveryNeeded);
    // (A) Exit-legal but unbacked: nonzero UPE top with a valid BLOCKED +
    // WORKER_RECLAIM row and a null remote. Every coupling passes, so ONLY
    // the backing predicate refuses it — deleting remote_backs_partial_effects
    // forwards this body and fails this leg.
    let mut lone = worker_row("contabo-1");
    lone.as_object_mut()
        .expect("worker object")
        .insert("outcome".to_owned(), Value::String("BLOCKED".to_owned()));
    lone.as_object_mut()
        .expect("worker object")
        .insert(
            "error".to_owned(),
            error_object("WORKER_RECLAIM", "WORKER_RECLAIM"),
        );
    let body_a = serde_json::json!({
        "schema": OWNER_SCHEMA,
        "request_id": "req-upe",
        "mode": "REPORT",
        "outcome": "UNKNOWN_PARTIAL_EFFECTS",
        "workers": [lone],
        "error": {"kind": "WORKER_RECLAIM", "code": "WORKER_RECLAIM", "message": "fixture", "evidence": []},
        "exit_code": 1
    });
    let raw_a = serde_json::to_vec(&body_a).expect("response JSON");
    // (B) Fully backed but exit 0: the backed fixture with only top/process
    // exit zeroed. Only exit legality refuses it, pinning the other half.
    let mut backed_zero: Value =
        serde_json::from_slice(&good).expect("backed fixture parses");
    backed_zero["exit_code"] = serde_json::json!(0);
    let raw_b = serde_json::to_vec(&backed_zero).expect("response JSON");
    for (name, raw, exit) in [
        ("unbacked-nonzero-upe", raw_a, 1u8),
        ("backed-zero-exit-upe", raw_b, 0u8),
    ] {
        let runtime = test_runtime();
        let recovery = recovery_cx_for(&runtime);
        let result = runtime.block_on(async {
            let cx = ambient_cx();
            consume_with(
                &cx,
                &recovery,
                "req-upe",
                OwnerMode::Report,
                move |_task_cx, args| match args.first().map(String::as_str) {
                    Some("status") => {
                        std::future::ready(Ok(process_output(b"not-json".to_vec(), 0)))
                    }
                    _ => std::future::ready(Ok(process_output(raw.clone(), exit))),
                },
            )
            .await
        });
        let error = result.expect_err("UPE gap must refuse");
        assert_eq!(
            error.reason(),
            OwnerMachineryReason::RecoveryNeeded,
            "gap {name} must refuse"
        );
    }
}

#[test]
fn empty_workers_forward_only_for_pre_roster_error_and_cancelled() {
    // Pre-roster shapes (owner lock/validation/roster): no rows, outcome
    // naming the failure, error carried, exit per matrix.
    for (outcome, exit, kind, code) in [
        ("ERROR", 77u8, "ROSTER_UNMEASURABLE", "ROSTER_UNMEASURABLE"),
        ("ERROR", 78u8, "OWNER_MACHINERY", "OWNER_MACHINERY"),
        ("ERROR", 78u8, "TRANSPORT", "TRANSPORT"),
        ("CANCELLED", 78u8, "CANCELLED", "OWNER_CANCELLED"),
    ] {
        let raw = owner_response_empty_workers(
            "req-empty-ok",
            "REPORT",
            outcome,
            exit,
            error_object(kind, code),
        );
        let forwarded = consume_fake(
            "req-empty-ok",
            OwnerMode::Report,
            Ok(process_output(raw.clone(), exit)),
        )
        .unwrap_or_else(|_| panic!("pre-roster {outcome}/{exit} forwards"));
        assert_eq!(forwarded.raw_stdout, raw);
        assert_eq!(forwarded.exit_code, exit);
    }
    // Anything implying visited workers — or an error claim without its
    // error — refuses.
    for (outcome, exit, error) in [
        ("SUCCEEDED", 0u8, Value::Null),
        ("PARTIAL", 1u8, Value::Null),
        ("BOUND_EXCEEDED", 124u8, Value::Null),
        ("ERROR", 78u8, Value::Null),
    ] {
        let raw = owner_response_empty_workers("req-empty-no", "REPORT", outcome, exit, error);
        let result = consume_fake(
            "req-empty-no",
            OwnerMode::Report,
            Ok(process_output(raw, exit)),
        );
        let refusal = result.expect_err("empty workers outside pre-roster shapes must refuse");
        assert_eq!(refusal.reason(), OwnerMachineryReason::RecoveryNeeded);
    }
}

#[test]
fn outcome_exit_mismatches_and_invented_values_refuse() {
    // Wrong pairings and invented outcomes/values refuse even though the
    // process exit agrees with the response field in every case.
    for (exit, outcome) in [
        (0u8, "PARTIAL"),
        (1u8, "SUCCEEDED"),
        (1u8, "ERROR"),
        (42u8, "SUCCEEDED"),
        (0u8, "OK"),
        (0u8, "BOGUS"),
        (0u8, "success"),
        (78u8, "SUCCEEDED"),
    ] {
        let runtime = test_runtime();
        let recovery = recovery_cx_for(&runtime);
        let result = runtime.block_on(async {
            let cx = ambient_cx();
            consume_with(
                &cx,
                &recovery,
                "req-matrix",
                OwnerMode::Report,
                move |_task_cx, args| match args.first().map(String::as_str) {
                    Some("status") => {
                        std::future::ready(Ok(process_output(b"not-json".to_vec(), 0)))
                    }
                    _ => std::future::ready(Ok(process_output(
                        owner_response("req-matrix", "REPORT", outcome, exit),
                        exit,
                    ))),
                },
            )
            .await
        });
        let error = result.expect_err("matrix violation must not forward");
        assert_eq!(error.reason(), OwnerMachineryReason::RecoveryNeeded);
        assert!(
            error.detail().contains(outcome) || error.detail().contains(&exit.to_string()),
            "the matrix refusal names the offending value"
        );
    }
}

#[test]
fn top_error_coupling_violations_refuse() {
    // The top error must be the kind that produces the paired exit: a
    // Succeeded answer carries none, Partial carries the folded worker
    // error, and Error carries the failure kind for its exit. STATUS stays
    // unmeasurable throughout.
    let cases: Vec<(&str, u8, Value)> = vec![
        ("SUCCEEDED", 0u8, error_object("WORKER_RECLAIM", "WORKER_RECLAIM")),
        ("PARTIAL", 1u8, Value::Null),
        (
            "PARTIAL",
            1u8,
            error_object("TRANSPORT", "TRANSPORT"),
        ),
        ("ERROR", 77u8, error_object("WORKER_RECLAIM", "WORKER_RECLAIM")),
        ("ERROR", 78u8, error_object("CANCELLED", "OWNER_CANCELLED")),
        ("CANCELLED", 78u8, Value::Null),
        ("BOUND_EXCEEDED", 124u8, Value::Null),
    ];
    for (outcome, exit, error) in cases {
        let body = if error.is_null() {
            owner_response("req-toperr", "REPORT", outcome, exit)
        } else {
            owner_response_with_error("req-toperr", "REPORT", outcome, exit, error)
        };
        let runtime = test_runtime();
        let recovery = recovery_cx_for(&runtime);
        let result = runtime.block_on(async {
            let cx = ambient_cx();
            consume_with(
                &cx,
                &recovery,
                "req-toperr",
                OwnerMode::Report,
                move |_task_cx, args| match args.first().map(String::as_str) {
                    Some("status") => {
                        std::future::ready(Ok(process_output(b"not-json".to_vec(), 0)))
                    }
                    _ => std::future::ready(Ok(process_output(body.clone(), exit))),
                },
            )
            .await
        });
        let refusal = result.expect_err("top error coupling violation must refuse");
        assert_eq!(refusal.reason(), OwnerMachineryReason::RecoveryNeeded);
    }
}

#[test]
fn missing_malformed_and_exitless_owner_results_are_restrictive() {
    assert_owner_refusal(
        consume_fake(
            "req-missing-owner",
            OwnerMode::Report,
            Err(ConsumerError::owner_machinery(
                OwnerMachineryReason::Spawn,
                "rch-reclaim-owner missing",
            )),
        ),
        OwnerMachineryReason::Spawn,
    );
    // Empty, malformed, and exit-less direct answers attempt STATUS first;
    // with an unmeasurable status they land restrictive.
    let cases: Vec<(&str, Vec<u8>, Option<u8>)> = vec![
        ("req-empty", Vec::new(), Some(0u8)),
        ("req-malformed", b"not-json".to_vec(), Some(0u8)),
        (
            "req-no-exit",
            owner_response("req-no-exit", "REPORT", "SUCCEEDED", 0),
            None,
        ),
    ];
    for (id, stdout, exit) in cases {
        let runtime = test_runtime();
        let recovery = recovery_cx_for(&runtime);
        let result = runtime.block_on(async {
            let cx = ambient_cx();
            consume_with(
                &cx,
                &recovery,
                id,
                OwnerMode::Report,
                move |_task_cx, args| match args.first().map(String::as_str) {
                    Some("status") => {
                        std::future::ready(Ok(process_output(b"not-json".to_vec(), 0)))
                    }
                    _ => std::future::ready(Ok(OwnerProcessOutput {
                        stdout: stdout.clone(),
                        stderr: Vec::new(),
                        exit_code: exit,
                    })),
                },
            )
            .await
        });
        let error = result.expect_err("ambiguous direct plus bad status must refuse");
        assert_eq!(error.reason(), OwnerMachineryReason::RecoveryNeeded);
    }
}

#[test]
fn oversize_combined_output_names_the_ceiling() {
    let big = vec![b'x'; MAX_OWNER_OUTPUT_BYTES + 1];
    let runtime = test_runtime();
    let recovery = recovery_cx_for(&runtime);
    let result = runtime.block_on(async {
        let cx = ambient_cx();
        consume_with(
            &cx,
            &recovery,
            "req-big",
            OwnerMode::Report,
            move |_task_cx, args| match args.first().map(String::as_str) {
                Some("status") => {
                    std::future::ready(Ok(process_output(b"not-json".to_vec(), 0)))
                }
                _ => std::future::ready(Ok(process_output(big.clone(), 0))),
            },
        )
        .await
    });
    let error = result.expect_err("oversize direct plus bad status must refuse");
    assert_eq!(error.reason(), OwnerMachineryReason::RecoveryNeeded);
    assert!(error.detail().contains("exceeds"));
}

#[test]
fn invalid_request_ids_refuse_before_any_spawn() {
    let bad: Vec<String> = vec![
        "".to_owned(),
        "x".repeat(97),
        "has space".to_owned(),
        "semi;colon".to_owned(),
        "quo'te".to_owned(),
    ];
    for id in &bad {
        let calls = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
        let runtime = test_runtime();
        let recovery = recovery_cx_for(&runtime);
        let result = runtime.block_on(async {
            let cx = ambient_cx();
            let calls = Arc::clone(&calls);
            consume_with(&cx, &recovery, id, OwnerMode::Report, move |_task_cx, args| {
                calls.lock().unwrap().push(args);
                std::future::ready(Err(ConsumerError::owner_machinery(
                    OwnerMachineryReason::Spawn,
                    "must never spawn",
                )))
            })
            .await
        });
        let error = result.expect_err("invalid request id must refuse");
        assert_eq!(error.reason(), OwnerMachineryReason::RequestIdInvalid);
        assert!(
            calls.lock().unwrap().is_empty(),
            "grammar refusal precedes any spawn for {id:?}"
        );
    }
}

#[test]
fn cancellation_invokes_cancel_and_status_in_order() {
    let terminal: Value =
        serde_json::from_slice(&owner_response("req-cancel", "REPORT", "SUCCEEDED", 0))
            .expect("terminal");
    let status_bytes = durable_status("req-cancel", "REPORT", &[false], terminal, 0);
    let cancel_bytes = cancel_response("req-cancel", true, 0);
    let calls = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
    let (started_tx, started_rx) = sync_channel::<()>(0);
    let runtime = test_runtime();
    let recovery = recovery_cx_for(&runtime);
    let result = runtime.block_on(async {
        let cx = ambient_cx();
        let cancel_cx = cx.clone();
        let trigger = thread::spawn(move || {
            started_rx.recv().expect("initial owner invocation");
            cancel_cx.cancel_with(CancelKind::User, Some("test cancellation"));
        });
        let calls_for_invoke = Arc::clone(&calls);
        let mut call_number = 0;
        let result = consume_with(
            &cx,
            &recovery,
            "req-cancel",
            OwnerMode::Report,
            move |_task_cx, args| {
                calls_for_invoke.lock().unwrap().push(args.clone());
                call_number += 1;
                match args.first().map(String::as_str) {
                    Some("report") => {
                        started_tx.send(()).expect("notify cancellation trigger");
                        boxed_pending()
                    }
                    Some("cancel") => {
                        boxed_ready(Ok(process_output(cancel_bytes.clone(), 0)))
                    }
                    Some("status") => {
                        boxed_ready(Ok(process_output(status_bytes.clone(), 0)))
                    }
                    other => panic!("unexpected owner command {other:?} at call {call_number}"),
                }
            },
        )
        .await;
        trigger.join().expect("cancellation trigger");
        result
    });

    // Recovery runs on its own context: the caller Cx above is cancelled and
    // the verdict below proves the recovery Cx was not.
    let recovered = result.expect("durable status recovery");
    assert!(recovered.recovered_from_status);
    assert_eq!(recovered.request_id, "req-cancel");
    assert_eq!(recovered.exit_code, 0);
    let observed = calls.lock().unwrap();
    assert_eq!(observed.len(), 3);
    assert_eq!(observed[0], vec!["report", "req-cancel"]);
    assert_eq!(observed[1], vec!["cancel", "req-cancel"]);
    assert_eq!(observed[2], vec!["status", "req-cancel"]);
}

#[test]
fn cancel_negatives_stay_restrictive_while_status_characterizes() {
    // Malformed, mismatched, negative, field-absent, nonzero, and empty
    // cancel answers: none grants anything alone, and STATUS still runs on
    // the independent recovery context.
    let shapes: Vec<(&str, Vec<u8>, u8)> = vec![
        ("false", cancel_response("req-cancelneg", false, 0), 0),
        ("mismatch", cancel_response("other-request", true, 0), 0),
        ("nonzero", cancel_response("req-cancelneg", true, 1), 1),
        (
            "absent",
            br#"{"schema":"control-plane.rch-reclaim-owner/v1","request_id":"req-cancelneg","exit_code":0}"#.to_vec(),
            0,
        ),
        ("garbage", b"not-json".to_vec(), 0),
        ("empty", Vec::new(), 0),
    ];
    for (name, body, exit) in shapes {
        let terminal: Value =
            serde_json::from_slice(&owner_response("req-cancelneg", "REPORT", "SUCCEEDED", 0))
                .expect("terminal");
        let status_bytes = durable_status("req-cancelneg", "REPORT", &[false], terminal, 0);
        let calls = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
        let (started_tx, started_rx) = sync_channel::<()>(0);
        let runtime = test_runtime();
        let recovery = recovery_cx_for(&runtime);
        let result = runtime.block_on(async {
            let cx = ambient_cx();
            let cancel_cx = cx.clone();
            let trigger = thread::spawn(move || {
                started_rx.recv().expect("initial owner invocation");
                cancel_cx.cancel_with(CancelKind::User, Some("test cancellation"));
            });
            let calls_for_invoke = Arc::clone(&calls);
            let result = consume_with(
                &cx,
                &recovery,
                "req-cancelneg",
                OwnerMode::Report,
                move |_task_cx, args| {
                    calls_for_invoke.lock().unwrap().push(args.clone());
                    match args.first().map(String::as_str) {
                        Some("report") => {
                            started_tx.send(()).expect("notify cancellation trigger");
                            boxed_ready(Err(ConsumerError::owner_machinery(
                                OwnerMachineryReason::OwnerWait,
                                "fake run observed cancel",
                            )))
                        }
                        Some("cancel") => boxed_ready(Ok(process_output(body.clone(), exit))),
                        Some("status") => {
                            boxed_ready(Ok(process_output(status_bytes.clone(), 0)))
                        }
                        other => panic!("unexpected owner command {other:?}"),
                    }
                },
            )
            .await;
            trigger.join().expect("cancellation trigger");
            result
        });
        let recovered = result
            .unwrap_or_else(|_| panic!("negative cancel {name} must still reach status"));
        assert!(recovered.recovered_from_status, "shape {name}");
        assert_eq!(recovered.exit_code, 0);
        let observed = calls.lock().unwrap();
        assert_eq!(observed.len(), 3, "shape {name}");
        assert_eq!(observed[0], vec!["report", "req-cancelneg"]);
        assert_eq!(observed[1], vec!["cancel", "req-cancelneg"]);
        assert_eq!(observed[2], vec!["status", "req-cancelneg"]);
    }
}

#[test]
fn negative_cancel_plus_bad_status_names_both_failures() {
    let runtime = test_runtime();
    let recovery = recovery_cx_for(&runtime);
    let result = runtime.block_on(async {
        let cx = ambient_cx();
        let cancel_cx = cx.clone();
        let (started_tx, started_rx) = sync_channel::<()>(0);
        let trigger = thread::spawn(move || {
            started_rx.recv().expect("initial owner invocation");
            cancel_cx.cancel_with(CancelKind::User, Some("test cancellation"));
        });
        let result = consume_with(
            &cx,
            &recovery,
            "req-cancelneg",
            OwnerMode::Report,
            move |_task_cx, args| match args.first().map(String::as_str) {
                Some("report") => {
                    started_tx.send(()).expect("notify cancellation trigger");
                    boxed_ready(Err(ConsumerError::owner_machinery(
                        OwnerMachineryReason::OwnerWait,
                        "fake run observed cancel",
                    )))
                }
                Some("cancel") => boxed_ready(Ok(process_output(
                    cancel_response("req-cancelneg", false, 0),
                    0,
                ))),
                Some("status") => boxed_ready(Ok(process_output(b"not-json".to_vec(), 0))),
                other => panic!("unexpected owner command {other:?}"),
            },
        )
        .await;
        trigger.join().expect("cancellation trigger");
        result
    });
    let error = result.expect_err("bad cancel plus bad status must refuse");
    assert_eq!(error.reason(), OwnerMachineryReason::RecoveryNeeded);
    assert_eq!(error.request_id(), Some("req-cancelneg"));
    assert!(
        error.detail().contains("cancel"),
        "cancel failure is named"
    );
    assert!(
        error.detail().contains("status"),
        "status failure is named"
    );
}

#[test]
fn malformed_status_returns_recovery_needed() {
    let calls = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
    let runtime = test_runtime();
    let recovery = recovery_cx_for(&runtime);
    let result = runtime.block_on(async {
        let cx = ambient_cx();
        let calls_for_invoke = Arc::clone(&calls);
        let result = consume_with(
            &cx,
            &recovery,
            "req-unknown-status",
            OwnerMode::Report,
            move |_task_cx, args| {
                calls_for_invoke.lock().unwrap().push(args.clone());
                match args.first().map(String::as_str) {
                    Some("report") => boxed_ready(Err(ConsumerError::owner_machinery(
                        OwnerMachineryReason::OwnerWait,
                        "fake run ended",
                    ))),
                    Some("cancel") => boxed_ready(Ok(process_output(
                        cancel_response("req-unknown-status", true, 0),
                        0,
                    ))),
                    Some("status") => {
                        boxed_ready(Ok(process_output(b"not-json".to_vec(), 0)))
                    }
                    other => panic!("unexpected owner command {other:?}"),
                }
            },
        )
        .await;
        result
    });

    let error = result.expect_err("malformed status must require recovery");
    assert_eq!(error.reason(), OwnerMachineryReason::RecoveryNeeded);
}

#[test]
fn status_gates_exact_worker_vector_and_terminal_presence() {
    let good_terminal: Value =
        serde_json::from_slice(&owner_response("req-status", "REPORT", "SUCCEEDED", 0))
            .expect("terminal");
    // One active lease refuses, even beside an inactive twin. Each case
    // cancels its dedicated recovery Cx on the first invalid observation:
    // poll_status would otherwise poll a parseable-but-nonterminal status
    // to the 1800s outer bound. Deterministic on a current-thread runtime:
    // the cancel flag precedes try_join every quantum, so the very next
    // poll expires instead of completing into another 15s sleep.
    for leases in [vec![true], vec![false, true], vec![true, false]] {
        let status = durable_status("req-status", "REPORT", &leases, good_terminal.clone(), 0);
        let runtime = test_runtime();
        let recovery = recovery_cx_for(&runtime);
        let cancel_cx = recovery.clone();
        let result = runtime.block_on(async {
            recover_terminal_response_with(
                &recovery,
                "req-status",
                OwnerMode::Report,
                move |_task_cx, args| {
                    assert_eq!(args, vec!["status", "req-status"]);
                    cancel_cx.cancel_with(CancelKind::User, Some("invalid status observed"));
                    std::future::ready(Ok(process_output(status.clone(), 0)))
                },
            )
            .await
        });
        let error = result.expect_err("active lease must refuse terminal forward");
        assert_eq!(error.reason(), OwnerMachineryReason::RecoveryNeeded);
        // No lease wording here: the cancel short-circuit refuses before
        // forwarding, so lease causality is pinned in the lib unit test
        // status_forward_names_lease_and_terminal_gates instead.
    }
    // No terminal refuses, even with every lease quiet. Same condition-driven
    // bound as above: cancel on the first observation, never poll to 1800s.
    let no_terminal = durable_status("req-status", "REPORT", &[false], Value::Null, 0);
    let runtime = test_runtime();
    let recovery = recovery_cx_for(&runtime);
    let cancel_cx = recovery.clone();
    let result = runtime.block_on(async {
        recover_terminal_response_with(
            &recovery,
            "req-status",
            OwnerMode::Report,
            move |_task_cx, args| {
                assert_eq!(args, vec!["status", "req-status"]);
                cancel_cx.cancel_with(CancelKind::User, Some("invalid status observed"));
                std::future::ready(Ok(process_output(no_terminal.clone(), 0)))
            },
        )
        .await
    });
    let error = result.expect_err("terminal absence must refuse");
    assert_eq!(error.reason(), OwnerMachineryReason::RecoveryNeeded);
    let mut terminal: Value = serde_json::from_slice(&owner_response_with_error(
        "req-stdout",
        "APPLY",
        "PARTIAL",
        1,
        error_object("WORKER_RECLAIM", "WORKER_RECLAIM"),
    ))
    .expect("terminal");
    // The folded top must still be present on a matching row: an Error
    // worker carrying the WORKER_RECLAIM error backs this terminal.
    terminal["workers"][0]["outcome"] = Value::String("ERROR".to_owned());
    terminal["workers"][0]["error"] = error_object("WORKER_RECLAIM", "WORKER_RECLAIM");
    let status = durable_status("req-stdout", "APPLY", &[false, false], terminal, 0);
    let calls = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
    let runtime = test_runtime();
    let recovery = recovery_cx_for(&runtime);
    let recovered = runtime
        .block_on(async {
            let calls = Arc::clone(&calls);
            recover_terminal_response_with(
                &recovery,
                "req-stdout",
                OwnerMode::Apply,
                move |_task_cx, args| {
                    calls.lock().unwrap().push(args);
                    std::future::ready(Ok(process_output(status.clone(), 0)))
                },
            )
            .await
        })
        .expect("status terminal response");

    assert!(recovered.recovered_from_status);
    assert_eq!(recovered.request_id, "req-stdout");
    assert_eq!(recovered.exit_code, 1);
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(calls.lock().unwrap()[0], vec!["status", "req-stdout"]);
}

#[test]
fn status_polls_until_every_lease_is_quiet() {
    let terminal: Value =
        serde_json::from_slice(&owner_response("req-poll", "REPORT", "SUCCEEDED", 0))
            .expect("terminal");
    let active = durable_status("req-poll", "REPORT", &[true], terminal.clone(), 0);
    let quiet = durable_status("req-poll", "REPORT", &[false], terminal, 0);
    let calls = Arc::new(Mutex::new(0usize));
    let runtime = test_runtime();
    let recovery = recovery_cx_for(&runtime);
    let recovered = runtime
        .block_on(async {
            let calls = Arc::clone(&calls);
            recover_terminal_response_with(
                &recovery,
                "req-poll",
                OwnerMode::Report,
                move |_task_cx, args| {
                    assert_eq!(args, vec!["status", "req-poll"]);
                    let mut count = calls.lock().unwrap();
                    *count += 1;
                    let body = if *count < 2 { active.clone() } else { quiet.clone() };
                    std::future::ready(Ok(process_output(body, 0)))
                },
            )
            .await
        })
        .expect("polling status reaches the quiet terminal");
    assert!(recovered.recovered_from_status);
    assert_eq!(*calls.lock().unwrap(), 2);
}

#[test]
fn json_parse_errors_keep_the_typed_stderr_envelope_shape() {
    let arguments = vec![
        "--request-id".to_owned(),
        "req-parse".to_owned(),
        "--json".to_owned(),
        "--not-an-owner-option".to_owned(),
    ];
    let error = parse_args(&arguments).expect_err("unknown CLI option must refuse");
    let rendered = render_cli_error(&error, request_json(&arguments));
    let envelope: Value = serde_json::from_str(&rendered).expect("JSON error envelope");
    assert_eq!(envelope["schema"], "contabo-reclaim/report-v1");
    assert_eq!(envelope["outcome"], "ERROR");
    assert!(envelope["error"]
        .as_str()
        .is_some_and(|message| message.contains("UNKNOWN_ARGUMENT")));
}

#[test]
fn parser_exposes_only_owner_mode_and_request_id() {
    let error = parse_args(&[
        "--worker".to_owned(),
        "contabo-1".to_owned(),
        "--base".to_owned(),
        "/var/lib/rch".to_owned(),
    ])
    .expect_err("local worker and base ownership must not remain");
    assert!(error.detail().contains("UNKNOWN_ARGUMENT"));

    let parsed = parse_args(&[
        "apply".to_owned(),
        "--request-id=req-ordered".to_owned(),
        "--json".to_owned(),
    ])
    .expect("owner arguments");
    assert_eq!(parsed.mode, OwnerMode::Apply);
    assert_eq!(parsed.request_id, "req-ordered");
    assert!(parsed.json);
}

#[test]
fn cli_duplicate_and_illegal_request_ids_refuse() {
    let error = parse_args(&[
        "--request-id".to_owned(),
        "req-a".to_owned(),
        "--request-id".to_owned(),
        "req-b".to_owned(),
        "report".to_owned(),
    ])
    .expect_err("duplicate request id must refuse, never last-wins");
    assert!(error.detail().contains("REQUEST_ID_MULTIPLE"));

    let error = parse_args(&[
        "--request-id=req-a".to_owned(),
        "--request-id=req-b".to_owned(),
        "report".to_owned(),
    ])
    .expect_err("duplicate request id must refuse in either spelling");
    assert!(error.detail().contains("REQUEST_ID_MULTIPLE"));

    let bad: Vec<String> = vec![
        "".to_owned(),
        "x".repeat(97),
        "has space".to_owned(),
        "semi;colon".to_owned(),
    ];
    for id in &bad {
        let error = parse_args(&[
            "--request-id".to_owned(),
            id.clone(),
            "report".to_owned(),
        ])
        .expect_err("illegal request id must refuse");
        assert!(
            error.detail().contains("REQUEST_ID_INVALID"),
            "illegal id {id:?} names the grammar"
        );
    }
}

/// Serializes the legs that mutate process-global env (PATH plus the fake
/// inputs). Cargo runs legs in threads; without this, two env writers could
/// interleave a fake read. Poisoning-tolerant: a prior panic still yields.
static ENV_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn lock_process_env() -> std::sync::MutexGuard<'static, ()> {
    ENV_SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Production adapter against a Rust fake executable plus the sandbox-only
/// tripwire. Both phases run sequentially in ONE test: only this leg touches
/// PATH and the fake env inputs, so no inter-test race exists. The fake is a
/// checked-in Rust binary addressed by absolute path, copied under a sandbox
/// name; NOTHING executable is committed by the test and no shell runs at any
/// point. The sandbox-only PATH has no fallback: a missing fake refuses
/// closed instead of reaching a real owner.
#[test]
fn production_adapter_fake_executable_forwards_and_sandbox_only_path_refuses() {
    let _env = lock_process_env();
    let fake_bin =
        std::path::PathBuf::from(env!("CARGO_BIN_EXE_fake-rch-reclaim-owner"));
    let sandbox = std::env::temp_dir().join(format!(
        "contabo-reclaim-fake-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&sandbox);
    std::fs::create_dir_all(&sandbox).expect("sandbox");
    let empty = sandbox.join("empty");
    std::fs::create_dir_all(&empty).expect("empty dir");
    let fake_path = sandbox.join(OWNER_BINARY);
    std::fs::copy(&fake_bin, &fake_path).expect("stage fake under owner name");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&fake_path)
            .expect("fake metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&fake_path, permissions).expect("fake executable");
    }

    struct RestoreEnv {
        path: Option<std::ffi::OsString>,
        response: Option<std::ffi::OsString>,
        argv_log: Option<std::ffi::OsString>,
        fake_exit: Option<std::ffi::OsString>,
    }
    impl Drop for RestoreEnv {
        fn drop(&mut self) {
            for (key, value) in [
                ("PATH", &self.path),
                ("FAKE_OWNER_RESPONSE", &self.response),
                ("FAKE_ARGV_LOG", &self.argv_log),
                ("FAKE_OWNER_EXIT", &self.fake_exit),
            ] {
                match value {
                    Some(previous) => std::env::set_var(key, previous),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
    let _guard = RestoreEnv {
        path: std::env::var_os("PATH"),
        response: std::env::var_os("FAKE_OWNER_RESPONSE"),
        argv_log: std::env::var_os("FAKE_ARGV_LOG"),
        fake_exit: std::env::var_os("FAKE_OWNER_EXIT"),
    };

    let argv_log = sandbox.join("argv.log");
    let response_body = owner_response("req-fake", "REPORT", "SUCCEEDED", 0);
    let response_text = String::from_utf8(response_body.clone()).expect("response utf8");
    std::env::set_var("FAKE_OWNER_RESPONSE", &response_text);
    std::env::set_var("FAKE_ARGV_LOG", &argv_log);
    std::env::remove_var("FAKE_OWNER_EXIT");
    // Sandbox-only PATH: the fake name resolves here or nowhere.
    std::env::set_var("PATH", &sandbox);

    let runtime = test_runtime();
    let recovery = recovery_cx_for(&runtime);
    let forwarded = runtime
        .block_on(async {
            let cx = ambient_cx();
            consume(&cx, &recovery, "req-fake", OwnerMode::Report).await
        })
        .expect("fake owner forward");
    assert_eq!(forwarded.raw_stdout, response_body);
    assert_eq!(forwarded.exit_code, 0);
    assert_eq!(forwarded.request_id, "req-fake");
    assert!(!forwarded.recovered_from_status);
    let logged = std::fs::read_to_string(&argv_log).expect("argv log");
    assert_eq!(logged, "report\nreq-fake\n");

    // Tripwire: with no owner reachable and no fallback on PATH, the
    // production adapter must refuse a NAMED spawn failure. Any suite change
    // that shells the real owner trips here instead of touching production.
    std::env::set_var("PATH", &empty);
    let runtime = test_runtime();
    let recovery = recovery_cx_for(&runtime);
    let result = runtime.block_on(async {
        let cx = ambient_cx();
        consume(&cx, &recovery, "req-fake", OwnerMode::Report).await
    });
    let error = result.expect_err("sandbox-only PATH without fake must refuse spawn");
    assert_eq!(error.reason(), OwnerMachineryReason::Spawn);
    assert!(error.detail().contains(OWNER_BINARY));

    std::fs::remove_dir_all(&sandbox).expect("sandbox cleanup");
}

type FakeFuture = Pin<Box<dyn Future<Output = Result<OwnerProcessOutput, ConsumerError>> + Send>>;

fn boxed_pending() -> FakeFuture {
    Box::pin(std::future::pending())
}


fn boxed_ready(result: Result<OwnerProcessOutput, ConsumerError>) -> FakeFuture {
    Box::pin(std::future::ready(result))
}

#[test]
fn top_error_coupling_negatives_refuse() {
    // Each pair below breaks exactly one coupling: error presence for
    // Succeeded, error kind for Partial/BoundExceeded/Cancelled, error kind
    // sets for Error exits, the release marker for preserved kinds, the
    // backing worker for worker-derived tops, and the retired
    // machinery-code 124 shape. STATUS stays unmeasurable throughout.
    let cases: Vec<(&str, u8, Value)> = vec![
        ("SUCCEEDED", 0u8, error_object("WORKER_RECLAIM", "WORKER_RECLAIM")),
        ("PARTIAL", 1u8, Value::Null),
        (
            "PARTIAL",
            1u8,
            error_object("TRANSPORT", "TRANSPORT"),
        ),
        // A WORKER_RECLAIM top on a SUCCEEDED worker row: nothing sources it.
        (
            "PARTIAL",
            1u8,
            error_object("WORKER_RECLAIM", "WORKER_RECLAIM"),
        ),
        ("BOUND_EXCEEDED", 124u8, Value::Null),
        ("CANCELLED", 78u8, Value::Null),
        ("ERROR", 77u8, error_object("WORKER_RECLAIM", "WORKER_RECLAIM")),
        ("ERROR", 78u8, error_object("CANCELLED", "OWNER_CANCELLED")),
        // A preserved Cancelled kind without the fold marker.
        (
            "ERROR",
            78u8,
            error_object("WORKER_RECLAIM", "WORKER_RECLAIM"),
        ),
        ("ERROR", 124u8, error_object("OWNER_MACHINERY", "SOME_OTHER_CODE")),
        // The retired shape: a worker-level restore code at the top level.
        (
            "ERROR",
            124u8,
            error_object("OWNER_MACHINERY", "ADMISSION_RESTORE_FAILED"),
        ),
        // Worker-severity tops on SUCCEEDED rows: no compatible backing row.
        // A Cancelled top needs a Cancelled worker, a BoundExceeded top a
        // BoundExceeded worker, and Machinery/Transport tops an Error worker.
        (
            "CANCELLED",
            78u8,
            error_object("CANCELLED", "OWNER_CANCELLED"),
        ),
        (
            "BOUND_EXCEEDED",
            124u8,
            error_object("BOUND_EXCEEDED", "BOUND_EXCEEDED"),
        ),
        (
            "ERROR",
            78u8,
            error_object("OWNER_MACHINERY", "DURABLE_COMPLETE_FAILED"),
        ),
        (
            "ERROR",
            78u8,
            error_object("TRANSPORT", "TRANSPORT"),
        ),
    ];
    for (outcome, exit, error) in cases {
        let body = if error.is_null() {
            owner_response("req-toperr", "REPORT", outcome, exit)
        } else {
            owner_response_with_error("req-toperr", "REPORT", outcome, exit, error)
        };
        let runtime = test_runtime();
        let recovery = recovery_cx_for(&runtime);
        let result = runtime.block_on(async {
            let cx = ambient_cx();
            consume_with(
                &cx,
                &recovery,
                "req-toperr",
                OwnerMode::Report,
                move |_task_cx, args| match args.first().map(String::as_str) {
                    Some("status") => {
                        std::future::ready(Ok(process_output(b"not-json".to_vec(), 0)))
                    }
                    _ => std::future::ready(Ok(process_output(body.clone(), exit))),
                },
            )
            .await
        });
        let refusal = result.expect_err("top error coupling violation must refuse");
        assert_eq!(refusal.reason(), OwnerMachineryReason::RecoveryNeeded);
    }
}

#[test]
fn release_folded_tops_forward_only_with_marker_and_backing() {
    // Exact release-failure shapes (owner operation_lock.rs
    // apply_release_failure): the prior error is preserved and the marker
    // appended while exit folds to 78 (or keeps a prior 124) and outcome
    // goes Error. Worker-severity priors need a compatible backing row;
    // pre-roster priors (empty workers) need none. A Lease prior never
    // survives the fold (no lease to release) and is a refuse-case below.
    let forwards: Vec<Vec<u8>> = vec![
        // Worker-derived prior: a Blocked worker still carries the error.
        owner_response_release_folded(
            "req-rel",
            Some("BLOCKED"),
            "WORKER_RECLAIM",
            "WORKER_RECLAIM",
        ),
        // Cancelled prior on a Cancelled worker (exact compatibility).
        owner_response_release_folded("req-rel", Some("CANCELLED"), "CANCELLED", "OWNER_CANCELLED"),
        // Pre-roster prior: empty workers stay admissible.
        owner_response_release_folded(
            "req-rel",
            None,
            "ROSTER_UNMEASURABLE",
            "ROSTER_UNMEASURABLE",
        ),
    ];
    for raw in forwards {
        let forwarded = consume_fake(
            "req-rel",
            OwnerMode::Report,
            Ok(process_output(raw.clone(), 78)),
        )
        .unwrap_or_else(|_| panic!("release-folded top forwards"));
        assert_eq!(forwarded.raw_stdout, raw);
        assert_eq!(forwarded.exit_code, 78);
    }
    // Exact Error/124 release shape: a prior bound exit keeps 124 with the
    // preserved BoundExceeded error plus the marker, backed by a
    // BoundExceeded worker.
    let mut bound: Value =
        serde_json::from_slice(&owner_response("req-rel124", "REPORT", "ERROR", 124))
            .expect("response");
    bound["workers"][0]["outcome"] = Value::String("BOUND_EXCEEDED".to_owned());
    bound["workers"][0]["error"] = error_object("BOUND_EXCEEDED", "BOUND_EXCEEDED");
    bound["error"] = serde_json::json!({
        "kind": "BOUND_EXCEEDED",
        "code": "BOUND_EXCEEDED",
        "message": "fixture",
        "evidence": ["OWNER_LOCK_RELEASE_FAILED message=fixture evidence=[]"]
    });
    let bound_raw = serde_json::to_vec(&bound).expect("response JSON");
    let forwarded = consume_fake(
        "req-rel124",
        OwnerMode::Report,
        Ok(process_output(bound_raw.clone(), 124)),
    )
    .expect("bound release-folded top forwards");
    assert_eq!(forwarded.raw_stdout, bound_raw);
    assert_eq!(forwarded.exit_code, 124);
}

#[test]
fn release_created_machinery_error_forwards_without_backing() {
    // The ONE exemption: a prior success (no top error, visited workers)
    // failing at lock release gets a FRESH OwnerMachinery error from the
    // release constructor (code OWNER_LOCK_RELEASE_FAILED) that no worker
    // sources. Kind+code identify the constructor exactly; any other
    // nonempty unmatched kind still refuses.
    let mut body: Value =
        serde_json::from_slice(&owner_response("req-relexc", "REPORT", "ERROR", 78))
            .expect("response");
    body["error"] = serde_json::json!({
        "kind": "OWNER_MACHINERY",
        "code": "OWNER_LOCK_RELEASE_FAILED",
        "message": "unlock OS owner lock fixture: fixture",
        "evidence": ["nonce=1 record=/fixture/record.json"]
    });
    let raw = serde_json::to_vec(&body).expect("response JSON");
    let forwarded = consume_fake(
        "req-relexc",
        OwnerMode::Report,
        Ok(process_output(raw.clone(), 78)),
    )
    .expect("release-created machinery error forwards");
    assert_eq!(forwarded.raw_stdout, raw);
    assert_eq!(forwarded.exit_code, 78);
}

#[test]
fn release_fold_and_coupling_gaps_refuse() {
    // Each body breaks exactly one exactness clause while keeping the rest:
    // a preserved kind without the marker; a marked top with no compatible
    // row (SUCCEEDED worker, empty workers); a bound top without the
    // marker; a marked 77 (the fold always leaves 77); worker-severity tops
    // with no rows at all (no owner seed builds them). STATUS stays
    // unmeasurable throughout.
    let mut unmarked: Value = serde_json::from_slice(&owner_response_release_folded(
        "req-rn",
        Some("BLOCKED"),
        "WORKER_RECLAIM",
        "WORKER_RECLAIM",
    ))
    .expect("response");
    unmarked["error"]["evidence"] = Value::Array(Vec::new());
    // Worker-severity tops with no rows at all: no owner seed constructor
    // builds them, so empty workers cannot be pre-worker shapes for them.
    let empty_reclaim: Value = serde_json::from_slice(&owner_response_empty_workers(
        "req-rn",
        "REPORT",
        "PARTIAL",
        1,
        error_object("WORKER_RECLAIM", "WORKER_RECLAIM"),
    ))
    .expect("response");
    let empty_bound: Value = serde_json::from_slice(&owner_response_empty_workers(
        "req-rn",
        "REPORT",
        "ERROR",
        124,
        error_object("BOUND_EXCEEDED", "BOUND_EXCEEDED"),
    ))
    .expect("response");
    let mut marked_succeeded: Value = serde_json::from_slice(&owner_response_release_folded(
        "req-rn",
        Some("BLOCKED"),
        "WORKER_RECLAIM",
        "WORKER_RECLAIM",
    ))
    .expect("response");
    marked_succeeded["workers"][0]["outcome"] = Value::String("SUCCEEDED".to_owned());
    marked_succeeded["workers"][0]["error"] = Value::Null;
    let mut marked_empty: Value = serde_json::from_slice(&owner_response_release_folded(
        "req-rn",
        Some("BLOCKED"),
        "WORKER_RECLAIM",
        "WORKER_RECLAIM",
    ))
    .expect("response");
    marked_empty["workers"] = Value::Array(Vec::new());
    let mut bound_unmarked: Value =
        serde_json::from_slice(&owner_response("req-rn", "REPORT", "ERROR", 124))
            .expect("response");
    bound_unmarked["error"] = error_object("BOUND_EXCEEDED", "BOUND_EXCEEDED");
    let mut marked_77: Value = serde_json::from_slice(&owner_response_empty_workers(
        "req-rn",
        "REPORT",
        "ERROR",
        77,
        error_object("ROSTER_UNMEASURABLE", "ROSTER_UNMEASURABLE"),
    ))
    .expect("response");
    marked_77["error"]["evidence"] =
        serde_json::json!(["OWNER_LOCK_RELEASE_FAILED message=fixture evidence=[]"]);
    // Forged markers: the needle as a prose substring and the prefix without
    // the later ` evidence=` separator. Both sit on an otherwise-valid
    // 78-preserved shape, so only the shape predicate refuses them.
    let mut forged_substring: Value = serde_json::from_slice(&owner_response_release_folded(
        "req-rn",
        Some("BLOCKED"),
        "WORKER_RECLAIM",
        "WORKER_RECLAIM",
    ))
    .expect("response");
    forged_substring["error"]["evidence"] =
        serde_json::json!(["note: OWNER_LOCK_RELEASE_FAILED happened (see log)"]);
    let mut prefix_no_separator: Value = serde_json::from_slice(&owner_response_release_folded(
        "req-rn",
        Some("BLOCKED"),
        "WORKER_RECLAIM",
        "WORKER_RECLAIM",
    ))
    .expect("response");
    prefix_no_separator["error"]["evidence"] =
        serde_json::json!(["OWNER_LOCK_RELEASE_FAILED message=fixture"]);
    for (name, body) in [
        ("unmarked-preserved-kind", unmarked),
        ("marked-top-succeeded-worker", marked_succeeded),
        ("marked-top-empty-workers", marked_empty),
        ("empty-reclaim-top-no-rows", empty_reclaim),
        ("empty-bound-top-no-rows", empty_bound),
        ("bound-top-without-marker", bound_unmarked),
        ("marked-77", marked_77),
        ("forged-marker-substring", forged_substring),
        ("marker-prefix-without-separator", prefix_no_separator),
    ] {
        let raw = serde_json::to_vec(&body).expect("response JSON");
        let exit = body["exit_code"].as_u64().expect("exit code") as u8;
        let runtime = test_runtime();
        let recovery = recovery_cx_for(&runtime);
        let result = runtime.block_on(async {
            let cx = ambient_cx();
            consume_with(
                &cx,
                &recovery,
                "req-rn",
                OwnerMode::Report,
                move |_task_cx, args| match args.first().map(String::as_str) {
                    Some("status") => {
                        std::future::ready(Ok(process_output(b"not-json".to_vec(), 0)))
                    }
                    _ => std::future::ready(Ok(process_output(raw.clone(), exit))),
                },
            )
            .await
        });
        let refusal = result.expect_err("release-fold gap must refuse");
        assert_eq!(
            refusal.reason(),
            OwnerMachineryReason::RecoveryNeeded,
            "gap {name} must refuse"
        );
    }
}

#[test]
fn malformed_worker_relations_refuse() {
    // NotVisited carrying a remote answer or an error; a succeeded worker
    // carrying an error; a succeeded worker wrapping a failed remote; a
    // Succeeded top response smuggling a worker error past the fold.
    let clean_succeeded = owner_response("req-worker", "REPORT", "SUCCEEDED", 0);
    let mut not_visited_remote =
        serde_json::from_slice::<Value>(&clean_succeeded).expect("response");
    not_visited_remote["workers"][0]["outcome"] = Value::String("NOT_VISITED".to_owned());
    not_visited_remote["workers"][0]["remote"] = serde_json::json!({
        "schema": OWNER_SCHEMA,
        "request_id": "req-worker",
        "mode": "REPORT",
        "outcome": "SUCCEEDED",
        "owner_root": "/fixture/owner-root",
        "idle_threshold_secs": 86400,
        "pressure_mode": "NORMAL",
        "candidates": [],
        "effects": [],
        "error": null,
        "exit_code": 0
    });
    let mut not_visited_error =
        serde_json::from_slice::<Value>(&clean_succeeded).expect("response");
    not_visited_error["workers"][0]["outcome"] = Value::String("NOT_VISITED".to_owned());
    not_visited_error["workers"][0]["error"] =
        error_object("WORKER_RECLAIM", "WORKER_RECLAIM");
    let mut succeeded_error =
        serde_json::from_slice::<Value>(&clean_succeeded).expect("response");
    succeeded_error["workers"][0]["error"] =
        error_object("WORKER_RECLAIM", "WORKER_RECLAIM");
    let mut succeeded_failed_remote =
        serde_json::from_slice::<Value>(&clean_succeeded).expect("response");
    succeeded_failed_remote["workers"][0]["remote"] = serde_json::json!({
        "schema": OWNER_SCHEMA,
        "request_id": "req-worker",
        "mode": "REPORT",
        "outcome": "ERROR",
        "owner_root": "/fixture/owner-root",
        "idle_threshold_secs": 86400,
        "pressure_mode": "NORMAL",
        "candidates": [],
        "effects": [],
        "error": {"kind": "WORKER_RECLAIM", "code": "WORKER_RECLAIM", "message": "fixture", "evidence": []},
        "exit_code": 1
    });
    let mut top_succeeded_worker_error =
        serde_json::from_slice::<Value>(&clean_succeeded).expect("response");
    top_succeeded_worker_error["workers"][0]["error"] =
        error_object("WORKER_RECLAIM", "WORKER_RECLAIM");
    for (name, body) in [
        ("not-visited-remote", not_visited_remote),
        ("not-visited-error", not_visited_error),
        ("succeeded-error", succeeded_error),
        ("succeeded-failed-remote", succeeded_failed_remote),
        ("top-succeeded-worker-error", top_succeeded_worker_error),
    ] {
        let raw = serde_json::to_vec(&body).expect("response JSON");
        let runtime = test_runtime();
        let recovery = recovery_cx_for(&runtime);
        let result = runtime.block_on(async {
            let cx = ambient_cx();
            consume_with(
                &cx,
                &recovery,
                "req-worker",
                OwnerMode::Report,
                move |_task_cx, args| match args.first().map(String::as_str) {
                    Some("status") => {
                        std::future::ready(Ok(process_output(b"not-json".to_vec(), 0)))
                    }
                    _ => std::future::ready(Ok(process_output(raw.clone(), 0))),
                },
            )
            .await
        });
        let refusal = match result {
            Err(error) => error,
            Ok(_) => panic!("malformed worker relation {name} must refuse"),
        };
        assert_eq!(refusal.reason(), OwnerMachineryReason::RecoveryNeeded);
    }
}

/// End-to-end proof through the real binary: the checked-in binary under a
/// sandbox-only PATH answers fixed argv with canned bytes, and the produced
/// `contabo-reclaim` process forwards exactly those bytes with the owner's
/// exit. No shell at any point; the library adapter leg stays alongside.
#[test]
fn main_binary_end_to_end_forwards_fake_owner_stdout_and_exit() {
    let fake_bin =
        std::path::PathBuf::from(env!("CARGO_BIN_EXE_fake-rch-reclaim-owner"));
    let real_bin =
        std::path::PathBuf::from(env!("CARGO_BIN_EXE_contabo-reclaim"));
    let sandbox = std::env::temp_dir().join(format!(
        "contabo-reclaim-e2e-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&sandbox);
    std::fs::create_dir_all(&sandbox).expect("sandbox");
    let empty = sandbox.join("empty");
    std::fs::create_dir_all(&empty).expect("empty dir");
    let fake_path = sandbox.join(OWNER_BINARY);
    let _env = lock_process_env();
    std::fs::copy(&fake_bin, &fake_path).expect("stage fake under owner name");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&fake_path)
            .expect("fake metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&fake_path, permissions).expect("fake executable");
    }

    let saved: Vec<(&str, Option<std::ffi::OsString>)> = ["PATH", "FAKE_OWNER_RESPONSE", "FAKE_ARGV_LOG", "FAKE_OWNER_EXIT"]
        .iter()
        .map(|key| (*key, std::env::var_os(key)))
        .collect();
    let restore = || {
        for (key, value) in &saved {
            match value {
                Some(previous) => std::env::set_var(key, previous),
                None => std::env::remove_var(key),
            }
        }
    };

    let argv_log = sandbox.join("argv.log");
    let response_body = owner_response("req-e2e", "REPORT", "SUCCEEDED", 0);
    std::env::set_var(
        "FAKE_OWNER_RESPONSE",
        String::from_utf8(response_body.clone()).expect("response utf8"),
    );
    std::env::set_var("FAKE_ARGV_LOG", &argv_log);
    std::env::remove_var("FAKE_OWNER_EXIT");
    // Sandbox-only PATH: the owner name resolves here or nowhere.
    std::env::set_var("PATH", &sandbox);
    let output = std::process::Command::new(&real_bin)
        .args(["--request-id", "req-e2e", "report"])
        .output()
        .expect("spawn real binary");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, response_body);
    let logged = std::fs::read_to_string(&argv_log).expect("argv log");
    assert_eq!(logged, "report\nreq-e2e\n");

    let partial_body = owner_response_partial_backed("req-e2e", "REPORT");
    std::env::set_var(
        "FAKE_OWNER_RESPONSE",
        String::from_utf8(partial_body.clone()).expect("response utf8"),
    );
    std::env::set_var("FAKE_OWNER_EXIT", "1");
    let failed = std::process::Command::new(&real_bin)
        .args(["--request-id", "req-e2e", "report"])
        .output()
        .expect("spawn real binary");
    assert_eq!(failed.status.code(), Some(1));
    assert_eq!(failed.stdout, partial_body);
    std::env::remove_var("FAKE_OWNER_EXIT");

    // Tripwire phase: no owner reachable and no fallback — the real binary
    // must fail closed with the named spawn refusal on stderr.
    std::env::set_var("PATH", &empty);
    let denied = std::process::Command::new(&real_bin)
        .args(["--request-id", "req-e2e", "report"])
        .output()
        .expect("spawn real binary");
    assert_eq!(denied.status.code(), Some(OWNER_MACHINERY_EXIT as i32));
    let stderr = String::from_utf8_lossy(&denied.stderr);
    assert!(
        stderr.contains("SPAWN"),
        "tripwire names the spawn refusal, got: {stderr}"
    );

    restore();
    std::fs::remove_dir_all(&sandbox).expect("sandbox cleanup");
}

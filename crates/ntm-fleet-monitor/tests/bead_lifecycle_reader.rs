use asupersync::runtime::RuntimeBuilder;
use asupersync::Cx;
use ntm_fleet_monitor::bead_lifecycle::reader::{
    LifecycleCursor, LifecycleReader, ReadOutcome, ReaderError, WaitOutcome, MAX_WAIT,
};
use serde_json::json;
use std::fs;
use std::future::Future;
use std::path::Path;
use std::time::Duration;
use tempfile::tempdir;

fn row(bead: &str, event: &str, id: &str) -> String {
    json!({
        "schema": "omp.bead.lifecycle.v1",
        "event": event,
        "status": event,
        "bead": bead,
        "repo": "/repo/omp-orchestrator",
        "session": "omp-orchestrator",
        "pane": "%7",
        "packet_digest": "sha256:test",
        "objective": "wake on lifecycle row",
        "idempotency_key": id,
        "freshness": {
            "observed_at_ms": 100,
            "now_ms": 100,
            "max_age_ms": 0,
            "age_ms": 0,
            "within_window": true
        },
        "invoker": "SCHEDULED",
        "evidence": {"source": "test"}
    })
    .to_string()
}

fn write_rows(path: &Path, rows: &[String]) {
    fs::write(path, format!("{}\n", rows.join("\n"))).unwrap();
}

fn reader(ledger: &Path, cursor: &Path) -> LifecycleReader {
    LifecycleReader::new(ledger, cursor, ["bead-a"])
        .unwrap()
        .with_poll_interval(Duration::from_millis(2))
        .unwrap()
}

fn run_runtime<F, T>(future: F) -> T
where
    F: Future<Output = T>,
{
    RuntimeBuilder::current_thread()
        .build()
        .expect("current-thread runtime")
        .block_on(future)
}

#[test]
fn rows_are_filtered_and_cursor_survives_restart_without_redelivery() {
    let temp = tempdir().unwrap();
    let ledger_path = temp.path().join("lifecycle.jsonl");
    let cursor_path = temp.path().join("lifecycle.cursor");
    write_rows(
        &ledger_path,
        &[
            row("bead-a", "receiver_verified", "receiver-a"),
            row("bead-b", "graded_pass", "grade-b"),
        ],
    );

    let mut first = reader(&ledger_path, &cursor_path);
    let outcome = first.read_available().unwrap();
    let ReadOutcome::Rows { cursor, rows } = outcome else {
        panic!("matching row must be readable");
    };
    assert_eq!(cursor, LifecycleCursor::new(2));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].bead, "bead-a");
    assert_eq!(rows[0].event, "receiver_verified");
    assert_eq!(fs::read_to_string(&cursor_path).unwrap(), "2\n");

    let mut restarted = reader(&ledger_path, &cursor_path);
    match restarted.read_available().unwrap() {
        ReadOutcome::Empty { cursor } => assert_eq!(cursor.value(), 2),
        other => panic!("expected persisted cursor, got {other:?}"),
    }
}

#[test]
fn timeout_is_typed_and_distinct_from_empty_scan() {
    let temp = tempdir().unwrap();
    let ledger_path = temp.path().join("missing-lifecycle.jsonl");
    let cursor_path = temp.path().join("lifecycle.cursor");
    let mut reader = reader(&ledger_path, &cursor_path);
    assert!(matches!(
        reader.read_available().unwrap(),
        ReadOutcome::Empty {
            cursor: LifecycleCursor::ZERO
        }
    ));

    let outcome = run_runtime(async {
        let cx = Cx::current().expect("runtime Cx");
        LifecycleReader::wait_for_rows(&cx, &mut reader, Duration::from_millis(12)).await
    })
    .unwrap();
    assert!(matches!(
        outcome,
        WaitOutcome::TimedOut {
            cursor: LifecycleCursor::ZERO,
            ..
        }
    ));
    assert!(outcome.timed_out());
}

#[test]
fn reader_rejects_unbounded_wait_and_empty_bead_scope() {
    let temp = tempdir().unwrap();
    let ledger_path = temp.path().join("lifecycle.jsonl");
    let cursor_path = temp.path().join("lifecycle.cursor");
    assert!(matches!(
        LifecycleReader::new(&ledger_path, &cursor_path, std::iter::empty::<&str>()),
        Err(ReaderError::EmptyBeadSet)
    ));
    let mut reader = reader(&ledger_path, &cursor_path);
    let result = run_runtime(async {
        let cx = Cx::current().expect("runtime Cx");
        LifecycleReader::wait_for_rows(&cx, &mut reader, MAX_WAIT + Duration::from_secs(1)).await
    });
    assert!(matches!(result, Err(ReaderError::WaitExceedsBound { .. })));
}

#[test]
fn malformed_suffix_is_an_error_not_a_silent_empty() {
    let temp = tempdir().unwrap();
    let ledger_path = temp.path().join("lifecycle.jsonl");
    let cursor_path = temp.path().join("lifecycle.cursor");
    fs::write(&ledger_path, "not-json\n").unwrap();
    let mut reader = reader(&ledger_path, &cursor_path);
    assert!(matches!(
        reader.read_available(),
        Err(ReaderError::MalformedRow { line: 1, .. })
    ));
}

#[test]
fn cursor_persistence_is_the_restart_deduplication_guard() {
    let temp = tempdir().unwrap();
    let ledger_path = temp.path().join("lifecycle.jsonl");
    let cursor_path = temp.path().join("lifecycle.cursor");
    write_rows(&ledger_path, &[row("bead-a", "graded_pass", "grade-a")]);
    let mut first = reader(&ledger_path, &cursor_path);
    assert!(matches!(
        first.read_available().unwrap(),
        ReadOutcome::Rows { .. }
    ));
    let mut restarted = reader(&ledger_path, &cursor_path);
    match restarted.read_available().unwrap() {
        ReadOutcome::Empty { cursor } => assert_eq!(cursor.value(), 1),
        other => panic!("expected persisted cursor, got {other:?}"),
    }
}

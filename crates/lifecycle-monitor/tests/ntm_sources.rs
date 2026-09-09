//! 2yrg legs: NTM per-source freshness mapping. Deterministic fixtures over
//! `serde_json::json!` (no subprocess, no daemon); one LIVE/MUTABLE smoke that
//! asserts the restrictive error when no daemon answers and the shipped shape
//! when one does. Population counts are never pinned.

use lifecycle_event::Layer;
use lifecycle_monitor::ntm_sources::{
    gate_ntm_sources, ntm_source_exit_code, parse_ntm_sources, read_live_snapshot, NtmSourceError,
};
use lifecycle_monitor::{LayerState, MonitorError};
use serde_json::{json, Value};

fn snapshot_with(sources: Value) -> Value {
    json!({ "sources": { "sources": sources }, "all_fresh": true })
}

fn good_row() -> Value {
    json!({
        "name": "work_coordination",
        "available": true,
        "fresh": true,
        "reason_code": "health:ok",
        "age_ms": 4222,
        "updated_at": "2026-09-09T12:35:01.058358Z",
    })
}

#[test]
fn all_fresh_fixture_maps_every_row_and_gates_ok() {
    let snapshot = snapshot_with(json!({
        "work_coordination": good_row(),
        "second_source": {
            "name": "second_source",
            "available": true,
            "fresh": true,
            "reason_code": "health:ok",
            "age_ms": 0,
        },
    }));
    let verdicts = parse_ntm_sources(&snapshot).expect("good fixture must parse");
    assert_eq!(
        verdicts.len(),
        2,
        "both rows map, count unpinned by contract"
    );
    for mapped in &verdicts {
        assert_eq!(mapped.verdict.layer, Layer::L4);
        assert_eq!(mapped.verdict.state, LayerState::Progressing);
        assert_eq!(mapped.verdict.row_count, 1);
        assert!(mapped.verdict.fresh);
    }
    let wc = verdicts
        .iter()
        .find(|v| v.source == "work_coordination")
        .expect("key preserved");
    assert_eq!(wc.verdict.last_reason, "health:ok");
    assert_eq!(wc.verdict.age_ms, 4222);
    gate_ntm_sources(&verdicts).expect("all progressing must gate Ok");
    println!("YR2G_GOOD count=2 aggregate=Ok");
}

#[test]
fn live_captured_envelope_shape_maps_progressing() {
    // LIVE_CAPTURED 2026-09-09T12:35:01Z via `ntm --robot-snapshot | jq .sources`.
    // MUTABLE: values are evidence of the shipped shape, not an oracle. The
    // parse is pure, so this leg is deterministic on every host.
    let captured_sources = json!({
        "sources": {
            "work_coordination": {
                "name": "work_coordination",
                "available": true,
                "fresh": true,
                "reason_code": "health:ok",
                "age_ms": 4222,
                "updated_at": "2026-09-09T12:35:01.058358Z",
            },
        },
        "all_fresh": true,
    });
    // The parser takes the FULL snapshot document; `.sources` is nested once more.
    let snapshot = json!({ "sources": captured_sources });
    let verdicts = parse_ntm_sources(&snapshot).expect("live shape must parse");
    assert_eq!(verdicts.len(), 1);
    assert_eq!(verdicts[0].source, "work_coordination");
    assert_eq!(verdicts[0].verdict.state, LayerState::Progressing);
    gate_ntm_sources(&verdicts).expect("live good row must gate Ok");
    println!("YR2G_LIVE_CAPTURED source=work_coordination state=progressing");
}

#[test]
fn unavailable_row_is_refusing_and_names_source() {
    let snapshot = snapshot_with(json!({
        "work_coordination": good_row(),
        "dark_source": {
            "available": false,
            "fresh": false,
            "reason_code": "health:unreachable",
            "age_ms": 99999,
        },
    }));
    let verdicts = parse_ntm_sources(&snapshot).expect("unavailable row still parses");
    let dark = verdicts
        .iter()
        .find(|v| v.source == "dark_source")
        .expect("dark row mapped");
    assert_eq!(dark.verdict.state, LayerState::Refusing);
    let error = gate_ntm_sources(&verdicts).expect_err("one refusing row must veto");
    match &error {
        NtmSourceError::StaleSources { sources } => {
            assert!(
                sources.iter().any(|s| s.contains("dark_source")),
                "source named: {sources:?}"
            );
        }
        other => panic!("expected StaleSources, got {other}"),
    }
    let text = error.to_string();
    assert!(
        text.starts_with("NTM_SOURCE_STALE") && text.contains("dark_source"),
        "diagnostic: {text}"
    );
    println!("YR2G_UNAVAILABLE diagnostic={text}");
}

#[test]
fn stale_row_is_silent_and_vetoes() {
    let snapshot = snapshot_with(json!({
        "work_coordination": {
            "available": true,
            "fresh": false,
            "reason_code": "health:ok",
            "age_ms": 600000,
        },
    }));
    let verdicts = parse_ntm_sources(&snapshot).expect("stale row still parses");
    assert_eq!(verdicts[0].verdict.state, LayerState::Silent);
    assert!(!verdicts[0].verdict.fresh);
    let error = gate_ntm_sources(&verdicts).expect_err("stale row must veto");
    assert!(
        matches!(error, NtmSourceError::StaleSources { .. }),
        "got {error}"
    );
    println!("YR2G_STALE diagnostic={error}");
}

#[test]
fn contradictory_unavailable_but_fresh_is_refusing() {
    // Availability dominates: a source that cannot be reached must never read
    // as merely slow, whatever `fresh` claims.
    let snapshot = snapshot_with(json!({
        "odd_source": {
            "available": false,
            "fresh": true,
            "reason_code": "health:ok",
            "age_ms": 10,
        },
    }));
    let verdicts = parse_ntm_sources(&snapshot).expect("contradictory row parses");
    assert_eq!(verdicts[0].verdict.state, LayerState::Refusing);
    assert!(gate_ntm_sources(&verdicts).is_err());
    println!("YR2G_CONTRADICTORY state=refusing");
}

#[test]
fn missing_fields_are_distinct_restrictive_errors() {
    for (field, row) in [
        (
            "available",
            json!({"fresh": true, "reason_code": "health:ok", "age_ms": 1}),
        ),
        (
            "fresh",
            json!({"available": true, "reason_code": "health:ok", "age_ms": 1}),
        ),
        (
            "reason_code",
            json!({"available": true, "fresh": true, "age_ms": 1}),
        ),
        (
            "age_ms",
            json!({"available": true, "fresh": true, "reason_code": "health:ok"}),
        ),
    ] {
        let snapshot = snapshot_with(json!({ "thin_source": row }));
        let error = parse_ntm_sources(&snapshot).expect_err("missing field must refuse");
        match &error {
            NtmSourceError::MissingField { source, field: got } => {
                assert_eq!(source, "thin_source");
                assert_eq!(got, &field, "field named");
            }
            other => panic!("expected MissingField({field}), got {other}"),
        }
    }
    println!("YR2G_MISSING_FIELDS distinct=4");
}

#[test]
fn wrong_types_are_distinct_restrictive_errors() {
    for (field, row) in [
        (
            "available",
            json!({"available": "yes", "fresh": true, "reason_code": "r", "age_ms": 1}),
        ),
        (
            "fresh",
            json!({"available": true, "fresh": 1, "reason_code": "r", "age_ms": 1}),
        ),
        (
            "reason_code",
            json!({"available": true, "fresh": true, "reason_code": 42, "age_ms": 1}),
        ),
        (
            "age_ms",
            json!({"available": true, "fresh": true, "reason_code": "r", "age_ms": -5}),
        ),
        (
            "age_ms",
            json!({"available": true, "fresh": true, "reason_code": "r", "age_ms": 1.5}),
        ),
        (
            "age_ms",
            json!({"available": true, "fresh": true, "reason_code": "r", "age_ms": "old"}),
        ),
    ] {
        let snapshot = snapshot_with(json!({ "typed_source": row }));
        let error = parse_ntm_sources(&snapshot).expect_err("wrong type must refuse");
        match &error {
            NtmSourceError::WrongType {
                source, field: got, ..
            } => {
                assert_eq!(source, "typed_source");
                assert_eq!(got, &field, "field named");
            }
            other => panic!("expected WrongType({field}), got {other}"),
        }
    }
    println!("YR2G_WRONG_TYPES distinct=6");
}

#[test]
fn empty_reason_is_its_own_error_not_missing() {
    let snapshot = snapshot_with(json!({
        "quiet_source": {"available": true, "fresh": true, "reason_code": "", "age_ms": 1},
    }));
    let error = parse_ntm_sources(&snapshot).expect_err("empty reason must refuse");
    assert!(
        matches!(error, NtmSourceError::EmptyReason { .. }),
        "got {error}"
    );
    assert!(error.to_string().contains("quiet_source"));
    println!("YR2G_EMPTY_REASON diagnostic={error}");
}

#[test]
fn exit_codes_are_pinned_per_class() {
    use std::process::ExitCode;
    assert_eq!(
        ntm_source_exit_code(&NtmSourceError::EmptySources),
        ExitCode::from(2)
    );
    assert_eq!(
        ntm_source_exit_code(&NtmSourceError::SnapshotUnavailable { detail: "x".into() }),
        ExitCode::from(3)
    );
    assert_eq!(
        ntm_source_exit_code(&NtmSourceError::StaleSources {
            sources: vec!["a".into()]
        }),
        ExitCode::from(1)
    );
    assert_eq!(
        ntm_source_exit_code(&NtmSourceError::MissingField {
            source: "s".into(),
            field: "available"
        }),
        ExitCode::from(1)
    );
    println!("YR2G_EXIT_CODES empty=2 unavailable=3 stale=1 missing=1");
}

#[test]
fn empty_sources_object_is_typed_empty_scan() {
    let snapshot = snapshot_with(json!({}));
    let error = parse_ntm_sources(&snapshot).expect_err("empty object is ERROR, never pass");
    assert!(matches!(error, NtmSourceError::EmptySources), "got {error}");
    assert_eq!(
        error.to_string(),
        "NTM_SOURCE_EMPTY_SCAN — empty is ERROR, never a pass"
    );
    // The aggregate is anti-vacuous too: no verdicts in, error out.
    let gate_error = gate_ntm_sources(&[]).expect_err("empty aggregate must refuse");
    assert!(matches!(gate_error, NtmSourceError::EmptySources));
    println!("YR2G_EMPTY_SCAN parse+aggregate");
}

#[test]
fn missing_envelope_is_unmeasured_never_absence() {
    for snapshot in [
        json!({}),
        json!({"sources": {}}),
        json!({"sources": {"all_fresh": true}}),
    ] {
        let error = parse_ntm_sources(&snapshot).expect_err("missing envelope must refuse");
        assert!(
            matches!(error, NtmSourceError::SnapshotUnavailable { .. }),
            "unmeasured, not absence: got {error}"
        );
        assert!(error.to_string().starts_with("NTM_SOURCE_UNAVAILABLE"));
    }
    let malformed = snapshot_with(json!([1, 2]));
    let error = parse_ntm_sources(&malformed).expect_err("array sources must refuse");
    assert!(
        matches!(error, NtmSourceError::MalformedSources { .. }),
        "got {error}"
    );
    println!("YR2G_UNMEASURED missing=3 malformed=1");
}

#[test]
fn shared_l4_gate_adjudicates_the_mapped_verdicts() {
    // The aggregate must not reimplement the L4 rule: prove the shared
    // `gate_freshness_verdict` agrees with our aggregate on both outcomes.
    let good = snapshot_with(json!({ "a": good_row() }));
    let verdicts = parse_ntm_sources(&good).expect("good parses");
    let inner: Vec<_> = verdicts.iter().map(|v| v.verdict.clone()).collect();
    assert!(lifecycle_monitor::gate_freshness_verdict(&inner).is_ok());
    assert!(gate_ntm_sources(&verdicts).is_ok());
    let bad = snapshot_with(json!({
        "a": {"available": true, "fresh": false, "reason_code": "r", "age_ms": 9},
    }));
    let verdicts = parse_ntm_sources(&bad).expect("bad parses");
    let inner: Vec<_> = verdicts.iter().map(|v| v.verdict.clone()).collect();
    assert!(matches!(
        lifecycle_monitor::gate_freshness_verdict(&inner),
        Err(MonitorError::StaleLayers { .. })
    ));
    assert!(gate_ntm_sources(&verdicts).is_err());
    println!("YR2G_SHARED_GATE agree=ok+stale");
}

#[test]
fn live_daemon_smoke_is_labelled_mutable() {
    // LIVE/MUTABLE: exercises the installed `ntm` when a daemon answers and
    // proves the restrictive path when none does. Either arm is exact; neither
    // pins the mutable population. Runs through the production async entry
    // (`Cx`-first `read_live_snapshot`), not around it.
    use asupersync::runtime::RuntimeBuilder;
    let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
    runtime.block_on(async {
        let cx = asupersync::Cx::current().expect("runtime Cx");
        match read_live_snapshot(&cx, "ntm").await {
            Ok(snapshot) => {
                let verdicts = parse_ntm_sources(&snapshot).expect("live shape must parse");
                assert!(
                    !verdicts.is_empty(),
                    "live population must be nonempty to smoke"
                );
                for mapped in &verdicts {
                    assert!(!mapped.source.is_empty());
                    assert!(!mapped.verdict.last_reason.is_empty());
                }
                // The aggregate is COMPUTED, not asserted: a stale live source is
                // evidence, not a test failure of this mapping.
                let aggregate = gate_ntm_sources(&verdicts);
                println!(
                    "YR2G_LIVE live: sources={} aggregate={}",
                    verdicts
                        .iter()
                        .map(|v| v.source.clone())
                        .collect::<Vec<_>>()
                        .join(","),
                    if aggregate.is_ok() {
                        "fresh"
                    } else {
                        "NOT_FRESH"
                    },
                );
            }
            Err(error) => {
                assert!(
                    matches!(error, NtmSourceError::SnapshotUnavailable { .. }),
                    "no daemon must be UNMEASURED, got {error}"
                );
                println!("YR2G_LIVE no-daemon: {error}");
            }
        }
    });
}

const GATE_BIN: &str = env!("CARGO_BIN_EXE_lifecycle-monitor");
const GATE_METRICS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../METRICS.toml");

fn fresh_all_layers_journal(dir: &tempfile::TempDir) -> std::path::PathBuf {
    // The `gate` verb journals ALL metric layers first; the NTM tail only
    // runs when every layer passes. One fresh row per layer (stage values
    // are not validated -- copied from freshness_gate.rs:38).
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_secs();
    let mut body = String::new();
    for layer in ["L0", "L1", "L2", "L3", "L4", "L5"] {
        body.push_str(&format!(
            r#"{{"schema":"omp.lifecycle_event.v1","layer":"{layer}","stage_from":"S1.X","stage_to":"S1.Y","actor":"test","outcome":"emitted","reason_code":"GATE_OK","ts_unix":{now}}}"#
        ));
        body.push('\n');
    }
    let journal = dir.path().join("fresh.jsonl");
    std::fs::write(&journal, body).expect("write fresh journal");
    journal
}
fn gate_with_snapshot(journal: &std::path::Path, extra: &[&str]) -> std::process::Output {
    let mut args = vec![
        "gate".to_owned(),
        "--journal".to_owned(),
        journal.to_str().expect("journal utf8").to_owned(),
        "--metrics".to_owned(),
        GATE_METRICS.to_owned(),
        "--ntm-sources".to_owned(),
    ];
    args.extend(extra.iter().map(|s| s.to_string()));
    std::process::Command::new(GATE_BIN)
        .args(&args)
        .output()
        .expect("lifecycle-monitor gate must launch")
}

fn write_snapshot(dir: &tempfile::TempDir, name: &str, body: &str) -> String {
    let path = dir.path().join(name);
    std::fs::write(&path, body).expect("write snapshot fixture");
    path.to_str().expect("fixture utf8").to_owned()
}

#[test]
fn cli_good_snapshot_file_exits_zero() {
    let dir = tempfile::tempdir().expect("cli fixture");
    let journal = fresh_all_layers_journal(&dir);
    let snapshot = write_snapshot(
        &dir,
        "good.json",
        r#"{"sources":{"sources":{"work_coordination":{"available":true,"fresh":true,"reason_code":"health:ok","age_ms":4222}}}}"#,
    );
    let output = gate_with_snapshot(&journal, &["--snapshot-file", &snapshot]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stdout={stdout} stderr={stderr}");
    assert!(stdout.contains("NTM_SOURCES_OK count=1"), "{stdout}");
    assert!(
        stdout.contains("source=work_coordination state=progressing"),
        "{stdout}"
    );
    println!("YR2G_CLI_GOOD exit=0");
}

#[test]
fn cli_stale_snapshot_file_exits_one_naming_source() {
    let dir = tempfile::tempdir().expect("cli fixture");
    let journal = fresh_all_layers_journal(&dir);
    let snapshot = write_snapshot(
        &dir,
        "stale.json",
        r#"{"sources":{"sources":{"dark_source":{"available":false,"fresh":false,"reason_code":"health:unreachable","age_ms":999}}}}"#,
    );
    let output = gate_with_snapshot(&journal, &["--snapshot-file", &snapshot]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr={stderr}");
    assert!(
        stderr.contains("NTM_SOURCE_STALE") && stderr.contains("dark_source"),
        "{stderr}"
    );
    println!("YR2G_CLI_STALE exit=1");
}

#[test]
fn cli_empty_snapshot_file_exits_two() {
    let dir = tempfile::tempdir().expect("cli fixture");
    let journal = fresh_all_layers_journal(&dir);
    let snapshot = write_snapshot(&dir, "empty.json", r#"{"sources":{"sources":{}}}"#);
    let output = gate_with_snapshot(&journal, &["--snapshot-file", &snapshot]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "stderr={stderr}");
    assert!(stderr.contains("NTM_SOURCE_EMPTY_SCAN"), "{stderr}");
    println!("YR2G_CLI_EMPTY exit=2");
}

#[test]
fn cli_missing_daemon_exits_three_through_production_mapping() {
    // No --snapshot-file: this traverses the production async spawn path
    // (`Cx`-first `read_live_snapshot`) end to end and pins its exit through
    // the typed mapper, not a string re-derivation.
    let dir = tempfile::tempdir().expect("cli fixture");
    let journal = fresh_all_layers_journal(&dir);
    let output = gate_with_snapshot(&journal, &["--ntm-bin", "/nonexistent-2yrg-snapshot"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(3), "stderr={stderr}");
    assert!(stderr.contains("NTM_SOURCE_UNAVAILABLE"), "{stderr}");
    println!("YR2G_CLI_UNAVAILABLE exit=3");
}

#[test]
fn cli_malformed_snapshot_file_exits_one() {
    let dir = tempfile::tempdir().expect("cli fixture");
    let journal = fresh_all_layers_journal(&dir);
    let snapshot = write_snapshot(&dir, "malformed.json", r#"{"sources":{"sources":[1,2]}}"#);
    let output = gate_with_snapshot(&journal, &["--snapshot-file", &snapshot]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr={stderr}");
    assert!(stderr.contains("NTM_SOURCE_MALFORMED_SOURCES"), "{stderr}");
    println!("YR2G_CLI_MALFORMED exit=1");
}

use lifecycle_monitor::load_metrics;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const METRICS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../METRICS.toml");

fn observe(journal: &Path) -> Output {
    let metrics = Path::new(METRICS_PATH);
    Command::new(env!("CARGO_BIN_EXE_lifecycle-monitor"))
        .args([
            "observe",
            "--journal",
            journal.to_str().expect("journal path utf8"),
            "--layer",
            "L4",
            "--metrics",
            metrics.to_str().expect("metrics path utf8"),
        ])
        .output()
        .expect("lifecycle-monitor observe must launch")
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_secs()
}

fn row_with_timestamp(ts: u64) -> String {
    format!(
        r#"{{"schema":"omp.lifecycle_event.v1","layer":"L4","stage_from":"S1.L3","stage_to":"S1.L4","actor":"test","outcome":"emitted","reason_code":"OBSERVE_OK","ts_unix":{ts}}}"#
    )
}

fn age_ms(stdout: &str) -> u64 {
    stdout
        .split_whitespace()
        .find_map(|field| field.strip_prefix("age_ms="))
        .expect("observe output must carry age_ms")
        .parse()
        .expect("age_ms must be numeric")
}

#[test]
fn real_l4_observe_reports_fresh_age_from_metrics() {
    let dir = tempfile::tempdir().expect("fresh journal fixture");
    let journal = dir.path().join("fresh.jsonl");
    std::fs::write(&journal, row_with_timestamp(now_secs())).expect("write fresh row");

    let metrics = load_metrics(Path::new(METRICS_PATH)).expect("METRICS.toml");
    let threshold = metrics
        .iter()
        .find(|spec| spec.layer.as_str() == "L4")
        .expect("L4 metric")
        .stall_after_ms;
    let output = observe(&journal);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("layer=L4 state=progressing"), "{stdout}");
    assert!(stdout.contains("fresh=true"), "{stdout}");
    assert!(age_ms(&stdout) <= threshold, "{stdout}");
    println!("FRESH_SMOKE {stdout}");
}

#[test]
fn real_l4_observe_reports_silent_with_measured_stale_age() {
    let dir = tempfile::tempdir().expect("stale journal fixture");
    let journal = dir.path().join("stale.jsonl");
    std::fs::write(&journal, row_with_timestamp(1)).expect("write stale row");

    let metrics = load_metrics(&metrics_path()).expect("METRICS.toml");
    let threshold = metrics
        .iter()
        .find(|spec| spec.layer.as_str() == "L4")
        .expect("L4 metric")
        .stall_after_ms;
    let output = observe(&journal);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("layer=L4 state=silent"), "{stdout}");
    assert!(stdout.contains("fresh=false"), "{stdout}");
    assert!(age_ms(&stdout) > threshold, "{stdout}");
    println!("STALE_SMOKE {stdout}");
}

#[test]
fn empty_l4_journal_is_typed_refusal() {
    let dir = tempfile::tempdir().expect("empty journal fixture");
    let journal = dir.path().join("empty.jsonl");
    std::fs::write(&journal, "").expect("write empty journal");

    let output = observe(&journal);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("LIFECYCLE_MONITOR_EMPTY_SCAN"), "{stderr}");
    assert!(!stderr.contains("state=silent"), "{stderr}");
    println!("EMPTY_SMOKE {stderr}");
}

#[test]
fn missing_and_malformed_l4_timestamps_are_typed_refusals() {
    let dir = tempfile::tempdir().expect("timestamp fixtures");
    let missing = dir.path().join("missing.jsonl");
    let malformed = dir.path().join("malformed.jsonl");
    let prefix = r#"{"schema":"omp.lifecycle_event.v1","layer":"L4","stage_from":"S1.L3","stage_to":"S1.L4","actor":"test","outcome":"emitted","reason_code":"OBSERVE_OK"}"#;
    std::fs::write(&missing, prefix).expect("write missing timestamp");
    let missing_output = observe(&missing);
    let missing_stderr = String::from_utf8_lossy(&missing_output.stderr);
    assert!(!missing_output.status.success(), "{missing_stderr}");
    assert!(
        missing_stderr.contains("LIFECYCLE_MONITOR_MISSING_TIMESTAMP"),
        "{missing_stderr}"
    );

    let malformed_row = prefix.trim_end_matches('}').to_owned() + r#","ts_unix":"future"}"#;
    std::fs::write(&malformed, malformed_row).expect("write malformed timestamp");
    let malformed_output = observe(&malformed);
    let malformed_stderr = String::from_utf8_lossy(&malformed_output.stderr);
    assert!(!malformed_output.status.success(), "{malformed_stderr}");
    assert!(
        malformed_stderr.contains("LIFECYCLE_MONITOR_MALFORMED_TIMESTAMP"),
        "{malformed_stderr}"
    );
}

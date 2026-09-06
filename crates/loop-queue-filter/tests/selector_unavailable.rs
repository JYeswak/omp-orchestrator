#![forbid(unsafe_code)]

use path_literal_guard::{scan, Verdict, HOST_BV_LITERAL};
use std::process::Command;
use std::{env, fs};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_loop-queue-filter")
}

#[test]
fn path_without_bv_is_typed_refusal_not_success() {
    let output = Command::new(bin())
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .args(["select-graph"])
        .output()
        .expect("spawn select-graph");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let code = output.status.code().unwrap_or(-1);
    assert_ne!(code, 0, "must not succeed without bv; stdout={stdout}");
    assert_eq!(code, 2, "selection stage exit; stderr={stderr}");
    assert!(
        stderr.contains("SELECTOR_UNAVAILABLE program=bv"),
        "typed refusal missing: stdout={stdout} stderr={stderr}"
    );
    assert!(!stdout.contains("AVAILABLE"), "bare success leaked: {stdout}");
    assert!(
        !stderr.to_lowercase().contains("panic"),
        "panic is not a typed refusal: {stderr}"
    );
    assert!(
        !stderr.contains("rank_ready") && !stdout.contains("recency"),
        "silent recency fallback: stdout={stdout} stderr={stderr}"
    );
}

#[test]
fn positive_control_with_bv_writes_receipt_and_exits_zero() {
    let path = env::var("PATH").expect("PATH");
    let receipt = env::temp_dir().join(format!(
        "selector-receipt-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ));
    let output = Command::new(bin())
        .env("PATH", &path)
        .args(["select-graph", "--receipt", receipt.to_str().unwrap()])
        .output()
        .expect("spawn select-graph with bv");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(code, 0, "positive control exit; stderr={stderr} stdout={stdout}");
    assert!(
        stdout.contains("\"status\":\"AVAILABLE\""),
        "receipt missing on stdout: {stdout}"
    );
    assert!(
        stdout.contains("\"selected_by\":\"bv\""),
        "selected_by missing: {stdout}"
    );
    let body = fs::read_to_string(&receipt).unwrap_or_else(|_| panic!("receipt absent at {}", receipt.display()));
    assert!(
        !body.trim().is_empty(),
        "empty receipt at {}",
        receipt.display()
    );
    assert!(body.contains("\"program\":\"bv\""), "{body}");
    let _ = fs::remove_file(&receipt);
}

#[test]
fn empty_scan_set_is_an_error_not_a_pass() {
    let root = env::temp_dir().join(format!("selector-empty-scan-{}", std::process::id()));
    fs::create_dir_all(&root).expect("empty root");
    let report = scan(&root);
    assert!(report.scanned.is_empty(), "expected empty scan set");
    assert_eq!(report.verdict(), Verdict::VacuousError);
    assert!(!report.is_pass(), "empty scan set must not pass");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn hardcoding_host_bv_path_goes_red_under_path_literal_guard() {
    let root = env::temp_dir().join(format!("selector-host-bv-{}", std::process::id()));
    let src = root.join("crates/loop-queue-filter/src");
    fs::create_dir_all(&src).expect("plant tree");
    fs::write(
        src.join("selector.rs"),
        format!("const BV: &str = \"{HOST_BV_LITERAL}\";\n"),
    )
    .expect("plant hardcoded host bv");
    let report = scan(&root);
    assert_eq!(
        report.verdict(),
        Verdict::Violation,
        "hard-coded host bv path must go RED: {report:?}"
    );
    assert!(!report.hits.is_empty(), "mutation produced no hits");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn selector_source_does_not_spell_the_host_path() {
    let source = include_str!("../src/selector.rs");
    let main = include_str!("../src/main.rs");
    assert!(
        !source.contains(HOST_BV_LITERAL),
        "selector.rs must not hard-code the host bv path"
    );
    assert!(
        !main.contains(HOST_BV_LITERAL),
        "main.rs must not hard-code the host bv path"
    );
}

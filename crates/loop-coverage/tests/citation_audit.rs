use std::process::Command;

#[test]
fn content_audit_materializes_declared_absences() {
    let output = Command::new(env!("CARGO_BIN_EXE_loop-coverage"))
        .arg("--audit")
        .output()
        .expect("loop-coverage audit must launch");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "audit should accept declared absences: stdout={stdout} stderr={stderr}"
    );
    assert!(
        stdout.contains("LOOP_COVERAGE_CONTENT_AUDIT status=PARTIAL"),
        "audit must materialize partial citation status: {stdout}"
    );
    assert!(
        stdout.contains("LOOP_COVERAGE_CONTENT_SKIP authority="),
        "audit must list declared skipped authorities: {stdout}"
    );
    assert!(
        stdout.contains("reason=pending-extraction:")
            || stdout.contains("reason=python-forbidden:"),
        "audit must materialize the skip class: {stdout}"
    );
    assert!(stderr.is_empty(), "unexpected audit stderr: {stderr}");
}

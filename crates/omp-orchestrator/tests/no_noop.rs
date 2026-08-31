use std::process::Command;

#[test]
fn missing_repository_is_not_a_successful_supervisor_run() {
    let output = Command::new(env!("CARGO_BIN_EXE_omp-orchestrator"))
        .args(["--once"])
        .env("OMP_REPO", "/definitely/missing/omp-repository")
        .output()
        .expect("run supervisor binary");
    assert!(
        !output.status.success(),
        "a missing repository must not take the no-op success path: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output_text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output_text.contains("repository") || output_text.contains("repo"),
        "failure must name the missing repository boundary: {output_text}"
    );
}

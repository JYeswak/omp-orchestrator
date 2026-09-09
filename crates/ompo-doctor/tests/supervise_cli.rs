use std::process::Command;

#[test]
fn supervise_is_reachable_through_the_canonical_ompo_binary() {
    let output = Command::new(env!("CARGO_BIN_EXE_ompo"))
        .args([
            "supervise",
            "--once",
            "--repo",
            "/definitely/missing/omp-repository",
        ])
        .output()
        .expect("run canonical ompo supervisor");

    assert_eq!(
        output.status.code(),
        Some(2),
        "missing repository must be a typed invocation refusal: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("CONFIG_REFUSED repository target does not exist"),
        "supervise must expose the resident parser's typed refusal: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

use std::process::Command;

#[test]
fn top_level_selftest_reaches_container_predicate() {
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-lane-budget"))
        .arg("--selftest")
        .output()
        .expect("cargo-lane-budget binary runs");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "selftest failed: {stdout}");
    assert!(stdout.contains("SELFTEST PASS container-vs-df"));
}

use std::fs;
use std::process::Command;

fn gate_binary() -> String {
    std::env::var("CARGO_BIN_EXE_orchestration-tick-gate")
        .or_else(|_| std::env::var("CARGO_BIN_EXE_orchestration_tick_gate"))
        .expect("Cargo must expose the orchestration-tick-gate binary")
}

#[test]
fn absent_ledger_is_an_error_with_machine_text() {
    let path = std::env::temp_dir().join(format!(
        "orchestration-tick-gate-missing-{}",
        std::process::id()
    ));
    let _ = fs::remove_file(&path);
    let output = Command::new(gate_binary())
        .args(["--ledger"])
        .arg(&path)
        .output()
        .expect("run orchestration-tick-gate");
    assert!(!output.status.success(), "missing ledger cannot pass");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"status\":\"ERROR\""), "{stdout}");
    assert!(
        stdout.contains("\"law\":\"LEDGER_NOTHING_TO_CHECK\""),
        "{stdout}"
    );
}

#[test]
fn empty_ledger_is_an_error_with_machine_text() {
    let path = std::env::temp_dir().join(format!(
        "orchestration-tick-gate-empty-{}",
        std::process::id()
    ));
    fs::write(&path, b"\n  \n").expect("write empty ledger");
    let output = Command::new(gate_binary())
        .args(["--ledger"])
        .arg(&path)
        .output()
        .expect("run orchestration-tick-gate");
    let _ = fs::remove_file(&path);
    assert!(!output.status.success(), "empty ledger cannot pass");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"status\":\"ERROR\""), "{stdout}");
    assert!(stdout.contains("zero receipt rows"), "{stdout}");
}

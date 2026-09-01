use pane_truth::run_external;
use std::process::Command;
use std::time::Duration;

#[test]
fn mutations_are_named_and_visible() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_pane-truth"));
    command.arg("--selftest");
    let out = run_external(command, Duration::from_secs(20)).unwrap();
    let text = out.stdout;
    assert!(
        text.contains("MUTATION RED two_capture_liveness"),
        "two-capture mutation leg was not exercised"
    );
    assert!(
        text.contains("MUTATION RED busy_markers"),
        "busy-marker mutation leg was not exercised"
    );
    assert!(
        text.contains("MUTATION RED awaiting_input"),
        "input-prompt mutation leg was not exercised"
    );
    println!("{text}");
}

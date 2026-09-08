use std::process::Command;

fn run_rust(args: &[&str]) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-lane-budget"))
        .args(args)
        .output()
        .expect("Rust budget binary runs");
    (
        output.status.code().unwrap_or(1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

/// The old shell oracle cannot exist under the repository's no-.sh rule. Keeping a
/// conditional shell comparison made an empty comparison set print PASS cases=0, which
/// was false differential evidence. This row is explicit and typed until a genuinely
/// independent Rust reference is specified.
#[test]
fn missing_shell_oracle_is_deliberately_not_a_differential() {
    let (exit, stdout) = run_rust(&["--resolve", "--session", "s", "--pane", "1"]);
    assert_eq!(exit, 0, "the live Rust implementation must remain executable");
    assert_eq!(stdout.trim(), "s", "the known-good Rust resolve contract changed");
    println!(
        "DIFFERENTIAL DELIBERATELY_NOT owner=n7mjb dies_when=an independent Rust reference algorithm or a recovered external oracle is specified; no shell path is required"
    );
}

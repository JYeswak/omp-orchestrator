//! ADDRESSABLE surface contract (bead omp-orchestrator-plan-04-7wn9.1).
//!
//! Borrowed shape: franken_markdown tests/cli_contract.rs — the test asserts
//! the CLI's own advertised surface and goes RED when a flag is removed.

#![forbid(unsafe_code)]

use omp_inventory_map::addressable::{self, FLAGS, RUN_COMMAND, SUBCOMMANDS};
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_omp-inventory-map"))
}

#[test]
fn help_exits_zero_with_versioned_json_envelope() {
    let output = bin()
        .arg("--help")
        .output()
        .expect("spawn omp-inventory-map --help");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(
        !stdout.contains("CONFIG_ERROR"),
        "ADDRESSABLE forbids CONFIG_ERROR help: {stdout}"
    );
    addressable::check_addressable(&stdout).expect("live --help is ADDRESSABLE");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("JSON envelope");
    assert_eq!(value["schema_version"], "omp-inventory-map/v1");
    assert_eq!(value["command"], "help");
    assert_eq!(value["status"], "OK");
    assert!(stdout.contains(RUN_COMMAND), "help must name doctor");
    for token in ["doctor", "--repo", "--omp", "--cargo", "--find", "--json", "--help"] {
        assert!(stdout.contains(token), "help omitted {token}");
    }

    let usage = value["data"]["usage"].as_str().expect("usage");
    assert!(usage.contains("doctor"), "{usage}");
}

#[test]
fn advertised_surface_is_present_in_help_and_in_parse() {
    let output = bin()
        .arg("--help")
        .output()
        .expect("spawn --help");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parser = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/main.rs"
    ));
    for token in SUBCOMMANDS.iter().chain(FLAGS.iter()) {
        assert!(
            stdout.contains(token),
            "--help omitted {token}: {stdout}"
        );
        assert!(
            parser.contains(token),
            "src/main.rs omitted advertised token {token}"
        );
    }
}

#[test]
fn config_error_help_fails_the_addressable_leg() {
    let planted = r#"{"schema_version":"omp-inventory-map/v1","command":"doctor","status":"ERROR","data":null,"error":"CONFIG_ERROR unknown argument --help"}"#;
    match addressable::check_addressable(planted) {
        Err(omp_inventory_map::addressable::AddressableError::ConfigErrorHelp) => {}
        other => panic!("ADDRESSABLE census must fail CONFIG_ERROR help, got {other:?}"),
    }
}

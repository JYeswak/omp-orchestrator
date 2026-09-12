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

/// KNOWN-BAD FOR THE ALLOWANCE TABLE ITSELF: an ADJUDICATED collision is
/// silent, and an UNDECLARED one beside it still reddens — message AND exit
/// code, on the real binary.
///
/// Landing the `GuardDecision` row (2026-09-11) proves the allowance works;
/// it does NOT prove the gate still bites, and those are different claims.
/// The planted workspace carries BOTH at once: the exact allowanced pair
/// (contabo-reclaim + omp-host-tool-guard declaring `GuardDecision`) and one
/// undeclared collision. A gate that has gone soft passes this; a gate that
/// only ever refuses fails the silence half.
///
/// The exit code is pinned as well as the text because `2` (REFUSED) and `1`
/// (scan ERROR) are different verdicts that a message-only pin conflates,
/// and because cargo's own `101` matches unrelated breakage.
#[test]
fn an_adjudicated_collision_is_silent_and_an_undeclared_one_still_reddens() {
    let root = std::env::temp_dir().join(format!(
        "omp-inventory-map-allowance-known-bad-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let plant = |krate: &str, body: &str| {
        let src = root.join("crates").join(krate).join("src");
        std::fs::create_dir_all(&src).expect("crate src");
        std::fs::write(
            root.join("crates").join(krate).join("Cargo.toml"),
            format!("[package]\nname = \"{krate}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
        )
        .expect("manifest");
        std::fs::write(src.join("lib.rs"), body).expect("source");
    };

    // KNOWN-GOOD HALF: only the adjudicated pair, in the exact crate set the
    // row names. Any other pair declaring `GuardDecision` is still refused —
    // that is what makes the row a registry and not a blanket pardon.
    plant("contabo-reclaim", "pub enum GuardDecision { Authorized }\n");
    plant("omp-host-tool-guard", "pub enum GuardDecision { Execute }\n");
    let allowed = bin()
        .args(["types", "--repo"])
        .arg(&root)
        .output()
        .expect("spawn types");
    let allowed_stdout = String::from_utf8_lossy(&allowed.stdout).into_owned();
    assert!(
        !allowed_stdout.contains("COLLISION GuardDecision"),
        "the adjudicated pair must not be refused: {allowed_stdout}"
    );

    // KNOWN-BAD HALF: one more collision, declared nowhere.
    plant("planted-left", "pub struct UndeclaredTwin;\n");
    plant("planted-right", "pub struct UndeclaredTwin;\n");
    let refused = bin()
        .args(["types", "--repo"])
        .arg(&root)
        .output()
        .expect("spawn types");
    let stdout = String::from_utf8_lossy(&refused.stdout).into_owned();
    assert_eq!(
        refused.status.code(),
        Some(2),
        "an undeclared collision must exit REFUSED=2, not 0 and not the scan-ERROR 1: {stdout}"
    );
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("JSON envelope");
    assert_eq!(value["status"], "REFUSED");
    let errors = value["data"]["gate_errors"]
        .as_array()
        .expect("gate_errors")
        .iter()
        .filter_map(|e| e.as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    assert!(
        errors
            .iter()
            .any(|e| e.contains("COLLISION UndeclaredTwin")
                && e.contains("planted-left")
                && e.contains("planted-right")),
        "the refusal must name the undeclared collision AND both crates: {errors:?}"
    );
    assert!(
        !errors.iter().any(|e| e.contains("COLLISION GuardDecision")),
        "the adjudicated row must stay silent while the gate bites: {errors:?}"
    );
    std::fs::remove_dir_all(&root).expect("remove planted workspace");
}

//! Observable CLI contract for the `ompo models` OMP RPC projection.

use serde_json::Value;
use std::process::Command;

fn ompo() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ompo"))
}

#[test]
fn models_verb_is_reachable_and_never_falls_through_to_unknown() {
    let output = ompo()
        .args(["models", "--json"])
        .output()
        .expect("ompo runs");
    assert!(
        matches!(output.status.code(), Some(0) | Some(1) | Some(4)),
        "models must return its typed outcome vocabulary: status={:?} stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("UAD_UNKNOWN_VERB"),
        "models dispatched through the unknown-verb tail"
    );

    if output.status.success() {
        let value: Value = serde_json::from_slice(&output.stdout).expect("success is JSON");
        assert_eq!(value["command"], "models");
        assert_eq!(value["status"], "OK");
        assert_eq!(value["data"]["adopted_method"], "get_available_models");
        assert!(value["data"]["count"].as_u64().is_some());
        assert!(value["data"]["models"].as_array().is_some());
    }
}

#[test]
fn capabilities_and_help_expose_the_models_verb() {
    let capabilities = ompo()
        .args(["capabilities", "--json"])
        .output()
        .expect("capabilities runs");
    assert!(capabilities.status.success());
    let value: Value = serde_json::from_slice(&capabilities.stdout).expect("capabilities JSON");
    let verbs = value["data"]["verbs"].as_array().expect("verbs array");
    assert!(verbs.iter().any(|verb| verb.as_str() == Some("models")));

    let help = ompo().arg("--help").output().expect("help runs");
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("models [--json]"));
}


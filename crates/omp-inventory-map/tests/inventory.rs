use omp_inventory_map::{
    InventoryInputs, ProbeState, build_inventory_map, classify_trigger_data, parse_cargo_metadata,
    parse_cli_commands, parse_rpc_slash_commands,
};
use serde_json::json;

fn metadata(names: &[&str]) -> String {
    json!({
        "workspace_root": "/fixture/workspace",
        "packages": names.iter().map(|name| json!({
            "name": name,
            "version": "0.1.0",
            "manifest_path": format!("/fixture/workspace/crates/{name}/Cargo.toml"),
            "targets": [{"name": name.replace('-', "_"), "kind": ["lib"]}],
            "dependencies": []
        })).collect::<Vec<_>>()
    })
    .to_string()
}

#[test]
fn empty_metadata_is_a_hard_error() {
    let result = parse_cargo_metadata("");
    assert!(
        result.is_err(),
        "empty metadata must never become a healthy zero-package map"
    );
}

#[test]
fn unknown_trigger_data_is_not_healthy() {
    assert_eq!(classify_trigger_data(None), ProbeState::Unknown);
    assert_eq!(classify_trigger_data(Some("   ")), ProbeState::Unknown);
    assert_eq!(
        classify_trigger_data(Some("direct:omp --help")),
        ProbeState::Known
    );

    let map = build_inventory_map(InventoryInputs {
        cargo_metadata: metadata(&["omp-inventory-map"]),
        ..InventoryInputs::default()
    })
    .expect("non-empty cargo fixture");
    assert_eq!(map.state, ProbeState::Unknown);
    assert!(map.rows.iter().any(|row| row.status == ProbeState::Unknown));
}

#[test]
fn subprocess_and_no_shell_positive_controls_are_visible() {
    let map = build_inventory_map(InventoryInputs {
        cargo_metadata: metadata(&["subprocess-contract", "no-shell-gate", "omp-inventory-map"]),
        ..InventoryInputs::default()
    })
    .expect("positive-control metadata");
    assert!(
        map.rows
            .iter()
            .any(|row| row.id == "crate:subprocess-contract")
    );
    assert!(map.rows.iter().any(|row| row.id == "crate:no-shell-gate"));
    assert!(
        map.rows
            .iter()
            .any(|row| row.crate_consumes_today == "omp-inventory-map"),
        "the map must contain a real mapped positive control"
    );
}

#[test]
fn cli_positive_control_requires_a_real_commands_block() {
    let help = "omp v18.0.11\n\nCOMMANDS\n  alpha       positive control\n  beta        another command\n\nEXAMPLES\n";
    let parsed = parse_cli_commands(help);
    assert_eq!(parsed.state, ProbeState::Known);
    assert_eq!(parsed.value.expect("commands"), vec!["alpha", "beta"]);
    assert_eq!(
        parse_cli_commands("omp v18.0.11").state,
        ProbeState::Unknown
    );
}

#[test]
fn rpc_slash_command_frame_is_parsed_as_nested_paths() {
    let output = r#"{"type":"available_commands_update","commands":[{"name":"security","subcommands":[{"name":"plan"}]}]}"#;
    let parsed = parse_rpc_slash_commands(output);
    assert_eq!(parsed.state, ProbeState::Known);
    assert_eq!(
        parsed.value.expect("slash commands"),
        vec!["/security", "/security/plan"]
    );
}

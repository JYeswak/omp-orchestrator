use asupersync::runtime::RuntimeBuilder;
use asupersync::types::Budget;
use omp_inventory_map::{
    InventoryInputs, ProbeConfig, ProbeState, SurfaceMapAuditOutcome, audit_surface_map_text,
    build_inventory_map, classify_trigger_data, collect_inventory, parse_cargo_metadata,
    parse_cli_commands, parse_rpc_slash_commands,
};
use omp_inventory_map::census_invariants::{
    CensusInvariantError, CensusInvariantRow, check_census_invariants,
};
use omp_inventory_map::types_inventory::scan_workspace_types;
use serde_json::json;
use std::path::PathBuf;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
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

#[test]
fn surface_map_known_good_accepts_explicit_map_to_none() {
    let map = "[meta]\nrevision = 2\n\n[crates.alpha]\nclassification = \"a\"\nomp_surface = \"none\"\n\n[crates.beta]\nclassification = \"b\"\nomp_surface = \"session\"\n";
    let audit = audit_surface_map_text(map, &metadata(&["alpha", "beta"]))
        .expect("metadata fixture parses");
    assert_eq!(audit.state, ProbeState::Known);
    assert!(audit.is_known());
    assert!(audit.outcomes.is_empty());
}

#[test]
fn surface_map_missing_package_is_undeclared_and_unknown() {
    let map = "[crates.alpha]\nclassification = \"a\"\nomp_surface = \"none\"\n";
    let audit = audit_surface_map_text(map, &metadata(&["alpha", "beta"]))
        .expect("metadata fixture parses");
    assert_eq!(audit.state, ProbeState::Unknown);
    assert!(audit.outcomes.iter().any(|outcome| matches!(
        outcome,
        SurfaceMapAuditOutcome::UndeclaredPackage { package_name } if package_name == "beta"
    )));
}

#[test]
fn surface_map_ghost_is_unknown() {
    let map = "[crates.alpha]\nclassification = \"a\"\nomp_surface = \"none\"\n\n[crates.phantom]\nclassification = \"c\"\nomp_surface = \"collab\"\n";
    let audit =
        audit_surface_map_text(map, &metadata(&["alpha"])).expect("metadata fixture parses");
    assert_eq!(audit.state, ProbeState::Unknown);
    assert!(audit.outcomes.iter().any(|outcome| matches!(
        outcome,
        SurfaceMapAuditOutcome::GhostDeclaration { package_name, .. } if package_name == "phantom"
    )));
}

#[test]
fn malformed_classification_is_typed_and_unknown() {
    let map = "[crates.alpha]\nclassification = \"invalid\"\nomp_surface = \"none\"\n";
    let audit =
        audit_surface_map_text(map, &metadata(&["alpha"])).expect("metadata fixture parses");
    assert_eq!(audit.state, ProbeState::Unknown);
    assert!(audit.outcomes.iter().any(|outcome| matches!(
        outcome,
        SurfaceMapAuditOutcome::InvalidClassification {
            package_name,
            classification,
            ..
        } if package_name == "alpha" && classification == "invalid"
    )));
}

#[test]
fn malformed_row_and_duplicate_declaration_are_typed() {
    let map = "[crates.alpha]\nclassification = \"a\"\n\n[crates.alpha]\nclassification = \"a\"\nomp_surface = \"none\"\n";
    let audit =
        audit_surface_map_text(map, &metadata(&["alpha"])).expect("metadata fixture parses");
    assert_eq!(audit.state, ProbeState::Unknown);
    assert!(audit.outcomes.iter().any(|outcome| matches!(
        outcome,
        SurfaceMapAuditOutcome::DuplicateDeclaration { package_name, .. } if package_name == "alpha"
    )));
    assert!(audit.outcomes.iter().any(|outcome| matches!(
        outcome,
        SurfaceMapAuditOutcome::MalformedRow { package_name, detail, .. }
            if package_name.as_deref() == Some("alpha") && detail.contains("omp_surface")
    )));
}

#[test]
fn hung_omp_probe_becomes_unknown_with_typed_timeout() {
    let root = std::env::temp_dir().join(format!(
        "omp-inventory-map-hung-probe-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("fixture root");
    let fake_omp = root.join("omp-hang");
    std::fs::write(&fake_omp, "#!/bin/sh\nsleep 30\n").expect("hung omp fixture");
    #[cfg(unix)]
    {
        let mut permissions = std::fs::metadata(&fake_omp).expect("fixture metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&fake_omp, permissions).expect("fixture executable");
    }

    let config = ProbeConfig {
        repo_root: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        omp_program: fake_omp,
        cargo_program: PathBuf::from("cargo"),
        find_program: PathBuf::from("find"),
    };
    let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
    let cx = runtime.request_cx_with_budget(Budget::INFINITE);
    let map = runtime
        .block_on(async { collect_inventory(&cx, &config).await })
        .expect("required cargo metadata still succeeds");
    let timed_out: Vec<_> = map
        .probes
        .iter()
        .filter(|probe| probe.detail.contains("TIMEOUT"))
        .collect();
    assert!(!timed_out.is_empty(), "hung omp must produce timeout evidence");
    assert!(
        timed_out
            .iter()
            .all(|probe| probe.state == ProbeState::Unknown),
        "timeout evidence must never be Known: {timed_out:?}"
    );
    assert!(
        timed_out.iter().any(|probe| probe.name == "omp_version"),
        "the hung omp version probe must be named in timeout evidence: {timed_out:?}"
    );
    std::fs::remove_dir_all(root).expect("remove fixture root");
}

#[test]
fn real_scan_set_refuses_orphan_and_blank_contract_then_restores_green() {
    let root = std::env::temp_dir().join(format!(
        "omp-inventory-map-known-bad-{}",
        std::process::id()
    ));
    let anchor = root.join("crates/scan-anchor");
    let orphan = root.join("crates/orphan-specimen");
    std::fs::create_dir_all(anchor.join("src")).expect("anchor source root");
    std::fs::create_dir_all(orphan.join("src")).expect("orphan source root");
    std::fs::write(
        anchor.join("Cargo.toml"),
        "[package]\nname = \"scan-anchor\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("anchor manifest");
    std::fs::write(
        orphan.join("Cargo.toml"),
        "[package]\nname = \"orphan-specimen\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("orphan manifest");
    std::fs::write(anchor.join("src/lib.rs"), "pub struct ScanAnchor;\n")
        .expect("anchor source");
    std::fs::write(orphan.join("src/lib.rs"), "// no public type\n")
        .expect("orphan source");

    let planted_inventory = scan_workspace_types(&root).expect("planted scan set");
    let inventory_errors = planted_inventory
        .check()
        .expect_err("orphan crate must be refused");
    assert!(
        inventory_errors
            .iter()
            .any(|error| error.contains("NAMED-ZERO REQUIRED") && error.contains("orphan-specimen")),
        "orphan refusal must name the crate: {inventory_errors:?}"
    );

    let clean_rows = vec![CensusInvariantRow {
        id: "surface:cli_command:anchor".to_owned(),
        kind: "cli_command".to_owned(),
        must_be_true: vec!["omp --help lists anchor".to_owned()],
        negative_evidence: vec!["a missing COMMANDS block is UNKNOWN for anchor".to_owned()],
        vacuity_mode: None,
        vacuity_reason: None,
        what_it_provides: String::new(),
        inputs: Vec::new(),
    }];
    let mut planted_rows = clean_rows.clone();
    planted_rows.push(CensusInvariantRow {
        id: "surface:cli_command:blank-specimen".to_owned(),
        kind: "cli_command".to_owned(),
        must_be_true: Vec::new(),
        negative_evidence: vec!["blank must_be_true is the planted defect".to_owned()],
        vacuity_mode: None,
        vacuity_reason: None,
        what_it_provides: String::new(),
        inputs: Vec::new(),
    });
    let blank_error = check_census_invariants(&planted_rows)
        .expect_err("blank must_be_true must be refused");
    assert!(
        matches!(
            &blank_error,
            CensusInvariantError::BlankInvariant { id, field }
                if id == "surface:cli_command:blank-specimen" && *field == "must_be_true"
        ),
        "blank refusal must name row and field: {blank_error:?}"
    );
    assert!(
        blank_error.to_string().contains("blank-specimen")
            && blank_error.to_string().contains("must_be_true"),
        "typed blank refusal must carry both identities: {blank_error}"
    );

    std::fs::remove_dir_all(&orphan).expect("remove orphan specimen");
    let restored_inventory = scan_workspace_types(&root).expect("restored scan set");
    assert!(
        !restored_inventory.crates.is_empty(),
        "restored scan must remain non-empty"
    );
    restored_inventory
        .check()
        .expect("removing orphan must restore a green non-empty scan");
    check_census_invariants(&clean_rows)
        .expect("removing blank row must restore a green non-empty census");
    std::fs::remove_dir_all(&anchor).expect("remove final anchor");
    let empty_scan_error = scan_workspace_types(&root)
        .expect_err("an empty real scan set must be an error");
    assert!(
        format!("{empty_scan_error:?}").contains("no member crates"),
        "empty scan refusal must name the missing member set: {empty_scan_error:?}"
    );
    std::fs::remove_dir_all(root).expect("remove specimen workspace");
}

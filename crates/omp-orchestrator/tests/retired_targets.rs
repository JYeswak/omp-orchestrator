#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("crates/<name> -> repository root")
        .to_path_buf()
}

#[test]
fn omp_idle_dispatch_is_retired_from_the_workspace_roster() {
    let root = repo_root();
    let workspace = std::fs::read_to_string(root.join("Cargo.toml")).expect("workspace manifest");
    assert!(
        workspace.contains("exclude = [\"crates/omp-idle-dispatch\"]"),
        "the old auto-dispatch package must be explicitly excluded from the workspace"
    );

    let retired_source =
        std::fs::read_to_string(root.join("crates/omp-idle-dispatch/src/main.rs"))
            .expect("retired source evidence");
    assert!(
        retired_source.contains("\"dispatches\": true"),
        "retirement evidence must name the behavior being removed"
    );

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--no-deps", "--format-version", "1", "--offline"])
        .current_dir(&root)
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "workspace metadata must remain readable: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("cargo metadata JSON");
    let present = metadata["packages"]
        .as_array()
        .expect("metadata packages")
        .iter()
        .any(|package| package["name"] == "omp-idle-dispatch");
    assert!(!present, "retired idle-dispatch must not be a workspace package");
}

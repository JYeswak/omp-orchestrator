use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_crate-soundness-verify"))
}

#[test]
fn top_level_selftest_reaches_planted_known_bads() {
    let output = Command::new(binary()).arg("--selftest").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "top-level selftest failed: {stdout}"
    );
    for leg in [
        "DERIVATION PASS",
        "MUTATION RED derivation_hardcoded_set",
        "MUTATION RED empty_workspace",
        "MUTATION RED missing_forbid",
        "MUTATION RED bounded_child",
    ] {
        assert!(
            stdout.contains(leg),
            "top-level detector missing {leg}: {stdout}"
        );
    }
}

#[test]
fn top_level_entry_point_refuses_real_missing_forbid_fixture() {
    let root = std::env::temp_dir().join(format!(
        "crate-soundness-top-level-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(root.join("crates/planted/src")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers=[\"crates/planted\"]\n",
    )
    .unwrap();
    std::fs::write(
        root.join("crates/planted/Cargo.toml"),
        "[package]\nname=\"planted\"\nversion=\"0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(root.join("crates/planted/src/main.rs"), "fn main() {}\n").unwrap();
    let output = Command::new(binary())
        .env("CRATE_SOUNDNESS_REPO_ROOT", &root)
        .env("CRATE_SOUNDNESS_CARGO_BIN", "/usr/bin/false")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(1),
        "missing_forbid fixture must be RED"
    );
    assert!(
        stdout.contains("RED stage=forbid") && stdout.contains("planted"),
        "top-level gate did not name missing_forbid: {stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}

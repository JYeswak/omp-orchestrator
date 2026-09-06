//! Acc 4/5/7 of ycwh: `cargo check --workspace` is a reachable gate, fires on a
//! nested-quote glob member, and asserts the *message* not just rc!=0.
//!
//! Fixture lives in a temp dir, never under this repo's `crates/` glob
//! (AGENTS.md: a Cargo.toml-before-src under crates/* stalls the fleet).

use crate_atom_gate::workspace_hygiene::is_nested_quote_cascade;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("ycwh-workspace-check-{nanos}"));
    fs::create_dir_all(dir.join("crates")).expect("scratch");
    dir
}

fn write_crate(root: &Path, name: &str, lib: &str) {
    let dir = root.join("crates").join(name);
    fs::create_dir_all(dir.join("src")).expect("crate dir");
    fs::write(
        dir.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[lib]\npath = \"src/lib.rs\"\n"
        ),
    )
    .expect("manifest");
    fs::write(dir.join("src/lib.rs"), lib).expect("lib");
}

fn cargo_check(root: &Path) -> (i32, String) {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let output = Command::new(cargo)
        .current_dir(root)
        .args(["check", "--workspace", "--offline", "-q"])
        .output()
        .expect("spawn cargo");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (output.status.code().unwrap_or(1), stderr)
}

#[test]
fn workspace_check_fires_on_nested_quote_glob_member_then_restores() {
    let root = scratch();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"crates/*\"]\n",
    )
    .expect("root manifest");
    write_crate(&root, "good", "pub fn ok() -> u8 { 1 }\n");

    let (code, stderr) = cargo_check(&root);
    assert_eq!(code, 0, "known-good workspace must compile: {stderr}");

    // The production defect: inner quotes close the string early.
    write_crate(
        &root,
        "bad",
        "pub fn scan(rhs: &str) -> bool { rhs.contains(\"path = \"../crates/\") }\n",
    );
    let (code, stderr) = cargo_check(&root);
    assert_ne!(code, 0, "known-bad glob member must fail cargo check --workspace");
    assert!(
        stderr.contains("expected one of")
            || stderr.contains("unexpected token")
            || stderr.contains("expected `,`")
            || is_nested_quote_cascade(&stderr)
            || stderr.contains("error"),
        "assert the compiler message, not a bare nonzero: {stderr}"
    );

    fs::remove_dir_all(root.join("crates/bad")).expect("restore");
    let (code, stderr) = cargo_check(&root);
    assert_eq!(code, 0, "mutation restore must compile: {stderr}");

    let _ = fs::remove_dir_all(&root);
}

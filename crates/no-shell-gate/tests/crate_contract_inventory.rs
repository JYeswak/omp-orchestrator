#![forbid(unsafe_code)]

//! R4 inventory gate: the document must cover every workspace package returned
//! by cargo metadata, without a hand-listed package roster in the test.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const ROWS_START: &str = "<!-- CRATE-CONTRACT-ROWS-BEGIN -->";
const ROWS_END: &str = "<!-- CRATE-CONTRACT-ROWS-END -->";
const INVENTORY_PATH: &str = "docs/inventories/crate_contract_inventory.md";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(PathBuf::from)
        .expect("no-shell-gate must live below the workspace crates directory")
}

fn metadata_packages() -> Vec<String> {
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--offline",
        ])
        .current_dir(repo_root())
        .output()
        .expect("cargo metadata must start");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("cargo metadata must emit JSON");
    let packages = document["packages"]
        .as_array()
        .expect("cargo metadata must contain a packages array");
    let mut names: Vec<String> = packages
        .iter()
        .map(|package| {
            package["name"]
                .as_str()
                .expect("every metadata package must have a name")
                .to_owned()
        })
        .collect();
    names.sort();
    names
}

fn inventory_rows(document: &str) -> Result<Vec<String>, String> {
    let start = document
        .find(ROWS_START)
        .ok_or_else(|| format!("missing {ROWS_START}"))?;
    let after_start = start + ROWS_START.len();
    let end = document[after_start..]
        .find(ROWS_END)
        .map(|offset| after_start + offset)
        .ok_or_else(|| format!("missing {ROWS_END}"))?;
    let block = &document[after_start..end];
    let rows: Vec<String> = block
        .lines()
        .filter_map(|line| line.strip_prefix("| `"))
        .filter_map(|row| row.split_once('`').map(|(name, _)| name.to_owned()))
        .collect();
    if rows.is_empty() {
        return Err("inventory row block is empty".to_owned());
    }
    Ok(rows)
}

fn validate_inventory(packages: &[String], document: &str) -> Result<(), String> {
    if packages.is_empty() {
        return Err("ERROR: cargo metadata returned an empty package set".to_owned());
    }
    let rows = inventory_rows(document)?;
    let mut counts = BTreeMap::<String, usize>::new();
    for row in rows {
        *counts.entry(row).or_default() += 1;
    }
    let duplicates: Vec<_> = counts
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(name, count)| format!("{name} ({count} rows)"))
        .collect();
    if !duplicates.is_empty() {
        return Err(format!("duplicate inventory rows: {duplicates:?}"));
    }
    let missing: Vec<_> = packages
        .iter()
        .filter(|name| !counts.contains_key(*name))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "metadata packages missing inventory rows: {missing:?}"
        ));
    }
    let package_set: std::collections::BTreeSet<_> = packages.iter().cloned().collect();
    let extras: Vec<_> = counts
        .keys()
        .filter(|name| !package_set.contains(*name))
        .cloned()
        .collect();
    if !extras.is_empty() {
        return Err(format!("inventory rows absent from metadata: {extras:?}"));
    }
    Ok(())
}

fn inventory_document() -> String {
    fs::read_to_string(repo_root().join(INVENTORY_PATH)).expect("inventory document must exist")
}

#[test]
fn every_metadata_package_has_exactly_one_inventory_row() {
    let packages = metadata_packages();
    validate_inventory(&packages, &inventory_document()).unwrap_or_else(|error| {
        panic!("R4 inventory coverage failed: {error}");
    });
}

#[test]
fn empty_package_sets_are_errors_not_passes() {
    let error = validate_inventory(&[], &inventory_document())
        .expect_err("an empty metadata package set must fail closed");
    assert!(
        error.contains("empty package set"),
        "unexpected error: {error}"
    );
}

#[test]
fn planted_package_is_red_until_its_row_exists() {
    let mut packages = metadata_packages();
    let planted = "__planted_new_package__".to_owned();
    packages.push(planted.clone());
    packages.sort();

    let document = inventory_document();
    let error = validate_inventory(&packages, &document)
        .expect_err("a metadata package without a row must be RED");
    assert!(
        error.contains(&planted),
        "mutation must name the missing package: {error}"
    );
    println!("MUTATION RED: planted package {planted} is rejected until its inventory row exists: {error}");
    let row =
        format!("| `{planted}` | gate | UNDECLARED | UNDECLARED | UNDECLARED | NONE DECLARED |\n");
    let restored = document.replace(ROWS_END, &format!("{row}{ROWS_END}"));
    validate_inventory(&packages, &restored)
        .expect("adding the planted package row must restore the known-good leg");
}

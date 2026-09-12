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

/// The coverage leg, and — when it fails — the REPAIR.
///
/// This document declares that its row block "is generated from Cargo metadata"
/// and that "package membership comes from Cargo metadata, never a hand-written
/// roster". Measured 2026-09-12: nothing in the repository generated it.
/// `CRATE-CONTRACT-ROWS` appeared in exactly two places — the document and this
/// file — so the block was hand-maintained by a document that says it is not, and
/// it had drifted to 72 rows against 94 packages. A registry beside a derivable
/// source drifts on a known period; this one had been red long enough that its
/// sibling leg (`planted_package_is_red_until_its_row_exists`) failed too, because
/// that leg's known-good restore runs through the same document.
///
/// So the refusal now CARRIES THE REPAIRED BLOCK, derived by
/// `omp_inventory_map::crate_contract` from each package's manifest and sources
/// under the document's own `CRI-*` rules. Existing rows are preserved
/// byte-for-byte — several carry prose no scan could produce, and regenerating
/// over them would replace a human's measurement with a scanner's.
#[test]
fn every_metadata_package_has_exactly_one_inventory_row() {
    let packages = metadata_packages();
    let document = inventory_document();
    if let Err(error) = validate_inventory(&packages, &document) {
        let repaired = repaired_block(&packages, &document);
        panic!("R4 inventory coverage failed: {error}\n\nREPAIRED BLOCK (paste between the markers):\n{repaired}");
    }
}

/// The derived block this document should contain, with every existing row kept.
fn repaired_block(packages: &[String], document: &str) -> String {
    let start = match document.find(ROWS_START) {
        Some(index) => index + ROWS_START.len(),
        None => return format!("cannot repair: missing {ROWS_START}"),
    };
    let end = match document[start..].find(ROWS_END) {
        Some(offset) => start + offset,
        None => return format!("cannot repair: missing {ROWS_END}"),
    };
    let set: std::collections::BTreeSet<String> = packages.iter().cloned().collect();
    let root = repo_root();
    match omp_inventory_map::crate_contract::merge_block(&document[start..end], &set, &|name| {
        omp_inventory_map::crate_contract::derive_row_from_disk(&root, name)
    }) {
        Ok(block) => block,
        Err(error) => format!("cannot repair: {error:?}"),
    }
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

#![forbid(unsafe_code)]

//! jplf.9: README current-status figures match live authorities.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn live_crate_dirs(root: &Path) -> usize {
    let crates = root.join("crates");
    fs::read_dir(&crates)
        .expect("crates/")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .count()
}

fn live_forbid_manifest(root: &Path) -> usize {
    let crates = root.join("crates");
    fs::read_dir(&crates)
        .expect("crates/")
        .filter_map(|e| e.ok())
        .filter(|e| {
            let toml = e.path().join("Cargo.toml");
            toml.is_file()
                && fs::read_to_string(&toml)
                    .map(|t| t.contains("unsafe_code = \"forbid\""))
                    .unwrap_or(false)
        })
        .count()
}

fn obsolete_unlabelled(text: &str) -> Vec<(usize, String)> {
    let re = regex_lite_patterns();
    let mut hits = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if re.iter().any(|p| line.contains(p.0) && p.1(line))
            && !(line.contains("HISTORICAL")
                || line.contains("PROJECTED")
                || line.contains("NO-CLAIM"))
        {
            hits.push((index + 1, line.to_owned()));
        }
    }
    hits
}

fn regex_lite_patterns() -> Vec<(&'static str, fn(&str) -> bool)> {
    vec![
        ("2 of 20", |_| true),
        ("26 crates", |_| true),
        ("39 subcommands", |_| true),
        ("shipped, verified", |_| true),
        ("currently verified", |_| true),
    ]
}

fn readme_status(text: &str, crates: usize, forbid: usize) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("EMPTY_SCAN: README.md is empty".to_owned());
    }
    let claim = format!("{forbid} of {crates}");
    if !text.contains(&claim) {
        return Err(format!(
            "README_STATUS_DRIFT missing live claim {claim} (NUMBERS.toml crates_forbidding_unsafe + cargo metadata)"
        ));
    }
    let hits = obsolete_unlabelled(text);
    if !hits.is_empty() {
        return Err(format!("README_OBSOLETE_UNLABELLED {hits:?}"));
    }
    Ok(())
}

#[test]
fn live_readme_status_matches_authorities() {
    let root = repo_root();
    let text = fs::read_to_string(root.join("README.md")).expect("README.md");
    let crates = live_crate_dirs(&root);
    let forbid = live_forbid_manifest(&root);
    assert!(crates > 0 && forbid > 0, "EMPTY_SCAN crate census");
    readme_status(&text, crates, forbid).expect("README status");
}

#[test]
fn mutation_of_the_live_forbid_claim_goes_red() {
    let root = repo_root();
    let original = fs::read_to_string(root.join("README.md")).expect("README.md");
    let crates = live_crate_dirs(&root);
    let forbid = live_forbid_manifest(&root);
    readme_status(&original, crates, forbid).expect("known-good");
    let claim = format!("{forbid} of {crates}");
    let mutated = original.replace(&claim, "2 of 20");
    let err = readme_status(&mutated, crates, forbid).expect_err("mutated figure must RED");
    assert!(
        err.contains("README_STATUS_DRIFT") || err.contains("README_OBSOLETE_UNLABELLED"),
        "{err}"
    );
    readme_status(&original, crates, forbid).expect("restore byte-identical");
}

#[test]
fn empty_readme_is_an_error_not_a_pass() {
    let err = readme_status("", 78, 74).expect_err("empty");
    assert!(err.contains("EMPTY_SCAN"), "{err}");
}

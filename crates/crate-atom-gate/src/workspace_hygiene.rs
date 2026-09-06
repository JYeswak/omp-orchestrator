//! Workspace glob-member hygiene. Pure: names in, verdict out.
//!
//! Measured 2026-09-06 (ycwh): `crates/fuzz-build-gate` sat on disk with a nested-quote
//! syntax error, was a `crates/*` glob member, and broke `cargo check --workspace` with
//! 23 cascade errors while `git ls-tree -r HEAD -- crates/fuzz-build-gate` returned 0
//! files. `git ls-files` would have been equally empty until someone staged it; the
//! HEAD tree is the authority that cannot be fooled by a dirty index.
//!
//! Pre-commit must still admit a *new* crate being added in the same commit, so staged
//! crate prefixes are unioned with HEAD. Disk members in neither set are UNTRACKED.

use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HygieneError {
    EmptyDiskScan,
}

impl fmt::Display for HygieneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDiskScan => f.write_str(
                "WORKSPACE_HYGIENE_UNRUN: empty crates/ scan — an empty member set is not a pass",
            ),
        }
    }
}

impl std::error::Error for HygieneError {}

/// Crate directory names under `crates/` that contain a `Cargo.toml`.
pub fn extra_glob_members(
    disk: &BTreeSet<String>,
    committed: &BTreeSet<String>,
    staged: &BTreeSet<String>,
) -> Result<Vec<String>, HygieneError> {
    if disk.is_empty() {
        return Err(HygieneError::EmptyDiskScan);
    }
    Ok(disk
        .iter()
        .filter(|name| !committed.contains(*name) && !staged.contains(*name))
        .cloned()
        .collect())
}

/// Second path component under `crates/` from `git ls-tree` / `git ls-files` paths.
pub fn crate_name_from_git_path(path: &str) -> Option<String> {
    let mut parts = path.split('/');
    if parts.next()? != "crates" {
        return None;
    }
    let name = parts.next()?;
    if name.is_empty() || name.starts_with('.') {
        return None;
    }
    Some(name.to_owned())
}

pub fn crate_names_from_git_paths<I>(paths: I) -> BTreeSet<String>
where
    I: IntoIterator<Item = String>,
{
    paths
        .into_iter()
        .filter_map(|path| crate_name_from_git_path(&path))
        .collect()
}

/// Signature of the ycwh cascade: a glob member whose lexer desynchronised on a nested
/// quote. Used by tests so the workspace-check gate asserts the *message*, not just rc!=0
/// (AGENTS.md: same exit 101 from an unrelated cargo load failure is not this finding).
pub fn is_nested_quote_cascade(stderr: &str) -> bool {
    stderr.contains("underscore literal suffix is not allowed")
        || stderr.contains("prefix `toml` is unknown")
        || (stderr.contains("error[E") && stderr.contains("fuzz-build-gate"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn known_good_all_disk_members_are_in_head() {
        let extras = extra_glob_members(&set(&["ack-spine", "crate-atom-gate"]), &set(&["ack-spine", "crate-atom-gate"]), &set(&[])).unwrap();
        assert!(extras.is_empty());
    }

    #[test]
    fn known_bad_untracked_glob_member_is_named() {
        let extras = extra_glob_members(
            &set(&["ack-spine", "fuzz-build-gate"]),
            &set(&["ack-spine"]),
            &set(&[]),
        )
        .unwrap();
        assert_eq!(extras, vec!["fuzz-build-gate".to_owned()]);
    }

    #[test]
    fn staged_new_crate_is_not_untracked() {
        let extras = extra_glob_members(
            &set(&["ack-spine", "fuzz-build-gate"]),
            &set(&["ack-spine"]),
            &set(&["fuzz-build-gate"]),
        )
        .unwrap();
        assert!(extras.is_empty());
    }

    #[test]
    fn empty_disk_scan_is_error() {
        let error = extra_glob_members(&set(&[]), &set(&["ack-spine"]), &set(&[])).unwrap_err();
        assert_eq!(error, HygieneError::EmptyDiskScan);
    }

    #[test]
    fn mutation_introduces_then_restores_untracked_member() {
        let committed = set(&["ack-spine"]);
        let mut disk = set(&["ack-spine"]);
        assert!(extra_glob_members(&disk, &committed, &set(&[])).unwrap().is_empty());
        disk.insert("ghost".to_owned());
        assert_eq!(
            extra_glob_members(&disk, &committed, &set(&[])).unwrap(),
            vec!["ghost".to_owned()]
        );
        disk.remove("ghost");
        assert!(extra_glob_members(&disk, &committed, &set(&[])).unwrap().is_empty());
    }

    #[test]
    fn crate_name_from_ls_tree_path() {
        assert_eq!(
            crate_name_from_git_path("crates/fuzz-build-gate/src/lib.rs").as_deref(),
            Some("fuzz-build-gate")
        );
        assert_eq!(crate_name_from_git_path("fuzz/Cargo.toml"), None);
    }
}

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

/// A tracked, manifest-bearing `crates/` dir that is excluded from `[workspace]`.
///
/// Keyed on the directory name (last component of the manifest path), NEVER
/// `package.name`. `omp-idle-dispatch` is AGENTS.md's dating tell and is
/// excluded at the root `Cargo.toml`; a standing red on that row gets the
/// gate routed around.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeclaredExclusion {
    pub dir: &'static str,
    pub owner: &'static str,
    pub dies_when: &'static str,
}

pub const DECLARED_WORKSPACE_EXCLUSIONS: &[DeclaredExclusion] = &[DeclaredExclusion {
    dir: "omp-idle-dispatch",
    owner: "josh",
    dies_when: "restored to [workspace] members or deleted; AGENTS.md dating-tell retired",
}];

#[must_use]
pub fn declared_exclusion_dirs() -> BTreeSet<String> {
    DECLARED_WORKSPACE_EXCLUSIONS
        .iter()
        .map(|row| row.dir.to_owned())
        .collect()
}

/// DISK vs CARGO METADATA. Directory names under `crates/` that have a
/// `Cargo.toml` minus those whose *manifest path* (not package name) is in
/// the metadata set, minus declared exclusions.
///
/// Distinct from [`extra_glob_members`]: that is DISK vs GIT (untracked).
/// This population is tracked, manifest-bearing, and workspace-excluded.
pub fn extra_metadata_exclusions(
    disk: &BTreeSet<String>,
    metadata_dirs: &BTreeSet<String>,
    allowed: &BTreeSet<String>,
) -> Result<Vec<String>, HygieneError> {
    if disk.is_empty() {
        return Err(HygieneError::EmptyDiskScan);
    }
    Ok(disk
        .iter()
        .filter(|name| !metadata_dirs.contains(*name) && !allowed.contains(*name))
        .cloned()
        .collect())
}

pub const EXIT_DISK_NOT_IN_METADATA: u8 = 1;
pub const EXIT_EMPTY_DISK_SCAN: u8 = 2;

#[must_use]
pub fn disk_not_in_metadata_reason(names: &[String]) -> String {
    format!(
        "DISK_NOT_IN_METADATA crates=[{}] key=manifest_dir next_action=add-to-workspace-or-declare-exclusion",
        names.join(",")
    )
}

/// Message AND exit: a count-only control is blind to this class.
#[must_use]
pub fn refuse_disk_not_in_metadata(names: &[String]) -> (u8, String) {
    (
        EXIT_DISK_NOT_IN_METADATA,
        disk_not_in_metadata_reason(names),
    )
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

    #[test]
    fn declared_exclusion_carries_owner_and_dies_when() {
        let row = DECLARED_WORKSPACE_EXCLUSIONS
            .iter()
            .find(|row| row.dir == "omp-idle-dispatch")
            .expect("dating tell must be declared");
        assert!(!row.owner.is_empty(), "owner= is required");
        assert!(!row.dies_when.is_empty(), "dies_when= is required");
    }

    /// KNOWN-GOOD: today's workspace with the declared exclusion must PASS.
    #[test]
    fn declared_exclusion_is_not_a_refusal() {
        let disk = set(&["no-shell-gate", "omp-idle-dispatch"]);
        let metadata = set(&["no-shell-gate"]);
        let extras =
            extra_metadata_exclusions(&disk, &metadata, &declared_exclusion_dirs()).unwrap();
        assert!(
            extras.is_empty(),
            "declared dating-tell must not refuse: {extras:?}"
        );
    }

    /// KNOWN-BAD: tracked excluded dir NOT in the allowance is NAMED. Exit 1.
    #[test]
    fn undeclared_disk_member_is_named_with_message_and_exit() {
        let disk = set(&["no-shell-gate", "ghost"]);
        let metadata = set(&["no-shell-gate"]);
        let extras =
            extra_metadata_exclusions(&disk, &metadata, &declared_exclusion_dirs()).unwrap();
        assert_eq!(extras, vec!["ghost".to_owned()]);
        let (code, reason) = refuse_disk_not_in_metadata(&extras);
        assert_eq!(code, EXIT_DISK_NOT_IN_METADATA);
        assert!(
            reason.contains("DISK_NOT_IN_METADATA") && reason.contains("ghost"),
            "must name the crate, not a count: {reason}"
        );
        assert!(
            !reason.chars().all(|c| c.is_ascii_digit() || c.is_whitespace()),
            "a count-only reason is the class this bead forbids: {reason}"
        );
    }

    /// The two checks cover different populations: a TRACKED excluded member
    /// does not fire UNTRACKED_GLOB_MEMBER.
    #[test]
    fn untracked_check_does_not_fire_on_tracked_excluded_member() {
        let disk = set(&["no-shell-gate", "omp-idle-dispatch"]);
        let committed = set(&["no-shell-gate", "omp-idle-dispatch"]);
        let untracked = extra_glob_members(&disk, &committed, &set(&[])).unwrap();
        assert!(
            untracked.is_empty(),
            "tracked excluded member is not untracked: {untracked:?}"
        );
        let metadata = set(&["no-shell-gate"]);
        let excluded =
            extra_metadata_exclusions(&disk, &metadata, &BTreeSet::new()).unwrap();
        assert_eq!(
            excluded,
            vec!["omp-idle-dispatch".to_owned()],
            "without the allowance the dating tell is visible to DISK vs METADATA"
        );
    }

    #[test]
    fn empty_metadata_scan_is_unrun_not_a_pass() {
        let error =
            extra_metadata_exclusions(&set(&[]), &set(&["no-shell-gate"]), &set(&[])).unwrap_err();
        assert_eq!(error, HygieneError::EmptyDiskScan);
        assert_eq!(EXIT_EMPTY_DISK_SCAN, 2);
        assert!(error.to_string().contains("WORKSPACE_HYGIENE_UNRUN"));
    }

    /// Key is manifest dir, not package.name. A crate named `idle` living in
    /// `crates/omp-idle-dispatch` must compare as `omp-idle-dispatch`.
    #[test]
    fn key_is_manifest_dir_not_package_name() {
        let disk = set(&["omp-idle-dispatch"]);
        let keyed_by_package_name = set(&["idle"]);
        let keyed_by_manifest_dir = set(&["omp-idle-dispatch"]);
        assert_eq!(
            extra_metadata_exclusions(&disk, &keyed_by_package_name, &set(&[])).unwrap(),
            vec!["omp-idle-dispatch".to_owned()],
            "a name-keyed comparison invents a false extra"
        );
        assert!(
            extra_metadata_exclusions(&disk, &keyed_by_manifest_dir, &set(&[]))
                .unwrap()
                .is_empty(),
            "manifest-dir key agrees with disk"
        );
    }
}

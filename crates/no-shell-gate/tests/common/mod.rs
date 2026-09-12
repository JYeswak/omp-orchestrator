#![allow(dead_code)]
//! THE ONE PLACE THREE SUITES DECIDE WHETHER THIS BOX MAY SPEAK ABOUT THE REPOSITORY.
//!
//! Bead `omp-orchestrator-typed-unreadable-roster-tihld`.
//!
//! # The defect, measured rather than argued
//!
//! `gate.rs`, `group_kill.rs` and `sender_identity.rs` each carried their OWN roster
//! read, and all three returned an EMPTY VECTOR when `git` failed. A failed read and a
//! genuinely empty tree then look identical, so the anti-vacuity guards downstream fire
//! with a GUESS in their own text -- "most likely `git ls-files` failed", "if git is
//! failing here the roster is a lie" -- and FAIL anyway. The cause was known to the code
//! and thrown away.
//!
//! Measured 2026-09-12 on `contabo-3`, the worker with NO `.git` at all: six legs across
//! those three files RED, every one of them for that single reason, and no edit to this
//! repository could clear any of them. A red that no edit can clear is the shape that
//! gets routed around.
//!
//! # What this module is, and what it deliberately is not
//!
//! It is a PORT of `oracle_routing.rs`'s `committed_crates` / `roster_or_unmeasured`
//! (InvMapRed, 518c90f), generalised over a pathspec so three more suites can share ONE
//! decision instead of growing a fourth copy. It is NOT a second vocabulary: the typed
//! failure is `omp_inventory_map::types_inventory::CensusSource`, borrowed, because a
//! parallel enum for "may I adjudicate this tree" is the duplicate-authority defect this
//! repository has already paid for once in `hook_digest`.
//!
//! It lives under `tests/` and NOT under `src/` on purpose. A module in `src/` compiles
//! into `pre-commit-gate` -- the live `.git/hooks/pre-commit` -- and also joins the hook's
//! source manifest, so every installed hook in the fleet would report `STALE_HEALING` on
//! its next commit. Neither cost buys this unit anything.
//!
//! # The three answers, kept distinct
//!
//! ```text
//! UNREADABLE   git cannot name a commit here      -> UNMEASURABLE, named reason, no verdict
//! EMPTY        git answered, and answered nothing -> ERROR, never a pass (unchanged)
//! ROWS         git answered with a roster         -> the leg adjudicates exactly as before
//! ```
//!
//! The middle row is why this is not a shrug: an empty-but-readable roster is still a
//! hard error, and on a box that can read, every assertion downstream is untouched.

use std::path::Path;
use std::process::Command;

pub use omp_inventory_map::types_inventory::binding_environment;
use omp_inventory_map::types_inventory::CensusSource;

/// Paths tracked AT THE COMMIT under `pathspec`, plus the revision they came from.
///
/// THE COMMIT IS THE CORRECT SURFACE BY SUBJECT, not merely the more robust one: these
/// legs ask "what can CI see", and CI checks out a commit. A staged-but-uncommitted file
/// is genuinely invisible to CI, and a commit-derived listing says so where an
/// index-derived one hides it.
///
/// A failure is TYPED, never an empty vector -- that fallback is the bug this module
/// exists to remove, and no amount of better wording downstream repairs it.
pub fn committed_paths(root: &Path, pathspec: &str) -> Result<(Vec<String>, String), CensusSource> {
    let git = |args: &[&str]| -> Result<String, String> {
        let out = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .map_err(|error| format!("cannot run `git {}`: {error}", args.join(" ")))?;
        if !out.status.success() {
            return Err(format!(
                "`git {}` failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    };

    let rev = match git(&["rev-parse", "HEAD"]) {
        Ok(text) => text.trim().to_owned(),
        Err(reason) => return Err(CensusSource::Worktree { reason }),
    };
    let listing = match git(&["ls-tree", "-r", "--name-only", &rev, "--", pathspec]) {
        Ok(text) => text,
        Err(reason) => return Err(CensusSource::Worktree { reason }),
    };

    let mut paths: Vec<String> = listing
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    paths.sort();
    paths.dedup();
    if paths.is_empty() {
        // READABLE AND EMPTY IS STILL AN ERROR -- it is simply a DIFFERENT error, and the
        // reason says which. Collapsing the two is the defect, in either direction.
        return Err(CensusSource::Worktree {
            reason: format!(
                "commit {} lists nothing under {pathspec} -- an empty listing is an ERROR, \
                 never a clean bill",
                &rev[..rev.len().min(12)]
            ),
        });
    }
    Ok((paths, rev))
}

/// The listing, or a NAMED UNKNOWN. The single decision point for the three suites.
///
/// IN THE ORACLE IT STAYS FATAL. CI checks out a commit, so a CI that cannot name one is
/// a broken checkout, and passing there is how the only box that adjudicates these legs
/// stops adjudicating them.
pub fn paths_or_unmeasured(root: &Path, leg: &str, pathspec: &str) -> Option<(Vec<String>, String)> {
    match committed_paths(root, pathspec) {
        Ok(listing) => Some(listing),
        Err(source) => {
            let reason = source
                .blocked_reason()
                .expect("a failed listing carries its reason")
                .to_owned();
            assert!(
                binding_environment().is_none(),
                "{} is the oracle for {leg} and it could not read the commit: {reason} \
                 -- fix the checkout, never the assertion",
                binding_environment().unwrap_or_default()
            );
            eprintln!(
                "GATE_RUNNER_UNMEASURABLE names={leg}:ROSTER_UNREADABLE reason={reason} \
                 pathspec={pathspec} -- this box cannot name a commit, so every verdict \
                 below would be a statement about the environment"
            );
            None
        }
    }
}

/// The INDEX listing under `pathspec`, typed the same way.
///
/// # Why a second surface rather than one "can git answer" probe
///
/// THE CONSUMER'S SURFACE IS THE ONLY ONE THAT CAN EXCUSE THE CONSUMER. `check_repo`
/// reads `git ls-files`; the commit read above reads `rev-parse` + `ls-tree`. On the boxes
/// measured so far both fail together -- `contabo-3` has no `.git` at all -- which is
/// exactly why a guard on the WRONG surface looked correct: two oracles that agree on
/// every observed sample and are guaranteed to diverge on the unobserved one (a
/// repository whose HEAD resolves and whose INDEX is unreadable: the FOSSIL/UNBORN middle
/// shapes, measured at 85/333/86 tracked against 1136+ local). There a commit-surface
/// guard ADMITS and the leg still fails FOR THE ENVIRONMENT.
///
/// The revision is reported as `index` rather than a sha: the index has no revision, and
/// inventing one would be the fabricated-provenance class.
pub fn index_paths(root: &Path, pathspec: &str) -> Result<(Vec<String>, String), CensusSource> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "--", pathspec])
        .output()
        .map_err(|error| CensusSource::Worktree {
            reason: format!("cannot run `git ls-files -- {pathspec}`: {error}"),
        })?;
    if !out.status.success() {
        return Err(CensusSource::Worktree {
            reason: format!(
                "`git ls-files -- {pathspec}` failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        });
    }
    let mut paths: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    paths.sort();
    paths.dedup();
    if paths.is_empty() {
        return Err(CensusSource::Worktree {
            reason: format!(
                "the index lists nothing under {pathspec} -- an empty listing is an ERROR, \
                 never a clean bill"
            ),
        });
    }
    Ok((paths, "index".to_owned()))
}

/// [`index_paths`], or a NAMED UNKNOWN -- for legs whose consumer reads the INDEX.
pub fn index_or_unmeasured(root: &Path, leg: &str, pathspec: &str) -> Option<(Vec<String>, String)> {
    match index_paths(root, pathspec) {
        Ok(listing) => Some(listing),
        Err(source) => {
            let reason = source
                .blocked_reason()
                .expect("a failed listing carries its reason")
                .to_owned();
            assert!(
                binding_environment().is_none(),
                "{} is the oracle for {leg} and it could not read the index: {reason} \
                 -- fix the checkout, never the assertion",
                binding_environment().unwrap_or_default()
            );
            eprintln!(
                "GATE_RUNNER_UNMEASURABLE names={leg}:INDEX_UNREADABLE reason={reason} \
                 pathspec={pathspec} -- this leg's consumer reads the INDEX, and this box \
                 cannot deliver one"
            );
            None
        }
    }
}

/// Crate names from the committed manifests, for suites that want the roster itself.
///
/// Fixture manifests nested under a crate's own `tests/` are not workspace members and
/// must not inflate the roster (4 of them on 2026-09-05).
pub fn committed_crate_names(root: &Path) -> Result<(Vec<String>, String), CensusSource> {
    let (paths, rev) = committed_paths(root, "crates")?;
    let mut names: Vec<String> = paths
        .iter()
        .filter_map(|line| line.strip_prefix("crates/"))
        .filter_map(|rest| rest.strip_suffix("/Cargo.toml"))
        .filter(|name| !name.contains('/'))
        .map(str::to_owned)
        .collect();
    names.sort();
    names.dedup();
    if names.is_empty() {
        return Err(CensusSource::Worktree {
            reason: format!(
                "commit {} lists no crates/<name>/Cargo.toml -- an empty roster is an ERROR, \
                 never a clean bill",
                &rev[..rev.len().min(12)]
            ),
        });
    }
    Ok((names, rev))
}

/// [`committed_crate_names`], or a named UNKNOWN.
pub fn crate_names_or_unmeasured(root: &Path, leg: &str) -> Option<(Vec<String>, String)> {
    match committed_crate_names(root) {
        Ok(roster) => Some(roster),
        Err(source) => {
            let reason = source
                .blocked_reason()
                .expect("a failed roster carries its reason")
                .to_owned();
            assert!(
                binding_environment().is_none(),
                "{} is the oracle for {leg} and it could not read the commit: {reason} \
                 -- fix the checkout, never the assertion",
                binding_environment().unwrap_or_default()
            );
            eprintln!(
                "GATE_RUNNER_UNMEASURABLE names={leg}:ROSTER_UNREADABLE reason={reason} \
                 -- the crate roster comes from the commit and this box cannot name one"
            );
            None
        }
    }
}

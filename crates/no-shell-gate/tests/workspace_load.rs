//! Workspace-load gate legs (bead omp-orchestrator-workspace-load-gate-of3).
//!
//! These legs run the load check against (a) THIS repository and (b) synthetic
//! workspace fixtures in tempdirs, so the known-bad demonstration never breaks
//! the shared checkout for other panes. Every leg asserts the DETECTOR NAME,
//! not a bare exit code — "the guard fired" must be distinguishable from "the
//! check could not start".

#![forbid(unsafe_code)]

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use no_shell_gate::{check_workspace_load, WorkspaceLoad};

static FIXTURE_SEQ: AtomicU32 = AtomicU32::new(0);

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonicalize this repo's root")
}

/// A minimal two-member synthetic workspace: one healthy member, one member
/// whose manifest is an UNCLOSED TABLE — the exact shape the 07:36Z incident
/// measured in the wild.
fn fixture_workspace_with_broken_member() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "workspace-load-{}-broken-{}",
        std::process::id(),
        FIXTURE_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(dir.join("healthy-member/src")).expect("create fixture dirs");
    std::fs::create_dir_all(dir.join("broken-member/src")).expect("create fixture dirs");
    std::fs::write(
        dir.join("Cargo.toml"),
        "[workspace]\nmembers = [\"healthy-member\", \"broken-member\"]\nresolver = \"2\"\n",
    )
    .expect("write workspace manifest");
    std::fs::write(
        dir.join("healthy-member/Cargo.toml"),
        "[package]\nname = \"healthy-member\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write healthy member");
    std::fs::write(
        dir.join("healthy-member/src/lib.rs"),
        "pub fn healthy() {}\n",
    )
    .expect("write healthy lib");
    // THE PLANT: an unclosed table — cargo metadata must refuse this manifest
    // by name.
    std::fs::write(dir.join("broken-member/Cargo.toml"), "[package\nname = \"broken\"\n")
        .expect("write broken member");
    dir
}

/// The CONTROL fixture: a fully healthy two-member workspace, no broken
/// sibling. Proves the gate's Loaded verdict is earned, not defaulted.
fn fixture_workspace_healthy() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "workspace-load-{}-healthy-{}",
        std::process::id(),
        FIXTURE_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(dir.join("crates/member-a/src")).expect("create fixture dirs");
    std::fs::create_dir_all(dir.join("crates/member-b/src")).expect("create fixture dirs");
    std::fs::write(
        dir.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"2\"\n",
    )
    .expect("write workspace manifest");
    std::fs::write(
        dir.join("crates/member-a/Cargo.toml"),
        "[package]\nname = \"member-a\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write member-a manifest");
    std::fs::write(dir.join("crates/member-a/src/lib.rs"), "pub fn a() {}\n")
        .expect("write lib");
    std::fs::write(
        dir.join("crates/member-b/Cargo.toml"),
        "[package]\nname = \"member-b\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write member-b manifest");
    std::fs::write(dir.join("crates/member-b/src/lib.rs"), "pub fn b() {}\n")
        .expect("write lib");
    dir
}

/// A minimal workspace that LOADS but enumerates zero members — the vacuous
/// scan set. Anti-vacuity says this is an ERROR, never a pass.
fn fixture_workspace_without_members() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "workspace-load-{}-empty-{}",
        std::process::id(),
        FIXTURE_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("create fixture dirs");
    std::fs::write(dir.join("Cargo.toml"), "[workspace]\nmembers = []\nresolver = \"2\"\n")
        .expect("write workspace manifest");
    dir
}

#[test]
fn this_workspace_loads_with_the_gate_among_members() {
    let verdict = check_workspace_load(&repo_root());
    assert!(
        verdict.is_loaded(),
        "the real workspace must load; got detector {} ({verdict:?})",
        verdict.detector()
    );
    match &verdict {
        WorkspaceLoad::Loaded { members } => {
            assert!(
                members.iter().any(|m| m == "no-shell-gate"),
                "the scan set must include this crate: {members:?}"
            );
            assert!(
                members.iter().any(|m| m == "tick-monitor"),
                "the scan set must include extracted crates: {members:?}"
            );
        }
        other => panic!("expected Loaded, got {other:?}"),
    }
}

#[test]
fn broken_member_manifest_is_typed_unloaded_naming_the_member() {
    let dir = fixture_workspace_with_broken_member();
    let verdict = check_workspace_load(&dir);
    assert_eq!(
        verdict.detector(),
        "WORKSPACE_MEMBER_UNREADABLE",
        "the planted unclosed table must produce the typed member detector, got {} ({verdict:?})",
        verdict.detector()
    );
    match &verdict {
        WorkspaceLoad::MemberUnreadable { manifest, detail } => {
            assert!(
                manifest.contains("broken-member/Cargo.toml"),
                "the typed error must name the offending member manifest: {manifest}"
            );
            assert!(
                !detail.trim().is_empty(),
                "cargo's own error text must be carried as the detail"
            );
        }
        other => panic!("expected MemberUnreadable, got {other:?}"),
    }
}

#[test]
fn healthy_workspace_control_loads() {
    // The CONTROL for the broken-member leg: same fixture shape, minus the
    // broken sibling. A member of a BROKEN workspace correctly reads unloaded
    // (cargo walks up to the workspace root and refuses there) — so the
    // control needs its own healthy workspace, not a member of a broken one.
    let dir = fixture_workspace_healthy();
    let verdict = check_workspace_load(&dir);
    assert_eq!(
        verdict.detector(),
        "WORKSPACE_LOADED",
        "the healthy control must load, got {} ({verdict:?})",
        verdict.detector()
    );
}

#[test]
fn loaded_but_members_empty_is_an_error_not_a_pass() {
    let dir = fixture_workspace_without_members();
    let verdict = check_workspace_load(&dir);
    assert_eq!(
        verdict.detector(),
        "WORKSPACE_MEMBERS_EMPTY",
        "a loaded workspace with zero members is a vacuous scan set: got {}",
        verdict.detector()
    );
    assert!(!verdict.is_loaded());
}

#[test]
fn missing_root_manifest_is_typed() {
    let dir = std::env::temp_dir().join(format!(
        "workspace-load-{}-noroot-{}",
        std::process::id(),
        FIXTURE_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("create empty fixture dir");
    let verdict = check_workspace_load(&dir);
    assert_eq!(
        verdict.detector(),
        "WORKSPACE_MANIFEST_MISSING",
        "no root manifest must produce its own detector, got {}",
        verdict.detector()
    );
    assert!(!verdict.is_loaded());
}


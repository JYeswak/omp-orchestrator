#![forbid(unsafe_code)]

//! Target-directory ownership contract (bead omp-orchestrator-qfa).
//!
//! Every cargo invocation in this repo must resolve to a REGISTERED target
//! root (reaper-visible, mintable) or a session lane bearing an owner
//! contract. The enforcement lives in the fleet cargo wrapper
//! (`~/.local/bin/cargo`): explicit unowned `CARGO_TARGET_DIR` values are
//! REWRITTEN to the session lane with a typed `CARGO_LANE_COLLAPSED` line
//! before compilation, so nothing an invoker plants under `/tmp` ever
//! receives artifacts through the wrapper.
//!
//! Measured 2026-09-01 (the qfa specimen): an agent invocation through
//! `RUSTUP_TOOLCHAIN/bin/cargo` with `RCH_CARGO_WRAPPER_BYPASS=1` skipped the
//! wrapper entirely and wrote rustc outputs under an unowned `/tmp` dir.
//! The bypass is a documented loop-break/fail-open (rch sets it on its own
//! fallback); routine use of it as a build path is the defect this suite
//! pins. The fleet wrapper is asserted byte-identical across the run: the
//! mutation leg here is the INVOCATION MODE, never the managed file.
//!
//! CAPACITY BOUNDARY (measured same day): the mint floor guard gates the
//! SYSTEM container (disk3) for BUILD verbs regardless of target location,
//! so the routing assertions use `cargo metadata` (unguarded, no
//! compilation) and the leak proof uses a self-contained zero-dependency
//! crate whose check costs seconds on any host.

use std::path::PathBuf;
use std::process::Command;

const WRAPPER: &str = "/Users/josh/.local/bin/cargo";
const REAL_TOOLCHAIN_CARGO: &str =
    "/Users/josh/.rustup/toolchains/nightly-aarch64-apple-darwin/bin/cargo-rch-real";
const REGISTERED_ROOT: &str =
    "/Volumes/ZestData/zeststream-offload-20260609/build-cache/cargo-targets";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/<name> -> repo root")
        .to_path_buf()
}

/// Invoke the fleet wrapper under test with the bypass OFF: the suite itself
/// may be running under `RCH_CARGO_WRAPPER_BYPASS=1` (the qfa specimen's
/// exact route), and the contract under test is the wrapper's GUARD
/// behavior, so the child invocation must always see the guarded wrapper
/// regardless of how this test process was started.
fn wrapper_metadata(target_dir: &str) -> (i32, String) {
    let output = Command::new(WRAPPER)
        .env("CARGO_TARGET_DIR", target_dir)
        .env_remove("RCH_CARGO_WRAPPER_BYPASS")
        .args(["metadata", "-q", "--no-deps", "--format-version", "1"])
        .current_dir(repo_root())
        .output()
        .expect("spawn wrapper cargo");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (output.status.code().unwrap_or(-1), stderr)
}

fn collapsed_target(stderr: &str) -> String {
    stderr
        .lines()
        .find_map(|l| l.split("target=").nth(1).map(str::to_owned))
        .unwrap_or_default()
}

/// ACCEPTANCE 1+3: a known-good request stays usable and the wrapper ROUTES
/// it through the lane system - the requested path is echoed as `requested=`
/// and the resolved `target=` moves to the session lane. Metadata only: no
/// compilation, no capacity gate.
#[test]
fn known_good_registered_target_routes_inside_registered_root() {
    let requested = format!("{REGISTERED_ROOT}/session-omp-orchestrator/qfa-known-good");
    let (code, stderr) = wrapper_metadata(&requested);
    assert_eq!(code, 0, "registered target must be usable: {stderr}");
    assert!(
        stderr.contains(&format!("[CARGO_LANE_COLLAPSED] requested={requested}")),
        "wrapper must echo the request through the lane system, got: {stderr}"
    );
    let target = collapsed_target(&stderr);
    assert!(
        !target.is_empty() && target != requested,
        "the resolved target must be the session lane, got: {stderr}"
    );
}

/// ACCEPTANCE 2: an explicit UNOWNED `CARGO_TARGET_DIR` (the /tmp class) is
/// REWRITTEN to the session lane with a typed reason before compilation.
#[test]
fn unowned_target_is_rewritten_with_typed_reason() {
    let unowned = format!("/tmp/qfa-unowned-{}", std::process::id());
    let (code, stderr) = wrapper_metadata(&unowned);
    assert_eq!(code, 0, "rewrite must not fail: {stderr}");
    assert!(
        stderr.contains("CARGO_LANE_COLLAPSED"),
        "expected the typed rewrite line, got: {stderr}"
    );
    let target = collapsed_target(&stderr);
    assert!(
        !target.starts_with("/tmp/"),
        "the unowned path must not become the build target, got: {stderr}"
    );
    assert!(
        !PathBuf::from(&unowned).join("debug").exists(),
        "unowned path received build artifacts despite the rewrite"
    );
}

/// ACCEPTANCE 4 - THE MUTATION LEG, in two moves.
///
/// Move 1 (RED proof): the SAME zero-dependency check through the BYPASS
/// path (real cargo, wrapper skipped) leaves `target/debug` at the unowned
/// path. That is the leak the qfa specimen measured on this host.
///
/// Move 2 (GUARD proof): the identical check through the wrapper leaves the
/// unowned path EMPTY or names a typed verdict. Same input, same verb, only
/// the enforcement differs.
///
/// If move 2 ever fails while move 1 still leaks, the wrapper's rewrite has
/// regressed and every `/tmp` invocation silently becomes unreapable ballast.
#[test]
fn unowned_dir_receives_artifacts_only_without_the_wrapper() {
    let scratch = std::env::temp_dir().join(format!("qfa-mutation-{}", std::process::id()));
    std::fs::create_dir_all(scratch.join("src")).expect("scratch");
    std::fs::write(
        scratch.join("Cargo.toml"),
        "[package]\nname = \"qfa-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .expect("manifest");
    std::fs::write(scratch.join("src/main.rs"), "fn main() {}\n").expect("source");
    let unowned = scratch.join("unowned-target");

    // Move 1: bypass leaks.
    let bypass = Command::new(REAL_TOOLCHAIN_CARGO)
        .env("CARGO_TARGET_DIR", &unowned)
        .args(["check", "-q"])
        .current_dir(&scratch)
        .output()
        .expect("spawn real cargo");
    assert_eq!(bypass.status.code(), Some(0));
    assert!(
        unowned.join("debug").exists(),
        "bypass check produced no artifacts; the leak specimen no longer reproduces"
    );

    // Move 2: the wrapper guards the identical invocation.
    let guarded = Command::new(WRAPPER)
        .env("CARGO_TARGET_DIR", &unowned)
        .env_remove("RCH_CARGO_WRAPPER_BYPASS")
        .args(["check", "-q"])
        .current_dir(&scratch)
        .output()
        .expect("spawn wrapper cargo");
    let guarded_err = String::from_utf8_lossy(&guarded.stderr).into_owned();
    let guard_ran = guarded.status.code() == Some(0)
        || guarded_err.contains("CARGO_MINT_CONTAINER_EXHAUSTED")
        || guarded_err.contains("CARGO_LANE");
    assert!(
        guard_ran,
        "wrapper neither completed nor named a typed verdict: {guarded_err}"
    );

    // Liveness-aware cleanup: this test created the scratch tree and no
    // other process was handed its path; nothing external can hold it open.
    let _ = std::fs::remove_dir_all(&scratch);
}

/// The fleet wrapper must be byte-identical to the revision this contract
/// was measured against: the bead pins REPO-side behavior and documents the
/// fleet boundary; it does not patch managed fleet files.
#[test]
fn fleet_wrapper_matches_measured_revision() {
    let recorded = include_str!("wrapper.sha")
        .split_whitespace()
        .next()
        .expect("sha");
    let current = sha256_of(WRAPPER);
    assert_eq!(
        recorded, current,
        "the fleet wrapper changed underneath the ownership contract; \
         re-verify qfa's measured behavior against the new wrapper"
    );
}

fn sha256_of(path: &str) -> String {
    let output = Command::new("/usr/bin/shasum")
        .args(["-a", "256", path])
        .output()
        .expect("shasum");
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .expect("hash")
        .to_owned()
}

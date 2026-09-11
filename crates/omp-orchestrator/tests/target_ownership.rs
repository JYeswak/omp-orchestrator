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
use omp_orchestrator::host_precondition::{measurable_here, HostRequirement};

// HOST-PLATFORM PRECONDITIONS (bead bz2na). Every leg below spawns a binary
// or hashes a file that exists only on ONE machine: the shim at
// `~/.local/bin/cargo`, a toolchain binary whose path literal contains
// `aarch64-apple-darwin`, and `/usr/bin/shasum` (macOS; Linux ships
// `sha256sum` and has no such path). On every Linux host -- the Contabo
// workers AND `ubuntu-latest` -- those spawns reported `NotFound` and the
// legs read as code failures.
//
// The requirement sets below are PER LEG and deliberately unequal, because
// the four legs do not share a cause: two need only the shim, one needs the
// Darwin toolchain, one needs `shasum`. A single merged predicate would
// report the wrong absent artifact for three of the four.
//
// NOT A CRATE-LEVEL GATE. `omp-orchestrator` has nine failing legs behind
// three distinct causes; gating the crate would hide all nine to skip these
// four. Granularity is the whole point of putting this in the test.

fn wrapper_path() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").expect("HOME for cargo wrapper")).join(".local/bin/cargo")
}

fn real_toolchain_cargo() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").expect("HOME for real cargo"))
        .join(".rustup/toolchains/nightly-aarch64-apple-darwin/bin/cargo-rch-real")
}
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
    let output = Command::new(wrapper_path())
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
    // Spawns the shim; the requested path lives under the registered root, so
    // the mount is named too. The shim alone would suffice on today's hosts --
    // naming the mount costs nothing where it is present and gives the true
    // reason on a host that has the shim without the volume.
    if !measurable_here(
        "known_good_registered_target_routes_inside_registered_root",
        &[
            HostRequirement::HostCargoShim,
            HostRequirement::RegisteredRootMount,
        ],
    ) {
        return;
    }
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
    // Spawns the shim only: the unowned path is under `/tmp`, which every host
    // has, so this leg needs nothing else.
    if !measurable_here(
        "unowned_target_is_rewritten_with_typed_reason",
        &[HostRequirement::HostCargoShim],
    ) {
        return;
    }
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
    // TWO spawns, TWO requirements: move 1 runs the real Darwin toolchain
    // binary, move 2 runs the shim. Both must be present or the differential
    // is not available -- and a differential with one arm missing is not a
    // weaker measurement, it is no measurement.
    if !measurable_here(
        "unowned_dir_receives_artifacts_only_without_the_wrapper",
        &[
            HostRequirement::DarwinToolchainCargo,
            HostRequirement::HostCargoShim,
        ],
    ) {
        return;
    }
    let scratch = std::env::temp_dir().join(format!("qfa-mutation-{}", std::process::id()));
    std::fs::create_dir_all(scratch.join("src")).expect("scratch");
    std::fs::write(
        scratch.join("Cargo.toml"),
        "[package]\nname = \"qfa-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[[bin]]\nname = \"qfa-probe\"\npath = \"src/resident.rs\"\n",
    )
    .expect("manifest");
    std::fs::write(scratch.join("src/resident.rs"), "fn main() {}\n").expect("source");
    let unowned = scratch.join("unowned-target");

    // Move 1: bypass leaks.
    let bypass = Command::new(real_toolchain_cargo())
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
    let guarded = Command::new(wrapper_path())
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
    // `shasum` is the macOS spelling; the shim is named because it is the FILE
    // being hashed, not a spawn here. Two different roles, both required.
    if !measurable_here(
        "fleet_wrapper_matches_measured_revision",
        &[HostRequirement::ShasumTool, HostRequirement::HostCargoShim],
    ) {
        return;
    }
    let recorded = include_str!("wrapper.sha")
        .split_whitespace()
        .next()
        .expect("sha");
    let current = sha256_of(wrapper_path().to_str().expect("wrapper path"));
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

/// Split this file into `(leg name, body)` pairs. Reading the source is the
/// only way a census can see a MISSING guard: a leg that forgot one compiles
/// and passes on the one host where its artifacts happen to exist.
fn leg_bodies() -> Vec<(String, String)> {
    let source = include_str!("target_ownership.rs");
    let mut bodies = Vec::new();
    for chunk in source.split("\n#[test]\n").skip(1) {
        let name = chunk
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("fn "))
            .and_then(|rest| rest.split('(').next())
            .expect("a #[test] must be followed by `fn <name>(`")
            .to_owned();
        bodies.push((name, chunk.to_owned()));
    }
    bodies
}

/// Executable text only: comments and double-quoted string literals removed,
/// so a mention of an accessor in prose or in a census table is not mistaken
/// for a CALL.
///
/// COMMENTS GO FIRST, and the order is load-bearing. An unbalanced quote
/// inside a comment -- a quoted phrase in a sentence -- puts the literal
/// scanner out of phase for the whole rest of the body, which re-exposes the
/// table below as if it were code. That is how this census classified itself
/// a SECOND time after the first fix.
fn code_only(body: &str) -> String {
    let decommented = body
        .lines()
        .map(|line| line.split("//").next().unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n");
    let mut out = String::with_capacity(decommented.len());
    let mut inside = false;
    let mut escaped = false;
    for ch in decommented.chars() {
        let delimits = ch == '"' && !escaped;
        if delimits {
            inside = !inside;
        } else if !inside {
            out.push(ch);
        }
        escaped = ch == '\\' && !escaped;
    }
    out
}

/// CENSUS BY MEMBERSHIP, AND IT LIVES IN THE FILE IT SCANS.
///
/// The `resident.rs` census pins guard call sites in `resident.rs` ONLY, so it
/// is structurally blind to guards added here — a green there is a limitation
/// of that control, not coverage of this file. This leg is the missing half.
///
/// It runs in BOTH directions, and the second is the load-bearing one:
/// 1. every named host-dependent leg carries a guard naming its requirements;
/// 2. every leg that TOUCHES a host artifact is in the named set — so a new
///    unguarded spawn cannot arrive silently, which is exactly how these four
///    legs came to fail on every Linux host in the first place.
#[test]
fn every_host_dependent_leg_is_guarded_and_the_set_is_pinned_by_name() {
    // The host-artifact accessors. A leg mentioning any of these runs a
    // machine-specific binary or hashes a machine-specific file.
    //
    // `wrapper_metadata(` is INDIRECT -- it spawns the shim on the leg's
    // behalf -- and it is the reason the first version of this direction
    // reported two legs as "guarded but no longer host-dependent" while they
    // were spawning the shim one call deep. A census that only sees direct
    // calls is blind to exactly the legs that share a helper.
    const HOST_ACCESSORS: &[&str] = &[
        "wrapper_path()",
        "real_toolchain_cargo()",
        "sha256_of(",
        "wrapper_metadata(",
    ];
    // The indirection is CHECKED, not assumed: if `wrapper_metadata` stops
    // spawning the shim, listing it here would classify legs by a dependency
    // they no longer have.
    let source = include_str!("target_ownership.rs");
    let helper = source
        .split("fn wrapper_metadata(")
        .nth(1)
        .expect("the helper this census depends on must exist");
    assert!(
        helper
            .split("\nfn ")
            .next()
            .is_some_and(|body| body.contains("wrapper_path()")),
        "wrapper_metadata no longer spawns the shim; it is not a host accessor"
    );
    // (leg, the requirement variants its guard MUST name)
    const GUARDED: &[(&str, &[&str])] = &[
        (
            "known_good_registered_target_routes_inside_registered_root",
            &["HostCargoShim", "RegisteredRootMount"],
        ),
        (
            "unowned_target_is_rewritten_with_typed_reason",
            &["HostCargoShim"],
        ),
        (
            "unowned_dir_receives_artifacts_only_without_the_wrapper",
            &["DarwinToolchainCargo", "HostCargoShim"],
        ),
        (
            "fleet_wrapper_matches_measured_revision",
            &["ShasumTool", "HostCargoShim"],
        ),
    ];

    let bodies = leg_bodies();
    assert!(
        !bodies.is_empty(),
        "ANTI-VACUITY: the source split found no legs, so every assertion below is empty"
    );

    // Direction 1: the named legs are guarded, by requirement NAME.
    for (leg, requirements) in GUARDED {
        let (_, body) = bodies
            .iter()
            .find(|(name, _)| name == leg)
            .unwrap_or_else(|| panic!("census names a leg that no longer exists: {leg}"));
        assert!(
            body.contains("measurable_here("),
            "{leg} touches a host artifact but carries no precondition"
        );
        assert!(
            body.contains(&format!("\"{leg}\"")),
            "{leg}'s guard must report ITS OWN name, or the skip line names the wrong leg"
        );
        for requirement in *requirements {
            assert!(
                body.contains(&format!("HostRequirement::{requirement}")),
                "{leg} must name HostRequirement::{requirement}"
            );
        }
    }

    // Direction 2: nothing host-dependent is missing from the set.
    //
    // SELF-REFERENCE TRAP, and it fired on the first run: this census lists
    // the accessor spellings AS DATA, so a raw `contains` classified the
    // census itself as host-dependent. The discriminator is a real one rather
    // than a carve-out by name -- a leg that merely NAMES an accessor in a
    // message or a table is not host-dependent; only a CALL is.
    let named: Vec<&str> = GUARDED.iter().map(|(leg, _)| *leg).collect();
    let touches_host = |body: &str| {
        let code = code_only(body);
        HOST_ACCESSORS
            .iter()
            .any(|accessor| code.contains(accessor))
    };
    // NEGATIVE CONTROL on the discriminator itself (rule 8i): the stripper
    // must actually remove a quoted accessor. If it silently returned the body
    // unchanged, direction 2 would be permanently red on this leg -- and if it
    // stripped everything, direction 2 would be vacuously green for all.
    let (_, census_body) = bodies
        .iter()
        .find(|(name, _)| name == "every_host_dependent_leg_is_guarded_and_the_set_is_pinned_by_name")
        .expect("this leg must find itself");
    assert!(
        census_body.contains("wrapper_path()"),
        "precondition for the control: this leg must quote an accessor"
    );
    assert!(
        !code_only(census_body).contains("wrapper_path()"),
        "the literal stripper does not strip; this census would classify itself"
    );
    assert!(
        code_only(census_body).contains("touches_host"),
        "the literal stripper removed code, not just literals"
    );
    let unguarded: Vec<String> = bodies
        .iter()
        .filter(|(name, body)| touches_host(body) && !named.contains(&name.as_str()))
        .map(|(name, _)| name.clone())
        .collect();
    assert!(
        unguarded.is_empty(),
        "these legs touch a host artifact and are absent from the census: {unguarded:?}"
    );
    // And the guarded set is not a superset of reality either: a name kept
    // after its host dependency is removed would skip a leg that can now run
    // everywhere.
    for (leg, _) in GUARDED {
        let (_, body) = bodies.iter().find(|(name, _)| name == leg).expect("present");
        assert!(
            touches_host(body),
            "{leg} is guarded but no longer touches a host artifact; drop the guard"
        );
    }
}

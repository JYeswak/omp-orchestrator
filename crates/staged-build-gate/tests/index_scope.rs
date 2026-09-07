#![forbid(unsafe_code)]
//! 929j — the scan is bounded by the index GIT hands the hook, not by a new mechanism.
//!
//! The bead recorded a 603-second first run and named four candidate mechanisms for
//! bounding the scan to the committing agent. **None was needed.** A path-scoped commit
//! makes git build a temporary index and point `GIT_INDEX_FILE` at it, so the staged set
//! inside the hook IS the committing agent's own pathspec.
//!
//! The values below are VERBATIM from a scratch two-writer repo, 2026-09-02, with a hook
//! that printed its own environment. They are recorded rather than reasoned about, because
//! the whole defect was a confident belief about what `git diff --cached` returns.

use staged_build_gate::IndexScope;

/// MEASURED. Two files staged by "different writers"; a hook printing `GIT_INDEX_FILE` and
/// `git diff --cached --name-only`.
///
/// | command | `GIT_INDEX_FILE` | hook saw |
/// |---|---|---|
/// | `git commit -- A/f` | `.git/next-index-43322.lock` | `A/f` |
/// | `git commit --only -- A/f` | `.git/next-index-95340.lock` | `A/f` |
/// | `git commit` | `.git/index` | `A/f B/f` |
/// | `git commit -a` | `.git/index.lock` | `A/f B/f` |
const MEASURED: &[(&str, &str, &str)] = &[
    (
        "git commit -- A/f",
        "/tmp/r24/scoped3-77313/.git/next-index-43322.lock",
        "scoped",
    ),
    (
        "git commit --only -- A/f",
        "/tmp/r24/scoped3-77313/.git/next-index-95340.lock",
        "scoped",
    ),
    ("git commit", "/tmp/r24/scoped3-77313/.git/index", "shared"),
    (
        "git commit -a",
        "/tmp/r24/scoped3-77313/.git/index.lock",
        "shared",
    ),
];

#[test]
fn the_four_measured_commit_forms_classify_as_recorded() {
    assert!(!MEASURED.is_empty(), "ANTI-VACUITY: no rows, no evidence");
    for (command, index_file, expected) in MEASURED {
        assert_eq!(
            IndexScope::classify(Some(index_file)).label(),
            *expected,
            "{command} sets GIT_INDEX_FILE={index_file} and must classify {expected}"
        );
    }
    // BOTH labels must actually appear, or the table proves only one direction.
    let labels: std::collections::BTreeSet<&str> = MEASURED.iter().map(|row| row.2).collect();
    assert_eq!(
        labels,
        ["scoped", "shared"].into_iter().collect(),
        "the table must contain a positive AND a negative case, saw {labels:?}"
    );
}

#[test]
fn only_a_scoped_index_may_be_built() {
    // The load-bearing predicate. `Shared` is the 603-second case: nine peers' staged paths
    // arriving as if they were the caller's.
    assert!(IndexScope::classify(Some("/r/.git/next-index-1.lock")).may_build());
    assert!(!IndexScope::classify(Some("/r/.git/index")).may_build());
    assert!(!IndexScope::classify(Some("/r/.git/index.lock")).may_build());
    assert!(!IndexScope::classify(None).may_build());
    assert!(
        !IndexScope::classify(Some("   ")).may_build(),
        "an empty GIT_INDEX_FILE is unset, not scoped"
    );
}

#[test]
fn an_unset_variable_is_its_own_state_and_not_folded_into_shared() {
    // The REMEDIES differ: a direct caller should pass its own paths; a committer should
    // scope its commit. Folding them would send an operator to fix the wrong thing, which
    // is how a gate stops being trusted.
    assert_eq!(IndexScope::classify(None), IndexScope::NotUnderHook);
    assert_eq!(IndexScope::classify(None).label(), "not-under-hook");
    assert_ne!(
        IndexScope::classify(None).label(),
        IndexScope::classify(Some("/r/.git/index")).label()
    );
}

#[test]
fn every_unbuildable_scope_refuses_with_a_named_remedy_and_a_named_bypass() {
    // ASSERT ON EMITTED TEXT. A refusal that does not say what to do is one an agent
    // deletes rather than satisfies, and a bypass nobody can name is one everybody uses.
    let shared = IndexScope::classify(Some("/r/.git/index"))
        .refusal()
        .expect("the shared index must refuse");
    for needle in [
        "STAGED_BUILD_GATE_REFUSED",
        "INDEX_NOT_SCOPED",
        "git commit -- <paths>",
        "--no-verify",
        "603s",
    ] {
        assert!(
            shared.contains(needle),
            "the shared refusal must carry {needle:?}: {shared}"
        );
    }

    let direct = IndexScope::classify(None)
        .refusal()
        .expect("a direct invocation must refuse");
    assert!(direct.contains("NOT_UNDER_HOOK"), "{direct}");
    assert_ne!(
        direct, shared,
        "two different causes must not produce the same text"
    );

    // KNOWN-GOOD ARM: the buildable scope must NOT refuse. A gate that refuses everything
    // is worse than the latency it was added to prevent.
    assert_eq!(
        IndexScope::classify(Some("/r/.git/next-index-9.lock")).refusal(),
        None
    );
}

#[test]
fn the_scoped_and_shared_paths_are_carried_for_diagnosis() {
    // A refusal naming no index is one an operator cannot check. The path is carried on
    // the variant rather than re-derived at the refusal site.
    match IndexScope::classify(Some("/r/.git/index")) {
        IndexScope::Shared(path) => assert_eq!(path, "/r/.git/index"),
        other => panic!("expected Shared, got {other:?}"),
    }
    match IndexScope::classify(Some("/r/.git/next-index-7.lock")) {
        IndexScope::Scoped(path) => assert_eq!(path, "/r/.git/next-index-7.lock"),
        other => panic!("expected Scoped, got {other:?}"),
    }
}

#[test]
fn from_env_agrees_with_classify_on_the_live_variable() {
    // `from_env` is the production entry point and `classify` is what the table above
    // pins. A test that only exercised `classify` would leave the wrapper unproven — the
    // same split that let `no-shell-gate` be correct and invoked by nothing.
    let live = std::env::var("GIT_INDEX_FILE").ok();
    assert_eq!(
        IndexScope::from_env(),
        IndexScope::classify(live.as_deref()),
        "the env wrapper must not disagree with the classifier it wraps"
    );
    // And under `cargo test` the variable is unset, so this run is the NotUnderHook case —
    // stated rather than left as an accident of the harness.
    if live.is_none() {
        assert_eq!(IndexScope::from_env(), IndexScope::NotUnderHook);
    }
}

// -----------------------------------------------------------------------------------
// g5e0 — the 247-second cause: a bare `cargo` is the RCH shim on this machine
// -----------------------------------------------------------------------------------

/// MEASURED 2026-09-02 on the live checkout, after this gate was installed and reverted:
///
/// ```text
/// which cargo            -> $HOME/.rch/shims/cargo   POSIX shell script
/// $HOME/.cargo/bin/cargo -> Mach-O 64-bit executable arm64
/// cmp                 -> NOT the same file
/// rch queue           -> cargo build -p dispatch-claim-fence on contabo-4, sync_up
/// hook wall time      -> 247s for ONE warm crate
/// ```
/// Machine-independent on purpose: `is_rch_shim` keys on the `/.rch/shims/` segment,
/// and a hardcoded author-home literal is what `path-literal-guard` exists to refuse.
const MEASURED_SHIM: &str = "/home/agent/.rch/shims/cargo";

#[test]
fn a_shim_path_is_recognised_and_a_real_toolchain_is_not() {
    assert!(
        staged_build_gate::is_rch_shim(MEASURED_SHIM),
        "the measured shim path must be recognised"
    );
    // KNOWN-GOOD ARM: a real toolchain must NOT be flagged, or the resolver would refuse
    // every cargo and the gate could never build anything.
    for real in [
        "/home/agent/.cargo/bin/cargo",
        "/opt/homebrew/bin/cargo",
        "cargo",
    ] {
        assert!(
            !staged_build_gate::is_rch_shim(real),
            "{real} is a toolchain, not a shim"
        );
    }
}

#[test]
fn the_resolver_drives_all_three_arms_and_never_yields_the_shim() {
    // THE LEG THAT MUTATION FORCED. Two mutations of the env-reading wrapper did not bite,
    // because `cargo test` sets $CARGO to the real toolchain and the wrapper returned from
    // the explicit arm before reaching the fallback they changed. The pure seam lets each
    // arm be driven.
    let home = std::path::Path::new("/home/agent");
    let present = |_: &std::path::Path| true;
    let absent = |_: &std::path::Path| false;

    // ARM 1 — an explicit real toolchain wins.
    assert_eq!(
        staged_build_gate::resolve_cargo_with(Some("/opt/rust/bin/cargo"), Some(home), &present),
        "/opt/rust/bin/cargo"
    );
    // ARM 1b — an explicit SHIM is refused and falls through. `$CARGO` is set by the shim
    // itself when it re-enters cargo, so honouring it would make the offload sticky.
    assert_eq!(
        staged_build_gate::resolve_cargo_with(Some(MEASURED_SHIM), Some(home), &present),
        "/home/agent/.cargo/bin/cargo"
    );
    // ARM 2 — no explicit value, rustup layout present.
    assert_eq!(
        staged_build_gate::resolve_cargo_with(None, Some(home), &present),
        "/home/agent/.cargo/bin/cargo"
    );
    // ARM 3 — no layout at all. Honestly returns a bare name and says so in the doc.
    assert_eq!(
        staged_build_gate::resolve_cargo_with(None, Some(home), &absent),
        "cargo"
    );
    assert_eq!(staged_build_gate::resolve_cargo_with(None, None, &present), "cargo");
    // And whitespace is not a toolchain.
    assert_eq!(
        staged_build_gate::resolve_cargo_with(Some("   "), Some(home), &present),
        "/home/agent/.cargo/bin/cargo"
    );

    // NO ARM may yield the shim.
    for (explicit, h, e) in [
        (Some(MEASURED_SHIM), Some(home), &present as &dyn Fn(&std::path::Path) -> bool),
        (Some(MEASURED_SHIM), Some(home), &absent as &dyn Fn(&std::path::Path) -> bool),
        (Some(MEASURED_SHIM), None, &present as &dyn Fn(&std::path::Path) -> bool),
    ] {
        let got = staged_build_gate::resolve_cargo_with(explicit, h, e);
        assert!(!staged_build_gate::is_rch_shim(&got), "yielded {got:?}");
    }
}

#[test]
fn the_resolved_cargo_is_never_the_shim() {
    // The whole defect in one assertion. `cargo_bin` used to be
    // `env::var("CARGO").unwrap_or("cargo")`, and a bare name resolves through `PATH` to
    // the shim.
    let resolved = staged_build_gate::local_cargo();
    assert!(
        !staged_build_gate::is_rch_shim(&resolved),
        "resolved {resolved:?} is a shim; a gate must measure a LOCAL build"
    );
    // Under `cargo test` the harness sets $CARGO to the real toolchain, so this run
    // exercises the explicit-CARGO arm. Stated rather than left as an accident.
    if let Ok(explicit) = std::env::var("CARGO") {
        if !staged_build_gate::is_rch_shim(&explicit) {
            assert_eq!(resolved, explicit.trim());
        }
    }
}

#[test]
fn the_local_build_env_pins_every_offload_switch() {
    // FIRES-ON-KNOWN-BAD for the refactor that already happened: the hook's first
    // implementation pinned these at its own call site, the KERNEL-ONLY rewrite deleted
    // that call site, and no test could see the loss because the env only matters on the
    // spawn. The pins now live in the kernel and this leg names each one.
    let env = staged_build_gate::local_build_env();
    assert!(!env.is_empty(), "ANTI-VACUITY: an empty pin set pins nothing");
    let keys: std::collections::BTreeSet<&str> = env.iter().map(|(k, _)| *k).collect();
    for required in ["RCH_ENABLED", "CARGO_MINT_MIN_CONTAINER_PCT", "CARGO"] {
        assert!(keys.contains(required), "the pin set must carry {required}");
    }
    let lookup = |key: &str| -> String {
        env.iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.clone())
            .expect("present")
    };
    assert_eq!(lookup("RCH_ENABLED"), "false", "remote offload must be OFF");
    assert_eq!(
        lookup("CARGO_MINT_MIN_CONTAINER_PCT"),
        "0",
        "the gate must not refuse for disk pressure; that is a different verdict"
    );
    assert!(
        !staged_build_gate::is_rch_shim(&lookup("CARGO")),
        "the pinned CARGO must not be the shim"
    );
}

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

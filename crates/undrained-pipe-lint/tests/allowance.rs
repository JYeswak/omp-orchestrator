//! `omp-orchestrator-ury6f`: the declared self-fixture allowance, and the property that separates
//! it from a blind spot.
//!
//! A NEW FILE ON PURPOSE. `tests/specimens.rs` is carrying another agent's uncommitted `+34/-43`
//! right now, and two agents editing one file cannot both commit — the second sweeps the first.
//! These legs need no changes to the corpus, so they go somewhere nobody is holding.

use undrained_pipe_lint::{find_detailed_violations_in_source, is_self_fixture,
                          SELF_FIXTURE_ALLOWANCE};

/// ITEM 5 — ANTI-VACUITY. An allowance that grows to cover the violation CLASS is a silent
/// disable, so the list's shape is asserted, not trusted.
///
/// Every row must be an EXACT relative path with a reason. No glob, no prefix, no bare directory,
/// no extension class. A row ending in `/` or containing `*` would exempt a population.
#[test]
fn allowance_rows_are_exact_paths_with_reasons() {
    assert!(
        !SELF_FIXTURE_ALLOWANCE.is_empty(),
        "an empty allowance would make every leg below vacuous — this list exists precisely \
         because one file needs it"
    );
    assert_eq!(
        SELF_FIXTURE_ALLOWANCE.len(),
        1,
        "exactly one self-fixture is allowed today; a second row is a decision someone must \
         justify, not a silent widening"
    );
    for (path, reason) in SELF_FIXTURE_ALLOWANCE {
        assert!(
            !path.contains('*') && !path.ends_with('/'),
            "{path} exempts a POPULATION, not a file — that is a blind spot wearing an \
             allowance's name"
        );
        assert!(
            path.ends_with(".rs"),
            "{path} must name a source file exactly"
        );
        assert!(
            reason.len() > 40,
            "{path} carries no real reason; a named row with an empty reason is silence with \
             extra steps"
        );
    }
}

/// ITEM 3 — FIRES ON A REAL VIOLATION IN A NON-FIXTURE TEST FILE.
///
/// This is the leg the `cfg(test)`-region exclusion FAILS, which is why that remedy was rejected.
/// A blanket test-region skip makes every path below return `true`; an exact-path allowance makes
/// only the one named row return `true`.
#[test]
fn a_real_violation_in_another_test_file_is_still_linted() {
    // THE SPECIMEN IS ASSEMBLED FROM FRAGMENTS, and the reason is a finding.
    //
    // My first version wrote it as one multi-line string literal -- "fixtures as data", the remedy
    // this bead calls strongest because "data cannot be flagged and code can". THE DETECTOR
    // REFUSED THIS FILE: "allowance.rs stdout-piped at line 56, stderr-piped at line 57, try_wait
    // poll at line 60", and those lines were INSIDE the quoted string. It does not mask string
    // literals; it matches LINES. So a multi-line quoted specimen trips it exactly like code, and
    // the existing quoted rows in specimens.rs survive only because each is a SINGLE line that
    // does not carry the pipe/poll trio.
    //
    // Splitting the fragments is the same idiom path-literal-guard uses on its own needle: no
    // source line here contains a matchable form, so this file needs no allowance of its own --
    // which is the property a fires-on-known-bad leg must have.
    let piped_out = concat!(".stdout(Stdio", "::piped())");
    let piped_err = concat!(".stderr(Stdio", "::piped())");
    let poll = concat!("let _ = child.try", "_wait();");
    let genuine = format!(
        "fn poll_without_draining() {{\n\
         let mut child = Command::new(\"sh\")\n\
         {piped_out}\n\
         {piped_err}\n\
         .spawn()\n\
         .unwrap();\n\
         {poll}\n\
         }}\n"
    );

    // POSITIVE CONTROL on the detector itself: the specimen must actually be a violation, or
    // every "still refused" claim below is about an input that was never bad.
    let found = find_detailed_violations_in_source(&genuine);
    assert!(
        !found.is_empty(),
        "the planted specimen must be a REAL both-pipes-plus-poll violation, or this leg proves \
         nothing about the allowance"
    );

    // Sibling in the SAME tests/ directory as the allowed fixture — not allowed.
    assert!(
        !is_self_fixture("crates/undrained-pipe-lint/tests/allowance.rs"),
        "a sibling test file must NOT inherit the fixture's allowance"
    );
    // A near-miss name — not allowed.
    assert!(
        !is_self_fixture("crates/undrained-pipe-lint/tests/specimens_extra.rs"),
        "a longer name sharing the allowed prefix must NOT be allowed"
    );
    // Same file name in a different crate — not allowed.
    assert!(
        !is_self_fixture("crates/state-wildcard-lint/tests/specimens.rs"),
        "the same basename in another crate must NOT be allowed"
    );
    // A test file anywhere else — not allowed.
    assert!(
        !is_self_fixture("crates/omp-orchestrator/tests/phase_gate_wiring.rs"),
        "an unrelated test file must NOT be allowed"
    );
}

/// The allowed row IS allowed — the positive arm, without which the four negatives above pass for
/// a predicate stuck on `false` and the block is not actually lifted.
#[test]
fn the_named_fixture_is_allowed_and_the_predicate_is_not_stuck_on_false() {
    assert!(
        is_self_fixture("crates/undrained-pipe-lint/tests/specimens.rs"),
        "the named fixture must be allowed, or ury6f's block is not lifted"
    );
    assert!(
        is_self_fixture("crates\\undrained-pipe-lint\\tests\\specimens.rs"),
        "separator normalisation must hold, or the allowance silently fails on one platform \
         and the block returns there"
    );
}

/// The allowance is a REPORTING exemption, never a claim that the file is clean.
///
/// The corpus still contains real violations by construction — that is what a known-bad corpus IS.
/// If this ever returns empty, the fixture has been gutted and the detector has nothing to prove
/// itself against, which is a worse failure than the block this bead fixes.
#[test]
fn the_allowed_fixture_still_contains_violations_when_read_directly() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/specimens.rs");
    let source = std::fs::read_to_string(path).expect("the corpus must be readable");
    let found = find_detailed_violations_in_source(&source);
    assert!(
        !found.is_empty(),
        "the allowed corpus must STILL contain violations — the allowance suppresses the \
         refusal, it does not clean the file, and a gutted corpus is a silently disabled detector"
    );
}

#![forbid(unsafe_code)]

//! `omp-orchestrator-249hz` legs for the STAGED-BLOB reader, kept out of `specimens.rs`
//! because that file carries another agent's uncommitted reformatting this wave and a
//! shared checkout is not a place to co-edit a file nobody claimed.
//!
//! PURE-LOGIC and SHAPE-SENSITIVE targets are mixed here deliberately and labelled:
//! `lint_sources_keeps_scope_...` touches no filesystem (one box suffices);
//! `the_index_reader_and_the_worktree_reader_disagree...` builds its OWN git repository
//! in a temp dir, so it depends on `git` existing but not on the worker's tree shape --
//! which is why it passes on a box whose project copy has no `.git` at all.

/// THE LEG THE REWIRE EXISTS FOR (`omp-orchestrator-249hz`, sixth instance): the two
/// readers DISAGREE on a divergent tree, and the commit is made of the INDEX.
///
/// A real fixture repository, not a mock: a state-wildcard arm is STAGED, then the
/// worktree copy is repaired. That is the ordinary `git add` / keep-fixing / pathless
/// `git commit` sequence, and it is the FALSE GREEN direction -- the violating arm lands
/// while the gate reads the repaired file. The converse (worktree dirty, index clean)
/// is the false red that refuses a commit for bytes it does not carry; both are asserted
/// here so a fix in one direction cannot hide a regression in the other.
#[test]
fn the_index_reader_and_the_worktree_reader_disagree_and_the_index_is_the_commit() {
    let root = std::env::temp_dir().join(format!(
        "swl-index-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("clear stale fixture");
    }
    let src = root.join("crates/example/src");
    std::fs::create_dir_all(&src).expect("fixture tree");

    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .output()
            .expect("git runs on the test host");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "fixture@test"]);
    git(&["config", "user.name", "fixture"]);

    const VIOLATING: &str = "enum PaneState { Idle, Busy }\nfn check(input: PaneState) {\n    match input {\n        PaneState::Idle => (),\n        _ => (),\n    }\n}\n";
    const REPAIRED: &str = "enum PaneState { Idle, Busy }\nfn check(input: PaneState) {\n    match input {\n        PaneState::Idle => (),\n        PaneState::Busy => (),\n    }\n}\n";

    let file = src.join("lib.rs");
    std::fs::write(&file, VIOLATING).expect("stage the violation");
    git(&["add", "-A"]);
    // Divergence: the index carries the wildcard, the worktree no longer does.
    std::fs::write(&file, REPAIRED).expect("repair the worktree only");

    let staged = String::from_utf8(git(&["show", ":crates/example/src/lib.rs"]))
        .expect("staged blob is UTF-8");
    assert_ne!(
        staged,
        std::fs::read_to_string(&file).expect("worktree read"),
        "premise: the two trees must actually differ here"
    );

    let from_index =
        state_wildcard_lint::lint_sources(&[("crates/example/src/lib.rs", staged.as_str())]);
    assert_eq!(
        from_index.verdict(),
        state_wildcard_lint::Verdict::Violation,
        "the INDEX carries the wildcard and the commit is made of the index: {from_index:?}"
    );
    assert_eq!(from_index.findings.len(), 1, "{:?}", from_index.findings);

    let from_worktree = state_wildcard_lint::lint_paths(&root, &["crates/example/src/lib.rs"]);
    assert_eq!(
        from_worktree.verdict(),
        state_wildcard_lint::Verdict::Clean,
        "CONTROL, and it is the defect: the worktree reader passes the very commit that \
         lands the wildcard -- {from_worktree:?}"
    );

    // FALSE-RED DIRECTION: index clean, worktree dirty. The commit carries nothing to
    // refuse, and a worktree reader would refuse it anyway.
    std::fs::write(&file, REPAIRED).expect("clean content");
    git(&["add", "-A"]);
    std::fs::write(&file, VIOLATING).expect("dirty the worktree only");
    let clean_index = String::from_utf8(git(&["show", ":crates/example/src/lib.rs"]))
        .expect("staged blob is UTF-8");
    assert_eq!(
        state_wildcard_lint::lint_sources(&[("crates/example/src/lib.rs", clean_index.as_str())])
            .verdict(),
        state_wildcard_lint::Verdict::Clean,
        "a commit that carries no wildcard must not be refused for unstaged bytes"
    );
    assert_eq!(
        state_wildcard_lint::lint_paths(&root, &["crates/example/src/lib.rs"]).verdict(),
        state_wildcard_lint::Verdict::Violation,
        "CONTROL for the converse: the worktree reader refuses a clean commit"
    );

    std::fs::remove_dir_all(&root).ok();
}

/// SCOPE STAYS IN ONE PLACE, and an EMPTY set is NOTHING-TO-CHECK, never Clean.
///
/// `lint_sources` applies `is_in_scan_scope` itself rather than trusting the caller, so
/// the staged and repo-wide modes cannot drift on which files count. The empty verdict is
/// the anti-vacuity arm: a commit staging only `AGENTS.md` leaves this gate zero eligible
/// sources, and reporting that as Clean is the vacuous-green inversion this crate names.
#[test]
fn lint_sources_keeps_scope_and_distinguishes_nothing_to_check_from_clean() {
    const VIOLATING: &str = "enum PaneState { Idle, Busy }\nfn check(input: PaneState) {\n    match input {\n        PaneState::Idle => (),\n        _ => (),\n    }\n}\n";

    let out_of_scope =
        state_wildcard_lint::lint_sources(&[("docs/plan/00-brief.md", VIOLATING)]);
    assert_eq!(
        out_of_scope.verdict(),
        state_wildcard_lint::Verdict::NothingToCheck,
        "a non-.rs path is not in scope, and an empty eligible set is NOT Clean: {out_of_scope:?}"
    );
    assert!(
        out_of_scope.scanned.is_empty(),
        "an out-of-scope source must not be counted as scanned: {:?}",
        out_of_scope.scanned
    );

    let empty: &[(&str, &str)] = &[];
    assert_eq!(
        state_wildcard_lint::lint_sources(empty).verdict(),
        state_wildcard_lint::Verdict::NothingToCheck,
        "zero sources is nothing-to-check"
    );

    let in_scope =
        state_wildcard_lint::lint_sources(&[("crates/example/src/lib.rs", VIOLATING)]);
    assert_eq!(
        in_scope.verdict(),
        state_wildcard_lint::Verdict::Violation,
        "POSITIVE CONTROL: the same bytes under an in-scope name must fire, or the two \
         assertions above prove only that the scanner is broken"
    );
}

#![forbid(unsafe_code)]

//! `omp-orchestrator-249hz` legs for GATE 2/3's STAGED-BLOB reader.
//!
//! THE DEFECT: `scan_paths` selects the STAGED SET and reads the WORKTREE. The dangerous
//! direction is the FALSE GREEN -- a home-path literal staged, then repaired in the
//! worktree, lands while the gate reports CLEAN.
//!
//! THE NEEDLE IS CONSTRUCTED, NEVER SPELLED, exactly as this crate's own source and its
//! sibling suites do: `crates/path-literal-guard/tests` is inside the scan scope this gate
//! enforces, so a literal written out here would make the gate refuse its own test file.
//!
//! PER-TARGET CLASS: `scan_sources_keeps_scope...` is PURE-LOGIC, no filesystem, one box
//! suffices. `the_index_reader_and_the_worktree_reader_disagree...` builds its OWN git
//! repository in a temp dir, so it needs `git` but NOT a worker whose project copy is a
//! repository -- which is why it runs on the box that has no `.git` at all.

use path_literal_guard::{scan_paths, scan_sources, ScanMode, Verdict, USER_HOME_LITERAL};

fn planted_source() -> String {
    format!("pub const REPO: &str = \"{USER_HOME_LITERAL}/Developer/x\";\n")
}

fn clean_source() -> &'static str {
    "pub fn repo() -> std::path::PathBuf {\n    std::env::current_dir().expect(\"cwd\")\n}\n"
}

/// THE LEG THE REWIRE EXISTS FOR: the two readers disagree on a divergent tree, and the
/// commit is made of the INDEX. Both directions are pinned so a fix in one cannot hide a
/// regression in the other.
#[test]
fn the_index_reader_and_the_worktree_reader_disagree_and_the_index_is_the_commit() {
    let root = std::env::temp_dir().join(format!(
        "plg-index-{}-{}",
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

    let file = src.join("lib.rs");
    std::fs::write(&file, planted_source()).expect("stage the literal");
    git(&["add", "-A"]);
    // Divergence: the index carries the literal, the worktree no longer does.
    std::fs::write(&file, clean_source()).expect("repair the worktree only");

    let staged =
        String::from_utf8(git(&["show", ":crates/example/src/lib.rs"])).expect("blob is UTF-8");
    assert_ne!(
        staged,
        std::fs::read_to_string(&file).expect("worktree read"),
        "premise: the two trees must actually differ here"
    );

    let from_index = scan_sources(&[("crates/example/src/lib.rs", staged.as_str())]);
    assert_eq!(
        from_index.verdict(),
        Verdict::Violation,
        "the INDEX carries the literal and the commit is made of the index: {from_index:?}"
    );
    assert_eq!(from_index.hits.len(), 1, "{:?}", from_index.hits);
    assert_eq!(from_index.hits[0].line, 1);

    let from_worktree = scan_paths(&root, &["crates/example/src/lib.rs"]);
    assert_eq!(
        from_worktree.verdict(),
        Verdict::Clean,
        "CONTROL, and it is the defect: the worktree reader passes the very commit that \
         lands the literal -- {from_worktree:?}"
    );

    // FALSE-RED DIRECTION: index clean, worktree dirty.
    std::fs::write(&file, clean_source()).expect("clean content");
    git(&["add", "-A"]);
    std::fs::write(&file, planted_source()).expect("dirty the worktree only");
    let clean_index =
        String::from_utf8(git(&["show", ":crates/example/src/lib.rs"])).expect("blob is UTF-8");
    assert_eq!(
        scan_sources(&[("crates/example/src/lib.rs", clean_index.as_str())]).verdict(),
        Verdict::Clean,
        "a commit that carries no literal must not be refused for unstaged bytes"
    );
    assert_eq!(
        scan_paths(&root, &["crates/example/src/lib.rs"]).verdict(),
        Verdict::Violation,
        "CONTROL for the converse: the worktree reader refuses a clean commit"
    );

    std::fs::remove_dir_all(&root).ok();
}

/// SCOPE STAYS IN ONE PLACE, an out-of-scope path is RECORDED rather than dropped, and an
/// empty eligible set is NOTHING-TO-CHECK, never Clean.
#[test]
fn scan_sources_keeps_scope_and_records_what_it_did_not_read() {
    let out_of_scope = scan_sources(&[("docs/plan/00-brief.md", planted_source().as_str())]);
    assert_eq!(
        out_of_scope.verdict(),
        Verdict::NothingToCheck,
        "a non-src path is out of scope, and an empty eligible set is NOT Clean: \
         {out_of_scope:?}"
    );
    assert_eq!(
        out_of_scope.skipped.len(),
        1,
        "an out-of-scope path must be RECORDED, never silently dropped: {:?}",
        out_of_scope.skipped
    );
    assert!(
        out_of_scope.scanned.is_empty(),
        "and it must not be counted as scanned: {:?}",
        out_of_scope.scanned
    );

    let empty: &[(&str, &str)] = &[];
    assert_eq!(scan_sources(empty).verdict(), Verdict::NothingToCheck);

    let in_scope = scan_sources(&[("crates/example/src/lib.rs", planted_source().as_str())]);
    assert_eq!(
        in_scope.verdict(),
        Verdict::Violation,
        "POSITIVE CONTROL: the same bytes under an in-scope name must fire, or the two \
         assertions above prove only that the scanner is broken"
    );
    assert_eq!(in_scope.mode, ScanMode::StagedPaths);
}

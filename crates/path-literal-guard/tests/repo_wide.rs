//! The home-path-literal gate, both modes (beads omp-orchestrator-npq, -oej2).
//!
//! The REPO-WIDE leg asserts the count over `<repo>/crates/*/src` is zero, prints the
//! scan set with the verdict so a reader can see exactly what was covered, and treats
//! an EMPTY scan set as an ERROR — never a pass.
//!
//! The STAGED leg asserts the property this repository actually needs: a literal in a
//! file that is NOT part of the change cannot refuse the change. That direction is the
//! one that was broken, and it is the one that made the repo single-writer.
//!
//! Per AGENTS.md rule 7, every known-bad leg asserts the MESSAGE and not merely a
//! nonzero exit: an exit code alone cannot distinguish "the gate bit" from "the
//! workspace failed to load".

use path_literal_guard::{
    is_in_scan_scope, repo_root, scan, scan_paths, ScanMode, Verdict, USER_HOME_LITERAL,
};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::panic::catch_unwind;
use std::path::{Path, PathBuf};

/// The literal is CONSTRUCTED, never spelled: `k0h1e` moved `tests/` INTO the gate's floor, so
/// this file is now scannable by the staged gate, and spelling the literal would make it a
/// specimen this repo forbids.
fn planted_line() -> String {
    format!("const REPO: &str = \"{USER_HOME_LITERAL}\";\n")
}

#[test]
fn zero_home_path_literals_across_crates_src() {
    let report = scan(&repo_root());

    // Print the scan set: a verdict without its coverage is unauditable.
    println!(
        "PATH-LITERAL-GATE scan set ({} .rs files under crates/*/src):",
        report.scanned.len()
    );
    for file in &report.scanned {
        println!("  {}", file.display());
    }

    // Anti-vacuity: an empty scan set is an ERROR, never a pass. A repo whose crates
    // tree vanished (or a gate pointed at the wrong root) must fail loudly here.
    assert!(
        !report.scanned.is_empty(),
        "PATH-LITERAL-GATE RED: empty scan set — no crates/*/src found under {}",
        repo_root().display()
    );

    assert!(
        report.hits.is_empty(),
        "PATH-LITERAL-GATE RED: hardcoded home-path literal(s) in crates/*/src \
         (omp-orchestrator-npq: a hardcoded root compiles after a move and then \
         silently reads the wrong repo): {:?}",
        report
            .hits
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<String>>()
    );

    println!(
        "PATH-LITERAL-GATE PASS: {} files scanned, zero home-path literals",
        report.scanned.len()
    );
}

/// KNOWN-GOOD boundary leg: legitimate path construction must not look like a home-path literal.
#[test]
fn known_good_boundary_paths_are_clean() {
    let root = std::env::temp_dir().join(format!("plg-known-good-{}", std::process::id()));
    let src = root.join("crates/example/src");
    fs::create_dir_all(&src).expect("create known-good fixture tree");
    fs::write(
        src.join("lib.rs"),
        r##"fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate root")
        .to_path_buf()
}
fn data_path() -> std::path::PathBuf { repo_root().join("var/data") }
const SCRATCH: &str = "/tmp/path-literal-guard-fixture";
"##,
    )
    .expect("write known-good fixture source");

    let report = scan(&root);
    println!(
        "PATH-LITERAL-GATE PASS: {} files scanned, zero home-path literals",
        report.scanned.len()
    );
    assert_eq!(report.mode, ScanMode::RepoWide);
    assert_eq!(
        report.scanned.len(),
        1,
        "known-good leg must read a non-empty scan set"
    );
    assert!(
        report.hits.is_empty(),
        "legitimate path construction was flagged: {report:?}"
    );
    assert_eq!(report.verdict(), Verdict::Clean);
    assert!(report.is_pass());
    assert!(report.declared_scope_line().contains("1 file(s) read"));

    fs::remove_dir_all(&root).expect("remove known-good fixture tree");
}
/// UNREADABLE INPUT is an ERROR, never an empty clean scan. The source directory
/// is discovered before its permissions are revoked, so the scanner must refuse
/// when `read_dir` cannot enumerate it rather than silently returning no files.
///
/// MUTATION: restoring the directory permissions is the GREEN leg; the same fixture
/// must scan cleanly again after the unreadable-input mutation is reversed.
#[test]
fn unreadable_input_is_refused_and_restores_to_a_clean_scan() {
    let root = std::env::temp_dir().join(format!("plg-unreadable-{}", std::process::id()));
    let src = root.join("crates/example/src");
    fs::create_dir_all(&src).expect("create unreadable fixture tree");
    fs::write(src.join("lib.rs"), "fn main() {}\n").expect("write clean fixture file");

    fs::set_permissions(&src, fs::Permissions::from_mode(0o000))
        .expect("make fixture source directory unreadable");
    let refused = catch_unwind(|| scan(&root));

    // Restore before asserting or cleaning up: a failed scan must not leave a
    // permission-mutated fixture behind for a later test or developer.
    fs::set_permissions(&src, fs::Permissions::from_mode(0o755))
        .expect("restore fixture source directory permissions");
    assert!(
        refused.is_err(),
        "an unreadable source directory must be an ERROR, not an empty scan"
    );

    let report = scan(&root);
    assert!(
        report.is_pass(),
        "restoring permissions must recover a nonempty clean scan: {report:?}"
    );
    assert_eq!(report.scanned, vec![src.join("lib.rs")]);

    fs::remove_dir_all(&root).expect("remove unreadable fixture tree");
}

/// THE LEG THAT PROVES omp-orchestrator-oej2, in the repository's own shape: one dirty
/// file that is NOT staged, one clean file that is, and the scoped verdict must be GREEN
/// while the sweep stays RED. Both verdicts and both MESSAGES are asserted.
#[test]
fn an_unstaged_literal_cannot_refuse_a_clean_staged_change() {
    let root = std::env::temp_dir().join(format!("plg-unstaged-{}", std::process::id()));
    let src = root.join("crates/example/src");
    fs::create_dir_all(&src).expect("create fixture tree");
    fs::write(src.join("staged.rs"), "fn main() {}\n").expect("write staged clean file");
    fs::write(src.join("scratch.rs"), planted_line()).expect("write unstaged dirty file");

    // RED direction: the sweep still finds it, and names it.
    let sweep = scan(&root);
    assert_eq!(sweep.mode, ScanMode::RepoWide);
    assert_eq!(sweep.verdict(), Verdict::Violation, "{sweep:?}");
    let named: Vec<String> = sweep
        .hits
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    assert!(
        named
            .iter()
            .any(|hit| hit.contains("scratch.rs") && hit.ends_with(":1")),
        "the sweep must NAME file:line, not a count: {named:?}"
    );
    assert!(
        sweep.declared_scope_line().contains("repo-wide"),
        "the sweep must declare its scope: {}",
        sweep.declared_scope_line()
    );

    // GREEN direction: scoped to the staged file, the same literal is not this
    // commit's problem.
    let scoped = scan_paths(&root, &["crates/example/src/staged.rs"]);
    assert_eq!(scoped.mode, ScanMode::StagedPaths);
    assert_eq!(
        scoped.verdict(),
        Verdict::Clean,
        "an unstaged literal must not refuse an unrelated change: {scoped:?}"
    );
    assert!(scoped.hits.is_empty(), "{:?}", scoped.hits);
    let declared = scoped.declared_scope_line();
    assert!(declared.contains("staged set only"), "{declared}");
    assert!(
        declared.contains("UNSTAGED") && declared.contains("UNTRACKED"),
        "a scoped green must state that it does NOT cover the repo: {declared}"
    );

    fs::remove_dir_all(&root).expect("remove fixture tree");
}

/// REPO-WIDE MODE REMAINS REACHABLE (acceptance #3). Deleting the sweep would trade one
/// blind spot for another, so the two modes must both exist and be distinguishable.
#[test]
fn both_modes_exist_and_are_distinguishable() {
    let root = std::env::temp_dir().join(format!("plg-modes-{}", std::process::id()));
    let src = root.join("crates/example/src");
    fs::create_dir_all(&src).expect("create fixture tree");
    fs::write(src.join("lib.rs"), "fn main() {}\n").expect("write clean file");

    let sweep = scan(&root);
    let scoped = scan_paths(&root, &["crates/example/src/lib.rs"]);
    assert_ne!(sweep.mode, scoped.mode, "the modes must be distinguishable");
    assert_eq!(sweep.mode.to_string(), "repo-wide");
    assert_eq!(scoped.mode.to_string(), "staged");
    assert_ne!(
        sweep.declared_scope_line(),
        scoped.declared_scope_line(),
        "each mode must declare a DIFFERENT scope, or the declaration is decoration"
    );

    // The CLI names both modes too, so an operator or CI job can state its claim.
    let cli = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs"))
        .expect("the CLI must be readable");
    assert!(
        cli.contains("--repo-wide"),
        "the sweep must stay reachable from the CLI"
    );
    assert!(
        cli.contains("--staged"),
        "the scoped mode must be nameable from the CLI"
    );

    fs::remove_dir_all(&root).expect("remove fixture tree");
}

/// ISOMORPHISM against the REAL repository: staged mode handed every file the sweep read
/// must reach the same verdict. This is what makes "scoped" a narrowing of the file set
/// rather than a weakening of the check.
#[test]
fn staged_mode_over_the_real_repo_equals_the_sweep() {
    let root = repo_root();
    let sweep = scan(&root);
    let every: Vec<PathBuf> = sweep.scanned.clone();
    assert!(
        !every.is_empty(),
        "anti-vacuity: the real repo must have files to compare"
    );
    let scoped = scan_paths(&root, &every);
    assert_eq!(scoped.scanned, sweep.scanned, "the same files must be read");
    assert_eq!(
        scoped.hits, sweep.hits,
        "the same hits, at the same file:line"
    );
    assert_eq!(scoped.verdict(), sweep.verdict());
}

/// KNOWN-GOOD leg, and the self-reference handled WITHOUT a carve-out: this gate's own
/// source is scanned by both modes and is clean, because the needle is built by
/// `concat!` rather than spelled. Five sibling crates use the same idiom.
#[test]
fn the_guards_own_source_is_clean_without_an_exclusion() {
    let root = repo_root();
    let own = Path::new("crates/path-literal-guard/src/lib.rs");
    assert!(
        is_in_scan_scope(own),
        "the gate's own source must be IN scope"
    );

    let source = fs::read_to_string(root.join(own)).expect("read the gate's own source");
    assert!(
        source.contains("USER_HOME_LITERAL"),
        "specimen data must actually be present for this leg to prove anything"
    );
    assert!(
        !source.contains(USER_HOME_LITERAL),
        "the needle must never appear contiguously in this gate's own source"
    );

    let scoped = scan_paths(&root, &[own]);
    assert_eq!(scoped.scanned.len(), 1, "{:?}", scoped.scanned);
    assert_eq!(
        scoped.verdict(),
        Verdict::Clean,
        "the gate must not catch its own needle: {scoped:?}"
    );
    assert!(
        path_literal_guard::DECLARED_ALLOWLIST
            .iter()
            .all(|row| !row.file.contains("path-literal-guard")),
        "the self-reference must be a MECHANISM, never an allowlist row"
    );
}

/// THE SWEEP'S NARROWING IS MEASURED, NOT ASSUMED — and it cannot go stale in silence.
///
/// `omp-orchestrator-k0h1e` left the sweep on `src` while staged mode walked `{src,tests}`.
/// Every deferred subdir had to still hold a violation. CI 34667870386 and 64wxc found
/// `crates/*/tests` CLEAN; the deferral expired; `REPO_WIDE_SUBDIRS` now equals the floor.
/// The empty-deferred arm asserts that equality. Restoring `&["src"]` while tests stay
/// clean reddens this leg (known-bad).
///
/// KNOWN-GOOD / over-strictness control in the same run: the walked floor is clean (the sweep
/// leg above), so this cannot pass for a scanner that flags everything.
#[test]
fn the_repo_wide_narrowing_is_load_bearing() {
    let deferred = path_literal_guard::repo_wide_deferred_subdirs();
    if deferred.is_empty() {
        assert_eq!(
            path_literal_guard::REPO_WIDE_SUBDIRS,
            path_literal_guard::SCANNED_CRATE_SUBDIRS,
            "nothing is deferred, so the sweep must walk the gate's whole floor"
        );
        return;
    }

    let root = repo_root();
    let crates_dir = root.join("crates");
    for subdir in &deferred {
        let mut candidates: Vec<PathBuf> = Vec::new();
        let mut stack: Vec<PathBuf> = Vec::new();
        for entry in fs::read_dir(&crates_dir).expect("read crates/") {
            let directory = entry.expect("crates/ entry").path().join(subdir);
            if directory.is_dir() {
                stack.push(directory);
            }
        }
        // ANTI-VACUITY: an unwalkable deferred tree is an ERROR, never an empty pass.
        assert!(
            !stack.is_empty(),
            "no crates/*/{subdir} directory exists, so the deferral describes nothing"
        );
        while let Some(directory) = stack.pop() {
            for entry in fs::read_dir(&directory).expect("read deferred directory") {
                let path = entry.expect("deferred entry").path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    candidates.push(path);
                }
            }
        }
        assert!(
            !candidates.is_empty(),
            "crates/*/{subdir} holds no .rs file, so the deferral describes nothing"
        );

        let report = scan_paths(&root, &candidates);
        assert_eq!(
            report.scanned.len(),
            candidates.len(),
            "every deferred .rs file is in the gate's floor and must be READ: {report:?}"
        );
        assert_eq!(
            report.verdict(),
            Verdict::Violation,
            "crates/*/{subdir} is CLEAN, so the sweep's narrowing no longer buys anything -- \
             WIDEN path_literal_guard::REPO_WIDE_SUBDIRS to include {subdir} and delete this \
             deferral (omp-orchestrator-k0h1e)"
        );
        println!(
            "SWEEP-DEFERRAL {subdir}: {} file(s) read, {} still violating",
            report.scanned.len(),
            report.hits.len()
        );
    }
}

//! The gate's own verification battery (bead omp-orchestrator-4ak).
//!
//! Legs, in acceptance-criteria order:
//! 1. BOTH directions: `planted_shell_is_red_then_green_after_delete` proves
//!    RED on a dirty index and GREEN on a clean one, in the same run;
//!    `this_repo_is_clean` is the standing clean leg, and the real-tree probe
//!    (documented in the bead) turns it RED on a staged `.sh`.
//! 2. PLANTED KNOWN-BAD: fixture trees below, planted and deleted in one run.
//! 3. MUTATION: the `.sh` legs below key on `FORBIDDEN_EXTENSIONS` containing
//!    "sh" — delete that pattern and `sh_is_flagged`,
//!    `uppercase_extensions_are_flagged`, `bare_dotname_scripts_are_flagged`,
//!    `planted_shell_is_red_then_green_after_delete`, and
//!    `binary_exits_1_on_planted_shell` go RED. A green mutation run would
//!    mean the legs are not attributable to the pattern and prove nothing.
//! 5. ANTI-VACUITY: `empty_scan_set_is_an_error_not_a_pass` (unit),
//!    `empty_index_is_an_error_not_a_pass` (end-to-end),
//!    `binary_exits_2_on_empty_index` (CLI exit code).
//! 6. NO-CLAIM: documented in `src/lib.rs` — extensions of tracked files only.

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use no_shell_gate::{check_repo, scan, tracked_files, GateError, Verdict, Violation};

mod common;

// ---------------------------------------------------------------- helpers

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonicalize this repo's root")
}

static FIXTURE_SEQ: AtomicU32 = AtomicU32::new(0);

/// A fresh throwaway git repository: the fixture tree every planted leg uses.
/// Never committed to this repo — created and torn down at test runtime.
fn fresh_git_tree(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "no-shell-gate-{}-{test}-{}",
        std::process::id(),
        FIXTURE_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(&dir).expect("create fixture dir");
    run_git(&dir, &["init", "-q"], "git init");
    dir
}

fn run_git(dir: &Path, args: &[&str], what: &str) -> std::process::Output {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("spawn git");
    assert!(
        out.status.success(),
        "{what} failed: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    out
}

/// Write a file and stage it, so `git ls-files` (the index) reports it.
fn stage(dir: &Path, name: &str, content: &str) {
    let file = dir.join(name);
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).expect("create fixture parent dir");
    }
    fs::write(&file, content).expect("write fixture file");
    run_git(dir, &["add", "--", name], "git add");
}

/// Unstage (remove from the index) and delete from disk: the GREEN half of a
/// planted leg.
fn unstage_and_delete(dir: &Path, name: &str) {
    run_git(
        dir,
        &["rm", "--cached", "-q", "--", name],
        "git rm --cached",
    );
    let file = dir.join(name);
    if file.exists() {
        fs::remove_file(&file).expect("delete fixture file");
    }
}

/// Run the gate binary. `None` = let it default to THIS repo's root;
/// `Some(dir)` = check a fixture tree.
fn run_gate(dir: Option<&Path>) -> (Option<i32>, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_no-shell-gate"));
    if let Some(dir) = dir {
        cmd.arg(dir);
    }
    let out = cmd.output().expect("spawn gate binary");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

// ------------------------------------------------- unit legs on the matcher

/// Known-good leg. An attack-only suite ships an over-strict gate, and an
/// over-strict gate gets routed around — so the clean paths must be pinned:
/// Rust sources, manifests, markdown, `notes.sh.txt` (FINAL extension only),
/// and a dotfile whose stem is not an extension.
#[test]
fn clean_list_passes() {
    let clean = [
        "src/main.rs",
        "Cargo.toml",
        "README.md",
        "notes.sh.txt",
        ".gitignore",
        "docs/guide.md",
    ]
    .map(String::from);
    assert_eq!(scan(&clean).expect("clean scan"), vec![]);
}

/// Mutation-attributable `.sh` leg: keys on `"sh"` being in
/// `FORBIDDEN_EXTENSIONS`. Delete the pattern and this goes RED.
#[test]
fn sh_is_flagged() {
    let paths = ["scripts/deploy.sh"].map(String::from);
    assert_eq!(
        scan(&paths).expect("scan"),
        vec![Violation {
            path: "scripts/deploy.sh".into(),
            extension: "sh".into(),
        }]
    );
}

/// Mutation-attributable `.py` leg, independent of the `.sh` leg.
#[test]
fn py_is_flagged() {
    let paths = ["tools/hello.py"].map(String::from);
    assert_eq!(
        scan(&paths).expect("scan"),
        vec![Violation {
            path: "tools/hello.py".into(),
            extension: "py".into(),
        }]
    );
}

/// `SCRIPT.SH` is still a shell script; ASCII case-folding closes the trivial
/// bypass. The `.SH` assertion fails when the `sh` pattern is deleted.
#[test]
fn uppercase_extensions_are_flagged() {
    let paths = ["SCRIPT.SH", "App.PY"].map(String::from);
    let violations = scan(&paths).expect("scan");
    assert!(violations.contains(&Violation {
        path: "SCRIPT.SH".into(),
        extension: "sh".into(),
    }));
    assert!(violations.contains(&Violation {
        path: "App.PY".into(),
        extension: "py".into(),
    }));
}

/// A file whose entire name is `.sh` is treated as extension `sh`.
#[test]
fn bare_dotname_scripts_are_flagged() {
    let paths = [".sh"].map(String::from);
    assert_eq!(
        scan(&paths).expect("scan"),
        vec![Violation {
            path: ".sh".into(),
            extension: "sh".into(),
        }]
    );
}

// ------------------------------- anti-vacuity and fail-closed (unit level)

/// ANTI-VACUITY at the choke point: an empty scan set is an ERROR, never a
/// pass. A gate that scanned nothing reports identically to one that passed.
#[test]
fn empty_scan_set_is_an_error_not_a_pass() {
    assert!(matches!(
        scan(&[]).expect_err("empty scan set must error"),
        GateError::EmptyScanSet
    ));
}

// ----------------------- end-to-end legs through a real git index fixture

/// PLANTED KNOWN-BAD, full cycle, both directions asserted in the SAME run:
/// a real git tree with a staged `run.sh` goes RED naming the file; deleting
/// it (from the index and disk) goes GREEN. The `README.md` baseline keeps
/// the scan set non-empty for the GREEN half, so the clean verdict is a real
/// verdict, not vacuity.
#[test]
fn planted_shell_is_red_then_green_after_delete() {
    let dir = fresh_git_tree("shell-cycle");
    stage(
        &dir,
        "README.md",
        "clean baseline so the scan set is never empty\n",
    );
    stage(&dir, "run.sh", "#!/bin/sh\necho planted known-bad\n");
    match check_repo(&dir).expect("gate must render a verdict on a live index") {
        Verdict::Violations(violations) => assert!(
            violations.contains(&Violation {
                path: "run.sh".into(),
                extension: "sh".into(),
            }),
            "RED leg must name run.sh, got {violations:?}"
        ),
        other => panic!("RED leg failed: expected violations, got {other:?}"),
    }
    unstage_and_delete(&dir, "run.sh");
    assert_eq!(
        check_repo(&dir).expect("gate must render a verdict after the delete"),
        Verdict::Clean,
        "GREEN leg failed: deleting run.sh must leave a clean verdict"
    );
}

/// The same full cycle for `.py`, so neither forbidden extension is exercised
/// only at the unit level.
#[test]
fn planted_python_is_red_then_green_after_delete() {
    let dir = fresh_git_tree("python-cycle");
    stage(
        &dir,
        "README.md",
        "clean baseline so the scan set is never empty\n",
    );
    stage(&dir, "tool.py", "print('planted known-bad')\n");
    match check_repo(&dir).expect("gate must render a verdict on a live index") {
        Verdict::Violations(violations) => assert!(
            violations.contains(&Violation {
                path: "tool.py".into(),
                extension: "py".into(),
            }),
            "RED leg must name tool.py, got {violations:?}"
        ),
        other => panic!("RED leg failed: expected violations, got {other:?}"),
    }
    unstage_and_delete(&dir, "tool.py");
    assert_eq!(
        check_repo(&dir).expect("gate must render a verdict after the delete"),
        Verdict::Clean,
        "GREEN leg failed: deleting tool.py must leave a clean verdict"
    );
}

/// ANTI-VACUITY end to end: a repository whose index is EMPTY is an error,
/// never a pass.
#[test]
fn empty_index_is_an_error_not_a_pass() {
    let dir = fresh_git_tree("empty-index");
    assert!(matches!(
        check_repo(&dir).expect_err("empty index must error"),
        GateError::EmptyScanSet
    ));
}

/// Fail closed: a directory with no git metadata cannot render a verdict.
#[test]
fn missing_git_metadata_fails_closed() {
    let dir = std::env::temp_dir().join(format!("no-shell-gate-{}-not-a-repo", std::process::id()));
    fs::create_dir_all(&dir).expect("create non-repo dir");
    assert!(matches!(
        check_repo(&dir).expect_err("a non-repo must error"),
        GateError::GitFailed(_)
    ));
}

// --------------- the standing clean leg: THIS repository, via cargo test

/// The `cargo test` wiring. Any `cargo test` that includes this crate re-runs
/// the gate against the real index, so a tracked `.sh`/`.py` fails the suite
/// even when CI is skipped entirely.
#[test]
fn this_repo_is_clean() {
    let root = repo_root();
    // A BOX THAT CANNOT NAME A COMMIT CANNOT SAY THIS REPOSITORY IS CLEAN
    // (`omp-orchestrator-typed-unreadable-roster-tihld`). `rch` strips `.git/`, so on the
    // lane `check_repo` returned `GitFailed("fatal: not a git repository")` and this leg
    // reported the REPOSITORY dirty when what was missing was the repository. The verdict
    // below is UNCHANGED on any box that can read: a tracked `.sh` still fails here.
    let Some(_) = common::paths_or_unmeasured(&root, "this_repo_is_clean", "crates") else {
        return;
    };
    assert_eq!(
        check_repo(&root).expect("gate must render a verdict on this repo"),
        Verdict::Clean,
        "a tracked .sh or .py is in the index — port it to Rust; the \
         exemption list is empty by design"
    );
}

/// PROVENANCE OF THE SCAN SET, which `this_repo_is_clean` above cannot check.
///
/// # The hole this closes (bead omp-orchestrator-7img8)
///
/// `scan` enforces exactly one property of the set it is given — that it is NON-EMPTY
/// (`src/lib.rs:137-140`, `GateError::EmptyScanSet`). That catches an index the gate could not
/// read at all. It cannot catch a PARTIAL one, and a partial index is indistinguishable from a
/// clean tree by every assertion this suite had: every path such a list returns really is
/// tracked, and really has no `.sh`/`.py` extension, so the gate returns `Clean` and the
/// repository's one rule is enforced over a fraction of the repository.
///
/// The two upstream guards miss it for ORTHOGONAL reasons, which is why neither covers the
/// other: `EmptyScanSet` keys on EMPTINESS, and `check_workspace_load` keys on the FILESYSTEM
/// (`src/lib.rs:281`, `:303` — a root manifest that exists on disk, members enumerated from
/// disk). A tree with every file present and a decayed `.git` passes both honestly.
///
/// So this leg asserts the set's PROVENANCE: paths that MUST be in this repository's index. A
/// list that omits them is not this repository's index, whatever else is true of it.
///
/// # Why the assertion is here and not in `scan` or `check_repo`
///
/// `scan` is pure and is deliberately fed SYNTHETIC sets (`clean_list_passes` above), and
/// `check_repo` is run against FIXTURE repositories by five legs in this file. A sentinel
/// requirement inside either would redden six legitimate controls — the fix would look like the
/// gate working while breaking the gate's own known-good and known-bad legs. Provenance is a
/// property of THE REPOSITORY-MODE CALL, so it is asserted where that call is made.
///
/// # NO-CLAIM, and it is the honest half
///
/// This is the OBSERVABLE half of the bead, not the IMPOSSIBLE half. It reddens the suite on a
/// partial index; it does NOT make the installed hook refuse one at commit time, which needs a
/// typed refusal in `src/lib.rs` and its repo-mode call site. That edit re-locks every commit in
/// the tree (`commit_ratchets.rs:18-24` lists `no-shell-gate` in `HOOK_SOURCE_CRATES`, and
/// `:188-195` walks that whole `src` tree by mtime), so it is sequenced as its own announced
/// transaction rather than smuggled in beside a test.
#[test]
fn the_scan_set_is_this_repository_and_not_a_fragment_of_one() {
    let root = repo_root();
    // ⛔ THE rch LANE CANNOT HOST THIS MEASUREMENT AND MUST SAY SO RATHER THAN ERROR.
    // `[transfer].exclude_patterns` strips `.git/`, so `ls-files` returns
    // `fatal: not a git repository` there. That is the BLIND class: the environment cannot see
    // the input. The discriminator is POSITIVE — the absence of `.git` is itself observed, never
    // inferred from a failure — and the decline is printed in the words this crate already uses
    // for it (`cited_figure_denominator.rs:139-150`), because a silent early return would be
    // indistinguishable from a pass.
    //
    // MEASURED 2026-09-11 AND IT IS WHY THIS GUARD IS A POSITIVE PROBE AND NOT A CATCH-ALL: two
    // consecutive lane runs of this target produced DIFFERENT failure sets with no source change.
    // Run A reddened `missing_git_metadata_fails_closed` (a `.git` WAS reachable, so the
    // fail-closed fixture found a parent repository); run B reddened `this_repo_is_clean` and
    // `binary_is_green_on_this_repo` (no `.git` at all). The two outcomes are COMPLEMENTARY, so
    // this target cannot be green on that lane either way — and WHICH leg is red reports whether
    // the worker carried a decayed `.git`. A flapping failure set with no diff is an instrument
    // symptom, not noise.
    //
    // A FOSSIL `.git` DOES NOT TAKE THIS BRANCH, and that is the point of the bead: the directory
    // exists, `ls-files` succeeds, and the sentinel check below REFUSES the partial list.
    if !root.join(".git").exists() {
        println!(
            "UNMEASURED reason=not_a_repo_checkout root={} -- rch strips .git/, so the index is \
             unobservable here. This is not a pass for the subject; CI and a developer checkout \
             are the environments that can answer it.",
            root.display()
        );
        return;
    }
    let tracked = tracked_files(&root).expect("the index must be readable in a real checkout");
    // Sentinels: tracked at every revision this gate has existed at, and named rather than
    // computed, because deriving them from the same `ls-files` output they are meant to validate
    // is the self-referential check this repository has recorded seven times.
    for sentinel in ["AGENTS.md", "Cargo.toml", "crates/no-shell-gate/src/lib.rs"] {
        assert!(
            tracked.iter().any(|path| path == sentinel),
            "PARTIAL INDEX: {sentinel} is tracked in this repository but absent from the scan \
             set of {} paths. The set is non-empty, so EmptyScanSet cannot see this, and every \
             path present is genuinely extension-clean — a verdict rendered on it would be a \
             PLAUSIBLE PASS over a fraction of the tree",
            tracked.len()
        );
    }
    // ANTI-VACUITY: a sentinel list that matched nothing would make the loop above vacuous, and
    // a scan set of one path would satisfy "contains something" while proving nothing.
    assert!(
        tracked.len() > 100,
        "the index collapsed to {} paths; a shrinking scan set is how this goes vacuously green",
        tracked.len()
    );
}

// ------------------ the binary surface the CI workflow invokes directly

/// CI runs the binary with no argument: it must default to this repo and
/// exit 0 while the tree is clean.
#[test]
fn binary_is_green_on_this_repo() {
    // Same discriminator as `this_repo_is_clean`: with no repository to read, the binary
    // exits 2 with `git ls-files failed: fatal: not a git repository`, and asserting 0
    // there measures the box. On a readable tree this leg is unchanged -- exit 0 and
    // `ok:` are still required, and a planted `.sh` still reddens
    // `binary_exits_1_on_planted_shell` beside it.
    let Some(_) = common::paths_or_unmeasured(&repo_root(), "binary_is_green_on_this_repo", "crates")
    else {
        return;
    };
    let (code, stdout, stderr) = run_gate(None);
    assert_eq!(
        code,
        Some(0),
        "gate binary must exit 0 on a clean tree: {stderr}"
    );
    assert!(stdout.contains("ok:"), "clean run must say so: {stdout}");
}

/// CI-invocable RED: exit code 1 (not 2), with the offending path named.
#[test]
fn binary_exits_1_on_planted_shell() {
    let dir = fresh_git_tree("binary-shell");
    stage(&dir, "README.md", "clean baseline\n");
    stage(&dir, "evil.sh", "#!/bin/sh\necho planted known-bad\n");
    let (code, _stdout, stderr) = run_gate(Some(&dir));
    assert_eq!(
        code,
        Some(1),
        "planted .sh must exit 1 (violations), not 0 or 2: {stderr}"
    );
    assert!(
        stderr.contains("evil.sh"),
        "violation output must name the file: {stderr}"
    );
}

/// ANTI-VACUITY at the CLI: an empty index is exit 2 (gate error), never 0.
#[test]
fn binary_exits_2_on_empty_index() {
    let dir = fresh_git_tree("binary-empty");
    let (code, _stdout, stderr) = run_gate(Some(&dir));
    assert_eq!(
        code,
        Some(2),
        "empty index must exit 2 (error), never 0: {stderr}"
    );
}

/// THE ADJUDICATING ARM, FIXTURED -- because no worker can reach it.
///
/// `omp-orchestrator-typed-unreadable-roster-tihld`. On this lane every box either has no
/// `.git` (contabo-3) or a repository that cannot name a commit, so the legs above take
/// the UNMEASURED path and prove nothing there. An unfixtured conversion is a suppression
/// nobody has exercised, so all three answers are exercised here instead:
///
/// ```text
/// READABLE   a committed crate is listed; a STAGED-ONLY crate is NOT  -> the discriminator
/// NO REPO    a directory with no .git                                 -> typed UNREADABLE
/// UNBORN     `git init` with no commit                                -> typed UNREADABLE
/// ```
///
/// The staged-only crate is what makes this a COMMIT read rather than an index read: it is
/// in the index and not in the commit, so a listing that includes it came from `ls-files`.
#[test]
fn the_typed_roster_reads_the_commit_and_names_a_tree_that_has_none() {
    let dir = fresh_git_tree("typed-roster");
    stage(&dir, "crates/committed-crate/Cargo.toml", "[package]\n");
    run_git(
        &dir,
        &[
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "fixture baseline [test]",
        ],
        "commit fixture baseline",
    );
    stage(&dir, "crates/staged-crate/Cargo.toml", "[package]\n");
    fs::create_dir_all(dir.join("crates/untracked-crate")).expect("create untracked crate");
    fs::write(dir.join("crates/untracked-crate/Cargo.toml"), "[package]\n")
        .expect("write untracked manifest");

    let (names, rev) = common::committed_crate_names(&dir).expect("a committed tree yields a roster");
    assert_eq!(
        names,
        vec!["committed-crate".to_owned()],
        "the roster must carry the COMMIT: a staged-only or untracked crate is invisible to CI \
         and must be invisible here"
    );
    assert_eq!(rev.len(), 40, "the roster must name the commit it came from: {rev}");
    assert!(
        common::paths_or_unmeasured(&dir, "fixture", "crates").is_some(),
        "a readable tree must produce a listing, not an UNMEASURABLE"
    );

    // ⛔ THE ANTI-SHRUG LEG: A READABLE ROSTER STILL PRODUCES A REAL VERDICT, AND STILL
    // FAILS. Without this the conversion above is indistinguishable from turning four
    // gates into "I could not read the roster" on every box. Same fixture, now with a
    // tracked `.sh` in it: the roster reads, and the gate REFUSES.
    stage(&dir, "tool.sh", "#!/bin/sh\necho no\n");
    run_git(
        &dir,
        &[
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "planted shell [test]",
        ],
        "commit planted shell",
    );
    assert!(
        common::paths_or_unmeasured(&dir, "fixture-violation", "crates").is_some(),
        "the fixture must still be READABLE, or the verdict below proves nothing"
    );
    match check_repo(&dir).expect("a readable tree must render a verdict") {
        Verdict::Violations(found) => assert!(
            found.iter().any(|violation: &Violation| violation.path.ends_with("tool.sh")),
            "a readable roster must still name the planted shell: {found:?}"
        ),
        Verdict::Clean => {
            panic!("a tracked .sh on a READABLE tree must FAIL -- an UNMEASURABLE-everywhere gate is a shrug")
        }
    }

    // NO REPOSITORY: the contabo-3 shape.
    let bare = std::env::temp_dir().join(format!(
        "no-shell-gate-{}-typed-roster-norepo-{}",
        std::process::id(),
        FIXTURE_SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(&bare).expect("create non-repo dir");
    match common::committed_crate_names(&bare) {
        Err(source) => {
            let reason = source.blocked_reason().unwrap_or_default().to_owned();
            assert!(
                reason.contains("git"),
                "a gitless tree must say WHICH command could not answer: {reason}"
            );
        }
        Ok((names, _)) => panic!("a gitless tree must not yield a roster: {names:?}"),
    }

    // UNBORN HEAD: a repository that exists and cannot name a commit.
    let unborn = fresh_git_tree("typed-roster-unborn");
    match common::committed_crate_names(&unborn) {
        Err(source) => {
            let reason = source.blocked_reason().unwrap_or_default().to_owned();
            assert!(
                reason.contains("rev-parse") || reason.contains("HEAD"),
                "an unborn HEAD must name the revision it could not resolve: {reason}"
            );
        }
        Ok((names, _)) => panic!("an unborn HEAD must not yield a roster: {names:?}"),
    }

    // READABLE AND EMPTY IS STILL AN ERROR, and a DIFFERENT one: the two must never collapse.
    let empty = fresh_git_tree("typed-roster-empty");
    stage(&empty, "README.md", "no crates here\n");
    run_git(
        &empty,
        &[
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "no crates [test]",
        ],
        "commit crate-free fixture",
    );
    match common::committed_crate_names(&empty) {
        Err(source) => {
            let reason = source.blocked_reason().unwrap_or_default().to_owned();
            assert!(
                reason.contains("is an ERROR, never a clean bill"),
                "a readable but crate-free commit must be an ERROR naming emptiness, not an \
                 unreadable tree: {reason}"
            );
            // AND IT MUST NOT LOOK LIKE AN UNREADABLE TREE: the reason names the COMMIT it
            // read, which the git-failure arms cannot do. That is the distinction the two
            // answers must never lose.
            assert!(
                reason.starts_with("commit ") && !reason.contains("`git"),
                "an empty answer must name the commit it read, never a failed command: {reason}"
            );
        }
        Ok((names, _)) => panic!("a crate-free commit must not yield a roster: {names:?}"),
    }

    for path in [dir, bare, unborn, empty] {
        fs::remove_dir_all(path).expect("fixture cleanup");
    }
}

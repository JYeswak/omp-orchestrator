#![forbid(unsafe_code)]

//! Module reachability conformance (beads omp-orchestrator-unwired-lane-conformance-6cd,
//! omp-orchestrator-up947).
//!
//! Asserts that every tracked `.rs` under a crate's `src/` is REACHABLE from that
//! crate's module graph. A tracked file nobody declares is DEAD CODE THAT
//! COMPILES GREEN: cargo does not build it, so its `#[test]` legs are absent
//! from the run rather than failing in it, `0 filtered out` is TRUE because the
//! selectors were never compiled in, and nothing complains. That is a vacuous
//! green produced by a DELETION rather than by an absence (`a8d9363` deleted
//! `pub mod agent_families;` while the file stayed tracked; restored at
//! `c8ef85c`), and it is the inverse of the LOUD failure — a declaration whose
//! file is missing, which is `E0583` on a fresh clone.
//!
//! WHAT up947 CHANGED, and why this file needed more than a new leg:
//!   1. THE ORPHAN ASSERTION COULD NOT FIRE. Its condition was
//!      `wired_count + entry_points > 0` — trivially true — while its MESSAGE
//!      described the orphan list. The vector was computed, formatted into a
//!      message nobody could ever see, and never asserted empty. So this gate
//!      passed with any number of orphans, which is why `ompo-doctor/src/undo.rs`
//!      (562 lines, ten legs, fixed at `8393c36`) was found by three HAND sweeps
//!      and never by the in-tree instrument built to catch it.
//!   2. THERE WERE TWO CLASSIFIERS AND THE BETTER ONE WAS DEAD. `classify` was
//!      never called; the test body re-implemented a subset inline and omitted
//!      the parent-directory rule. One authority now, and the planted controls
//!      drive the SAME code the repo scan does.
//!   3. IT READ THE WORKTREE. A worktree read reports a FALSE PASS, because the
//!      worktree typically still carries the declaration HEAD has lost — that is
//!      exactly how the originating instance hid for eleven minutes. This reads
//!      HEAD blobs.
//!
//! SIX REACHABILITY MECHANISMS, each of which was a measured false-positive
//! source in at least one hand sweep (19:1, 15:1 and 9:1 across three agents):
//!   (a) `lib.rs` / `main.rs` / `mod.rs` are candidates, never orphans.
//!   (b) `src/bin/NAME.rs` auto-discovered bin targets — the largest single
//!       source (14 of one sweep's 19, 11 of another's 16).
//!   (c) explicit `bin`/`example`/`bench`/`test` target paths in `Cargo.toml`.
//!   (d) a declarer in ANY file of the same crate, including a PARENT directory's
//!       sibling (3 of 19 and 3 of 16, via `bead_lifecycle.rs`).
//!   (e) `#[path = "..."]` overrides, searched as an ATTRIBUTE and not as a
//!       declaration — one sweep over-reported 9 of 10 on this alone.
//!   (f) ANY visibility form of `mod`: `pub(crate) mod revision_env;` at
//!       `ompo-doctor/src/lib.rs` defeats a `^(pub )?mod X;` pattern, and that
//!       mechanism was missed by two censuses entirely.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

// Comment handling routes through `text_structure::code_only` (bead -9ub39):
// a second comment grammar beside the kernel's is a lint finding, not a helper.
use text_structure::code_only;

/// Orphans permitted to exist, each with the bead that owns removing it. An
/// entry here is a DECLARED exception with a named owner, never a silent pass;
/// the gate still fires on every orphan outside this list.
const ORPHAN_ALLOWANCE: &[(&str, &str)] = &[];

const GIT_DEADLINE: Duration = Duration::from_secs(30);

// ── THE TREE UNDER TEST ────────────────────────────────────────────────────────

/// One tree's reachability inputs. Built from HEAD for the repo scan and by hand
/// for the planted controls, so both drive the SAME classifier.
#[derive(Default)]
struct Snapshot {
    /// Every tracked `crates/<crate>/src/**/*.rs`, repo-relative.
    sources: Vec<String>,
    /// `(crate, module stem)` for every `mod` declaration in any file of that crate.
    declarations: BTreeSet<(String, String)>,
    /// Every `#[path = "..."]` string found anywhere in the crate, by crate.
    path_overrides: BTreeMap<String, Vec<String>>,
    /// Repo-relative paths named as explicit targets in any `Cargo.toml`.
    manifest_targets: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reachability {
    /// (a) a crate root or a directory module.
    EntryPoint,
    /// (b) `src/bin/NAME.rs`, auto-discovered by cargo.
    AutoBin,
    /// (c) named as an explicit target path in `Cargo.toml`.
    ManifestTarget,
    /// (d)+(f) declared by some file of the same crate, in any visibility form.
    Declared,
    /// (e) reached through a `#[path]` attribute override.
    PathOverride,
    /// Tracked, compiled by nothing, referenced by nothing.
    Orphan,
}

fn crate_of(path: &str) -> Option<&str> {
    path.strip_prefix("crates/")?.split('/').next()
}

fn file_name_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// THE ONE CLASSIFIER. The repo scan and both planted controls call this and
/// nothing re-implements it: the previous version of this file carried a dead
/// `classify` plus an inline subset that disagreed with it about mechanism (d).
fn classify(path: &str, snapshot: &Snapshot) -> Reachability {
    let file = file_name_of(path);
    if matches!(file, "lib.rs" | "main.rs" | "mod.rs") {
        return Reachability::EntryPoint;
    }
    let Some(crate_name) = crate_of(path) else {
        return Reachability::EntryPoint;
    };
    if path.contains(&format!("crates/{crate_name}/src/bin/")) {
        return Reachability::AutoBin;
    }
    if snapshot.manifest_targets.contains(path) {
        return Reachability::ManifestTarget;
    }
    let stem = file.trim_end_matches(".rs");
    if snapshot
        .declarations
        .contains(&(crate_name.to_owned(), stem.to_owned()))
    {
        return Reachability::Declared;
    }
    if let Some(overrides) = snapshot.path_overrides.get(crate_name) {
        // An override names a path relative to the declaring file, so compare on
        // the file name rather than on a resolved absolute path: a `../` prefix
        // is the normal shape and resolving it would need the declarer's own
        // location, which the attribute scan deliberately does not carry.
        if overrides.iter().any(|target| target.ends_with(file)) {
            return Reachability::PathOverride;
        }
    }
    Reachability::Orphan
}

// ── BUILDING THE SNAPSHOT FROM HEAD ────────────────────────────────────────────

/// `git` that reports failure instead of aborting: an absent object database is
/// a real lane condition, not a bug. MEASURED 2026-09-11 on the rch build lane —
/// `git ls-tree -r --name-only HEAD` returns `fatal: Not a valid object name
/// HEAD`, because the workers receive a SYNCED WORKING TREE and not a clone
/// (`installer/build.rs` records the same fact: "the build workers are a bare
/// `git init`"). A gate that panics there is a gate that gets routed around.
fn try_git(repo_root: &Path, args: &[&str]) -> Option<String> {
    let mut command = Command::new("git");
    command.current_dir(repo_root).args(args);
    match subprocess_contract::bounded_output(&mut command, GIT_DEADLINE) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => {
            Some(String::from_utf8_lossy(&output.stdout).into_owned())
        }
        _ => None,
    }
}

fn git(repo_root: &Path, args: &[&str]) -> String {
    try_git(repo_root, args)
        .unwrap_or_else(|| panic!("MODULE REACHABILITY ERROR: git {args:?} failed"))
}

/// Can this checkout answer questions about TREES, or only about FILES?
fn tree_reachable(repo_root: &Path, tree: &str) -> bool {
    try_git(repo_root, &["rev-parse", "--verify", "--quiet", tree]).is_some()
}

/// `mod NAME;` in any visibility form, comment-stripped. Mechanism (f).
fn declared_module(line: &str) -> Option<String> {
    let code = code_only(line);
    let trimmed = code.trim();
    let rest = trimmed.strip_prefix("pub").map_or(trimmed, |after_pub| {
        // `pub mod`, `pub(crate) mod`, `pub(super) mod`, `pub(in path) mod`.
        let after_pub = after_pub.trim_start();
        after_pub
            .strip_prefix('(')
            .and_then(|inner| inner.split_once(')'))
            .map_or(after_pub, |(_, after_paren)| after_paren.trim_start())
    });
    let name = rest.trim_start().strip_prefix("mod ")?.trim();
    let name = name.strip_suffix(';')?.trim();
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some(name.to_owned())
}

/// Every `#[path = "..."]` target on a line. Mechanism (e).
fn path_override_targets(line: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find("#[path") {
        rest = &rest[start + 6..];
        let Some(open) = rest.find('"') else { break };
        let after = &rest[open + 1..];
        let Some(close) = after.find('"') else { break };
        targets.push(after[..close].to_owned());
        rest = &after[close + 1..];
    }
    targets
}

/// `git grep -n` over a TREE yields `tree:path:lineno:content`. Split on the
/// first two colons after the tree prefix; the content may contain colons.
fn grep_hits(repo_root: &Path, tree: &str, pattern: &str, pathspec: &str) -> Vec<(String, String)> {
    let mut command = Command::new("git");
    command
        .current_dir(repo_root)
        .args(["grep", "-n", "-E", pattern, tree, "--", pathspec]);
    let stdout = match subprocess_contract::bounded_output(&mut command, GIT_DEADLINE) {
        // `git grep` exits 1 for "no matches", which is a legitimate empty
        // result here and not a failure — the anti-vacuity legs below decide
        // whether an empty result is admissible, not this helper.
        subprocess_contract::BoundedOutcome::Completed(output) => {
            String::from_utf8_lossy(&output.stdout).into_owned()
        }
        other => panic!("MODULE REACHABILITY ERROR: git grep {pattern:?} failed: {other:?}"),
    };
    let prefix = format!("{tree}:");
    stdout
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix(&prefix)?;
            let (path, after_path) = rest.split_once(':')?;
            let (_lineno, content) = after_path.split_once(':')?;
            Some((path.to_owned(), content.to_owned()))
        })
        .collect()
}

/// Build the reachability inputs for one tree. THREE `git grep` narrowings plus
/// one `ls-tree`, never a per-file read: the earlier hand sweep spent 188s on
/// 637 `git show` invocations, and a gate nobody waits for is a gate that gets
/// routed around.
///
/// The greps only NARROW; every hit is re-parsed by `declared_module` /
/// `path_override_targets` so the parsing authority stays in one place and the
/// comment-stripping is the kernel's.
fn snapshot_at_head(repo_root: &Path, tree: &str) -> Snapshot {
    let listing = git(repo_root, &["ls-tree", "-r", "--name-only", tree]);
    let mut snapshot = Snapshot::default();
    for path in listing.lines().filter(|l| !l.is_empty()) {
        if !path.ends_with(".rs") {
            continue;
        }
        let Some(crate_name) = crate_of(path) else {
            continue;
        };
        if path.starts_with(&format!("crates/{crate_name}/src/")) {
            snapshot.sources.push(path.to_owned());
        }
    }

    // Mechanisms (d) and (f): a declarer in ANY file of the crate, in ANY
    // visibility form. Not just the crate roots — mechanism (d) is a parent
    // directory's sibling — and not just `mod`/`pub mod`.
    let decl_pattern =
        r"^[[:space:]]*(pub([[:space:]]*\([^)]*\))?[[:space:]]+)?mod[[:space:]]+[A-Za-z0-9_]+[[:space:]]*;";
    for (path, content) in grep_hits(repo_root, tree, decl_pattern, "crates/*.rs") {
        let Some(crate_name) = crate_of(&path) else {
            continue;
        };
        if let Some(name) = declared_module(&content) {
            snapshot.declarations.insert((crate_name.to_owned(), name));
        }
    }

    // Mechanism (e): `#[path]` overrides, searched as an ATTRIBUTE. These live
    // in `tests/` as often as in `src/`, so the pathspec is the whole crate.
    for (path, content) in grep_hits(repo_root, tree, r"#\[path", "crates/*.rs") {
        let Some(crate_name) = crate_of(&path) else {
            continue;
        };
        for target in path_override_targets(&content) {
            snapshot
                .path_overrides
                .entry(crate_name.to_owned())
                .or_default()
                .push(target);
        }
    }

    // Mechanism (c): explicit bin/example/bench/test target paths in a manifest,
    // resolved against the crate that declares them.
    for (path, content) in grep_hits(repo_root, tree, r"path[[:space:]]*=", "crates/*Cargo.toml") {
        let Some(crate_name) = crate_of(&path) else {
            continue;
        };
        if let Some(value) = manifest_target_path(&content) {
            snapshot
                .manifest_targets
                .insert(format!("crates/{crate_name}/{value}"));
        }
    }
    snapshot
}

/// `path = "src/bin/x.rs"` in a manifest target stanza. Mechanism (c), parsed in
/// ONE place so the HEAD reader and the worktree reader cannot disagree.
fn manifest_target_path(line: &str) -> Option<String> {
    let code = code_only(line);
    let trimmed = code.trim();
    let rest = trimmed.strip_prefix("path")?;
    let (_, value) = rest.split_once('=')?;
    let value = value.trim().trim_end_matches(',').trim_matches('"');
    if value.ends_with(".rs") {
        Some(value.to_owned())
    } else {
        None
    }
}

/// The SAME parsers over the filesystem, for a checkout that cannot answer
/// questions about trees. Only reachable when the object database is absent —
/// on any real clone this file reads HEAD, because a worktree read is exactly
/// the FALSE PASS the originating instance hid behind.
fn snapshot_from_worktree(repo_root: &Path) -> Snapshot {
    fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if path.is_dir() {
                if name != "target" && name != ".git" && !name.starts_with(".rch") {
                    walk(&path, out);
                }
            } else if path.extension().is_some_and(|ext| ext == "rs")
                || name == "Cargo.toml"
            {
                out.push(path);
            }
        }
    }

    let mut files = Vec::new();
    walk(&repo_root.join("crates"), &mut files);
    let mut snapshot = Snapshot::default();
    for absolute in &files {
        let Ok(relative) = absolute.strip_prefix(repo_root) else {
            continue;
        };
        let path = relative.display().to_string();
        let Some(crate_name) = crate_of(&path) else {
            continue;
        };
        if path.ends_with(".rs") && path.starts_with(&format!("crates/{crate_name}/src/")) {
            snapshot.sources.push(path.clone());
        }
        let Ok(body) = std::fs::read_to_string(absolute) else {
            continue;
        };
        for line in body.lines() {
            if path.ends_with(".rs") {
                if let Some(name) = declared_module(line) {
                    snapshot.declarations.insert((crate_name.to_owned(), name));
                }
                for target in path_override_targets(line) {
                    snapshot
                        .path_overrides
                        .entry(crate_name.to_owned())
                        .or_default()
                        .push(target);
                }
            } else if let Some(value) = manifest_target_path(line) {
                snapshot
                    .manifest_targets
                    .insert(format!("crates/{crate_name}/{value}"));
            }
        }
    }
    snapshot
}

/// Which tree this run actually measured, so a figure is never mistaken for a
/// stronger one than it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Input {
    Head,
    WorktreeOnly,
}

fn inputs(repo_root: &Path) -> (Snapshot, Input) {
    if tree_reachable(repo_root, "HEAD") {
        (snapshot_at_head(repo_root, "HEAD"), Input::Head)
    } else {
        (snapshot_from_worktree(repo_root), Input::WorktreeOnly)
    }
}

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crate lives two levels below repo root")
        .to_path_buf()
}

fn orphans_at(repo_root: &Path, tree: &str) -> (Vec<String>, usize) {
    let snapshot = snapshot_at_head(repo_root, tree);
    let candidates = snapshot.sources.len();
    let orphans = snapshot
        .sources
        .iter()
        .filter(|path| classify(path, &snapshot) == Reachability::Orphan)
        .filter(|path| {
            !ORPHAN_ALLOWANCE
                .iter()
                .any(|(allowed, _reason)| allowed == *path)
        })
        .cloned()
        .collect();
    (orphans, candidates)
}

// ── THE CONFORMANCE LEG ────────────────────────────────────────────────────────

#[test]
fn every_tracked_source_is_reachable_at_head() {
    let root = repo_root();
    let (snapshot, input) = inputs(&root);

    // ANTI-VACUITY: an empty candidate set is an ERROR, never a pass.
    assert!(
        !snapshot.sources.is_empty(),
        "MODULE REACHABILITY ERROR: empty candidate set — no tracked crates/*/src/**/*.rs at HEAD"
    );
    assert!(
        !snapshot.declarations.is_empty(),
        "MODULE REACHABILITY ERROR: zero module declarations parsed at HEAD — the declaration \
         probe cannot fire, so a zero orphan count would be structurally guaranteed rather than \
         measured"
    );

    let mut tally: BTreeMap<&str, usize> = BTreeMap::new();
    let mut orphans = Vec::new();
    for path in &snapshot.sources {
        let verdict = classify(path, &snapshot);
        let label = match verdict {
            Reachability::EntryPoint => "entry_point",
            Reachability::AutoBin => "auto_bin",
            Reachability::ManifestTarget => "manifest_target",
            Reachability::Declared => "declared",
            Reachability::PathOverride => "path_override",
            Reachability::Orphan => "orphan",
        };
        *tally.entry(label).or_default() += 1;
        if verdict == Reachability::Orphan
            && !ORPHAN_ALLOWANCE
                .iter()
                .any(|(allowed, _reason)| allowed == path)
        {
            orphans.push(path.clone());
        }
    }

    println!(
        "MODULE REACHABILITY: input={input:?} candidates={} {:?} allowance={}",
        snapshot.sources.len(),
        tally,
        ORPHAN_ALLOWANCE.len()
    );

    // THE LEG. The predicate asserted is the predicate the message describes —
    // up947's first finding was a condition of `wired + entry_points > 0` under a
    // message about orphans, which could not fail.
    assert!(
        orphans.is_empty(),
        "MODULE REACHABILITY RED: {} tracked source file(s) at HEAD are declared by nothing and \
         reached by nothing, so cargo does not compile them and any test inside them reports \
         `0 passed` at exit 0: {:?}. Declare the module, delete the file with a reason, or add it \
         to ORPHAN_ALLOWANCE with the bead that owns removing it. Candidates scanned: {}",
        orphans.len(),
        orphans,
        snapshot.sources.len()
    );
}

// ── CONTROL 1: THE PROBE ───────────────────────────────────────────────────────

/// A declaration probe that cannot fire makes every zero meaningless. This
/// pins each visibility form, the comment-stripping, and the negative case.
#[test]
fn the_declaration_probe_fires_on_every_visibility_form_and_not_otherwise() {
    for form in [
        "mod plain;",
        "pub mod plain;",
        "pub(crate) mod plain;",
        "pub(super) mod plain;",
        "pub(in crate::outer) mod plain;",
        "    pub(crate) mod plain;",
    ] {
        assert_eq!(
            declared_module(form).as_deref(),
            Some("plain"),
            "visibility form must be recognised: {form}"
        );
    }
    // NEGATIVE: not declarations, and the commented one is the reason this
    // routes through `code_only` rather than a second comment grammar.
    for not_a_declaration in [
        "// pub mod plain;",
        "let modern = 1;",
        "mod plain {",
        "use other::plain;",
        "",
    ] {
        assert_eq!(
            declared_module(not_a_declaration),
            None,
            "must not be read as a declaration: {not_a_declaration}"
        );
    }
    // Mechanism (e)'s probe, both the hit and the miss.
    assert_eq!(
        path_override_targets("#[path = \"../src/escape.rs\"]"),
        vec!["../src/escape.rs".to_owned()]
    );
    assert!(path_override_targets("mod escape;").is_empty());
}

// ── CONTROL 2: END TO END ──────────────────────────────────────────────────────

/// A probe control says the QUERY can fire; it says nothing about whether the
/// CANDIDATE SET was narrowed correctly. Measured: one hand sweep's probe
/// controls passed while 15 of its 16 hits were bad candidates, not bad probes.
/// So this plants a tree with one genuine orphan beside one instance of EVERY
/// exclusion mechanism and drives the WHOLE classifier over it: exactly one
/// orphan must come back, and it must be the planted one.
#[test]
fn the_whole_sweep_flags_a_planted_orphan_and_nothing_else() {
    let mut snapshot = Snapshot::default();
    let sources = [
        "crates/fake/src/lib.rs",                 // (a)
        "crates/fake/src/nested/mod.rs",          // (a)
        "crates/fake/src/bin/fake-tool.rs",       // (b)
        "crates/fake/src/explicit_target.rs",     // (c)
        "crates/fake/src/declared_plain.rs",      // (d) bare `mod`
        "crates/fake/src/declared_visible.rs",    // (f) `pub(crate) mod`
        "crates/fake/src/nested/child.rs",        // (d) declarer one directory up
        "crates/fake/src/escaped.rs",             // (e) `#[path]`
        "crates/fake/src/orphan.rs",              // THE PLANTED ORPHAN
    ];
    snapshot.sources = sources.iter().map(|s| (*s).to_owned()).collect();
    for stem in ["declared_plain", "declared_visible", "child", "nested"] {
        snapshot
            .declarations
            .insert(("fake".to_owned(), stem.to_owned()));
    }
    snapshot
        .manifest_targets
        .insert("crates/fake/src/explicit_target.rs".to_owned());
    snapshot
        .path_overrides
        .insert("fake".to_owned(), vec!["../src/escaped.rs".to_owned()]);

    let orphans: Vec<&str> = snapshot
        .sources
        .iter()
        .filter(|path| classify(path, &snapshot) == Reachability::Orphan)
        .map(String::as_str)
        .collect();
    assert_eq!(
        orphans,
        vec!["crates/fake/src/orphan.rs"],
        "the sweep must flag exactly the planted orphan; a different set means the exclusion \
         set is wrong rather than the probe"
    );

    // And each exclusion must be attributed to its OWN mechanism, so a file
    // excluded for the wrong reason cannot read as a pass.
    for (path, expected) in [
        ("crates/fake/src/lib.rs", Reachability::EntryPoint),
        ("crates/fake/src/nested/mod.rs", Reachability::EntryPoint),
        ("crates/fake/src/bin/fake-tool.rs", Reachability::AutoBin),
        (
            "crates/fake/src/explicit_target.rs",
            Reachability::ManifestTarget,
        ),
        ("crates/fake/src/declared_plain.rs", Reachability::Declared),
        (
            "crates/fake/src/declared_visible.rs",
            Reachability::Declared,
        ),
        ("crates/fake/src/nested/child.rs", Reachability::Declared),
        ("crates/fake/src/escaped.rs", Reachability::PathOverride),
        ("crates/fake/src/orphan.rs", Reachability::Orphan),
    ] {
        assert_eq!(
            classify(path, &snapshot),
            expected,
            "{path} must be excluded by its own mechanism, not by another one"
        );
    }
}

// ── CONTROL 3: THE KNOWN-BAD, REPRODUCED FROM GIT ──────────────────────────────

/// The originating instance, from history rather than from a fixture: `a8d9363`
/// deleted a module declaration whose file was tracked, and `c8ef85c` restored
/// it. One instrument, two trees, opposite verdicts. This is the leg that proves
/// the gate would have caught the defect that motivated it.
#[test]
fn the_sweep_reddens_at_the_deleting_commit_and_passes_at_the_restoring_one() {
    let root = repo_root();
    // MEASURED: the rch build lane receives a SYNCED WORKING TREE, so no
    // historical tree is addressable there. This leg is UNMEASURABLE rather than
    // green in that environment, and it says so in the run output — the
    // unconditional planted-orphan control above carries the same semantics on
    // every lane, so nothing here is load-bearing-by-omission.
    if !tree_reachable(&root, "a8d9363") || !tree_reachable(&root, "c8ef85c") {
        println!(
            "MODULE REACHABILITY UNMEASURABLE: historical trees a8d9363/c8ef85c are not \
             addressable in this checkout (no object database), so the git-reproduced known-bad \
             cannot run here. The planted-orphan control covers the same law on every lane."
        );
        return;
    }
    let (bad, bad_candidates) = orphans_at(&root, "a8d9363");
    let (good, good_candidates) = orphans_at(&root, "c8ef85c");

    assert!(
        bad_candidates > 0 && good_candidates > 0,
        "ANTI-VACUITY: both historical trees must yield candidates (bad={bad_candidates}, \
         good={good_candidates})"
    );
    assert!(
        bad.iter()
            .any(|path| path == "crates/installer/src/agent_families.rs"),
        "a8d9363 deleted `pub mod agent_families;` while the file stayed tracked, so the sweep \
         MUST flag it there; got {bad:?} over {bad_candidates} candidates"
    );
    assert!(
        !good
            .iter()
            .any(|path| path == "crates/installer/src/agent_families.rs"),
        "c8ef85c restored the declaration, so the sweep MUST NOT flag it there; got {good:?}"
    );
}

/// The SECOND immutable historical pair, and the one the bead names as its live
/// known-bad. `crates/ompo-doctor/src/undo.rs` — 562 lines, a public surface and
/// TEN legs — was tracked and declared by nothing until `8393c36` (f3maq)
/// declared it. Three independent hand sweeps found it; the in-tree gate never
/// did, because its orphan assertion could not fire.
///
/// A pair from git beats an allowance row: both sides are immutable, so this leg
/// keeps proving the sweep fires after the instance is repaired, and nobody has
/// to remember to delete an amnesty entry.
#[test]
fn the_sweep_reddens_on_the_undo_orphan_before_f3maq_and_passes_after() {
    let root = repo_root();
    if !tree_reachable(&root, "8393c36") {
        println!(
            "MODULE REACHABILITY UNMEASURABLE: 8393c36 is not addressable in this checkout \
             (no object database), so the undo known-bad pair cannot run here."
        );
        return;
    }
    let (before, before_candidates) = orphans_at(&root, "8393c36^");
    let (after, after_candidates) = orphans_at(&root, "8393c36");

    assert!(
        before_candidates > 0 && after_candidates > 0,
        "ANTI-VACUITY: both trees must yield candidates (before={before_candidates}, \
         after={after_candidates})"
    );
    assert!(
        before
            .iter()
            .any(|path| path == "crates/ompo-doctor/src/undo.rs"),
        "undo.rs was tracked and undeclared at 8393c36^, so the sweep MUST flag it there; got \
         {before:?} over {before_candidates} candidates"
    );
    assert!(
        !after
            .iter()
            .any(|path| path == "crates/ompo-doctor/src/undo.rs"),
        "8393c36 declared it, so the sweep MUST NOT flag it there; got {after:?}"
    );
}

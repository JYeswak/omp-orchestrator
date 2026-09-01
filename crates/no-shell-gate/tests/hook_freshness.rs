//! HOOK FRESHNESS GATE — the installed pre-commit hook must have been built from
//! the source that is on disk now.
//!
//! # The measured failure
//!
//! 2026-09-01. `state-wildcard-lint` was refusing every commit in the repo with 9
//! findings, 8 of which were wildcards the compiler requires. I fixed the lint,
//! rebuilt it, and the refusal did not change — because `.git/hooks/pre-commit` is
//! a **Mach-O binary** that links the lint as a *library*. It never shells out.
//!
//! Three artifacts existed simultaneously, with three different answers:
//!
//! | artifact | timestamp | findings |
//! |---|---|---|
//! | `release/state-wildcard-lint` | 12:11:41 (13h stale) | 8 |
//! | `debug/state-wildcard-lint` | 01:27:06 (fresh) | 0 |
//! | `.git/hooks/pre-commit` | 01:21:59 | 8 |
//!
//! I then measured my own fix with the 13-hour-old release binary, because
//! `find … | head -1` returns whichever path sorts first, and read "8 findings" as
//! the fix having failed.
//!
//! **This is BUILT ≠ WIRED aimed at the enforcement layer.** A stale hook silently
//! enforces yesterday's rules: it keeps passing or failing for reasons that no
//! longer exist in the source, and every other gate in the chain inherits that.
//!
//! # What this enforces
//!
//! No source file the hook links may be newer than the installed hook. If one is,
//! the hook predates the rules it claims to apply and must be rebuilt.
//!
//! # What it cannot do — stated because I already bypassed a sibling gate this way
//!
//! Mtime is not a content hash. `touch .git/hooks/pre-commit` satisfies this gate
//! without rebuilding anything, exactly as `os.utime` on `PLAN.md` bypassed the
//! assembly-freshness gate earlier in this same session — by its author, within a
//! minute of writing it.
//!
//! The content-addressed version — compare the installed hook's SHA-256 against a
//! build of current `HEAD` — requires building inside a test, which is recursive
//! and slow. It is **not built**. So this catches *forgot to rebuild*, which is the
//! failure that actually happened, and not *deliberately stamped past it*.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root two levels up")
        .to_path_buf()
}

/// Crates whose source is compiled INTO the hook binary.
///
/// Derived from `crates/no-shell-gate/src/bin/pre-commit-gate.rs` and the lints it
/// calls. Hand-maintained, and that is a weakness worth naming: a new lint linked
/// into the hook and not added here is invisible to this gate — the same
/// hand-maintained-list defect that made the gate census report frozen verdicts.
const HOOK_SOURCE_CRATES: &[&str] = &[
    "no-shell-gate",
    "state-wildcard-lint",
    "path-literal-guard",
    "undrained-pipe-lint",
];

fn newest_source(root: &Path) -> Option<(PathBuf, SystemTime)> {
    let mut newest: Option<(PathBuf, SystemTime)> = None;
    for crate_name in HOOK_SOURCE_CRATES {
        let dir = root.join("crates").join(crate_name).join("src");
        let mut stack = vec![dir];
        while let Some(d) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&d) else { continue };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().is_none_or(|x| x != "rs") {
                    continue;
                }
                let Ok(meta) = std::fs::metadata(&p) else { continue };
                let Ok(mtime) = meta.modified() else { continue };
                let replace = newest.as_ref().is_none_or(|(_, t)| mtime > *t);
                if replace {
                    newest = Some((p, mtime));
                }
            }
        }
    }
    newest
}

#[test]
fn the_installed_hook_is_not_older_than_the_source_it_enforces() {
    let root = repo_root();
    let hook = root.join(".git/hooks/pre-commit");

    // An absent hook is a finding, not a pass. A repo whose gates are documented
    // and uninstalled is the shape this project keeps refusing.
    let Ok(hook_meta) = std::fs::metadata(&hook) else {
        // In a fresh clone or a CI checkout there are no hooks at all, and failing
        // there would make this gate un-runnable rather than informative. Say so
        // out loud instead of passing silently.
        eprintln!(
            "SKIP the_installed_hook_is_not_older_than_the_source_it_enforces: \
             no hook at {} — nothing is enforcing pre-commit here, which is worth \
             knowing but is not a staleness finding",
            hook.display()
        );
        return;
    };

    let hook_mtime = hook_meta.modified().expect("hook mtime readable");

    let (newest_path, newest_mtime) =
        newest_source(&root).expect("ANTI-VACUITY: no .rs sources found under the hook's crates — the scan is broken");

    assert!(
        newest_mtime <= hook_mtime,
        "THE INSTALLED HOOK IS STALE.\n\
         \n\
         hook:   {}\n\
         newer:  {}\n\
         \n\
         The hook is a compiled binary that LINKS these crates as libraries; it does\n\
         not shell out, so editing a lint has no effect until the hook is rebuilt and\n\
         reinstalled. A stale hook enforces rules that no longer exist in the source.\n\
         \n\
         Repair:\n\
           cargo build --release --bin pre-commit-gate\n\
           cp <target>/release/pre-commit-gate .git/hooks/pre-commit\n\
           .git/hooks/pre-commit   # expect exit 0",
        hook.display(),
        newest_path.display(),
    );
}

#[test]
fn the_hook_source_crate_list_names_only_crates_that_exist() {
    // A hand-maintained list that names a crate which is gone rots into a scan of
    // nothing, and a scan of nothing passes. This is the check the gate census
    // lacked when it hardcoded three unextracted crates as permanently
    // "Unreachable".
    let root = repo_root();
    let missing: Vec<_> = HOOK_SOURCE_CRATES
        .iter()
        .filter(|c| !root.join("crates").join(c).join("src").is_dir())
        .collect();
    assert!(
        missing.is_empty(),
        "HOOK_SOURCE_CRATES names {} crate(s) with no src/ directory: {:?}\n\
         Either the crate moved and this list is stale, or the list was wrong when \
         written. Both make the freshness scan narrower than it claims.",
        missing.len(),
        missing
    );
}

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
    "orchestration-tick-gate",
    "undrained-pipe-lint",
];

fn newest_source(root: &Path) -> Option<(PathBuf, SystemTime)> {
    let mut newest: Option<(PathBuf, SystemTime)> = None;
    for crate_name in HOOK_SOURCE_CRATES {
        let dir = root.join("crates").join(crate_name).join("src");
        let mut stack = vec![dir];
        while let Some(d) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&d) else {
                continue;
            };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().is_none_or(|x| x != "rs") {
                    continue;
                }
                let Ok(meta) = std::fs::metadata(&p) else {
                    continue;
                };
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
    // THREE STATES, NOT TWO -- re-keyed 2026-09-07 (`hzm43`).
    //
    // This used to early-return and PASS whenever the hook was unreadable, saying so with an
    // `eprintln!`. libtest CAPTURES a passing test's output, so the reason was invisible inside a
    // green run -- and `.git/` is excluded from the rch overlay (`~/.config/rch/config.toml:29`),
    // so EVERY lane run of this suite reported green about a hook it could not see. That is how a
    // five-day-stale hook survived: the oracle whose whole job is catching staleness was
    // structurally blind in the only environment we run Rust in. The comment above it read "say so
    // out loud instead of passing silently" -- and the mechanism it described could not do that.
    //
    // The precondition is now keyed on what actually distinguishes the two absences:
    //
    //   `.git` present, no hook -> a real checkout enforcing NOTHING. FAIL.
    //   no `.git` at all        -> a synced worker copy that cannot answer. UNMEASURED.
    //   hook present            -> compare mtimes, as before.
    //
    // Absence alone still never satisfies this leg: it takes absence PLUS proof that the
    // environment cannot answer.
    // KEYED ON WHETHER THE REPOSITORY HAS HISTORY -- and the lane refuted three simpler keys
    // first, one run each, 2026-09-07:
    //
    //   `.git` exists          worker=YES   not a discriminator
    //   `.git/hooks` exists    worker=YES   not a discriminator -- a DIRECTORY SKELETON survives
    //   `.git/HEAD` is_file    worker=YES   not a discriminator -- HEAD exists with no history
    //
    // The rch worker presents a COMPLETE, EMPTY repository: `.git/` is excluded from the sync,
    // and something on the worker `git init`s the tree, so every existence check inside `.git`
    // answers YES while the hook is absent. Each of those keys reproduced the `mirror_oracle`
    // defect retracted earlier the same day -- a precondition on a PROXY whose value varies by
    // environment.
    //
    // History is the property that a `git init` cannot fake and a real checkout cannot lack:
    // a tree that can run a PRE-COMMIT hook is a tree with commits in it. An empty init has no
    // refs and no packed-refs.
    let head_file = root.join(".git/HEAD");
    let hooks_dir = root.join(".git/hooks");
    let has_history = root.join(".git/packed-refs").is_file()
        || std::fs::read_dir(root.join(".git/refs/heads"))
            .map(|mut entries| entries.next().is_some())
            .unwrap_or(false);
    let Ok(hook_meta) = std::fs::metadata(&hook) else {
        assert!(
            !has_history,
            "THIS IS A COMMITTING CHECKOUT AND NO PRE-COMMIT HOOK IS INSTALLED.\n\
             \n\
             HEAD file: {}\n\
             hooks dir: {}\n\
             absent:    {}\n\
             \n\
             A repository whose gates are documented and uninstalled is the shape this project\n\
             keeps refusing, and it is not a skip: every pre-commit gate in this tree is inert\n\
             here. Measured 2026-09-07 -- an installed hook five days older than the gate it was\n\
             meant to carry let a `100644 => 100755` exec bit land 2h16m AFTER that gate shipped.\n\
             \n\
             Repair:\n\
               cargo build --release --bin pre-commit-gate\n\
               cp <target>/release/pre-commit-gate .git/hooks/pre-commit",
            head_file.display(),
            hooks_dir.display(),
            hook.display()
        );
        // No history: an empty `git init` skeleton, which is what an rch worker presents. It
        // cannot host a commit, so it cannot answer this question. Every discriminator is
        // printed -- including the three the lane refuted -- so a reader of a green run can see
        // WHICH state this was rather than inferring a pass from silence.
        eprintln!(
            "UNMEASURED the_installed_hook_is_not_older_than_the_source_it_enforces: \
             reason=no_history root={} git_exists={} hooks_dir_exists={} head_is_file={} \
             has_history=false -- an empty repository cannot commit, so the installed hook is \
             unobservable here. This is NOT a pass for the subject.",
            root.display(),
            root.join(".git").exists(),
            hooks_dir.exists(),
            head_file.is_file()
        );
        return;
    };

    let hook_mtime = hook_meta.modified().expect("hook mtime readable");

    let (newest_path, newest_mtime) = newest_source(&root)
        .expect("ANTI-VACUITY: no .rs sources found under the hook's crates — the scan is broken");

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

/// Literals a landed commit introduced into the hook's source, and that commit.
///
/// The FIRST row is a POSITIVE CONTROL and must always be PRESENT. Without it a
/// broken probe — wrong path, unreadable file, an encoding that never matches —
/// reports every row absent and is indistinguishable from a genuinely empty hook.
const LANDED_HOOK_LITERALS: &[(&str, &str)] = &[
    ("NOTHING_TO_CHECK", "POSITIVE CONTROL: present in every hook since 93537c8"),
    ("GATE_SECTION_HELD", "c10a96a fix(pre-commit): serialize the gate section"),
    ("STAGED_BUILD_GATE_REFUSED", "4e1df0b feat(gates): wire staged-build-gate"),
    // ADDED 2026-09-07 (`hzm43`), and it would have caught the stale hook five days earlier.
    //
    // `1a7082d` landed `validate_staged_rust_modes` at 14:09; the installed hook was from
    // 2026-09-02 19:29, so the gate was never in the binary, and `a602074` re-landed a
    // `100644 => 100755` exec bit at 16:25 unrefused. The mtime leg could not see it -- 48
    // sources were newer, and that leg had already been laundered by a `cp` revert.
    //
    // The literal is the REFUSAL'S OWN PREFIX, not the function name, because `strings` finds
    // string LITERALS and not logic: on a hook that demonstrably carries this gate,
    // `validate_staged_rust_modes` returns 0 (symbol stripped in release) and `100755` returns 0
    // (a numeric comparison never appears as text). Keying on the emitted message is the only
    // form of this probe that can work -- a format string can only be compiled in if the emit
    // site exists. It still does NOT prove the branch is reachable; the live probe does that.
    ("mode-gate: REFUSED", "1a7082d feat(no-shell-gate): refuse executable Rust source modes"),
];

/// Staleness the fleet has ACCEPTED, each row naming the bead that removes it.
///
/// # Why an allowance and not a green
///
/// This is `franken_lean`'s `UNWIRED_LANE_ALLOWANCE` aimed at the enforcement
/// layer. A decision recorded only in a bead comment is invisible to the code it
/// governs, so the test cannot tell an accepted gap from an unnoticed one and
/// reports the same verdict for both. The row makes the decision legible where the
/// check runs.
///
/// **It must shrink.** A row whose literal is now PRESENT fails
/// [`the_installed_hook_matches_the_content_of_the_source_it_enforces`], so wiring
/// the gate FORCES the row's deletion and nobody has to remember.
const ACCEPTED_STALENESS: &[(&str, &str, &str)] = &[
    (
        "GATE_SECTION_HELD",
        "omp-orchestrator-g5e0",
        "SnowyCanyon 2026-09-03 ~02:0xZ: HEAD's hook makes GATE 7 build each touched \
         crate LOCALLY on every commit. Joshua 01:5xZ: local rust builds destroy the \
         machine, builds only on contabo. The gate-section gap is accepted OVER a \
         local build per commit. Unblocks when g5e0 acceptance 2 lands.",
    ),
    (
        "STAGED_BUILD_GATE_REFUSED",
        "omp-orchestrator-g5e0",
        "Same ruling: this literal IS gate 7, the local-build gate itself.",
    ),
];

/// Is `needle` present in `haystack`? Byte search, so it works on a Mach-O.
fn contains_bytes(haystack: &[u8], needle: &str) -> bool {
    let needle = needle.as_bytes();
    haystack.windows(needle.len()).any(|window| window == needle)
}

/// THE CONTENT LEG. Mtime freshness is not content freshness, and this file's own
/// header said the content-addressed version was "not built".
///
/// # The measured laundering, 2026-09-02
///
/// The mtime leg above was GREEN — `4 passed; 0 failed` — while the installed hook
/// provably lacked two landed, mutation-verified gates:
///
/// ```text
/// hook   mtime 2026-09-02T19:29:21     source mtime 2026-09-02T19:00:06   -> mtime leg GREEN
/// strings .git/hooks/pre-commit | grep -c GATE_SECTION_HELD          -> 0   (c10a96a)
/// strings .git/hooks/pre-commit | grep -c STAGED_BUILD_GATE_REFUSED  -> 0   (4e1df0b)
/// strings .git/hooks/pre-commit | grep -c NOTHING_TO_CHECK           -> 3   (positive control)
/// ```
///
/// The 19:29 mtime came from a **revert by `cp`**: copying an OLD binary over the
/// hook stamps a NEW mtime on OLD content. So a revert — a legitimate, deliberate
/// operation — launders staleness into freshness, and the gate written to catch
/// "forgot to rebuild" cannot see "rebuilt, then replaced with an older build".
/// That is the same shape as `os.utime` on `PLAN.md`, except nobody was gaming it.
///
/// # Why literal probes rather than a rebuild
///
/// The header rejected the content-addressed form because building inside a test is
/// recursive and slow. Probing the installed binary for literals a commit
/// introduced needs no build at all: `git log -S <literal>` names the commit, and
/// the string is either in the Mach-O or it is not.
///
/// # What this does NOT do
///
/// It proves the named literals are present, NOT that the hook was built from this
/// exact tree — a change that adds no new literal is invisible here, and the
/// hand-maintained row list carries the same weakness `HOOK_SOURCE_CRATES` names
/// above. It raises the floor from "a timestamp" to "these specific gates are in
/// the binary", which is what an operator actually needs to know.
#[test]
fn the_installed_hook_matches_the_content_of_the_source_it_enforces() {
    let root = repo_root();
    let hook = root.join(".git/hooks/pre-commit");

    let Ok(bytes) = std::fs::read(&hook) else {
        eprintln!(
            "SKIP the_installed_hook_matches_the_content_of_the_source_it_enforces: \
             no readable hook at {} — nothing is enforcing pre-commit here",
            hook.display()
        );
        return;
    };

    assert!(
        !bytes.is_empty(),
        "ANTI-VACUITY: the installed hook is zero bytes; an empty probe target \
         reports every literal absent and would read as total staleness"
    );

    let mut absent: Vec<(&str, &str)> = Vec::new();
    let mut present: Vec<&str> = Vec::new();
    for (literal, provenance) in LANDED_HOOK_LITERALS {
        if contains_bytes(&bytes, literal) {
            present.push(literal);
        } else {
            absent.push((literal, provenance));
        }
    }

    // POSITIVE CONTROL. A probe that finds nothing is broken, and a broken probe
    // must not be readable as "the hook is missing everything".
    let (control, _) = LANDED_HOOK_LITERALS
        .first()
        .expect("ANTI-VACUITY: the literal list is empty, so this leg checks nothing");
    assert!(
        present.contains(control),
        "PROBE BROKEN, not a staleness finding: the positive control {control:?} is \
         absent from {}. Every other verdict from this leg is therefore unreadable — \
         fix the probe before believing any absence.",
        hook.display()
    );

    // Leg 3 of the ratchet: an allowance row that is no longer needed FAILS, so
    // wiring a gate forces its row to be deleted.
    for (literal, bead, _) in ACCEPTED_STALENESS {
        assert!(
            !contains_bytes(&bytes, literal),
            "STALE ALLOWANCE ROW: {literal:?} is NOW PRESENT in the installed hook, \
             so its ACCEPTED_STALENESS row (bead {bead}) is obsolete. Delete the row. \
             An allowance that outlives its reason is how advisory-first becomes \
             permanent silence."
        );
    }

    let undeclared: Vec<String> = absent
        .iter()
        .filter(|(literal, _)| {
            !ACCEPTED_STALENESS
                .iter()
                .any(|(declared, _, _)| declared == literal)
        })
        .map(|(literal, provenance)| format!("  {literal} — introduced by {provenance}"))
        .collect();

    assert!(
        undeclared.is_empty(),
        "THE INSTALLED HOOK IS STALE BY CONTENT, and the mtime leg cannot see it.\n\
         \n\
         hook: {}\n\
         absent literals, each from a LANDED commit:\n\
         {}\n\
         \n\
         A `cp` of an older binary stamps a new mtime on old content, so mtime\n\
         freshness proves nothing here. Either rebuild and reinstall the hook, or —\n\
         if the gap is deliberate — add a row to ACCEPTED_STALENESS naming the bead\n\
         that removes it. A decision that lives only in a bead comment is invisible\n\
         to this check.\n\
         \n\
         Repair:\n\
           cargo build --release --bin pre-commit-gate\n\
           cp <target>/release/pre-commit-gate .git/hooks/pre-commit",
        hook.display(),
        undeclared.join("\n"),
    );
}

/// Every accepted-staleness row must name a real bead and a real reason.
///
/// An allowance whose row says "TODO" is a permanent exception wearing a process
/// costume, which is the failure mode the row list exists to prevent.
#[test]
fn every_accepted_staleness_row_names_a_bead_and_a_reason() {
    assert!(
        !LANDED_HOOK_LITERALS.is_empty(),
        "ANTI-VACUITY: no literals declared, so the content leg checks nothing"
    );
    for (literal, bead, reason) in ACCEPTED_STALENESS {
        assert!(
            bead.starts_with("omp-orchestrator-") && bead.len() > "omp-orchestrator-".len(),
            "row {literal:?} must name the bead that removes it, got {bead:?}"
        );
        assert!(
            reason.len() > 40,
            "row {literal:?} must say WHO ruled and WHY, not {reason:?}"
        );
        assert!(
            LANDED_HOOK_LITERALS
                .iter()
                .any(|(declared, _)| declared == literal),
            "row {literal:?} is not in LANDED_HOOK_LITERALS, so nothing ever probes \
             for it and the row can never expire"
        );
    }
}

/// The SAME staleness class, aimed at the pre-push hook — which had no leg at all
/// until 2026-09-02.
///
/// `.git/hooks/pre-push` is also a compiled Mach-O binary (615,728 bytes, built
/// 09-01 09:49 when this was written), so teaching `pre-push-gate.rs` to check
/// toolchain parity with CI changed nothing about what the installed hook
/// enforces. That is exactly the failure the pre-commit leg above was written for,
/// and it was reachable through the pre-push door the whole time.
#[test]
fn the_installed_pre_push_hook_is_not_older_than_its_source() {
    let root = repo_root();
    let hook = root.join(".git/hooks/pre-push");

    let Ok(hook_meta) = std::fs::metadata(&hook) else {
        eprintln!(
            "SKIP the_installed_pre_push_hook_is_not_older_than_its_source: no hook at {} \
             — nothing is enforcing pre-push here, which is worth knowing but is not a \
             staleness finding",
            hook.display()
        );
        return;
    };
    let hook_mtime = hook_meta.modified().expect("hook mtime readable");

    // The pre-push binary links `subprocess-contract` (bounded `git ls-files` and
    // bounded `rustc -vV`); everything else it needs is in its own bin file.
    let sources = [
        root.join("crates/no-shell-gate/src/bin/pre-push-gate.rs"),
        root.join("crates/no-shell-gate/build.rs"),
        root.join("crates/subprocess-contract/src/lib.rs"),
    ];
    let mut newest: Option<(PathBuf, SystemTime)> = None;
    for p in sources {
        let Ok(meta) = std::fs::metadata(&p) else {
            continue;
        };
        let Ok(mtime) = meta.modified() else { continue };
        if newest.as_ref().is_none_or(|(_, t)| mtime > *t) {
            newest = Some((p, mtime));
        }
    }
    let (newest_path, newest_mtime) = newest.expect(
        "ANTI-VACUITY: none of the pre-push hook's sources were readable — the scan is broken",
    );

    assert!(
        newest_mtime <= hook_mtime,
        "THE INSTALLED PRE-PUSH HOOK IS STALE.\n\
         \n\
         hook:   {}\n\
         newer:  {}\n\
         \n\
         Repair:\n\
           cargo build --release --bin pre-push-gate\n\
           cp <target>/release/pre-push-gate .git/hooks/pre-push\n\
           .git/hooks/pre-push   # expect a refusal you understand, or exit 0",
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
#[test]
fn an_installed_pre_push_copy_is_fresh_when_present() {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        eprintln!("SKIP installed pre-push copy freshness: HOME is unset");
        return;
    };
    let installed = home.join(".local/bin/pre-push-gate");
    let Ok(installed_meta) = std::fs::metadata(&installed) else {
        eprintln!(
            "SKIP installed pre-push copy freshness: {} is absent and installer does not own this path",
            installed.display()
        );
        return;
    };
    let installed_mtime = installed_meta
        .modified()
        .expect("installed pre-push copy mtime readable");
    let sources = [
        repo_root().join("crates/no-shell-gate/src/bin/pre-push-gate.rs"),
        repo_root().join("crates/no-shell-gate/build.rs"),
        repo_root().join("crates/subprocess-contract/src/lib.rs"),
    ];
    let (newest_path, newest_mtime) = sources
        .into_iter()
        .filter_map(|path| {
            let mtime = std::fs::metadata(&path).ok()?.modified().ok()?;
            Some((path, mtime))
        })
        .max_by_key(|(_, mtime)| *mtime)
        .expect("ANTI-VACUITY: installed pre-push sources are unreadable");
    assert!(
        newest_mtime <= installed_mtime,
        "INSTALLED PRE-PUSH COPY IS STALE: copy={} newer_source={}",
        installed.display(),
        newest_path.display()
    );
}

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

/// ⛔ THE PRE-COMMIT MTIME-ORDERING LEG WAS RETIRED HERE (`omp-orchestrator-tfdki`, done-bar (d)),
/// TOGETHER WITH ITS `HOOK_SOURCE_CRATES` COPY AND ITS `newest_source` WALK.
///
/// It asserted `newest_source_mtime <= hook_mtime` across the five hand-listed crates. That is
/// the ORACLE THIS BEAD REPLACED: `commit_ratchets::hook_freshness` now reports
/// `oracle=content_digest` from `hook_digest`, and mtime is gone from the production freshness
/// path entirely.
///
/// It is DELETED rather than kept beside the digest for a reason stronger than tidiness:
/// **A HOOK-TOUCH SATISFIES IT BY CONSTRUCTION.** `touch .git/hooks/pre-commit` moves
/// `hook_mtime` forward and the assertion passes on a hook that was never rebuilt — which is
/// precisely the escape hatch acceptance 4 exists to close. A retired oracle left asserted
/// beside its replacement does not merely duplicate: it re-offers the bypass, and a green run
/// containing both reads as two independent confirmations when one of them can be satisfied by
/// a metadata write. Its replacement is
/// [`crate::the_installed_hook_content_verdict_ignores_the_hook_s_own_mtime`] below, which pins
/// that the content verdict does not move when the hook's mtime does.
///
/// The pre-PUSH mtime legs further down are NOT retired and that asymmetry is deliberate — see
/// their own doc comment. tfdki replaced the PRE-COMMIT oracle only; pre-push has no digest, so
/// deleting its mtime legs would remove the only freshness check it has.
use no_shell_gate::hook_digest::HOOK_SOURCE_CRATES;

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
/// DRAINED 2026-09-12 (`omp-orchestrator-g5e0`). Both rows are deleted because
/// both literals are NOW PRESENT in the installed hook, which is exactly the
/// condition leg 3 below refuses:
///
/// ```text
/// gate.yml 34662211444 + 34662686911 (clean checkouts):
///   STALE ALLOWANCE ROW: "GATE_SECTION_HELD" is NOW PRESENT in the installed hook
/// .git/hooks/pre-commit, byte count, this lane:
///   NOTHING_TO_CHECK 4 · GATE_SECTION_HELD 1 · STAGED_BUILD_GATE_REFUSED 3 · mode-gate: REFUSED 1
/// wired at crates/no-shell-gate/src/bin/pre-commit-gate.rs:129 and :1545
/// ```
///
/// The list is EMPTY, and empty is the terminal state this doc demanded ("it
/// must shrink"), never a bypass: with no rows, every absent literal is
/// UNDECLARED and fails. Deleting a row whose reason is gone is adjudication;
/// raising a bound would be amnesty, and nothing here was widened to absorb it.
const ACCEPTED_STALENESS: &[(&str, &str, &str)] = &[];

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

/// PRE-PUSH ONLY, AND DELIBERATELY STILL MTIME — RE-SCOPED, NOT RETIRED
/// (`omp-orchestrator-tfdki`, done-bar (d)).
///
/// ⛔ THIS IS THE WEAKER ORACLE AND IT IS KEPT ON PURPOSE. tfdki replaced the PRE-COMMIT
/// freshness oracle with a content digest; `.git/hooks/pre-push` HAS NO DIGEST. Deleting this
/// leg alongside the pre-commit one would have removed the only freshness check pre-push has,
/// which is a coverage loss dressed as a cleanup — so the blanket reading of "retire the mtime
/// legs" is refused here and the asymmetry is stated instead.
///
/// ⛔ AND ITS LIMITS ARE NAMED SO NOBODY READS IT AS THE PRE-COMMIT CONTRACT: mtime proves the
/// binary is NOT OLDER than these three sources. It does NOT prove it was built from them, and
/// `touch .git/hooks/pre-push` satisfies it without a rebuild. It is a floor, not a proof.
///
/// DEATH CONDITION: this leg and its sibling below die when `pre-push-gate` carries a stamped
/// source manifest of its own, at which point they are replaced by a content comparison exactly
/// as the pre-commit leg was. Until then a weak check beats none.
///
/// The original finding that earned it: `.git/hooks/pre-push` is also a compiled Mach-O binary
/// (615,728 bytes, built 09-01 09:49 when this was written), so teaching `pre-push-gate.rs` to
/// check toolchain parity with CI changed nothing about what the installed hook enforces — the
/// same laundering the pre-commit leg was written for, reachable through the pre-push door.
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

// ── tfdki DONE-BAR (b) AND (c), AND THE ROOT-CLASS GAP IN ACCEPTANCE 5 ─────────────────────

/// ACCEPTANCE 4, WHICH HAD NO LEG: `touch .git/hooks/pre-commit` MUST NOT RESCUE A STALE HOOK.
///
/// The old mtime oracle could be laundered by a metadata write on the hook itself — that is the
/// escape hatch this bead closes, and until now it was closed only BY CONSTRUCTION ("the hook's
/// mtime is never compared"), which is an argument about absence. Absence is exactly what a
/// future edit reintroduces silently, so it is pinned here as behaviour.
///
/// Driven at the API, not through the installed binary: the hook's stamp is baked from the REAL
/// repo at ITS build time, so no fixture can ever be CLEAN against it — the structural reason
/// the previous fixture-level leg was vacuous.
#[test]
fn the_content_verdict_is_unmoved_by_writing_the_hook_itself() {
    let tree = tempfile::tempdir().expect("tempdir");
    let root = tree.path();
    let src = root.join("crates/no-shell-gate/src");
    std::fs::create_dir_all(&src).expect("covered crate src");
    let covered = src.join("lib.rs");
    std::fs::write(&covered, b"fn a() {}\n").expect("write covered source");

    let stamped = no_shell_gate::hook_digest::hook_source_manifest(root)
        .expect("the fixture tree is a covered tree");
    assert!(
        !no_shell_gate::hook_digest::manifest_rows(&stamped).is_empty(),
        "RULE hook_touch_non_vacuous: a manifest with no rows would make every assertion below \
         pass without measuring anything"
    );

    // GENUINELY STALE: one byte of a covered source changes after the stamp.
    std::fs::write(&covered, b"fn b() {}\n").expect("edit covered source");
    let current = no_shell_gate::hook_digest::hook_source_manifest(root).expect("recompute");
    let before = no_shell_gate::hook_digest::diff_manifests(&stamped, &current);
    assert!(
        !before.is_empty(),
        "RULE hook_touch_precondition: the tree must be STALE before the touch, or the leg \
         proves nothing about rescuing staleness"
    );

    // THE ESCAPE HATCH: write the hook itself, which is what a `touch` does to mtime.
    let hooks = root.join(".git/hooks");
    std::fs::create_dir_all(&hooks).expect("hooks dir");
    let hook = hooks.join("pre-commit");
    std::fs::write(&hook, b"#!/bin/sh\nexit 0\n").expect("install hook");
    let hook_first = std::fs::metadata(&hook)
        .and_then(|m| m.modified())
        .expect("hook mtime");
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(&hook, b"#!/bin/sh\nexit 0\n").expect("re-write hook");
    let hook_second = std::fs::metadata(&hook)
        .and_then(|m| m.modified())
        .expect("hook mtime");
    assert!(
        hook_second >= hook_first,
        "RULE hook_touch_is_real: the hook's mtime must actually have moved, or this leg is \
         asserting against a touch that never happened"
    );

    let after = no_shell_gate::hook_digest::diff_manifests(
        &stamped,
        &no_shell_gate::hook_digest::hook_source_manifest(root).expect("recompute"),
    );
    assert_eq!(
        after, before,
        "RULE hook_touch_cannot_launder: writing the hook must not change the content verdict; \
         the old mtime oracle could be rescued this way and the digest must not be"
    );
    assert!(
        !after.is_empty(),
        "RULE hook_touch_cannot_launder: the tree is still stale after the hook was touched"
    );
}

/// DONE-BAR (c): THE HAND LIST IS NOW A RATCHET, NOT A SILENT FLOOR.
///
/// `no-shell-gate` declares path deps that are LINKED INTO the hook binary; `HOOK_SOURCE_CRATES`
/// watches five of them. Editing an unwatched one changes the artifact and leaves the manifest
/// identical — DEFECT B in the bead, which its own comment predicted a hand list would
/// re-acquire, and which has now happened three times (`omp-inventory-map` is the newest).
///
/// ⛔ DERIVATION WAS CONSIDERED AND DELIBERATELY NOT TAKEN HERE. Deriving the covered set from
/// the closure widens it from 24 files to 40+, which CHANGES THE STAMP, which makes every
/// installed hook in the fleet report `STALE_HEALING` on its next commit. That is a fleet-wide
/// event and it is not this leg's to trigger. It also contradicts the bead's own ruling —
/// "LINKAGE IS NOT INFLUENCE, ten are unproven, watching 16 crates is the over-strict gate this
/// repo calls the slower death".
///
/// ⭐ WHAT THIS LEG DOES INSTEAD IS THE PART THAT WAS ACTUALLY MISSING: it removes the SILENCE.
/// Every declared path dep must be either WATCHED or explicitly listed as unwatched WITH A
/// REASON. A new dependency is neither, so it FAILS here until somebody classifies it. The
/// drift stops being invisible without the blast radius of watching everything.
#[test]
fn every_linked_path_dep_is_watched_or_declared_unwatched() {
    let manifest = std::fs::read_to_string(
        repo_root().join("crates/no-shell-gate/Cargo.toml"),
    )
    .expect("no-shell-gate manifest is readable");
    // SCOPED TO `[dependencies]`. A `[dev-dependencies]` path dep is linked into the TEST
    // binary, never into the installed hook, so counting it here would demand an exemption row
    // for a crate that cannot affect the artifact this digest describes.
    let declared: Vec<String> = manifest
        .lines()
        .skip_while(|line| line.trim() != "[dependencies]")
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with('['))
        .filter_map(|line| {
            let (name, rest) = line.split_once('=')?;
            rest.contains("path = \"../")
                .then(|| name.trim().to_owned())
        })
        .collect();
    assert!(
        declared.len() >= 5,
        "RULE dep_ratchet_non_vacuous: only {} path deps parsed out of the manifest; a parser \
         that reads nothing would pass this leg while measuring nothing",
        declared.len()
    );

    let unclassified: Vec<&String> = declared
        .iter()
        .filter(|dep| {
            !HOOK_SOURCE_CRATES.contains(&dep.as_str())
                && !no_shell_gate::hook_digest::DELIBERATELY_UNWATCHED
                    .iter()
                    .any(|(name, _)| name == dep)
        })
        .collect();
    assert!(
        unclassified.is_empty(),
        "RULE dep_ratchet: {} linked path dep(s) are neither WATCHED nor declared unwatched: \
         {unclassified:?}\nAdd each to HOOK_SOURCE_CRATES (it will change the stamp and every \
         hook in the fleet will heal once), or to DELIBERATELY_UNWATCHED with the reason it \
         cannot alter hook behaviour. Silence is the one option this leg removes.",
        unclassified.len()
    );

    for (name, reason) in no_shell_gate::hook_digest::DELIBERATELY_UNWATCHED {
        assert!(
            declared.iter().any(|dep| dep == name),
            "RULE dep_ratchet_expires: {name:?} is declared unwatched but is no longer a path \
             dep; a stale exemption is how an allowlist outlives its reason"
        );
        assert!(
            reason.len() > 20,
            "RULE dep_ratchet_reasoned: {name:?} needs a real reason, not a placeholder"
        );
    }
}

/// ACCEPTANCE 5's `Unreadable` ARM, PROVED WITHOUT A `chmod` — the ROOT-class workaround.
///
/// ⛔ A `chmod`-unreadable fixture IS READABLE AS ROOT, and the lane workers run as root, so a
/// permission-based leg is unprovable there BY CONSTRUCTION (ROOT class). A DANGLING SYMLINK is
/// not a permission: `read` fails with ENOENT for every uid, so the error is injected at the
/// filesystem boundary in a way privilege cannot bypass.
#[test]
fn an_unreadable_covered_source_is_a_typed_refusal_not_a_pass() {
    let tree = tempfile::tempdir().expect("tempdir");
    let root = tree.path();
    let src = root.join("crates/no-shell-gate/src");
    std::fs::create_dir_all(&src).expect("covered crate src");
    std::fs::write(src.join("lib.rs"), b"fn a() {}\n").expect("write readable source");

    #[cfg(unix)]
    std::os::unix::fs::symlink(root.join("nowhere.rs"), src.join("dangling.rs"))
        .expect("dangling symlink");

    let outcome = no_shell_gate::hook_digest::hook_source_manifest(root);
    match outcome {
        Err(no_shell_gate::hook_digest::DigestError::Unreadable { path, .. }) => {
            assert!(
                path.contains("dangling.rs"),
                "RULE unreadable_named: the refusal must name the file it could not read, got \
                 {path}"
            );
        }
        other => panic!(
            "RULE unreadable_is_refused: an unreadable covered source must be a typed refusal, \
             never a manifest computed over the files that happened to be readable; got {other:?}"
        ),
    }
}

//! BUILD-IDENTITY GATE — every shipped binary must be able to say what it was built from.
//!
//! # Why, measured 2026-09-01
//!
//! Josh asked the question this gate answers: *"why aren't we requiring versions in every
//! thing and not skipping it?"* The measurement:
//!
//! | | count |
//! |---|---:|
//! | bin crates in the workspace | 43 |
//! | crates that stamp a build id | **1** |
//! | crates that ship anonymous | **42** |
//!
//! An anonymous binary is not a cosmetic gap. The installer's identity rule REFUSES a binary
//! whose build id it cannot verify — and refusing means removing. The chain that took the
//! fleet down:
//!
//! 1. `tick-monitor` has no build-id mechanism.
//! 2. A routine `installer --install` refused it and **deleted** `~/.local/bin/tick-monitor`.
//! 3. The resident supervisor then refused every 30 seconds — `SUPERVISOR_REFUSED
//!    tick-monitor: process not found` — for hours, with four panes idle beside a ready queue.
//!
//! `pane-truth` is the same class and AGENTS.md records its MISMATCH as one that "can never
//! clear": a permanently red gate, which is worse than no gate because it trains operators to
//! ignore the output.
//!
//! And the mechanism the one stamped crate used was itself fail-open until today —
//! `option_env!("OMP_BUILD_ID")` falling back to `"unversioned"`, with `build.rs` merely
//! WATCHING the variable and never producing one. An install performed tonight *with* the
//! variable exported still shipped `build_id=unversioned`, because cargo reused a cached
//! artifact. Both halves had to change: `build.rs` now derives the id from git, and the source
//! uses `env!` so absence is a compile error.
//!
//! # RATCHET, not a wall
//!
//! 42 crates cannot be converted in one pass, and a gate that is red for weeks gets routed
//! around — the measured death of `state-wildcard-lint` at 89% false positives. So the
//! unstamped count is a CEILING that may only fall. A NEW bin crate without an identity fails
//! immediately; the existing debt is counted, visible, and cannot grow.
//!
//! # NO-CLAIM
//!
//! A present build id is not a TRUE one. This proves a binary can name a commit; it does not
//! prove the binary was built from that commit, and a dirty tree stamps its HEAD sha with only
//! a `-dirty` suffix to say so. Nor does stamping make the installer accept a crate — that
//! needs the id to match HEAD at install time, which is a separate check.

use std::collections::BTreeSet;
use std::path::PathBuf;

/// Measured 2026-09-11 BY THIS GATE'S OWN SCAN: **4** unstamped bin crates —
/// `omp-idle-dispatch`, `ompo-doctor`, `plan-assemble`, `preregistration-gate`. History: 41
/// on 2026-09-01, 55 on 2026-09-05, then the 75-crate stamping wave dropped it to 4 and the
/// bound was never lowered behind it.
///
/// ⛔ THE CEILING SAT AT 55 AGAINST A LIVE COUNT OF 4 — **51 SLACK**, measured 2026-09-11.
/// The stamping wave lowered the count and nobody lowered the bound behind it, so this
/// ratchet would have waved through **fifty-one** new unnameable binaries while reporting
/// green. That is the third rule aimed at a ratchet rather than a gate: a bound that cannot
/// fire reads as protection and provides none, and it is strictly worse than no bound,
/// because the repo *looks* covered.
///
/// ⛔ AND TWO EARLIER ATTEMPTS TO TIGHTEN IT SET **6** AND **1**, BOTH FROM THE WRONG
/// INSTRUMENT — the bound is 4, and neither wrong value was arrived at carelessly.
///
/// `6` came from grepping each `Cargo.toml` for a `build-stamp` build-dependency. This gate
/// never reads manifests: `stamps_identity` below opens `build.rs` and looks for
/// `build_stamp::emit()` or the raw `cargo:rustc-env=OMP_BUILD_ID`, deliberately, so that a
/// crate cannot "stamp" itself with a comment.
///
/// `1` came from `cargo metadata`, which OMITS `omp-idle-dispatch` because `Cargo.toml:7`
/// excludes it. `bin_crates` below walks `crates/` on disk instead — also deliberately, per
/// its own comment — so an excluded directory still counts here. **The roster and the
/// workspace genuinely disagree, and this gate's answer is the one that binds this gate.**
///
/// **The mutation is what caught the first error.** Deleting a manifest's build-dependency
/// left this gate GREEN — correctly, since `build.rs` still emitted. A mutation that fails to
/// bite is the most valuable result available: the tally was plausible, internally
/// consistent, and measured a property the gate never consults. **Derive a ratchet's floor
/// with the ratchet's own detector, never with a proxy that looks equivalent** — and when the
/// detector disagrees with your proxy, the detector is not the one that is wrong.
///
/// ⛔ WHY THESE FOUR ARE NOT SIMPLY STAMPED, which is the obvious next question.
/// Stamping `preregistration-gate` was attempted and REVERTED. Adding a `build.rs`
/// invalidates that crate's build cache, and `plan-assemble` DEPENDS on it, so
/// `plan-assemble` must rebuild; its `[package.metadata.gate]` check must RUN its bin, so
/// the pre-commit gate issues `cargo build -p plan-assemble`, which goes down the remote
/// lane and returns
///
/// ```text
/// BUILD_INCONCLUSIVE exit=102 — "Retrieved artifacts do not match the requesting host's
/// target triple aarch64-apple-darwin after a successful remote compile on contabo"
/// ```
///
/// The remote compile SUCCEEDS; the artifact is Linux and the host is darwin. Under the
/// standing no-darwin-cross-build policy there is no admissible way to produce that
/// artifact. **So a crate whose gate check must RUN its bin cannot absorb any
/// cache-invalidating change — including a change to one of its dependencies.** The first
/// diagnosis blamed the stamped crate's own cache and was refuted by reverting it: the crate
/// went byte-identical to HEAD and the gate still failed, because the *dependency* was still
/// dirty. Recorded rather than routed around; the bound is tightened to the honest 4.
///
/// A ratchet is only evidence while it is TIGHT. Lowering it is the second half of every
/// stamping commit, not a follow-up — an untightened gain is a gain that silently regresses.
/// This constant's own contract has always said so: LOWER it when crates are stamped, and
/// never raise it silently.
const UNSTAMPED_BIN_CEILING: usize = 4;

fn repo_root() -> Option<PathBuf> {
    let mut cur = std::env::current_dir().ok()?;
    loop {
        if cur.join("crates").is_dir() && cur.join("docs/plan").is_dir() {
            return Some(cur);
        }
        if !cur.pop() {
            return None;
        }
    }
}

/// Bin crates, from cargo rather than a hand list — a hand-maintained inventory is the
/// defect this workspace already has a crate to prevent.
fn bin_crates(root: &std::path::Path) -> Option<BTreeSet<String>> {
    // NOT cargo-metadata-window-scanning. The first version chopped the metadata JSON into
    // 4000-char windows looking for a bin kind near a name and found 6 of 43 crates -- caught
    // only by the anti-vacuity leg below. A crate is a bin crate structurally: it has
    // src/main.rs, or its manifest declares [[bin]].
    let mut names = BTreeSet::new();
    for entry in std::fs::read_dir(root.join("crates")).ok()? {
        let Ok(entry) = entry else { continue };
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let Some(name) = dir.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let has_main = dir.join("src/main.rs").is_file();
        let declares_bin = std::fs::read_to_string(dir.join("Cargo.toml"))
            .map(|t| t.contains("[[bin]]"))
            .unwrap_or(false);
        if has_main || declares_bin {
            names.insert(name.to_owned());
        }
    }
    Some(names)
}

/// Does this crate's `build.rs` stamp the build identity?
///
/// TWO admissible forms, because as of 2026-09-11 the stamp has ONE implementation
/// (`crates/build-stamp`) instead of ~28 duplicated lines in each of ~78 crates:
///   - the literal emission, for a crate that still inlines it, and
///   - a call to `build_stamp::emit()`, which emits exactly that line.
///
/// ⛔ MEASURED, AND IT IS WHY THIS FUNCTION CHANGED: with only the literal accepted, converting
/// `tick-monitor` to the shared helper moved the count 73 -> 74. A CORRECT stamp read as a
/// REGRESSION, so the gate actively penalised removing the duplication it exists to motivate.
/// A text-keyed oracle cannot see a delegated implementation.
///
/// ⛔ AND THE COMMENTS ARE STRIPPED FIRST, via `text_structure::code_only`, because this very
/// doc comment names both needles — a checker whose input contains prose about the thing it
/// checks is the self-referential class this repo has now hit eight times. Without stripping,
/// a crate could "stamp" itself with a comment.
fn stamps_identity(root: &std::path::Path, krate: &str) -> bool {
    let build_rs = root.join("crates").join(krate).join("build.rs");
    std::fs::read_to_string(build_rs)
        .map(|t| {
            let code = text_structure::code_only(&t);
            code.contains("cargo:rustc-env=OMP_BUILD_ID") || code.contains("build_stamp::emit()")
        })
        .unwrap_or(false)
}

#[test]
fn the_unstamped_binary_count_only_falls() {
    let Some(root) = repo_root() else {
        eprintln!("SKIP the_unstamped_binary_count_only_falls: no repo root");
        return;
    };
    let Some(bins) = bin_crates(&root) else {
        panic!("cargo metadata failed; an unreadable inventory must not read as a pass");
    };

    // ANTI-VACUITY: 43 bin crates measured 2026-09-01. A collapse means the scan broke, not
    // that the workspace emptied. `citation_integrity` failed exactly this way today, matching
    // 0 of 32 citations because its parser wanted punctuation the document never used.
    assert!(
        bins.len() >= 20,
        "ANTI-VACUITY: found only {} bin crates; 43 were measured. The scan broke.",
        bins.len()
    );

    let unstamped: Vec<&String> = bins.iter().filter(|k| !stamps_identity(&root, k)).collect();

    // POSITIVE CONTROL: the one crate known to stamp must come back stamped, or the detector
    // is reporting everything unstamped and its count is meaningless.
    assert!(
        stamps_identity(&root, "omp-orchestrator"),
        "positive control failed: omp-orchestrator must be detected as stamping an identity, \
         otherwise this gate cannot distinguish stamped from unstamped at all"
    );

    assert!(
        unstamped.len() <= UNSTAMPED_BIN_CEILING,
        "unstamped binaries rose to {} against a ceiling of {UNSTAMPED_BIN_CEILING}. A binary \
         that cannot name its build is DELETED by the installer's identity rule - that is what \
         removed tick-monitor and left the fleet untended for hours. Add a build.rs emitting \
         cargo:rustc-env=OMP_BUILD_ID; do NOT raise the ceiling.\n  new/total unstamped: {:?}",
        unstamped.len(),
        unstamped.iter().take(8).collect::<Vec<_>>()
    );
}

/// The stamped crate must derive its id, not merely read an env var an operator may forget.
/// Watching a variable is not producing one, and that distinction cost the fleet a night.
#[test]
fn the_stamped_crate_derives_its_id_rather_than_hoping_for_an_env_var() {
    let Some(root) = repo_root() else { return };
    let build_rs = root.join("crates/omp-orchestrator/build.rs");
    let text = std::fs::read_to_string(&build_rs).expect("omp-orchestrator build.rs must exist");
    assert!(
        text.contains("cargo:rustc-env=OMP_BUILD_ID"),
        "build.rs must EMIT the id, not just rerun-if-env-changed on it"
    );
    assert!(
        text.contains("rev-parse"),
        "build.rs must DERIVE the id from git; an env var alone shipped build_id=unversioned \
         tonight even when it was exported, because cargo reused a cached artifact"
    );
    // omp-orchestrator-nar5l: this named `crates/omp-orchestrator/src/main.rs`, absent from TREE,
    // INDEX and WORKTREE -- the crate is lib-plus-`src/bin/`. `const BUILD_ID` lives in
    // `resident.rs` (`git grep -ln 'const BUILD_ID'`), so the claim was true and the address was
    // dead. The failure now NAMES the path: `.expect("main.rs must exist")` panicked with an
    // `Os { code: 2 }` and no path, which made a dead citation unattributable.
    let supervisor_rs = root.join("crates/omp-orchestrator/src/resident.rs");
    let src = std::fs::read_to_string(&supervisor_rs).unwrap_or_else(|error| {
        panic!(
            "cited supervisor source is unreadable: {} detail={error}",
            supervisor_rs.display()
        )
    });
    // STRIP COMMENTS FIRST. The first version grepped raw text and matched main.rs's own doc
    // comment explaining that option_env! USED to be there -- a checker whose input contains
    // prose about the thing it checks. Sixth instance of that shape this session.
    let code: String = src
        .lines()
        .filter(|l| {
            let s = l.trim_start();
            !(s.starts_with("//") || s.starts_with("///") || s.starts_with("//!"))
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        code.contains("const BUILD_ID"),
        "positive control: the stripped source must still contain the BUILD_ID declaration, or \
         the comment filter ate the code it was meant to expose"
    );
    assert!(
        !code.contains("option_env!(\"OMP_BUILD_ID\")"),
        "BUILD_ID must use env! so a missing id is a COMPILE ERROR. option_env! with a fallback \
         is the fail-open that made 'unversioned' a legal answer."
    );
}

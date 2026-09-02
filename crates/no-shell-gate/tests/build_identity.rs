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

/// Measured 2026-09-01 BY THIS GATE'S OWN SCAN. Lower it as crates gain identity; never
/// raise it.
///
/// It was first set to 42 from a cargo-metadata count while the gate counted structurally
/// and produced 41 - one slot of slack, and the mutation probe (add an unstamped bin crate)
/// passed when it should have failed. A ratchet seeded from a NEIGHBOURING measurement is
/// not a ratchet; it is a ceiling with room to grow into.
const UNSTAMPED_BIN_CEILING: usize = 41;

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
        let Some(name) = dir.file_name().and_then(|n| n.to_str()) else { continue };
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

fn stamps_identity(root: &std::path::Path, krate: &str) -> bool {
    let build_rs = root.join("crates").join(krate).join("build.rs");
    std::fs::read_to_string(build_rs)
        .map(|t| t.contains("cargo:rustc-env=OMP_BUILD_ID"))
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
    let main_rs = root.join("crates/omp-orchestrator/src/main.rs");
    let src = std::fs::read_to_string(&main_rs).expect("main.rs must exist");
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

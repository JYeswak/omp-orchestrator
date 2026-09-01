//! SPAWN CONTRACT GATE — a crate that spawns a child process must route through
//! `subprocess-contract`, or carry a named allowance with a reason.
//!
//! # The binding contract this enforces
//!
//! `AGENTS.md`, on asupersync 0.4.9:
//!
//! > Every subprocess — `tmux`, `ntm`, `br`, `bv`, a build — is cancellable work
//! > with a deadline. […] **Kill the process GROUP, never the pid.** […] **Drain
//! > both pipes.** Undrained stdout+stderr with a `try_wait()` poll deadlocks past
//! > ~64 KiB. […] **A timeout is not a verdict.**
//!
//! Each of those is a property of *how* a child is spawned, so a bare
//! `Command::new` cannot satisfy them. `subprocess-contract` is where they live.
//!
//! # Measured state, 2026-09-01
//!
//! 12 crates contain `Command::new`; 6 declare `subprocess-contract`. The overlap
//! is partial, so **8 crates and 38 raw spawn sites do not route through it** —
//! including `tick-monitor` (4 sites, the orchestrator's primary sensor, spawning
//! `tmux`) and `omp-rpc-session` (3 sites, spawning the OMP RPC child).
//!
//! `GradeCrates` filed this in round 13 as a MAJOR:
//!
//! > `omp-rpc-session` and `omp-inventory-map` both spawn processes and neither
//! > routes through `subprocess-contract`.
//!
//! # Why an allowance list rather than a hard failure
//!
//! Lints and test harnesses spawn `cargo`, `git` and `grep` in a build context
//! where a deadline is the harness's job, not theirs. Forcing them through the
//! runtime contract would be ceremony. So the list is **explicit and reasoned**,
//! following `franken_lean`'s `UNWIRED_LANE_ALLOWANCE` shape — an exception is a
//! named row with a reason, never silence.
//!
//! # What this does NOT prove
//!
//! Declaring the dependency is not using it. A crate can depend on
//! `subprocess-contract` and still call `Command::new` directly beside it —
//! `subprocess-contract` itself does, legitimately, since it *is* the wrapper.
//! Proving every call site routes correctly needs per-site analysis, which is
//! unbuilt. This gate raises the floor from *no relationship at all* to *declared
//! relationship or stated reason*, which is strictly weaker than the contract and
//! strictly stronger than nothing.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root two levels up")
        .to_path_buf()
}

/// Crates permitted to spawn without the runtime contract, each with the reason.
///
/// Adding a row is cheap and auditable; leaving one out is a build failure. That
/// asymmetry is the point.
const SPAWN_ALLOWANCE: &[(&str, &str)] = &[
    (
        "subprocess-contract",
        "it IS the wrapper — its own Command::new calls are the implementation",
    ),
    (
        "no-shell-gate",
        "test harness: spawns cargo/git/grep to derive figures, under the harness's own deadline",
    ),

    (
        "pre-delete-citation-check",
        "pre-commit-time check, bounded by the hook's lifetime rather than a runtime deadline",
    ),
    (
        "receiver-receipt",
        "single tmux capture-pane read at hook time; ALLOWANCE IS WEAK — this is runtime-adjacent \
         and should route through the contract once the fence lands",
    ),
    (
        "installer",
        "PROVISIONAL, and the most user-visible of these: 5 sites. A hung install is a failure \
         a real adopter experiences directly, so this is the allowance whose absence of a \
         deadline costs the most",
    ),
    (
        "kernel-bypass-gate",
        "PROVISIONAL: 3 sites in a gate that inspects kernel-level bypass; runs at hook time \
         under the hook's lifetime, but the boundary is not stated anywhere",
    ),
    (
        "dispatch-silence-watch",
        "PROVISIONAL: 2 sites. Wired as a path dependency this session (gate-wiring-wave2-at2), \
         so it is newly live and inherits no deadline yet",
    ),
    (
        "fleet-composite",
        "PROVISIONAL: 2 sites. One of the three shell->Rust ported crates; the shell original \
         had no deadline either, so this is inherited debt rather than new",
    ),
    (
        "tick-monitor",
        "PROVISIONAL, recorded as debt not as a decision: 4 sites spawning tmux from the \
         orchestrator's primary sensor. This is the allowance most likely to be wrong, and \
         GradeCrates flagged its sibling in round 13",
    ),
    (
        "omp-rpc-session",
        "PROVISIONAL, recorded as debt: 3 sites spawning the OMP RPC child, which is exactly \
         the cancellable-work-with-a-deadline case the contract exists for",
    ),
    (
        "omp-inventory-map",
        "PROVISIONAL, recorded as debt: named by GradeCrates in round 13",
    ),
];

fn crates_with_spawn(root: &Path) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(root.join("crates")) else {
        return out;
    };
    for e in entries.flatten() {
        let dir = e.path();
        if !dir.is_dir() {
            continue;
        }
        let name = match dir.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_owned(),
            None => continue,
        };
        let mut count = 0usize;
        let mut stack = vec![dir.join("src")];
        while let Some(d) = stack.pop() {
            let Ok(items) = std::fs::read_dir(&d) else { continue };
            for it in items.flatten() {
                let p = it.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().is_none_or(|x| x != "rs") {
                    continue;
                }
                if let Ok(body) = std::fs::read_to_string(&p) {
                    count += body.matches("Command::new").count();
                }
            }
        }
        if count > 0 {
            out.push((name, count));
        }
    }
    out.sort();
    out
}

fn declares_contract(root: &Path, crate_name: &str) -> bool {
    let manifest = root.join("crates").join(crate_name).join("Cargo.toml");
    std::fs::read_to_string(manifest)
        .map(|t| t.contains("subprocess-contract"))
        .unwrap_or(false)
}

#[test]
fn every_spawning_crate_routes_through_the_contract_or_is_allowed() {
    let root = repo_root();
    let spawners = crates_with_spawn(&root);
    assert!(
        !spawners.is_empty(),
        "ANTI-VACUITY: no crate contains Command::new. This workspace drives tmux, ntm, br \
         and cargo, so a zero here means the scan is broken — the silent-false-zero defect, \
         not a clean bill."
    );
    let allowed: std::collections::HashSet<&str> =
        SPAWN_ALLOWANCE.iter().map(|(c, _)| *c).collect();
    let unrouted: Vec<String> = spawners
        .iter()
        .filter(|(c, _)| !declares_contract(&root, c) && !allowed.contains(c.as_str()))
        .map(|(c, n)| format!("{c} ({n} site(s))"))
        .collect();
    assert!(
        unrouted.is_empty(),
        "{} crate(s) spawn child processes with no subprocess-contract dependency and no \
         allowance row:\n{:#?}\n\n\
         AGENTS.md: every subprocess is cancellable work with a deadline — kill the process \
         GROUP, drain both pipes, and a timeout is not a verdict. A bare Command::new \
         satisfies none of those.\n\n\
         Either add the dependency, or add a row to SPAWN_ALLOWANCE with the reason.",
        unrouted.len(),
        unrouted
    );
}

#[test]
fn every_allowance_row_names_a_crate_that_still_spawns() {
    // An allowance for a crate that no longer spawns is a stale exemption, and a
    // stale exemption is how an allowlist quietly becomes a rubber stamp. This is
    // the same rot the gate census had when it hardcoded three crates as
    // permanently Unreachable.
    let root = repo_root();
    let spawners: std::collections::HashSet<String> =
        crates_with_spawn(&root).into_iter().map(|(c, _)| c).collect();
    let stale: Vec<&str> = SPAWN_ALLOWANCE
        .iter()
        .map(|(c, _)| *c)
        .filter(|c| !spawners.contains(*c))
        .collect();
    assert!(
        stale.is_empty(),
        "{} allowance row(s) name a crate that no longer spawns: {:?}\n\
         Remove them — an exemption nobody needs is an exemption nobody rechecks.",
        stale.len(),
        stale
    );
}

#[test]
fn every_allowance_row_carries_a_reason() {
    let empty: Vec<&str> = SPAWN_ALLOWANCE
        .iter()
        .filter(|(_, why)| why.trim().len() < 20)
        .map(|(c, _)| *c)
        .collect();
    assert!(
        empty.is_empty(),
        "{} allowance row(s) carry no usable reason: {:?}\n\
         A row without a reason is silence with extra steps.",
        empty.len(),
        empty
    );
}

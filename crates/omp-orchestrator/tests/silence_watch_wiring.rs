#![forbid(unsafe_code)]
//! `dispatch-silence-watch` IS WIRED, AND THE PROOF IS THE SPAWN SITE — `omp-orchestrator-poumg.1`.
//!
//! # Why this assertion lives HERE and not in `dispatch-silence-watch`
//!
//! **The call site is in THIS crate**, at `src/resident.rs`, so this crate owns the assertion.
//! Putting it in the callee's suite would mean an engineer refactoring `resident.rs` gets a RED
//! in a crate they never touched, in a suite whose owner cannot see why. Locality of failure
//! follows locality of the construct — the same rule that moved `silent-success-census`'s
//! positive controls onto files it owns (`poumg.5`). Cross-lane edit, authorised and disclosed.
//!
//! # What this file refuses to accept as wiring
//!
//! **1. A crontab entry.** The deleted test `dispatch_silence_watch_is_in_crontab` demanded one.
//! That is unsatisfiable by construction: the binary's usage is
//! `dispatch-silence-watch <bead-id> <session> <dispatch-assignee> <dispatch-epoch>
//! <deadline-secs>` — five required positionals, four of them per-dispatch facts. **A cron row
//! has no bead to name**, so any entry would hard-code a bead id and an epoch and be meaningless
//! three minutes later: a fabricated trigger that reads as protection. The test also anchored its
//! cadence to `controller-tick`, whose only appearance in the live crontab is a comment recording
//! its own retirement by `5621319` — scheduled three minutes after a tick that no longer exists.
//!
//! **2. The coverage census.** `COVERAGE_WAVE_OUTPUT_CRATES` carries a `dispatch-silence-watch`
//! row, and asserting on it would be a VACUOUS GREEN: `coverage_output_reachability` (lib.rs:684)
//! returns `Reachable` when `crates/<name>/Cargo.toml` **is a file**. That is existence, not
//! reachability — it cannot distinguish a wired crate from an unwired one, and every crate in the
//! workspace satisfies it. Reported rather than used.
//!
//! # What it accepts
//!
//! The three links that make a spawn real, each independently checkable: the caller names the
//! binary, the call site exists in the caller's own source, and the named binary is a real
//! `[[bin]]` target. Break any one and the chain is not a chain.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| {
        panic!(
            "SILENCE_WATCH_WIRING_UNREADABLE path={} detail={error} — an unreadable source file \
             is an ERROR, never a pass: it is the state in which nothing is checked and nothing \
             reports",
            path.display()
        )
    })
}

/// The binary name the caller spawns, as the caller declares it.
const EXPECTED_BIN: &str = "dispatch-silence-watch";

/// LINK 1 + 2: the caller names the binary AND spawns it, in this crate's own source.
///
/// The spawn is `invoke(cx, config, SILENCE_WATCH, &args)` inside `run_silence_watch`. This leg
/// checks the constant resolves to the real binary name and that the call site is present, rather
/// than that any particular line reads a particular way: it locates the function and asserts the
/// invocation appears within it.
#[test]
fn the_resident_spawns_the_silence_watch_binary() {
    let source = read(&repo_root().join("crates/omp-orchestrator/src/resident.rs"));

    assert!(
        source.contains(&format!("const SILENCE_WATCH: &str = \"{EXPECTED_BIN}\"")),
        "the resident no longer names `{EXPECTED_BIN}`. If the binary was renamed, this leg and \
         the callee's [[bin]] must move together; if the watch was removed, delete this file \
         rather than weakening it"
    );

    let start = source
        .find("async fn run_silence_watch(")
        .expect("run_silence_watch is GONE from the resident — the silence watch is unwired");
    let body = &source[start..];
    let end = body
        .find("\n}\n")
        .expect("run_silence_watch has no terminating brace at column 0");
    let body = &body[..end];

    assert!(
        body.contains("invoke(cx, config, SILENCE_WATCH"),
        "run_silence_watch exists but no longer SPAWNS the binary. A function that returns a \
         verdict without invoking the watch is BUILT != WIRED with extra steps"
    );
    assert!(
        body.contains("require_success("),
        "the spawn's exit status is not checked. An unchecked spawn cannot distinguish a verdict \
         from a crash, which is the silence this crate exists to detect"
    );
}

/// LINK 3: the spawned name is a real `[[bin]]`, not a string that resolves to nothing.
///
/// This is the half that a source grep alone cannot supply. A caller can name any string; only
/// the callee's manifest says whether that string is executable.
#[test]
fn the_spawned_name_is_a_real_bin_target() {
    let manifest = read(&repo_root().join("crates/dispatch-silence-watch/Cargo.toml"));
    assert!(
        manifest.contains("[[bin]]") && manifest.contains(&format!("name = \"{EXPECTED_BIN}\"")),
        "`{EXPECTED_BIN}` is spawned by the resident but declares no [[bin]] of that name — the \
         caller names a binary that cannot exist"
    );
}

/// THE CONTRACT BETWEEN THE TWO CRATES: the caller passes exactly the positionals the callee
/// requires.
///
/// This is the leg that would have caught the real hazard. The callee takes FIVE required
/// positional arguments; the caller builds a five-element `args` vector. If either side changes
/// its arity alone, every dispatch fails at runtime with a usage error and the silence watch
/// reports nothing — a failure mode indistinguishable, from outside, from a quiet board.
#[test]
fn caller_and_callee_agree_on_the_argument_count() {
    let callee = read(&repo_root().join("crates/dispatch-silence-watch/src/main.rs"));
    let usage_line = callee
        .lines()
        .find(|line| line.contains("usage: dispatch-silence-watch"))
        .expect("the callee no longer prints a usage line; its arity is unstatable");
    let required = usage_line.matches('<').count();
    assert_eq!(
        required, 5,
        "the callee's declared arity changed to {required}; the resident's args vector must \
         change with it. usage: {usage_line}"
    );

    let caller = read(&repo_root().join("crates/omp-orchestrator/src/resident.rs"));
    let start = caller
        .find("async fn run_silence_watch(")
        .expect("run_silence_watch is gone");
    let args_start = caller[start..]
        .find("let args = vec![")
        .expect("run_silence_watch no longer builds an args vector");
    let args_block = &caller[start + args_start..];
    let args_end = args_block.find("];").expect("unterminated args vector");
    let supplied = args_block[..args_end].matches(',').count();
    assert_eq!(
        supplied, required,
        "ARITY DRIFT: the resident supplies {supplied} positionals and `{EXPECTED_BIN}` requires \
         {required}. Every dispatch would fail with a usage error, and a watch that never runs \
         reports no silence — which looks exactly like no silence to detect"
    );
}

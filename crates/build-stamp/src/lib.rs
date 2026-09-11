#![forbid(unsafe_code)]
//! The build-identity stamp, as ONE implementation instead of N copies.
//!
//! # Why this crate exists
//!
//! A binary that cannot name its build is DELETED by the installer's identity rule. That rule
//! is what removed `tick-monitor` and left the fleet untended for hours, and it is why
//! `no-shell-gate`'s `the_unstamped_binary_count_only_falls` exists.
//!
//! Measured 2026-09-11: **89 bin targets, 6 crates emitting `OMP_BUILD_ID`.** The stamped count
//! had not moved while the ceiling went `41 -> 55 -> 73` — the workspace growing past a frozen
//! numerator. Closing that by hand means ~28 lines of identical `build.rs` in each of ~83
//! crates, which is ~2,300 lines whose only property is being the same. **A defect fixed by
//! mass duplication is a second defect**, and the duplicate copies drift: the six existing
//! `build.rs` files are already NOT byte-identical.
//!
//! So the per-crate cost is one line:
//!
//! ```ignore
//! fn main() { build_stamp::emit(); }
//! ```
//!
//! # What it guarantees, and what it does not
//!
//! It emits `cargo:rustc-env=OMP_BUILD_ID=<value>` exactly once, choosing the first available of
//! an explicit `OMP_BUILD_ID`, then `git rev-parse HEAD`, then the literal `unavailable`.
//!
//! **`unavailable` IS A REAL OUTCOME AND IS NOT AN ERROR.** A source tarball with no `.git` and
//! no environment override genuinely cannot name its build, and a `build.rs` that panicked there
//! would make the crate unbuildable off a git checkout. The stamp's job is that the binary can
//! always ANSWER the question — including answering "I do not know" — because the failure being
//! prevented is a binary with no `OMP_BUILD_ID` symbol at all, which reads to the installer as
//! an unidentifiable artifact.

use std::process::Command;

/// Emit the build-identity stamp. Call from a crate's `build.rs` `main`.
///
/// Precedence: `OMP_BUILD_ID` env (trimmed, non-empty) -> `git rev-parse HEAD` -> `unavailable`.
pub fn emit() {
    println!("cargo:rerun-if-env-changed=OMP_BUILD_ID");
    // Relative to the CRATE's directory, which is build.rs's cwd. Every workspace member sits
    // at crates/<name>/, so the repository .git is two levels up.
    for path in [
        "../../.git/HEAD",
        "../../.git/index",
        "../../.git/packed-refs",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    println!("cargo:rustc-env=OMP_BUILD_ID={}", resolve());
}

/// The precedence chain, separated from the `println!` side effects so it is testable.
///
/// Takes its inputs as arguments rather than reading the environment: a test that sets a process
/// -wide env var races every other test in the binary, and this repo has already paid for one
/// shared-mutable-state test defect.
pub fn resolve_from(env: Option<&str>, git: Option<&str>) -> String {
    env.map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| git.map(str::trim).filter(|value| !value.is_empty()))
        .unwrap_or("unavailable")
        .to_owned()
}

fn resolve() -> String {
    let env = std::env::var("OMP_BUILD_ID").ok();
    let git = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned());
    resolve_from(env.as_deref(), git.as_deref())
}

#[cfg(test)]
mod tests {
    use super::resolve_from;

    /// KNOWN-GOOD: an explicit build id wins and is trimmed.
    #[test]
    fn an_explicit_build_id_wins_over_git() {
        assert_eq!(resolve_from(Some("  abc123\n"), Some("deadbeef")), "abc123");
    }

    /// The fallback ORDER is the property, not merely that a value appears.
    #[test]
    fn git_is_used_only_when_the_env_is_absent_or_blank() {
        assert_eq!(resolve_from(None, Some("deadbeef\n")), "deadbeef");
        // A BLANK env var must not shadow a good git answer. This is the arm that a naive
        // `env.or(git)` fails: `Some("")` is `Some`, so precedence alone returns the empty
        // string and the binary ships a stamp that is present and says nothing.
        assert_eq!(resolve_from(Some("   "), Some("deadbeef")), "deadbeef");
    }

    /// KNOWN-BAD DIRECTION: with nothing available the answer is the literal `unavailable`,
    /// never an empty string. An empty `OMP_BUILD_ID=` is exactly the "present but says nothing"
    /// shape the installer's identity rule cannot distinguish from a healthy stamp.
    #[test]
    fn nothing_available_yields_unavailable_and_never_an_empty_string() {
        assert_eq!(resolve_from(None, None), "unavailable");
        assert_eq!(resolve_from(Some(""), Some("")), "unavailable");
        assert!(!resolve_from(None, None).is_empty());
    }
}

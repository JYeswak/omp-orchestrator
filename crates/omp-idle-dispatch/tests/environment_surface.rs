//! WITHDRAWN PARITY CLAIM — this file is a record, not a check. Ruled 2026-09-11.
//!
//! WHAT THE CLAIM WAS. `every_historical_environment_surface_is_covered_by_its_rust_replacement`
//! read each deleted shell/python lane out of git at the port commit's parent, parsed its
//! environment surface (every `export`ed name a lane depended on), parsed the same surface
//! out of the Rust replacement's source, and asserted the Rust side covered it. Four
//! contracts: `bin/fleet-composite.py`, `bin/omp-idle-dispatch.sh`,
//! `bin/pane-dispatch-ready.sh`, `bin/wired-but-inert-guard.sh`.
//!
//! WHY IT IS GONE RATHER THAN REPAIRED. It could never run in this repository, and the
//! remedy is evidence this checkout cannot produce:
//!
//!   const PORT_COMMIT: &str = "45c613d^";
//!   historical_source(): format!("{PORT_COMMIT}:{path}") -> git show -> assert!(success)
//!
//!   git rev-parse --verify 45c613d                 -> fatal: Needed a single revision
//!   git show 45c613d^:bin/omp-idle-dispatch.sh     -> fatal: invalid object name
//!   git log --all --diff-filter=D -- 'bin/*'       -> EMPTY across all 1633 commits
//!
//! So every leg failed at its first `git show`, and "find the real revision" is unobtainable
//! here: the history it keys on is not in this repository, and the `control-plane@` prefix
//! the rest of this crate now carries names a repository this checkout cannot reach.
//!
//! WHY NOBODY NOTICED, and this is the part that outranks the oracle itself. `Cargo.toml:7`
//! reads `exclude = ["crates/omp-idle-dispatch"]`. The crate is absent from
//! `cargo metadata`, so no `cargo test -p` runs it, no CI lane builds it, and no gate scans
//! it as a member. A WORKSPACE-EXCLUDED CRATE HAS NO COMPILER, NO CI AND NO GATE: every
//! check inside one is UNRUN BY CONSTRUCTION, and the repository still READS as protected
//! by them. That is gate rule 3 -- a gate that cannot fire is worse than none -- reached
//! through the build graph instead of through the gate's own logic.
//!
//! THE DEATH CONDITION, so this record can end. Re-derive the parity claim if and only if
//! BOTH hold: (1) this crate re-enters the workspace at `Cargo.toml`, so a compiler and CI
//! see it at all; and (2) a RESOLVABLE revision carrying `bin/` is available to
//! `git show` from this checkout. Until both hold, the claim stays withdrawn and the four
//! contracts above are documentation of what was once compared, not a statement that it is
//! compared now.
//!
//! WHAT STILL CHECKS THE SAME SUBJECT, so the deletion does not leave a hole: the env-parity
//! contract for this port lives in `tests/environment.rs`, which asserts the Rust binary's
//! OWN set-if-unset behaviour under `env_clear` and needs no historical source. That suite
//! records the deleted script's exports as a transcription (marked as such) rather than
//! re-deriving them, and is only as good as whoever transcribed it -- which is precisely the
//! guarantee this file used to provide and no longer does.

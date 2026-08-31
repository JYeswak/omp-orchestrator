#![forbid(unsafe_code)]

//! Ack spine (slice a): the asupersync step ledger.
//!
//! Every dispatch step is a region-owned operation taking &Cx FIRST and
//! emitting exactly ONE typed row. The step count is ASSERTED — a dispatch
//! that emits fewer rows than steps taken FAILS.
//!
//! THE DEFECT THIS REPLACES (measured 2026-08-31): the supervisor refused 29
//! consecutive ticks with a byte-identical log line. 29 copies of one sentence
//! carry no state; a typed row per step is readable.
//!
//! CANCEL CONSISTENCY: cancellation mid-dispatch leaves the ledger CONSISTENT
//! and the intent RECOVERABLE, never a half-state. The existing
//! pending-dispatch marker is the precedent — it survived 29 refused ticks
//! and correctly refused to retry.
//!
//! ANTI-VACUITY: zero steps observed is an ERROR, never a clean dispatch.

/// Slice-c: the ack detector — an ack is a bead comment confirmed by READ-BACK.
/// Owned by SilverWolf (%1409); `tests/ack_detector.rs` imports it as
/// `ack_spine::ack`, which is the correct form and requires this line.
pub mod ack;

/// The three ack authorities — transport success, observational delivery, and
/// ack — proven non-substitutable. Owned by BlueLantern (%1414).
///
/// WIRING NOTE, measured 2026-08-31: `tests/authorities.rs` reaches this file
/// with `#[path = "../src/authorities.rs"] mod authorities;`, compiling a
/// PRIVATE COPY into the test binary. Those 8 tests were genuinely green while
/// the library contained no `authorities` at all and `cargo build --lib` could
/// not typecheck it — real green, absent wiring. This line is the fix; the test
/// should switch to `use ack_spine::authorities::…` so one copy is compiled.
pub mod authorities;

/// Slice-a: the asupersync step ledger — one typed row per dispatch step.
/// Owned by AmberGate (%1408), implemented in `ledger.rs`.
///
/// WIRING NOTE, measured 2026-08-31: `ledger.rs` arrived as an UNTRACKED,
/// BYTE-IDENTICAL copy of the 285 lines that lived here — `diff` from
/// `pub enum StepKind` onward was **0 lines**. The crate therefore defined
/// `StepKind`/`StepRecord`/`StepLedger` twice: once compiled via this file,
/// once inert in `ledger.rs`.
///
/// THAT WAS MY RULE'S COST, not a worker error. Single-writer-per-file put the
/// move in AmberGate's hands and the deletion in mine, so a correct half-move
/// was the only reachable state until I finished it. A rule that splits one
/// edit across two owners must name who closes it; this one did not.
pub mod ledger;

/// Flat re-export, load-bearing: `main.rs:8` does
/// `use ack_spine::{StepKind, StepLedger}`. Moving the types into a module
/// without re-exporting them would break that path silently at the callsite.
pub use ledger::*;

/// The typed heartbeat row — `build_id` + `pid`, findable by a third party.
/// Owned by SilverWolf (%1409), landed in `41744d6`.
///
/// WIRING NOTE: `heartbeat.rs` was committed to HEAD and declared NOWHERE — a
/// sixteenth unwired-source instance, and the first one found already *inside*
/// the repository rather than in a working tree. It replaced
/// `scripts/heartbeat-check.sh`, whose defect was that a shell checker writes
/// nothing typed: BlueLantern cited heartbeat row #93 with `build_id=85828bf`
/// and a search of every path under `~/.local/state` found no file containing
/// `build_id` at all. That identity leg was unverifiable BY CONSTRUCTION.
/// Porting it fixed the writer; this line makes the reader reachable.
pub mod heartbeat;

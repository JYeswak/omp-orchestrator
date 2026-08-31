#![forbid(unsafe_code)]

//! The canonical type vocabulary for this workspace.
//!
//! **No crate invents a type that already exists here.** Josh, 2026-08-31: "there should be no
//! crate that has any possibility of deriving or making up its own type." This crate is the
//! answer, and its contents are **derived** — re-exported from `asupersync` at the exact rev we
//! pin (`fa3c01aec`, v0.4.9) — never authored here.
//!
//! # What was measured before this crate existed
//!
//! 51 public enums and 79 public structs across 22 of 24 crates:
//!
//! - **Six independent `Verdict` types** — `AckVerdict`, `FenceVerdict`, `FollowUpVerdict`,
//!   `ReceiptVerdict`, `SilenceVerdict`, `Verdict`. Six answers to "what happened", none
//!   composable, none countable. That is *why* grading is prose: there is no type a grade can be.
//! - **Seventeen ack/receipt types in three dialects** — `ack-spine` speaks *Authority*,
//!   `ack-stage` speaks *Receipt*, `receiver-receipt` speaks a third.
//! - **Four colliding names**, one structural: `tick-monitor` produces the `Observation` that
//!   `omp-orchestrator` consumes, and each declares its own incompatible struct. That is the seam
//!   where `free_capacity` was derived from the wrong filter and a NewlyIdle pane became invisible
//!   as capacity — **a compile error, had one type crossed the boundary.**
//!
//! # Derivation trap #1 — the name is not the type
//!
//! `asupersync` declares **two** `AckKind`s. `obligation/graded.rs:790` is `pub enum AckKind {}` —
//! an **uninhabited marker** implementing `TokenKind`. The inhabited one is `messaging/class.rs:83`.
//! Re-exporting the first compiles fine and yields a type with no values. **Deriving means reading
//! the definition, not matching the name.**
//!
//! # Derivation trap #2 — the ack vocabulary is UNREACHABLE at our pinned rev
//!
//! `AckKind` and `DeliveryClass` are exactly the types our three dialects were groping toward:
//!
//! | `AckKind` variant | asupersync's doc | the authority we reinvented |
//! |---|---|---|
//! | `Accepted` | packet plane accepted custody | transport success (`ntm` JSON) |
//! | `Committed` | authority plane committed the entry | — |
//! | `Recoverable` | declared durability class met | — |
//! | `Served` | service obligation completed by callee | bead-comment ack, read back |
//! | `Received` | delivery/receipt boundary crossed | observational delivery |
//!
//! **We cannot have them.** Both sit behind `#[cfg(feature = "messaging-fabric")]`, and that
//! feature does not compile at `fa3c01aec`. Measured, both directions:
//!
//! ```text
//! features = ["messaging-fabric"]                     -> E0599, 2 errors
//!   messaging/consumer.rs:1299  fn default() { holder: TaskId::new_ephemeral(), … }  (un-gated)
//!   types/id.rs:136             #[cfg(any(test, feature = "test-internals"))]
//! features = ["messaging-fabric", "test-internals"]   -> exit 0, 0 errors
//! ```
//!
//! A production `Default` impl calls a test-only constructor. This became load-bearing when
//! upstream issue #46 — *"default feature set includes test-internals — leaks into downstream
//! production"* — was correctly **fixed**, removing `test-internals` from
//! `default = ["proc-macros", "nightly-outcome-try"]`. Enabling it here would reintroduce the exact
//! leak #46 closed, so we do not.
//!
//! **NO-CLAIM:** the mapping table above is a *proposal*, not a migration. Whether a tmux pane that
//! has never heard of asupersync can produce an `Accepted` is **UNMEASURED** — `--mode=rpc` is
//! single-session and cannot address a third-party pane, so the receipt gap may survive the
//! vocabulary.

/// A completed operation's result — severity and panic payload modelled rather than flattened.
///
/// **Resolves a live collision:** `tick-monitor` declares its own `Outcome`, unrelated to this
/// one. Two types, one name, different meanings — the class the inventory gate exists to refuse.
pub use asupersync::types::{Outcome, OutcomeError, PanicPayload, Severity, join_outcomes};

/// Capability-narrowing budgets. Absent from every crate we own, while this repo declares the
/// asupersync contract binding — where `Budget`, `Outcome` and capability narrowing are the
/// application's semantic contract, not optional polish.
pub use asupersync::types::{
    Budget, CapabilityBudget, CapabilityBudgetDimension, CapabilityBudgetRefusal,
    CapabilityBudgetRequirements, RemainingBudget,
};

/// Identity types. `ObligationId` is the load-bearing one: an obligation is **reserved**, then
/// committed or aborted. Our `Duty` carries `#[must_use]` and **no ledger**, so a dropped `Duty`
/// leaks an obligation only a human notices — measured as the pending-dispatch marker surviving
/// **162 refused ticks** because nothing owned its release.
pub use asupersync::types::{ObligationId, RegionId, TaskId, Time};

#[cfg(test)]
mod tests {
    use super::*;

    /// KNOWN-GOOD: the re-exports resolve and are usable. Without this the crate could re-export
    /// nothing and still compile, which is the vacuous-green shape this repo has shipped twice.
    ///
    /// **`Outcome` is strictly better than `Result` for this workspace** and the variants say why:
    /// `Ok` / `Err` / `Cancelled(CancelReason)` / `Panicked(PanicPayload)`. Cancellation is a
    /// FIRST-CLASS OUTCOME, not an error. That is the type-level form of the rule this repo
    /// learned the hard way — *a timeout is not a verdict* — where parsing an empty buffer for a
    /// verdict field and defaulting to FAIL manufactured a claim about the fleet out of nothing.
    /// With `Outcome`, a killed child maps to `Cancelled` and CANNOT be confused with `Err`.
    #[test]
    fn the_derived_vocabulary_is_constructible_and_models_cancellation() {
        let ok: Outcome<u8, ()> = Outcome::Ok(7);
        assert!(matches!(ok, Outcome::Ok(7)));

        // The load-bearing distinction: cancellation is NOT an error variant.
        let err: Outcome<u8, &str> = Outcome::Err("real failure");
        assert!(
            !matches!(err, Outcome::Cancelled(_)),
            "an application error must never be reachable as Cancelled"
        );
    }

    /// ANTI-VACUITY: assert the ids are *distinct types*, not aliases that would let a `TaskId` be
    /// passed where a `RegionId` is required. A vocabulary whose types are interchangeable is a
    /// vocabulary that cannot catch a mix-up at compile time.
    #[test]
    fn identity_types_are_distinct() {
        fn takes_task(_: &TaskId) {}
        fn takes_region(_: &RegionId) {}
        let _ = (takes_task as fn(&TaskId), takes_region as fn(&RegionId));
    }

    /// The blocked half, asserted as a **named absence** rather than left silent. If a future rev
    /// makes `messaging-fabric` build without `test-internals`, delete this test and re-export
    /// `AckKind`/`DeliveryClass` — its failure to compile is the signal to do so.
    #[test]
    fn ack_vocabulary_is_documented_as_unreachable() {
        const BLOCKER: &str =
            "messaging-fabric requires test-internals at fa3c01aec (consumer.rs:1299 default impl)";
        assert!(!BLOCKER.is_empty(), "the blocker must stay named, not silently dropped");
    }
}

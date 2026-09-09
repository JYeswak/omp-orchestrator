#![forbid(unsafe_code)]

//! Targeted, fail-closed reclaim of regenerable Contabo build artifacts.
//!
//! The library owns the typed whitelist and dual live-build authority decision. The binary is only
//! argument parsing and runtime presentation. Every remote subprocess is bounded, group-owned, and
//! drains both output streams through the Asupersync process surface.

pub mod model;
pub mod probe;

pub use model::{
    basename_is_whitelisted, decide_guards, parse_listing, validate_candidate, worker_by_id,
    ActiveBuild, Candidate, CandidateSet, ControlSnapshot, EntryKind, FleetOutcome, FleetReport,
    GuardDecision, ReclaimError, ReclaimMode, ReclaimRefusal, ReclaimReport, RefusalReason,
    RemoteProcessObservation, RunOutcome, ValidatedCandidate, WhitelistRule, WorkerSelection,
    WorkerSpec, WHITELIST, WORKERS,
};
pub use probe::{run, run_all_workers, Config};

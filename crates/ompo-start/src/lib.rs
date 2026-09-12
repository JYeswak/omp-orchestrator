#![forbid(unsafe_code)]

//! `ompo start` owns one `STEPS` array. Both renderers borrow it.
//!
//! `view()` is identity. Persona / not-live / HD predicates set `status`;
//! they must not drop rows. Filtering here is LAW-L3-SKIP-STAYS going red
//! while the array still looks complete.

pub mod steps;
pub mod spawn;
pub mod spawn_mail;
pub mod pack;
pub mod inception;
pub mod foundation;

pub mod liveness;
pub mod mail;
pub mod portal;
pub mod portal_contract;
pub mod hd0009;
pub use foundation::{append_s1_foundation, s1_row, s1_rows_citing_inception, INCEPTION_REF, SOURCE};
pub use pack::{retain_pack_receipt, PackError, PackReceipt, SendAttempt, TICK_ZERO_TARGET};
pub use spawn::{
    generate_wave, panes_from_list_panes, post_spawn_recheck, sha256_hex, spawn_gate,
    spawn_retain_wave_hash, verify_retained_hash, PaneId, Recheck, SpawnGate, SpawnReceipt,
    SpawnWaveError,
};
pub use steps::{
    apply_predicates, check_id_parity, fixture_steps, json_next_command, json_ordered_ids,
    next_command, next_step, ordered_ids, tui_next_command, tui_ordered_ids, view, ParityMismatch,
    Predicate, Step, StepStatus,
};

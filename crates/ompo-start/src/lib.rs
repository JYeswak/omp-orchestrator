#![forbid(unsafe_code)]

//! `ompo start` owns one `STEPS` array. Both renderers borrow it.
//!
//! `view()` is identity. Persona / not-live / HD predicates set `status`;
//! they must not drop rows. Filtering here is LAW-L3-SKIP-STAYS going red
//! while the array still looks complete.

pub mod steps;

pub use steps::{
    apply_predicates, fixture_steps, json_ordered_ids, ordered_ids, tui_ordered_ids, view, Predicate,
    Step, StepStatus,
};

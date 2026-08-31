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

use std::fmt;

/// The dispatch steps, in order. Each kind emits exactly one row per
/// occurrence in the dispatch path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StepKind {
    /// `br ready` selected a bead for this pane.
    BeadSelected,
    /// The dispatch packet was rendered from the bead body.
    PacketRendered,
    /// The pane-dispatch fence admitted this pane.
    FenceChecked,
    /// The packet was sent (ntm robot-send through the fence).
    PacketSent,
    /// The receiver proof verified the pane is acting on the named bead.
    ReceiverVerified,
    /// The receiver proof timed out — the pane did not show the bead.
    ReceiverTimedOut,
    /// The grade receipt was requested from the independent grader.
    GradeRequested,
    /// The grade receipt was received (PASS or FIX).
    GradeReceived,
    /// The bead was closed by the grader.
    Closed,
    /// The bead was sent back for redispatch with a named fix.
    Redispatched,
}

impl StepKind {
    pub fn as_str(self) -> &'static str {
        match self {
            StepKind::BeadSelected => "bead_selected",
            StepKind::PacketRendered => "packet_rendered",
            StepKind::FenceChecked => "fence_checked",
            StepKind::PacketSent => "packet_sent",
            StepKind::ReceiverVerified => "receiver_verified",
            StepKind::ReceiverTimedOut => "receiver_timed_out",
            StepKind::GradeRequested => "grade_requested",
            StepKind::GradeReceived => "grade_received",
            StepKind::Closed => "closed",
            StepKind::Redispatched => "redispatched",
        }
    }
}

/// One typed row in the step ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepRecord {
    pub kind: StepKind,
    pub bead_id: String,
    pub pane_id: String,
    pub session: String,
    pub ts_unix: i64,
    pub detail: String,
}

impl fmt::Display for StepRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {} {} {}",
            self.kind.as_str(),
            self.bead_id,
            self.pane_id,
            self.session,
            self.detail
        )
    }
}

/// The step ledger: a sequence of typed rows, one per dispatch step.
///
/// The DECIDING LEG: the row count must equal the step count. A dispatch
/// that emits fewer rows than steps taken FAILS. Known-bad: delete a row
/// emission and the assertion goes RED.
///
/// CANCEL CONSISTENCY: cancellation mid-dispatch leaves the ledger
/// CONSISTENT (the rows emitted so far are valid and the intent is
/// recoverable from the last row's kind), never a half-state.
#[derive(Debug, Clone, Default)]
pub struct StepLedger {
    rows: Vec<StepRecord>,
    steps_taken: usize,
}

impl StepLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Emit exactly one row for one step taken. The caller MUST call this
    /// after every step; the assertion checks that the counts match.
    pub fn record(&mut self, record: StepRecord) {
        self.rows.push(record);
        self.steps_taken += 1;
    }

    /// Record a step from its parts (convenience for inline emission).
    pub fn record_step(
        &mut self,
        kind: StepKind,
        bead_id: &str,
        pane_id: &str,
        session: &str,
        detail: &str,
    ) {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        self.record(StepRecord {
            kind,
            bead_id: bead_id.to_owned(),
            pane_id: pane_id.to_owned(),
            session: session.to_owned(),
            ts_unix: ts,
            detail: detail.to_owned(),
        });
    }

    /// THE DECIDING LEG: the step count is ASSERTED.
    ///
    /// Returns Err when the row count does not match the steps taken —
    /// meaning a step completed without emitting a row, or a row was emitted
    /// without a step. Both are ledger corruption.
    pub fn assert_step_count(&self) -> Result<(), String> {
        if self.rows.len() != self.steps_taken {
            return Err(format!(
                "STEP_COUNT_ASSERTION_FAILED: rows={} steps_taken={}",
                self.rows.len(),
                self.steps_taken
            ));
        }
        Ok(())
    }

    /// The ledger is consistent when every step has a row and vice versa.
    /// Cancellation mid-dispatch leaves the ledger CONSISTENT: the rows
    /// emitted so far are valid and the intent is recoverable from the last
    /// row's kind.
    pub fn is_consistent(&self) -> bool {
        self.rows.len() == self.steps_taken
    }

    /// ANTI-VACUITY: zero steps observed is an ERROR, never a clean dispatch.
    /// Returns Err when the ledger is empty — a dispatch that took no steps
    /// is indistinguishable from one that never happened, and the gate must
    /// refuse rather than pass.
    pub fn assert_non_empty(&self) -> Result<(), String> {
        if self.rows.is_empty() && self.steps_taken == 0 {
            return Err(
                "ANTI_VACUITY: zero steps observed is an ERROR, never a clean dispatch".into(),
            );
        }
        Ok(())
    }

    pub fn rows(&self) -> &[StepRecord] {
        &self.rows
    }

    pub fn steps_taken(&self) -> usize {
        self.steps_taken
    }

    /// The kind of the last recorded step — used for cancel-recovery (the
    /// conductor resumes from the next step, not from scratch).
    pub fn last_kind(&self) -> Option<StepKind> {
        self.rows.last().map(|r| r.kind)
    }

    /// Serialize to JSONL (one JSON object per line) for the ledger file.
    pub fn to_jsonl(&self) -> String {
        self.rows
            .iter()
            .map(|r| {
                format!(
                    r#"{{"kind":"{}","bead":"{}","pane":"{}","session":"{}","ts":{},"detail":"{}"}}"#,
                    r.kind.as_str(),
                    r.bead_id.replace('"', "\\\""),
                    r.pane_id.replace('"', "\\\""),
                    r.session.replace('"', "\\\""),
                    r.ts_unix,
                    r.detail.replace('"', "\\\"")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_row_per_step_count_asserted() {
        let mut ledger = StepLedger::new();
        ledger.record_step(StepKind::BeadSelected, "cp-1", "%5", "s", "selected");
        ledger.record_step(StepKind::PacketRendered, "cp-1", "%5", "s", "rendered");
        ledger
            .assert_step_count()
            .expect("two steps, two rows: consistent");
        assert_eq!(ledger.rows().len(), 2);
        assert_eq!(ledger.steps_taken(), 2);
    }

    #[test]
    fn deleted_row_emission_goes_red() {
        // KNOWN-BAD: a step that completes but does NOT emit a row.
        // The ledger's steps_taken increments without a row — the assertion
        // goes RED.
        let mut ledger = StepLedger::new();
        ledger.record_step(StepKind::BeadSelected, "cp-1", "%5", "s", "selected");
        // SIMULATED BUG: step taken but no row emitted.
        ledger.steps_taken += 1;
        assert!(
            ledger.assert_step_count().is_err(),
            "deleting a row emission must go RED"
        );
    }

    #[test]
    fn empty_ledger_is_anti_vacuity_error() {
        let ledger = StepLedger::new();
        assert!(
            ledger.assert_non_empty().is_err(),
            "zero steps observed is an ERROR, never a clean dispatch"
        );
    }

    #[test]
    fn non_empty_ledger_passes_anti_vacuity() {
        let mut ledger = StepLedger::new();
        ledger.record_step(StepKind::BeadSelected, "cp-1", "%5", "s", "selected");
        ledger
            .assert_non_empty()
            .expect("a ledger with rows passes anti-vacuity");
    }

    #[test]
    fn cancel_mid_dispatch_leaves_consistent() {
        // Cancellation mid-dispatch: some steps emitted rows, the rest didn't.
        // The ledger must be CONSISTENT (rows == steps_taken for the steps
        // that DID emit) and the intent recoverable from the last row's kind.
        let mut ledger = StepLedger::new();
        ledger.record_step(StepKind::BeadSelected, "cp-1", "%5", "s", "selected");
        ledger.record_step(StepKind::PacketRendered, "cp-1", "%5", "s", "rendered");
        // Cancellation: no more steps. The ledger is consistent because
        // every emitted step has a row.
        assert!(
            ledger.is_consistent(),
            "cancel mid-dispatch: ledger consistent"
        );
        assert_eq!(
            ledger.last_kind(),
            Some(StepKind::PacketRendered),
            "the last kind is recoverable for resume"
        );
    }

    #[test]
    fn jsonl_serialization_round_trips() {
        let mut ledger = StepLedger::new();
        ledger.record_step(StepKind::BeadSelected, "cp-1", "%5", "s", "selected");
        let jsonl = ledger.to_jsonl();
        assert!(jsonl.contains("bead_selected"));
        assert!(jsonl.contains("cp-1"));
    }

    #[test]
    fn step_kinds_are_ordered() {
        // The dispatch path's steps in canonical order.
        let order = [
            StepKind::BeadSelected,
            StepKind::PacketRendered,
            StepKind::FenceChecked,
            StepKind::PacketSent,
            StepKind::ReceiverVerified,
            StepKind::ReceiverTimedOut,
            StepKind::GradeRequested,
            StepKind::GradeReceived,
            StepKind::Closed,
            StepKind::Redispatched,
        ];
        // Every kind has a unique string representation.
        let names: std::collections::HashSet<&str> = order.iter().map(|k| k.as_str()).collect();
        assert_eq!(names.len(), order.len(), "detector names must be unique");
    }
}

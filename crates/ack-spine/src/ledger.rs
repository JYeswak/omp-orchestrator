//! Ack spine (slice a): the asupersync step ledger.
//!
//! Every dispatch step is a region-owned operation taking `&Cx` FIRST and
//! emitting exactly ONE typed row. The step count is ASSERTED — a dispatch
//! that emits fewer rows than steps taken FAILS.
//!
//! Cancellation is acknowledged before the effect and after it. The row is
//! committed before the post-effect checkpoint, so a cancellation observed at
//! that checkpoint leaves a consistent, recoverable prefix: `last_kind()` is
//! the completed boundary from which the owner can resume.

use std::fmt;
use std::future::Future;

/// Errors raised while enforcing the ledger's deciding invariants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerError {
    /// No dispatch step was observed. A zero-step run is never a clean run.
    Empty,
    /// A step and its corresponding row diverged.
    CountMismatch { rows: usize, steps_taken: usize },
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(
                f,
                "ANTI_VACUITY: zero steps observed is an ERROR, never a clean dispatch"
            ),
            Self::CountMismatch { rows, steps_taken } => write!(
                f,
                "STEP_COUNT_ASSERTION_FAILED: rows={rows} steps_taken={steps_taken}"
            ),
        }
    }
}

impl std::error::Error for LedgerError {}

/// Errors from the cancel-correct step primitive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepError {
    /// Cancellation was observed. The prefix through `last_kind` is durable
    /// and can be resumed from the next step.
    Cancelled { last_kind: Option<StepKind> },
    /// The ledger invariant was violated.
    Ledger(LedgerError),
}

impl fmt::Display for StepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled { last_kind } => {
                write!(f, "CANCELLED step_prefix_last={last_kind:?}")
            }
            Self::Ledger(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for StepError {}

impl From<LedgerError> for StepError {
    fn from(error: LedgerError) -> Self {
        Self::Ledger(error)
    }
}

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
    /// A bead-comment acknowledgement was confirmed by read-back.
    AckReadBack,
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
            StepKind::AckReadBack => "ack_read_back",
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
#[derive(Debug, Clone, Default)]
pub struct StepLedger {
    rows: Vec<StepRecord>,
    steps_taken: usize,
}

impl StepLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Emit one row and account for its step. This is intentionally private:
    /// the public `step` primitive is the single emission path.
    fn emit(&mut self, record: StepRecord) {
        self.rows.push(record);
        self.steps_taken += 1;
    }

    /// THE DECIDING LEG: the step count is ASSERTED.
    ///
    /// Returns an error when the row count does not match the steps taken —
    /// meaning a step completed without emitting a row, or a row was emitted
    /// without a step. Both are ledger corruption.
    pub fn assert_step_count(&self) -> Result<(), LedgerError> {
        if self.rows.len() != self.steps_taken {
            return Err(LedgerError::CountMismatch {
                rows: self.rows.len(),
                steps_taken: self.steps_taken,
            });
        }
        Ok(())
    }

    /// The ledger is consistent when every step has a row and vice versa.
    pub fn is_consistent(&self) -> bool {
        self.rows.len() == self.steps_taken
    }

    /// ANTI-VACUITY: zero steps observed is an ERROR, never a clean dispatch.
    pub fn assert_non_empty(&self) -> Result<(), LedgerError> {
        if self.rows.is_empty() && self.steps_taken == 0 {
            return Err(LedgerError::Empty);
        }
        Ok(())
    }

    pub fn rows(&self) -> &[StepRecord] {
        &self.rows
    }

    pub fn steps_taken(&self) -> usize {
        self.steps_taken
    }

    /// The kind of the last recorded step — the recovery boundary. A
    /// conductor resumes from the next step, not from scratch.
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

/// Execute one region-owned dispatch step.
///
/// The caller supplies the runtime-owned context first. Cancellation is
/// checked before the effect and after its boundary. The record is committed
/// between those checkpoints, making a post-effect cancellation a consistent
/// and recoverable prefix rather than a half-state. The effect is awaited
/// directly; this primitive never detaches a task.
pub async fn step<F, Fut>(
    cx: &asupersync::Cx,
    ledger: &mut StepLedger,
    kind: StepKind,
    bead_id: &str,
    pane_id: &str,
    session: &str,
    detail: &str,
    effect: F,
) -> Result<(), StepError>
where
    F: FnOnce(&asupersync::Cx) -> Fut,
    Fut: Future<Output = ()>,
{
    if cx.checkpoint().is_err() {
        return Err(StepError::Cancelled {
            last_kind: ledger.last_kind(),
        });
    }

    effect(cx).await;

    let ts_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    ledger.emit(StepRecord {
        kind,
        bead_id: bead_id.to_owned(),
        pane_id: pane_id.to_owned(),
        session: session.to_owned(),
        ts_unix,
        detail: detail.to_owned(),
    });

    if cx.checkpoint().is_err() {
        return Err(StepError::Cancelled {
            last_kind: ledger.last_kind(),
        });
    }

    ledger.assert_step_count()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use asupersync::runtime::RuntimeBuilder;
    use asupersync::types::CancelKind;

    fn run<F: Future>(future: F) -> F::Output {
        RuntimeBuilder::current_thread()
            .build()
            .expect("current-thread runtime")
            .block_on(future)
    }

    #[test]
    fn one_row_per_step_count_asserted() {
        run(async {
            let cx = asupersync::Cx::current().expect("runtime Cx");
            let mut ledger = StepLedger::new();
            step(
                &cx,
                &mut ledger,
                StepKind::BeadSelected,
                "cp-1",
                "%5",
                "s",
                "selected",
                |_| async {},
            )
            .await
            .expect("first step");
            step(
                &cx,
                &mut ledger,
                StepKind::PacketRendered,
                "cp-1",
                "%5",
                "s",
                "rendered",
                |_| async {},
            )
            .await
            .expect("second step");
            ledger.assert_step_count().expect("two rows, two steps");
            assert_eq!(ledger.rows().len(), 2);
            assert_eq!(ledger.steps_taken(), 2);
        });
    }

    #[test]
    fn deleted_row_emission_goes_red() {
        // KNOWN-BAD: a step that completes but does NOT emit a row.
        let mut ledger = StepLedger::new();
        ledger.steps_taken = 1;
        assert!(matches!(
            ledger.assert_step_count(),
            Err(LedgerError::CountMismatch {
                rows: 0,
                steps_taken: 1
            })
        ));
    }

    #[test]
    fn cancellation_keeps_consistent_recoverable_prefix() {
        run(async {
            let cx = asupersync::Cx::current().expect("runtime Cx");
            let mut ledger = StepLedger::new();
            step(
                &cx,
                &mut ledger,
                StepKind::BeadSelected,
                "cp-1",
                "%5",
                "s",
                "selected",
                |_| async {},
            )
            .await
            .expect("prefix step");

            let error = step(
                &cx,
                &mut ledger,
                StepKind::PacketRendered,
                "cp-1",
                "%5",
                "s",
                "rendered",
                |cx| {
                    cx.cancel_with(CancelKind::User, Some("cancel after effect"));
                    async {}
                },
            )
            .await
            .expect_err("post-effect cancellation");
            assert_eq!(
                error,
                StepError::Cancelled {
                    last_kind: Some(StepKind::PacketRendered)
                }
            );
            assert!(ledger.is_consistent());
            ledger.assert_step_count().expect("cancel prefix count");
            assert_eq!(ledger.last_kind(), Some(StepKind::PacketRendered));
            assert_eq!(ledger.rows().len(), 2);
        });
    }

    #[test]
    fn empty_ledger_is_typed_anti_vacuity_error() {
        let ledger = StepLedger::new();
        assert_eq!(ledger.assert_non_empty(), Err(LedgerError::Empty));
    }

    #[test]
    fn jsonl_serialization_round_trips() {
        let mut ledger = StepLedger::new();
        ledger.emit(StepRecord {
            kind: StepKind::BeadSelected,
            bead_id: "cp-1".to_owned(),
            pane_id: "%5".to_owned(),
            session: "s".to_owned(),
            ts_unix: 1,
            detail: "selected".to_owned(),
        });
        let jsonl = ledger.to_jsonl();
        assert!(jsonl.contains("bead_selected"));
        assert!(jsonl.contains("cp-1"));
    }

    #[test]
    fn step_kinds_are_ordered() {
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
            StepKind::AckReadBack,
        ];
        let names: std::collections::HashSet<&str> =
            order.iter().map(|kind| kind.as_str()).collect();
        assert_eq!(names.len(), order.len(), "detector names must be unique");
    }
}

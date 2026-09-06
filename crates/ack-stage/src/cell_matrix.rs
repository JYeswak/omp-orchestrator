#![forbid(unsafe_code)]

//! One typed matrix row per dispatch.
//!
//! WWJD `README.md:167-177` reports five independently verified launch cells
//! and forbids counting a `send-text` as started. This crate already had the
//! evidence (`AckStageResult`, transport / observation / composer) and
//! collapsed it into one status word — which is why `DISPATCH_RESULT_RECORDED=27`
//! / `ReceiptConfirmed=0` read as ambiguous instead of cells 1–3 OBSERVED /
//! 4–5 ABSENT.
//!
//! # Why seven, not five
//!
//! WWJD's five are `paneCreated`, `promptSubmitted`, `activityObserved`,
//! `attemptMarker`, `independentVerify`. We add `beadClaimed` (AGENTS.md
//! file→claim→dispatch; an unclaimed packet is invisible to follow-up) and
//! `gradeObserved` (a different agent must close). Transport / observation /
//! composer stay **named subcells of `independentVerify`**, not extra top-level
//! cells: splitting them would recreate the status scatter this row replaces.
//!
//! Quote ≠ policy: WWJD's five-cell table is a quote. The mechanical change
//! here is a typed row whose every cell is OBSERVED, ABSENT, or INDETERMINATE.
//! A cell that was not checked is ABSENT, never blank.
//!
//! NO-CLAIM: a matrix row does not fix the confirmation defect. It makes the
//! gap legible.

use crate::AckStageResult;
use receiver_receipt::{PostSendObservation, ReceiptVerdict};
use std::fmt;
use tick_monitor::PaneState;

/// Ordered cells. Indices 0..4 are WWJD's five; 5 and 6 are ours.
pub const CELL_NAMES: [&str; 7] = [
    "paneCreated",
    "promptSubmitted",
    "activityObserved",
    "attemptMarker",
    "independentVerify",
    "beadClaimed",
    "gradeObserved",
];

pub const CELL_COUNT: usize = CELL_NAMES.len();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellState {
    Observed,
    Absent,
    Indeterminate,
}

impl CellState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Observed => "OBSERVED",
            Self::Absent => "ABSENT",
            Self::Indeterminate => "INDETERMINATE",
        }
    }

    /// Unchecked is ABSENT. Never blank.
    pub const fn from_check(checked: bool, observed: bool, indeterminate: bool) -> Self {
        if !checked {
            Self::Absent
        } else if indeterminate {
            Self::Indeterminate
        } else if observed {
            Self::Observed
        } else {
            Self::Absent
        }
    }
}

impl fmt::Display for CellState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatrixError {
    MatrixScanEmpty,
    IncompleteCellSet { missing: String },
}

impl fmt::Display for MatrixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MatrixScanEmpty => f.write_str(
                "MATRIX_SCAN_EMPTY: zero dispatches is a typed state, not an all-ABSENT matrix",
            ),
            Self::IncompleteCellSet { missing } => {
                write!(f, "MATRIX_INCOMPLETE missing={missing}")
            }
        }
    }
}

/// Facts the supervisor already has at emit time. Not recovered by grepping
/// heartbeat status words after the fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchFacts {
    pub dispatch_id: String,
    pub pane_id: String,
    pub bead_id: String,
    pub pane_created: CellState,
    pub prompt_submitted: CellState,
    pub activity_observed: CellState,
    pub attempt_marker: CellState,
    pub independent_verify: CellState,
    pub bead_claimed: CellState,
    pub grade_observed: CellState,
    pub transport: CellState,
    pub observation: CellState,
    pub composer: CellState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchCellMatrix {
    pub dispatch_id: String,
    pub pane_id: String,
    pub bead_id: String,
    cells: [CellState; CELL_COUNT],
    pub transport: CellState,
    pub observation: CellState,
    pub composer: CellState,
}

/// Identity projection. Mutating Observed → Absent turns the tonight known-bad
/// leg RED on `activityObserved`.
fn project_activity(state: CellState) -> CellState {
    match state {
        CellState::Observed => CellState::Observed,
        CellState::Absent => CellState::Absent,
        CellState::Indeterminate => CellState::Indeterminate,
    }
}


impl DispatchCellMatrix {
    pub const ROW_STATUS: &'static str = "DISPATCH_CELL_MATRIX";

    pub fn from_facts(facts: DispatchFacts) -> Result<Self, MatrixError> {
        if facts.dispatch_id.trim().is_empty() {
            return Err(MatrixError::IncompleteCellSet {
                missing: "dispatch_id".into(),
            });
        }
        let cells = [
            facts.pane_created,
            facts.prompt_submitted,
            project_activity(facts.activity_observed),

            facts.attempt_marker,
            facts.independent_verify,
            facts.bead_claimed,
            facts.grade_observed,
        ];
        if cells.len() != CELL_COUNT {
            return Err(MatrixError::IncompleteCellSet {
                missing: "cell_count".into(),
            });
        }
        Ok(Self {
            dispatch_id: facts.dispatch_id,
            pane_id: facts.pane_id,
            bead_id: facts.bead_id,
            cells,
            transport: facts.transport,
            observation: facts.observation,
            composer: facts.composer,
        })
    }

    pub fn cell(&self, name: &str) -> Option<CellState> {
        CELL_NAMES
            .iter()
            .position(|n| *n == name)
            .map(|i| self.cells[i])
    }

    /// Counting `promptSubmitted` as started is forbidden (WWJD README.md:169).
    pub fn is_started(&self) -> bool {
        self.cells[2] == CellState::Observed && self.cells[3] == CellState::Observed
    }

    pub fn is_confirmed(&self) -> bool {
        self.cells[4] == CellState::Observed
    }

    pub fn detail(&self) -> String {
        let mut out = format!(
            "dispatch_id={} pane={} bead={}",
            self.dispatch_id, self.pane_id, self.bead_id
        );
        for (name, state) in CELL_NAMES.iter().zip(self.cells.iter()) {
            out.push_str(&format!(" {name}={}", state.as_str()));
        }
        out.push_str(&format!(
            " transport={} observation={} composer={}",
            self.transport.as_str(),
            self.observation.as_str(),
            self.composer.as_str()
        ));
        out
    }
}

pub fn scan_matrices(rows: &[DispatchCellMatrix]) -> Result<(), MatrixError> {
    if rows.is_empty() {
        return Err(MatrixError::MatrixScanEmpty);
    }
    Ok(())
}

/// Project an assessed stage into cells without grepping heartbeat rows.
pub fn facts_from_stage(
    dispatch_id: String,
    bead_id: String,
    pane_id: String,
    pane_present: bool,
    prompt_submitted: bool,
    bead_claimed: bool,
    grade_observed: bool,
    _pending_marker: bool,
    result: Option<&AckStageResult>,
    post_send: Option<&PostSendObservation>,
) -> DispatchFacts {
    let activity = activity_observed(post_send, result);
    let (verify, transport, observation, composer, attempt) = match result {
        Some(stage) => {
            let verify = if stage.is_confirmed() {
                CellState::Observed
            } else if !stage.transport.supports_delivery_claim() {
                CellState::Indeterminate
            } else {
                CellState::Absent
            };
            let transport = CellState::Observed;
            let observation = match &stage.delivery {
                ReceiptVerdict::ReceiptConfirmed { .. } => CellState::Observed,
                ReceiptVerdict::Indeterminate { .. } => CellState::Indeterminate,
                ReceiptVerdict::AckConfirmed { .. }
                | ReceiptVerdict::NoReceipt { .. }
                | ReceiptVerdict::Dead { .. } => CellState::Absent,
            };
            let composer = match &stage.delivery {
                ReceiptVerdict::ReceiptConfirmed {
                    stable_content_changed: true,
                    ..
                } => CellState::Observed,
                ReceiptVerdict::ReceiptConfirmed {
                    stable_content_changed: false,
                    ..
                }
                | ReceiptVerdict::AckConfirmed { .. }
                | ReceiptVerdict::NoReceipt { .. }
                | ReceiptVerdict::Dead { .. }
                | ReceiptVerdict::Indeterminate { .. } => CellState::Absent,
            };
            // attemptMarker is receiver-side evidence the packet landed, never send-text
            // and never a local pending-dispatch file.
            let attempt = match &stage.delivery {
                ReceiptVerdict::ReceiptConfirmed { .. } => CellState::Observed,
                ReceiptVerdict::AckConfirmed { .. } => CellState::Observed,
                ReceiptVerdict::Indeterminate { .. } => CellState::Indeterminate,
                ReceiptVerdict::NoReceipt { .. } | ReceiptVerdict::Dead { .. } => CellState::Absent,
            };


            (verify, transport, observation, composer, attempt)
        }
        None => (
            CellState::Absent,
            CellState::Absent,
            CellState::Absent,
            CellState::Absent,
            CellState::Absent,
        ),
    };
    DispatchFacts {
        dispatch_id,
        pane_id,
        bead_id,
        pane_created: CellState::from_check(true, pane_present, false),
        prompt_submitted: CellState::from_check(true, prompt_submitted, false),
        activity_observed: activity,
        attempt_marker: attempt,
        independent_verify: verify,
        bead_claimed: CellState::from_check(true, bead_claimed, false),
        grade_observed: CellState::from_check(true, grade_observed, false),
        transport,
        observation,
        composer,
    }
}

/// Isolated so a mutation of this function turns the known-bad/known-good legs RED.
pub fn activity_observed(
    post_send: Option<&PostSendObservation>,
    result: Option<&AckStageResult>,
) -> CellState {
    if let Some(stage) = result {
        match &stage.delivery {
            ReceiptVerdict::ReceiptConfirmed { .. } => return CellState::Observed,
            ReceiptVerdict::Indeterminate { .. } => return CellState::Indeterminate,
            ReceiptVerdict::Dead { .. } | ReceiptVerdict::NoReceipt { .. } => {
                return CellState::Absent
            }
            ReceiptVerdict::AckConfirmed { .. } => {}
        }
    }
    match post_send {
        None => CellState::Absent,
        Some(PostSendObservation::Missing) | Some(PostSendObservation::EmptyPaneList) => {
            CellState::Absent
        }
        Some(PostSendObservation::Absent) => CellState::Absent,
        Some(PostSendObservation::Present(obs)) => match obs.state {
            PaneState::Working { .. } => CellState::Observed,
            PaneState::Idle
            | PaneState::Wedged
            | PaneState::Dialog { .. }
            | PaneState::ProviderError402
            | PaneState::Unproven => CellState::Absent,
        },

    }
}

/// Fixture: tonight's 0-of-27 census as cells 1–3 OBSERVED / 4–5 ABSENT.
pub fn tonight_zero_of_27_facts() -> DispatchFacts {
    DispatchFacts {
        dispatch_id: "fixture-tonight-0-of-27".into(),
        pane_id: "%9".into(),
        bead_id: "omp-orchestrator-eg0m".into(),
        pane_created: CellState::Observed,
        prompt_submitted: CellState::Observed,
        activity_observed: CellState::Observed,
        attempt_marker: CellState::Absent,
        independent_verify: CellState::Absent,
        bead_claimed: CellState::Observed,
        grade_observed: CellState::Absent,
        transport: CellState::Absent,
        observation: CellState::Observed,
        composer: CellState::Absent,
    }
}

/// Fixture: send returned success:[4] and the packet never arrived (`cp-z42vu`).
pub fn send_success_no_arrival_facts() -> DispatchFacts {
    DispatchFacts {
        dispatch_id: "fixture-cp-z42vu".into(),
        pane_id: "%4".into(),
        bead_id: "omp-orchestrator-cp-z42vu".into(),
        pane_created: CellState::Observed,
        prompt_submitted: CellState::Observed,
        activity_observed: CellState::Absent,
        attempt_marker: CellState::Absent,
        independent_verify: CellState::Absent,
        bead_claimed: CellState::Observed,
        grade_observed: CellState::Absent,
        transport: CellState::Observed,
        observation: CellState::Absent,
        composer: CellState::Absent,
    }
}

/// Inverse of `cp-z42vu`: pending-dispatch marker, no send.
pub fn pending_marker_no_send_facts() -> DispatchFacts {
    DispatchFacts {
        dispatch_id: "fixture-pending-no-send".into(),
        pane_id: "%4".into(),
        bead_id: "omp-orchestrator-cp-z42vu".into(),
        pane_created: CellState::Observed,
        prompt_submitted: CellState::Absent,
        activity_observed: CellState::Absent,
        attempt_marker: CellState::Absent,
        independent_verify: CellState::Absent,
        bead_claimed: CellState::Observed,
        grade_observed: CellState::Absent,
        transport: CellState::Absent,
        observation: CellState::Absent,
        composer: CellState::Absent,
    }
}

/// Fixture: fully confirmed dispatch. Live all-OBSERVED has never been produced.
pub fn known_good_all_observed_facts() -> DispatchFacts {
    DispatchFacts {
        dispatch_id: "fixture-known-good-all-observed".into(),
        pane_id: "%9".into(),
        bead_id: "omp-orchestrator-hx3n".into(),
        pane_created: CellState::Observed,
        prompt_submitted: CellState::Observed,
        activity_observed: CellState::Observed,
        attempt_marker: CellState::Observed,
        independent_verify: CellState::Observed,
        bead_claimed: CellState::Observed,
        grade_observed: CellState::Observed,
        transport: CellState::Observed,
        observation: CellState::Observed,
        composer: CellState::Observed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn states(m: &DispatchCellMatrix) -> Vec<(&'static str, CellState)> {
        CELL_NAMES
            .iter()
            .map(|n| (*n, m.cell(n).unwrap()))
            .collect()
    }

    #[test]
    fn tonight_census_is_cells_1_3_observed_4_5_absent() {
        let m = DispatchCellMatrix::from_facts(tonight_zero_of_27_facts()).unwrap();
        assert_eq!(m.cell("paneCreated"), Some(CellState::Observed));
        assert_eq!(m.cell("promptSubmitted"), Some(CellState::Observed));
        assert_eq!(m.cell("activityObserved"), Some(CellState::Observed));
        assert_eq!(m.cell("attemptMarker"), Some(CellState::Absent));
        assert_eq!(m.cell("independentVerify"), Some(CellState::Absent));
        assert_eq!(m.cell("beadClaimed"), Some(CellState::Observed));
        assert_eq!(m.cell("gradeObserved"), Some(CellState::Absent));
        assert!(!m.is_confirmed(), "0 ReceiptConfirmed must not confirm");
        assert!(
            m.detail().contains("attemptMarker=ABSENT")
                && m.detail().contains("independentVerify=ABSENT"),
            "ABSENT cells must be named: {}",
            m.detail()
        );
        assert_eq!(m.detail().matches("INDETERMINATE").count(), 0);
    }

    #[test]
    fn known_good_is_all_observed_and_not_downgraded() {
        let m = DispatchCellMatrix::from_facts(known_good_all_observed_facts()).unwrap();
        for name in CELL_NAMES {
            assert_eq!(
                m.cell(name),
                Some(CellState::Observed),
                "{name} downgraded: {:?}",
                states(&m)
            );
        }
        assert!(m.is_started());
        assert!(m.is_confirmed());
    }

    #[test]
    fn send_success_is_not_started() {
        let arrived = DispatchCellMatrix::from_facts(send_success_no_arrival_facts()).unwrap();
        assert_eq!(arrived.cell("promptSubmitted"), Some(CellState::Observed));
        assert_eq!(arrived.cell("activityObserved"), Some(CellState::Absent));
        assert_eq!(arrived.cell("attemptMarker"), Some(CellState::Absent));
        assert!(!arrived.is_started());
        assert!(!arrived.is_confirmed());

        let pending = DispatchCellMatrix::from_facts(pending_marker_no_send_facts()).unwrap();
        assert_eq!(pending.cell("promptSubmitted"), Some(CellState::Absent));
        assert_ne!(
            arrived.cell("promptSubmitted"),
            pending.cell("promptSubmitted"),
            "send-success and pending-no-send must be distinguishable"
        );
    }

    #[test]
    fn empty_scan_is_typed_error() {
        let err = scan_matrices(&[]).unwrap_err();
        assert!(
            matches!(err, MatrixError::MatrixScanEmpty),
            "{err}"
        );
        let m = DispatchCellMatrix::from_facts(tonight_zero_of_27_facts()).unwrap();
        scan_matrices(&[m]).unwrap();
    }

    #[test]
    fn empty_dispatch_id_is_incomplete() {
        let mut facts = tonight_zero_of_27_facts();
        facts.dispatch_id.clear();
        let err = DispatchCellMatrix::from_facts(facts).unwrap_err();
        assert!(matches!(err, MatrixError::IncompleteCellSet { .. }));
    }

    #[test]
    fn cell_names_are_data_not_prose() {
        let m = DispatchCellMatrix::from_facts(tonight_zero_of_27_facts()).unwrap();
        assert_eq!(DispatchCellMatrix::ROW_STATUS, "DISPATCH_CELL_MATRIX");
        for name in CELL_NAMES {
            assert!(
                m.detail().contains(name),
                "cell {name} missing from {}",
                m.detail()
            );
        }
        assert_eq!(CELL_COUNT, 7);
    }
}

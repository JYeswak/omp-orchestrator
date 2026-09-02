//! Graded evidence for one terminal pane.
//!
//! Raw terminal parsing belongs to `pane-truth`; this module receives the already-selected last
//! status-line state and composes single or two-capture evidence without collapsing independent
//! facts into booleans.

use std::fmt;

/// Minimum separation required before capture motion can strengthen liveness evidence.
pub const MIN_TWO_CAPTURE_INTERVAL_SECS: u64 = 75;

/// Why a pane could not be classified from its last status line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownReason {
    Unreadable,
    MissingLastStatusLine,
    Unclassified,
}

/// Mutually exclusive liveness state selected from the last status line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneLiveness {
    Unknown(UnknownReason),
    Idle,
    Working { elapsed_secs: u64 },
}

/// Readiness is independent from pane liveness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchAdmissibility {
    Unknown,
    Allowed,
    Refused,
}

/// One capture after terminal parsing has selected the last status line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureSnapshot {
    captured_at_secs: u64,
    liveness: PaneLiveness,
    timer_token: Option<String>,
    spinner_stripped_content_hash: String,
}

impl CaptureSnapshot {
    pub fn new(
        captured_at_secs: u64,
        liveness: PaneLiveness,
        timer_token: Option<String>,
        spinner_stripped_content_hash: String,
    ) -> Self {
        Self {
            captured_at_secs,
            liveness,
            timer_token,
            spinner_stripped_content_hash,
        }
    }

    pub fn captured_at_secs(&self) -> u64 {
        self.captured_at_secs
    }
}

/// Strength of the evidence carrier, independent from the liveness value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceGrade {
    /// One selected last-status-line capture.
    SingleCapture,
    /// Two captures with the minimum interval and meaningful motion.
    TwoCapture {
        interval_secs: u64,
        timer_changed: bool,
        content_hash_changed: bool,
    },
}

impl EvidenceGrade {
    /// True only for a valid two-capture grade against a single-capture grade.
    pub fn dominates(&self, weaker: &Self) -> bool {
        matches!(
            (self, weaker),
            (
                Self::TwoCapture {
                    interval_secs,
                    timer_changed,
                    content_hash_changed,
                },
                Self::SingleCapture
            ) if *interval_secs >= MIN_TWO_CAPTURE_INTERVAL_SECS
                && (*timer_changed || *content_hash_changed)
        )
    }
}

/// Constructor refusal for an attempted two-capture grade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationError {
    IntervalTooShort { actual_secs: u64 },
    NoMeaningfulChange,
}

impl fmt::Display for ObservationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IntervalTooShort { actual_secs } => write!(
                formatter,
                "two captures are {} seconds apart; {} seconds are required",
                actual_secs, MIN_TWO_CAPTURE_INTERVAL_SECS
            ),
            Self::NoMeaningfulChange => {
                formatter.write_str("timer and spinner-stripped content hash are unchanged")
            }
        }
    }
}

impl std::error::Error for ObservationError {}

/// One pane's liveness evidence plus an independent dispatch-readiness fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneObservation {
    pane_id: String,
    liveness: PaneLiveness,
    evidence: EvidenceGrade,
    dispatch_admissibility: DispatchAdmissibility,
}

impl PaneObservation {
    /// Construct an inhabited unknown value. Unknown is not idle.
    pub fn unknown(pane_id: impl Into<String>, reason: UnknownReason) -> Self {
        Self {
            pane_id: pane_id.into(),
            liveness: PaneLiveness::Unknown(reason),
            evidence: EvidenceGrade::SingleCapture,
            dispatch_admissibility: DispatchAdmissibility::Unknown,
        }
    }

    /// Construct weak evidence from the already-selected LAST status line.
    pub fn from_last_status_line(
        pane_id: impl Into<String>,
        last_line_state: PaneLiveness,
        dispatch_admissibility: DispatchAdmissibility,
    ) -> Self {
        Self {
            pane_id: pane_id.into(),
            liveness: last_line_state,
            evidence: EvidenceGrade::SingleCapture,
            dispatch_admissibility,
        }
    }

    /// Construct stronger evidence only after the interval and motion checks pass.
    pub fn from_two_captures(
        pane_id: impl Into<String>,
        previous: CaptureSnapshot,
        current: CaptureSnapshot,
        dispatch_admissibility: DispatchAdmissibility,
    ) -> Result<Self, ObservationError> {
        let interval_secs = current
            .captured_at_secs
            .saturating_sub(previous.captured_at_secs);
        if interval_secs < MIN_TWO_CAPTURE_INTERVAL_SECS {
            return Err(ObservationError::IntervalTooShort {
                actual_secs: interval_secs,
            });
        }
        let timer_changed = previous.timer_token != current.timer_token;
        let content_hash_changed =
            previous.spinner_stripped_content_hash != current.spinner_stripped_content_hash;
        if !timer_changed && !content_hash_changed {
            return Err(ObservationError::NoMeaningfulChange);
        }
        Ok(Self {
            pane_id: pane_id.into(),
            liveness: current.liveness,
            evidence: EvidenceGrade::TwoCapture {
                interval_secs,
                timer_changed,
                content_hash_changed,
            },
            dispatch_admissibility,
        })
    }

    pub fn pane_id(&self) -> &str {
        &self.pane_id
    }

    pub fn liveness(&self) -> &PaneLiveness {
        &self.liveness
    }

    pub fn evidence(&self) -> &EvidenceGrade {
        &self.evidence
    }

    pub fn dispatch_admissibility(&self) -> &DispatchAdmissibility {
        &self.dispatch_admissibility
    }
}

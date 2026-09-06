//! RULE 0.1 metric authorization — bead `omp-orchestrator-adopt-efficiency-metric-vector-92zh`.
//!
//! A proposed crate does not count as an efficiency improvement unless it names an
//! **adopted** measure, that measure has a predeclared denominator and countermetric,
//! and the measurement ledger is non-empty. A dashboard nobody branches on is process
//! porn; this module is the branch.

use std::fmt;

/// Flip to `false` only in the mutation leg.
pub const REQUIRE_METRIC_CLAIM: bool = true;

/// WWJD section-1 names we ADOPT here. Twelve is theirs; these two are ours.
pub const ADOPTED_HUMAN_INTERRUPTIONS: &str = "human_interruptions";
pub const ADOPTED_READY_FRONTIER: &str = "ready_frontier_utilization";

/// One adopted (or explicitly rejected) WWJD measure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricDef {
    pub id: String,
    pub denominator: String,
    pub countermetric: String,
    pub adopted: bool,
    pub why: String,
}

/// One recorded observation. Every figure names its producing artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measurement {
    pub metric: String,
    pub numerator: u64,
    pub denominator_value: u64,
    pub source_artifact: String,
    pub quoted: String,
}

/// A crate proposed for admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateProposal {
    pub crate_name: String,
    /// Which adopted measure this crate claims to improve. `None` is the known-bad.
    pub improves: Option<String>,
}

/// The frozen vector plus its ledger.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetricVector {
    pub metrics: Vec<MetricDef>,
    pub measurements: Vec<Measurement>,
}

/// Typed empty-ledger / unnamed-claim failures. Never a quiet pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    ZeroMeasurements,
    MissingDenominator { metric: String },
    MissingCountermetric { metric: String },
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::ZeroMeasurements => write!(
                f,
                "ZERO_MEASUREMENTS: a metric surface with no data reports identically to a healthy one"
            ),
            AuthError::MissingDenominator { metric } => {
                write!(f, "METRIC_NO_DENOMINATOR metric={metric}")
            }
            AuthError::MissingCountermetric { metric } => {
                write!(f, "METRIC_NO_COUNTERMETRIC metric={metric}")
            }
        }
    }
}

impl std::error::Error for AuthError {}

/// Admission verdict for one proposed crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthVerdict {
    Admitted {
        crate_name: String,
        metric: String,
    },
    Refused {
        crate_name: String,
        reason: String,
    },
}

impl AuthVerdict {
    pub fn admits(&self) -> bool {
        matches!(self, AuthVerdict::Admitted { .. })
    }
}

impl MetricVector {
    pub fn adopted(&self, id: &str) -> Option<&MetricDef> {
        self.metrics
            .iter()
            .find(|m| m.adopted && m.id == id)
    }
}

/// Anti-vacuity: zero recorded measurements is an error, never a pass.
pub fn require_measurements(vector: &MetricVector) -> Result<(), AuthError> {
    if vector.measurements.is_empty() {
        Err(AuthError::ZeroMeasurements)
    } else {
        Ok(())
    }
}

/// Authorize a **new** crate against the adopted vector.
///
/// Known-bad: `improves = None` (or REQUIRE_METRIC_CLAIM flipped off in the mutation
pub fn authorize_new_crate(
    proposal: &CrateProposal,
    vector: &MetricVector,
) -> Result<AuthVerdict, AuthError> {
    authorize_new_crate_with(REQUIRE_METRIC_CLAIM, proposal, vector)
}

pub fn authorize_new_crate_with(
    require_claim: bool,
    proposal: &CrateProposal,
    vector: &MetricVector,
) -> Result<AuthVerdict, AuthError> {
    require_measurements(vector)?;
    let Some(metric_id) = proposal.improves.as_deref() else {
        if !require_claim {
            return Ok(AuthVerdict::Admitted {
                crate_name: proposal.crate_name.clone(),
                metric: "UNCLAIMED".to_string(),
            });
        }
        return Ok(AuthVerdict::Refused {
            crate_name: proposal.crate_name.clone(),
            reason: "METRIC_UNNAMED: a proposed crate naming no measure is refused".to_string(),
        });
    };
    let Some(def) = vector.adopted(metric_id) else {
        return Ok(AuthVerdict::Refused {
            crate_name: proposal.crate_name.clone(),
            reason: format!("METRIC_NOT_ADOPTED metric={metric_id}"),
        });
    };
    if def.denominator.is_empty() {
        return Err(AuthError::MissingDenominator {
            metric: def.id.clone(),
        });
    }
    if def.countermetric.is_empty() {
        return Err(AuthError::MissingCountermetric {
            metric: def.id.clone(),
        });
    }
    Ok(AuthVerdict::Admitted {
        crate_name: proposal.crate_name.clone(),
        metric: def.id.clone(),
    })
}

/// The adopted subset, constructed in-code so tests do not depend on a parser for the
/// known-bad/known-good legs. The on-disk vector is `docs/plan/METRIC-VECTOR.toml`.
pub fn adopted_subset() -> MetricVector {
    MetricVector {
        metrics: vec![
            MetricDef {
                id: ADOPTED_HUMAN_INTERRUPTIONS.to_string(),
                denominator: "hd_ask".to_string(),
                countermetric: "illegitimate_escalations".to_string(),
                adopted: true,
                why: "Joshua asked for owner-required escalations counted with a denominator. \
                      Pairs with omp-orchestrator-two-escalation-authorities-0lc1 (closed): \
                      legitimate = money/irreversible/API-keys; everything else that reads \
                      AWAIT_HUMAN is an agent stopping early."
                    .to_string(),
            },
            MetricDef {
                id: ADOPTED_READY_FRONTIER.to_string(),
                denominator: "available_agent_slots".to_string(),
                countermetric: "idle_beside_ready_queue".to_string(),
                adopted: true,
                why: "AGENTS.md: an idle worker beside a ready queue is the conductor's \
                      failure. Productive slots / available slots; the countermetric is idle \
                      slots while br ready is nonempty."
                    .to_string(),
            },
        ],
        measurements: Vec::new(),
    }
}

/// Count `[[measurement]]` tables in the on-disk vector. Zero is [`AuthError::ZeroMeasurements`].
pub fn measurement_block_count(text: &str) -> usize {
    text.lines()
        .filter(|line| line.trim() == "[[measurement]]")
        .count()
}

pub fn require_vector_text(text: &str) -> Result<(), AuthError> {
    if measurement_block_count(text) == 0 {
        Err(AuthError::ZeroMeasurements)
    } else {
        Ok(())
    }
}


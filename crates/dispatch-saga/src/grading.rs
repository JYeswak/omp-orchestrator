//! IMPL → GRADING decision. Decide-only: this module never spawns `br`.
//!
//! Attribution authority is `grader_attribution_gate::assess_close` (tracker field,
//! not English in `close_reason`). A pane must not grade its own bead.

use grader_attribution_gate::{
    assess_close, AttributionRefusal, AttributionVerdict, CloseAttempt, DEFAULT_AUTHORS,
};

pub use blocker_taxonomy::ReportError;


/// The twelve pane-1 hand-moved beads. A detector that selects none of these is broken.
pub const HAND_MOVED_TWELVE: &[&str] = &[
    "omp-orchestrator-s0-audit-detached-spawn-w21v",
    "omp-orchestrator-subprocess-contract-groupkill-reap-failures-chj5",
    "omp-orchestrator-tick-monitor-unbounded-reader-join-znwo",
    "omp-orchestrator-kill-group-missing-double-dash-o3eb",
    "omp-orchestrator-exit-const-collision-cas",
    "omp-orchestrator-plan-04-7wn9.5",
    "omp-orchestrator-typed-blocker-taxonomy-report-redispatch-zey6",
    "omp-orchestrator-wire-blocker-taxonomy-into-selector-jphd",
    "omp-orchestrator-s0-dag-pagerank-legibility-xbcl",
    "omp-orchestrator-s0-audit-spawn-census-falsifier-eoqd",
    "omp-orchestrator-s0-audit-budget-outcome-reach-yt9l",
    "omp-orchestrator-s0-audit-checkpoint-density-xv5r",
];

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct GradingCandidate {
    pub id: String,
    pub status: String,
    pub implementer: String,
    pub landed_commit: bool,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GradingDecision {
    Transition {
        bead: String,
        grader: String,
        /// Intended argv. Never executed here.
        br_update: Vec<String>,
    },
    RefuseSelfGrade {
        bead: String,
        implementer: String,
    },
    RefuseNoCommit {
        bead: String,
    },
}

impl GradingDecision {
    pub fn is_transition(&self) -> bool {
        matches!(self, Self::Transition { .. })
    }
}

pub fn is_hand_moved_twelve(id: &str) -> bool {
    HAND_MOVED_TWELVE.iter().any(|known| *known == id)
}

/// in_progress + cited commit. Empty input is zey6/jphd vacuity, reused.
pub fn detect<'a>(
    candidates: &'a [GradingCandidate],
) -> Result<Vec<&'a GradingCandidate>, ReportError> {
    if candidates.is_empty() {
        return Err(ReportError::VacuousWhileBaselineNonzero);
    }
    Ok(candidates
        .iter()
        .filter(|c| c.status == "in_progress" && c.landed_commit)
        .collect())
}

/// THE non-author check. Mutation deletes this call; the self-grade test must go RED.
pub fn reject_self_grade(
    bead: &str,
    implementer: &str,
    proposed_grader: &str,
) -> Result<String, AttributionRefusal> {
    let attempt = CloseAttempt {
        bead_id: bead.to_string(),
        implementer: Some(implementer.to_string()),
        actor: Some(proposed_grader.to_string()),
        close_reason: "IMPL_TO_GRADING".to_string(),
        comment_authors: Vec::new(),
        tmux_pane: None,
    };
    match assess_close(&attempt, DEFAULT_AUTHORS) {
        AttributionVerdict::Pass(pass) => Ok(pass.actor),
        AttributionVerdict::Refused(refusal) => Err(refusal),
    }
}

pub fn decide(
    candidate: &GradingCandidate,
    eligible_graders: &[String],
) -> GradingDecision {
    if !candidate.landed_commit {
        return GradingDecision::RefuseNoCommit {
            bead: candidate.id.clone(),
        };
    }
    if candidate.status != "in_progress" {
        return GradingDecision::RefuseNoCommit {
            bead: candidate.id.clone(),
        };
    }
    let mut last_refusal: Option<AttributionRefusal> = None;
    for grader in eligible_graders {
        match reject_self_grade(&candidate.id, &candidate.implementer, grader) {
            Ok(grader) => {
                return GradingDecision::Transition {
                    bead: candidate.id.clone(),
                    grader: grader.clone(),
                    br_update: vec![
                        "br".into(),
                        "update".into(),
                        candidate.id.clone(),
                        "--status".into(),
                        "grading".into(),
                        "--actor".into(),
                        grader,
                    ],
                };
            }
            Err(refusal) => last_refusal = Some(refusal),
        }
    }


    let _ = last_refusal;
    GradingDecision::RefuseSelfGrade {
        bead: candidate.id.clone(),
        implementer: candidate.implementer.clone(),
    }
}

pub fn decide_all(
    candidates: &[GradingCandidate],
    eligible_graders: &[String],
) -> Result<Vec<GradingDecision>, ReportError> {
    let detected = detect(candidates)?;
    Ok(detected
        .into_iter()
        .map(|c| decide(c, eligible_graders))
        .collect())
}

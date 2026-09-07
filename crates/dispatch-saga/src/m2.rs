//! M2: route a `grading` bead to an idle cross-lineage grader.
//!
//! Lineage is `--profile` in process argv. `AGENT_NAME` is uniform (WildStone)
//! and is not consulted. Decide-only: this module never sends.

pub use blocker_taxonomy::ReportError;
use crate::grading::reject_self_grade;



#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelLineage {
    Grok,
    Codex,
    Claude,
}

impl ModelLineage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Grok => "grok",
            Self::Codex => "codex",
            Self::Claude => "claude",
        }
    }
}

/// Parse `--profile` from a process command line. Ignores every other token.
pub fn lineage_from_argv(argv: &str) -> Option<ModelLineage> {
    let tokens: Vec<&str> = argv.split_whitespace().collect();
    for (i, tok) in tokens.iter().enumerate() {
        if *tok == "--profile" {
            return tokens.get(i + 1).copied().and_then(parse_profile);
        }
        if let Some(rest) = tok.strip_prefix("--profile=") {
            return parse_profile(rest);
        }
    }
    None
}

fn parse_profile(s: &str) -> Option<ModelLineage> {
    match s.trim().to_ascii_lowercase().as_str() {
        "grok" => Some(ModelLineage::Grok),
        "codex" => Some(ModelLineage::Codex),
        "claude" => Some(ModelLineage::Claude),
        _ => None,
    }
}

/// Ground-truth compatibility used by the three already-performed grades.
pub fn lineage_eligible(author: ModelLineage, grader: ModelLineage) -> bool {
    author != grader
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pane {
    pub pane: String,
    pub argv: String,
    pub idle: bool,
    pub safe_to_dispatch: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GradingBead {
    pub id: String,
    pub status: String,
    pub implementer_pane: String,
    pub implementer_profile: ModelLineage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RouteDecision {
    Route {
        bead: String,
        grader_pane: String,
        grader_profile: ModelLineage,
        /// Intended send. Never executed here.
        ntm_send: Vec<String>,
    },
    RefuseSelfGrade {
        bead: String,
        pane: String,
    },
    RefuseSameLineage {
        bead: String,
        implementer_profile: ModelLineage,
        other_pane: String,
        other_profile: ModelLineage,
    },
    RefuseNotGrading {
        bead: String,
    },
}

/// THE profile comparison. Mutation deletes this; same-profile refusal must go RED.
pub fn reject_same_lineage(
    author: ModelLineage,
    grader: ModelLineage,
) -> Result<(), (ModelLineage, ModelLineage)> {
    if lineage_eligible(author, grader) {
        Ok(())
    } else {
        Err((author, grader))
    }

}

pub fn route(bead: &GradingBead, panes: &[Pane]) -> Result<RouteDecision, ReportError> {
    if panes.is_empty() {
        return Err(ReportError::VacuousWhileBaselineNonzero);
    }
    if bead.status != "grading" {
        return Ok(RouteDecision::RefuseNotGrading {
            bead: bead.id.clone(),
        });
    }
    let mut same_lineage: Option<(String, ModelLineage)> = None;
    let mut self_grade: Option<String> = None;
    for pane in panes.iter().filter(|p| p.idle && p.safe_to_dispatch) {
        let Some(profile) = lineage_from_argv(&pane.argv) else {
            continue;
        };
        if pane.pane == bead.implementer_pane {
            self_grade = Some(pane.pane.clone());
            continue;
        }
        if let Err((_, other)) = reject_same_lineage(bead.implementer_profile, profile) {
            same_lineage = Some((pane.pane.clone(), other));
            continue;
        }
        match reject_self_grade(
            &bead.id,
            bead.implementer_profile.as_str(),
            profile.as_str(),
        ) {
            Ok(_) => {
                return Ok(RouteDecision::Route {
                    bead: bead.id.clone(),
                    grader_pane: pane.pane.clone(),
                    grader_profile: profile,
                    ntm_send: vec![
                        "ntm".into(),
                        format!("--robot-send=omp-orchestrator"),
                        format!("--panes={}", pane.pane.trim_start_matches('%')),
                    ],
                });
            }
            Err(_) => {
                self_grade = Some(pane.pane.clone());
            }
        }
    }
    if let Some((other_pane, other_profile)) = same_lineage {
        return Ok(RouteDecision::RefuseSameLineage {
            bead: bead.id.clone(),
            implementer_profile: bead.implementer_profile,
            other_pane,
            other_profile,
        });
    }
    if let Some(pane) = self_grade {
        return Ok(RouteDecision::RefuseSelfGrade {
            bead: bead.id.clone(),
            pane,
        });
    }
    Err(ReportError::VacuousWhileBaselineNonzero)
}

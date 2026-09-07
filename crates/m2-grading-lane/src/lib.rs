#![forbid(unsafe_code)]

//! Named consumer of `dispatch_saga::m2::route`. Decide-only.

pub use dispatch_saga::m2::{
    lineage_eligible, lineage_from_argv, route, GradingBead, ModelLineage, Pane, RouteDecision,
};

/// The lane entry: same decision as the kernel, so the kernel has a caller.
pub fn autoroute(
    bead: &GradingBead,
    panes: &[Pane],
) -> Result<RouteDecision, dispatch_saga::m2::ReportError> {
    route(bead, panes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consumer_calls_kernel_route() {
        let bead = GradingBead {
            id: "omp-orchestrator-exit-const-collision-cas".into(),
            status: "grading".into(),
            implementer_pane: "%9".into(),
            implementer_profile: ModelLineage::Grok,
        };
        let pane = Pane {
            pane: "%8".into(),
            argv: "omp --profile codex".into(),
            idle: true,
            safe_to_dispatch: true,
        };
        match autoroute(&bead, &[pane]).unwrap() {
            RouteDecision::Route { grader_profile, .. } => {
                assert_eq!(grader_profile, ModelLineage::Codex);
            }
            other => panic!("{other:?}"),
        }
    }
}


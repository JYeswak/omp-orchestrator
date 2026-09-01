#![forbid(unsafe_code)]

//! Convert recurring supervisor decisions into complete, schedulable findings.
//!
//! The first occurrence stays a log-level observation. The third identical occurrence
//! crosses the same escalation threshold used by the loop-enforcement contract and becomes
//! a finding. Healthy supervision is deliberately outside this mapping: filing a bead for
//! a fully occupied fleet would turn a correct state into alarm fatigue.

use finding::Finding;
use omp_orchestrator::SupervisorDecision;

/// The loop-enforcement threshold: two observations establish recurrence; the third
/// creates durable work. A caller should persist its recurrence counter between ticks.
pub const FINDING_THRESHOLD: u32 = 3;

/// Turn the threshold crossing for one supervisor decision into a complete finding.
///
/// Returns None before the threshold, after the crossing has already been emitted, and
/// for healthy/non-recurring decisions. The exact crossing avoids duplicate beads when a
/// caller continues observing the same condition after filing it once.
pub fn finding_for(decision: &SupervisorDecision, recurrence_count: u32) -> Option<Finding> {
    if recurrence_count != FINDING_THRESHOLD {
        return None;
    }

    let (what, why, acceptance, labels) = match decision {
        SupervisorDecision::EscalateIdleIncident {
            dispatchable_count,
            ready_count,
        } => (
            format!(
                "Escalate recurring idle-capacity incident: {dispatchable_count} dispatchable panes with {ready_count} ready beads"
            ),
            "The supervisor repeatedly observed free capacity beside ready work without an authorized dispatch. A printed warning is not durable work and previously allowed idle capacity to persist for 178 ticks.".to_owned(),
            format!(
                "Run the supervisor for three consecutive observations with dispatchable_count={dispatchable_count} and ready_count={ready_count}; verify the third observation produces this finding, names the idle panes and queue evidence in its bead comments, and routes the bead to a worker."
            ),
            vec![
                "supervisor".to_owned(),
                "idle-capacity".to_owned(),
                "recurring-incident".to_owned(),
            ],
        ),
        SupervisorDecision::QueueEmptyNeedsJosh { free_capacity_count } => (
            format!(
                "Escalate recurring queue-empty supervision decision with {free_capacity_count} free panes"
            ),
            "The queue stayed empty while workers were free and no authorization covered idleness. This requires an explicit operator decision instead of disappearing into a printed status line.".to_owned(),
            format!(
                "Run the supervisor for three consecutive observations with free_capacity_count={free_capacity_count}; verify the third observation produces this finding and records Josh's queue-empty decision before any autonomous dispatch."
            ),
            vec![
                "supervisor".to_owned(),
                "queue-empty".to_owned(),
                "josh-decision".to_owned(),
            ],
        ),
        SupervisorDecision::MonitorBlind { detail } => (
            "Escalate recurring monitor-blind condition".to_owned(),
            format!(
                "The supervisor cannot establish fleet truth, so dispatch decisions are unsafe. Repeating a blind observation without durable work leaves the monitor failure unowned. Observed detail: {detail}"
            ),
            "Run the supervisor for three consecutive blind observations; verify the third observation produces this finding, preserves the monitor error detail, and routes monitor repair before dispatch resumes.".to_owned(),
            vec![
                "supervisor".to_owned(),
                "monitor-blind".to_owned(),
                "liveness".to_owned(),
            ],
        ),
        SupervisorDecision::WorkspaceUnloaded { detail } => (
            "Escalate recurring unloaded-workspace condition".to_owned(),
            format!(
                "The workspace-load gate repeatedly refused a trusted repository context, so dispatch would target an unverified workspace. Printed errors do not create durable repair work. Observed detail: {detail}"
            ),
            "Run the supervisor for three consecutive workspace-unloaded observations; verify the third observation produces this finding, preserves the workspace error detail, and routes workspace repair before dispatch resumes.".to_owned(),
            vec![
                "supervisor".to_owned(),
                "workspace-unloaded".to_owned(),
                "dispatch-safety".to_owned(),
            ],
        ),
        SupervisorDecision::GateUnwired { unwired } => (
            "Escalate recurring unwired-gate condition".to_owned(),
            format!(
                "The gate census repeatedly found unreachable gates, so their protections cannot be treated as active. Observed unwired gates: {unwired:?}"
            ),
            "Run the supervisor for three consecutive gate-unwired observations; verify the third observation produces this finding, preserves every unreachable gate name, and routes trigger repair before dispatch resumes.".to_owned(),
            vec![
                "supervisor".to_owned(),
                "gate-unwired".to_owned(),
                "dispatch-safety".to_owned(),
            ],
        ),
        SupervisorDecision::SupervisedWorking { .. }
        | SupervisorDecision::Dispatch { .. }
        | SupervisorDecision::QueueUnreadable { .. }
        | SupervisorDecision::AuthorizedIdle { .. } => return None,
    };

    // These values are literals assembled from a closed enum. Failure here is a programmer
    // invariant violation: silently returning None would recreate the finding leak.
    Some(
        Finding::new(what, why, acceptance, labels, 1)
            .expect("supervisor decision mapping must satisfy Finding's bead contract"),
    )
}

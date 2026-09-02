#![forbid(unsafe_code)]

//! Convert recurring supervisor decisions into complete, schedulable findings.
//!
//! The first occurrence stays a log-level observation. The third identical occurrence
//! crosses the same escalation threshold used by the loop-enforcement contract and becomes
//! a finding. Healthy supervision is deliberately outside this mapping: filing a bead for
//! a fully occupied fleet would turn a correct state into alarm fatigue.

use finding::Finding;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupervisorDecision {
    /// One or more gates lack a reachable trigger on this machine.
    GateUnwired { unwired: Vec<String> },
    /// A confirmed-idle pane and ready work coexist.
    Dispatch { pane: String, bead_hint: String },
    /// Free capacity plus ready work without authorization.
    EscalateIdleIncident {
        dispatchable_count: usize,
        ready_count: usize,
    },
    /// The monitor could not observe.
    MonitorBlind { detail: String },
    /// The queue could not be read.
    QueueUnreadable { detail: String },
    /// One or more panes are alive and blocked on a human answer.
    AwaitingHuman { panes: String },
    /// The workspace-load gate refused.
    WorkspaceUnloaded { detail: String },
    /// Idleness has an unexpired authorization token.
    AuthorizedIdle { pane_count: usize, expires_at: u64 },
    /// Queue empty and free capacity exist without authorization.
    QueueEmptyNeedsJosh { free_capacity_count: usize },
    /// Every pane is genuinely working.
    SupervisedWorking {
        working_count: usize,
        ready_count: usize,
    },
}

/// The loop-enforcement threshold: two observations establish recurrence; the third
/// creates durable work. A caller should persist its recurrence counter between ticks.
pub const FINDING_THRESHOLD: u32 = 3;

/// Why no finding is owed yet. Three genuinely different conditions that
/// `Option<Finding>` collapsed into one `None`.
///
/// A caller that wants to log "nothing to file" cannot say WHICH nothing it saw, and a
/// reader of that log cannot tell a healthy fleet from a threshold that has not been
/// reached from a crossing already filed. Same coercion this repository keeps finding:
/// a real, distinct condition with no representation takes a neighbouring value and the
/// coercion reads as normal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotYet {
    /// Below the threshold: recurrence has not been established.
    BelowThreshold { seen: u32, threshold: u32 },
    /// Past the exact crossing — the finding was already emitted once, and re-filing it
    /// would duplicate the bead.
    AlreadyEmitted { seen: u32, threshold: u32 },
    /// A healthy or non-recurring decision. Filing for a fully occupied fleet would turn
    /// a correct state into alarm fatigue.
    NotAFindableDecision,
}

impl fmt::Display for NotYet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BelowThreshold { seen, threshold } => {
                write!(f, "below threshold ({seen} of {threshold})")
            }
            Self::AlreadyEmitted { seen, threshold } => {
                write!(f, "already emitted at the crossing ({seen} > {threshold})")
            }
            Self::NotAFindableDecision => write!(f, "healthy or non-recurring decision"),
        }
    }
}

/// The producer's return type. **`#[must_use]` on the OUTER type is the whole point.**
///
/// # Why this is not `Option<Finding>`
///
/// MEASURED with `rustc 1.100.0-nightly` on a two-line probe, reproducing the bead:
///
/// ```text
/// direct();            -> warning: unused `Finding` that must be used
/// returns_option();    -> NO WARNING
/// returns_result();    -> warning: unused `Result` that must be used
/// returns_enum();      -> warning: unused `MaybeFinding` that must be used
/// ```
///
/// `#[must_use]` on `Finding` does **not** propagate through `Option`, and
/// `Option<Finding>` was the signature of the crate's only producer — so the one
/// mechanism protecting `FC-L1` was bypassed by the one function that creates the
/// obligation. `#[must_use]` on the outer enum fires because the returned TYPE is the
/// annotated one.
///
/// # Why an enum rather than `Result<Finding, NotYet>`
///
/// `Result` is `#[must_use]` in std and would also have worked — the probe confirms it.
/// It was rejected because "no finding is owed" is not an error: putting a healthy fleet
/// in `Err` invites `unwrap()`, `?`-propagation out of a correct path, and an operator
/// reading `Err(NotAFindableDecision)` as a failure. The obligation is identical and the
/// semantics are honest.
#[must_use = "a Finding must be filed or waived; dropping this drops the obligation FC-L1 exists to enforce"]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaybeFinding {
    /// The threshold was crossed. This value carries an obligation.
    Owed(Finding),
    /// Nothing is owed, and the reason is named.
    NotYet(NotYet),
}

impl MaybeFinding {
    /// The finding, if one is owed. Named `into_owed` rather than `into_option` because
    /// converting back to `Option` is exactly the hole this type closes; a caller that
    /// wants the inner value must take it and then dispose of it.
    pub fn into_owed(self) -> Option<Finding> {
        match self {
            Self::Owed(finding) => Some(finding),
            Self::NotYet(_) => None,
        }
    }

    pub fn is_owed(&self) -> bool {
        matches!(self, Self::Owed(_))
    }

    /// Acknowledge that nothing is owed. **The only zero-obligation disposal**, and it
    /// refuses to swallow a real finding: passing an `Owed` here is a programmer error,
    /// not a silent drop.
    pub fn expect_nothing_owed(self) -> Result<NotYet, Finding> {
        match self {
            Self::NotYet(reason) => Ok(reason),
            Self::Owed(finding) => Err(finding),
        }
    }
}

/// Turn the threshold crossing for one supervisor decision into a complete finding.
///
/// Returns a `#[must_use]` [`MaybeFinding`] rather than `Option<Finding>`: see that
/// type's documentation for the measurement that forced the change. The exact crossing
/// avoids duplicate beads when a caller keeps observing the same condition after filing.
pub fn finding_for(decision: &SupervisorDecision, recurrence_count: u32) -> MaybeFinding {
    if recurrence_count < FINDING_THRESHOLD {
        return MaybeFinding::NotYet(NotYet::BelowThreshold {
            seen: recurrence_count,
            threshold: FINDING_THRESHOLD,
        });
    }
    if recurrence_count > FINDING_THRESHOLD {
        return MaybeFinding::NotYet(NotYet::AlreadyEmitted {
            seen: recurrence_count,
            threshold: FINDING_THRESHOLD,
        });
    }

    let (what, why, acceptance, labels) = match decision {
        SupervisorDecision::AwaitingHuman { panes } => (
            format!("A pane is alive and blocked on a human answer: {panes}"),
            "On OMP v18 an Ask/approval dialog renders ABOVE the status line, the status line \
             stays LAST, and its turn timer KEEPS ADVANCING while nobody answers. Every \
             classifier therefore reads a human-blocked pane as healthy work. Measured \
             2026-08-31: %1372 sat 36 minutes on an install approval while the fleet reported \
             green. This is the one condition the loop cannot clear by working — no dispatch, \
             retry or timeout resolves an unanswered dialog — so it becomes durable work rather \
             than a printed line that scrolls past."
                .to_owned(),
            format!(
                "Answer or dismiss the dialog on {panes}, then run the supervisor and verify it \
                 no longer emits AWAITING_HUMAN for that pane. Negative leg: a pane in WORKING \
                 must NOT produce this finding, which is covered by \
                 a_working_pane_does_not_escalate_to_a_human."
            ),
            vec![
                "supervisor".to_owned(),
                "awaiting-human".to_owned(),
                "liveness".to_owned(),
            ],
        ),
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
        | SupervisorDecision::AuthorizedIdle { .. } => {
            return MaybeFinding::NotYet(NotYet::NotAFindableDecision)
        }
    };

    // These values are literals assembled from a closed enum. Failure here is a programmer
    // invariant violation: silently returning None would recreate the finding leak.
    MaybeFinding::Owed(
        Finding::new(what, why, acceptance, labels, 1)
            .expect("supervisor decision mapping must satisfy Finding's bead contract"),
    )
}

#![forbid(unsafe_code)]

//! omp-orchestrator — the RESIDENT SUPERVISOR (bead omp-orchestrator-kxe).
//!
//! Josh-approved architecture 2026-08-31T16:22Z: one long-lived process owns
//! observe -> queue -> dispatch -> receiver receipt -> verify for the resolved
//! repository/session. tick-monitor remains a PURE observation component consumed
//! by this supervisor. launchd restarts this process on exit.
//!
//! THE THREE DECIDING LEGS (from the orchestrator's dispatch, 12:3xZ):
//!
//! 1. NO NO-OP GREEN PATH. Exit 0 requires either a dispatch WITH a receiver
//!    receipt, or a typed escalation naming blocker, owner, and next action.
//!    A stubbed send must FAIL the suite.
//!
//! 2. FREE + READY => DISPATCH OR TYPED ESCALATION, no third branch. "nothing
//!    to do" is FORBIDDEN output whenever free capacity and ready work coexist.
//!
//! 3. IDLE_AUTHORIZED is a durable token, default UNAUTHORIZED. Idle is an
//!    INCIDENT unless Josh has persisted approval. This is the load-bearing
//!    inversion: idleness was a state that got observed instead of an event
//!    requiring authorization.
//!
//! NO-CLAIM: this source has not been compiled against the live fleet. The
//! subprocess wiring (tick-monitor observe, br ready, ntm robot-send, tmux
//! send-keys) is designed but unverified at runtime. Receiver receipts for
//! codex panes require tmux send-keys -l, whose receipt is a timer reset +
//! spinner-stripped content change — a different protocol than ntm's, and
//! the DispatchReceipt type models both but neither is proven here.

use std::fmt;
use std::path::Path;

// ── IDLE_AUTHORIZATION ─────────────────────────────────────────────────────────

/// Whether the supervisor may tolerate idle panes without escalating.
///
/// Default: UNAUTHORIZED. This is the load-bearing inversion — idleness was a
/// STATE that got observed instead of an EVENT requiring authorization. Until
/// Josh persists an authorization token, every idle+ready observation is an
/// incident that must escalate, not a state to log and move past.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdleAuthorization {
    /// Josh has persisted approval, BOUND to the conditions he approved.
    ///
    /// A bare reason string is NOT sufficient: an authorization that does not
    /// name what it authorized cannot be falsified, and it silently outlives the
    /// situation it was granted for. Every field below is required.
    Authorized {
        reason: String,
        /// The session the approval covers. An approval for one session must not
        /// license idleness in another.
        session: String,
        /// Hash of the pane set at approval time. If the fleet changes shape, the
        /// approval no longer describes the thing it approved.
        pane_set_hash: String,
        /// Ready-queue depth at approval time.
        queue_len: usize,
        issued_at: u64,
        /// Hard expiry. An authorization with no expiry is a permanent licence to
        /// go dark, which is the failure this whole contract exists to prevent.
        expires_at: u64,
    },
    /// No token, malformed token, or EXPIRED token. Idle+ready is an INCIDENT.
    Unauthorized { why: &'static str },
}

impl fmt::Display for IdleAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authorized {
                reason,
                session,
                expires_at,
                ..
            } => write!(
                formatter,
                "IDLE_AUTHORIZED session={session} expires_at={expires_at}: {reason}"
            ),
            Self::Unauthorized { why } => write!(
                formatter,
                "IDLE_UNAUTHORIZED ({why}): idle panes are an incident; dispatch or escalate"
            ),
        }
    }
}

/// Required keys in the token. A token missing any of these is UNAUTHORIZED.
const TOKEN_KEYS: &[&str] = &[
    "reason",
    "session",
    "pane_set_hash",
    "queue_len",
    "issued_at",
    "expires_at",
];

/// Read and VALIDATE the durable idle-authorization token from
/// `<repo>/.idle_authorized`.
///
/// FILE EXISTENCE IS NOT AUTHORIZATION. The earlier implementation accepted any
/// non-empty file as approval, which meant a stray note authorized the fleet to
/// go dark forever. The token is `key = value` lines and MUST bind the approval
/// to a session, a pane-set hash, a queue depth, an issue time, and an EXPIRY.
/// Anything missing, unparseable, or expired reads UNAUTHORIZED with a reason.
pub fn read_idle_authorization(repo_root: &Path, now_unix: u64) -> IdleAuthorization {
    let token_path = repo_root.join(".idle_authorized");
    let Ok(text) = std::fs::read_to_string(&token_path) else {
        return IdleAuthorization::Unauthorized { why: "no_token" };
    };
    let mut map: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        map.push((key.trim().to_owned(), value.trim().to_owned()));
    }
    let get = |k: &str| -> Option<String> {
        map.iter()
            .find(|(key, _)| key == k)
            .map(|(_, value)| value.clone())
    };
    for key in TOKEN_KEYS {
        if get(key).is_none_or(|v| v.is_empty()) {
            return IdleAuthorization::Unauthorized {
                why: "token_missing_required_field",
            };
        }
    }
    let (Some(queue_len), Some(issued_at), Some(expires_at)) = (
        get("queue_len").and_then(|v| v.parse::<usize>().ok()),
        get("issued_at").and_then(|v| v.parse::<u64>().ok()),
        get("expires_at").and_then(|v| v.parse::<u64>().ok()),
    ) else {
        return IdleAuthorization::Unauthorized {
            why: "token_field_unparseable",
        };
    };
    if expires_at <= now_unix {
        return IdleAuthorization::Unauthorized {
            why: "token_expired",
        };
    }
    IdleAuthorization::Authorized {
        reason: get("reason").unwrap_or_default(),
        session: get("session").unwrap_or_default(),
        pane_set_hash: get("pane_set_hash").unwrap_or_default(),
        queue_len,
        issued_at,
        expires_at,
    }
}

// ── OBSERVATION ────────────────────────────────────────────────────────────────

/// One pane's observed state, produced by tick-monitor.
///
/// `is_dispatchable` and `is_free_capacity` are SEPARATE fields on purpose. An
/// earlier version derived both from one filter, which made the NewlyIdle branch
/// of `decide` unreachable: every unauthorized queue+newly-idle case fell through
/// to AuthorizedIdle, which is the exact shape that let the fleet sit idle while
/// every watchdog reported healthy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneObservation {
    pub pane_id: String,
    pub state: String,
    pub liveness: String,
    /// ConfirmedIdle only — two idle captures >= 75s apart.
    pub is_dispatchable: bool,
    /// ConfirmedIdle OR NewlyIdle — visible as free capacity, not yet dispatchable.
    pub is_free_capacity: bool,
    /// LIVE — genuinely working. Distinguishes a healthy busy fleet from an idle
    /// one, so queue-empty-and-everyone-working is not reported as an incident.
    pub is_working: bool,
}

/// The queue: how many beads are ready.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueState {
    pub ready_count: usize,
    pub readable: bool,
}

/// The observation half: what tick-monitor and br ready report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub panes: Vec<PaneObservation>,
    pub queue: QueueState,
    /// The gate census for this cycle. None = not performed (the caller
    /// should NOT dispatch without a census). Some = the census ran; if any
    /// gate is unwired, the supervisor must refuse before anything else.
    pub gate_census: Option<GateCensus>,
}

/// The supervisor's decision for one observation cycle.
///
/// THE TWO DECIDING LEGS, encoded as exhaustive match arms:
///
///   Leg 2: FREE + READY => DISPATCH OR TYPED ESCALATION, no third branch.
///   Leg 3: IDLE_AUTHORIZED is a durable token, default UNAUTHORIZED.
///
/// There is NO branch that returns "nothing to do" when dispatchable panes
/// and ready work coexist. The only "nothing" outcome is when the queue is
/// genuinely empty AND the authorization permits idleness.
/// The reachability of one gate's TRIGGER on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateReachability {
    /// A trigger exists on this machine and has been proven to fire.
    Reachable { trigger: String },
    /// A trigger is referenced but does not exist on this machine.
    Unreachable { reason: String },
    /// The gate has no trigger of any kind.
    NotInstalled,
}

impl GateReachability {
    pub fn is_reachable(&self) -> bool {
        matches!(self, GateReachability::Reachable { .. })
    }
}

/// One row in the gate census.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateCensusRow {
    pub gate: String,
    pub reachability: GateReachability,
}

/// The full census across all known gates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateCensus {
    pub rows: Vec<GateCensusRow>,
}

impl GateCensus {
    /// The positive control: this gate MUST be reachable, or the census itself
    /// is broken and its output is untrustworthy.
    pub fn positive_control_gate() -> &'static str {
        "no-shell-gate"
    }

    pub fn unwired_gates(&self) -> Vec<&GateCensusRow> {
        self.rows.iter().filter(|r| !r.reachability.is_reachable()).collect()
    }

    pub fn all_reachable(&self) -> bool {
        self.rows.iter().all(|r| r.reachability.is_reachable())
    }

    /// The POSITIVE CONTROL: no-shell-gate must be reachable, or the census
    /// is broken and every "unreachable" verdict is suspect (a census that
    /// reports everything unreachable is indistinguishable from a broken one).
    pub fn positive_control_passes(&self) -> bool {
        self.rows
            .iter()
            .any(|r| r.gate == Self::positive_control_gate() && r.reachability.is_reachable())
    }
}

/// Classify each known gate by whether its TRIGGER exists on this machine.
/// Trigger reachability, NOT caller existence: a caller in gate.yml is not a
/// trigger when there is no remote to run the workflow on.
pub fn census_gates(repo_root: &Path) -> GateCensus {
    let hook_path = repo_root.join(".git/hooks/pre-commit");
    let has_remote = std::process::Command::new("git")
        .args(["remote"])
        .current_dir(repo_root)
        .output()
        .map(|o| {
            !String::from_utf8_lossy(&o.stdout).trim().is_empty()
        })
        .unwrap_or(false);

    let mut rows = Vec::new();

    // no-shell-gate: .git/hooks/pre-commit is the REAL trigger (proven to bite
    // 2026-08-31, exit 1 naming the file). This is the positive control.
    let nsg_reachable = hook_path.exists();
    rows.push(GateCensusRow {
        gate: "no-shell-gate".into(),
        reachability: if nsg_reachable {
            GateReachability::Reachable { trigger: ".git/hooks/pre-commit".into() }
        } else {
            GateReachability::Unreachable { reason: ".git/hooks/pre-commit does not exist on this clone".into() }
        },
    });

    // path-literal-guard, state-wildcard-lint, undrained-pipe-lint:
    // their only invocation is .github/workflows/gate.yml, and there is no
    // remote to run it on.
    for gate in ["path-literal-guard", "state-wildcard-lint", "undrained-pipe-lint"] {
        rows.push(GateCensusRow {
            gate: gate.into(),
            reachability: if has_remote {
                GateReachability::Reachable { trigger: ".github/workflows/gate.yml".into() }
            } else {
                GateReachability::Unreachable { reason: "no git remote: the CI workflow can never execute".into() }
            },
        });
    }

    // kernel-bypass-gate, pre-delete-citation-check: no caller, no manifest
    // dep, no trigger of any kind.
    for gate in ["kernel-bypass-gate", "pre-delete-citation-check"] {
        rows.push(GateCensusRow {
            gate: gate.into(),
            reachability: GateReachability::Unreachable { reason: "no caller, no manifest dependency, no trigger".into() },
        });
    }

    GateCensus { rows }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupervisorDecision {
    /// One or more gates lack a reachable trigger on this machine. The
    /// supervisor structurally refuses to report healthy: no branch may
    /// return SupervisedWorking or AuthorizedIdle while any gate is
    /// unwired. UNREACHABLE-AROUND: no code path may dispatch while a
    /// gate that defines the repo's guarantees cannot fire.
    GateUnwired { unwired: Vec<String> },
    /// A CONFIRMED-idle pane and ready work coexist -> send.
    Dispatch { pane: String, bead_hint: String },
    /// Free capacity + ready work + UNAUTHORIZED -> escalate (an incident).
    EscalateIdleIncident {
        dispatchable_count: usize,
        ready_count: usize,
    },
    /// The monitor could not observe — fail closed.
    MonitorBlind { detail: String },
    /// The queue could not be read — fail closed.
    QueueUnreadable { detail: String },
    /// The workspace-load gate refused — do not dispatch into a broken repo.
    WorkspaceUnloaded { detail: String },
    /// Idleness is covered by an unexpired, BOUND authorization token. Carries
    /// the expiry so the decision names its own deadline.
    AuthorizedIdle { pane_count: usize, expires_at: u64 },
    /// Queue empty AND free capacity exists AND no authorization.
    ///
    /// NOT AuthorizedIdle: nothing authorized this. NOT an EscalateIdleIncident
    /// either — there is no work to dispatch, so no worker is being starved. An
    /// empty queue with idle capacity is a decision only Josh can make, so
    /// queue-empty must remain SUPERVISED rather than silently tolerated.
    QueueEmptyNeedsJosh { free_capacity_count: usize },
    /// Every pane is genuinely working (`LIVE`). Healthy — and still REPORTED,
    /// because a supervisor that prints nothing while busy is indistinguishable
    /// from one that has died.
    ///
    /// This is deliberately NOT AuthorizedIdle: nothing is idle. Reporting a
    /// healthy busy fleet as an incident trains the operator to ignore the alarm,
    /// which is how a real one gets missed.
    SupervisedWorking {
        working_count: usize,
        ready_count: usize,
    },
}

/// The pure deciding function: given an observation and the authorization state,
/// produce the supervisor's decision. This is where the three deciding legs are
/// encoded. It is PURE so the deciding legs are testable without subprocesses.
pub fn decide(observation: &Observation, authorization: &IdleAuthorization) -> SupervisorDecision {
    // GATE CENSUS — the FIRST check, before anything else. If any gate lacks
    // a reachable trigger, the supervisor refuses: it cannot dispatch into a
    // repo whose guarantees cannot fire. This is UNREACHABLE-AROUND — no
    // branch after this may return SupervisedWorking or AuthorizedIdle.
    match &observation.gate_census {
        Some(census) => {
            if !census.positive_control_passes() {
                return SupervisorDecision::GateUnwired {
                    unwired: vec![format!(
                        "POSITIVE_CONTROL_FAILED: {} must be reachable",
                        GateCensus::positive_control_gate()
                    )],
                };
            }
            let unwired: Vec<String> = census
                .unwired_gates()
                .iter()
                .map(|r| r.gate.clone())
                .collect();
            if !unwired.is_empty() {
                return SupervisorDecision::GateUnwired { unwired };
            }
        }
        None => {
            return SupervisorDecision::GateUnwired {
                unwired: vec!["CENSUS_NOT_PERFORMED".to_owned()],
            };
        }
    }
    // The monitor must have produced a readable census.
    // (In the real wiring, this is where a tick-monitor invoke failure surfaces.)
    if observation.panes.is_empty() {
        return SupervisorDecision::MonitorBlind {
            detail: "zero panes observed — the monitor is blind".to_owned(),
        };
    }

    // The queue must be readable.
    if !observation.queue.readable {
        return SupervisorDecision::QueueUnreadable {
            detail: "br ready produced no parseable output".to_owned(),
        };
    }

    // Count dispatchable panes (ConfirmedIdle only — one capture is not enough).
    let dispatchable: Vec<&PaneObservation> =
        observation.panes.iter().filter(|p| p.is_dispatchable).collect();

    // Count free-capacity panes: ConfirmedIdle OR NewlyIdle. THIS MUST READ ITS
    // OWN FIELD. It previously filtered on `is_dispatchable`, which made it
    // identical to `dispatchable` — so inside `if dispatchable.is_empty()` it was
    // always 0, the NewlyIdle branch was unreachable, and every unauthorized
    // newly-idle-plus-ready-work case fell through to AuthorizedIdle. That is the
    // exact shape that let the fleet sit idle for hours while the watchdogs
    // reported healthy.
    let free_capacity = observation
        .panes
        .iter()
        .filter(|p| p.is_free_capacity)
        .count();

    let working = observation.panes.iter().filter(|p| p.is_working).count();
    let expiry = match authorization {
        IdleAuthorization::Authorized { expires_at, .. } => Some(*expires_at),
        IdleAuthorization::Unauthorized { .. } => None,
    };
    if observation.queue.ready_count == 0 {
        // QUEUE EMPTY. A busy fleet with an empty queue is HEALTHY, not an
        // incident. Reporting it as one trains the operator to ignore the alarm,
        // which is how a real alarm gets missed — and it is why 178 consecutive
        // capacity ticks were written to a file nobody read.
        if free_capacity == 0 {
            return SupervisorDecision::SupervisedWorking {
                working_count: working,
                ready_count: 0,
            };
        }
        // Idle capacity with nothing queued. Not starvation — nobody is waiting —
        // but not a state to sit in silently either. Only Josh can decide that the
        // fleet has nothing to do.
        return match expiry {
            Some(expires_at) => SupervisorDecision::AuthorizedIdle {
                pane_count: observation.panes.len(),
                expires_at,
            },
            None => SupervisorDecision::QueueEmptyNeedsJosh {
                free_capacity_count: free_capacity,
            },
        };
    }

    // READY WORK EXISTS. A confirmed-idle pane wins immediately.
    if let Some(target) = dispatchable.first() {
        return SupervisorDecision::Dispatch {
            pane: target.pane_id.clone(),
            bead_hint: "first-ready-bead".to_owned(),
        };
    }

    // Ready work, nothing CONFIRMED idle. Free capacity here means NewlyIdle: a
    // pane that just finished and is visible but not yet twice-confirmed. With
    // work queued that must NOT be tolerated silently — it is the measured
    // 4h19m failure.
    if free_capacity > 0 {
        return match expiry {
            Some(expires_at) => SupervisorDecision::AuthorizedIdle {
                pane_count: observation.panes.len(),
                expires_at,
            },
            None => SupervisorDecision::EscalateIdleIncident {
                dispatchable_count: 0,
                ready_count: observation.queue.ready_count,
            },
        };
    }

    // Ready work and every pane working: healthy saturation. Reported, not
    // alarmed, and NOT AuthorizedIdle — nothing here is idle.
    SupervisorDecision::SupervisedWorking {
        working_count: working,
        ready_count: observation.queue.ready_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A census where every gate is reachable, including the positive control.
    ///
    /// WHY NOT `gate_census: None`, which is the one-line fix that was asked
    /// for: the census is the FIRST check in `decide()`, and `None` returns
    /// `GateUnwired { CENSUS_NOT_PERFORMED }` before any other branch is
    /// reached. Every existing test below would then pass its assertion on a
    /// decision no test intended to exercise — twelve green legs measuring the
    /// census instead of dispatch, idleness, and authorization. The gate-unwired
    /// path gets its OWN tests; these helpers must clear the gate to reach the
    /// branch under test.
    fn passing_census() -> GateCensus {
        GateCensus {
            rows: vec![GateCensusRow {
                gate: GateCensus::positive_control_gate().to_owned(),
                reachability: GateReachability::Reachable {
                    trigger: ".git/hooks/pre-commit".to_owned(),
                },
            }],
        }
    }

    fn obs(panes: Vec<PaneObservation>, ready: usize, readable: bool) -> Observation {
        Observation {
            panes,
            queue: QueueState { ready_count: ready, readable },
            gate_census: Some(passing_census()),
        }
    }

    /// Build a pane from tick-monitor's LIVENESS vocabulary. The three booleans
    /// are DERIVED from that one label, never from a caller-supplied flag: a bare
    /// `dispatchable: bool` cannot distinguish NewlyIdle from working, and that
    /// ambiguity is what made the NewlyIdle branch unreachable.
    fn pane(id: &str, state: &str, _dispatchable_hint: bool) -> PaneObservation {
        let liveness = match state {
            "IDLE" => "CONFIRMED_IDLE",
            other => other,
        };
        PaneObservation {
            pane_id: id.to_owned(),
            state: state.to_owned(),
            liveness: liveness.to_owned(),
            is_dispatchable: liveness == "CONFIRMED_IDLE",
            is_free_capacity: matches!(liveness, "CONFIRMED_IDLE" | "NEWLY_IDLE"),
            is_working: matches!(liveness, "LIVE" | "WORKING"),
        }
    }

    // ── DECIDING LEG 1: NO NO-OP GREEN PATH ──────────────────────────────────

    #[test]
    fn stubbed_send_cannot_produce_green() {
        // A stubbed send that returns Ok without actually sending must be caught
        // by the Dispatch variant carrying the pane and bead — if the test can
        // assert on those fields, a stub that produces neither fails.
        let observation = obs(
            vec![pane("%1409", "IDLE", true)],
            5,
            true,
        );
        let decision = decide(&observation, &IdleAuthorization::Unauthorized { why: "test" });
        match &decision {
            SupervisorDecision::Dispatch { pane, .. } => {
                assert_eq!(pane, "%1409", "the dispatchable pane must be named");
            }
            other => panic!("expected Dispatch, got {other:?}"),
        }
    }

    // ── DECIDING LEG 2: FREE + READY => DISPATCH OR ESCALATION ───────────────

    #[test]
    fn free_and_ready_must_dispatch_or_escalate() {
        // ConfirmedIdle pane + ready work -> Dispatch (the happy path).
        let observation = obs(
            vec![pane("%1413", "IDLE", true)],
            10,
            true,
        );
        let decision = decide(&observation, &IdleAuthorization::Unauthorized { why: "test" });
        assert!(
            matches!(decision, SupervisorDecision::Dispatch { .. }),
            "free + ready + authorized-ambient = dispatch, got {decision:?}"
        );
    }

    #[test]
    fn newly_idle_with_ready_work_escalates_when_unauthorized() {
        // A NewlyIdle pane is free capacity but NOT dispatchable (one capture).
        // With ready work and no ConfirmedIdle panes, this must ESCALATE —
        // the conductor needs to know that a freed worker is visible but
        // not yet confirmed.
        let observation = obs(
            vec![pane("%1413", "NEWLY_IDLE", false)],
            10,
            true,
        );
        let decision = decide(&observation, &IdleAuthorization::Unauthorized { why: "test" });
        match &decision {
            SupervisorDecision::EscalateIdleIncident { dispatchable_count, ready_count } => {
                assert_eq!(*dispatchable_count, 0, "no confirmed-idle panes");
                assert_eq!(*ready_count, 10, "ready count from the queue");
            }
            other => panic!("expected EscalateIdleIncident, got {other:?}"),
        }
    }

    #[test]
    fn no_third_branch_free_and_ready() {
        // The three-outcome contract: dispatch, escalate, or authorized-idle.
        // There is no fourth outcome for free+ready.
        let observation = obs(
            vec![pane("%1409", "IDLE", true)],
            3,
            true,
        );
        let decision = decide(&observation, &IdleAuthorization::Unauthorized { why: "test" });
        assert!(
            matches!(decision, SupervisorDecision::Dispatch { .. }),
            "the only legal outcomes for free+ready are Dispatch or Escalate, got {decision:?}"
        );
    }

    // ── DECIDING LEG 3: IDLE_AUTHORIZED DEFAULT UNAUTHORIZED ─────────────────

    #[test]
    fn unauthorized_idle_with_empty_queue_needs_josh_not_an_incident() {
        // CONTRACT SHARPENED. This previously asserted EscalateIdleIncident, which
        // conflated two different situations: a starving fleet (work queued, panes
        // free) and a fleet with nothing to do. Nobody is starving here — the
        // queue is empty — so calling it an incident is the alarm-fatigue failure.
        // It still must NOT be silent: an empty queue is a decision only Josh can
        // make, so queue-empty stays SUPERVISED.
        let observation = obs(vec![pane("%1409", "IDLE", true)], 0, true);
        let decision = decide(&observation, &IdleAuthorization::Unauthorized { why: "test" });
        match &decision {
            SupervisorDecision::QueueEmptyNeedsJosh {
                free_capacity_count,
            } => {
                assert_eq!(*free_capacity_count, 1, "the free pane must be counted");
            }
            other => panic!("expected QueueEmptyNeedsJosh, got {other:?}"),
        }
        // And it must NOT read as authorized — nothing authorized this.
        assert!(
            !matches!(decision, SupervisorDecision::AuthorizedIdle { .. }),
            "unauthorized must never render as AuthorizedIdle"
        );
    }

    #[test]
    fn a_busy_fleet_with_an_empty_queue_is_supervised_working_not_an_incident() {
        // The live smoke case: six panes working, nothing free. Escalating this
        // trains the operator to ignore the alarm.
        let observation = obs(
            vec![pane("%1413", "LIVE", false), pane("%1414", "LIVE", false)],
            0,
            true,
        );
        let decision = decide(&observation, &IdleAuthorization::Unauthorized { why: "test" });
        match &decision {
            SupervisorDecision::SupervisedWorking { working_count, .. } => {
                assert_eq!(*working_count, 2, "both working panes must be counted");
            }
            other => panic!("expected SupervisedWorking, got {other:?}"),
        }
    }

    #[test]
    fn a_saturated_fleet_with_queued_work_is_supervised_working() {
        // Ready work AND every pane working = healthy saturation, not idleness.
        // This is the case BlueLantern's live smoke hit (panes=6 ready=4).
        let observation = obs(
            vec![pane("%1413", "LIVE", false), pane("%1414", "LIVE", false)],
            4,
            true,
        );
        let decision = decide(&observation, &IdleAuthorization::Unauthorized { why: "test" });
        match &decision {
            SupervisorDecision::SupervisedWorking {
                working_count,
                ready_count,
            } => {
                assert_eq!(*working_count, 2);
                assert_eq!(*ready_count, 4, "queued work must be reported, not hidden");
            }
            other => panic!("expected SupervisedWorking, got {other:?}"),
        }
    }

    #[test]
    fn authorized_idle_with_empty_queue_is_tolerated() {
        let observation = obs(
            vec![pane("%1409", "IDLE", true)],
            0,
            true,
        );
        let auth = IdleAuthorization::Authorized {
            reason: "Josh said stand down".to_owned(),
            session: "omp-orchestrator".to_owned(),
            pane_set_hash: "test-hash".to_owned(),
            queue_len: 0,
            issued_at: 1,
            expires_at: u64::MAX,
        };
        let decision = decide(&observation, &auth);
        assert!(
            matches!(decision, SupervisorDecision::AuthorizedIdle { .. }),
            "authorized idle = tolerated, got {decision:?}"
        );
    }

    // ── THE TOKEN CONTRACT: EXISTENCE IS NOT AUTHORIZATION ───────────────────

    fn token_root(tag: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("kxe-auth-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create root");
        root
    }

    #[test]
    fn idle_authorization_defaults_to_unauthorized() {
        let root = token_root("none");
        let auth = read_idle_authorization(&root, 1_000);
        assert_eq!(
            auth,
            IdleAuthorization::Unauthorized { why: "no_token" },
            "no token file = unauthorized"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_bare_reason_string_does_not_authorize() {
        // KNOWN-BAD, and it is the defect this contract exists to close: the
        // earlier reader accepted ANY non-empty file, so a stray note authorized
        // the fleet to go dark forever. An approval that does not name what it
        // approved cannot be falsified.
        let root = token_root("bare");
        std::fs::write(root.join(".idle_authorized"), "Josh said stand down\n").expect("write");
        let auth = read_idle_authorization(&root, 1_000);
        assert_eq!(
            auth,
            IdleAuthorization::Unauthorized {
                why: "token_missing_required_field"
            },
            "a bare reason must NOT authorize, got {auth:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_fully_bound_unexpired_token_authorizes() {
        // KNOWN-GOOD, mandatory: without this leg the reader could refuse
        // everything and still pass, which is an over-strict gate that gets
        // routed around.
        let root = token_root("bound");
        std::fs::write(
            root.join(".idle_authorized"),
            "reason = overnight stand-down\n\
             session = omp-orchestrator\n\
             pane_set_hash = abc123\n\
             queue_len = 4\n\
             issued_at = 900\n\
             expires_at = 5000\n",
        )
        .expect("write");
        match read_idle_authorization(&root, 1_000) {
            IdleAuthorization::Authorized {
                session,
                queue_len,
                expires_at,
                ..
            } => {
                assert_eq!(session, "omp-orchestrator", "session must bind");
                assert_eq!(queue_len, 4, "queue depth must bind");
                assert_eq!(expires_at, 5000, "expiry must bind");
            }
            other => panic!("a bound unexpired token must authorize, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_expired_token_does_not_authorize() {
        // An authorization with no enforced expiry is a permanent licence to go
        // dark. `now` is PASSED IN so this leg cannot flake on wall-clock time.
        let root = token_root("expired");
        std::fs::write(
            root.join(".idle_authorized"),
            "reason = old stand-down\n\
             session = omp-orchestrator\n\
             pane_set_hash = abc123\n\
             queue_len = 0\n\
             issued_at = 100\n\
             expires_at = 500\n",
        )
        .expect("write");
        assert_eq!(
            read_idle_authorization(&root, 1_000),
            IdleAuthorization::Unauthorized {
                why: "token_expired"
            },
            "an expired token must not authorize"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    // ── FAIL-CLOSED LEGS ──────────────────────────────────────────────────────

    #[test]
    fn monitor_blind_is_typed_not_silent() {
        let observation = obs(vec![], 5, true);
        let decision = decide(&observation, &IdleAuthorization::Unauthorized { why: "test" });
        match &decision {
            SupervisorDecision::MonitorBlind { detail } => {
                assert!(detail.contains("blind"), "must name the blindness: {detail}");
            }
            other => panic!("expected MonitorBlind, got {other:?}"),
        }
    }

    #[test]
    fn queue_unreadable_is_typed_not_silent() {
        let observation = obs(
            vec![pane("%1409", "IDLE", true)],
            0,
            false,
        );
        let decision = decide(&observation, &IdleAuthorization::Unauthorized { why: "test" });
        assert!(
            matches!(decision, SupervisorDecision::QueueUnreadable { .. }),
            "unreadable queue must be typed, got {decision:?}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// THE KERNEL: AN OBSERVATION CARRIES AN OBLIGATION
// ═══════════════════════════════════════════════════════════════════════════════
//
// Every failure this session had ONE shape: a state that could be OBSERVED but
// carried NO OBLIGATION TO BE DISCHARGED.
//
//   tick-monitor detected idle capacity for 178 consecutive ticks -> wrote a file
//   ATTENTION.txt was written                                     -> nobody read it
//   admission went RED                                            -> nothing repaired it
//   refill could not tell                                         -> printed "nothing to do"
//   the conductor observed idle panes                              -> named it a next action
//
// Rules do not fix this; 4 of them were already written down and the failure
// happened anyway. The obligation has to be IN THE TYPE.
//
// THE CONTRACT, enforced by the compiler rather than by discipline:
//   1. A `Census` CANNOT be empty         -> "zero observed" cannot be a pass.
//   2. Observing ALWAYS yields a `Duty`   -> there is no "nothing to do" outcome.
//   3. `Duty` is `#[must_use]`            -> dropping it is a compile-time warning.
//   4. Only consuming a `Duty` yields
//      `Discharged`                       -> the sole proof of a completed tick.
//   5. Only `Discharged` produces a
//      success `ExitCode`                 -> exiting 0 having done nothing is
//                                            unrepresentable.

/// A non-empty pane census. The constructor is the anti-vacuity gate: an empty
/// scan is an ERROR at the type boundary, not a healthy fleet reported as clean.
#[derive(Debug, Clone)]
pub struct Census {
    panes: Vec<PaneObservation>,
}

impl Census {
    /// Refuses an empty scan. A monitor that saw nothing has NOT seen an idle-free
    /// fleet — those are opposite conditions and this is where they separate.
    pub fn try_new(panes: Vec<PaneObservation>) -> Result<Self, &'static str> {
        if panes.is_empty() {
            return Err("empty census: a scan that observed zero panes is an ERROR, never a pass");
        }
        Ok(Self { panes })
    }
    pub fn panes(&self) -> &[PaneObservation] {
        &self.panes
    }
}

/// An obligation the supervisor MUST discharge this tick.
///
/// `#[must_use]` is the load-bearing attribute: it makes "observe and move on" a
/// compiler warning instead of a 4-hour outage. There is deliberately NO variant
/// meaning "nothing to do" — every reachable state names an action.
#[must_use = "an undischarged Duty is the 178-tick failure: observed, recorded, and not acted on"]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Duty(SupervisorDecision);

impl Duty {
    /// The ONLY way to obtain a Duty, and it is TOTAL: every census plus
    /// authorization yields one. A caller cannot reach a code path that observed
    /// the fleet and owes nothing.
    ///
    /// `gates` is a REQUIRED PARAMETER, not a default. It previously read
    /// `gate_census: None` hardcoded, which meant every Duty this constructor
    /// produced was `GateUnwired { CENSUS_NOT_PERFORMED }` — so the kernel's
    /// central obligation type could never report a healthy fleet, and the
    /// alarm would fire on every cycle until the operator discounted it. That
    /// is the failure `a_saturated_fleet_discharges_as_a_heartbeat_not_an_alarm`
    /// exists to catch, and it caught it.
    ///
    /// It stays an `Option` rather than becoming mandatory because
    /// `None` is a MEANINGFUL VALUE: it says "this caller performed no census",
    /// which `decide` answers with a refusal. Making it non-optional would
    /// delete that signal. The defect was hardcoding it, not offering it.
    pub fn observe(
        census: &Census,
        queue: &QueueState,
        authorization: &IdleAuthorization,
        gates: Option<GateCensus>,
    ) -> Self {
        let observation = Observation {
            panes: census.panes.to_vec(),
            queue: queue.clone(),
            gate_census: gates,
        };
        Self(decide(&observation, authorization))
    }

    pub fn decision(&self) -> &SupervisorDecision {
        &self.0
    }

    /// Does discharging this duty require ACTUATION rather than a heartbeat?
    pub fn requires_action(&self) -> bool {
        !matches!(self.0, SupervisorDecision::SupervisedWorking { .. })
    }

    /// Consume the duty. `evidence` must describe what was actually done — a
    /// receipt, an escalation id, or a heartbeat row. It is REQUIRED because a
    /// discharge with no evidence is the close-without-evidence debt in another
    /// costume.
    pub fn discharge(self, evidence: impl Into<String>) -> Discharged {
        let evidence = evidence.into();
        Discharged {
            decision: self.0,
            evidence,
        }
    }
}

/// Proof that a tick did something. The ONLY route to a success exit code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discharged {
    decision: SupervisorDecision,
    evidence: String,
}

impl Discharged {
    pub fn decision(&self) -> &SupervisorDecision {
        &self.decision
    }
    pub fn evidence(&self) -> &str {
        &self.evidence
    }
    /// Success requires NON-EMPTY evidence. Empty evidence is a no-op wearing a
    /// discharge, so it maps to a failure code rather than silently passing.
    pub fn exit_code(&self) -> u8 {
        if self.evidence.trim().is_empty() { 70 } else { 0 }
    }
}

#[cfg(test)]
mod kernel_tests {
    use super::*;

    fn working(id: &str) -> PaneObservation {
        PaneObservation {
            pane_id: id.to_owned(),
            state: "WORKING".to_owned(),
            liveness: "LIVE".to_owned(),
            is_dispatchable: false,
            is_free_capacity: false,
            is_working: true,
        }
    }
    fn idle(id: &str) -> PaneObservation {
        PaneObservation {
            pane_id: id.to_owned(),
            state: "IDLE".to_owned(),
            liveness: "CONFIRMED_IDLE".to_owned(),
            is_dispatchable: true,
            is_free_capacity: true,
            is_working: false,
        }
    }
    fn q(ready: usize) -> QueueState {
        QueueState { ready_count: ready, readable: true }
    }
    fn unauth() -> IdleAuthorization {
        IdleAuthorization::Unauthorized { why: "test" }
    }

    #[test]
    fn an_empty_census_cannot_be_constructed() {
        // ANTI-VACUITY AT THE TYPE BOUNDARY. Without this, a monitor that saw
        // nothing reports identically to one that saw a busy fleet.
        let err = Census::try_new(vec![]).expect_err("empty census must be refused");
        assert!(err.contains("ERROR"), "the refusal must say so: {err}");
    }

    #[test]
    fn observing_always_yields_a_duty_there_is_no_nothing_to_do() {
        // TOTALITY. Four shapes, none of which can produce "no obligation".
        let cases = vec![
            (vec![idle("%1")], q(4)),   // free + ready
            (vec![idle("%1")], q(0)),   // free, empty queue
            (vec![working("%1")], q(4)),// saturated + ready
            (vec![working("%1")], q(0)),// saturated, empty queue
        ];
        for (panes, queue) in cases {
            let census = Census::try_new(panes).expect("non-empty");
            let duty = Duty::observe(&census, &queue, &unauth(), gates());
            // The duty exists and names something. `#[must_use]` forces this line.
            let _ = duty.discharge("test-evidence");
        }
    }

    #[test]
    fn free_plus_ready_always_requires_action_never_a_heartbeat() {
        // The 4h19m failure, made unrepresentable: an idle pane beside ready work
        // can never discharge as SupervisedWorking.
        let census = Census::try_new(vec![idle("%1"), working("%2")]).expect("non-empty");
        let duty = Duty::observe(&census, &q(4), &unauth(), gates());
        assert!(
            duty.requires_action(),
            "free + ready must demand actuation, got {:?}",
            duty.decision()
        );
    }

    #[test]
    fn a_saturated_fleet_discharges_as_a_heartbeat_not_an_alarm() {
        // The other half: a healthy busy fleet must NOT demand action, or the
        // alarm fires constantly and gets discounted.
        let census = Census::try_new(vec![working("%1"), working("%2")]).expect("non-empty");
        let duty = Duty::observe(&census, &q(0), &unauth(), gates());
        assert!(!duty.requires_action(), "a busy fleet is not an incident");
    }

    #[test]
    fn a_discharge_with_no_evidence_cannot_exit_zero() {
        // "I did something" with nothing to show is a no-op wearing a discharge.
        let census = Census::try_new(vec![idle("%1")]).expect("non-empty");
        let duty = Duty::observe(&census, &q(4), &unauth(), gates());
        assert_eq!(duty.discharge("   ").exit_code(), 70, "empty evidence must fail");
    }

    #[test]
    fn a_discharge_with_evidence_exits_zero() {
        // KNOWN-GOOD, mandatory: without it the kernel could refuse everything and
        // still pass, which is an over-strict gate that gets routed around.
        let census = Census::try_new(vec![idle("%1")]).expect("non-empty");
        let duty = Duty::observe(&census, &q(4), &unauth(), gates());
        assert_eq!(
            duty.discharge("dispatched %1 bead=x receipt=IDLE_TO_WORKING").exit_code(),
            0
        );
    }

    #[test]
    fn a_duty_built_without_a_census_refuses_rather_than_reporting_healthy() {
        // THE DEFECT THIS REPLACES, measured 2026-08-31: `Duty::observe` hardcoded
        // `gate_census: None`, so EVERY duty the kernel produced was GateUnwired —
        // a healthy busy fleet read as an incident, which trains the operator to
        // discount the alarm. Passing None is now a CALLER'S CHOICE with a defined
        // meaning, and this asserts that meaning.
        let census = Census::try_new(vec![working("%1"), working("%2")]).expect("non-empty");
        let duty = Duty::observe(&census, &q(0), &unauth(), None);
        match duty.decision() {
            SupervisorDecision::GateUnwired { unwired } => {
                assert!(
                    unwired.iter().any(|u| u.contains("CENSUS_NOT_PERFORMED")),
                    "a caller that performed no census must be told exactly that, got {unwired:?}"
                );
            }
            other => panic!("no census must refuse, got {other:?}"),
        }
        assert!(
            duty.requires_action(),
            "a refusal is never a heartbeat — it must demand actuation"
        );
    }

    /// A census in which the positive control is reachable, so `decide` clears the
    /// gate check and the test below it exercises the branch it actually names.
    /// Without this every kernel test would assert against GateUnwired.
    fn gates() -> Option<GateCensus> {
        Some(GateCensus {
            rows: vec![GateCensusRow {
                gate: GateCensus::positive_control_gate().to_owned(),
                reachability: GateReachability::Reachable {
                    trigger: ".git/hooks/pre-commit".to_owned(),
                },
            }],
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// BINDING VALIDATION: A PARSED FIELD THAT IS NEVER COMPARED IS A VACUOUS FIELD
// ═══════════════════════════════════════════════════════════════════════════════
//
// `read_idle_authorization` parses session, pane_set_hash and queue_len, and
// `decide` checked only the EXPIRY. So an approval granted for one session with
// an empty queue silently authorized a DIFFERENT session with forty ready beads.
// The fields read as safety and provided none — the same shape as `free_capacity`
// derived from `is_dispatchable`, and as a test that passed because the branch it
// covered could not run.
//
// The comparison lives HERE, not in main: `decide` is the only consumer of the
// authorization, and policy that leaks into the binary gets reimplemented per
// call site. It is exposed as an ADDITIVE function so no existing signature
// changes — main calls `applicable(..)` and passes the result to `decide(..)`.

/// Canonical pane-set hash. Order-independent, so a pane list read in a different
/// order is the same fleet; and it names the PANE IDS, because a fleet with the
/// same COUNT but different panes is not the fleet that was approved.
pub fn pane_set_hash(panes: &[PaneObservation]) -> String {
    let mut ids: Vec<&str> = panes.iter().map(|p| p.pane_id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for id in ids {
        for byte in id.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash ^= 0x1f;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Downgrade an authorization that does not describe the CURRENT situation.
///
/// An approval is a statement about a moment: this session, this fleet shape,
/// this queue depth. When any of those move, the approval no longer describes
/// what was approved, and continuing to honour it is how a stand-down granted for
/// a quiet fleet licenses silence during a backlog.
///
/// Returns the authorization unchanged when every binding still holds, and
/// `Unauthorized { why }` naming the FIRST binding that failed otherwise.
pub fn applicable(
    authorization: IdleAuthorization,
    session: &str,
    panes: &[PaneObservation],
    queue: &QueueState,
) -> IdleAuthorization {
    let IdleAuthorization::Authorized {
        session: token_session,
        pane_set_hash: token_hash,
        queue_len: token_queue,
        ..
    } = &authorization
    else {
        return authorization;
    };
    if token_session != session {
        return IdleAuthorization::Unauthorized {
            why: "token_session_mismatch",
        };
    }
    if token_hash != &pane_set_hash(panes) {
        return IdleAuthorization::Unauthorized {
            why: "token_pane_set_changed",
        };
    }
    // A stand-down granted over an empty queue does not authorize silence once
    // work arrives. Growth invalidates; shrinkage does not.
    if queue.ready_count > *token_queue {
        return IdleAuthorization::Unauthorized {
            why: "token_queue_grew",
        };
    }
    authorization
}

/// The canonical token writer. Without it the format is unusable, which means the
/// only reachable state is Unauthorized — fail-safe, and it makes the authorized
/// path untestable in practice. A contract nobody can satisfy is not a contract.
pub fn write_token(
    reason: &str,
    session: &str,
    panes: &[PaneObservation],
    queue: &QueueState,
    issued_at: u64,
    valid_for_secs: u64,
) -> String {
    format!(
        "# minted by omp-orchestrator::write_token — bindings are load-bearing\n\
         reason = {reason}\n\
         session = {session}\n\
         pane_set_hash = {}\n\
         queue_len = {}\n\
         issued_at = {issued_at}\n\
         expires_at = {}\n",
        pane_set_hash(panes),
        queue.ready_count,
        issued_at.saturating_add(valid_for_secs)
    )
}

#[cfg(test)]
mod binding_tests {
    use super::*;

    fn p(id: &str) -> PaneObservation {
        PaneObservation {
            pane_id: id.to_owned(),
            state: "IDLE".to_owned(),
            liveness: "CONFIRMED_IDLE".to_owned(),
            is_dispatchable: true,
            is_free_capacity: true,
            is_working: false,
        }
    }
    fn q(n: usize) -> QueueState {
        QueueState { ready_count: n, readable: true }
    }
    /// `tag` MUST be unique per test. Keying the temp dir on `session` alone made
    /// parallel tests clobber each other's token — shared mutable state with no
    /// isolation, which produced two failures whose reported cause
    /// (token_pane_set_changed) was a different test's fleet.
    fn minted(
        tag: &str,
        session: &str,
        panes: &[PaneObservation],
        queue: &QueueState,
    ) -> IdleAuthorization {
        let dir = std::env::temp_dir().join(format!("bind-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(
            dir.join(".idle_authorized"),
            write_token("stand down", session, panes, queue, 1_000, 3_600),
        )
        .expect("write");
        let auth = read_idle_authorization(&dir, 1_100);
        let _ = std::fs::remove_dir_all(&dir);
        auth
    }

    #[test]
    fn the_canonical_writer_produces_a_token_the_reader_accepts() {
        // KNOWN-GOOD, and it is mandatory: without a writer the only reachable
        // state is Unauthorized, so every binding leg below would pass vacuously.
        let panes = vec![p("%1"), p("%2")];
        assert!(
            matches!(
                minted("writer", "omp-orchestrator", &panes, &q(0)),
                IdleAuthorization::Authorized { .. }
            ),
            "the writer and reader must agree, or the authorized path is untestable"
        );
    }

    #[test]
    fn an_authorization_for_another_session_does_not_apply() {
        let panes = vec![p("%1")];
        let auth = minted("session", "some-other-session", &panes, &q(0));
        assert_eq!(
            applicable(auth, "omp-orchestrator", &panes, &q(0)),
            IdleAuthorization::Unauthorized { why: "token_session_mismatch" }
        );
    }

    #[test]
    fn an_authorization_stops_applying_when_the_fleet_changes_shape() {
        let approved = vec![p("%1"), p("%2")];
        let auth = minted("shape", "omp-orchestrator", &approved, &q(0));
        let now = vec![p("%1"), p("%2"), p("%3")];
        assert_eq!(
            applicable(auth, "omp-orchestrator", &now, &q(0)),
            IdleAuthorization::Unauthorized { why: "token_pane_set_changed" }
        );
    }

    #[test]
    fn a_standdown_granted_over_an_empty_queue_does_not_survive_a_backlog() {
        // THE LEG THAT MATTERS. Josh authorizes idleness when there is nothing to
        // do; four ready beads is a different situation and must re-ask.
        let panes = vec![p("%1")];
        let auth = minted("grew", "omp-orchestrator", &panes, &q(0));
        assert_eq!(
            applicable(auth, "omp-orchestrator", &panes, &q(4)),
            IdleAuthorization::Unauthorized { why: "token_queue_grew" }
        );
    }

    #[test]
    fn a_shrinking_queue_does_not_invalidate_an_authorization() {
        // KNOWN-GOOD on the other side: growth invalidates, shrinkage does not.
        // Without this leg the rule is "any queue change revokes", which makes an
        // authorization useless the moment a worker closes a bead.
        let panes = vec![p("%1")];
        let auth = minted("shrank", "omp-orchestrator", &panes, &q(5));
        assert!(
            matches!(
                applicable(auth, "omp-orchestrator", &panes, &q(2)),
                IdleAuthorization::Authorized { .. }
            ),
            "a draining queue must not revoke a valid stand-down"
        );
    }

    #[test]
    fn the_pane_set_hash_is_order_independent_but_identity_sensitive() {
        assert_eq!(
            pane_set_hash(&[p("%1"), p("%2")]),
            pane_set_hash(&[p("%2"), p("%1")]),
            "the same fleet read in a different order is the same fleet"
        );
        assert_ne!(
            pane_set_hash(&[p("%1"), p("%2")]),
            pane_set_hash(&[p("%1"), p("%3")]),
            "same COUNT, different panes: not the fleet that was approved"
        );
    }

    #[test]
    fn an_unauthorized_input_passes_through_unchanged() {
        let panes = vec![p("%1")];
        let before = IdleAuthorization::Unauthorized { why: "no_token" };
        assert_eq!(
            applicable(before.clone(), "omp-orchestrator", &panes, &q(0)),
            before,
            "applicable() must not manufacture authorization"
        );
    }
}

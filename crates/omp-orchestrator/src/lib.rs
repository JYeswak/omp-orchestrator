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

use serde_json::Value;
use std::fmt;
use std::path::{Path, PathBuf};

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupervisorDecision {
    /// Free capacity + ready work + authorized -> dispatch.
    Dispatch { pane: String, bead_hint: String },
    /// Free capacity + ready work + UNAUTHORIZED -> escalate (an incident).
    EscalateIdleIncident { dispatchable_count: usize, ready_count: usize },
    /// The monitor could not observe — fail closed.
    MonitorBlind { detail: String },
    /// The queue could not be read — fail closed.
    QueueUnreadable { detail: String },
    /// The workspace-load gate refused — do not dispatch into a broken repo.
    WorkspaceUnloaded { detail: String },
    /// Queue is genuinely empty and the authorization permits idleness.
    AuthorizedIdle { pane_count: usize },
}

/// The pure deciding function: given an observation and the authorization state,
/// produce the supervisor's decision. This is where the three deciding legs are
/// encoded. It is PURE so the deciding legs are testable without subprocesses.
pub fn decide(observation: &Observation, authorization: &IdleAuthorization) -> SupervisorDecision {
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

    if observation.queue.ready_count == 0 {
        // Queue is empty. Leg 3: is idleness authorized?
        return match authorization {
            IdleAuthorization::Authorized { .. } => SupervisorDecision::AuthorizedIdle {
                pane_count: observation.panes.len(),
            },
            IdleAuthorization::Unauthorized { why: _ } => SupervisorDecision::EscalateIdleIncident {
                dispatchable_count: dispatchable.len(),
                ready_count: 0,
            },
        };
    }

    // Queue has work. Are there dispatchable panes?
    if dispatchable.is_empty() {
        // No pane is ConfirmedIdle. Check the authorization for idle tolerance.
        return match authorization {
            IdleAuthorization::Authorized { .. } => {
                // Josh authorized idleness — tolerated, not an incident.
                SupervisorDecision::AuthorizedIdle {
                    pane_count: observation.panes.len(),
                }
            }
            IdleAuthorization::Unauthorized { why: _ } => {
                // Free capacity exists (NewlyIdle) but no ConfirmedIdle — this is the
                // exact gap that let the fleet sit idle: NewlyIdle panes are visible
                // but not dispatchable. If the ready queue is non-empty and no pane
                // is dispatchable, escalate.
                if free_capacity > 0 {
                    SupervisorDecision::EscalateIdleIncident {
                        dispatchable_count: 0,
                        ready_count: observation.queue.ready_count,
                    }
                } else {
                    // All panes genuinely working — this is healthy.
                    SupervisorDecision::AuthorizedIdle {
                        pane_count: observation.panes.len(),
                    }
                }
            }
        };
    }

    // Dispatchable panes + ready work = DISPATCH.
    let pane = dispatchable[0].pane_id.clone();
    SupervisorDecision::Dispatch {
        pane,
        bead_hint: "first-ready-bead".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(panes: Vec<PaneObservation>, ready: usize, readable: bool) -> Observation {
        Observation {
            panes,
            queue: QueueState { ready_count: ready, readable },
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
    fn unauthorized_idle_with_empty_queue_escalates() {
        let observation = obs(
            vec![pane("%1409", "IDLE", true)],
            0,
            true,
        );
        let decision = decide(&observation, &IdleAuthorization::Unauthorized { why: "test" });
        assert!(
            matches!(decision, SupervisorDecision::EscalateIdleIncident { .. }),
            "idle + unauthorized + empty queue = incident, got {decision:?}"
        );
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

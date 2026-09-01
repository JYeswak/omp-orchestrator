//! Typed consumption of NTM's native agent-plugin activity response.
//!
//! NTM owns the session and the send operation. This module owns only the
//! boundary between NTM's JSON observation and our fail-closed wave facts.
//! In particular, `safe_to_dispatch` is never trusted by itself: a live OMP
//! pane whose structured state conflicts with NTM's TUI observation remains
//! non-dispatchable.

use serde_json::Value;
use std::fmt;

/// Native OMP plugin variants registered by NTM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OmpVariant {
    /// The generic `omp` plugin, whose configured profile is external to NTM.
    Generic,
    /// `omp-claude`, the Claude OAuth-profile preset.
    Claude,
    /// `omp-grok`, the Grok OAuth-profile preset.
    Grok,
}

impl OmpVariant {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Generic => "omp",
            Self::Claude => "omp-claude",
            Self::Grok => "omp-grok",
        }
    }
}

/// Agent type reported by NTM. Unknown plugin names are retained but are never
/// dispatchable; an upstream plugin addition cannot silently become trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentKind {
    Omp(OmpVariant),
    Unknown(String),
}

impl AgentKind {
    fn parse(name: &str) -> Self {
        match name {
            "omp" => Self::Omp(OmpVariant::Generic),
            "omp-claude" => Self::Omp(OmpVariant::Claude),
            "omp-grok" => Self::Omp(OmpVariant::Grok),
            other => Self::Unknown(other.to_owned()),
        }
    }

    pub const fn is_omp(&self) -> bool {
        matches!(self, Self::Omp(_))
    }

    /// True for known OMP plugins and future OMP-named plugins. Unknown OMP
    /// names must not fall through to the legacy safe-pane classifier.
    pub fn is_omp_family(&self) -> bool {
        match self {
            Self::Omp(_) => true,
            Self::Unknown(name) => name == "omp" || name.starts_with("omp-"),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Omp(v) => v.as_str(),
            Self::Unknown(v) => v,
        }
    }
}

/// The normalized state of one NTM activity signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalState {
    Idle,
    Working,
    Error,
    Unknown,
}

/// The state used for a dispatch decision after comparing NTM's state fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Readiness {
    Idle,
    Working,
    Error,
    Conflicting,
    Unknown,
}

impl Readiness {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Working => "working",
            Self::Error => "error",
            Self::Conflicting => "conflicting",
            Self::Unknown => "unknown",
        }
    }
}

/// Freshness of the evidence that came from NTM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceFreshness {
    /// Both live-capture fields were present and said live/fresh.
    Live,
    /// A source explicitly reported stale data.
    Stale,
    /// Provenance or freshness was absent or unrecognized.
    Missing,
}

impl EvidenceFreshness {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Stale => "stale",
            Self::Missing => "missing",
        }
    }
}

/// One agent row from `ntm --robot-activity=<session>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentObservation {
    pub pane: String,
    pub kind: AgentKind,
    pub state: SignalState,
    pub observation_state: SignalState,
    pub readiness: Readiness,
    /// Raw NTM hint. It is only one input to [`Self::dispatchable`].
    pub safe_to_dispatch: bool,
    pub freshness: EvidenceFreshness,
    /// Producer timestamp for the per-agent capture, when supplied by NTM.
    pub capture_collected_at: Option<String>,
    /// Raw producer confidence, retained without floating-point equality loss.
    pub observation_confidence: Option<String>,
}

impl AgentObservation {
    /// True only when all independent facts agree on a fresh, idle OMP pane.
    /// A missing field, stale source, state conflict, error, or unknown plugin
    /// is conservative and returns false.
    pub fn dispatchable(&self) -> bool {
        self.kind.is_omp()
            && self.readiness == Readiness::Idle
            && self.freshness == EvidenceFreshness::Live
            && self.safe_to_dispatch
    }

    /// True when this OMP row may be handed to the independent pane-capture
    /// liveness gate. NTM currently reports an idle OMP pane with `state=UNKNOWN`
    /// in some healthy snapshots, so this is deliberately weaker than
    /// [`Self::dispatchable`]: it never authorizes a send on its own.
    pub fn capture_eligible(&self) -> bool {
        self.kind.is_omp()
            && self.observation_state == SignalState::Idle
            && !matches!(self.readiness, Readiness::Error | Readiness::Conflicting)
            && self.freshness == EvidenceFreshness::Live
            && self.safe_to_dispatch
    }
}

/// A non-empty NTM activity response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivitySnapshot {
    pub agents: Vec<AgentObservation>,
}

impl ActivitySnapshot {
    pub fn omp_agents(&self) -> impl Iterator<Item = &AgentObservation> {
        self.agents.iter().filter(|agent| agent.kind.is_omp())
    }

    pub fn dispatchable_omp_count(&self) -> usize {
        self.omp_agents()
            .filter(|agent| agent.dispatchable())
            .count()
    }
}

/// Why NTM activity could not become a typed snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityError {
    InvalidJson(String),
    RootNotObject,
    NtmReportedFailure(String),
    MissingField(&'static str),
    AgentsNotArray,
    EmptyAgents,
    AgentNotObject,
    PaneNotString,
}

impl fmt::Display for ActivityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(detail) => write!(f, "invalid NTM JSON: {detail}"),
            Self::RootNotObject => f.write_str("NTM response root is not an object"),
            Self::NtmReportedFailure(detail) => write!(f, "NTM reported failure: {detail}"),
            Self::MissingField(name) => write!(f, "NTM response missing field: {name}"),
            Self::AgentsNotArray => f.write_str("NTM response agents is not an array"),
            Self::EmptyAgents => {
                f.write_str("NTM response agents is empty; observation is unknown")
            }
            Self::AgentNotObject => f.write_str("NTM response contains a non-object agent"),
            Self::PaneNotString => f.write_str("NTM response pane is not a string"),
        }
    }
}

fn signal_state(value: Option<&Value>) -> SignalState {
    let Some(value) = value.and_then(Value::as_str) else {
        return SignalState::Unknown;
    };
    match value.to_ascii_lowercase().as_str() {
        "idle" | "ready" => SignalState::Idle,
        "busy" | "working" | "thinking" => SignalState::Working,
        "error" | "failed" => SignalState::Error,
        _ => SignalState::Unknown,
    }
}

fn source_health_stale(root: &serde_json::Map<String, Value>) -> bool {
    let Some(source_health) = root.get("source_health") else {
        return false;
    };
    let Some(sources) = source_health.as_object() else {
        return true;
    };
    !sources.is_empty()
        && sources.values().any(|source| {
            let status = source
                .get("status")
                .and_then(Value::as_str)
                .map(str::to_ascii_lowercase);
            !matches!(status.as_deref(), Some("fresh") | Some("live"))
        })
}

fn freshness(agent: &Value, source_health_is_stale: bool) -> EvidenceFreshness {
    if source_health_is_stale {
        return EvidenceFreshness::Stale;
    }
    let provenance = agent
        .get("capture_provenance")
        .and_then(Value::as_str)
        .map(str::to_ascii_lowercase);
    let freshness = agent
        .get("observation_freshness")
        .and_then(Value::as_str)
        .map(str::to_ascii_lowercase);
    match (provenance.as_deref(), freshness.as_deref()) {
        (Some("live"), Some("fresh")) => EvidenceFreshness::Live,
        (Some("stale"), _) | (_, Some("stale")) => EvidenceFreshness::Stale,
        _ => EvidenceFreshness::Missing,
    }
}

fn readiness(state: SignalState, observation_state: SignalState) -> Readiness {
    if state == SignalState::Error || observation_state == SignalState::Error {
        Readiness::Error
    } else if state == SignalState::Unknown || observation_state == SignalState::Unknown {
        Readiness::Unknown
    } else if state != observation_state {
        Readiness::Conflicting
    } else {
        match state {
            SignalState::Idle => Readiness::Idle,
            SignalState::Working => Readiness::Working,
            SignalState::Error | SignalState::Unknown => Readiness::Unknown,
        }
    }
}

/// Parse the exact machine-readable response emitted by NTM's activity robot.
/// Empty agent sets are an error rather than a healthy zero, preventing a
/// broken probe from authorizing a dispatch.
pub fn parse_activity_json(input: &str) -> Result<ActivitySnapshot, ActivityError> {
    let root: Value = serde_json::from_str(input)
        .map_err(|error| ActivityError::InvalidJson(error.to_string()))?;
    let root = root.as_object().ok_or(ActivityError::RootNotObject)?;
    if root.get("success").and_then(Value::as_bool) != Some(true) {
        let detail = root
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("success was not true");
        return Err(ActivityError::NtmReportedFailure(detail.to_owned()));
    }
    let agents = root
        .get("agents")
        .or_else(|| root.get("panes"))
        .ok_or(ActivityError::MissingField("agents"))?
        .as_array()
        .ok_or(ActivityError::AgentsNotArray)?;
    if agents.is_empty() {
        return Err(ActivityError::EmptyAgents);
    }

    let source_health_is_stale = source_health_stale(root);
    let mut observations = Vec::with_capacity(agents.len());
    for agent in agents {
        let agent = agent.as_object().ok_or(ActivityError::AgentNotObject)?;
        let pane = match agent.get("pane") {
            Some(Value::String(value)) => value.clone(),
            Some(Value::Number(value)) => value.to_string(),
            Some(_) => return Err(ActivityError::PaneNotString),
            None => return Err(ActivityError::MissingField("agents[].pane")),
        };
        let kind_name = agent
            .get("agent_type")
            .or_else(|| agent.get("agent"))
            .and_then(Value::as_str)
            .ok_or(ActivityError::MissingField("agents[].agent_type"))?;
        let state = signal_state(agent.get("state"));
        let observation_state = signal_state(agent.get("observation_state"));
        observations.push(AgentObservation {
            pane,
            kind: AgentKind::parse(kind_name),
            state,
            observation_state,
            readiness: readiness(state, observation_state),
            safe_to_dispatch: agent
                .get("safe_to_dispatch")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            freshness: freshness(&Value::Object(agent.clone()), source_health_is_stale),
            capture_collected_at: agent
                .get("capture_collected_at")
                .and_then(Value::as_str)
                .map(str::to_owned),
            observation_confidence: agent.get("observation_confidence").map(Value::to_string),
        });
    }
    Ok(ActivitySnapshot {
        agents: observations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(agent_type: &str, state: &str, observation_state: &str) -> String {
        format!(
            r#"{{"pane":"3","agent_type":"{agent_type}","state":"{state}","observation_state":"{observation_state}","safe_to_dispatch":true,"capture_provenance":"live","observation_freshness":"fresh"}}"#
        )
    }

    fn response(rows: &[String]) -> String {
        format!(r#"{{"success":true,"agents":[{}]}}"#, rows.join(","))
    }

    #[test]
    fn all_native_omp_variants_are_typed() {
        for (name, expected) in [
            ("omp", OmpVariant::Generic),
            ("omp-claude", OmpVariant::Claude),
            ("omp-grok", OmpVariant::Grok),
        ] {
            let snapshot = parse_activity_json(&response(&[row(name, "IDLE", "idle")])).unwrap();
            assert_eq!(snapshot.agents[0].kind, AgentKind::Omp(expected));
        }
    }

    #[test]
    fn fresh_agreeing_idle_omp_is_dispatchable_observation() {
        let snapshot =
            parse_activity_json(&response(&[row("omp-claude", "IDLE", "idle")])).unwrap();
        let agent = &snapshot.agents[0];
        assert_eq!(agent.readiness, Readiness::Idle);
        assert!(agent.dispatchable());
        assert_eq!(snapshot.dispatchable_omp_count(), 1);
    }

    #[test]
    fn thinking_and_idle_conflict_is_not_dispatchable() {
        let snapshot = parse_activity_json(&response(&[row("omp", "THINKING", "idle")])).unwrap();
        let agent = &snapshot.agents[0];
        assert_eq!(agent.readiness, Readiness::Conflicting);
        assert!(!agent.dispatchable());
    }

    #[test]
    fn error_stale_and_unknown_inputs_fail_closed() {
        for (state, observed, expected) in [
            ("ERROR", "idle", Readiness::Error),
            ("IDLE", "idle", Readiness::Idle),
            ("MYSTERY", "idle", Readiness::Unknown),
        ] {
            let mut text = response(&[row("omp-grok", state, observed)]);
            if state == "IDLE" {
                text = text.replace(
                    "\"capture_provenance\":\"live\"",
                    "\"capture_provenance\":\"stale\"",
                );
            }
            let snapshot = parse_activity_json(&text).unwrap();
            let agent = &snapshot.agents[0];
            assert_eq!(agent.readiness, expected);
            assert!(!agent.dispatchable());
        }

        let snapshot =
            parse_activity_json(&response(&[row("omp-future", "IDLE", "idle")])).unwrap();
        assert!(matches!(snapshot.agents[0].kind, AgentKind::Unknown(_)));
        assert!(!snapshot.agents[0].dispatchable());
    }

    #[test]
    fn stale_source_health_overrides_fresh_agent_evidence() {
        let snapshot = parse_activity_json(
            r#"{"success":true,"source_health":{"tmux":{"status":"stale","freshness_sec":9,"stale_after_sec":5}},"agents":[{"pane":"3","agent_type":"omp-claude","state":"IDLE","observation_state":"idle","safe_to_dispatch":true,"capture_provenance":"live","observation_freshness":"fresh"}]}"#,
        )
        .unwrap();
        let agent = &snapshot.agents[0];
        assert_eq!(agent.freshness, EvidenceFreshness::Stale);
        assert!(!agent.dispatchable());
    }

    #[test]
    fn live_capture_metadata_is_preserved() {
        let snapshot = parse_activity_json(
            r#"{"success":true,"source_health":{"tmux":{"status":"fresh"}},"agents":[{"pane":"3","agent_type":"omp-claude","state":"IDLE","observation_state":"idle","safe_to_dispatch":true,"capture_provenance":"live","observation_freshness":"fresh","capture_collected_at":"2026-08-31T03:01:32Z","observation_confidence":0.95}]}"#,
        )
        .unwrap();
        let agent = &snapshot.agents[0];
        assert_eq!(
            agent.capture_collected_at.as_deref(),
            Some("2026-08-31T03:01:32Z")
        );
        assert_eq!(agent.observation_confidence.as_deref(), Some("0.95"));
    }

    #[test]
    fn malformed_or_empty_activity_is_not_a_clean_scan() {
        assert!(matches!(
            parse_activity_json("not-json"),
            Err(ActivityError::InvalidJson(_))
        ));
        assert_eq!(
            parse_activity_json(r#"{"success":true,"agents":[]}"#),
            Err(ActivityError::EmptyAgents)
        );
        assert!(matches!(
            parse_activity_json(r#"{"success":false,"error":"transport closed"}"#),
            Err(ActivityError::NtmReportedFailure(_))
        ));
    }

    #[test]
    fn native_panes_shape_and_numeric_pane_ids_are_typed() {
        let snapshot = parse_activity_json(
            r#"{"success":true,"panes":[{"pane":4,"agent":"omp-grok","state":"IDLE","observation_state":"idle","safe_to_dispatch":true,"capture_provenance":"live","observation_freshness":"fresh"}]}"#,
        )
        .unwrap();
        assert_eq!(snapshot.agents[0].pane, "4");
        assert_eq!(snapshot.agents[0].kind.as_str(), "omp-grok");
        assert!(snapshot.agents[0].dispatchable());
    }
}

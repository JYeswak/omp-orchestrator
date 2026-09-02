#![forbid(unsafe_code)]

//! Live blocker availability from the durable br graph.
//!
//! The graph is authoritative for blocker status. BV's optional `unblocks`
//! summary is preserved as UNKNOWN when absent; it is never coerced to zero.

use asupersync::process::Command;
use asupersync::time::timeout;
use asupersync::Cx;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::time::Duration;
use subprocess_contract::{run_output, RunError};

const BR_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueStatus {
    Open,
    InProgress,
    Grading,
    Blocked,
    Closed,
    Unknown,
}

impl IssueStatus {
    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value {
            "open" => Self::Open,
            "in_progress" => Self::InProgress,
            "grading" => Self::Grading,
            "blocked" => Self::Blocked,
            "closed" => Self::Closed,
            _ => Self::Unknown,
        }
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Closed)
    }

    #[must_use]
    pub const fn is_non_terminal(self) -> bool {
        !self.is_terminal()
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::InProgress => "in_progress",
            Self::Grading => "grading",
            Self::Blocked => "blocked",
            Self::Closed => "closed",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueRecord {
    pub id: String,
    pub status: IssueStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockerEdge {
    pub child_id: String,
    pub blocker_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BlockerState {
    Blocking,
    Released,
    StaleEdge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Availability {
    Available,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Unblocks {
    Known { count: u64 },
    Unknown { reason: String },
}

impl Unblocks {
    #[must_use]
    pub const fn known(count: u64) -> Self {
        Self::Known { count }
    }

    #[must_use]
    pub fn unknown(reason: impl Into<String>) -> Self {
        Self::Unknown {
            reason: reason.into(),
        }
    }

    #[must_use]
    pub const fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockerReport {
    pub blocker_id: String,
    pub blocker_status: IssueStatus,
    pub state: BlockerState,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeadAvailability {
    pub bead_id: String,
    pub status: IssueStatus,
    pub availability: Availability,
    pub blockers: Vec<BlockerReport>,
    pub unblocks: Unblocks,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaleEdge {
    pub child_id: String,
    pub blocker_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphReport {
    pub schema: &'static str,
    pub issues: Vec<BeadAvailability>,
    pub stale_edges: Vec<StaleEdge>,
}

impl GraphReport {
    #[must_use]
    pub fn issue(&self, bead_id: &str) -> Option<&BeadAvailability> {
        self.issues.iter().find(|issue| issue.bead_id == bead_id)
    }

    #[must_use]
    pub fn render_text(&self) -> String {
        let mut output = format!("schema=bead-availability/v1 issues={}\n", self.issues.len());
        for issue in &self.issues {
            output.push_str(&format!(
                "BEAD id={} status={} availability={} unblocks={}\n",
                issue.bead_id,
                issue.status.as_str(),
                match issue.availability {
                    Availability::Available => "AVAILABLE",
                    Availability::Blocked => "BLOCKED",
                    Availability::Unknown => "UNKNOWN",
                },
                render_unblocks(&issue.unblocks),
            ));
            for blocker in &issue.blockers {
                output.push_str(&format!(
                    "  BLOCKER id={} status={} state={} reason={}\n",
                    blocker.blocker_id,
                    blocker.blocker_status.as_str(),
                    match blocker.state {
                        BlockerState::Blocking => "BLOCKING",
                        BlockerState::Released => "RELEASED",
                        BlockerState::StaleEdge => "STALE_EDGE",
                    },
                    blocker.reason,
                ));
            }
        }
        for edge in &self.stale_edges {
            output.push_str(&format!(
                "STALE_EDGE child={} blocker={} reason={}\n",
                edge.child_id, edge.blocker_id, edge.reason
            ));
        }
        output
    }
}

fn render_unblocks(unblocks: &Unblocks) -> String {
    match unblocks {
        Unblocks::Known { count } => count.to_string(),
        Unblocks::Unknown { reason } => format!("UNKNOWN({reason})"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    EmptyIssueSet,
    DuplicateIssue(String),
    MalformedIssues(String),
    MalformedEdges(String),
}

impl fmt::Display for GraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIssueSet => {
                formatter.write_str("EMPTY_ISSUE_SET: br list returned no issues")
            }
            Self::DuplicateIssue(id) => write!(formatter, "DUPLICATE_ISSUE: {id}"),
            Self::MalformedIssues(detail) => write!(formatter, "MALFORMED_ISSUES: {detail}"),
            Self::MalformedEdges(detail) => write!(formatter, "MALFORMED_EDGES: {detail}"),
        }
    }
}

impl std::error::Error for GraphError {}

#[derive(Debug)]
enum LiveError {
    Run { command: String, error: RunError },
    CommandFailed { command: String, detail: String },
    Json { command: String, detail: String },
}

impl fmt::Display for LiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Run { command, error } => {
                write!(formatter, "RUN_FAILED command={command} error={error}")
            }
            Self::CommandFailed { command, detail } => {
                write!(
                    formatter,
                    "COMMAND_FAILED command={command} detail={detail}"
                )
            }
            Self::Json { command, detail } => {
                write!(formatter, "JSON_FAILED command={command} detail={detail}")
            }
        }
    }
}

/// Parse a br list -a JSON envelope. Closed issues are required because their
/// status releases children; the default br list omission is not acceptable.
pub fn parse_br_issues(value: &Value) -> Result<Vec<IssueRecord>, GraphError> {
    let issues = value
        .get("issues")
        .and_then(Value::as_array)
        .ok_or_else(|| GraphError::MalformedIssues("missing issues array".to_owned()))?;
    if issues.is_empty() {
        return Err(GraphError::EmptyIssueSet);
    }
    let mut records = Vec::with_capacity(issues.len());
    let mut ids = BTreeSet::new();
    for issue in issues {
        let id = issue
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.trim().is_empty())
            .ok_or_else(|| GraphError::MalformedIssues("issue has no id".to_owned()))?;
        if !ids.insert(id.to_owned()) {
            return Err(GraphError::DuplicateIssue(id.to_owned()));
        }
        let status = issue
            .get("status")
            .and_then(Value::as_str)
            .map(IssueStatus::parse)
            .unwrap_or(IssueStatus::Unknown);
        records.push(IssueRecord {
            id: id.to_owned(),
            status,
        });
    }
    Ok(records)
}

/// Parse the down direction of br dep list. Only blocks edges are blocker
/// edges; other dependency types remain outside this admission question.
pub fn parse_br_blockers(value: &Value, child_id: &str) -> Result<Vec<BlockerEdge>, GraphError> {
    let edges = value.as_array().ok_or_else(|| {
        GraphError::MalformedEdges(format!("{child_id}: response is not an array"))
    })?;
    let mut result = Vec::new();
    for edge in edges {
        if edge.get("type").and_then(Value::as_str) != Some("blocks") {
            continue;
        }
        let issue_id = edge.get("issue_id").and_then(Value::as_str);
        let depends_on_id = edge.get("depends_on_id").and_then(Value::as_str);
        match (issue_id, depends_on_id) {
            (Some(issue_id), Some(depends_on_id)) if issue_id == child_id => {
                result.push(BlockerEdge {
                    child_id: child_id.to_owned(),
                    blocker_id: depends_on_id.to_owned(),
                });
            }
            _ => {
                return Err(GraphError::MalformedEdges(format!(
                    "{child_id}: malformed blocks edge"
                )));
            }
        }
    }
    Ok(result)
}

/// Preserve an optional BV recommendation field without converting absence to
/// zero. This is intentionally separate from the complete br graph count.
pub fn parse_bv_unblocks(recommendation: &Value) -> Unblocks {
    match recommendation.get("unblocks") {
        Some(value) => value
            .as_u64()
            .map(Unblocks::known)
            .unwrap_or_else(|| Unblocks::unknown("unblocks field is not an integer")),
        None => Unblocks::unknown("unblocks field absent from BV response"),
    }
}

/// Evaluate a complete issue set and its live blocks edges.
pub fn evaluate_graph(
    issues: &[IssueRecord],
    edges: &[BlockerEdge],
) -> Result<GraphReport, GraphError> {
    if issues.is_empty() {
        return Err(GraphError::EmptyIssueSet);
    }
    let status_by_id = issues
        .iter()
        .map(|issue| (issue.id.clone(), issue.status))
        .collect::<BTreeMap<_, _>>();
    if status_by_id.len() != issues.len() {
        let mut seen = BTreeSet::new();
        let duplicate = issues
            .iter()
            .find(|issue| !seen.insert(issue.id.as_str()))
            .map(|issue| issue.id.clone())
            .unwrap_or_else(|| "<unknown>".to_owned());
        return Err(GraphError::DuplicateIssue(duplicate));
    }

    let mut blockers_by_child = BTreeMap::<String, Vec<String>>::new();
    let mut dependents_by_blocker = BTreeMap::<String, u64>::new();
    let mut stale_edges = Vec::new();
    for edge in edges {
        if !status_by_id.contains_key(&edge.child_id) {
            stale_edges.push(StaleEdge {
                child_id: edge.child_id.clone(),
                blocker_id: edge.blocker_id.clone(),
                reason: "child id is absent from br list -a".to_owned(),
            });
            continue;
        }
        if !status_by_id.contains_key(&edge.blocker_id) {
            stale_edges.push(StaleEdge {
                child_id: edge.child_id.clone(),
                blocker_id: edge.blocker_id.clone(),
                reason: "blocker id is absent from br list -a".to_owned(),
            });
            blockers_by_child
                .entry(edge.child_id.clone())
                .or_default()
                .push(edge.blocker_id.clone());
            continue;
        }
        blockers_by_child
            .entry(edge.child_id.clone())
            .or_default()
            .push(edge.blocker_id.clone());
        if status_by_id[&edge.child_id].is_non_terminal() {
            *dependents_by_blocker
                .entry(edge.blocker_id.clone())
                .or_default() += 1;
        }
    }

    let mut reports = Vec::with_capacity(issues.len());
    for issue in issues {
        let mut blockers = Vec::new();
        let mut has_blocking = false;
        let mut has_unknown = false;
        for blocker_id in blockers_by_child.get(&issue.id).into_iter().flatten() {
            let (blocker_status, state, reason) = match status_by_id.get(blocker_id) {
                Some(IssueStatus::Closed) => (
                    IssueStatus::Closed,
                    BlockerState::Released,
                    "closed blocker releases this child".to_owned(),
                ),
                Some(status) if status.is_non_terminal() => (
                    *status,
                    BlockerState::Blocking,
                    "non-terminal blocker still holds this child".to_owned(),
                ),
                Some(status) => (
                    *status,
                    BlockerState::StaleEdge,
                    "blocker status is not a recognized terminal or live state".to_owned(),
                ),
                None => (
                    IssueStatus::Unknown,
                    BlockerState::StaleEdge,
                    "blocker id is absent from br list -a".to_owned(),
                ),
            };
            has_blocking |= state == BlockerState::Blocking;
            has_unknown |= state == BlockerState::StaleEdge;
            blockers.push(BlockerReport {
                blocker_id: blocker_id.clone(),
                blocker_status,
                state,
                reason,
            });
        }
        let availability = if has_unknown {
            Availability::Unknown
        } else if has_blocking {
            Availability::Blocked
        } else {
            Availability::Available
        };
        let unblocks = if status_by_id.contains_key(&issue.id) {
            Unblocks::known(dependents_by_blocker.get(&issue.id).copied().unwrap_or(0))
        } else {
            Unblocks::unknown("issue status unavailable")
        };
        reports.push(BeadAvailability {
            bead_id: issue.id.clone(),
            status: issue.status,
            availability,
            blockers,
            unblocks,
        });
    }
    reports.sort_by(|left, right| left.bead_id.cmp(&right.bead_id));
    Ok(GraphReport {
        schema: "bead-availability/v1",
        issues: reports,
        stale_edges,
    })
}

async fn run_br(cx: &Cx, program: &str, args: &[&str]) -> Result<Vec<u8>, LiveError> {
    let command_text = format!("{program} {}", args.join(" "));
    cx.checkpoint().map_err(|_| LiveError::CommandFailed {
        command: command_text.clone(),
        detail: "caller context cancelled before spawn".to_owned(),
    })?;
    let mut command = Command::new(program);
    command.args(args);
    let output = match timeout(
        cx.now_for_observability(),
        BR_COMMAND_TIMEOUT,
        run_output(cx, command),
    )
    .await
    {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            return Err(LiveError::Run {
                command: command_text,
                error,
            })
        }
        Err(_) => {
            return Err(LiveError::CommandFailed {
                command: command_text,
                detail: format!("TIMEOUT after {}s", BR_COMMAND_TIMEOUT.as_secs()),
            });
        }
    };
    if !output.status.success() {
        return Err(LiveError::CommandFailed {
            command: format!("{program} {}", args.join(" ")),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(output.stdout)
}

/// Read every non-terminal issue with br -a and ask br for its live blockers.
/// The -a flag is load-bearing: default br list omits the closed blockers this
/// command must report as RELEASED.
pub async fn collect_live(cx: &Cx, br_program: &str) -> Result<GraphReport, String> {
    let issue_command = format!("{br_program} list -a --json");
    let issue_bytes = run_br(cx, br_program, &["list", "-a", "--json"])
        .await
        .map_err(|error| error.to_string())?;
    let issue_value: Value = serde_json::from_slice(&issue_bytes).map_err(|error| {
        LiveError::Json {
            command: issue_command,
            detail: error.to_string(),
        }
        .to_string()
    })?;
    let issues = parse_br_issues(&issue_value).map_err(|error| error.to_string())?;
    let mut edges = Vec::new();
    for issue in issues.iter().filter(|issue| issue.status.is_non_terminal()) {
        cx.checkpoint()
            .map_err(|_| format!("CANCELLED before blocker read bead={}", issue.id))?;
        let output = run_br(
            cx,
            br_program,
            &["dep", "list", &issue.id, "--direction", "down", "--json"],
        )
        .await
        .map_err(|error| error.to_string())?;
        let value: Value = serde_json::from_slice(&output).map_err(|error| {
            format!(
                "JSON_FAILED command=br dep list bead={} detail={error}",
                issue.id
            )
        })?;
        edges.extend(parse_br_blockers(&value, &issue.id).map_err(|error| error.to_string())?);
    }
    evaluate_graph(&issues, &edges).map_err(|error| error.to_string())
}

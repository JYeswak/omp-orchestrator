//! Bead-DAG priority inversion, classified with asupersync's `InversionType`.
//!
//! Adopted from
//! `/Volumes/ZestData/dicklesworthstone-mirror/asupersync/src/lab/oracle/priority_inversion.rs`
//! (`Direct`, `Transitive`, `InheritanceFailure`). That file constructs Direct
//! and Transitive; `InheritanceFailure` is in the enum and Display and is never
//! assigned — the same hole `br` has: priority is stored and never inherited.
//! Dropping the third arm because nothing implements it would hide the hole.
//!
//! Beads encode urgency as a smaller integer (P0 = 0 beats P2 = 2). An inversion
//! is a live bead blocked, along `type=blocks` edges only, by a live bead with a
//! strictly worse (larger) priority. `parent-child` and `related` are parsed so
//! the field names stay honest; they are not inversion edges.
//!
//! Counts are unique victim beads per arm, not edges. A bead may be Direct and
//! Transitive at once. `InheritanceFailure` is the union: every inverted bead,
//! because `br` has no inheritance mechanism that could raise the blocker.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

/// Closed three-arm taxonomy. A single `inversions=` integer fails the contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InversionType {
    /// High-urgency bead blocked directly by a lower-urgency bead.
    Direct,
    /// Blocking chain of length ≥ 2 ending at a lower-urgency bead.
    Transitive,
    /// Priority inheritance did not fire. `br` cannot fire it.
    InheritanceFailure,
}

impl fmt::Display for InversionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Direct => write!(f, "Direct"),
            Self::Transitive => write!(f, "Transitive"),
            Self::InheritanceFailure => write!(f, "InheritanceFailure"),
        }
    }
}

/// Why the third arm is always populated when any inversion exists.
pub const BR_HAS_NO_INHERITANCE: &str = "br stores priority and never inherits it";

/// Production walk: unbounded. A depth-1-only walk misses Transitive.
pub const DETECT_TRANSITIVE: bool = true;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanConfig {
    /// Inclusive max edge distance. Production is `usize::MAX`.
    pub max_depth: usize,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            max_depth: usize::MAX,
        }
    }
}

impl ScanConfig {
    #[must_use]
    pub fn production() -> Self {
        Self::default()
    }

    /// Depth-1-only walk. Used by the mutation leg to prove Transitive is load-bearing.
    #[must_use]
    pub fn direct_only() -> Self {
        Self { max_depth: 1 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DagIssue {
    pub id: String,
    /// Missing priority is skipped, never coerced to a sentinel (the `or 9` trap).
    pub priority: Option<i32>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DagEdge {
    /// Blocked bead. Production column name: `issue_id`.
    pub issue_id: String,
    /// Blocker bead. Production column name: `depends_on_id`.
    pub depends_on_id: String,
    /// `blocks` / `parent-child` / `related`.
    pub dep_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inversion {
    pub inversion_type: InversionType,
    pub blocked_id: String,
    pub blocked_priority: i32,
    pub blocking_id: String,
    pub blocking_priority: i32,
    pub depth: usize,
    pub blocking_chain: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classification {
    pub direct: Vec<Inversion>,
    pub transitive: Vec<Inversion>,
    pub inheritance_failures: Vec<Inversion>,
    /// Always true: `br` has no inheritance mechanism.
    pub inheritance_mechanism_absent: bool,
    pub p0_with_lower_priority_blocker: Vec<String>,
}

impl Classification {
    #[must_use]
    pub fn direct_count(&self) -> usize {
        self.direct.len()
    }

    #[must_use]
    pub fn transitive_count(&self) -> usize {
        self.transitive.len()
    }

    #[must_use]
    pub fn inheritance_failure_count(&self) -> usize {
        self.inheritance_failures.len()
    }

    #[must_use]
    pub fn has_inversion(&self) -> bool {
        !self.direct.is_empty() || !self.transitive.is_empty()
    }

    /// Exit 1 on any Direct/Transitive inversion. Clean graphs stay 0.
    #[must_use]
    pub fn gate_exit(&self) -> u8 {
        if self.has_inversion() {
            1
        } else {
            0
        }
    }

    #[must_use]
    pub fn report(&self) -> String {
        let mut out = String::new();
        push_arm(
            &mut out,
            InversionType::Direct,
            &self.direct,
        );
        push_arm(
            &mut out,
            InversionType::Transitive,
            &self.transitive,
        );
        push_arm(
            &mut out,
            InversionType::InheritanceFailure,
            &self.inheritance_failures,
        );
        out.push_str("INHERITANCE_MECHANISM absent ");
        out.push_str(BR_HAS_NO_INHERITANCE);
        out.push('\n');
        let p0_example = self
            .p0_with_lower_priority_blocker
            .first()
            .map(String::as_str)
            .unwrap_or("-");
        out.push_str(&format!(
            "P0_WITH_LOWER_PRIORITY_BLOCKER count={} example={p0_example}\n",
            self.p0_with_lower_priority_blocker.len()
        ));
        out
    }
}

fn push_arm(out: &mut String, kind: InversionType, rows: &[Inversion]) {
    let example = rows
        .first()
        .map(|row| row.blocked_id.as_str())
        .unwrap_or("-");
    out.push_str(&format!(
        "INVERSION_TYPE {kind} count={} example={example}\n",
        rows.len()
    ));
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanError {
    EmptyIssueSet,
    DuplicateIssue(String),
    Malformed(&'static str),
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIssueSet => write!(f, "INVERSION_SCAN_EMPTY: no issues to classify"),
            Self::DuplicateIssue(id) => write!(f, "INVERSION_DUPLICATE_ISSUE id={id}"),
            Self::Malformed(detail) => write!(f, "INVERSION_GRAPH_MALFORMED {detail}"),
        }
    }
}

impl ScanError {
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::EmptyIssueSet => 2,
            Self::DuplicateIssue(_) | Self::Malformed(_) => 3,
        }
    }
}

/// Live means not closed. `grading` / `blocked` / `redispatch_required` stay in the DAG.
#[must_use]
pub fn is_live(status: &str) -> bool {
    status != "closed"
}

#[must_use]
pub fn classify(issues: &[DagIssue], edges: &[DagEdge]) -> Result<Classification, ScanError> {
    classify_with(issues, edges, ScanConfig::production())
}

pub fn classify_with(
    issues: &[DagIssue],
    edges: &[DagEdge],
    config: ScanConfig,
) -> Result<Classification, ScanError> {
    if issues.is_empty() {
        return Err(ScanError::EmptyIssueSet);
    }
    let mut by_id = BTreeMap::<&str, &DagIssue>::new();
    for issue in issues {
        if by_id.insert(issue.id.as_str(), issue).is_some() {
            return Err(ScanError::DuplicateIssue(issue.id.clone()));
        }
    }

    let max_depth = if DETECT_TRANSITIVE {
        config.max_depth
    } else {
        1
    };

    let mut adj: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for edge in edges {
        if edge.dep_type != "blocks" {
            continue;
        }
        let Some(child) = by_id.get(edge.issue_id.as_str()) else {
            continue;
        };
        let Some(blocker) = by_id.get(edge.depends_on_id.as_str()) else {
            continue;
        };
        if !is_live(&child.status) || !is_live(&blocker.status) {
            continue;
        }
        if child.priority.is_none() || blocker.priority.is_none() {
            continue;
        }
        adj.entry(edge.issue_id.as_str())
            .or_default()
            .push(edge.depends_on_id.as_str());
    }

    let mut direct = Vec::new();
    let mut transitive = Vec::new();
    let mut p0 = Vec::new();

    for issue in issues {
        if !is_live(&issue.status) {
            continue;
        }
        let Some(priority) = issue.priority else {
            continue;
        };
        let mut found_direct: Option<Inversion> = None;
        let mut found_trans: Option<Inversion> = None;
        let mut found_any_lower = false;

        let mut seen = BTreeSet::new();
        seen.insert(issue.id.as_str());
        let mut queue = VecDeque::new();
        queue.push_back((issue.id.as_str(), 0usize, Vec::<String>::new()));

        while let Some((id, depth, chain)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }
            let Some(nexts) = adj.get(id) else {
                continue;
            };
            for &nxt in nexts {
                if !seen.insert(nxt) {
                    continue;
                }
                let Some(node) = by_id.get(nxt) else {
                    continue;
                };
                let Some(node_pri) = node.priority else {
                    continue;
                };
                let next_depth = depth + 1;
                let mut next_chain = chain.clone();
                next_chain.push(nxt.to_owned());
                if priority < node_pri {
                    found_any_lower = true;
                    let row = Inversion {
                        inversion_type: if next_depth == 1 {
                            InversionType::Direct
                        } else {
                            InversionType::Transitive
                        },
                        blocked_id: issue.id.clone(),
                        blocked_priority: priority,
                        blocking_id: nxt.to_owned(),
                        blocking_priority: node_pri,
                        depth: next_depth,
                        blocking_chain: next_chain.clone(),
                    };
                    if next_depth == 1 && found_direct.is_none() {
                        found_direct = Some(row.clone());
                    }
                    if next_depth >= 2 && found_trans.is_none() {
                        found_trans = Some(row);
                    }
                }
                queue.push_back((nxt, next_depth, next_chain));
            }
        }

        if let Some(row) = found_direct {
            direct.push(row);
        }
        if let Some(row) = found_trans {
            transitive.push(row);
        }
        if priority == 0 && found_any_lower {
            p0.push(issue.id.clone());
        }
    }

    direct.sort_by(|a, b| a.blocked_id.cmp(&b.blocked_id));
    transitive.sort_by(|a, b| a.blocked_id.cmp(&b.blocked_id));
    p0.sort();

    let mut inheritance_ids = BTreeSet::new();
    for row in direct.iter().chain(transitive.iter()) {
        inheritance_ids.insert(row.blocked_id.clone());
    }
    let inheritance_failures = inheritance_ids
        .into_iter()
        .map(|id| {
            let src = direct
                .iter()
                .chain(transitive.iter())
                .find(|row| row.blocked_id == id)
                .expect("union id comes from an arm");
            Inversion {
                inversion_type: InversionType::InheritanceFailure,
                blocked_id: src.blocked_id.clone(),
                blocked_priority: src.blocked_priority,
                blocking_id: src.blocking_id.clone(),
                blocking_priority: src.blocking_priority,
                depth: src.depth,
                blocking_chain: src.blocking_chain.clone(),
            }
        })
        .collect::<Vec<_>>();

    Ok(Classification {
        direct,
        transitive,
        inheritance_failures,
        inheritance_mechanism_absent: true,
        p0_with_lower_priority_blocker: p0,
    })
}

pub fn parse_graph_json(value: &Value) -> Result<(Vec<DagIssue>, Vec<DagEdge>), ScanError> {
    let obj = value
        .as_object()
        .ok_or(ScanError::Malformed("graph is not an object"))?;
    let issues_val = obj
        .get("issues")
        .and_then(Value::as_array)
        .ok_or(ScanError::Malformed("issues is not an array"))?;
    let deps_val = obj
        .get("dependencies")
        .and_then(Value::as_array)
        .ok_or(ScanError::Malformed("dependencies is not an array"))?;

    let mut issues = Vec::with_capacity(issues_val.len());
    for issue in issues_val {
        let id = issue
            .get("id")
            .and_then(Value::as_str)
            .ok_or(ScanError::Malformed("issue has no id"))?
            .to_owned();
        let priority = match issue.get("priority") {
            None | Some(Value::Null) => None,
            Some(Value::Number(n)) => n.as_i64().map(|v| v as i32),
            Some(_) => return Err(ScanError::Malformed("priority is not an integer")),
        };
        let status = issue
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("open")
            .to_owned();
        issues.push(DagIssue {
            id,
            priority,
            status,
        });
    }

    let mut edges = Vec::with_capacity(deps_val.len());
    for edge in deps_val {
        let issue_id = edge
            .get("issue_id")
            .and_then(Value::as_str)
            .ok_or(ScanError::Malformed("edge missing issue_id"))?
            .to_owned();
        let depends_on_id = edge
            .get("depends_on_id")
            .and_then(Value::as_str)
            .ok_or(ScanError::Malformed("edge missing depends_on_id"))?
            .to_owned();
        let dep_type = edge
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("blocks")
            .to_owned();
        edges.push(DagEdge {
            issue_id,
            depends_on_id,
            dep_type,
        });
    }
    Ok((issues, edges))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn issue(id: &str, priority: i32, status: &str) -> DagIssue {
        DagIssue {
            id: id.to_owned(),
            priority: Some(priority),
            status: status.to_owned(),
        }
    }

    fn blocks(child: &str, blocker: &str) -> DagEdge {
        DagEdge {
            issue_id: child.to_owned(),
            depends_on_id: blocker.to_owned(),
            dep_type: "blocks".to_owned(),
        }
    }

    fn parent_child(child: &str, parent: &str) -> DagEdge {
        DagEdge {
            issue_id: child.to_owned(),
            depends_on_id: parent.to_owned(),
            dep_type: "parent-child".to_owned(),
        }
    }

    #[test]
    fn claim_header() {
        let report = classify(
            &[issue("a", 0, "open"), issue("b", 0, "open")],
            &[blocks("a", "b")],
        )
        .expect("known-good equal-priority graph")
        .report();
        assert!(report.contains("INVERSION_TYPE Direct"), "{report}");
        assert!(report.contains("INVERSION_TYPE Transitive"), "{report}");
        assert!(
            report.contains("INVERSION_TYPE InheritanceFailure"),
            "{report}"
        );
        assert!(
            report.contains(BR_HAS_NO_INHERITANCE),
            "must state br has no inheritance, got {report}"
        );
        assert!(
            !report.contains("inversions="),
            "a single inversions= integer is not a classification: {report}"
        );
        let _ = InversionType::Direct;
        let _ = InversionType::Transitive;
        let _ = InversionType::InheritanceFailure;
    }

    #[test]
    fn fires_on_known_bad() {
        let graph = classify(
            &[issue("p0", 0, "open"), issue("p2", 2, "open")],
            &[blocks("p0", "p2")],
        )
        .expect("P2-blocks-P0 is a well-formed graph");
        assert_eq!(graph.direct_count(), 1);
        assert_eq!(graph.direct[0].inversion_type, InversionType::Direct);
        assert_eq!(graph.direct[0].blocked_id, "p0");
        assert_eq!(graph.direct[0].blocking_id, "p2");
        assert_eq!(graph.direct[0].depth, 1);
        assert_eq!(graph.gate_exit(), 1);
        assert!(graph.report().contains("INVERSION_TYPE Direct count=1"));
    }

    #[test]
    fn passes_known_good() {
        let graph = classify(
            &[
                issue("urgent", 0, "open"),
                issue("blocker", 0, "open"),
                issue("lower", 2, "open"),
            ],
            &[blocks("lower", "urgent")],
        )
        .expect("P2 waiting on P0 is not an inversion");
        assert_eq!(graph.direct_count(), 0);
        assert_eq!(graph.transitive_count(), 0);
        assert_eq!(graph.inheritance_failure_count(), 0);
        assert_eq!(graph.gate_exit(), 0);
        assert!(graph.inheritance_mechanism_absent);
    }

    #[test]
    fn empty_scan_is_error() {
        let error = classify(&[], &[]).expect_err("empty scan must not pass");
        assert!(matches!(error, ScanError::EmptyIssueSet), "{error}");
        assert_eq!(error.exit_code(), 2);
        assert!(error.to_string().contains("INVERSION_SCAN_EMPTY"));
    }

    #[test]
    fn mutation_goes_red() {
        let issues = [
            issue("victim-p0", 0, "open"),
            issue("mid-p0", 0, "open"),
            issue("tail-p1", 1, "open"),
        ];
        let edges = [blocks("victim-p0", "mid-p0"), blocks("mid-p0", "tail-p1")];
        let production = classify(&issues, &edges).expect("chain");
        let victim_direct = production
            .direct
            .iter()
            .any(|row| row.blocked_id == "victim-p0");
        let victim_trans = production
            .transitive
            .iter()
            .find(|row| row.blocked_id == "victim-p0")
            .expect("victim must be Transitive");
        assert!(!victim_direct, "direct neighbor is also P0");
        assert_eq!(victim_trans.inversion_type, InversionType::Transitive);
        assert_eq!(victim_trans.blocking_id, "tail-p1");
        assert_eq!(victim_trans.depth, 2);

        let depth1 =
            classify_with(&issues, &edges, ScanConfig::direct_only()).expect("depth-1 walk");
        assert!(depth1
            .transitive
            .iter()
            .all(|row| row.blocked_id != "victim-p0"));
        assert!(depth1
            .direct
            .iter()
            .all(|row| row.blocked_id != "victim-p0"));
        assert_eq!(production.gate_exit(), 1);
    }

    #[test]
    fn parent_child_is_not_an_inversion_edge() {
        let graph = classify(
            &[issue("leaf-p0", 0, "open"), issue("epic-p1", 1, "open")],
            &[parent_child("leaf-p0", "epic-p1")],
        )
        .expect("parent-child graph");
        assert_eq!(
            graph.direct_count(),
            0,
            "parent-child must not be walked; that was the 88 P1→P0 miscount"
        );
    }

    #[test]
    fn missing_priority_is_not_coerced_to_nine() {
        let issues = [
            DagIssue {
                id: "no-pri".into(),
                priority: None,
                status: "open".into(),
            },
            issue("p2", 2, "open"),
        ];
        let graph = classify(&issues, &[blocks("no-pri", "p2")]).expect("graph");
        assert_eq!(
            graph.direct_count(),
            0,
            "missing priority must skip, not become a fake P9 inversion"
        );
    }

    #[test]
    fn closed_blocker_is_not_live() {
        let graph = classify(
            &[issue("p0", 0, "open"), issue("p2", 2, "closed")],
            &[blocks("p0", "p2")],
        )
        .expect("graph");
        assert_eq!(graph.direct_count(), 0);
    }

    #[test]
    fn edge_fields_are_issue_id_and_depends_on_id() {
        let ok = json!({
            "issues": [{"id": "p0", "priority": 0, "status": "open"}, {"id": "p2", "priority": 2, "status": "open"}],
            "dependencies": [{"issue_id": "p0", "depends_on_id": "p2", "type": "blocks"}]
        });
        let (issues, edges) = parse_graph_json(&ok).expect("production field names");
        let graph = classify(&issues, &edges).expect("classified");
        assert_eq!(graph.direct_count(), 1);

        let missing = json!({
            "issues": [{"id": "p0", "priority": 0, "status": "open"}],
            "dependencies": [{"child_id": "p0", "blocker_id": "p2", "type": "blocks"}]
        });
        let error = parse_graph_json(&missing).expect_err("wrong field names must not parse");
        assert!(
            error.to_string().contains("issue_id"),
            "positive control is issue_id, got {error}"
        );
    }

    #[test]
    fn reproduces_measured_state() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/fixtures/live-dag-20260906.json"
        );
        let text = std::fs::read_to_string(path).expect("snapshot present");
        let value: serde_json::Value = serde_json::from_str(&text).expect("json");
        let (issues, edges) = parse_graph_json(&value).expect("snapshot parses");
        let graph = classify(&issues, &edges).expect("snapshot classifies");
        assert_eq!(graph.direct_count(), 51, "{}", graph.report());
        assert_eq!(graph.transitive_count(), 97, "{}", graph.report());
        assert_eq!(
            graph.p0_with_lower_priority_blocker.len(),
            65,
            "{}",
            graph.report()
        );
        assert!(graph.inheritance_mechanism_absent);
        assert!(!graph.inheritance_failures.is_empty());
        let report = graph.report();
        assert!(report.contains("INVERSION_TYPE Direct count=51"));
        assert!(report.contains("INVERSION_TYPE Transitive count=97"));
        assert!(report.contains(BR_HAS_NO_INHERITANCE));
        assert!(!report.contains("inversions="));
        assert_eq!(graph.direct[0].blocked_id, "omp-orchestrator-17pm");
    }

    #[test]
    fn known_bad_in_tree_fixture_is_direct_and_nonzero() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/fixtures/p2-blocks-p0.json"
        );
        let text = std::fs::read_to_string(path).expect("fixture");
        let value: serde_json::Value = serde_json::from_str(&text).expect("json");
        let (issues, edges) = parse_graph_json(&value).expect("parse");
        let graph = classify(&issues, &edges).expect("classify");
        assert_eq!(graph.gate_exit(), 1);
        assert_eq!(graph.direct_count(), 1);
    }
}


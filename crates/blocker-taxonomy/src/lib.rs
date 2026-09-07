#![forbid(unsafe_code)]

//! Closed blocker taxonomy. Progress tokens are a *different* type so they cannot
//! enter the enum. Every `BlockerKind` has a detection predicate and a remedy class.
//!
//! Census input (not re-derived as 20 claim probes): xbcl comments 2348/2349.
//! Both-direction SQL on `.beads/beads.db` (2026-09-06, this pane): live=729,
//! status=blocked=228, unresolved out-`blocks`=181, open+unresolved-out-blocks=138
//! (P0=56), status-blocked with *no* unresolved out-`blocks`=210, epics with
//! parent-child out-edge to a live child=29. The 18-edge figure used out-edges
//! only through a parser AGENTS.md warns is not evidence.
//!
//! Pinned *report* positive-control baseline (measured 2026-09-06 pane 1, b2d131b):
//! 119 status-blocked + 173 open-dep-blocked = 292, P0=67 among the dep-blocked.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Pinned pane-1 baseline. Live counts drift; the fixture must still hit these.
pub const BASELINE_STATUS_BLOCKED: usize = 119;
pub const BASELINE_DEP_BLOCKED: usize = 173;
pub const BASELINE_BLOCKED_TOTAL: usize = 292;
pub const BASELINE_DEP_BLOCKED_P0: usize = 67;

/// Closed set of *blocker* kinds. Progress tokens are not members.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlockerKind {
    TrackerBlocked,
    DependencyBlocked,
    CrossPaneHold,
    PendingDispatchMarker,
    StrandedGradingClaim,
    AckIndeterminate,
    PacketRefused,
    QueueUnranked,
    MissingReceiverAgent,
    /// Measured: `kxe` blocked by own child `2lqd` via parent-child.
    OwnChildBlocksEpic,
    /// Measured: `open`+assignee or `in_progress`+no assignee. Refuses dispatch.
    HalfClaim,
}

/// Every variant, compiler-enforced: adding a kind without updating this fails tests.
pub const ALL_KINDS: &[BlockerKind] = &[
    BlockerKind::TrackerBlocked,
    BlockerKind::DependencyBlocked,
    BlockerKind::CrossPaneHold,
    BlockerKind::PendingDispatchMarker,
    BlockerKind::StrandedGradingClaim,
    BlockerKind::AckIndeterminate,
    BlockerKind::PacketRefused,
    BlockerKind::QueueUnranked,
    BlockerKind::MissingReceiverAgent,
    BlockerKind::OwnChildBlocksEpic,
    BlockerKind::HalfClaim,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RemedyClass {
    SelfClearing,
    OperatorClearable,
    KernelClearable,
    HumanGated,
}

/// Benign heartbeat tokens. Not variants of [`BlockerKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProgressToken {
    StillChanging,
    Working,
    VerdictPosted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Progress(ProgressToken),
    BlockerHint(BlockerKind),
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeadRecord {
    pub id: String,
    pub status: String,
    pub assignee: Option<String>,
    pub issue_type: String,
    pub priority: i32,
    /// Out-edges of type `blocks` to a *live* (non-closed) issue.
    pub unresolved_out_blocks: Vec<String>,
    /// Out-edges of type `parent-child` to a live issue (epic waits on children).
    pub unresolved_out_parent_child: Vec<String>,
    pub heartbeat_reason: Option<String>,
    pub hold_pane: Option<String>,
    pub pending_dispatch_marker: bool,
    pub grading_started_unterminal: bool,
}

impl BeadRecord {
    pub fn assignee_set(&self) -> bool {
        self.assignee
            .as_deref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
    }
}

pub fn parse_reason(raw: &str) -> Reason {
    let t = raw.trim();
    if t.eq_ignore_ascii_case("still_changing") {
        return Reason::Progress(ProgressToken::StillChanging);
    }
    if t.eq_ignore_ascii_case("working") {
        return Reason::Progress(ProgressToken::Working);
    }
    if t.eq_ignore_ascii_case("VERDICT_POSTED") || t.eq_ignore_ascii_case("verdict_posted") {
        return Reason::Progress(ProgressToken::VerdictPosted);
    }
    if t.contains("MISSING_RECEIVER_AGENT") {
        return Reason::BlockerHint(BlockerKind::MissingReceiverAgent);
    }
    if t.eq_ignore_ascii_case("UNRANKED") || t.contains("UNRANKED") {
        return Reason::BlockerHint(BlockerKind::QueueUnranked);
    }
    if t.contains("DISPATCH_FAILED_BEFORE_RECEIPT")
        || t.eq_ignore_ascii_case("Refused")
        || t.contains("packet refused")
    {
        return Reason::BlockerHint(BlockerKind::PacketRefused);
    }
    if t.contains("ACK_PANE_MISMATCH") || t.contains("ack_readback_missing") {
        return Reason::BlockerHint(BlockerKind::AckIndeterminate);
    }
    if t.contains("CROSS_PANE") {
        return Reason::BlockerHint(BlockerKind::CrossPaneHold);
    }
    Reason::Unknown
}

pub fn remedy(kind: BlockerKind) -> RemedyClass {
    match kind {
        BlockerKind::DependencyBlocked => RemedyClass::SelfClearing,
        BlockerKind::CrossPaneHold | BlockerKind::PendingDispatchMarker => {
            RemedyClass::KernelClearable
        }
        BlockerKind::MissingReceiverAgent => RemedyClass::HumanGated,
        BlockerKind::TrackerBlocked
        | BlockerKind::StrandedGradingClaim
        | BlockerKind::AckIndeterminate
        | BlockerKind::PacketRefused
        | BlockerKind::QueueUnranked
        | BlockerKind::OwnChildBlocksEpic
        | BlockerKind::HalfClaim => RemedyClass::OperatorClearable,
    }
}

pub fn predicate_fires(kind: BlockerKind, bead: &BeadRecord) -> bool {
    match kind {
        BlockerKind::TrackerBlocked => bead.status == "blocked",
        BlockerKind::DependencyBlocked => {
            bead.status != "closed" && !bead.unresolved_out_blocks.is_empty()
        }
        BlockerKind::CrossPaneHold => bead.hold_pane.is_some(),
        BlockerKind::PendingDispatchMarker => bead.pending_dispatch_marker,
        BlockerKind::StrandedGradingClaim => bead.grading_started_unterminal,
        BlockerKind::AckIndeterminate => matches!(
            bead.heartbeat_reason.as_deref().map(parse_reason),
            Some(Reason::BlockerHint(BlockerKind::AckIndeterminate))
        ),
        BlockerKind::PacketRefused => matches!(
            bead.heartbeat_reason.as_deref().map(parse_reason),
            Some(Reason::BlockerHint(BlockerKind::PacketRefused))
        ),
        BlockerKind::QueueUnranked => matches!(
            bead.heartbeat_reason.as_deref().map(parse_reason),
            Some(Reason::BlockerHint(BlockerKind::QueueUnranked))
        ),
        BlockerKind::MissingReceiverAgent => matches!(
            bead.heartbeat_reason.as_deref().map(parse_reason),
            Some(Reason::BlockerHint(BlockerKind::MissingReceiverAgent))
        ),
        BlockerKind::OwnChildBlocksEpic => {
            bead.issue_type == "epic" && !bead.unresolved_out_parent_child.is_empty()
        }
        BlockerKind::HalfClaim => {
            (bead.status == "open" && bead.assignee_set())
                || (bead.status == "in_progress" && !bead.assignee_set())
        }
    }
}

/// All kinds that fire. Empty means not a blocker (including progress-only).
pub fn classify(bead: &BeadRecord) -> Vec<BlockerKind> {
    ALL_KINDS
        .iter()
        .copied()
        .filter(|k| predicate_fires(*k, bead))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReportedBead {
    pub id: String,
    pub kinds: Vec<BlockerKind>,
    pub remedies: Vec<RemedyClass>,
    pub evidence: String,
    pub tracker_class: TrackerClass,
    pub priority: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrackerClass {
    /// `status == blocked`. The visible class.
    StatusBlocked,
    /// Open (or other non-blocked live status) with unresolved out-`blocks`.
    DependencyBlocked,
    /// Other typed blockers that are neither of the two tracker classes.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub status_blocked: usize,
    pub dependency_blocked: usize,
    pub dependency_blocked_p0: usize,
    pub other: usize,
    pub total_listed: usize,
    pub both_classes_reported: bool,
    pub beads: Vec<ReportedBead>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportError {
    /// Empty blocked set while the pinned baseline is nonzero.
    VacuousWhileBaselineNonzero,
}

pub fn tracker_class(bead: &BeadRecord) -> Option<TrackerClass> {
    if bead.status == "closed" {
        return None;
    }
    if bead.status == "blocked" {
        return Some(TrackerClass::StatusBlocked);
    }
    if !bead.unresolved_out_blocks.is_empty() {
        return Some(TrackerClass::DependencyBlocked);
    }
    if !classify(bead).is_empty() {
        return Some(TrackerClass::Other);
    }
    None
}

pub fn report(beads: &[BeadRecord]) -> Result<Report, ReportError> {
    let mut listed = Vec::new();
    let mut status_blocked = 0usize;
    let mut dependency_blocked = 0usize;
    let mut dependency_blocked_p0 = 0usize;
    let mut other = 0usize;

    for bead in beads {
        let class = match tracker_class(bead) {
            Some(c) => c,
            None => continue,
        };
        match class {
            TrackerClass::StatusBlocked => status_blocked += 1,
            TrackerClass::DependencyBlocked => {
                dependency_blocked += 1;
                if bead.priority == 0 {
                    dependency_blocked_p0 += 1;
                }
            }
            TrackerClass::Other => other += 1,
        }
        let kinds = classify(bead);
        let evidence = format!(
            "status={} out_blocks={} parent_child={} hold={:?} marker={} grading={} reason={:?} assignee={:?}",
            bead.status,
            bead.unresolved_out_blocks.len(),
            bead.unresolved_out_parent_child.len(),
            bead.hold_pane,
            bead.pending_dispatch_marker,
            bead.grading_started_unterminal,
            bead.heartbeat_reason,
            bead.assignee
        );
        listed.push(ReportedBead {
            id: bead.id.clone(),
            remedies: kinds.iter().copied().map(remedy).collect(),
            kinds,
            evidence,
            tracker_class: class,
            priority: bead.priority,
        });
    }

    let both = status_blocked > 0 && dependency_blocked > 0;
    if listed.is_empty() && BASELINE_BLOCKED_TOTAL > 0 {
        return Err(ReportError::VacuousWhileBaselineNonzero);
    }
    Ok(Report {
        status_blocked,
        dependency_blocked,
        dependency_blocked_p0,
        other,
        total_listed: listed.len(),
        both_classes_reported: both,
        beads: listed,
    })
}

/// Pinned 292-row census: 119 status-blocked + 173 open-dep-blocked, 67 of the latter P0.
pub fn baseline_292_fixture() -> Vec<BeadRecord> {
    let mut rows = Vec::with_capacity(BASELINE_BLOCKED_TOTAL);
    for i in 0..BASELINE_STATUS_BLOCKED {
        rows.push(BeadRecord {
            id: format!("status-blocked-{i}"),
            status: "blocked".into(),
            assignee: None,
            issue_type: "task".into(),
            priority: 1,
            unresolved_out_blocks: Vec::new(),
            unresolved_out_parent_child: Vec::new(),
            heartbeat_reason: None,
            hold_pane: None,
            pending_dispatch_marker: false,
            grading_started_unterminal: false,
        });
    }
    for i in 0..BASELINE_DEP_BLOCKED {
        let priority = if i < BASELINE_DEP_BLOCKED_P0 { 0 } else { 1 };
        rows.push(BeadRecord {
            id: format!("dep-blocked-{i}"),
            status: "open".into(),
            assignee: None,
            issue_type: "task".into(),
            priority,
            unresolved_out_blocks: vec![format!("blocker-of-{i}")],
            unresolved_out_parent_child: Vec::new(),
            heartbeat_reason: None,
            hold_pane: None,
            pending_dispatch_marker: false,
            grading_started_unterminal: false,
        });
    }
    rows
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveSetError {
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveSet {
    panes: BTreeSet<String>,
}

impl LiveSet {
    pub fn new(panes: impl IntoIterator<Item = impl Into<String>>) -> Result<Self, LiveSetError> {
        let panes: BTreeSet<String> = panes.into_iter().map(Into::into).collect();
        if panes.is_empty() {
            return Err(LiveSetError::Empty);
        }
        Ok(Self { panes })
    }

    pub fn contains(&self, pane: &str) -> bool {
        self.panes.contains(pane)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "decision", rename_all = "kebab-case")]
pub enum RedispatchDecision {
    Release {
        bead: String,
        pane: String,
    },
    KeepAlive {
        bead: String,
        pane: String,
    },
    RefuseHumanGated {
        bead: String,
        kind: BlockerKind,
    },
    RefuseNotKernelClearable {
        bead: String,
        kind: BlockerKind,
    },
}

/// Production path. Empty live set cannot be constructed ([`LiveSet::new`]).
pub fn decide_redispatch(
    bead: &str,
    kind: BlockerKind,
    hold_pane: Option<&str>,
    live: &LiveSet,
) -> RedispatchDecision {
    match remedy(kind) {
        RemedyClass::HumanGated => RedispatchDecision::RefuseHumanGated {
            bead: bead.to_string(),
            kind,
        },
        RemedyClass::KernelClearable => match hold_pane {
            Some(pane) if live.contains(pane) => RedispatchDecision::KeepAlive {
                bead: bead.to_string(),
                pane: pane.to_string(),
            },
            Some(pane) => RedispatchDecision::Release {
                bead: bead.to_string(),
                pane: pane.to_string(),
            },
            None => RedispatchDecision::RefuseNotKernelClearable {
                bead: bead.to_string(),
                kind,
            },
        },
        RemedyClass::SelfClearing | RemedyClass::OperatorClearable => {
            RedispatchDecision::RefuseNotKernelClearable {
                bead: bead.to_string(),
                kind,
            }
        }
    }
}

/// Defective twin: treats every hold as dead. Production must not call this.
/// The known-bad redispatch leg asserts this *would* release a live pane.
pub fn decide_redispatch_ignoring_liveness(
    bead: &str,
    kind: BlockerKind,
    hold_pane: Option<&str>,
    _live: &LiveSet,
) -> RedispatchDecision {
    match remedy(kind) {
        RemedyClass::HumanGated => RedispatchDecision::RefuseHumanGated {
            bead: bead.to_string(),
            kind,
        },
        RemedyClass::KernelClearable => match hold_pane {
            Some(pane) => RedispatchDecision::Release {
                bead: bead.to_string(),
                pane: pane.to_string(),
            },
            None => RedispatchDecision::RefuseNotKernelClearable {
                bead: bead.to_string(),
                kind,
            },
        },
        RemedyClass::SelfClearing | RemedyClass::OperatorClearable => {
            RedispatchDecision::RefuseNotKernelClearable {
                bead: bead.to_string(),
                kind,
            }
        }
    }
}

pub fn exhibiting_input(kind: BlockerKind) -> BeadRecord {
    let mut b = BeadRecord {
        id: format!("exhibit-{kind:?}"),
        status: "open".into(),
        assignee: None,
        issue_type: "task".into(),
        priority: 1,
        unresolved_out_blocks: Vec::new(),
        unresolved_out_parent_child: Vec::new(),
        heartbeat_reason: None,
        hold_pane: None,
        pending_dispatch_marker: false,
        grading_started_unterminal: false,
    };
    match kind {
        BlockerKind::TrackerBlocked => b.status = "blocked".into(),
        BlockerKind::DependencyBlocked => b.unresolved_out_blocks = vec!["live-blocker".into()],
        BlockerKind::CrossPaneHold => b.hold_pane = Some("%1408".into()),
        BlockerKind::PendingDispatchMarker => b.pending_dispatch_marker = true,
        BlockerKind::StrandedGradingClaim => b.grading_started_unterminal = true,
        BlockerKind::AckIndeterminate => {
            b.heartbeat_reason = Some("ack_readback_missing".into());
        }
        BlockerKind::PacketRefused => {
            b.heartbeat_reason = Some("DISPATCH_FAILED_BEFORE_RECEIPT".into());
        }
        BlockerKind::QueueUnranked => b.heartbeat_reason = Some("UNRANKED".into()),
        BlockerKind::MissingReceiverAgent => {
            b.heartbeat_reason = Some("MISSING_RECEIVER_AGENT".into());
        }
        BlockerKind::OwnChildBlocksEpic => {
            b.issue_type = "epic".into();
            b.id = "kxe".into();
            b.unresolved_out_parent_child = vec!["2lqd".into()];
        }
        BlockerKind::HalfClaim => {
            b.status = "in_progress".into();
            b.assignee = None;
        }
    }
    b
}

pub fn kinds_by_id(report: &Report) -> BTreeMap<String, Vec<BlockerKind>> {
    report
        .beads
        .iter()
        .map(|b| (b.id.clone(), b.kinds.clone()))
        .collect()
}

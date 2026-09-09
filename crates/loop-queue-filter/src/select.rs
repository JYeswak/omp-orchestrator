#![forbid(unsafe_code)]

//! Dispatch selection order for `2ceb`.
//!
//! 1. actionable blocker (`blockers_to_clear`, ready, unassigned, `unblocks_count > 0`)
//! 2. grading assignment (reapable, grader proven distinct from author)
//! 3. ranked head (`priority` ASC, `score` DESC) — absorbed from `rank_ready` at `9038924`
//! 4. remaining ready (prior order, unranked tail)
//!
//! Grader identity is the pane-scoped assignee string (`pane4-%9`). Agent names
//! (`WildStone`) are not unique across panes. `created_by` is `josh` or `None`
//! and is not a usable author key. If the grader cannot be proven distinct from
//! the author, that bead is not offered as grading (`GRADER_IDENTITY_UNRESOLVED`
//! on a forced-guess path).

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Pane-scoped assignee: unique on one tmux server for one occupancy window.
#[allow(dead_code)]
const PANE_SCOPED: &str = r"^pane[0-9]+-%[0-9]+$";

fn pane_scoped(id: &str) -> bool {
    let b = id.as_bytes();
    if !id.starts_with("pane") {
        return false;
    }
    let rest = &id[4..];
    let Some((digits, pane)) = rest.split_once("-%") else {
        return false;
    };
    !digits.is_empty()
        && digits.bytes().all(|c| c.is_ascii_digit())
        && !pane.is_empty()
        && pane.bytes().all(|c| c.is_ascii_digit())
        && b.iter().all(|c| c.is_ascii())
}

/// Ranked ready queue. Moved from `omp-orchestrator` `rank_ready` at `9038924`.
pub fn rank_ready(triage: &[u8], ready: &[String]) -> Result<Vec<String>, String> {
    let value: Value = serde_json::from_slice(triage)
        .map_err(|error| format!("QUEUE_UNRANKED bv triage JSON: {error}"))?;
    let recs = value
        .get("triage")
        .and_then(|t| t.get("recommendations"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "QUEUE_UNRANKED bv triage has no .triage.recommendations array".to_owned()
        })?;
    if recs.is_empty() {
        return Err("QUEUE_UNRANKED bv triage .triage.recommendations is empty".to_owned());
    }
    let mut ranked: Vec<(u64, i64, String)> = Vec::new();
    for row in recs {
        let Some(id) = row.get("id").and_then(Value::as_str) else {
            continue;
        };
        if !ready.iter().any(|candidate| candidate == id) {
            continue;
        }
        if row
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind.eq_ignore_ascii_case("epic"))
        {
            continue;
        }
        if row
            .get("assignee")
            .and_then(Value::as_str)
            .is_some_and(|who| !who.trim().is_empty())
        {
            continue;
        }
        let priority = row
            .get("priority")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX);
        let score = row
            .get("score")
            .and_then(Value::as_f64)
            .map(|s| -((s * 1_000_000.0) as i64))
            .unwrap_or(0);
        ranked.push((priority, score, id.to_owned()));
    }
    ranked.sort();
    let mut out: Vec<String> = ranked.into_iter().map(|(_, _, id)| id).collect();
    for id in ready {
        if !out.iter().any(|chosen| chosen == id) {
            out.push(id.clone());
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankWindow {
    EmptyReady,
    FilteredFiledOnly,
    RecommendationsOnly,
    RecommendationsPlusPriorityFallback,
    PriorityFallback,
}
impl RankWindow {
    pub const fn label(self) -> &'static str {
        match self {
            Self::EmptyReady => "EMPTY_READY",
            Self::FilteredFiledOnly => "FILTERED_FILED_ONLY",
            Self::RecommendationsOnly => "RECOMMENDATIONS_ONLY",
            Self::RecommendationsPlusPriorityFallback => "RECOMMENDATIONS_PLUS_PRIORITY_FALLBACK",
            Self::PriorityFallback => "PRIORITY_FALLBACK",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedOrder {
    pub ids: Vec<String>,
    pub window: RankWindow,
    pub scored: usize,
}

/// Rank every ready bead. br ready supplies the authoritative priority;
/// bv's score only breaks ties for beads it actually scored.
pub fn rank_ready_with_priorities(
    triage: &[u8],
    ready: &[String],
    ready_priorities: &BTreeMap<String, u64>,
) -> Result<RankedOrder, String> {
    let value: Value = serde_json::from_slice(triage)
        .map_err(|error| format!("QUEUE_UNRANKED bv triage JSON: {error}"))?;
    let recs = value
        .get("triage")
        .and_then(|t| t.get("recommendations"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "QUEUE_UNRANKED bv triage has no .triage.recommendations array".to_owned()
        })?;
    if recs.is_empty() {
        return Err("QUEUE_UNRANKED bv triage .triage.recommendations is empty".to_owned());
    }
    let mut ranked: Vec<(u64, i64, String)> = Vec::new();
    let mut scored: BTreeMap<String, (u64, i64)> = BTreeMap::new();
    for row in recs {
        let Some(id) = row.get("id").and_then(Value::as_str) else {
            continue;
        };
        let priority = row
            .get("priority")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX);
        let score = row
            .get("score")
            .and_then(Value::as_f64)
            .map(|value| -((value * 1_000_000.0) as i64))
            .unwrap_or(0);
        scored.insert(id.to_owned(), (priority, score));
        if !ready.iter().any(|candidate| candidate == id)
            || row
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind.eq_ignore_ascii_case("epic"))
            || row
                .get("assignee")
                .and_then(Value::as_str)
                .is_some_and(|who| !who.trim().is_empty())
        {
            continue;
        }
        ranked.push((
            ready_priorities.get(id).copied().unwrap_or(priority),
            score,
            id.to_owned(),
        ));
    }
    ranked.sort();
    let mut ids: Vec<String> = ranked.iter().map(|(_, _, id)| id.clone()).collect();
    let mut remaining: Vec<(u64, i64, usize, String)> = ready
        .iter()
        .enumerate()
        .filter(|(_, id)| !ids.iter().any(|chosen| chosen == *id))
        .map(|(index, id)| {
            let (fallback_priority, score) = scored.get(id).copied().unwrap_or((u64::MAX, 0));
            (
                ready_priorities
                    .get(id)
                    .copied()
                    .unwrap_or(fallback_priority),
                score,
                index,
                id.clone(),
            )
        })
        .collect();
    remaining.sort();
    let had_ranked = !ids.is_empty();
    let ranked_count = ids.len();
    ids.extend(remaining.into_iter().map(|(_, _, _, id)| id));
    let window = if ready.is_empty() {
        RankWindow::EmptyReady
    } else if !had_ranked {
        RankWindow::PriorityFallback
    } else if ids.len() > ranked_count {
        RankWindow::RecommendationsPlusPriorityFallback
    } else {
        RankWindow::RecommendationsOnly
    };
    Ok(RankedOrder {
        ids,
        window,
        scored: 0,
    })
}

/// Rank the ready set with bv's complete PageRank map.
///
/// The robot-triage command intentionally returns only the top ten
/// recommendations, which can all be blocked or already assigned. The insights
/// payload carries the full graph metric map, so filtering those ten rows must not
/// demote the whole ready set to br priority ordering.
pub fn rank_ready_with_pagerank(
    triage: &[u8],
    ready: &[String],
    ready_priorities: &BTreeMap<String, u64>,
    insights: &[u8],
) -> Result<RankedOrder, String> {
    let triage_value: Value = serde_json::from_slice(triage)
        .map_err(|error| format!("QUEUE_UNRANKED bv triage JSON: {error}"))?;
    let recs = triage_value
        .get("triage")
        .and_then(|t| t.get("recommendations"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            "QUEUE_UNRANKED bv triage has no .triage.recommendations array".to_owned()
        })?;
    if recs.is_empty() {
        return Err("QUEUE_UNRANKED bv triage .triage.recommendations is empty".to_owned());
    }
    if ready.is_empty() {
        return Err("QUEUE_UNRANKED bv ready set is empty".to_owned());
    }

    let insights_value: Value = serde_json::from_slice(insights)
        .map_err(|error| format!("QUEUE_UNRANKED bv insights JSON: {error}"))?;
    let triage_hash = triage_value
        .get("data_hash")
        .and_then(Value::as_str)
        .ok_or_else(|| "QUEUE_UNRANKED bv triage has no data_hash".to_owned())?;
    let insights_hash = insights_value
        .get("data_hash")
        .and_then(Value::as_str)
        .ok_or_else(|| "QUEUE_UNRANKED bv insights has no data_hash".to_owned())?;
    if triage_hash != insights_hash {
        return Err(format!(
            "QUEUE_UNRANKED bv snapshot mismatch triage={triage_hash} insights={insights_hash}"
        ));
    }
    let page_rank_state = insights_value
        .get("status")
        .and_then(|status| status.get("PageRank"))
        .and_then(|page_rank| page_rank.get("state"))
        .and_then(Value::as_str)
        .ok_or_else(|| "QUEUE_UNRANKED bv insights has no PageRank status".to_owned())?;
    if page_rank_state != "computed" {
        return Err(format!(
            "QUEUE_UNRANKED bv insights PageRank state={page_rank_state}"
        ));
    }
    let page_ranks = insights_value
        .get("full_stats")
        .and_then(|stats| stats.get("pagerank"))
        .and_then(Value::as_object)
        .ok_or_else(|| "QUEUE_UNRANKED bv insights has no full_stats.pagerank map".to_owned())?;
    if page_ranks.is_empty() {
        return Err("QUEUE_UNRANKED bv insights full_stats.pagerank is empty".to_owned());
    }

    let mut ranked: Vec<(i64, u64, String)> = Vec::with_capacity(ready.len());
    let mut scored = 0usize;
    for id in ready {
        let score = match page_ranks.get(id) {
            Some(value) => value.as_f64().ok_or_else(|| {
                format!("QUEUE_UNRANKED bv insights PageRank is not numeric for {id}")
            })?,
            // bv omits zero-valued nodes from its sparse metric map.
            None => 0.0,
        };
        if !score.is_finite() || score < 0.0 {
            return Err(format!(
                "QUEUE_UNRANKED bv insights PageRank is invalid for {id}: {score}"
            ));
        }
        if score > 0.0 {
            scored += 1;
        }
        let priority = ready_priorities
            .get(id)
            .copied()
            .ok_or_else(|| format!("QUEUE_UNRANKED br ready row has no priority for {id}"))?;
        let scaled = score * 1_000_000_000_000_000.0;
        if !scaled.is_finite() || scaled > i64::MAX as f64 {
            return Err(format!(
                "QUEUE_UNRANKED bv insights PageRank is out of range for {id}: {score}"
            ));
        }
        ranked.push((-(scaled.round() as i64), priority, id.clone()));
    }
    if scored == 0 {
        return Err(
            "QUEUE_UNRANKED bv insights has no nonzero PageRank score in the ready set".to_owned(),
        );
    }
    ranked.sort();
    let ids = ranked.into_iter().map(|(_, _, id)| id).collect::<Vec<_>>();
    Ok(RankedOrder {
        ids,
        window: RankWindow::RecommendationsOnly,
        scored,
    })
}

fn assigned_ids(triage: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Some(recs) = triage
        .get("triage")
        .and_then(|t| t.get("recommendations"))
        .and_then(Value::as_array)
    else {
        return out;
    };
    for row in recs {
        let Some(id) = row.get("id").and_then(Value::as_str) else {
            continue;
        };
        if row
            .get("assignee")
            .and_then(Value::as_str)
            .is_some_and(|who| !who.trim().is_empty())
        {
            out.insert(id.to_owned());
        }
    }
    out
}

fn first_actionable_blocker(
    triage: &Value,
    ready: &[String],
    assigned: &BTreeSet<String>,
) -> Result<Option<String>, String> {
    let blockers = triage
        .get("triage")
        .and_then(|t| t.get("blockers_to_clear"))
        .ok_or_else(|| {
            "QUEUE_UNRANKED bv triage has no .triage.blockers_to_clear array".to_owned()
        })?;
    let blockers = blockers.as_array().ok_or_else(|| {
        "QUEUE_UNRANKED bv triage .triage.blockers_to_clear is not an array".to_owned()
    })?;
    for row in blockers {
        let Some(id) = row.get("id").and_then(Value::as_str) else {
            continue;
        };
        let actionable = row
            .get("actionable")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let unblocks = row
            .get("unblocks_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if !actionable || unblocks == 0 {
            continue;
        }
        if !ready.iter().any(|candidate| candidate == id) {
            continue;
        }
        if assigned.contains(id) {
            continue;
        }
        return Ok(Some(id.to_owned()));
    }
    Ok(None)
}

fn strip_nonsemantic(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut output = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        let quote = chars[index];
        if !matches!(quote, '\'' | '"' | '\u{60}') {
            output.push(quote);
            index += 1;
            continue;
        }
        let mut end = index + 1;
        let mut escaped = false;
        while end < chars.len() {
            if quote != '\u{60}' && chars[end] == '\n' {
                break;
            }
            if chars[end] == quote && !escaped {
                break;
            }
            if chars[end] == '\\' {
                escaped = !escaped;
            } else {
                escaped = false;
            }
            end += 1;
        }
        if end < chars.len() && chars[end] == quote {
            for _ in index..=end {
                output.push(' ');
            }
            index = end + 1;
        } else {
            output.push(quote);
            index += 1;
        }
    }
    output
}

fn filed_only_marker(text: &str) -> bool {
    let lower = strip_nonsemantic(text).to_ascii_lowercase();
    if lower.contains("file not claim") || lower.contains("filed only") {
        return true;
    }
    lower.lines().any(|line| {
        let line = line.trim_start();
        (line.starts_with("status:") || line.starts_with("stage:") || line.starts_with("marker:"))
            && line.contains("do not claim")
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BeadComments {
    id: String,
    status: String,
    assignee: String,
    filed_only: bool,
    authors: Vec<String>,
    texts: Vec<String>,
}

fn parse_jsonl(jsonl: &str) -> Result<Vec<BeadComments>, String> {
    let mut out = Vec::new();
    for (i, line) in jsonl.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line)
            .map_err(|error| format!("QUEUE_UNRANKED comments jsonl line {i}: {error}"))?;
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let status = value
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let assignee = value
            .get("assignee")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_owned();
        let description = value
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("");
        let acceptance = value
            .get("acceptance_criteria")
            .and_then(Value::as_str)
            .unwrap_or("");
        let filed_only = filed_only_marker(description) || filed_only_marker(acceptance);
        let mut authors = Vec::new();
        let mut texts = Vec::new();
        if let Some(comments) = value.get("comments").and_then(Value::as_array) {
            for c in comments {
                if let Some(author) = c.get("author").and_then(Value::as_str) {
                    authors.push(author.to_owned());
                }
                if let Some(text) = c.get("text").and_then(Value::as_str) {
                    texts.push(text.to_owned());
                }
            }
        }
        out.push(BeadComments {
            id,
            status,
            assignee,
            filed_only,
            authors,
            texts,
        });
    }
    Ok(out)
}

fn dispatchable_ready_ids(ready: &[String], jsonl: &str) -> Result<Vec<String>, String> {
    let filed_ids: BTreeSet<String> = parse_jsonl(jsonl)?
        .into_iter()
        .filter(|bead| bead.filed_only)
        .map(|bead| bead.id)
        .collect();
    Ok(ready
        .iter()
        .filter(|id| !filed_ids.contains(*id))
        .cloned()
        .collect())
}

fn done_evidence(bead: &BeadComments) -> bool {
    bead.texts.iter().any(|t| {
        t.contains("MUTATION-VERIFIED")
            || t.contains("STAGE: IMPL -> GRADING")
            || t.contains("DONE ")
            || t.starts_with("DONE")
    })
}

/// Open or in_progress with DONE evidence. Closed, grading, blocked, and
/// tombstone are never reapable. grading is terminal for this slot: the
/// bead is already out for a grader; re-offering is a second dispatch.
fn is_reapable(bead: &BeadComments) -> bool {
    !bead.filed_only
        && matches!(bead.status.as_str(), "open" | "in_progress")
        && done_evidence(bead)
}

fn pane_scoped_authors(bead: &BeadComments) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    if pane_scoped(&bead.assignee) {
        out.insert(bead.assignee.clone());
    }
    for a in &bead.authors {
        if pane_scoped(a) {
            out.insert(a.clone());
        }
    }
    out
}

/// Outcome of [`comment_count`]. Absence is a typed variant, never a silent zero,
/// so no caller can flatten a missing row into a healthy count with `unwrap_or(0)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentCount {
    /// Row present with this many comments (0 for a comment-free row).
    Found(usize),
    /// No row carries the requested id (includes empty documents).
    RowAbsent { id: String },
}

/// Count comments for `id`, distinguishing absence from a present zero.
///
/// Returns [`CommentCount::Found`] for a present row and [`CommentCount::RowAbsent`]
/// when no row carries `id`. Malformed JSONL remains a parse error.
pub fn comment_count(jsonl: &str, id: &str) -> Result<CommentCount, String> {
    let beads = parse_jsonl(jsonl)?;
    Ok(beads
        .iter()
        .find(|b| b.id == id)
        .map(|b| CommentCount::Found(b.authors.len().max(b.texts.len())))
        .unwrap_or_else(|| CommentCount::RowAbsent {
            id: id.to_owned(),
        }))
}

/// Grading priority is an optimisation over an already-correct ranked order.
/// Unreadable JSONL / no eligible grader SKIP the slot. They do not refuse
/// the cycle. Ranking refusals stay in [`select_dispatch_order`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GradingSlot {
    Offer(String),
    Skip { reason: &'static str },
}

impl fmt::Display for GradingSlot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Offer(id) => write!(formatter, "GRADING_SLOT_OFFER bead={id}"),
            Self::Skip { reason } => write!(formatter, "GRADING_SLOT_SKIP reason={reason}"),
        }
    }
}

pub fn grading_slot(jsonl: &str, ready: &[String], grader: &str) -> GradingSlot {
    let beads = match parse_jsonl(jsonl) {
        Ok(beads) => beads,
        Err(_) => {
            return GradingSlot::Skip {
                reason: "unreadable_jsonl",
            }
        }
    };
    if beads.is_empty() {
        return GradingSlot::Skip {
            reason: "empty_grading_slot",
        };
    }
    let ledger = BTreeMap::new();
    let reapable: Vec<&BeadComments> = beads
        .iter()
        .filter(|bead| ready.iter().any(|id| id == &bead.id) && is_reapable(bead))
        .collect();
    if reapable.is_empty() {
        return GradingSlot::Skip {
            reason: "no_reapable_bead",
        };
    }
    if !pane_scoped(grader) {
        return GradingSlot::Skip {
            reason: "grader_not_pane_scoped",
        };
    }
    let mut saw_empty_authors = false;
    let mut saw_self = false;
    for id in ready {
        let Some(bead) = reapable.iter().find(|bead| &bead.id == id) else {
            continue;
        };
        let authors = author_panes(bead, &ledger);
        if authors.is_empty() {
            saw_empty_authors = true;
            continue;
        }
        if authors_include_pane(&authors, grader) || authors.contains(grader) {
            saw_self = true;
            continue;
        }
        return GradingSlot::Offer(id.clone());
    }
    if saw_empty_authors {
        return GradingSlot::Skip {
            reason: "grader_identity_unresolved",
        };
    }
    if saw_self {
        return GradingSlot::Skip {
            reason: "no_distinct_idle_peer",
        };
    }
    GradingSlot::Skip {
        reason: "no_reapable_bead",
    }
}

fn first_grading_assignment(jsonl: &str, ready: &[String], grader: &str) -> Option<String> {
    match grading_slot(jsonl, ready, grader) {
        GradingSlot::Offer(id) => Some(id),
        GradingSlot::Skip { .. } => None,
    }
}

/// Full dispatch order. grader must be pane-scoped (pane4-%9) to take a grading slot.
pub fn select_dispatch_order(
    triage: &[u8],
    ready: &[String],
    jsonl: &str,
    grader: &str,
) -> Result<Vec<String>, String> {
    let value: Value = serde_json::from_slice(triage)
        .map_err(|error| format!("QUEUE_UNRANKED bv triage JSON: {error}"))?;
    let dispatchable_ready = dispatchable_ready_ids(ready, jsonl)?;
    let assigned = assigned_ids(&value);
    let mut out: Vec<String> = Vec::new();
    if let Some(blocker) = first_actionable_blocker(&value, &dispatchable_ready, &assigned)? {
        out.push(blocker);
    }
    if let Some(grade) = first_grading_assignment(jsonl, &dispatchable_ready, grader) {
        if !out.iter().any(|id| id == &grade) {
            out.push(grade);
        }
    }
    let ranked = rank_ready(triage, &dispatchable_ready)?;
    for id in ranked {
        if !out.iter().any(|chosen| chosen == &id) {
            out.push(id);
        }
    }
    Ok(out)
}

/// Full dispatch order with authoritative ready-set priorities.
pub fn select_dispatch_order_with_priorities(
    triage: &[u8],
    ready: &[String],
    ready_priorities: &BTreeMap<String, u64>,
    jsonl: &str,
    grader: &str,
) -> Result<RankedOrder, String> {
    let value: Value = serde_json::from_slice(triage)
        .map_err(|error| format!("QUEUE_UNRANKED bv triage JSON: {error}"))?;
    let dispatchable_ready = dispatchable_ready_ids(ready, jsonl)?;
    let assigned = assigned_ids(&value);
    let mut out: Vec<String> = Vec::new();
    if let Some(blocker) = first_actionable_blocker(&value, &dispatchable_ready, &assigned)? {
        out.push(blocker);
    }
    if let Some(grade) = first_grading_assignment(jsonl, &dispatchable_ready, grader) {
        if !out.iter().any(|id| id == &grade) {
            out.push(grade);
        }
    }
    let ranked = rank_ready_with_priorities(triage, &dispatchable_ready, ready_priorities)?;
    let scored = ranked.scored;
    let window = if !ready.is_empty() && dispatchable_ready.is_empty() {
        RankWindow::FilteredFiledOnly
    } else {
        ranked.window
    };
    for id in ranked.ids {
        if !out.iter().any(|chosen| chosen == &id) {
            out.push(id);
        }
    }
    Ok(RankedOrder {
        ids: out,
        window,
        scored,
    })
}

/// Full dispatch order using bv's complete PageRank ready frontier.
pub fn select_dispatch_order_with_pagerank(
    triage: &[u8],
    ready: &[String],
    ready_priorities: &BTreeMap<String, u64>,
    insights: &[u8],
    jsonl: &str,
    grader: &str,
) -> Result<RankedOrder, String> {
    let value: Value = serde_json::from_slice(triage)
        .map_err(|error| format!("QUEUE_UNRANKED bv triage JSON: {error}"))?;
    let dispatchable_ready = dispatchable_ready_ids(ready, jsonl)?;
    let assigned = assigned_ids(&value);
    let mut out: Vec<String> = Vec::new();
    if let Some(blocker) = first_actionable_blocker(&value, &dispatchable_ready, &assigned)? {
        out.push(blocker);
    }
    if let Some(grade) = first_grading_assignment(jsonl, &dispatchable_ready, grader) {
        if !out.iter().any(|id| id == &grade) {
            out.push(grade);
        }
    }
    if !ready.is_empty() && dispatchable_ready.is_empty() {
        return Ok(RankedOrder {
            ids: out,
            window: RankWindow::FilteredFiledOnly,
            scored: 0,
        });
    }
    let ranked = rank_ready_with_pagerank(triage, &dispatchable_ready, ready_priorities, insights)?;
    let scored = ranked.scored;
    let window = ranked.window;
    for id in ranked.ids {
        if !out.iter().any(|chosen| chosen == &id) {
            out.push(id);
        }
    }
    Ok(RankedOrder {
        ids: out,
        window,
        scored,
    })
}

/// One pane as the assignment path sees it. The observer may be WORKING;
/// the grader must not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedPane {
    pub pane_id: String,
    pub liveness: String,
    pub is_dispatchable: bool,
    pub is_working: bool,
}

impl ObservedPane {
    fn is_confirmed_idle(&self) -> bool {
        self.is_dispatchable && self.liveness == "CONFIRMED_IDLE" && !self.is_working
    }
}

/// Successful grading assignment. Observer and grader are different panes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GradeAssignment {
    pub bead: String,
    pub grader_pane: String,
    pub grader_assignee: String,
    pub observer_pane: String,
}

/// Typed refusals for the dispatch-based grading path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignGradeError {
    EmptyObservation,
    ObserverPaneUnresolved,
    NoEligibleGrader {
        reason: &'static str,
    },
    GraderCarryingOwnDispatch {
        pane: String,
        liveness: String,
        is_working: bool,
    },
    GraderIdentityUnresolved {
        detail: String,
    },
    Jsonl(String),
}

impl fmt::Display for AssignGradeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyObservation => formatter.write_str(
                "EMPTY_OBSERVATION no eligible grader can be decided from an empty pane set",
            ),
            Self::ObserverPaneUnresolved => {
                formatter.write_str("PEER_GRADING_REFUSED reason=observer_pane_unresolved")
            }
            Self::NoEligibleGrader { reason } => {
                write!(formatter, "NO_ELIGIBLE_GRADER reason={reason}")
            }
            Self::GraderCarryingOwnDispatch {
                pane,
                liveness,
                is_working,
            } => write!(
                formatter,
                "GRADER_CARRYING_OWN_DISPATCH pane={pane} liveness={liveness} is_working={is_working}"
            ),
            Self::GraderIdentityUnresolved { detail } => {
                write!(formatter, "GRADER_IDENTITY_UNRESOLVED {detail}")
            }
            Self::Jsonl(detail) => formatter.write_str(detail),
        }
    }
}

/// `pane9-%9` from tmux pane id `%9`. Unique on one tmux server.
pub fn pane_assignee_key(pane_id: &str) -> Option<String> {
    let id = pane_id.trim();
    let rest = id.strip_prefix('%')?;
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(format!("pane{rest}-{id}"))
}

fn ack_pane(text: &str) -> Option<String> {
    if !text.starts_with("ACK ") {
        return None;
    }
    let marker = " on %";
    let start = text.find(marker)?;
    let rest = &text[start + " on ".len()..];
    let end = rest.find(" --")?;
    let pane = &rest[..end];
    let digits = pane.strip_prefix('%')?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(pane.to_owned())
}

fn author_panes(
    bead: &BeadComments,
    ledger: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeSet<String> {
    let mut out = pane_scoped_authors(bead);
    for text in &bead.texts {
        if let Some(pane) = ack_pane(text) {
            out.insert(pane.clone());
            if let Some(key) = pane_assignee_key(&pane) {
                out.insert(key);
            }
        }
    }
    if let Some(panes) = ledger.get(&bead.id) {
        for pane in panes {
            out.insert(pane.clone());
            if let Some(key) = pane_assignee_key(pane) {
                out.insert(key);
            }
        }
    }
    out
}

fn tmux_pane_id(raw: &str) -> Option<String> {
    let pane = raw.trim();
    let digits = pane.strip_prefix('%')?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(pane.to_owned())
}

fn push_pane_field(out: &mut BTreeSet<String>, value: Option<&Value>) {
    if let Some(pane) = value.and_then(Value::as_str).and_then(tmux_pane_id) {
        out.insert(pane);
    }
}

/// Third author source: lifecycle ledger `grader_pane` and `pane` fields.
pub fn parse_ledger_authors(ledger_jsonl: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for line in ledger_jsonl.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let bead = value
            .get("bead")
            .and_then(Value::as_str)
            .or_else(|| value.pointer("/identity/bead").and_then(Value::as_str))
            .unwrap_or("")
            .trim();
        if bead.is_empty() {
            continue;
        }
        let panes = out.entry(bead.to_owned()).or_default();
        push_pane_field(panes, value.get("grader_pane"));
        push_pane_field(panes, value.get("pane"));
        push_pane_field(panes, value.get("receiver_pane"));
        push_pane_field(panes, value.pointer("/identity/target/pane"));
        push_pane_field(panes, value.pointer("/target/pane"));
        push_pane_field(panes, value.pointer("/evidence/grader_pane"));
        push_pane_field(panes, value.pointer("/evidence/pane"));
        push_pane_field(panes, value.pointer("/evidence/receiver_pane"));
        if panes.is_empty() {
            out.remove(bead);
        }
    }
    out
}

fn authors_include_pane(authors: &BTreeSet<String>, pane_id: &str) -> bool {
    if authors.contains(pane_id) {
        return true;
    }
    if let Some(key) = pane_assignee_key(pane_id) {
        if authors.contains(&key) {
            return true;
        }
    }
    authors
        .iter()
        .any(|author| pane_scoped(author) && author.ends_with(pane_id))
}

fn parse_one_pane(value: &Value) -> Option<ObservedPane> {
    let pane_id = value
        .get("pane_id")
        .or_else(|| value.get("pane"))
        .and_then(Value::as_str)?
        .trim()
        .to_owned();
    if pane_id.is_empty() {
        return None;
    }
    let liveness = value
        .get("liveness")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let state = value.get("state").and_then(Value::as_str).unwrap_or("");
    let is_working = value
        .get("is_working")
        .and_then(Value::as_bool)
        .unwrap_or(state == "WORKING" || liveness == "LIVE");
    let is_dispatchable = value
        .get("is_dispatchable")
        .and_then(Value::as_bool)
        .unwrap_or(liveness == "CONFIRMED_IDLE");
    Some(ObservedPane {
        pane_id,
        liveness,
        is_dispatchable,
        is_working,
    })
}

/// Parse a compact pane array or a tick-monitor `omp_lifecycle.panes` envelope.
pub fn parse_observed_panes(bytes: &[u8]) -> Result<Vec<ObservedPane>, AssignGradeError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        AssignGradeError::Jsonl(format!("ASSIGN_GRADE_REFUSED observation JSON: {error}"))
    })?;
    let arrays = [
        value.get("panes").and_then(Value::as_array),
        value
            .pointer("/omp_lifecycle/panes")
            .and_then(Value::as_array),
    ];
    let mut panes = Vec::new();
    for array in arrays.into_iter().flatten() {
        for row in array {
            if let Some(pane) = parse_one_pane(row) {
                panes.push(pane);
            }
        }
        if !panes.is_empty() {
            break;
        }
    }
    if panes.is_empty() {
        return Err(AssignGradeError::EmptyObservation);
    }
    Ok(panes)
}

/// Refuse a grader that is carrying its own dispatch. This is the guarantee
/// `current_pane_not_confirmed_idle` provided, applied to the TARGET pane.
pub fn require_idle_grader(
    grader_pane: &str,
    panes: &[ObservedPane],
) -> Result<(), AssignGradeError> {
    let Some(pane) = panes.iter().find(|pane| pane.pane_id == grader_pane) else {
        return Err(AssignGradeError::NoEligibleGrader {
            reason: "grader_observation_row_missing",
        });
    };
    if !pane.is_confirmed_idle() {
        return Err(AssignGradeError::GraderCarryingOwnDispatch {
            pane: grader_pane.to_owned(),
            liveness: pane.liveness.clone(),
            is_working: pane.is_working,
        });
    }
    Ok(())
}

/// Pick a grader that is not the observer. The observer may be WORKING.
pub fn assign_peer_grade(
    observer_pane: &str,
    panes: &[ObservedPane],
    jsonl: &str,
) -> Result<GradeAssignment, AssignGradeError> {
    assign_peer_grade_with_ledger(observer_pane, panes, jsonl, "")
}

/// Same as [`assign_peer_grade`], with lifecycle-ledger pane attribution.
pub fn assign_peer_grade_with_ledger(
    observer_pane: &str,
    panes: &[ObservedPane],
    jsonl: &str,
    ledger_jsonl: &str,
) -> Result<GradeAssignment, AssignGradeError> {
    if observer_pane.trim().is_empty() {
        return Err(AssignGradeError::ObserverPaneUnresolved);
    }
    if panes.is_empty() {
        return Err(AssignGradeError::EmptyObservation);
    }
    let others: Vec<&ObservedPane> = panes
        .iter()
        .filter(|pane| pane.pane_id != observer_pane)
        .collect();
    let idle: Vec<&ObservedPane> = others
        .iter()
        .copied()
        .filter(|pane| pane.is_confirmed_idle())
        .collect();
    if idle.is_empty() {
        let reason = if !others.is_empty() && others.iter().all(|pane| pane.is_working) {
            "all_panes_carrying_dispatches"
        } else {
            "no_idle_pane"
        };
        return Err(AssignGradeError::NoEligibleGrader { reason });
    }
    let beads = parse_jsonl(jsonl).map_err(AssignGradeError::Jsonl)?;
    let ledger = parse_ledger_authors(ledger_jsonl);
    let reapable: Vec<&BeadComments> = beads.iter().filter(|bead| is_reapable(bead)).collect();
    if reapable.is_empty() {
        return Err(AssignGradeError::NoEligibleGrader {
            reason: "empty_reapable_set",
        });
    }
    let mut missing_attribution: BTreeSet<String> = BTreeSet::new();
    for grader in &idle {
        require_idle_grader(&grader.pane_id, panes)?;
        let Some(grader_assignee) = pane_assignee_key(&grader.pane_id) else {
            return Err(AssignGradeError::GraderIdentityUnresolved {
                detail: format!("grader_pane={} is not a tmux pane id", grader.pane_id),
            });
        };
        for bead in &reapable {
            let authors = author_panes(bead, &ledger);
            if authors.is_empty() {
                missing_attribution.insert(bead.id.clone());
                continue;
            }
            if authors_include_pane(&authors, &grader.pane_id) {
                continue;
            }
            return Ok(GradeAssignment {
                bead: bead.id.clone(),
                grader_pane: grader.pane_id.clone(),
                grader_assignee,
                observer_pane: observer_pane.to_owned(),
            });
        }
    }
    if !missing_attribution.is_empty() {
        return Err(AssignGradeError::GraderIdentityUnresolved {
            detail: format!("missing_attribution count={}", missing_attribution.len()),
        });
    }
    Err(AssignGradeError::NoEligibleGrader {
        reason: "no_distinct_idle_peer",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranking_prefers_a_p0_articulation_point_over_an_older_p1_leaf() {
        let ready = vec!["old-p1-leaf".to_owned(), "p0-articulation".to_owned()];
        let triage = br#"{"triage":{"recommendations":[
            {"id":"old-p1-leaf","priority":1,"score":0.9},
            {"id":"p0-articulation","priority":0,"score":0.2}
        ]}}"#;
        let ranked = rank_ready(triage, &ready).expect("ranked");
        assert_eq!(
            ranked.first().map(String::as_str),
            Some("p0-articulation"),
            "priority outranks both creation order and a higher graph score: {ranked:?}"
        );
    }

    #[test]
    fn within_a_priority_the_graph_score_decides() {
        let ready = vec!["low".to_owned(), "high".to_owned()];
        let triage = br#"{"triage":{"recommendations":[
            {"id":"low","priority":0,"score":0.10},
            {"id":"high","priority":0,"score":0.80}
        ]}}"#;
        let ranked = rank_ready(triage, &ready).expect("ranked");
        assert_eq!(ranked.first().map(String::as_str), Some("high"));
    }

    #[test]
    fn epics_and_assigned_rows_are_not_ranked_but_are_not_lost() {
        let ready = vec!["epic".to_owned(), "taken".to_owned(), "free".to_owned()];
        let triage = br#"{"triage":{"recommendations":[
            {"id":"epic","priority":0,"score":0.99,"type":"epic"},
            {"id":"taken","priority":0,"score":0.98,"assignee":"pane3-%8"},
            {"id":"free","priority":2,"score":0.01}
        ]}}"#;
        let ranked = rank_ready(triage, &ready).expect("ranked");
        assert_eq!(ranked.first().map(String::as_str), Some("free"));
        assert_eq!(ranked.len(), 3, "no ready id may be dropped: {ranked:?}");
    }

    #[test]
    fn priority_fallback_uses_ready_priority_when_recommendations_do_not_survive() {
        let ready = vec!["older-p1".to_owned(), "p0-now".to_owned()];
        let triage = br#"{"triage":{"recommendations":[
            {"id":"older-p1","priority":1,"score":0.9,"type":"epic"},
            {"id":"p0-now","priority":0,"score":0.1,"assignee":"pane3-%8"}
        ]}}"#;
        let priorities = BTreeMap::from([("older-p1".to_owned(), 1), ("p0-now".to_owned(), 0)]);
        let ranked = rank_ready_with_priorities(triage, &ready, &priorities).unwrap();
        assert_eq!(ranked.window, RankWindow::PriorityFallback);
        assert_eq!(ranked.ids, vec!["p0-now", "older-p1"]);
    }

    #[test]
    fn surviving_recommendations_still_use_priority_then_score() {
        let ready = vec!["p1".to_owned(), "p0".to_owned()];
        let triage = br#"{"triage":{"recommendations":[
            {"id":"p1","priority":1,"score":0.99},
            {"id":"p0","priority":0,"score":0.01}
        ]}}"#;
        let priorities = BTreeMap::from([("p1".to_owned(), 1), ("p0".to_owned(), 0)]);
        let ranked = rank_ready_with_priorities(triage, &ready, &priorities).unwrap();
        assert_eq!(ranked.window, RankWindow::RecommendationsOnly);
        assert_eq!(ranked.ids, vec!["p0", "p1"]);
    }

    #[test]
    fn empty_ready_set_has_a_typed_empty_rank_window() {
        let ranked = rank_ready_with_priorities(
            br#"{"triage":{"recommendations":[{"id":"not-ready","score":1.0}]}}"#,
            &[],
            &BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(ranked.window, RankWindow::EmptyReady);
        assert!(ranked.ids.is_empty());
    }

    #[test]
    fn pagerank_reaches_ready_beads_beyond_top_ten_recommendations() {
        let ready = vec!["low".to_owned(), "high".to_owned()];
        let priorities = BTreeMap::from([("low".to_owned(), 1), ("high".to_owned(), 3)]);
        let triage = br#"{"data_hash":"fixture","triage":{"recommendations":[
            {"id":"blocked","status":"blocked","score":0.99}
        ]}}"#;
        let insights = br#"{"data_hash":"fixture","status":{"PageRank":{"state":"computed"}},
            "full_stats":{"pagerank":{"low":0.1,"high":0.9}}}"#;
        let ranked = rank_ready_with_pagerank(triage, &ready, &priorities, insights).unwrap();
        assert_eq!(ranked.window, RankWindow::RecommendationsOnly);
        assert_eq!(ranked.ids, vec!["high", "low"]);
        assert_eq!(ranked.scored, 2);
    }

    #[test]
    fn pagerank_dispatch_order_is_not_br_priority_order() {
        let ready = vec!["low".to_owned(), "high".to_owned()];
        let priorities = BTreeMap::from([("low".to_owned(), 0), ("high".to_owned(), 3)]);
        let triage =
            br#"{"data_hash":"fixture","triage":{"blockers_to_clear":[],"recommendations":[
            {"id":"blocked","status":"blocked","score":0.99}
        ]}}"#;
        let insights = br#"{"data_hash":"fixture","status":{"PageRank":{"state":"computed"}},
            "full_stats":{"pagerank":{"low":0.1,"high":0.9}}}"#;
        let ordered = select_dispatch_order_with_pagerank(
            triage,
            &ready,
            &priorities,
            insights,
            empty_jsonl(),
            "pane4-%9",
        )
        .unwrap();
        assert_eq!(ordered.window, RankWindow::RecommendationsOnly);
        assert_eq!(ordered.ids, vec!["high", "low"]);
        assert_eq!(ordered.scored, 2);
    }

    #[test]
    fn equal_pagerank_uses_priority_before_id() {
        let ready = vec!["a-p3".to_owned(), "z-p0".to_owned()];
        let priorities = BTreeMap::from([("a-p3".to_owned(), 3), ("z-p0".to_owned(), 0)]);
        let triage = br#"{"data_hash":"fixture","triage":{"recommendations":[{"id":"anchor"}]}}"#;
        let insights = br#"{"data_hash":"fixture","status":{"PageRank":{"state":"computed"}},"full_stats":{"pagerank":{"a-p3":0.5,"z-p0":0.5}}}"#;
        let ranked = rank_ready_with_pagerank(triage, &ready, &priorities, insights).unwrap();
        assert_eq!(
            ranked.ids,
            vec!["z-p0", "a-p3"],
            "equal scores must use priority: {ranked:?}"
        );
        assert_eq!(ranked.scored, 2);
    }

    #[test]
    fn higher_pagerank_outranks_priority() {
        let ready = vec!["p3".to_owned(), "p0".to_owned()];
        let priorities = BTreeMap::from([("p3".to_owned(), 3), ("p0".to_owned(), 0)]);
        let triage = br#"{"data_hash":"fixture","triage":{"recommendations":[{"id":"anchor"}]}}"#;
        let insights = br#"{"data_hash":"fixture","status":{"PageRank":{"state":"computed"}},"full_stats":{"pagerank":{"p3":0.9,"p0":0.1}}}"#;
        let ranked = rank_ready_with_pagerank(triage, &ready, &priorities, insights).unwrap();
        assert_eq!(
            ranked.ids,
            vec!["p3", "p0"],
            "graph score remains primary: {ranked:?}"
        );
    }

    #[test]
    fn current_live_scored_head_is_pinned() {
        let ready = vec![
            "omp-orchestrator-kldh".to_owned(),
            "omp-orchestrator-plan-04-7wn9.3".to_owned(),
            "omp-orchestrator-plan-11-vcd7.7".to_owned(),
        ];
        let priorities = BTreeMap::from([
            ("omp-orchestrator-kldh".to_owned(), 1),
            ("omp-orchestrator-plan-04-7wn9.3".to_owned(), 2),
            ("omp-orchestrator-plan-11-vcd7.7".to_owned(), 2),
        ]);
        let triage = br#"{"data_hash":"fixture","triage":{"recommendations":[{"id":"anchor"}]}}"#;
        let insights = br#"{"data_hash":"fixture","status":{"PageRank":{"state":"computed"}},"full_stats":{"pagerank":{"omp-orchestrator-kldh":0.00107696675300789,"omp-orchestrator-plan-04-7wn9.3":0.000984621986015491,"omp-orchestrator-plan-11-vcd7.7":0.0009649276036815638}}}"#;
        let ranked = rank_ready_with_pagerank(triage, &ready, &priorities, insights).unwrap();
        assert_eq!(
            ranked.ids,
            vec![
                "omp-orchestrator-kldh",
                "omp-orchestrator-plan-04-7wn9.3",
                "omp-orchestrator-plan-11-vcd7.7",
            ]
        );
        assert_eq!(ranked.scored, 3);
    }

    #[test]
    fn empty_or_all_zero_pagerank_is_a_typed_refusal() {
        let priorities = BTreeMap::from([("p0".to_owned(), 0)]);
        let triage = br#"{"data_hash":"fixture","triage":{"recommendations":[{"id":"anchor"}]}}"#;
        let insights = br#"{"data_hash":"fixture","status":{"PageRank":{"state":"computed"}},"full_stats":{"pagerank":{"p0":0.0}}}"#;
        let empty = rank_ready_with_pagerank(triage, &[], &priorities, insights)
            .expect_err("empty ready set must refuse");
        assert!(empty.contains("ready set is empty"), "{empty}");
        let zero = rank_ready_with_pagerank(triage, &["p0".to_owned()], &priorities, insights)
            .expect_err("all-zero PageRank must refuse");
        assert!(zero.contains("no nonzero PageRank"), "{zero}");
    }

    #[test]
    fn missing_recommendations_is_a_typed_refusal_not_silent_fifo() {
        let ready = vec!["a".to_owned()];
        for payload in [
            &br#"{"triage":{}}"#[..],
            &br#"{"triage":{"quick_ref":{"top_picks":["a"]}}}"#[..],
            &br#"{"triage":{"recommendations":[]}}"#[..],
            &br#"{}"#[..],
        ] {
            let error = rank_ready(payload, &ready).expect_err("must refuse");
            assert!(
                error.starts_with("QUEUE_UNRANKED"),
                "refusal must be typed and named: {error}"
            );
        }
    }

    fn empty_jsonl() -> &'static str {
        ""
    }

    fn triage_with_blockers(blockers: &str, recs: &str) -> Vec<u8> {
        format!(r#"{{"triage":{{"blockers_to_clear":{blockers},"recommendations":{recs}}}}}"#)
            .into_bytes()
    }

    #[test]
    fn actionable_blocker_outranks_a_p0_leaf() {
        let ready = vec!["p0-leaf".to_owned(), "blocker".to_owned()];
        let triage = triage_with_blockers(
            r#"[{"id":"blocker","actionable":true,"unblocks_count":6}]"#,
            r#"[{"id":"p0-leaf","priority":0,"score":0.99},{"id":"blocker","priority":1,"score":0.01}]"#,
        );
        let ordered = select_dispatch_order(&triage, &ready, empty_jsonl(), "pane4-%9").unwrap();
        assert_eq!(ordered.first().map(String::as_str), Some("blocker"));
        assert_eq!(
            rank_ready(&triage, &ready)
                .unwrap()
                .first()
                .map(String::as_str),
            Some("p0-leaf")
        );
    }

    #[test]
    fn nonactionable_blocker_is_not_picked() {
        let ready = vec!["p0-leaf".to_owned(), "blocker".to_owned()];
        let triage = triage_with_blockers(
            r#"[{"id":"blocker","actionable":false,"unblocks_count":6}]"#,
            r#"[{"id":"p0-leaf","priority":0,"score":0.99},{"id":"blocker","priority":1,"score":0.01}]"#,
        );
        let ordered = select_dispatch_order(&triage, &ready, empty_jsonl(), "pane4-%9").unwrap();
        assert_ne!(ordered.first().map(String::as_str), Some("blocker"));
        assert_eq!(ordered.first().map(String::as_str), Some("p0-leaf"));
    }

    #[test]
    fn missing_blockers_to_clear_is_queue_unranked() {
        let ready = vec!["a".to_owned()];
        let triage = br#"{"triage":{"recommendations":[{"id":"a","priority":0,"score":1.0}]}}"#;
        let error = select_dispatch_order(triage, &ready, empty_jsonl(), "pane4-%9")
            .expect_err("must refuse");
        assert!(error.starts_with("QUEUE_UNRANKED"), "{error}");
        assert!(error.contains("blockers_to_clear"), "{error}");
    }

    fn reapable_jsonl(id: &str, assignee: &str, author: &str) -> String {
        format!(
            r#"{{"id":"{id}","status":"in_progress","assignee":"{assignee}","comments":[{{"author":"{author}","text":"STAGE: IMPL -> GRADING. sha abc. Re-run: cargo test."}}]}}"#
        )
    }

    fn reapable_jsonl_status(id: &str, status: &str, assignee: &str, author: &str) -> String {
        format!(
            r#"{{"id":"{id}","status":"{status}","assignee":"{assignee}","comments":[{{"author":"{author}","text":"DONE work"}}]}}"#
        )
    }

    #[test]
    fn grading_outranks_ranked_head_when_grader_is_distinct() {
        let ready = vec!["feature".to_owned(), "reap-me".to_owned()];
        let triage = triage_with_blockers(
            "[]",
            r#"[{"id":"feature","priority":0,"score":0.99},{"id":"reap-me","priority":2,"score":0.01}]"#,
        );
        let jsonl = reapable_jsonl("reap-me", "pane3-%8", "pane3-%8");
        let ordered = select_dispatch_order(&triage, &ready, &jsonl, "pane4-%9").unwrap();
        assert_eq!(ordered.first().map(String::as_str), Some("reap-me"));
    }

    #[test]
    fn author_is_not_offered_their_own_reapable() {
        let ready = vec!["feature".to_owned(), "reap-me".to_owned()];
        let triage = triage_with_blockers(
            "[]",
            r#"[{"id":"feature","priority":0,"score":0.99},{"id":"reap-me","priority":2,"score":0.01}]"#,
        );
        let jsonl = reapable_jsonl("reap-me", "pane4-%9", "pane4-%9");
        let ordered = select_dispatch_order(&triage, &ready, &jsonl, "pane4-%9").unwrap();
        assert_ne!(ordered.first().map(String::as_str), Some("reap-me"));
        assert_eq!(ordered.first().map(String::as_str), Some("feature"));
    }

    #[test]
    fn unproven_grader_identity_skips_grading_and_keeps_ranked_head() {
        let ready = vec!["reap-me".to_owned()];
        let triage = triage_with_blockers("[]", r#"[{"id":"reap-me","priority":0,"score":0.5}]"#);
        let jsonl = reapable_jsonl("reap-me", "pane3-%8", "pane3-%8");
        let ordered = select_dispatch_order(&triage, &ready, &jsonl, "WildStone")
            .expect("grading skip must not refuse the cycle");
        assert_eq!(ordered.first().map(String::as_str), Some("reap-me"));
        assert_eq!(
            grading_slot(&jsonl, &ready, "WildStone"),
            GradingSlot::Skip {
                reason: "grader_not_pane_scoped"
            }
        );
    }

    #[test]
    fn no_distinct_idle_peer_skips_grading_and_returns_ranked_head() {
        let ready = vec!["feature".to_owned(), "reap-me".to_owned()];
        let triage = triage_with_blockers(
            "[]",
            r#"[{"id":"feature","priority":0,"score":0.99},{"id":"reap-me","priority":2,"score":0.01}]"#,
        );
        let jsonl = reapable_jsonl("reap-me", "pane4-%9", "pane4-%9");
        let ordered = select_dispatch_order(&triage, &ready, &jsonl, "pane4-%9").unwrap();
        assert_eq!(ordered.first().map(String::as_str), Some("feature"));
        assert_eq!(
            grading_slot(&jsonl, &ready, "pane4-%9"),
            GradingSlot::Skip {
                reason: "no_distinct_idle_peer"
            }
        );
    }

    #[test]
    fn unreadable_ranking_still_refuses_when_jsonl_is_fine() {
        let ready = vec!["reap-me".to_owned()];
        let jsonl = reapable_jsonl("reap-me", "pane3-%8", "pane3-%8");
        let error = select_dispatch_order(br#"{}"#, &ready, &jsonl, "pane4-%9")
            .expect_err("ranking must still refuse");
        assert!(error.starts_with("QUEUE_UNRANKED"), "{error}");
    }

    #[test]
    fn empty_grading_slot_is_a_typed_skip() {
        assert_eq!(
            grading_slot("", &["a".to_owned()], "pane4-%9").to_string(),
            "GRADING_SLOT_SKIP reason=empty_grading_slot"
        );
        assert_eq!(
            grading_slot("not-json", &["a".to_owned()], "pane4-%9").to_string(),
            "GRADING_SLOT_SKIP reason=unreadable_jsonl"
        );
    }

    #[test]
    fn closed_lwdo1_row_is_not_offered() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../.beads/issues.jsonl");
        let jsonl = std::fs::read_to_string(path).expect("issues.jsonl");
        assert!(
            jsonl.contains("\"id\":\"omp-orchestrator-plan-02-lwdo.1\""),
            "C38: the known-bad row must be the real lwdo.1 line"
        );
        let ready = vec![
            "feature".to_owned(),
            "omp-orchestrator-plan-02-lwdo.1".to_owned(),
        ];
        let triage = triage_with_blockers(
            "[]",
            r#"[{"id":"feature","priority":0,"score":0.99},{"id":"omp-orchestrator-plan-02-lwdo.1","priority":2,"score":0.01}]"#,
        );
        let ordered = select_dispatch_order(&triage, &ready, &jsonl, "pane4-%9").unwrap();
        assert_ne!(
            ordered.first().map(String::as_str),
            Some("omp-orchestrator-plan-02-lwdo.1")
        );
        assert_eq!(
            grading_slot(
                &jsonl,
                &["omp-orchestrator-plan-02-lwdo.1".to_owned()],
                "pane4-%9"
            ),
            GradingSlot::Skip {
                reason: "no_reapable_bead"
            }
        );
    }

    #[test]
    fn in_progress_done_bead_is_still_offered() {
        let jsonl = reapable_jsonl("reap-me", "pane3-%8", "pane3-%8");
        assert_eq!(
            grading_slot(&jsonl, &["reap-me".to_owned()], "pane4-%9"),
            GradingSlot::Offer("reap-me".to_owned())
        );
    }

    #[test]
    fn open_done_bead_is_offered_and_grading_status_is_not() {
        let open = reapable_jsonl_status("open-done", "open", "pane3-%8", "pane3-%8");
        assert_eq!(
            grading_slot(&open, &["open-done".to_owned()], "pane4-%9"),
            GradingSlot::Offer("open-done".to_owned())
        );
        let grading = reapable_jsonl_status("already-out", "grading", "pane3-%8", "pane3-%8");
        assert_eq!(
            grading_slot(&grading, &["already-out".to_owned()], "pane4-%9"),
            GradingSlot::Skip {
                reason: "no_reapable_bead"
            }
        );
    }

    #[test]
    fn healthy_case_matches_rank_ready() {
        let ready = vec!["old-p1-leaf".to_owned(), "p0-articulation".to_owned()];
        let triage = triage_with_blockers(
            "[]",
            r#"[{"id":"old-p1-leaf","priority":1,"score":0.9},{"id":"p0-articulation","priority":0,"score":0.2}]"#,
        );
        let ranked = rank_ready(&triage, &ready).unwrap();
        let ordered = select_dispatch_order(&triage, &ready, empty_jsonl(), "pane4-%9").unwrap();
        assert_eq!(ordered, ranked);
    }

    /// REPLACES `eg0m_jsonl_comment_count_is_seventeen` (`omp-orchestrator-325h`).
    ///
    /// The old test read the LIVE `.beads/issues.jsonl` and asserted `== 17` against a
    /// tracker count that only grows. It was RED-FOREVER BY CONSTRUCTION: `eg0m` had 27
    /// comments when this landed and will never have 17 again. Re-pinning 17 -> 27 would
    /// re-arm the identical trap for the next comment added, so the tracker is replaced
    /// by a FIXTURE and the assertion is on the CONTRACT the test actually cared about --
    /// "the JSONL reader can read comments at all", per its own message *"if this is 0
    /// the reader is broken, not the data"*.
    ///
    /// WHAT IS LOST, stated rather than hidden: this no longer proves the live
    /// `.beads/issues.jsonl` PATH resolves from this crate. `closed_lwdo1_row_is_not_offered`
    /// in this module still reads the live file, so that coverage survives elsewhere; if it
    /// is ever removed, path resolution becomes untested.
    #[test]
    fn the_jsonl_reader_counts_comments_from_a_fixture() {
        let jsonl = concat!(
            r#"{"id":"fx-three","status":"closed","comments":[{"author":"a","text":"one"},"#,
            r#"{"author":"b","text":"two"},{"author":"c","text":"three"}]}"#,
            "\n",
            r#"{"id":"fx-zero","status":"open","comments":[]}"#,
            "\n"
        );
        assert_eq!(
            comment_count(jsonl, "fx-three").expect("fixture must parse"),
            CommentCount::Found(3),
            "the reader must count every comment on the addressed row"
        );
        assert_eq!(
            comment_count(jsonl, "fx-zero").expect("fixture must parse"),
            CommentCount::Found(0),
            "a row with an empty comments array is found with zero"
        );
    }

    /// FIRES-ON-KNOWN-BAD for the replacement: malformed JSONL must produce the SPECIFIC
    /// typed message, never a silent zero. A reader that answers 0 for unreadable input is
    /// indistinguishable from one answering 0 for a comment-free bead -- which is exactly
    /// how the pinned test's own "if this is 0 the reader is broken" ambiguity arose.
    ///
    /// Asserts the MESSAGE, not `is_err()`: AGENTS.md gate rule 7 records `cargo` exiting
    /// 101 for two unrelated causes, and the same logic applies to a bare `Err`.
    #[test]
    fn unreadable_jsonl_is_a_named_error_not_a_silent_zero() {
        let error = comment_count("{not json at all", "fx-three")
            .expect_err("malformed JSONL must be an error, never Ok(0)");
        assert!(
            error.contains("QUEUE_UNRANKED comments jsonl line 0"),
            "the refusal must name the surface and the offending line; got {error:?}"
        );
    }

    /// ANTI-VACUITY: an EMPTY scan set must not read as a healthy zero. Absence is the
    /// typed [`CommentCount::RowAbsent`] variant carrying the requested id -- never
    /// `Found(0)`, which is reserved for a present comment-free row.
    #[test]
    fn an_absent_row_is_typed_absence_never_found_zero() {
        for jsonl in ["", r#"{"id":"other","comments":[{"author":"a","text":"x"}]}"#] {
            let outcome = comment_count(jsonl, "fx-three").expect("absent parses");
            assert_eq!(outcome, CommentCount::RowAbsent { id: "fx-three".to_owned() });
            assert_ne!(outcome, CommentCount::Found(0));
        }
    }

    #[test]
    fn pane_scoped_key_rejects_agent_names() {
        assert!(pane_scoped("pane4-%9"));
        assert!(pane_scoped("pane3-%8"));
        assert!(!pane_scoped("WildStone"));
        assert!(!pane_scoped("RubyGate"));
        assert!(!pane_scoped("%9"));
        let _ = PANE_SCOPED;
    }

    fn jsonl_done_ack(id: &str, ack_pane: &str) -> String {
        format!(
            r#"{{"id":"{id}","status":"in_progress","assignee":"WildStone","comments":[{{"author":"WildStone","text":"ACK token on {ack_pane} -- agent=WildStone"}},{{"author":"WildStone","text":"DONE work"}}]}}"#
        )
    }

    fn pane(id: &str, liveness: &str, working: bool) -> ObservedPane {
        ObservedPane {
            pane_id: id.to_owned(),
            liveness: liveness.to_owned(),
            is_dispatchable: liveness == "CONFIRMED_IDLE",
            is_working: working,
        }
    }

    #[test]
    fn assign_peer_grade_picks_an_idle_peer_not_the_working_observer() {
        let panes = vec![
            pane("%9", "LIVE", true),
            pane("%3", "CONFIRMED_IDLE", false),
        ];
        let jsonl = jsonl_done_ack("reap-me", "%9");
        let assigned = assign_peer_grade("%9", &panes, &jsonl).expect("idle peer");
        assert_eq!(assigned.bead, "reap-me");
        assert_eq!(assigned.grader_pane, "%3");
        assert_eq!(assigned.grader_assignee, "pane3-%3");
        assert_eq!(assigned.observer_pane, "%9");
    }

    #[test]
    fn assign_peer_grade_excludes_a_confirmed_idle_observer() {
        let panes = vec![
            pane("%9", "CONFIRMED_IDLE", false),
            pane("%3", "CONFIRMED_IDLE", false),
        ];
        let jsonl = jsonl_done_ack("reap-me", "%8");
        let assigned = assign_peer_grade("%9", &panes, &jsonl).expect("peer not observer");
        assert_eq!(
            assigned.grader_pane, "%3",
            "idle observer must never be the grader: {assigned:?}"
        );
        assert_ne!(assigned.grader_pane, "%9");
        assert_eq!(assigned.grader_assignee, "pane3-%3");
    }

    fn ledger_row(bead: &str, pane: &str) -> String {
        format!(
            r#"{{"bead":"{bead}","event":"dispatched","pane":"{pane}","grader_pane":"{pane}"}}"#
        )
    }

    #[test]
    fn ledger_only_attribution_is_gradeable_by_a_different_pane() {
        let panes = vec![
            pane("%9", "LIVE", true),
            pane("%3", "CONFIRMED_IDLE", false),
        ];
        let jsonl = r#"{"id":"reap-me","status":"in_progress","assignee":"WildStone","comments":[{"author":"WildStone","text":"DONE work"}]}"#;
        let ledger = ledger_row("reap-me", "%8");
        let assigned = assign_peer_grade_with_ledger("%9", &panes, jsonl, &ledger)
            .expect("ledger pane %8 is not the idle grader");
        assert_eq!(assigned.bead, "reap-me");
        assert_eq!(assigned.grader_pane, "%3");
    }

    #[test]
    fn ledger_naming_candidate_grader_is_self_author() {
        let panes = vec![
            pane("%9", "LIVE", true),
            pane("%3", "CONFIRMED_IDLE", false),
        ];
        let jsonl = r#"{"id":"reap-me","status":"in_progress","assignee":"WildStone","comments":[{"author":"WildStone","text":"DONE work"}]}"#;
        let ledger = ledger_row("reap-me", "%3");
        let error = assign_peer_grade_with_ledger("%9", &panes, jsonl, &ledger)
            .expect_err("idle grader authored the ledger row");
        assert_eq!(
            error.to_string(),
            "NO_ELIGIBLE_GRADER reason=no_distinct_idle_peer"
        );
        let without = assign_peer_grade("%9", &panes, jsonl).expect_err("ledger removed");
        assert!(
            without.to_string().contains("missing_attribution count=1"),
            "known-bad must fail when ledger source is removed: {without}"
        );
    }

    #[test]
    fn missing_attribution_names_the_count_not_no_distinct_idle_peer() {
        let panes = vec![
            pane("%9", "LIVE", true),
            pane("%3", "CONFIRMED_IDLE", false),
        ];
        let jsonl = concat!(
            r#"{"id":"a","status":"in_progress","assignee":"WildStone","comments":[{"author":"WildStone","text":"DONE a"}]}"#,
            "\n",
            r#"{"id":"b","status":"in_progress","assignee":"WildStone","comments":[{"author":"WildStone","text":"DONE b"}]}"#,
        );
        let error = assign_peer_grade("%9", &panes, jsonl).expect_err("unattributed");
        let text = error.to_string();
        assert!(text.starts_with("GRADER_IDENTITY_UNRESOLVED"), "{text}");
        assert!(text.contains("missing_attribution count=2"), "{text}");
        assert!(!text.contains("no_distinct_idle_peer"), "{text}");
    }

    #[test]
    fn empty_reapable_set_is_error_not_no_distinct_idle_peer() {
        let panes = vec![
            pane("%9", "LIVE", true),
            pane("%3", "CONFIRMED_IDLE", false),
        ];
        let error = assign_peer_grade("%9", &panes, "").expect_err("empty");
        assert_eq!(
            error.to_string(),
            "NO_ELIGIBLE_GRADER reason=empty_reapable_set"
        );
    }

    #[test]
    fn assign_peer_grade_refuses_a_pane_carrying_its_own_dispatch() {
        let panes = vec![pane("%3", "LIVE", true)];
        let error = require_idle_grader("%3", &panes).expect_err("working grader");
        let text = error.to_string();
        assert!(text.starts_with("GRADER_CARRYING_OWN_DISPATCH"), "{text}");
        assert!(text.contains("pane=%3"), "{text}");
        assert!(text.contains("is_working=true"), "{text}");
    }

    #[test]
    fn assign_peer_grade_empty_observation_is_typed() {
        let error = assign_peer_grade("%9", &[], "").expect_err("empty");
        assert_eq!(
            error.to_string(),
            "EMPTY_OBSERVATION no eligible grader can be decided from an empty pane set"
        );
    }

    #[test]
    fn assign_peer_grade_all_others_working_is_typed() {
        let panes = vec![
            pane("%9", "LIVE", true),
            pane("%3", "LIVE", true),
            pane("%8", "LIVE", true),
        ];
        let error = assign_peer_grade("%9", &panes, &jsonl_done_ack("reap-me", "%9"))
            .expect_err("all carrying");
        assert_eq!(
            error.to_string(),
            "NO_ELIGIBLE_GRADER reason=all_panes_carrying_dispatches"
        );
    }

    #[test]
    fn assign_peer_grade_no_idle_pane_is_typed() {
        let panes = vec![pane("%9", "LIVE", true), pane("%3", "NEWLY_IDLE", false)];
        let error =
            assign_peer_grade("%9", &panes, &jsonl_done_ack("reap-me", "%9")).expect_err("no idle");
        assert_eq!(error.to_string(), "NO_ELIGIBLE_GRADER reason=no_idle_pane");
    }

    #[test]
    fn assign_peer_grade_refuses_unresolved_author() {
        let panes = vec![
            pane("%9", "LIVE", true),
            pane("%3", "CONFIRMED_IDLE", false),
        ];
        let jsonl = r#"{"id":"reap-me","status":"in_progress","assignee":"WildStone","comments":[{"author":"WildStone","text":"DONE work"}]}"#;
        let error = assign_peer_grade("%9", &panes, jsonl).expect_err("unresolved");
        assert!(
            error.to_string().starts_with("GRADER_IDENTITY_UNRESOLVED"),
            "{}",
            error
        );
    }

    #[test]
    fn assign_peer_grade_does_not_self_grade() {
        let panes = vec![
            pane("%9", "LIVE", true),
            pane("%3", "CONFIRMED_IDLE", false),
        ];
        let jsonl = jsonl_done_ack("reap-me", "%3");
        let error = assign_peer_grade("%9", &panes, &jsonl).expect_err("self");
        assert_eq!(
            error.to_string(),
            "NO_ELIGIBLE_GRADER reason=no_distinct_idle_peer"
        );
    }
    fn filed_only_fixture() -> String {
        concat!(
            r#"{"id":"filed","status":"open","description":"STATUS: filed only; do not claim or implement this record.","acceptance_criteria":"Run the recorded check; expect exit 0.","comments":[]}
"#,
            r#"{"id":"normal","status":"open","description":"Implement the normal work item.","acceptance_criteria":"Run cargo test; expect exit 0.","comments":[]}
"#,
        )
        .to_owned()
    }

    #[test]
    fn filed_only_ready_records_are_filtered_from_dispatch_order() {
        let ready = vec!["filed".to_owned(), "normal".to_owned()];
        let triage = br#"{"data_hash":"fixture","triage":{"blockers_to_clear":[],"recommendations":[{"id":"filed","priority":0,"score":1.0},{"id":"normal","priority":1,"score":0.1}]}}"#
            .to_vec();
        let jsonl = filed_only_fixture();
        let ordered =
            select_dispatch_order(&triage, &ready, &jsonl, "pane4-%9").expect("selector pass");
        assert_eq!(ordered, vec!["normal"]);

        let priorities = BTreeMap::from([("filed".to_owned(), 0), ("normal".to_owned(), 1)]);
        let ranked =
            select_dispatch_order_with_priorities(&triage, &ready, &priorities, &jsonl, "pane4-%9")
                .expect("priority selector pass");
        assert_eq!(ranked.ids, vec!["normal"]);
        assert_ne!(ranked.ids, vec!["filed"]);

        let insights = br#"{"data_hash":"fixture","status":{"PageRank":{"state":"computed"}},"full_stats":{"pagerank":{"filed":1.0,"normal":0.1}}}"#;
        let pageranked = select_dispatch_order_with_pagerank(
            &triage,
            &ready,
            &priorities,
            insights,
            &jsonl,
            "pane4-%9",
        )
        .expect("pagerank selector pass");
        assert_eq!(pageranked.ids, vec!["normal"]);
    }

    #[test]
    fn quoted_filed_only_language_does_not_filter_a_real_record() {
        let ready = vec!["b4iv".to_owned()];
        let triage = triage_with_blockers("[]", r#"[{"id":"b4iv","priority":0,"score":1.0}]"#);
        let jsonl = r#"{"id":"b4iv","status":"open","description":"This audit quotes 'filed only' and 'do not claim' as examples.","acceptance_criteria":"Run cargo test; expect exit 0.","comments":[]}"#;
        let ordered =
            select_dispatch_order(&triage, &ready, jsonl, "pane4-%9").expect("selector pass");
        assert_eq!(ordered, vec!["b4iv"]);
    }

    #[test]
    fn all_filed_only_ready_records_report_a_typed_filtered_window() {
        let ready = vec!["filed".to_owned()];
        let triage = triage_with_blockers("[]", r#"[{"id":"filed","priority":0,"score":1.0}]"#);
        let priorities = BTreeMap::from([("filed".to_owned(), 0)]);
        let ranked = select_dispatch_order_with_priorities(
            &triage,
            &ready,
            &priorities,
            &filed_only_fixture(),
            "pane4-%9",
        )
        .expect("selector pass");
        assert!(ranked.ids.is_empty());
        assert_eq!(ranked.window, RankWindow::FilteredFiledOnly);
    }
}

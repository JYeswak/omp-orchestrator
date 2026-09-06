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
use std::collections::BTreeSet;

/// Pane-scoped assignee: unique on one tmux server for one occupancy window.
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
        let priority = row.get("priority").and_then(Value::as_u64).unwrap_or(u64::MAX);
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
        let actionable = row.get("actionable").and_then(Value::as_bool).unwrap_or(false);
        let unblocks = row.get("unblocks_count").and_then(Value::as_u64).unwrap_or(0);
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct BeadComments {
    id: String,
    status: String,
    assignee: String,
    authors: Vec<String>,
    texts: Vec<String>,
}

fn parse_jsonl(jsonl: &str) -> Result<Vec<BeadComments>, String> {
    let mut out = Vec::new();
    for (i, line) in jsonl.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line).map_err(|error| {
            format!("QUEUE_UNRANKED comments jsonl line {i}: {error}")
        })?;
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
            authors,
            texts,
        });
    }
    Ok(out)
}

fn is_reapable(bead: &BeadComments) -> bool {
    if bead.status != "in_progress" {
        return false;
    }
    bead.texts.iter().any(|t| {
        t.contains("MUTATION-VERIFIED")
            || t.contains("STAGE: IMPL -> GRADING")
            || t.contains("DONE ")
            || t.starts_with("DONE")
    })
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

/// Count comments for `id`. Positive control: `eg0m` has 17 in production jsonl.
pub fn comment_count(jsonl: &str, id: &str) -> Result<usize, String> {
    let beads = parse_jsonl(jsonl)?;
    Ok(beads
        .iter()
        .find(|b| b.id == id)
        .map(|b| b.authors.len().max(b.texts.len()))
        .unwrap_or(0))
}

fn first_grading_assignment(
    jsonl: &str,
    ready: &[String],
    grader: &str,
) -> Result<Option<String>, String> {
    if !pane_scoped(grader) {
        let beads = parse_jsonl(jsonl)?;
        if beads.iter().any(|b| ready.iter().any(|r| r == &b.id) && is_reapable(b)) {
            return Err("GRADER_IDENTITY_UNRESOLVED grader is not pane-scoped".to_owned());
        }
        return Ok(None);
    }
    let beads = parse_jsonl(jsonl)?;
    for id in ready {
        let Some(bead) = beads.iter().find(|b| &b.id == id) else {
            continue;
        };
        if !is_reapable(bead) {
            continue;
        }
        let authors = pane_scoped_authors(bead);
        if authors.is_empty() {
            // Cannot prove distinct from WildStone-style authors. Do not offer.
            continue;
        }
        if authors.contains(grader) {
            continue;
        }
        return Ok(Some(id.clone()));
    }
    Ok(None)
}

/// Full dispatch order. `grader` must be pane-scoped (`pane4-%9`) to take a grading slot.
pub fn select_dispatch_order(
    triage: &[u8],
    ready: &[String],
    jsonl: &str,
    grader: &str,
) -> Result<Vec<String>, String> {
    let value: Value = serde_json::from_slice(triage)
        .map_err(|error| format!("QUEUE_UNRANKED bv triage JSON: {error}"))?;
    let assigned = assigned_ids(&value);
    let mut out: Vec<String> = Vec::new();
    if let Some(blocker) = first_actionable_blocker(&value, ready, &assigned)? {
        out.push(blocker);
    }
    if let Some(grade) = first_grading_assignment(jsonl, ready, grader)? {
        if !out.iter().any(|id| id == &grade) {
            out.push(grade);
        }
    }
    let ranked = rank_ready(triage, ready)?;
    for id in ranked {
        if !out.iter().any(|chosen| chosen == &id) {
            out.push(id);
        }
    }
    Ok(out)
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
    fn missing_recommendations_is_a_typed_refusal_not_silent_fifo() {
        let ready = vec!["a".to_owned()];
        for payload in [
            &br#"{"triage":{}}"#[..],
            &br#"{"triage":{"quick_ref":{"top_picks":["a"]}}}"#[..],
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
        format!(
            r#"{{"triage":{{"blockers_to_clear":{blockers},"recommendations":{recs}}}}}"#
        )
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
        assert_eq!(rank_ready(&triage, &ready).unwrap().first().map(String::as_str), Some("p0-leaf"));
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
        assert!(
            error.starts_with("QUEUE_UNRANKED"),
            "{error}"
        );
        assert!(error.contains("blockers_to_clear"), "{error}");
    }

    fn reapable_jsonl(id: &str, assignee: &str, author: &str) -> String {
        format!(
            r#"{{"id":"{id}","status":"in_progress","assignee":"{assignee}","comments":[{{"author":"{author}","text":"STAGE: IMPL -> GRADING. sha abc. Re-run: cargo test."}}]}}"#
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
    fn unproven_grader_identity_refuses_when_reapable_exists() {
        let ready = vec!["reap-me".to_owned()];
        let triage = triage_with_blockers(
            "[]",
            r#"[{"id":"reap-me","priority":0,"score":0.5}]"#,
        );
        let jsonl = reapable_jsonl("reap-me", "pane3-%8", "pane3-%8");
        let error = select_dispatch_order(&triage, &ready, &jsonl, "WildStone")
            .expect_err("must refuse");
        assert!(
            error.starts_with("GRADER_IDENTITY_UNRESOLVED"),
            "{error}"
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

    #[test]
    fn eg0m_jsonl_comment_count_is_seventeen() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../.beads/issues.jsonl");
        let text = std::fs::read_to_string(path).expect("issues.jsonl");
        let n = comment_count(&text, "omp-orchestrator-eg0m").expect("parse");
        assert_eq!(n, 17, "if this is 0 the reader is broken, not the data");
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
}

#![forbid(unsafe_code)]

//! Grader attribution must be a tracker field, not English inside `close_reason`.
//!
//! Measured 2026-09-02 (`omp-orchestrator-gcyf`): 23 of 40 closes named their grader
//! only in prose; `comments[].author` was `josh` on all of them, so
//! "grader ≠ implementer" was unverifiable. Positive controls already exist
//! (`leht` GreenFrog `--actor`, `mj8w` AmberGate named). The mechanism works when
//! used; this crate refuses the two shapes that leave it unused.
//!
//! Neighbouring hole, cited not solved: `gfm6` (a bead cannot name which agent
//! holds it or whether that agent is alive).
//!
//! This crate does not wrap `br`. A wrapper that is not called is BUILT ≠ WIRED
//! (`fh N043`). Callers are tests and `dispatch_packet`'s printed `--actor` form.

use std::fmt;

/// Git/`$USER` identity `br` writes when `--actor` is omitted.
///
/// Case-folded comparison is load-bearing (`Josh` vs `josh`, measured in
/// `bead-holder`).
pub const DEFAULT_AUTHORS: &[&str] = &["josh"];

/// One attempted close, as the tracker will record it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloseAttempt {
    pub bead_id: String,
    /// Who implemented. Not the current assignee (`gfm6`: that field is free text).
    pub implementer: Option<String>,
    /// `--actor` on `br close` / `br comments add`. `None` means the flag was omitted.
    pub actor: Option<String>,
    pub close_reason: String,
    pub comment_authors: Vec<String>,
    /// `Some` when `$TMUX_PANE` is set — a pane close without `--actor` is the defect.
    pub tmux_pane: Option<String>,
}

/// Why a close is not attributable.
///
/// Two variants, two messages. Collapsing them would make "forgot `--actor`"
/// indistinguishable from "graded your own work".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributionRefusal {
    ActorAbsent { pane: Option<String> },
    SelfGrade { actor: String, implementer: String },
    GraderProseMismatch { prose_name: String, actor: String },
}

impl fmt::Display for AttributionRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ActorAbsent { pane } => match pane {
                Some(pane) => write!(
                    f,
                    "ACTOR_REQUIRED pane={pane} -- br close without --actor from a tmux pane \
                     attributes the closer as josh; pass --actor <AgentMail name>"
                ),
                None => write!(
                    f,
                    "ACTOR_REQUIRED pane=none -- br close without --actor attributes the closer \
                     as josh; pass --actor <AgentMail name>"
                ),
            },
            Self::SelfGrade { actor, implementer } => write!(
                f,
                "SELF_GRADE_REFUSED actor={actor} implementer={implementer} -- grader must be a \
                 different identity than the implementer"
            ),
            Self::GraderProseMismatch { prose_name, actor } => write!(
                f,
                "GRADER_PROSE_MISMATCH prose={prose_name} actor={actor} -- close_reason names a \
                 grader the CLI actor is not"
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributionPass {
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributionVerdict {
    Pass(AttributionPass),
    Refused(AttributionRefusal),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmptyScan;

impl fmt::Display for EmptyScan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            "ATTRIBUTION_SCAN_EMPTY -- an empty scan set is an ERROR, never a pass; \
             a deliverable never checked reports identically to one that passed",
        )
    }
}

pub fn is_default_author(name: &str, defaults: &[&str]) -> bool {
    let folded = name.trim().to_lowercase();
    defaults
        .iter()
        .any(|default| default.eq_ignore_ascii_case(&folded))
}

/// Names a close_reason that says "by AmberGate" / "grader WildStone" without `--actor`.
pub fn prose_grader(close_reason: &str) -> Option<String> {
    for (needle, skip) in [("grader ", 7), ("by ", 3)] {
        if let Some(idx) = close_reason.to_ascii_lowercase().find(needle) {
            let rest = close_reason[idx + skip..].trim_start();
            let token = rest
                .split(|c: char| c.is_whitespace() || c == '(' || c == ',' || c == ';')
                .next()
                .unwrap_or("");
            if token
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_uppercase())
                && token.len() > 2
            {
                return Some(token.to_owned());
            }
        }
    }
    None
}

/// Non-default comment authors plus a prose grader, if any.
///
/// A zero from this matcher on a record known to carry attribution is a broken matcher.
pub fn attribution_hits(authors: &[String], close_reason: &str, defaults: &[&str]) -> Vec<String> {
    let mut hits: Vec<String> = authors
        .iter()
        .filter(|author| !is_default_author(author, defaults))
        .cloned()
        .collect();
    if let Some(prose) = prose_grader(close_reason) {
        if !hits.iter().any(|hit| hit.eq_ignore_ascii_case(&prose)) {
            hits.push(prose);
        }
    }
    hits
}

pub fn assess_close(input: &CloseAttempt, defaults: &[&str]) -> AttributionVerdict {
    let actor = input
        .actor
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty());
    let actor = match actor {
        None => {
            return AttributionVerdict::Refused(AttributionRefusal::ActorAbsent {
                pane: input.tmux_pane.clone(),
            });
        }
        Some(name) if is_default_author(name, defaults) => {
            return AttributionVerdict::Refused(AttributionRefusal::ActorAbsent {
                pane: input.tmux_pane.clone(),
            });
        }
        Some(name) => name,
    };
    if let Some(implementer) = input
        .implementer
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        if actor.eq_ignore_ascii_case(implementer) {
            return AttributionVerdict::Refused(AttributionRefusal::SelfGrade {
                actor: actor.to_owned(),
                implementer: implementer.to_owned(),
            });
        }
    }
    if let Some(prose) = prose_grader(&input.close_reason) {
        if !is_default_author(&prose, defaults) && !prose.eq_ignore_ascii_case(actor) {
            return AttributionVerdict::Refused(AttributionRefusal::GraderProseMismatch {
                prose_name: prose,
                actor: actor.to_owned(),
            });
        }
    }
    AttributionVerdict::Pass(AttributionPass {
        actor: actor.to_owned(),
    })
}

/// Anti-vacuity: scanning nothing is an error.
pub fn scan_closes(
    rows: &[CloseAttempt],
    defaults: &[&str],
) -> Result<Vec<AttributionVerdict>, EmptyScan> {
    if rows.is_empty() {
        return Err(EmptyScan);
    }
    Ok(rows
        .iter()
        .map(|row| assess_close(row, defaults))
        .collect())
}

/// One closed bead as recorded in `.beads/issues.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedBead {
    pub id: String,
    pub closed_at: String,
    pub close_reason: String,
    pub comment_authors: Vec<String>,
}

/// Parse closed rows from the production JSONL. Zero closed rows is EmptyScan.
pub fn parse_closed_beads(jsonl: &str) -> Result<Vec<ClosedBead>, EmptyScan> {
    let mut rows = Vec::new();
    for line in jsonl.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("status").and_then(|s| s.as_str()) != Some("closed") {
            continue;
        }
        let id = value
            .get("id")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_owned();
        if id.is_empty() {
            continue;
        }
        let authors = value
            .get("comments")
            .and_then(|c| c.as_array())
            .map(|comments| {
                comments
                    .iter()
                    .filter_map(|comment| {
                        comment
                            .get("author")
                            .and_then(|author| author.as_str())
                            .map(str::to_owned)
                    })
                    .collect()
            })
            .unwrap_or_default();
        rows.push(ClosedBead {
            id,
            closed_at: value
                .get("closed_at")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_owned(),
            close_reason: value
                .get("close_reason")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_owned(),
            comment_authors: authors,
        });
    }
    if rows.is_empty() {
        Err(EmptyScan)
    } else {
        Ok(rows)
    }
}

/// Closed beads whose comments are only the git default author (or have none).
pub fn unattributed_close_ids(rows: &[ClosedBead], defaults: &[&str]) -> Vec<String> {
    rows.iter()
        .filter(|row| {
            !row
                .comment_authors
                .iter()
                .any(|author| !is_default_author(author, defaults))
        })
        .map(|row| row.id.clone())
        .collect()
}

/// Exit 1 names unattributed closes; 0 is a fully attributed ledger; 2 is empty.
pub fn ledger_gate_exit(unattributed: &[String]) -> i32 {
    if unattributed.is_empty() {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attempt(
        actor: Option<&str>,
        implementer: Option<&str>,
        reason: &str,
        pane: Option<&str>,
    ) -> CloseAttempt {
        CloseAttempt {
            bead_id: "omp-orchestrator-gcyf".into(),
            implementer: implementer.map(str::to_owned),
            actor: actor.map(str::to_owned),
            close_reason: reason.into(),
            comment_authors: actor.into_iter().map(str::to_owned).collect(),
            tmux_pane: pane.map(str::to_owned),
        }
    }

    #[test]
    fn actor_absent_from_a_pane_is_actor_required() {
        let verdict = assess_close(
            &attempt(None, Some("AmberGate"), "MUTATION-VERIFIED prose grader", Some("%9")),
            DEFAULT_AUTHORS,
        );
        let AttributionVerdict::Refused(refusal) = verdict else {
            panic!("absent actor must refuse, got {verdict:?}");
        };
        let text = refusal.to_string();
        assert!(
            text.starts_with("ACTOR_REQUIRED pane=%9"),
            "KNOWN-BAD absent --actor must name the pane: {text}"
        );
        assert!(!text.contains("SELF_GRADE_REFUSED"));
    }

    #[test]
    fn default_josh_actor_is_actor_required_not_self_grade() {
        let verdict = assess_close(
            &attempt(
                Some("josh"),
                Some("AmberGate"),
                "MUTATION-VERIFIED by SnowyCanyon",
                Some("%1397"),
            ),
            DEFAULT_AUTHORS,
        );
        let AttributionVerdict::Refused(refusal) = verdict else {
            panic!("josh actor is omitted --actor, got {verdict:?}");
        };
        assert!(
            refusal.to_string().starts_with("ACTOR_REQUIRED"),
            "{}",
            refusal
        );
    }

    #[test]
    fn actor_equal_implementer_is_self_grade() {
        let verdict = assess_close(
            &attempt(
                Some("BlueLantern"),
                Some("BlueLantern"),
                "MUTATION-VERIFIED self",
                Some("%1414"),
            ),
            DEFAULT_AUTHORS,
        );
        let AttributionVerdict::Refused(refusal) = verdict else {
            panic!("self-grade must refuse, got {verdict:?}");
        };
        let text = refusal.to_string();
        assert!(
            text.starts_with("SELF_GRADE_REFUSED actor=BlueLantern implementer=BlueLantern"),
            "KNOWN-BAD self-grade must name both identities: {text}"
        );
        assert!(!text.contains("ACTOR_REQUIRED"));
    }

    #[test]
    fn the_two_refusals_do_not_share_a_message() {
        let absent = AttributionRefusal::ActorAbsent {
            pane: Some("%9".into()),
        }
        .to_string();
        let self_grade = AttributionRefusal::SelfGrade {
            actor: "BlueLantern".into(),
            implementer: "BlueLantern".into(),
        }
        .to_string();
        assert_ne!(
            absent, self_grade,
            "two distinct failures, two distinct messages"
        );
        assert!(absent.contains("ACTOR_REQUIRED"));
        assert!(self_grade.contains("SELF_GRADE_REFUSED"));
        assert!(!absent.contains("SELF_GRADE_REFUSED"));
        assert!(!self_grade.contains("ACTOR_REQUIRED"));
    }

    #[test]
    fn iis6_close_passes() {
        let iis6 = CloseAttempt {
            bead_id: "omp-orchestrator-iis6".into(),
            implementer: Some("AmberGate".into()),
            actor: Some("WildStone".into()),
            close_reason: "MUTATION-VERIFIED grader WildStone %9 re-executed cat-file 4d8c784"
                .into(),
            comment_authors: vec!["WildStone".into()],
            tmux_pane: Some("%9".into()),
        };
        match assess_close(&iis6, DEFAULT_AUTHORS) {
            AttributionVerdict::Pass(pass) => assert_eq!(pass.actor, "WildStone"),
            other => panic!("KNOWN-GOOD iis6 must PASS, got {other:?}"),
        }
    }

    #[test]
    fn prose_grader_mismatch_is_refused() {
        let verdict = assess_close(
            &attempt(
                Some("WildStone"),
                Some("AmberGate"),
                "MUTATION-VERIFIED by SnowyCanyon (orchestrator)",
                Some("%9"),
            ),
            DEFAULT_AUTHORS,
        );
        let AttributionVerdict::Refused(AttributionRefusal::GraderProseMismatch { prose_name, actor }) =
            verdict
        else {
            panic!("prose/actor split must refuse, got {verdict:?}");
        };
        assert_eq!(prose_name, "SnowyCanyon");
        assert_eq!(actor, "WildStone");
    }

    #[test]
    fn empty_scan_is_an_error() {
        let error = scan_closes(&[], DEFAULT_AUTHORS).expect_err("empty scan must not pass");
        assert!(error.to_string().contains("ATTRIBUTION_SCAN_EMPTY"));
    }

    #[test]
    fn leht_and_mj8w_are_positive_controls() {
        let leht_authors = vec![
            "josh".into(),
            "AmberGate".into(),
            "GreenFrog".into(),
            "BlueLantern".into(),
        ];
        let leht_hits = attribution_hits(
            &leht_authors,
            "APPROVED: independent regrade verified derived membership",
            DEFAULT_AUTHORS,
        );
        assert!(
            leht_hits.iter().any(|h| h == "GreenFrog"),
            "POSITIVE CONTROL leht: matcher must fire on GreenFrog, got {leht_hits:?}"
        );

        let mj8w_authors = vec!["josh".into(), "BlueLantern".into(), "Pass7Grader".into()];
        let mj8w_hits = attribution_hits(
            &mj8w_authors,
            "APPROVED by AmberGate (pane 4, non-implementer grade of BlueLantern's 4893b36).",
            DEFAULT_AUTHORS,
        );
        assert!(
            mj8w_hits.iter().any(|h| h == "AmberGate"),
            "POSITIVE CONTROL mj8w: matcher must fire on AmberGate, got {mj8w_hits:?}"
        );
        assert!(
            mj8w_hits.iter().any(|h| h == "BlueLantern"),
            "POSITIVE CONTROL mj8w: matcher must fire on BlueLantern, got {mj8w_hits:?}"
        );
        assert!(
            !leht_hits.is_empty() && !mj8w_hits.is_empty(),
            "a zero from a pattern that cannot match is not evidence"
        );
    }

    #[test]
    fn mutation_collapsing_messages_is_detectable() {
        let absent = AttributionRefusal::ActorAbsent {
            pane: Some("%9".into()),
        }
        .to_string();
        let self_grade = AttributionRefusal::SelfGrade {
            actor: "BlueLantern".into(),
            implementer: "BlueLantern".into(),
        }
        .to_string();
        assert_ne!(absent, self_grade);
        assert!(absent.contains("ACTOR_REQUIRED"));
        assert!(self_grade.contains("SELF_GRADE_REFUSED"));
    }
}

#![forbid(unsafe_code)]

//! Refill every idle pane from the bv DAG — the pure decision layer.
//!
//! # Why this crate exists
//!
//! Joshua, 2026-08-27: *"why are you and all of your agents idle? you are the one
//! building this process and you can't stay busy."* Measured at that moment: **3 panes
//! idle, 334 actionable beads, a 316-deep ready queue** — and the controller was
//! hand-writing one dispatch packet at a time. A controller that must compose prose
//! before every dispatch will always starve its own fleet.
//!
//! Then, immediately after: *"we've been migrating all .sh to rust in
//! /ntm-fleet-monitor how do you keep forgetting this"*. He was right — the first cut
//! of this shipped as 230 lines of shell into a repo whose dispatch chain is
//! **30 components, all `status = "verified"` Rust**. `bin/refill-idle-panes.sh`
//! remains as the differential oracle, the same contract every other row in
//! `registries/dispatch_chain_migration.toml` follows.
//!
//! # The decision, and why it is pure
//!
//! Everything here is a function of text already captured. No process spawning, no
//! clock, no filesystem. That is deliberate: the shell version could only be tested by
//! running it against a live fleet, which is why its riskiest rule — the two-surface
//! intersection — had no unit coverage at all. Here every rule is a fixture.
//!
//! # The safety property
//!
//! **Two surfaces disagree, and dispatching on the wrong one sends real work into a
//! void.** Measured 2026-08-27: `ntm --robot-activity` reported control-plane pane 4
//! `safe_to_dispatch: true` while `pane-dispatch-ready` reported `NO_AGENT — bare
//! shell`. The oracle was right; that pane had no agent process and could never have
//! received a packet.
//!
//! So [`dispatchable_panes`] takes the **intersection**, and an unparseable probe
//! yields **zero** candidates rather than all of them. An absent measurement is never a
//! green light — least of all in the direction that burns queue depth producing nothing.

use std::collections::BTreeSet;

/// Why a pane was not selected. Named rather than boolean so a caller can report the
/// cause: "no candidates" and "the probe was unreadable" have opposite remedies, and
/// collapsing them is how a starved fleet reads as a quiet one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneRefusal {
    /// The activity surface did not mark it `safe_to_dispatch`.
    ActivityBusy,
    /// The readiness oracle did not report `FREE` — includes `NO_AGENT`, a bare shell.
    OracleNotFree,
    /// A probe could not be parsed. Fails closed for every pane.
    ProbeUnreadable,
}

impl PaneRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ActivityBusy => "activity-busy",
            Self::OracleNotFree => "oracle-not-free",
            Self::ProbeUnreadable => "probe-unreadable",
        }
    }
}

/// The two probes' verdicts, already parsed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaneSurvey {
    /// Panes `ntm --robot-activity` marked `safe_to_dispatch: true`.
    pub activity_safe: BTreeSet<String>,
    /// Panes `pane-dispatch-ready` reported as `FREE`.
    pub oracle_free: BTreeSet<String>,
    /// True when either probe failed to parse — every pane is then refused.
    pub unreadable: bool,
}

/// Parse `ntm --robot-activity=<session>` output.
///
/// Returns `None` on unparseable input. The caller MUST treat that as "refuse
/// everything", never as "nothing is busy".
pub fn parse_activity(text: &str) -> Option<BTreeSet<String>> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let agents = value.get("agents")?.as_array()?;
    Some(
        agents
            .iter()
            .filter(|a| a.get("safe_to_dispatch").and_then(serde_json::Value::as_bool) == Some(true))
            .filter_map(|a| pane_id(a.get("pane")))
            .collect(),
    )
}

/// Parse `pane-dispatch-ready <session> --json` output.
///
/// Only `FREE` counts. `BUSY` means an agent is present but working — dispatchable
/// later. `NO_AGENT` is a bare shell and never dispatchable; treating it as free is one
/// of the named-forbidden moves in `xtask check-product`.
pub fn parse_oracle(text: &str) -> Option<BTreeSet<String>> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let panes = value.get("panes")?.as_array()?;
    Some(
        panes
            .iter()
            .filter(|p| p.get("state").and_then(serde_json::Value::as_str) == Some("FREE"))
            .filter_map(|p| pane_id(p.get("pane")))
            .collect(),
    )
}

/// Pane ids arrive as either `"2"` or `2` depending on the surface. Accept both rather
/// than silently dropping one form — a dropped pane reads as a busy fleet.
fn pane_id(value: Option<&serde_json::Value>) -> Option<String> {
    match value? {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Build the survey from both probes' raw text.
pub fn survey(activity_text: &str, oracle_text: &str) -> PaneSurvey {
    match (parse_activity(activity_text), parse_oracle(oracle_text)) {
        (Some(activity_safe), Some(oracle_free)) => PaneSurvey {
            activity_safe,
            oracle_free,
            unreadable: false,
        },
        // FAIL CLOSED. One unreadable probe refuses every pane.
        _ => PaneSurvey {
            unreadable: true,
            ..PaneSurvey::default()
        },
    }
}

/// The panes BOTH surfaces call free, in stable numeric-then-lexical order so a run is
/// reproducible and a differential against the shell oracle is meaningful.
pub fn dispatchable_panes(survey: &PaneSurvey) -> Vec<String> {
    if survey.unreadable {
        return Vec::new();
    }
    let mut panes: Vec<String> = survey
        .activity_safe
        .intersection(&survey.oracle_free)
        .cloned()
        .collect();
    panes.sort_by(|a, b| {
        match (a.parse::<u64>(), b.parse::<u64>()) {
            (Ok(x), Ok(y)) => x.cmp(&y),
            _ => a.cmp(b),
        }
    });
    panes
}

/// Explain why one pane was refused. `None` means it is dispatchable.
pub fn refusal_for(survey: &PaneSurvey, pane: &str) -> Option<PaneRefusal> {
    if survey.unreadable {
        return Some(PaneRefusal::ProbeUnreadable);
    }
    if !survey.activity_safe.contains(pane) {
        return Some(PaneRefusal::ActivityBusy);
    }
    if !survey.oracle_free.contains(pane) {
        return Some(PaneRefusal::OracleNotFree);
    }
    None
}

/// Parse `bv --robot-triage` recommendations into descending-score bead ids.
///
/// `quick_ref.top_picks` is a weaker summary and does not carry the ranking score
/// that dispatch needs. Recommendations are sorted explicitly so this parser does
/// not silently inherit a different envelope order.
pub fn parse_recommendations(text: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let Some(recommendations) = value
        .get("triage")
        .and_then(|t| t.get("recommendations"))
        .and_then(serde_json::Value::as_array)
    else {
        return Vec::new();
    };

    let mut ranked = recommendations
        .iter()
        .enumerate()
        .filter_map(|(position, recommendation)| {
            let id = recommendation.get("id")?.as_str()?;
            let score = recommendation.get("score")?.as_f64()?;
            score.is_finite().then(|| (score, position, id.to_string()))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.1.cmp(&right.1))
    });
    ranked.into_iter().map(|(_, _, id)| id).collect()
}

/// One pane paired with the bead it should receive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    pub pane: String,
    pub bead: String,
}

/// Pair dispatchable panes with ranked beads, capped.
///
/// Zip semantics: the shorter side bounds it. Fewer beads than panes leaves panes idle
/// (correct — there is no work); fewer panes than beads leaves beads queued (correct —
/// there is nowhere to put them). Neither is an error.
pub fn plan(panes: &[String], beads: &[String], max: usize) -> Vec<Assignment> {
    panes
        .iter()
        .zip(beads.iter())
        .take(max)
        .map(|(pane, bead)| Assignment {
            pane: pane.clone(),
            bead: bead.clone(),
        })
        .collect()
}

/// Minimum bytes for a packet to be worth sending.
///
/// A worker handed a title and no spec invents the rest, and invents it differently
/// every time. Refusing is strictly better than dispatching a stub.
pub const MIN_PACKET_BYTES: usize = 400;

/// Is this rendered packet substantial enough to send?
pub const fn packet_is_sendable(bytes: usize) -> bool {
    bytes > MIN_PACKET_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACT_2_AND_4: &str = r#"{"agents":[
        {"pane":"2","safe_to_dispatch":true},
        {"pane":"3","safe_to_dispatch":false},
        {"pane":"4","safe_to_dispatch":true}]}"#;

    /// The measured 2026-08-27 case: activity says pane 4 is free, the oracle says it
    /// is a bare shell.
    const ORACLE_2_FREE_4_NO_AGENT: &str = r#"{"panes":[
        {"pane":"2","state":"FREE"},
        {"pane":"3","state":"BUSY"},
        {"pane":"4","state":"NO_AGENT"}]}"#;

    #[test]
    fn only_panes_both_surfaces_call_free_are_dispatchable() {
        let s = survey(ACT_2_AND_4, ORACLE_2_FREE_4_NO_AGENT);
        assert_eq!(dispatchable_panes(&s), vec!["2".to_string()]);
    }

    /// THE MEASURED DEFECT. Pane 4 looked dispatchable on one surface and was a bare
    /// shell. Dispatching there sends real work into a void.
    #[test]
    fn a_bare_shell_is_never_dispatchable_even_when_activity_says_yes() {
        let s = survey(ACT_2_AND_4, ORACLE_2_FREE_4_NO_AGENT);
        assert!(s.activity_safe.contains("4"), "fixture must model the disagreement");
        assert!(!dispatchable_panes(&s).contains(&"4".to_string()));
        assert_eq!(refusal_for(&s, "4"), Some(PaneRefusal::OracleNotFree));
    }

    /// ANTI-VACUITY. Without a positive case the selector is a stopped clock that
    /// refuses every pane and reports itself healthy — which is indistinguishable from
    /// a busy fleet and is exactly the outage this crate exists to end.
    #[test]
    fn a_pane_both_surfaces_call_free_is_selected() {
        let s = survey(
            r#"{"agents":[{"pane":"2","safe_to_dispatch":true}]}"#,
            r#"{"panes":[{"pane":"2","state":"FREE"}]}"#,
        );
        assert_eq!(dispatchable_panes(&s), vec!["2".to_string()]);
        assert_eq!(refusal_for(&s, "2"), None);
    }

    #[test]
    fn an_unreadable_probe_refuses_every_pane() {
        for (a, o) in [
            ("not json", r#"{"panes":[{"pane":"2","state":"FREE"}]}"#),
            (r#"{"agents":[{"pane":"2","safe_to_dispatch":true}]}"#, "not json"),
            ("not json", "not json"),
        ] {
            let s = survey(a, o);
            assert!(s.unreadable, "a broken probe must set unreadable");
            assert!(
                dispatchable_panes(&s).is_empty(),
                "unreadable must yield ZERO candidates, never all of them"
            );
            assert_eq!(refusal_for(&s, "2"), Some(PaneRefusal::ProbeUnreadable));
        }
    }

    /// A missing `agents`/`panes` key is malformed, not "nothing is free".
    #[test]
    fn a_missing_key_is_unreadable_not_empty() {
        let s = survey("{}", r#"{"panes":[]}"#);
        assert!(s.unreadable);
    }

    #[test]
    fn busy_is_not_free_but_is_not_a_bare_shell() {
        let s = survey(
            r#"{"agents":[{"pane":"3","safe_to_dispatch":true}]}"#,
            r#"{"panes":[{"pane":"3","state":"BUSY"}]}"#,
        );
        assert!(dispatchable_panes(&s).is_empty());
        assert_eq!(refusal_for(&s, "3"), Some(PaneRefusal::OracleNotFree));
    }

    /// Pane ids arrive as both string and number across surfaces. Dropping one form
    /// silently shrinks the candidate set, which reads as a busier fleet than exists.
    #[test]
    fn numeric_and_string_pane_ids_both_parse() {
        let s = survey(
            r#"{"agents":[{"pane":2,"safe_to_dispatch":true}]}"#,
            r#"{"panes":[{"pane":"2","state":"FREE"}]}"#,
        );
        assert_eq!(dispatchable_panes(&s), vec!["2".to_string()]);
    }

    #[test]
    fn panes_sort_numerically_not_lexically() {
        let s = PaneSurvey {
            activity_safe: ["10", "2", "3"].iter().map(|s| (*s).to_string()).collect(),
            oracle_free: ["10", "2", "3"].iter().map(|s| (*s).to_string()).collect(),
            unreadable: false,
        };
        assert_eq!(dispatchable_panes(&s), vec!["2", "3", "10"]);
    }

    #[test]
    fn recommendations_parse_in_descending_score_order() {
        let text = r#"{"triage":{
            "quick_ref":{"top_picks":[{"id":"cp-wrong","unblocks":99}]},
            "recommendations":[
                {"id":"cp-low","score":0.2},
                {"id":"cp-high","score":0.9},
                {"id":"cp-no-score"},
                {"id":"cp-mid","score":0.5}
            ]}}"#;
        assert_eq!(
            parse_recommendations(text),
            vec!["cp-high", "cp-mid", "cp-low"]
        );
    }

    #[test]
    fn unparseable_triage_yields_no_recommendations() {
        assert!(parse_recommendations("not json").is_empty());
        assert!(parse_recommendations("{}").is_empty());
        assert!(parse_recommendations(
            r#"{"triage":{"quick_ref":{"top_picks":[{"id":"cp-stale"}]}}}"#
        )
        .is_empty());
    }

    #[test]
    fn plan_pairs_panes_with_beads_and_respects_the_cap() {
        let panes = vec!["2".to_string(), "3".to_string(), "4".to_string()];
        let beads = vec!["cp-a".to_string(), "cp-b".to_string(), "cp-c".to_string()];
        assert_eq!(plan(&panes, &beads, 8).len(), 3);
        assert_eq!(plan(&panes, &beads, 2).len(), 2);
        // Fewer beads than panes is not an error: there is simply no more work.
        assert_eq!(plan(&panes, &beads[..1], 8).len(), 1);
        // Fewer panes than beads is not an error either.
        assert_eq!(plan(&panes[..1], &beads, 8).len(), 1);
    }

    #[test]
    fn plan_never_assigns_one_bead_to_two_panes() {
        let panes = vec!["2".to_string(), "3".to_string()];
        let beads = vec!["cp-a".to_string(), "cp-b".to_string()];
        let out = plan(&panes, &beads, 8);
        assert_eq!(out[0].bead, "cp-a");
        assert_eq!(out[1].bead, "cp-b");
        assert_ne!(out[0].bead, out[1].bead, "two panes must not race one bead");
    }

    #[test]
    fn an_undersized_packet_is_refused_and_a_full_one_accepted() {
        assert!(!packet_is_sendable(0));
        assert!(!packet_is_sendable(MIN_PACKET_BYTES));
        // ANTI-VACUITY on the size rule: a real packet must pass, or the guard refuses
        // every dispatch while reporting itself healthy.
        assert!(packet_is_sendable(MIN_PACKET_BYTES + 1));
        assert!(packet_is_sendable(7_347));
    }
}

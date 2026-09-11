#![forbid(unsafe_code)]

use ompo_start::liveness::{classify, LiveVerdict, SourceVerdict};

fn source(name: &str, panes: &[&str], available: bool, fresh: bool) -> SourceVerdict {
    SourceVerdict {
        name: name.to_owned(),
        available,
        fresh,
        reason_code: "fixture".to_owned(),
        age_ms: Some(1),
        panes: panes.iter().map(|pane| (*pane).to_owned()).collect(),
    }
}

#[test]
fn three_fresh_sources_with_equal_panes_are_live() {
    let verdict = classify(vec![
        source("tick-monitor", &["%7", "%8"], true, true),
        source("agent-mail", &["%7", "%8"], true, true),
        source("ntm", &["%7", "%8"], true, true),
    ])
    .expect("complete source set");
    assert!(matches!(verdict, LiveVerdict::Live { .. }));
    assert_eq!(verdict.status(), "LIVE");
}

#[test]
fn silent_third_source_forces_not_live() {
    let verdict = classify(vec![
        source("ntm", &["%7"], true, true),
        source("tick-monitor", &["%7"], true, true),
        source("agent-mail", &["%7"], false, false),
    ])
    .expect("complete source set");
    assert!(matches!(verdict, LiveVerdict::NotLive { .. }));
    assert_eq!(verdict.status(), "NOT_LIVE");
    assert!(verdict.reason_code().contains("agent-mail"));
}

#[test]
fn empty_source_set_is_an_error() {
    let error = classify(Vec::new()).expect_err("empty liveness input must refuse");
    assert!(error.contains("L4_EMPTY_SOURCE_SET"));
}

// ---------------------------------------------------------------------------
// L4-OBS-NTM (xl56): the ntm census must NAME the panes it saw.
// Known-bad: a row carrying only availability, so a source that saw zero panes
// and one that saw three read identically from outside.
// ---------------------------------------------------------------------------

#[test]
fn ntm_census_names_the_panes_present() {
    let ntm = source("ntm", &["%7", "%8"], true, true);
    let row = ompo_start::liveness::source_json(&ntm);
    assert_eq!(
        row["names"],
        serde_json::json!(["%7", "%8"]),
        "the ntm census must list pane NAMES, not just a count"
    );
    assert_eq!(row["pane_count"], serde_json::json!(2));
    assert_eq!(row["age_ms"], serde_json::json!(1), "per-source age_ms");
    assert_eq!(row["name"], serde_json::json!("ntm"));
}

#[test]
fn known_bad_census_without_names_is_indistinguishable() {
    // The pre-fix row shape: availability only. Two sources that saw DIFFERENT
    // pane sets produce byte-identical rows, which is the defect.
    let seen_none = source("ntm", &[], true, true);
    let seen_three = source("ntm", &["%7", "%8", "%9"], true, true);
    let known_bad = |s: &ompo_start::liveness::SourceVerdict| {
        serde_json::json!({"available": s.available, "fresh": s.fresh})
    };
    assert_eq!(
        known_bad(&seen_none),
        known_bad(&seen_three),
        "KNOWN-BAD: the availability-only row cannot tell 0 panes from 3"
    );
    // The writer under test separates them.
    assert_ne!(
        ompo_start::liveness::source_json(&seen_none),
        ompo_start::liveness::source_json(&seen_three),
        "the census writer must distinguish the two"
    );
}

#[test]
fn sources_json_is_keyed_by_canonical_and_short_name() {
    let sources = vec![
        source("ntm", &["%7"], true, true),
        source("tick-monitor", &["%7"], true, true),
        source("agent-mail", &["%7"], true, true),
    ];
    let map = ompo_start::liveness::sources_json(&sources);
    for key in ["ntm", "tick", "mail", "tick-monitor", "agent-mail"] {
        assert!(
            map.get(key).is_some(),
            "jq .liveness.sources.{key} must not read null"
        );
    }
    assert_eq!(map["tick"], map["tick-monitor"], "alias carries the same row");
    assert_eq!(map["tick"]["name"], serde_json::json!("tick-monitor"));
}

#[test]
fn all_fresh_is_false_for_an_empty_or_silent_source_set() {
    let fresh = vec![
        source("ntm", &["%7"], true, true),
        source("tick-monitor", &["%7"], true, true),
        source("agent-mail", &["%7"], true, true),
    ];
    assert!(ompo_start::liveness::all_fresh(&fresh), "known-good leg");
    assert!(
        !ompo_start::liveness::all_fresh(&[]),
        "ANTI-VACUITY: nothing observed must not read as all fresh"
    );
    let mut silent = fresh.clone();
    silent[2].age_ms = None;
    assert!(
        !ompo_start::liveness::all_fresh(&silent),
        "a source with no age is SILENT, so not all_fresh"
    );
}

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

// ---------------------------------------------------------------------------
// L4-OBS-TICK (17nw): gap_secs, and EITHER an age or silent=true — never neither.
// Known-bad: observed_at absent read as age 0, i.e. the freshest possible answer
// for the source that answered least.
// ---------------------------------------------------------------------------

#[test]
fn tick_row_carries_gap_secs_derived_from_age() {
    let mut tick = source("tick-monitor", &["%7"], true, true);
    tick.age_ms = Some(90_500);
    let row = ompo_start::liveness::source_json(&tick);
    assert_eq!(row["age_ms"], serde_json::json!(90_500));
    assert_eq!(row["gap_secs"], serde_json::json!(90), "whole-second gap");
    assert_eq!(row["silent"], serde_json::json!(false));
}

#[test]
fn tick_row_with_no_observed_at_is_silent_not_zero() {
    let mut tick = source("tick-monitor", &["%7"], true, true);
    tick.age_ms = None;
    let row = ompo_start::liveness::source_json(&tick);
    // KNOWN-BAD, pinned: the pre-fix reading of a missing observed_at.
    let known_bad_gap = tick.age_ms.unwrap_or(0) / 1000;
    assert_eq!(known_bad_gap, 0, "KNOWN-BAD: absent observed_at read as gap 0s");
    // The writer under test refuses that default.
    assert_eq!(row["gap_secs"], serde_json::Value::Null, "unmeasured, not 0");
    assert_eq!(row["age_ms"], serde_json::Value::Null);
    assert_eq!(
        row["silent"],
        serde_json::json!(true),
        "acceptance: age_ms OR silent=true"
    );
}

#[test]
fn every_source_row_carries_an_age_or_silent_true() {
    // The acceptance is a disjunction, so the invariant is that it can never be
    // unsatisfied: no row may have a null age AND silent=false.
    for age in [None, Some(0u64), Some(1), Some(300_000)] {
        for (available, fresh) in [(true, true), (true, false), (false, false)] {
            let mut probe = source("tick-monitor", &["%7"], available, fresh);
            probe.age_ms = age;
            let row = ompo_start::liveness::source_json(&probe);
            let has_age = !row["age_ms"].is_null();
            let silent = row["silent"] == serde_json::json!(true);
            assert!(
                (has_age && !silent) || silent,
                "row satisfies neither leg: {row}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// L4-OBS-MAIL (l2de): the mail row emits a derived age or silent=true WITH a
// reason. Known-bad: _meta absent is SILENT, and a bare boolean cannot tell
// "no _meta.timestamp" from "the probe never answered".
// ---------------------------------------------------------------------------

#[test]
fn mail_row_with_meta_timestamp_emits_derived_age() {
    let mut mail = source("agent-mail", &["%7"], true, true);
    mail.age_ms = Some(4_200);
    let map = ompo_start::liveness::sources_json(std::slice::from_ref(&mail));
    let row = &map["mail"];
    assert_eq!(row["age_ms"], serde_json::json!(4_200), "derived from _meta");
    assert_eq!(row["silent"], serde_json::json!(false));
    assert_eq!(
        row["silent_reason"],
        serde_json::Value::Null,
        "a non-silent row carries no reason"
    );
}

#[test]
fn mail_row_without_meta_is_silent_with_a_distinguishing_reason() {
    let mut no_meta = source("agent-mail", &["%7"], true, true);
    no_meta.age_ms = None;
    let mut unavailable = source("agent-mail", &["%7"], false, false);
    unavailable.age_ms = None;
    // KNOWN-BAD, pinned: the bare boolean collapses two different defects.
    assert_eq!(
        ompo_start::liveness::is_silent(&no_meta),
        ompo_start::liveness::is_silent(&unavailable),
        "KNOWN-BAD: silent=true alone cannot tell absent _meta from a dead probe"
    );
    // The writer under test separates them.
    assert_eq!(
        ompo_start::liveness::silent_reason(&no_meta).as_deref(),
        Some("L4_SILENT_NO_TIMESTAMP")
    );
    assert_eq!(
        ompo_start::liveness::silent_reason(&unavailable).as_deref(),
        Some("L4_SILENT_UNAVAILABLE")
    );
    let row = ompo_start::liveness::source_json(&no_meta);
    assert_eq!(row["silent"], serde_json::json!(true));
    assert_eq!(
        row["silent_reason"],
        serde_json::json!("L4_SILENT_NO_TIMESTAMP")
    );
}

#[test]
fn silent_reason_is_null_exactly_when_not_silent() {
    for age in [None, Some(7u64)] {
        for (available, fresh) in [(true, true), (true, false), (false, false)] {
            let mut probe = source("agent-mail", &["%7"], available, fresh);
            probe.age_ms = age;
            assert_eq!(
                ompo_start::liveness::silent_reason(&probe).is_some(),
                ompo_start::liveness::is_silent(&probe),
                "reason and boolean must agree for available={available} fresh={fresh} age={age:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// L4-OBS-AGREE (l7ve): pane-set equality writer. Two agreeing sources plus a
// third SILENT must not report live=true, and an all-empty census is NOT
// agreement: empty sets are trivially equal, which is how a dead swarm reads
// as live.
// ---------------------------------------------------------------------------

#[test]
fn silent_third_is_not_live() {
    let verdict = classify(vec![
        source("ntm", &["%7", "%8"], true, true),
        source("tick-monitor", &["%7", "%8"], true, true),
        source("agent-mail", &["%7", "%8"], false, false),
    ])
    .expect("complete source set");
    println!(
        "L4_AGREE status={} reason_code={}",
        verdict.status(),
        verdict.reason_code()
    );
    assert_eq!(
        verdict.status(),
        "NOT_LIVE",
        "two sources agreeing plus a silent third is NOT live"
    );
    assert!(verdict.reason_code().contains("agent-mail"));
    // The two that DID answer agree — so agreement alone is not the verdict.
    let agreement = ompo_start::liveness::pane_set_agreement(&[
        source("ntm", &["%7", "%8"], true, true),
        source("tick-monitor", &["%8", "%7"], true, true),
    ]);
    assert_eq!(agreement["agree"], serde_json::json!(true));
}

#[test]
fn agreement_emits_the_three_pane_sets_and_a_verdict() {
    let sources = vec![
        source("ntm", &["%8", "%7"], true, true),
        source("tick-monitor", &["%7", "%8"], true, true),
        source("agent-mail", &["%7", "%8", "%8"], true, true),
    ];
    let agreement = ompo_start::liveness::pane_set_agreement(&sources);
    assert_eq!(
        agreement["agree"],
        serde_json::json!(true),
        "order and duplicates are not disagreement: agreement is a SET question"
    );
    assert_eq!(agreement["reason_code"], serde_json::json!("L4_PANE_SET_AGREE"));
    assert_eq!(agreement["source_count"], serde_json::json!(3));
    for key in ["ntm", "tick", "mail"] {
        assert_eq!(
            agreement["pane_sets"][key],
            serde_json::json!(["%7", "%8"]),
            "the writer must emit the {key} pane set it compared"
        );
    }
}

#[test]
fn two_empty_pane_sets_are_equal_but_are_not_agreement() {
    let empty = vec![
        source("ntm", &[], true, true),
        source("tick-monitor", &[], true, true),
        source("agent-mail", &[], true, true),
    ];
    // KNOWN-BAD, pinned: raw equality says these three agree.
    let raw: Vec<&Vec<String>> = empty.iter().map(|s| &s.panes).collect();
    assert!(
        raw.windows(2).all(|pair| pair[0] == pair[1]),
        "KNOWN-BAD: empty sets ARE equal, which is why equality alone is unsafe"
    );
    // The writer under test refuses to call that agreement.
    let agreement = ompo_start::liveness::pane_set_agreement(&empty);
    assert_eq!(
        agreement["agree"],
        serde_json::json!(false),
        "ANTI-VACUITY: three sources that saw nothing agree about nothing"
    );
    assert_eq!(
        agreement["reason_code"],
        serde_json::json!("L4_PANE_SET_VACUOUS")
    );
    // And classify must not report the swarm live off a vacuous census.
    let verdict = classify(empty).expect("complete source set");
    assert_eq!(verdict.status(), "NOT_LIVE");
    assert_eq!(verdict.reason_code(), "L4_PANE_SET_VACUOUS");
}

#[test]
fn disagreeing_pane_sets_are_named_as_disagreement_not_vacuity() {
    let agreement = ompo_start::liveness::pane_set_agreement(&[
        source("ntm", &["%7"], true, true),
        source("tick-monitor", &["%9"], true, true),
        source("agent-mail", &["%7"], true, true),
    ]);
    assert_eq!(agreement["agree"], serde_json::json!(false));
    assert_eq!(
        agreement["reason_code"],
        serde_json::json!("L4_PANE_SET_DISAGREE"),
        "a non-empty mismatch is disagreement, a distinct defect from vacuity"
    );
}

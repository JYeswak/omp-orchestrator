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

/// L4-LIVE (hw99) and L4-TEST-THREE-FRESH (3feu). Both acceptances name this
/// selector, so the leg carries the name rather than a synonym: a command that
/// matches no test exits 0 and proves nothing. Renamed from
/// `three_fresh_sources_with_equal_panes_are_live` — one authority, not two legs
/// asserting the same law under different names.
///
/// LIVE requires all THREE of: every required source present, every one fresh
/// with an age, and their pane sets equal. Each is dropped in turn below so the
/// leg cannot pass on the strength of only one of them.
#[test]
fn live_requires_three_fresh_agreeing() {
    let verdict = classify(vec![
        source("tick-monitor", &["%7", "%8"], true, true),
        source("agent-mail", &["%7", "%8"], true, true),
        source("ntm", &["%7", "%8"], true, true),
    ])
    .expect("complete source set");
    assert!(matches!(verdict, LiveVerdict::Live { .. }));
    assert_eq!(verdict.status(), "LIVE");

    // 1. THREE: two of the three is not live, and the absence is NAMED.
    let two_only = classify(vec![
        source("ntm", &["%7", "%8"], true, true),
        source("tick-monitor", &["%7", "%8"], true, true),
    ])
    .expect("a short source set still classifies");
    assert_eq!(two_only.status(), "NOT_LIVE");
    assert!(
        two_only.reason_code().contains("agent-mail"),
        "the missing source must be named: {}",
        two_only.reason_code()
    );

    // 2. FRESH: one source with no age is SILENT, so not live.
    let mut ageless = source("agent-mail", &["%7", "%8"], true, true);
    ageless.age_ms = None;
    let silent = classify(vec![
        ageless,
        source("ntm", &["%7", "%8"], true, true),
        source("tick-monitor", &["%7", "%8"], true, true),
    ])
    .expect("complete source set");
    assert_eq!(silent.status(), "NOT_LIVE", "a source with no age is silent");

    // 3. AGREEING: three fresh sources that disagree on panes are not live.
    let disagreeing = classify(vec![
        source("ntm", &["%7", "%8"], true, true),
        source("tick-monitor", &["%7"], true, true),
        source("agent-mail", &["%7", "%8"], true, true),
    ])
    .expect("complete source set");
    assert_eq!(disagreeing.status(), "NOT_LIVE");
    println!(
        "LIVE requires three+fresh+agreeing; drops: {} / {} / {}",
        two_only.reason_code(),
        silent.reason_code(),
        disagreeing.reason_code()
    );
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

/// L4-SILENT (gbyo): a REQUIRED source that cannot produce the freshness
/// quadruple — available / fresh / reason_code / age_ms — is Silent, and Silent
/// is NotLive. The two failure shapes are distinct and both must refuse:
///   present-but-unfreshable: in the vector, age_ms None
///   absent entirely:         not in the vector at all
/// An ABSENT source must NOT read as fresh; absence is the defect this leg
/// exists to catch, because a missing row is the easiest thing for a
/// population-driven classifier to score as all-fresh.
#[test]
fn silent_is_not_live() {
    // KNOWN-GOOD control first: the same three names, all freshness-complete,
    // are LIVE — so this leg is not simply refusing everything.
    let live = classify(vec![
        source("ntm", &["%7"], true, true),
        source("tick-monitor", &["%7"], true, true),
        source("agent-mail", &["%7"], true, true),
    ])
    .expect("complete source set");
    assert_eq!(live.status(), "LIVE", "the control must be LIVE");

    // 1. PRESENT BUT LACKING FRESHNESS: available and fresh both claim true,
    //    but no age was produced. A claim of freshness with no age behind it is
    //    not freshness.
    let unfreshable = SourceVerdict {
        name: "agent-mail".to_owned(),
        available: true,
        fresh: true,
        reason_code: String::new(),
        age_ms: None,
        panes: vec!["%7".to_owned()],
    };
    let verdict = classify(vec![
        source("ntm", &["%7"], true, true),
        source("tick-monitor", &["%7"], true, true),
        unfreshable,
    ])
    .expect("three named sources is a complete set");
    println!(
        "L4_SILENT_NO_AGE status={} reason_code={}",
        verdict.status(),
        verdict.reason_code()
    );
    assert!(
        matches!(verdict, LiveVerdict::NotLive { .. }),
        "a required source with no age_ms cannot be Live"
    );
    assert_eq!(verdict.status(), "NOT_LIVE");
    assert!(
        verdict.reason_code().contains("agent-mail"),
        "the reason must name the silent source, got {}",
        verdict.reason_code()
    );

    // 2. ABSENT ENTIRELY: the row is missing, not merely stale. This must NOT
    //    read as fresh, and it must not be scored against the population that
    //    happens to be present.
    let absent = classify(vec![
        source("ntm", &["%7"], true, true),
        source("tick-monitor", &["%7"], true, true),
    ])
    .expect("a short vector is a verdict, not a parse error");
    println!(
        "L4_SILENT_ABSENT status={} reason_code={}",
        absent.status(),
        absent.reason_code()
    );
    assert!(
        matches!(absent, LiveVerdict::NotLive { .. }),
        "an absent required source must not read as fresh"
    );
    assert_eq!(absent.status(), "NOT_LIVE");
    assert!(
        absent.reason_code().contains("agent-mail"),
        "the reason must name the source that never reported, got {}",
        absent.reason_code()
    );

    // 3. STALE is also Silent: an age was produced, but the source disclaims
    //    freshness. Stale must not be rescued by the presence of an age_ms.
    let stale = classify(vec![
        source("ntm", &["%7"], true, true),
        source("tick-monitor", &["%7"], true, true),
        source("agent-mail", &["%7"], true, false),
    ])
    .expect("complete source set");
    assert_eq!(stale.status(), "NOT_LIVE", "a stale required source is Silent");
    assert!(stale.reason_code().contains("agent-mail"));
}

/// L4-SRC-MAIL (fols), landed where the bead's acceptance command points:
/// `cargo test -p ompo-start --test l4_liveness missing_mail_meta_is_silent`.
/// Without this fn that command matched no test and exited 0 — a silent green.
/// The full battery lives in `tests/l4_mail.rs`; this leg is not a delegating
/// stub, it asserts the derivation AND the swarm verdict end to end.
///
/// AN ABSENT TIMESTAMP IS NOT AGE ZERO. `am robot status` with no
/// `_meta.timestamp` yields SILENT with `age_ms` UNKNOWN, and one silent source
/// makes the swarm NOT_LIVE.
#[test]
fn missing_mail_meta_is_silent() {
    // The observer clock, deliberately far in the future (year 2096 in epoch ms)
    // so the control envelope below yields a POSITIVE age. For the absent case the
    // reading does not matter: with no writer clock the difference is UNKNOWN.
    let observer_ms = 4_000_000_000_000_i64;
    let no_meta = r#"{"_alerts":[],"sessions":[]}"#;

    let freshness =
        ompo_start::mail::mail_freshness(no_meta, observer_ms).expect("the envelope is JSON");
    println!(
        "SILENT status={} reason={}",
        freshness.status(),
        freshness.reason_code()
    );
    assert_eq!(freshness.status(), "SILENT");
    assert_eq!(freshness.age_ms(), None, "absent must be UNKNOWN");
    assert_ne!(
        freshness.age_ms(),
        Some(0),
        "an absent _meta.timestamp must NOT read as age 0"
    );

    // KNOWN-GOOD control in the same leg, so SILENT is not simply what this
    // returns for every input: a stamped envelope yields a real age.
    let stamped = r#"{"_meta":{"timestamp":"2026-09-11T18:05:39.843+00:00"}}"#;
    let fresh = ompo_start::mail::mail_freshness(stamped, observer_ms).expect("stamped is JSON");
    assert_eq!(fresh.status(), "FRESH");
    assert!(fresh.age_ms().is_some());

    // And SILENT propagates: the mail source cannot carry the swarm.
    let silent_mail = ompo_start::mail::mail_source(no_meta, observer_ms, vec!["%7".to_owned()])
        .expect("the envelope is JSON");
    assert!(!silent_mail.fresh);
    assert_eq!(silent_mail.age_ms, None);
    let verdict = classify(vec![
        silent_mail,
        source("ntm", &["%7"], true, true),
        source("tick-monitor", &["%7"], true, true),
    ])
    .expect("complete source set");
    println!("SILENT swarm={} {}", verdict.status(), verdict.reason_code());
    assert!(matches!(verdict, LiveVerdict::NotLive { .. }));
    assert_eq!(verdict.status(), "NOT_LIVE");
    assert!(verdict.reason_code().contains("agent-mail"));
}

/// L4-NOT-LIVE (8r0r): three fresh sources that disagree on their pane sets are
/// NOT_LIVE, and the verdict says DISAGREEMENT rather than something vaguer.
/// The law is adjacent to `disagreeing_pane_sets_are_named_as_disagreement_not_vacuity`,
/// which asserts the AGREEMENT WRITER's json; this leg asserts the CLASSIFIER's
/// verdict, which is the different thing 8r0r's acceptance names.
#[test]
fn pane_set_disagree_is_not_live() {
    // KNOWN-GOOD control: identical pane sets, same three sources, LIVE.
    let agreeing = classify(vec![
        source("ntm", &["%7", "%8"], true, true),
        source("tick-monitor", &["%7", "%8"], true, true),
        source("agent-mail", &["%7", "%8"], true, true),
    ])
    .expect("complete source set");
    assert_eq!(agreeing.status(), "LIVE", "the control must be LIVE");

    // Every source is available, fresh and aged. ONLY the pane sets differ, so a
    // freshness-only classifier would call this live.
    let verdict = classify(vec![
        source("ntm", &["%7", "%8"], true, true),
        source("tick-monitor", &["%7", "%9"], true, true),
        source("agent-mail", &["%7", "%8"], true, true),
    ])
    .expect("complete source set");
    println!("DISAGREE verdict={} {}", verdict.status(), verdict.reason_code());
    assert!(matches!(verdict, LiveVerdict::NotLive { .. }));
    assert_eq!(verdict.status(), "NOT_LIVE");
    assert!(
        verdict.reason_code().contains("DISAGREE"),
        "the reason must name disagreement, not merely refuse: {}",
        verdict.reason_code()
    );
    // Order must not decide it: the same three in any order disagree identically.
    let reordered = classify(vec![
        source("agent-mail", &["%7", "%8"], true, true),
        source("ntm", &["%7", "%8"], true, true),
        source("tick-monitor", &["%7", "%9"], true, true),
    ])
    .expect("complete source set");
    assert_eq!(reordered.reason_code(), verdict.reason_code());
}

/// L4-METRIC-SILENT (vdxb): a nonzero SILENT COUNT makes the swarm NOT_LIVE, and
/// zero silent sources is the only state that can be LIVE. The count is derived
/// from `is_silent`.
///
/// ⛔ CORRECTED (j4ert). This comment previously claimed that deriving the count
/// from `is_silent` meant "the metric and the verdict cannot disagree about what
/// silent means". THAT WAS FALSE WHEN WRITTEN: `classify` re-implemented the
/// condition inline and `all_fresh` carried it a third time in the NEGATED
/// spelling, so three copies with disjoint consumers held one law and nothing
/// pinned them. It was disproved, not argued: gutting `is_silent` left every
/// verdict leg GREEN. The sentence is true only because 3274c9f collapsed
/// `classify` onto `is_silent` and this commit collapsed `all_fresh` too —
/// asserted by `the_row_writer_and_the_verdict_share_one_silence_predicate`, and
/// measured by a mutation of `is_silent` that now reddens row-writer, verdict AND
/// all_fresh legs in one run where it previously reddened none of the last group.
///
/// A guarantee a comment asserts is worth nothing until something fails when it
/// stops holding.
#[test]
fn silent_count_nonzero_is_not_live() {
    fn silent_count(sources: &[SourceVerdict]) -> usize {
        sources
            .iter()
            .filter(|source| ompo_start::liveness::is_silent(source))
            .count()
    }

    let all_fresh = vec![
        source("ntm", &["%7"], true, true),
        source("tick-monitor", &["%7"], true, true),
        source("agent-mail", &["%7"], true, true),
    ];
    assert_eq!(silent_count(&all_fresh), 0, "known-good: nothing is silent");
    assert_eq!(
        classify(all_fresh.clone())
            .expect("complete source set")
            .status(),
        "LIVE",
        "zero silent is the only state that may be LIVE"
    );

    // Each of the two silent SHAPES independently drives the count up and the
    // verdict down: no age at all, and an unavailable probe.
    for (label, mutate) in [
        (
            "no age",
            (|s: &mut SourceVerdict| s.age_ms = None) as fn(&mut SourceVerdict),
        ),
        ("unavailable", |s: &mut SourceVerdict| {
            s.available = false;
            s.fresh = false;
        }),
    ] {
        let mut sources = all_fresh.clone();
        mutate(&mut sources[2]);
        let count = silent_count(&sources);
        let verdict = classify(sources).expect("complete source set");
        println!("SILENT_COUNT {label} count={count} verdict={}", verdict.status());
        assert_eq!(count, 1, "{label} must count as exactly one silent source");
        assert_eq!(verdict.status(), "NOT_LIVE", "{label}");
        assert!(verdict.reason_code().contains("agent-mail"), "{label}");
    }

    // ANTI-VACUITY: an empty set has a silent count of 0, and that must NOT read
    // as live — zero silent out of nothing observed is not health.
    assert_eq!(silent_count(&[]), 0);
    let empty = classify(Vec::new()).expect_err("an empty source set must refuse");
    assert!(empty.contains("L4_EMPTY_SOURCE_SET"), "{empty}");
}

/// L4-TEST-MISSING-GAP (z7dj): a source with no observed_at emits SILENT, never
/// `gap_secs = 0` or `age_ms = 0`. Zero is the freshest possible reading and is
/// exactly what an absent observation must not be reported as.
#[test]
fn missing_gap_is_silent() {
    let mut absent = source("tick-monitor", &["%7"], true, true);
    absent.age_ms = None;
    let row = ompo_start::liveness::source_json(&absent);
    println!("MISSING_GAP row={row}");

    // KNOWN-BAD, pinned: the unwrap_or(0) reading an implementation reaches for.
    assert_eq!(absent.age_ms.unwrap_or(0) / 1_000, 0, "KNOWN-BAD is gap 0s");

    // The writer refuses it: UNMEASURED, and it says why.
    assert_eq!(row["gap_secs"], serde_json::Value::Null, "SILENT, not 0");
    assert_eq!(row["age_ms"], serde_json::Value::Null, "SILENT, not 0");
    assert_ne!(row["gap_secs"], serde_json::json!(0));
    assert_ne!(row["age_ms"], serde_json::json!(0));
    assert_eq!(row["silent"], serde_json::json!(true));
    assert_eq!(
        row["silent_reason"],
        serde_json::json!("L4_SILENT_NO_TIMESTAMP"),
        "absent observed_at is distinguishable from a dead probe"
    );

    // KNOWN-GOOD, so the leg is not a blanket ban on zero: a genuine zero gap is
    // reported as a zero gap, because both clocks were read.
    let mut measured_zero = source("tick-monitor", &["%7"], true, true);
    measured_zero.age_ms = Some(0);
    let zero_row = ompo_start::liveness::source_json(&measured_zero);
    assert_eq!(zero_row["age_ms"], serde_json::json!(0));
    assert_eq!(zero_row["gap_secs"], serde_json::json!(0));
    assert_eq!(zero_row["silent"], serde_json::json!(false));
    assert_eq!(zero_row["silent_reason"], serde_json::Value::Null);
}

// ---------------------------------------------------------------------------
// ONE AUTHORITY FOR SILENCE. Two graders mutating from two sites measured that
// `is_silent` fed ONLY the row writer while `classify` re-implemented the same
// condition inline, so gutting the predicate left every verdict leg green — and
// a doc comment in this file claimed the metric and the verdict "cannot
// disagree about what silent means". They could. This leg makes the seam unable
// to return: it asserts the ROW WRITER and the SWARM VERDICT answer from the
// same predicate across the whole matrix, so a future inline copy reddens here.
// ---------------------------------------------------------------------------

#[test]
fn the_row_writer_and_the_verdict_share_one_silence_predicate() {
    let mut checked = 0usize;
    for age in [None, Some(0u64), Some(1), Some(300_000)] {
        for (available, fresh) in [(true, true), (true, false), (false, true), (false, false)] {
            let mut mail = source("agent-mail", &["%7"], available, fresh);
            mail.age_ms = age;
            let predicate = ompo_start::liveness::is_silent(&mail);

            // The ROW WRITER's encoding of the same fact.
            let row = ompo_start::liveness::source_json(&mail);
            assert_eq!(
                row["silent"],
                serde_json::json!(predicate),
                "row writer disagrees with is_silent for available={available} fresh={fresh} age={age:?}"
            );

            // The SWARM VERDICT's use of the same fact: the other two sources are
            // fresh and agree, so the verdict can only turn on this one.
            let verdict = classify(vec![
                source("ntm", &["%7"], true, true),
                source("tick-monitor", &["%7"], true, true),
                mail.clone(),
            ])
            .expect("complete source set");
            if predicate {
                assert_eq!(
                    verdict.status(),
                    "NOT_LIVE",
                    "a silent source must not be live: available={available} fresh={fresh} age={age:?}"
                );
                assert_eq!(
                    verdict.reason_code(),
                    "L4_SILENT_SOURCE source=agent-mail",
                    "the verdict must name the source the predicate flagged"
                );
            } else {
                assert_eq!(
                    verdict.status(),
                    "LIVE",
                    "a non-silent, agreeing set must stay live: age={age:?}"
                );
            }
            checked += 1;
        }
    }
    // ANTI-VACUITY: an empty matrix would satisfy every assertion above.
    assert_eq!(checked, 16, "the matrix must actually have been walked");
}

/// LAW-L4-NO-TMUX-RAW: `LiveVerdict` has no bool `tmux has-session` constructor.
///
/// `tmux has-session` is a bool with no `age_ms`. Feeding that shape as a
/// required source must be `NOT_LIVE` / silent, never `Live`. Construction is
/// `classify(Vec<SourceVerdict>)` only — there is no `from_tmux_bool`.
#[test]
fn no_bool_tmux_source() {
    let live = classify(vec![
        source("ntm", &["%7"], true, true),
        source("tick-monitor", &["%7"], true, true),
        source("agent-mail", &["%7"], true, true),
    ])
    .expect("complete source set");
    assert_eq!(live.status(), "LIVE", "control must be LIVE");

    // has-session shape: available + claimed-fresh, no age. Cannot be Live.
    let has_session_bool = SourceVerdict {
        name: "agent-mail".to_owned(),
        available: true,
        fresh: true,
        reason_code: String::new(),
        age_ms: None,
        panes: vec!["%7".to_owned()],
    };
    let verdict = classify(vec![
        source("ntm", &["%7"], true, true),
        source("tick-monitor", &["%7"], true, true),
        has_session_bool,
    ])
    .expect("complete source set");
    println!(
        "NO_BOOL_TMUX status={} reason_code={}",
        verdict.status(),
        verdict.reason_code()
    );
    assert!(
        matches!(verdict, LiveVerdict::NotLive { .. }),
        "a bool has-session source (no age_ms) must not be Live"
    );
    assert_eq!(verdict.status(), "NOT_LIVE");
    assert!(
        verdict.reason_code().contains("agent-mail"),
        "silent source must be named, got {}",
        verdict.reason_code()
    );

    let empty = classify(Vec::new());
    assert!(
        empty.is_err(),
        "empty source set is an error, never a vacuous Live"
    );
    assert!(
        empty.unwrap_err().contains("L4_EMPTY_SOURCE_SET"),
        "empty-set error must be typed"
    );
}

/// L4-SPAWN (ol44): `ntm spawn --assign --cass-context` proceeds only when
/// the swarm is NotLive AND the HD-0010 row is decided. Both refusals are
/// typed `SpawnGate::Refused` values naming the bar — never a silent skip,
/// never an `Ok` with nothing behind it.
#[test]
fn spawn_refused_without_hd0010() {
    use ompo_start::spawn::{spawn_gate, SpawnGate};

    // KNOWN-GOOD control first: NotLive + decided ALLOWS — the gate does not
    // refuse everything, so the refusal below is load-bearing, not vacuous.
    let mut ageless = source("agent-mail", &["%7"], true, true);
    ageless.age_ms = None;
    let not_live = classify(vec![
        source("ntm", &["%7"], true, true),
        source("tick-monitor", &["%7"], true, true),
        ageless,
    ])
    .expect("complete source set");
    assert_eq!(not_live.status(), "NOT_LIVE", "the control must be NotLive");
    assert!(
        matches!(spawn_gate(&not_live, true), SpawnGate::Allowed),
        "NotLive + decided HD-0010 must allow spawn"
    );

    // TARGET: NotLive WITHOUT HD-0010 refuses, and the reason names HD-0010.
    let refused = spawn_gate(&not_live, false);
    println!("L4_SPAWN_NO_HD0010 gate={refused:?}");
    let reason = match refused {
        SpawnGate::Refused { reason } => reason,
        SpawnGate::Allowed => panic!("spawn without HD-0010 must refuse, not allow"),
    };
    assert!(
        reason.contains("HD-0010"),
        "the refusal must name HD-0010, got {reason}"
    );

    // ONLY-WHEN-NOTLIVE: a Live swarm refuses even with HD-0010 decided —
    // spawn is the NotLive recovery path, not a second launcher.
    let live = classify(vec![
        source("ntm", &["%7"], true, true),
        source("tick-monitor", &["%7"], true, true),
        source("agent-mail", &["%7"], true, true),
    ])
    .expect("complete source set");
    assert_eq!(live.status(), "LIVE");
    assert!(
        matches!(spawn_gate(&live, true), SpawnGate::Refused { .. }),
        "a Live swarm must refuse spawn even when HD-0010 is decided"
    );
}

/// L4-SPAWN-RECHECK (hcik): after a spawn, liveness is re-read. A swarm
/// still NotLive must HALT with the verdict's cause attached — never proceed
/// to L5 on a pre-spawn reading, and never halt silently.
#[test]
fn post_spawn_not_live_halts() {
    use ompo_start::spawn::{post_spawn_recheck, Recheck};

    // KNOWN-GOOD control first: a Live recheck proceeds — the gate does not
    // halt everything, so the halt below is load-bearing, not vacuous.
    let live = classify(vec![
        source("ntm", &["%7"], true, true),
        source("tick-monitor", &["%7"], true, true),
        source("agent-mail", &["%7"], true, true),
    ])
    .expect("complete source set");
    assert_eq!(live.status(), "LIVE");
    assert!(
        matches!(post_spawn_recheck(&live), Recheck::ProceedToL5),
        "a Live recheck must proceed to L5"
    );

    // TARGET: still NotLive after the spawn halts, naming the cause.
    let mut ageless = source("agent-mail", &["%7"], true, true);
    ageless.age_ms = None;
    let not_live = classify(vec![
        source("ntm", &["%7"], true, true),
        source("tick-monitor", &["%7"], true, true),
        ageless,
    ])
    .expect("complete source set");
    assert_eq!(not_live.status(), "NOT_LIVE");
    let halted = post_spawn_recheck(&not_live);
    println!("L4_RECHECK_HALT recheck={halted:?}");
    let reason = match halted {
        Recheck::Halt { reason } => reason,
        Recheck::ProceedToL5 => panic!("a still-NotLive recheck must halt, not proceed to L5"),
    };
    assert!(
        reason.contains("agent-mail"),
        "the halt must carry the verdict's cause, got {reason}"
    );
}

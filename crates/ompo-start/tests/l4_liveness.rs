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

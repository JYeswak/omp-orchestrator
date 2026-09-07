#![forbid(unsafe_code)]

use loop_queue_filter::{run, Runtime};
use std::collections::BTreeMap;


fn runtime() -> Runtime {
    let dir = std::env::temp_dir().join(format!(
        "loop-queue-filter-starvation-{}",
        std::process::id()
    ));
    let _ = std::fs::create_dir_all(&dir);
    let mut env = BTreeMap::new();
    env.insert(
        "QUEUE_COOLDOWN_FILE".into(),
        dir.join("cooldown.json").display().to_string(),
    );
    env.insert("REPO_DIR".into(), dir.display().to_string());
    Runtime {
        env,
        cwd: dir,
        now: 1_000.0,
    }
}


/// Fires-on-known-bad: empty dispatchable queue + stale-label TrackerBlocked.
/// Mutation: delete `append_blocker_taxonomy_when_starved` from `run`; this goes RED.
#[test]
fn starved_queue_names_tracker_blocked() {
    let input = r#"{"issues":[{"id":"stale-blocked-16l","title":"stale label","status":"blocked","issue_type":"task","priority":0}]}"#;
    let out = run(input, &[], &runtime());
    assert!(
        out.stdout.is_empty(),
        "starved queue must pick nothing, got stdout={:?}",
        out.stdout
    );
    assert!(
        out.stderr.contains("BLOCKER_KIND") && out.stderr.contains("TrackerBlocked"),
        "starved output must name TrackerBlocked, stderr={:?}",
        out.stderr
    );
}

#[test]
fn empty_scan_set_is_vacuous_while_baseline_nonzero() {
    let out = run(r#"{"issues":[]}"#, &[], &runtime());
    assert_ne!(out.code, 0, "empty scan must not pass");
    assert!(
        out.stderr.contains("VacuousWhileBaselineNonzero"),
        "must reuse zey6 vacuity, stderr={:?}",
        out.stderr
    );
}

#[test]
fn wired_path_performs_no_tracker_mutation() {
    let lib = include_str!("../src/lib.rs");
    let main = include_str!("../src/main.rs");
    let select = include_str!("../src/select.rs");
    let selector = include_str!("../src/selector.rs");
    let blob = format!("{lib}\n{main}\n{select}\n{selector}");
    for needle in [
        "br update",
        "br dep",
        "tmux send",
        "Command::new(\"br\")",
        "Command::new(\"tmux\")",
        "decide_redispatch",
    ] {
        assert!(
            !blob.contains(needle),
            "wired path must stay dry; found {needle}"
        );
    }
    assert!(
        lib.contains("append_blocker_taxonomy_when_starved"),
        "call site must exist so the mutation leg can delete it"
    );
}

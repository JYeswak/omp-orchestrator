//! Source-level wiring proof for the resident read-only `ompo ps` observation.
//!
//! This deliberately avoids starting a daemon or broker: the production contract is that the
//! supervisor invokes the bounded child beside tick-monitor observation and keeps old/unavailable
//! installations as typed, non-fatal evidence.

use std::fs;
use std::path::PathBuf;

fn supervisor_source() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/main.rs");
    fs::read_to_string(path).expect("orchestrator source is readable")
}

#[test]
fn run_cycle_owns_the_read_only_ompo_ps_caller_and_exact_argv() {
    let source = supervisor_source();
    let cycle = source
        .split_once("async fn run_cycle")
        .map(|(_, body)| body)
        .expect("run_cycle exists");
    let caller = "let ompo_ps = observe_ompo_ps(cx, config).await?;";
    let parse = "let mut observation = parse_observation";
    assert!(
        cycle.contains(caller),
        "run_cycle must own the reachable ompo observation caller"
    );
    assert!(
        cycle.find(caller).expect("caller present") < cycle.find(parse).expect("parse present"),
        "ompo observation must run adjacent to, and before consuming, tick-monitor output"
    );

    assert!(source.contains("\"ps\".to_owned()"));
    assert!(source.contains("\"--repo\".to_owned()"));
    assert!(source.contains("config.repo.display().to_string()"));
    assert!(source.contains("\"--json\".to_owned()"));
    assert!(source.contains("invoke(cx, config, &config.ompo, &args).await"));
}

#[test]
fn old_or_missing_ompo_is_typed_and_safe_to_observe() {
    let source = supervisor_source();
    assert!(source.contains("UAD_UNKNOWN_VERB"));
    assert!(source.contains("old_ompo_unknown_verb"));
    assert!(source.contains("ompo_binary_unavailable"));
    assert!(source.contains("OMPO_PS_UNMEASURED scope=repo reason={reason}"));
    assert!(source.contains(
        "OMPO_PS_OBSERVATION scope=repo project_scopes={project_scopes} daemon_count={daemon_count}"
    ));
    assert!(!source.contains("broker_token"));
}

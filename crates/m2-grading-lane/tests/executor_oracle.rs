#![forbid(unsafe_code)]

//! Live-executor contract: default path records the artifact oracle and
//! does not send. launchd is the reachable trigger; `--apply` is absent.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn bin() -> PathBuf {
    std::env::var_os("CARGO_BIN_EXE_m2_grading_lane")
        .or_else(|| std::env::var_os("CARGO_BIN_EXE_m2-grading-lane"))
        .map(PathBuf::from)
        .expect("Cargo must set CARGO_BIN_EXE_m2_grading_lane")
}


fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> PathBuf {
    crate_root()
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/m2-grading-lane")
        .to_path_buf()
}

#[test]
fn default_path_records_oracle_and_exits_nonzero() {
    let dir = std::env::temp_dir().join(format!(
        "m2-oracle-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("oracle dir");
    let oracle = dir.join("m2-grading-lane.jsonl");
    let output = Command::new(bin())
        .env("M2_ORACLE", &oracle)
        .env("HOME", &dir)
        .output()
        .expect("spawn m2-grading-lane");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(3),
        "empty feed is ERROR not pass, stderr={stderr}"
    );
    assert!(
        stderr.contains("decide-only"),
        "stderr must name decide-only, stderr={stderr}"
    );
    let body = fs::read_to_string(&oracle).expect("oracle written");
    assert!(
        body.contains("m2-grading-lane.tick.v1"),
        "oracle schema missing: {body}"
    );
    assert!(
        body.contains("DECIDE_ONLY_NO_FEED"),
        "decision missing: {body}"
    );
    assert!(
        body.contains("\"apply\":false"),
        "apply must be false: {body}"
    );
    assert!(
        !body.contains("\"apply\":true"),
        "default must not claim apply: {body}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn apply_is_refused_and_still_records_oracle() {
    let dir = std::env::temp_dir().join(format!(
        "m2-apply-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("oracle dir");
    let oracle = dir.join("m2-grading-lane.jsonl");
    let output = Command::new(bin())
        .arg("--apply")
        .env("M2_ORACLE", &oracle)
        .env("HOME", &dir)
        .output()
        .expect("spawn m2-grading-lane --apply");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "stderr={stderr}");
    let body = fs::read_to_string(&oracle).expect("oracle written");
    assert!(
        body.contains("APPLY_REFUSED_NO_FEED"),
        "refused apply must be recorded: {body}"
    );
    assert!(
        !body.contains("\"apply\":true"),
        "refused apply must not set apply=true: {body}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn bin_source_never_spawns_ntm_or_br() {
    let main = fs::read_to_string(crate_root().join("src/main.rs")).expect("main");
    assert!(
        !main.contains("Command::new(\"ntm\")"),
        "decide-only bin must not spawn ntm"
    );
    assert!(
        !main.contains("Command::new(\"br\")"),
        "decide-only bin must not spawn br"
    );
}

#[test]
fn launchd_plist_is_the_reachable_trigger_and_has_no_apply() {
    let plist = workspace_root()
        .join("launchd/ai.zeststream.omp-orchestrator.m2-grading-lane.plist");
    let text = fs::read_to_string(&plist).unwrap_or_else(|e| {
        panic!("reachable trigger missing at {}: {e}", plist.display())
    });
    assert!(
        text.contains("m2-grading-lane"),
        "plist must name the bin: {text}"
    );
    assert!(
        text.contains("StartInterval"),
        "grading is a tick, not KeepAlive: {text}"
    );
    assert!(
        !text.contains("--apply"),
        "executor must fire the default path, not --apply: {text}"
    );
    assert!(
        !text.contains("KeepAlive"),
        "KeepAlive would tight-loop a nonzero decide-only tick: {text}"
    );
}

#[test]
fn deleting_the_plist_turns_wiring_red() {
    let plist = workspace_root()
        .join("launchd/ai.zeststream.omp-orchestrator.m2-grading-lane.plist");
    assert!(
        plist.is_file(),
        "deleting launchd/ai.zeststream.omp-orchestrator.m2-grading-lane.plist turns this RED"
    );
}

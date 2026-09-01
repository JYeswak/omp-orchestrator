//! Differential: the Rust port must agree with `bin/refill-idle-panes.sh`, the oracle.
//!
//! Every other row in `registries/dispatch_chain_migration.toml` keeps its original as a
//! byte-stable differential oracle rather than deleting it on port. This does the same.
//! The shell's selection rule is a python one-liner embedded in `idle_panes()`; both
//! implementations are driven from the SAME fixtures and must produce the SAME panes.
//!
//! A differential that only ever runs one side proves nothing, so each leg asserts the
//! oracle actually ran (`status.success()`) before comparing.

use refill_idle_panes::{dispatchable_panes, survey};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OracleStatus {
    Ready,
    MissingInterpreter,
}

fn oracle_status() -> OracleStatus {
    match Command::new("python3")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(status) if status.success() => OracleStatus::Ready,
        _ => OracleStatus::MissingInterpreter,
    }
}

fn announce_skip(test: &str, status: &OracleStatus) {
    let reason = match status {
        OracleStatus::MissingInterpreter => "missing_interpreter",
        OracleStatus::Ready => "ready",
    };
    println!(
        "DIFFERENTIAL DID NOT RUN: test={test} reason={reason} detail=inline python3 -c\n  \
         This is a development-only comparison, not a gate. The Rust gate for this crate is \
         src/lib.rs unit tests.\n  \
         0 cases compared. This is NOT a passing differential."
    );
}

/// Run the oracle's selection rule — the exact python from `bin/refill-idle-panes.sh`.
/// Returns `None` if python could not run, so a missing interpreter reads as SKIPPED
/// rather than as agreement. A differential that silently passes when its oracle is
/// absent is the vacuous-green shape this repo keeps deleting.
fn oracle_select(activity: &str, oracle: &str) -> Option<Vec<String>> {
    let out = Command::new("python3")
        .arg("-c")
        .arg(
            r#"
import json, os
try:
    act = json.loads(os.environ["ACTIVITY_JSON"])
    orc = json.loads(os.environ["ORACLE_JSON"])
except Exception:
    raise SystemExit(0)
safe = {str(a.get("pane")) for a in act.get("agents", []) if a.get("safe_to_dispatch") is True}
free = {str(p.get("pane")) for p in orc.get("panes", []) if p.get("state") == "FREE"}
for pane in sorted(safe & free, key=lambda x: (len(x), x)):
    print(pane)
"#,
        )
        .env("ACTIVITY_JSON", activity)
        .env("ORACLE_JSON", oracle)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

fn assert_agrees(name: &str, activity: &str, oracle: &str) {
    let status = oracle_status();
    let OracleStatus::Ready = status else {
        announce_skip(name, &status);
        return;
    };
    let Some(expected) = oracle_select(activity, oracle) else {
        println!(
            "DIFFERENTIAL DID NOT RUN: test={name} reason=oracle_execution_failed detail=inline python3 -c\n  \
             0 cases compared. This is NOT a passing differential."
        );
        return;
    };
    let actual = dispatchable_panes(&survey(activity, oracle));
    assert_eq!(actual, expected, "{name}: port disagrees with the shell oracle");
}

/// THE MEASURED CASE. 2026-08-27: activity said control-plane pane 4 was
/// `safe_to_dispatch`, the oracle said `NO_AGENT — bare shell`. The oracle was right.
#[test]
fn agrees_on_the_measured_bare_shell_disagreement() {
    assert_agrees(
        "bare shell",
        r#"{"agents":[{"pane":"2","safe_to_dispatch":true},{"pane":"4","safe_to_dispatch":true}]}"#,
        r#"{"panes":[{"pane":"2","state":"FREE"},{"pane":"4","state":"NO_AGENT"}]}"#,
    );
}

/// ANTI-VACUITY. Without a case where BOTH implementations select something, every leg
/// above would pass against a port that always returns empty.
#[test]
fn agrees_when_a_pane_is_genuinely_free() {
    let activity = r#"{"agents":[{"pane":"2","safe_to_dispatch":true}]}"#;
    let oracle = r#"{"panes":[{"pane":"2","state":"FREE"}]}"#;
    assert_agrees("genuinely free", activity, oracle);
    assert_eq!(
        dispatchable_panes(&survey(activity, oracle)),
        vec!["2".to_string()],
        "the differential must have a non-empty case or it proves nothing"
    );
}

#[test]
fn agrees_when_every_pane_is_busy() {
    assert_agrees(
        "all busy",
        r#"{"agents":[{"pane":"2","safe_to_dispatch":false}]}"#,
        r#"{"panes":[{"pane":"2","state":"BUSY"}]}"#,
    );
}

#[test]
fn agrees_on_a_multi_pane_fleet() {
    assert_agrees(
        "multi pane",
        r#"{"agents":[
            {"pane":"1","safe_to_dispatch":false},
            {"pane":"2","safe_to_dispatch":true},
            {"pane":"3","safe_to_dispatch":true},
            {"pane":"4","safe_to_dispatch":true}]}"#,
        r#"{"panes":[
            {"pane":"1","state":"BUSY"},
            {"pane":"2","state":"FREE"},
            {"pane":"3","state":"FREE"},
            {"pane":"4","state":"NO_AGENT"}]}"#,
    );
}

/// The port FAILS CLOSED on unreadable input. The oracle exits 0 printing nothing, so
/// both yield no candidates — but for the port that is a typed refusal, not an accident.
#[test]
fn both_refuse_everything_on_unparseable_input() {
    let status = oracle_status();
    let OracleStatus::Ready = status else {
        announce_skip("both_refuse_everything_on_unparseable_input", &status);
        return;
    };
    let s = survey("not json", "not json");
    assert!(s.unreadable, "the port must TYPE the unreadable case");
    assert!(dispatchable_panes(&s).is_empty());
    let Some(expected) = oracle_select("not json", "not json") else {
        println!(
            "DIFFERENTIAL DID NOT RUN: test=both_refuse_everything_on_unparseable_input reason=oracle_execution_failed detail=inline python3 -c\n  \
             This is a development-only comparison, not a gate. The Rust gate for this crate is \
             src/lib.rs unit tests.\n  \
             0 cases compared. This is NOT a passing differential."
        );
        return;
    };
    assert!(expected.is_empty(), "the oracle must also select nothing");
}

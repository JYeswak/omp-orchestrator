//! Differential against `bin/refill-idle-panes.sh` — and, since 2026-09-02, the leg
//! where the Rust port must DIVERGE from it.
//!
//! Every other row in `registries/dispatch_chain_migration.toml` keeps its original as a
//! byte-stable differential oracle rather than deleting it on port. This did the same,
//! and that was correct while the shell's rule was correct.
//!
//! # Why the shell is now the KNOWN-BAD, not the oracle
//!
//! The shell's selection rule is a python one-liner embedded in `idle_panes()`:
//!
//! ```text
//! safe = {a["pane"] for a in act["agents"] if a["safe_to_dispatch"] is True}
//! free = {p["pane"] for p in orc["panes"] if p["state"] == "FREE"}
//! selected = safe & free
//! ```
//!
//! On the verbatim 2026-09-02 03:14:55Z capture that intersection is EMPTY, and the
//! shell reports it by printing nothing and exiting 0. It was refusing correctly — the
//! two surfaces genuinely contradict on panes 1, 2 and 3, both at
//! `observation_confidence: 0.95` — but a silent empty answer is indistinguishable from
//! a quiet fleet, and that is what cost the session.
//!
//! So this file has two kinds of leg:
//!
//! * AGREEMENT legs. Where the surfaces agree, the port must select exactly what the
//!   shell selects. That is the conservatism worth keeping, and dropping it silently
//!   would turn a repair into a regression.
//! * The DIVERGENCE leg. On the live capture both select nothing, and the port must
//!   additionally NAME the contradiction the shell swallowed. The known-bad is the
//!   silence, not the selection, so the leg asserts BOTH: the shell selects nothing,
//!   and the port reports a nonzero per-pane conflict on the same bytes.
//!
//! A differential that only ever runs one side proves nothing, so each leg asserts the
//! oracle actually ran (`status.success()`) before comparing.

use refill_idle_panes::{
    conflict_verdict, decide, parse_activity_view, parse_oracle_view, run_outcome,
};
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

/// Run the shell's selection rule — the exact python from `bin/refill-idle-panes.sh`.
/// Returns `None` if python could not run, so a missing interpreter reads as SKIPPED
/// rather than as agreement. A differential that silently passes when its oracle is
/// absent is the vacuous-green shape this repo keeps deleting.
fn shell_select(activity: &str, oracle: &str) -> Option<Vec<String>> {
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

fn port_select(activity: &str, oracle: &str) -> Vec<String> {
    let (Some(a), Some(o)) = (parse_activity_view(activity), parse_oracle_view(oracle)) else {
        return Vec::new();
    };
    decide(&a, &o).dispatchable
}

fn assert_agrees(name: &str, activity: &str, oracle: &str) {
    let status = oracle_status();
    let OracleStatus::Ready = status else {
        announce_skip(name, &status);
        return;
    };
    let Some(expected) = shell_select(activity, oracle) else {
        println!(
            "DIFFERENTIAL DID NOT RUN: test={name} reason=oracle_execution_failed detail=inline python3 -c\n  \
             0 cases compared. This is NOT a passing differential."
        );
        return;
    };
    assert_eq!(
        port_select(activity, oracle),
        expected,
        "{name}: port disagrees with the shell on a leg where both surfaces are CONFIDENT"
    );
}

/// VERBATIM `ntm --robot-activity=omp-orchestrator`, 2026-09-02 03:14:55Z, trimmed to
/// the fields the two rules read. Panes 2 and 3 are codex.
const LIVE_ACTIVITY: &str = r#"{"agents":[
    {"pane":"1","agent_type":"claude","state":"UNKNOWN","confidence":0.5,"observation_state":"idle",
     "observation_confidence":0.95,"capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":true},
    {"pane":"2","agent_type":"codex","state":"UNKNOWN","confidence":0.5,"observation_state":"working",
     "observation_confidence":0.95,"capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":false},
    {"pane":"3","agent_type":"codex","state":"UNKNOWN","confidence":0.5,"observation_state":"working",
     "observation_confidence":0.95,"capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":false},
    {"pane":"4","agent_type":"omp-glm","state":"ERROR","confidence":0.95,"observation_state":"working",
     "observation_confidence":0.95,"capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":false},
    {"pane":"5","agent_type":"omp-glm","state":"UNKNOWN","confidence":0.5,"observation_state":"working",
     "observation_confidence":0.95,"capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":false}]}"#;

/// VERBATIM `pane-dispatch-ready omp-orchestrator --json`, same minute.
const LIVE_ORACLE: &str = r#"{"panes":[
    {"pane":"0","state":"BUSY"},
    {"pane":"1","state":"BUSY"},
    {"pane":"2","state":"FREE"},
    {"pane":"3","state":"FREE"},
    {"pane":"4","state":"BUSY"},
    {"pane":"5","state":"NO_AGENT"}]}"#;

/// THE DIVERGENCE LEG, and the known-bad reproduced in the same test.
///
/// Both implementations select NOTHING on the live capture, and that selection is
/// CORRECT — the surfaces genuinely contradict. The divergence is in what each one
/// SAYS about it. The shell prints nothing and exits 0; the port must name the
/// conflicting panes and exit nonzero. Asserting only the port would leave the claim
/// "the shell was silent here" unmeasured, which is the entire defect.
#[test]
fn the_port_names_the_conflict_the_shell_rule_reports_as_silence() {
    let status = oracle_status();
    let OracleStatus::Ready = status else {
        announce_skip(
            "the_port_names_the_conflict_the_shell_rule_reports_as_silence",
            &status,
        );
        return;
    };
    let Some(shell) = shell_select(LIVE_ACTIVITY, LIVE_ORACLE) else {
        println!(
            "DIFFERENTIAL DID NOT RUN: test=the_port_names_the_conflict_the_shell_rule_reports_as_silence \
             reason=oracle_execution_failed\n  0 cases compared. This is NOT a passing differential."
        );
        return;
    };
    assert!(
        shell.is_empty(),
        "KNOWN-BAD NOT REPRODUCED: the shell rule selected {shell:?} on the live capture. \
         The whole repair rests on it selecting nothing, and saying nothing, here."
    );
    assert_eq!(
        port_select(LIVE_ACTIVITY, LIVE_ORACLE),
        Vec::<String>::new(),
        "the port must agree that nothing is dispatchable — refusing is right while the \
         surfaces contradict"
    );

    let activity = parse_activity_view(LIVE_ACTIVITY).expect("fixture parses");
    let oracle = parse_oracle_view(LIVE_ORACLE).expect("fixture parses");
    let decision = decide(&activity, &oracle);
    assert_eq!(
        decision.conflicts,
        vec!["1".to_string(), "2".to_string(), "3".to_string()],
        "and it must NAME the three panes the shell's empty set concealed"
    );
    let outcome = run_outcome(&decision, &conflict_verdict(&activity, &oracle));
    assert_eq!(
        outcome.code, 1,
        "the shell exits 0 on this capture; the port must not"
    );
}

/// THE MEASURED CASE, 2026-08-27, with the confidence `ntm` actually publishes when it
/// HAS classified: activity says pane 4 is free, the oracle says bare shell. Both rules
/// must refuse it — the conservatism this repair preserves.
#[test]
fn agrees_on_the_measured_bare_shell_disagreement() {
    assert_agrees(
        "bare shell",
        r#"{"agents":[
            {"pane":"2","state":"IDLE","confidence":0.95,"observation_state":"idle","observation_confidence":0.95,"capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":true},
            {"pane":"4","state":"IDLE","confidence":0.95,"observation_state":"idle","observation_confidence":0.95,"capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":true}]}"#,
        r#"{"panes":[{"pane":"2","state":"FREE"},{"pane":"4","state":"NO_AGENT"}]}"#,
    );
}

/// ANTI-VACUITY. Without a case where BOTH implementations select something, every
/// agreement leg would pass against a port that always returns empty.
#[test]
fn agrees_when_a_pane_is_genuinely_free() {
    let activity =
        r#"{"agents":[{"pane":"2","state":"IDLE","confidence":0.95,"observation_state":"idle","observation_confidence":0.95,"capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":true}]}"#;
    let oracle = r#"{"panes":[{"pane":"2","state":"FREE"}]}"#;
    assert_agrees("genuinely free", activity, oracle);
    assert_eq!(
        port_select(activity, oracle),
        vec!["2".to_string()],
        "the differential must have a non-empty case or it proves nothing"
    );
}

#[test]
fn agrees_when_every_pane_is_busy() {
    assert_agrees(
        "all busy",
        r#"{"agents":[{"pane":"2","state":"THINKING","confidence":0.9,"observation_state":"working","observation_confidence":0.95,"capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":false}]}"#,
        r#"{"panes":[{"pane":"2","state":"BUSY"}]}"#,
    );
}

#[test]
fn agrees_on_a_multi_pane_fleet() {
    assert_agrees(
        "multi pane",
        r#"{"agents":[
            {"pane":"1","state":"THINKING","confidence":0.9,"observation_state":"working","observation_confidence":0.95,"capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":false},
            {"pane":"2","state":"IDLE","confidence":0.95,"observation_state":"idle","observation_confidence":0.95,"capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":true},
            {"pane":"3","state":"IDLE","confidence":0.95,"observation_state":"idle","observation_confidence":0.95,"capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":true},
            {"pane":"4","state":"IDLE","confidence":0.95,"observation_state":"idle","observation_confidence":0.95,"capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":true}]}"#,
        r#"{"panes":[
            {"pane":"1","state":"BUSY"},
            {"pane":"2","state":"FREE"},
            {"pane":"3","state":"FREE"},
            {"pane":"4","state":"NO_AGENT"}]}"#,
    );
}

/// The port FAILS CLOSED on unreadable input, and now does so LOUDLY. The shell exits 0
/// printing nothing — indistinguishable from a quiet fleet, which is the second half of
/// the same defect.
#[test]
fn the_port_types_unreadable_input_where_the_shell_reports_silence() {
    let status = oracle_status();
    let OracleStatus::Ready = status else {
        announce_skip("the_port_types_unreadable_input_where_the_shell_reports_silence", &status);
        return;
    };
    assert!(port_select("not json", "not json").is_empty());
    let refusal = refill_idle_panes::measurability_refusal(
        &refill_idle_panes::measurability_verdict("not json", "not json"),
    )
    .expect("the port must TYPE the unreadable case");
    assert_eq!(refusal.code, 2, "and must exit NONZERO on it");
    let Some(shell) = shell_select("not json", "not json") else {
        println!(
            "DIFFERENTIAL DID NOT RUN: test=the_port_types_unreadable_input_where_the_shell_reports_silence \
             reason=oracle_execution_failed\n  0 cases compared. This is NOT a passing differential."
        );
        return;
    };
    assert!(
        shell.is_empty(),
        "the shell selects nothing here too — but silently, at exit 0"
    );
}

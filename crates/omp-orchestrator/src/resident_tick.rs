//! Per-interval reaction to a supervisor decision.
//!
//! A gate refusal is a VERDICT ABOUT THIS INTERVAL, not a reason to stop being
//! a resident. `SURVIVE_GATE_UNWIRED` is the mutation target: flip it to `false`
//! and the known-bad capture grows multiple pids / tick reset / restarts.

use crate::SupervisorDecision;
use std::collections::BTreeSet;

/// Load-bearing: a GATE_UNWIRED tick must not exit the resident.
/// Mutation-goes-RED flips this to `false`.
pub const SURVIVE_GATE_UNWIRED: bool = true;

/// Exact measured 2026-09-05 uds-loop unwired payload. Must stay byte-identical.
pub const MEASURED_UNWIRED: &str =
    "POSITIVE_CONTROL_FAILED: no-shell-gate must be reachable";

/// k0i6 empty-census token. Owned by uds-k0i6; this crate must not weaken it.
pub const K0I6_POSITIVE_CONTROL_FAILED: &str =
    "POSITIVE_CONTROL_FAILED: no independently reachable gate in this census";

pub const REFUSAL_TOKENS: &[&str] = &["SUPERVISOR_REFUSED", "GATE_UNWIRED"];
pub const FATAL_TOKEN: &str = "SUPERVISOR_FATAL";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FatalReason {
    NoRuntimeContext,
    RuntimeBuild,
    IdentityAmbient,
}

impl FatalReason {
    #[must_use]
    pub fn line(&self) -> String {
        let reason = match self {
            Self::NoRuntimeContext => "no_runtime_context",
            Self::RuntimeBuild => "runtime_build",
            Self::IdentityAmbient => "identity_ambient",
        };
        format!("{FATAL_TOKEN} {reason}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TickOutcome {
    NoDispatch {
        line: String,
        skip_reap: bool,
    },
    Dispatch,
    Fatal {
        line: String,
    },
}

#[must_use]
pub fn gate_unwired_line(unwired: &[String]) -> String {
    format!(
        "SUPERVISOR_REFUSED GATE_UNWIRED unwired={}",
        unwired.join(" ")
    )
}

#[must_use]
pub fn outcome_for(decision: &SupervisorDecision, survive: bool) -> TickOutcome {
    match decision {
        SupervisorDecision::GateUnwired { unwired } => {
            let line = gate_unwired_line(unwired);
            if survive {
                TickOutcome::NoDispatch {
                    line,
                    skip_reap: true,
                }
            } else {
                TickOutcome::Fatal { line }
            }
        }
        SupervisorDecision::Dispatch { .. } => TickOutcome::Dispatch,
        SupervisorDecision::EscalateIdleIncident { .. }
        | SupervisorDecision::AuthorizedIdle { .. }
        | SupervisorDecision::SupervisedWorking { .. }
        | SupervisorDecision::AwaitingHuman { .. }
        | SupervisorDecision::QueueEmptyNeedsJosh { .. }
        | SupervisorDecision::MonitorBlind { .. }
        | SupervisorDecision::QueueUnreadable { .. }
        | SupervisorDecision::WorkspaceUnloaded { .. } => TickOutcome::NoDispatch {
            line: format!("SUPERVISOR_REFUSED {}", decision_tag(decision)),
            skip_reap: true,
        },
    }
}

fn decision_tag(decision: &SupervisorDecision) -> &'static str {
    match decision {
        SupervisorDecision::GateUnwired { .. } => "GATE_UNWIRED",
        SupervisorDecision::Dispatch { .. } => "DISPATCH",
        SupervisorDecision::EscalateIdleIncident { .. } => "IDLE_UNAUTHORIZED",
        SupervisorDecision::AuthorizedIdle { .. } => "IDLE_AUTHORIZED",
        SupervisorDecision::SupervisedWorking { .. } => "SUPERVISED_WORKING",
        SupervisorDecision::AwaitingHuman { .. } => "AWAITING_HUMAN",
        SupervisorDecision::QueueEmptyNeedsJosh { .. } => "QUEUE_EMPTY_NEEDS_JOSH",
        SupervisorDecision::MonitorBlind { .. } => "MONITOR_BLIND",
        SupervisorDecision::QueueUnreadable { .. } => "QUEUE_UNREADABLE",
        SupervisorDecision::WorkspaceUnloaded { .. } => "WORKSPACE_UNLOADED",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntervalRow {
    pub pid: u32,
    pub tick: u64,
    pub line: String,
    pub dispatched: bool,
    pub skipped_reap: bool,
    pub exited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    pub rows: Vec<IntervalRow>,
    pub assertion_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarnessError {
    EmptyCapture,
    NoIntervalsObserved,
}

impl std::fmt::Display for HarnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyCapture => write!(f, "EMPTY_CAPTURE"),
            Self::NoIntervalsObserved => write!(f, "NO_INTERVALS_OBSERVED"),
        }
    }
}

/// Drive N decisions as one resident. Fatal outcomes spawn a new pid and reset tick
/// (the crash-loop shape under `restart=on-failure`).
pub fn drive_resident(decisions: &[SupervisorDecision], survive: bool, pid0: u32) -> Capture {
    let mut rows = Vec::new();
    let mut pid = pid0;
    let mut tick = 0u64;
    for decision in decisions {
        match outcome_for(decision, survive) {
            TickOutcome::NoDispatch { line, skip_reap } => {
                tick += 1;
                rows.push(IntervalRow {
                    pid,
                    tick,
                    line,
                    dispatched: false,
                    skipped_reap: skip_reap,
                    exited: false,
                });
            }
            TickOutcome::Dispatch => {
                tick += 1;
                rows.push(IntervalRow {
                    pid,
                    tick,
                    line: "DISPATCH".to_owned(),
                    dispatched: true,
                    skipped_reap: false,
                    exited: false,
                });
            }
            TickOutcome::Fatal { line } => {
                rows.push(IntervalRow {
                    pid,
                    tick: 1,
                    line,
                    dispatched: false,
                    skipped_reap: false,
                    exited: true,
                });
                pid = pid.wrapping_add(1);
                tick = 0;
            }
        }
    }
    Capture {
        rows,
        assertion_count: 0,
    }
}

pub fn grade_known_bad(capture: &mut Capture, n: usize, expected_line: &str) -> Result<(), String> {
    if capture.rows.is_empty() {
        return Err(HarnessError::EmptyCapture.to_string());
    }
    let refusals: Vec<&IntervalRow> = capture
        .rows
        .iter()
        .filter(|row| row.line.contains("GATE_UNWIRED"))
        .collect();
    if refusals.is_empty() {
        return Err(HarnessError::NoIntervalsObserved.to_string());
    }
    capture.assertion_count += 1;
    if refusals.len() < n {
        return Err(format!(
            "NO_INTERVALS_OBSERVED want>={n} got={}",
            refusals.len()
        ));
    }
    capture.assertion_count += 1;
    let pid = refusals[0].pid;
    if refusals.iter().any(|row| row.pid != pid) {
        return Err(format!(
            "PID_UNSTABLE pids={:?}",
            refusals.iter().map(|r| r.pid).collect::<Vec<_>>()
        ));
    }
    capture.assertion_count += 1;
    if refusals.iter().any(|row| row.exited) {
        return Err("EXIT_OBSERVED".to_owned());
    }
    capture.assertion_count += 1;
    let ticks: Vec<u64> = refusals.iter().map(|r| r.tick).collect();
    if ticks[0] != 1 || ticks.windows(2).any(|w| w[1] <= w[0]) {
        return Err(format!("TICK_RESET_OR_NON_MONOTONE ticks={ticks:?}"));
    }
    capture.assertion_count += 1;
    if refusals.iter().skip(1).any(|row| row.tick == 1) {
        return Err("TICK_RESET_TO_1".to_owned());
    }

    capture.assertion_count += 1;
    if refusals.iter().any(|row| !row.skipped_reap) {
        return Err("REAP_NOT_SKIPPED".to_owned());
    }
    capture.assertion_count += 1;
    if refusals.iter().any(|row| row.line != expected_line) {
        return Err(format!(
            "REFUSAL_TEXT_DRIFT got={:?} want={expected_line}",
            refusals.iter().map(|r| r.line.as_str()).collect::<Vec<_>>()
        ));
    }
    capture.assertion_count += 1;
    if capture.assertion_count < n {
        return Err(format!(
            "ASSERTION_COUNT_BELOW_N count={} n={n}",
            capture.assertion_count
        ));
    }
    Ok(())
}

pub fn grade_known_good(capture: &mut Capture, n: usize) -> Result<(), String> {
    if capture.rows.is_empty() {
        return Err(HarnessError::EmptyCapture.to_string());
    }
    capture.assertion_count += 1;
    let dispatches = capture.rows.iter().filter(|r| r.dispatched).count();
    let refusals = capture
        .rows
        .iter()
        .filter(|r| r.line.contains("GATE_UNWIRED"))
        .count();
    if dispatches != n {
        return Err(format!("DISPATCH_COUNT want={n} got={dispatches}"));
    }
    capture.assertion_count += 1;
    if refusals != 0 {
        return Err(format!("REFUSAL_COUNT want=0 got={refusals}"));
    }
    capture.assertion_count += 1;
    let pid = capture.rows[0].pid;
    if capture.rows.iter().any(|r| r.pid != pid || r.exited) {
        return Err("PID_UNSTABLE_OR_EXITED".to_owned());
    }
    capture.assertion_count += 1;
    Ok(())
}

pub fn grade_mixed(capture: &mut Capture) -> Result<(), String> {
    if capture.rows.len() < 3 {
        return Err(HarnessError::NoIntervalsObserved.to_string());
    }
    capture.assertion_count += 1;
    let pid = capture.rows[0].pid;
    if capture.rows.iter().any(|r| r.pid != pid) {
        return Err("MIXED_PID_UNSTABLE".to_owned());
    }
    capture.assertion_count += 1;
    if !capture.rows[0].line.contains("GATE_UNWIRED") || !capture.rows[1].line.contains("GATE_UNWIRED")
    {
        return Err("MIXED_PREFIX_NOT_REFUSAL".to_owned());
    }
    capture.assertion_count += 1;
    if !capture.rows[2].dispatched {
        return Err("MIXED_TICK3_DID_NOT_DISPATCH".to_owned());
    }
    capture.assertion_count += 1;
    Ok(())
}

#[must_use]
pub fn refusal_token_set() -> BTreeSet<&'static str> {
    REFUSAL_TOKENS.iter().copied().collect()
}

#[must_use]
pub fn fatal_token_set() -> BTreeSet<&'static str> {
    let mut set = BTreeSet::new();
    set.insert(FATAL_TOKEN);
    set
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unwired() -> SupervisorDecision {
        SupervisorDecision::GateUnwired {
            unwired: vec![MEASURED_UNWIRED.to_owned()],
        }
    }

    fn dispatch() -> SupervisorDecision {
        SupervisorDecision::Dispatch {
            pane: "%9".into(),
            bead_hint: "omp-orchestrator-8kel".into(),
        }
    }

    #[test]
    fn fires_on_known_bad() {
        let expected = gate_unwired_line(&[MEASURED_UNWIRED.to_owned()]);
        assert_eq!(
            expected,
            format!("SUPERVISOR_REFUSED GATE_UNWIRED unwired={MEASURED_UNWIRED}")
        );
        let mut cap = drive_resident(&[unwired(), unwired(), unwired()], true, 77234);
        grade_known_bad(&mut cap, 3, &expected).expect("known-bad");
        assert!(cap.assertion_count >= 3);
        assert!(cap.rows.iter().all(|r| r.pid == 77234 && !r.exited && r.skipped_reap));
        assert_eq!(
            cap.rows.iter().map(|r| r.tick).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn passes_known_good() {
        let mut cap = drive_resident(&[dispatch(), dispatch(), dispatch()], true, 10);
        grade_known_good(&mut cap, 3).expect("known-good");
    }

    #[test]
    fn mixed_refuse_then_clear_same_pid() {
        let mut cap = drive_resident(&[unwired(), unwired(), dispatch()], true, 42);
        grade_mixed(&mut cap).expect("mixed");
        assert_eq!(cap.rows[0].pid, cap.rows[2].pid);
        assert!(cap.rows[2].dispatched);
    }

    #[test]
    fn empty_scan_is_error() {
        let mut cap = Capture {
            rows: vec![],
            assertion_count: 0,
        };
        let err = grade_known_bad(&mut cap, 3, "x").expect_err("empty");
        assert!(err.contains("EMPTY_CAPTURE"), "{err}");
    }

    #[test]
    fn no_intervals_is_error() {
        let mut cap = drive_resident(&[dispatch()], true, 1);
        let err = grade_known_bad(&mut cap, 3, "x").expect_err("no refusals");
        assert!(err.contains("NO_INTERVALS_OBSERVED"), "{err}");
    }

    #[test]
    fn mutation_goes_red() {
        let mut cap = drive_resident(&[unwired(), unwired(), unwired()], false, 100);
        let err = grade_known_bad(&mut cap, 3, &gate_unwired_line(&[MEASURED_UNWIRED.to_owned()]))
            .expect_err("survive=false must RED");
        assert!(
            err.contains("PID_UNSTABLE")
                || err.contains("TICK_RESET")
                || err.contains("EXIT_OBSERVED")
                || err.contains("NO_INTERVALS"),
            "{err}"
        );
        assert!(cap.rows.iter().any(|r| r.exited));
        assert!(cap.rows.iter().map(|r| r.pid).collect::<BTreeSet<_>>().len() > 1);
        assert!(cap.rows.iter().all(|r| r.tick == 1));
    }

    #[test]
    fn exit_reserved_tokens_are_disjoint() {
        let refusal = refusal_token_set();
        let fatal = fatal_token_set();
        assert!(
            refusal.is_disjoint(&fatal),
            "overlap={:?}",
            refusal.intersection(&fatal).collect::<Vec<_>>()
        );
        for reason in [
            FatalReason::NoRuntimeContext,
            FatalReason::RuntimeBuild,
            FatalReason::IdentityAmbient,
        ] {
            let line = reason.line();
            assert!(line.starts_with(FATAL_TOKEN), "{line}");
            assert!(!line.contains("SUPERVISOR_REFUSED"), "{line}");
            assert!(!line.contains("GATE_UNWIRED"), "{line}");
        }
        let measured = gate_unwired_line(&[MEASURED_UNWIRED.to_owned()]);
        assert!(measured.contains("SUPERVISOR_REFUSED"));
        assert!(measured.contains("GATE_UNWIRED"));
        assert!(!measured.contains(FATAL_TOKEN));
    }

    #[test]
    fn k0i6_canary_text_is_unchanged() {
        assert_eq!(
            K0I6_POSITIVE_CONTROL_FAILED,
            "POSITIVE_CONTROL_FAILED: no independently reachable gate in this census"
        );
        assert_eq!(
            crate::POSITIVE_CONTROL_FAILED_UNWIRED,
            K0I6_POSITIVE_CONTROL_FAILED
        );
    }

    #[test]
    fn survive_const_is_true() {
        assert!(SURVIVE_GATE_UNWIRED);
    }

    #[test]
    fn mutation_file_restore_is_byte_identical() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/resident_tick.rs");
        let original = std::fs::read(&path).expect("read");
        let pre = shasum(&path);
        let text = String::from_utf8(original.clone()).expect("utf8");
        let mutated = text.replace(
            "pub const SURVIVE_GATE_UNWIRED: bool = true;",
            "pub const SURVIVE_GATE_UNWIRED: bool = false;",
        );
        assert_ne!(text, mutated, "mutation must change the survive const");
        std::fs::write(&path, mutated.as_bytes()).expect("mutate");
        let mid = shasum(&path);
        assert_ne!(pre, mid, "mutated sha must differ");
        std::fs::write(&path, &original).expect("restore");
        let post = shasum(&path);
        assert_eq!(pre, post, "restored checksum must match pre-mutation");
        eprintln!("MUTATION_SHA256 pre={pre} mutated={mid} restored={post}");
    }

    fn shasum(path: &std::path::Path) -> String {
        let out = std::process::Command::new("shasum")
            .args(["-a", "256"])
            .arg(path)
            .output()
            .expect("shasum");
        assert!(out.status.success());
        String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .next()
            .expect("hash")
            .to_owned()
    }
}


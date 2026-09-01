#![forbid(unsafe_code)]

//! Fast-dispatch admission and pane selection, ported from `bin/fast-dispatch.sh`.
//!
//! The shell file is the differential oracle and is not edited by this crate.
//! check.sh and loop-queue-filter are EXTERNAL commands with stable CLI contracts;
//! this crate does not reimplement them.
//!
//! ADMISSION CONTRACT (do not weaken):
//!   admit ONLY on a FRESH standing PASS. Stale PASS refuses. Non-PASS refuses.
//!   Bound to completed_ts (UTC), never file mtime. Absent/unparseable/future refuse.
//!   Subject identity: a stamped verdict for a different tree refuses; an unstamped
//!   verdict is admitted only inside the tighter legacy window.

use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_FRESH_SECONDS: f64 = 1500.0;
pub const DEFAULT_LEGACY_FRESH_SECONDS: f64 = 300.0;
pub const ALLOWED_PANE_STATES: &[&str] =
    &["FREE", "BUSY", "QUOTA_BLOCKED", "NO_AGENT", "UNREADABLE"];

/// fh C75: one enum is the authority for mutation-rule names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FastDispatchRule {
    OverallMustBePass,
    FreshnessWindow,
    FreeStateOnly,
}

impl FastDispatchRule {
    pub const ALL: &'static [FastDispatchRule] = &[
        FastDispatchRule::OverallMustBePass,
        FastDispatchRule::FreshnessWindow,
        FastDispatchRule::FreeStateOnly,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            FastDispatchRule::OverallMustBePass => "overall_must_be_pass",
            FastDispatchRule::FreshnessWindow => "freshness_window",
            FastDispatchRule::FreeStateOnly => "free_state_only",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|r| r.as_str() == name)
    }
}

#[derive(Clone, Debug)]
pub struct FastDispatchRules {
    pub overall_must_be_pass: bool,
    pub freshness_window: bool,
    pub free_state_only: bool,
}

impl Default for FastDispatchRules {
    fn default() -> Self {
        Self {
            overall_must_be_pass: true,
            freshness_window: true,
            free_state_only: true,
        }
    }
}

impl FastDispatchRules {
    pub fn all_enabled(&self) -> bool {
        self.overall_must_be_pass && self.freshness_window && self.free_state_only
    }

    pub fn disable(&mut self, name: &str) -> bool {
        let Some(rule) = FastDispatchRule::parse(name) else {
            return false;
        };
        match rule {
            FastDispatchRule::OverallMustBePass => self.overall_must_be_pass = false,
            FastDispatchRule::FreshnessWindow => self.freshness_window = false,
            FastDispatchRule::FreeStateOnly => self.free_state_only = false,
        }
        true
    }

    pub fn known_names_csv() -> String {
        FastDispatchRule::ALL
            .iter()
            .map(|r| r.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Clone, Debug)]
pub struct AdmissionConfig {
    pub fresh_seconds: f64,
    pub legacy_fresh_seconds: f64,
    pub now: f64,
    pub subject_id: String,
    pub rules: FastDispatchRules,
}

impl AdmissionConfig {
    pub fn from_env() -> Self {
        Self {
            fresh_seconds: env_f64("ADMISSION_FRESH_SECONDS", DEFAULT_FRESH_SECONDS),
            legacy_fresh_seconds: env_f64(
                "ADMISSION_LEGACY_FRESH_SECONDS",
                DEFAULT_LEGACY_FRESH_SECONDS,
            ),
            now: now_secs(),
            subject_id: std::env::var("ADMISSION_SUBJECT_ID").unwrap_or_default(),
            rules: FastDispatchRules::default(),
        }
    }
}

fn env_f64(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

pub fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Match CPython `datetime.strptime(ts, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)`.
/// This is UTC, unlike verify-dispatch's oracle which uses local mktime.
pub fn parse_completed_ts_utc(ts: &str) -> Option<f64> {
    let naive = chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%SZ").ok()?;
    Some(naive.and_utc().timestamp() as f64)
}

/// Admit iff the standing verdict is a fresh PASS for this tree.
/// Returns true to admit (shell rc=0), false to refuse (shell rc=1).
pub fn admission_fresh_pass(ledger_path: &Path, cfg: &AdmissionConfig) -> bool {
    let text = match std::fs::read_to_string(ledger_path) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let data: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return false,
    };
    if cfg.rules.overall_must_be_pass && data.get("overall").and_then(|v| v.as_str()) != Some("PASS")
    {
        return false;
    }
    let ts = match data.get("completed_ts").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s,
        _ => return false,
    };
    let when = match parse_completed_ts_utc(ts) {
        Some(t) => t,
        None => return false,
    };
    let age = cfg.now - when;
    if cfg.rules.freshness_window && !(0.0 <= age && age <= cfg.fresh_seconds) {
        return false;
    }

    let stamped = data
        .get("subject_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let current = cfg.subject_id.as_str();
    if !stamped.is_empty() && !current.is_empty() && stamped != current {
        return false;
    }
    if stamped.is_empty() {
        return age <= cfg.legacy_fresh_seconds;
    }
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectError {
    Invalid,
}

/// Only `state == "FREE"` is selected. `safe_to_dispatch` is ignored (the tempting lie).
/// Schema drift or free_count disagreement is Invalid (shell exit 2), not "no panes".
pub fn select_free_panes(json: &str, rules: &FastDispatchRules) -> Result<Vec<String>, SelectError> {
    let data: Value = serde_json::from_str(json).map_err(|_| SelectError::Invalid)?;
    if data.get("schema").and_then(|v| v.as_str()) != Some("zs.dispatch-ready.v1") {
        return Err(SelectError::Invalid);
    }
    let panes = data
        .get("panes")
        .and_then(|v| v.as_array())
        .ok_or(SelectError::Invalid)?;
    let declared_free = data
        .get("free_count")
        .and_then(|v| v.as_i64())
        .ok_or(SelectError::Invalid)?;
    let mut free = Vec::new();
    let allowed: BTreeSet<&str> = ALLOWED_PANE_STATES.iter().copied().collect();
    for row in panes {
        let obj = row.as_object().ok_or(SelectError::Invalid)?;
        let pane = obj
            .get("pane")
            .and_then(|v| v.as_str())
            .ok_or(SelectError::Invalid)?;
        let state = obj
            .get("state")
            .and_then(|v| v.as_str())
            .ok_or(SelectError::Invalid)?;
        if !allowed.contains(state) {
            return Err(SelectError::Invalid);
        }
        let is_free = if rules.free_state_only {
            state == "FREE"
        } else {
            true
        };
        if is_free {
            free.push(pane.to_string());
        }
    }
    if declared_free != free.len() as i64 && rules.free_state_only {
        return Err(SelectError::Invalid);
    }
    Ok(free)
}

pub fn is_conductor_routed(session: &str, list: &str) -> bool {
    let padded = format!(" {list} ");
    padded.contains(&format!(" {session} "))
}

pub fn session_repo_dir(session: &str, home: &Path) -> PathBuf {
    match session {
        "clutterfreespaces" => home.join("Developer/clutterfreespaces.ios"),
        other => home.join("Developer").join(other),
    }
}

pub const CORPUS_FIRST_CONTRACT: &str = "\
CORPUS-FIRST (mandatory before authoring any new check):\n\
  Query the measured corpus first: fh suggest \"<one-sentence mission>\"\n\
  Report exactly one outcome in the handoff:\n\
    CORPUS: CITED <row-id> — <path:line> — \"<deciding quote>\"\n\
    CORPUS: NEW — fh returned no relevant row; propose the new doctrine row\n\
  A null result is not a clean pass: preserve the query and outcome so the next wave can re-derive it.\n";

/// Exact crontab parent, matching bin/fast-dispatch.sh classify_invoker (fh C54).
/// Cron on this box wraps the job as `/bin/sh -c <crontab command>`. Both expected
/// shapes derive from the RESOLVED repository root and home (omp-orchestrator-npq):
/// the matcher keeps its semantics on any machine without carrying a literal checkout
/// path — a literal would match this box's crontab and silently miss every other.
pub fn classify_invoker(
    parent: &str,
    state_dir: &Path,
    repo_root: &Path,
    home: &Path,
) -> (&'static str, &'static str) {
    let state = state_dir.display();
    let old = format!(
        "/bin/sh -c /bin/bash {}/bin/fast-dispatch.sh >> {}/fast-dispatch.log 2>&1",
        repo_root.display(),
        state
    );
    let new = format!(
        "/bin/sh -c {}/.local/bin/fast-dispatch >> {}/fast-dispatch.log 2>&1",
        home.display(),
        state
    );
    let t = parent.trim();
    if t == old || t == new {
        ("SCHEDULED", "cron_parent")
    } else {
        ("MANUAL", "unproven_parent")
    }
}

/// Load-scaled cargo-lane-budget wrapper bound. Gate's own deadline is the
/// real backstop; this only avoids SIGKILL of a still-running scan (8d4e054).
pub fn cargo_lane_timeout_secs(load: u64, ncpu: u64) -> u64 {
    let ncpu = ncpu.max(1);
    let mut mult = 1 + load / ncpu;
    mult = mult.clamp(1, 6);
    90 * mult
}

pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for d in chars.by_ref() {
                if d.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Wedge/composer liveness, same three refusals as the shell pane_is_live.
/// `composer_occupied` is the result of piping the raw tail to composer-typed.py.
pub fn wedge_reason(tail_plain: &str, composer_occupied: bool) -> Option<&'static str> {
    let footer: Vec<&str> = tail_plain.lines().rev().take(8).collect();
    let footer = footer.into_iter().rev().collect::<Vec<_>>().join("\n");
    if footer
        .split('\n')
        .any(|l| l.contains("Weekly limit left:") && l.contains("0%"))
    {
        return Some("provider_quota_exhausted");
    }
    if tail_plain.contains("Press up to edit queued messages") {
        return Some("queued_unsubmitted");
    }
    if composer_occupied {
        return Some("composer_nonempty");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::{Command, Stdio};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum OracleStatus {
        Ready,
        MissingInterpreter,
    }

    fn oracle_status() -> OracleStatus {
        match Command::new("python3")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
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
         the in-module Rust tests.\n  \
         0 cases compared. This is NOT a passing differential."
        );
    }

    fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "fast-dispatch-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::create_dir_all(&p);
        p.join("ledger.json")
    }

    fn cfg_at(now: f64) -> AdmissionConfig {
        AdmissionConfig {
            fresh_seconds: 1500.0,
            legacy_fresh_seconds: 300.0,
            now,
            subject_id: String::new(),
            rules: FastDispatchRules::default(),
        }
    }

    fn stamp(now: f64, age: f64) -> String {
        chrono::DateTime::<chrono::Utc>::from_timestamp((now - age) as i64, 0)
            .unwrap()
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string()
    }

    fn write_verdict(path: &Path, overall: &str, ts: &str, subject: Option<&str>) {
        let subj = match subject {
            Some(s) => format!(",\"subject_id\":\"{s}\""),
            None => String::new(),
        };
        fs::write(
            path,
            format!("{{\"overall\":\"{overall}\",\"completed_ts\":\"{ts}\"{subj}}}"),
        )
        .unwrap();
    }

    #[test]
    fn fresh_pass_admits() {
        let path = tmp("fresh-pass");
        let now = 1_700_000_000.0;
        write_verdict(&path, "PASS", &stamp(now, 120.0), Some("deadbeef:00"));
        let mut c = cfg_at(now);
        c.subject_id = "deadbeef:00".into();
        assert!(
            admission_fresh_pass(&path, &c),
            "rule overall_must_be_pass: a recent PASS must admit"
        );
    }

    #[test]
    fn fresh_fail_refuses() {
        let path = tmp("fresh-fail");
        let now = 1_700_000_000.0;
        write_verdict(&path, "FAIL", &stamp(now, 120.0), Some("deadbeef:00"));
        assert!(
            !admission_fresh_pass(&path, &cfg_at(now)),
            "rule overall_must_be_pass: a non-PASS verdict must REFUSE"
        );
    }

    #[test]
    fn stale_pass_refuses() {
        let path = tmp("stale-pass");
        let now = 1_700_000_000.0;
        write_verdict(&path, "PASS", &stamp(now, 9000.0), Some("deadbeef:00"));
        assert!(
            !admission_fresh_pass(&path, &cfg_at(now)),
            "rule freshness_window: a STALE PASS must REFUSE"
        );
    }

    #[test]
    fn future_stamp_refuses() {
        let path = tmp("future");
        let now = 1_700_000_000.0;
        write_verdict(&path, "PASS", &stamp(now, -60.0), None);
        assert!(
            !admission_fresh_pass(&path, &cfg_at(now)),
            "rule freshness_window: a future completed_ts must REFUSE"
        );
    }

    #[test]
    fn absent_ledger_refuses() {
        let path = tmp("absent");
        let _ = fs::remove_file(&path);
        assert!(
            !admission_fresh_pass(&path, &cfg_at(1_700_000_000.0)),
            "absent ledger must REFUSE"
        );
    }

    #[test]
    fn corrupt_ledger_refuses() {
        let path = tmp("corrupt");
        fs::write(&path, "not json\n").unwrap();
        assert!(
            !admission_fresh_pass(&path, &cfg_at(1_700_000_000.0)),
            "corrupt ledger must REFUSE"
        );
    }

    #[test]
    fn missing_completed_ts_refuses() {
        let path = tmp("no-ts");
        fs::write(&path, "{\"overall\":\"PASS\"}").unwrap();
        assert!(
            !admission_fresh_pass(&path, &cfg_at(1_700_000_000.0)),
            "verdict without completed_ts must REFUSE"
        );
    }

    #[test]
    fn subject_mismatch_refuses() {
        let path = tmp("subj");
        let now = 1_700_000_000.0;
        write_verdict(&path, "PASS", &stamp(now, 120.0), Some("other:00"));
        let mut c = cfg_at(now);
        c.subject_id = "deadbeef:00".into();
        assert!(
            !admission_fresh_pass(&path, &c),
            "a fresh PASS for a DIFFERENT tree must REFUSE"
        );
    }

    #[test]
    fn unstamped_beyond_legacy_refuses() {
        let path = tmp("legacy");
        let now = 1_700_000_000.0;
        write_verdict(&path, "PASS", &stamp(now, 900.0), None);
        assert!(
            !admission_fresh_pass(&path, &cfg_at(now)),
            "unstamped verdict older than the legacy window must REFUSE"
        );
    }

    #[test]
    fn disabling_overall_must_be_pass_false_admits_fail() {
        let path = tmp("mut-overall");
        let now = 1_700_000_000.0;
        write_verdict(&path, "FAIL", &stamp(now, 120.0), None);
        let mut c = cfg_at(now);
        assert!(c.rules.disable("overall_must_be_pass"));
        assert!(
            admission_fresh_pass(&path, &c),
            "mutation overall_must_be_pass: disabling it must admit a FAIL"
        );
    }

    #[test]
    fn disabling_freshness_window_false_admits_stale_pass() {
        let path = tmp("mut-fresh");
        let now = 1_700_000_000.0;
        write_verdict(&path, "PASS", &stamp(now, 9000.0), Some("deadbeef:00"));
        let mut c = cfg_at(now);
        c.subject_id = "deadbeef:00".into();
        assert!(c.rules.disable("freshness_window"));
        assert!(
            admission_fresh_pass(&path, &c),
            "mutation freshness_window: disabling it must admit a STALE PASS"
        );
    }

    #[test]
    fn busy_pane_is_not_selected() {
        let json = r#"{"schema":"zs.dispatch-ready.v1","panes":[{"pane":"2","state":"BUSY","safe_to_dispatch":false}],"free_count":0}"#;
        let got = select_free_panes(json, &FastDispatchRules::default()).expect("parse");
        assert!(
            got.is_empty(),
            "rule free_state_only: a BUSY pane must never be selected, got {got:?}"
        );
    }

    #[test]
    fn free_pane_is_selected() {
        let json = r#"{"schema":"zs.dispatch-ready.v1","panes":[{"pane":"2","state":"FREE","safe_to_dispatch":false}],"free_count":1}"#;
        let got = select_free_panes(json, &FastDispatchRules::default()).expect("parse");
        assert_eq!(
            got,
            vec!["2".to_string()],
            "anti-vacuous: a genuinely FREE pane IS selected"
        );
    }

    #[test]
    fn quota_blocked_with_safe_to_dispatch_is_not_selected() {
        let json = r#"{"schema":"zs.dispatch-ready.v1","panes":[{"pane":"1","state":"QUOTA_BLOCKED","safe_to_dispatch":true},{"pane":"3","state":"BUSY","safe_to_dispatch":false}],"free_count":0}"#;
        let got = select_free_panes(json, &FastDispatchRules::default()).expect("parse");
        assert!(
            got.is_empty(),
            "rule free_state_only: QUOTA_BLOCKED with safe_to_dispatch=true must not be selected, got {got:?}"
        );
    }

    #[test]
    fn schema_drift_is_invalid_not_empty() {
        let json = r#"{"schema":"nope","panes":[],"free_count":0}"#;
        assert_eq!(
            select_free_panes(json, &FastDispatchRules::default()),
            Err(SelectError::Invalid)
        );
    }

    #[test]
    fn free_count_disagreement_is_invalid() {
        let json = r#"{"schema":"zs.dispatch-ready.v1","panes":[{"pane":"2","state":"FREE","safe_to_dispatch":false}],"free_count":0}"#;
        assert_eq!(
            select_free_panes(json, &FastDispatchRules::default()),
            Err(SelectError::Invalid)
        );
    }

    #[test]
    fn disabling_free_state_only_selects_busy() {
        let json = r#"{"schema":"zs.dispatch-ready.v1","panes":[{"pane":"2","state":"BUSY","safe_to_dispatch":false}],"free_count":0}"#;
        let mut r = FastDispatchRules::default();
        assert!(r.disable("free_state_only"));
        let got = select_free_panes(json, &r).expect("parse");
        assert_eq!(
            got,
            vec!["2".to_string()],
            "mutation free_state_only: disabling it must select a BUSY pane"
        );
    }

    #[test]
    fn conductor_routed_is_word_bounded() {
        assert!(is_conductor_routed("clutterfreespaces", "clutterfreespaces"));
        assert!(!is_conductor_routed(
            "clutterfreespaces-ios",
            "clutterfreespaces"
        ));
    }

    #[test]
    fn session_repo_override_is_exact() {
        // Home comes from the environment, so the mapping assertion holds on any
        // machine (omp-orchestrator-npq) — the function under test takes home as a
        // parameter, and so does its test.
        let home = PathBuf::from(std::env::var_os("HOME").filter(|v| !v.is_empty()).unwrap_or_default());
        let developer = home.join("Developer");
        assert_eq!(
            session_repo_dir("clutterfreespaces", &home),
            developer.join("clutterfreespaces.ios")
        );
        assert_eq!(
            session_repo_dir("control-plane", &home),
            developer.join("control-plane")
        );
        assert_eq!(
            session_repo_dir("clutterfreespaces-ios", &home),
            developer.join("clutterfreespaces-ios")
        );
    }

    #[test]
    fn every_named_rule_is_disableable() {
        assert!(
            !FastDispatchRule::ALL.is_empty(),
            "C75: an empty FastDispatchRule::ALL is not a clean bill"
        );
        for rule in FastDispatchRule::ALL {
            let mut g = FastDispatchRules::default();
            assert!(g.all_enabled());
            assert!(
                g.disable(rule.as_str()),
                "C75: FastDispatchRule::ALL entry {} is not disableable",
                rule.as_str()
            );
            assert!(
                !g.all_enabled(),
                "C75: disabling {} did not change the struct",
                rule.as_str()
            );
        }
    }

    #[test]
    fn utc_parse_matches_python_timezone_utc() {
        let status = oracle_status();
        let OracleStatus::Ready = status else {
            announce_skip("utc_parse_matches_python_timezone_utc", &status);
            return;
        };
        let ts = "2026-08-26T16:00:00Z";
        let rust = parse_completed_ts_utc(ts).expect("parse");
        let py = std::process::Command::new("python3")
            .args([
                "-c",
                "import datetime,sys; t=datetime.datetime.strptime(sys.argv[1],'%Y-%m-%dT%H:%M:%SZ').replace(tzinfo=datetime.timezone.utc); print(t.timestamp())",
                ts,
            ])
            .output()
            .expect("python");
        let py: f64 = String::from_utf8_lossy(&py.stdout)
            .trim()
            .parse()
            .expect("float");
        assert!(
            (rust - py).abs() < 1.0,
            "admission timestamps must be UTC, rust {rust} vs python {py}"
        );
    }

    #[test]
    fn classify_invoker_exact_cron_parent_only() {
        // All inputs derive from the environment (omp-orchestrator-npq): the test
        // asserts the matcher recognizes the cron shapes for THIS machine's home and
        // the discovered repo, exactly as production derives them.
        let home = PathBuf::from(std::env::var_os("HOME").filter(|v| !v.is_empty()).unwrap_or_default());
        let state = home.join(".local/state/flywheel");
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("crate lives two levels below the repository root");
        let old = format!(
            "/bin/sh -c /bin/bash {}/bin/fast-dispatch.sh >> {}/fast-dispatch.log 2>&1",
            repo_root.display(),
            state.display()
        );
        let new = format!(
            "/bin/sh -c {}/.local/bin/fast-dispatch >> {}/fast-dispatch.log 2>&1",
            home.display(),
            state.display()
        );
        assert_eq!(
            classify_invoker(&old, &state, repo_root, &home),
            ("SCHEDULED", "cron_parent")
        );
        assert_eq!(
            classify_invoker(&new, &state, repo_root, &home),
            ("SCHEDULED", "cron_parent")
        );
        assert_eq!(
            classify_invoker("bash", &state, repo_root, &home),
            ("MANUAL", "unproven_parent"),
            "a detached non-TTY is not scheduler proof"
        );
    }

    #[test]
    fn wedge_quota_and_queued_refuse() {
        assert_eq!(
            wedge_reason("Weekly limit left: 0%\nready", false),
            Some("provider_quota_exhausted")
        );
        assert_eq!(
            wedge_reason("Press up to edit queued messages\n", false),
            Some("queued_unsubmitted")
        );
        assert_eq!(wedge_reason("❯ ", true), Some("composer_nonempty"));
        assert_eq!(wedge_reason("❯ waiting for input", false), None);
    }

    #[test]
    fn cargo_lane_timeout_scales_and_caps() {
        assert_eq!(cargo_lane_timeout_secs(0, 8), 90);
        assert_eq!(cargo_lane_timeout_secs(16, 8), 270);
        assert_eq!(cargo_lane_timeout_secs(200, 8), 540);
    }
}

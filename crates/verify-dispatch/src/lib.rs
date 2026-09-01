#![forbid(unsafe_code)]

//! Ground-truth dispatch verification, ported from `bin/verify-dispatch.py`.
//!
//! The Python file is the differential oracle and is not edited by this crate.
//! Verification uses bead status from `br show` only — never a pane label, idle
//! flag, or agent self-report. Exit 0 always on the live path: this reports, it
//! does not gate (matching the oracle).
//!
//! TIMESTAMP CONTRACT. The oracle does
//! `time.mktime(time.strptime(ts, "%Y-%m-%dT%H:%M:%SZ"))`, which treats the `Z`
//! as a literal and interprets the naive wall time in the *local* zone. A
//! "correct" UTC parse would disagree with the oracle near the window edge.
//! This port matches the oracle, it does not "fix" it.

use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEFAULT_LEDGER: &str = ".local/state/flywheel/controller-tick.jsonl";
const DEFAULT_OUT: &str = ".local/state/flywheel/dispatch-verify.jsonl";
const DEFAULT_WINDOW_H: f64 = 6.0;

const BR_TIMEOUT: Duration = Duration::from_secs(90);

/// fh C75: ONE enum is the authority for mutation-rule names. A parallel
/// `&'static [&str]` that must be kept in sync with `disable()` is the
/// ROOT_SUBCOMMANDS failure — a name missing from the mirror is silently
/// reinterpreted (here: a mutation harness that cannot disable the rule it
/// thinks it is disabling). `VerifyDispatchRule::ALL` is the set; `as_str`/`parse` are
/// derived from it; `disable` exhaustively matches the enum so a new
/// variant that is not wired is a compile error, not a vacuous pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerifyDispatchRule {
    NamedBeadsAllClosed,
    OnlyClosedStatusCounts,
    LegacyIdlessReported,
    WindowCutoff,
    EventIsDispatched,
}

impl VerifyDispatchRule {
    pub const ALL: &'static [VerifyDispatchRule] = &[
        VerifyDispatchRule::NamedBeadsAllClosed,
        VerifyDispatchRule::OnlyClosedStatusCounts,
        VerifyDispatchRule::LegacyIdlessReported,
        VerifyDispatchRule::WindowCutoff,
        VerifyDispatchRule::EventIsDispatched,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            VerifyDispatchRule::NamedBeadsAllClosed => "named_beads_all_closed",
            VerifyDispatchRule::OnlyClosedStatusCounts => "only_closed_status_counts",
            VerifyDispatchRule::LegacyIdlessReported => "legacy_idless_reported",
            VerifyDispatchRule::WindowCutoff => "window_cutoff",
            VerifyDispatchRule::EventIsDispatched => "event_is_dispatched",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|r| r.as_str() == name)
    }
}

#[derive(Clone, Debug)]
pub struct VerifyDispatchRules {
    pub named_beads_all_closed: bool,
    pub only_closed_status_counts: bool,
    pub legacy_idless_reported: bool,
    pub window_cutoff: bool,
    pub event_is_dispatched: bool,
}

impl Default for VerifyDispatchRules {
    fn default() -> Self {
        Self {
            named_beads_all_closed: true,
            only_closed_status_counts: true,
            legacy_idless_reported: true,
            window_cutoff: true,
            event_is_dispatched: true,
        }
    }
}

impl VerifyDispatchRules {
    pub fn known_names_csv() -> String {
        VerifyDispatchRule::ALL
            .iter()
            .map(|r| r.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn all_enabled(&self) -> bool {
        self.named_beads_all_closed
            && self.only_closed_status_counts
            && self.legacy_idless_reported
            && self.window_cutoff
            && self.event_is_dispatched
    }

    pub fn disable(&mut self, name: &str) -> bool {
        let Some(rule) = VerifyDispatchRule::parse(name) else {
            return false;
        };
        match rule {
            VerifyDispatchRule::NamedBeadsAllClosed => self.named_beads_all_closed = false,
            VerifyDispatchRule::OnlyClosedStatusCounts => self.only_closed_status_counts = false,
            VerifyDispatchRule::LegacyIdlessReported => self.legacy_idless_reported = false,
            VerifyDispatchRule::WindowCutoff => self.window_cutoff = false,
            VerifyDispatchRule::EventIsDispatched => self.event_is_dispatched = false,
        }
        true
    }
}

#[derive(Clone, Debug)]
pub struct VerifyDispatchConfig {
    pub ledger: PathBuf,
    pub out: PathBuf,
    pub window_h: f64,
    pub developer_root: PathBuf,
    pub now: f64,
    pub rules: VerifyDispatchRules,
}

impl VerifyDispatchConfig {
    pub fn from_env() -> Result<Self, String> {
        let home = std::env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| {
                "HOME is unset; cannot resolve default ledger/out paths; set VERIFY_LEDGER and VERIFY_OUT".to_owned()
            })?;
        let ledger = std::env::var("VERIFY_LEDGER")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(DEFAULT_LEDGER));
        let out = std::env::var("VERIFY_OUT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(DEFAULT_OUT));
        let window_h = match std::env::var("VERIFY_WINDOW_H") {
            Ok(v) => v
                .parse::<f64>()
                .map_err(|e| format!("usage error: VERIFY_WINDOW_H is not a float: {e}"))?,
            Err(_) => DEFAULT_WINDOW_H,
        };
        Ok(Self {
            ledger,
            out,
            window_h,
            developer_root: std::env::var_os("VERIFY_DEVELOPER_ROOT")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join("Developer")),
            now: now_secs(),
            rules: VerifyDispatchRules::default(),
        })
    }
}

#[derive(Debug)]
pub struct VerifyDispatchRunOutput {
    pub stdout: String,
    pub code: i32,
}

#[derive(Serialize)]
struct OutRow {
    ts: String,
    event: String,
    repo: String,
    dispatches: usize,
    beads_closed: usize,
    beads_total: usize,
    detector: String,
}

pub fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Match CPython `time.mktime(time.strptime(ts, "%Y-%m-%dT%H:%M:%SZ"))`.
/// `Z` is a literal suffix; the naive tuple is local wall time, not UTC.
pub fn parse_ledger_ts(ts: &str) -> Option<f64> {
    let naive = chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%SZ").ok()?;
    match naive.and_local_timezone(chrono::Local) {
        chrono::LocalResult::Single(dt) => Some(dt.timestamp() as f64),
        chrono::LocalResult::Ambiguous(a, _) => Some(a.timestamp() as f64),
        chrono::LocalResult::None => None,
    }
}

pub fn utc_now_stamp() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Local wall clock with a literal `Z` suffix — the same stamp shape the
/// Python selftest writes via `time.strftime("%Y-%m-%dT%H:%M:%SZ")`.
pub fn local_wall_z_stamp() -> String {
    chrono::Local::now()
        .naive_local()
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string()
}

/// `br show <bead> --json` in `repo_dir`. stdin is null so a caller pipe cannot
/// deadlock the child (the failure class this port exists to make inexpressible).
/// Verdicts are unchanged: `br show` does not read stdin.
pub fn bead_status_via_br(repo_dir: &Path, bead: &str) -> Option<String> {
    let mut cmd = Command::new("br");
    cmd.arg("show")
        .arg(bead)
        .arg("--json")
        .current_dir(repo_dir)
        .env("RUST_LOG", "error")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = match cmd.output() {
        Ok(o) => o,
        Err(_) => return None,
    };
    // A timeout is not expressible on Command::output without a helper thread;
    // the live caller wraps the whole binary in `timeout 400`. A spawn failure
    // or nonzero br is "not closed", matching the oracle's `return None`.
    let _ = BR_TIMEOUT;
    if !output.status.success() {
        return None;
    }
    let out = String::from_utf8_lossy(&output.stdout);
    if out.trim().is_empty() {
        return None;
    }
    parse_br_status(&out)
}

pub fn parse_br_status(out: &str) -> Option<String> {
    let d: Value = serde_json::from_str(out.trim()).ok()?;
    let r = if let Some(arr) = d.as_array() {
        arr.first()?.clone()
    } else {
        d
    };
    r.get("status")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
}

fn collect_beads(ds: &[Value]) -> Vec<String> {
    let mut beads = Vec::new();
    for d in ds {
        let items = match d.get("beads") {
            None | Some(Value::Null) => continue,
            Some(Value::Bool(false)) => continue,
            Some(Value::Number(n)) if n.as_f64() == Some(0.0) => continue,
            Some(Value::String(s)) if s.is_empty() => continue,
            Some(Value::Array(arr)) => arr
                .iter()
                .filter_map(|b| b.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>(),
            // Match CPython `for b in beads`: a string iterates as characters;
            // an object iterates as keys. A leftover non-iterable would raise
            // TypeError in the oracle; we do not invent a third behaviour.
            Some(Value::String(s)) => s.chars().map(|c| c.to_string()).collect(),
            Some(Value::Object(map)) => map.keys().cloned().collect(),
            Some(Value::Bool(true)) | Some(Value::Number(_)) => {
                // Oracle TypeError. Surface it rather than guess.
                return vec!["__oracle_typeerror_beads__".to_string()];
            }
        };
        for b in items {
            if !b.is_empty() && !beads.contains(&b) {
                beads.push(b);
            }
        }
    }
    beads
}

fn is_legacy(d: &Value) -> bool {
    match d.get("beads") {
        None | Some(Value::Null) => true,
        Some(Value::Bool(false)) => true,
        Some(Value::Number(n)) if n.as_f64() == Some(0.0) => true,
        Some(Value::String(s)) if s.is_empty() => true,
        Some(Value::Array(a)) if a.is_empty() => {
            // `d.get("beads") or []` is truthy for []? In Python, [] is falsy,
            // so `not d.get("beads")` is True for []. `if not d.get("beads")`
            // is the legacy test. Empty list IS legacy.
            true
        }
        Some(_) => false,
    }
}

pub fn run(cfg: &VerifyDispatchConfig, status: &dyn Fn(&Path, &str) -> Option<String>) -> VerifyDispatchRunOutput {
    let mut stdout = String::new();
    if !cfg.ledger.exists() {
        stdout.push_str(&format!(
            "no controller-tick ledger at {}\n",
            cfg.ledger.display()
        ));
        return VerifyDispatchRunOutput { stdout, code: 0 };
    }

    let text = match std::fs::read_to_string(&cfg.ledger) {
        Ok(t) => t,
        Err(_) => {
            // Oracle `open()` would traceback. A traceback is a usage/IO failure,
            // not a verdict; keep it off stdout.
            return VerifyDispatchRunOutput { stdout, code: 0 };
        }
    };

    let cutoff = cfg.now - cfg.window_h * 3600.0;
    let mut dispatches: Vec<Value> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let d: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if cfg.rules.event_is_dispatched && d.get("event").and_then(|e| e.as_str()) != Some("dispatched")
        {
            continue;
        }
        let ts = d.get("ts").and_then(|t| t.as_str()).unwrap_or("");
        let t = match parse_ledger_ts(ts) {
            Some(v) => v,
            None => continue,
        };
        if cfg.rules.window_cutoff && t < cutoff {
            continue;
        }
        dispatches.push(d);
    }

    if dispatches.is_empty() {
        stdout.push_str(&format!(
            "no dispatches in the last {:.0}h\n",
            cfg.window_h
        ));
        return VerifyDispatchRunOutput { stdout, code: 0 };
    }

    stdout.push_str(&format!(
        "VERIFYING {} dispatch(es) from the last {:.0}h — ground truth only\n\n",
        dispatches.len(),
        cfg.window_h
    ));

    let mut by_repo: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for d in dispatches {
        let repo = d
            .get("repo")
            .and_then(|r| r.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("control-plane")
            .to_string();
        by_repo.entry(repo).or_default().push(d);
    }

    let mut verified = 0usize;
    let mut unverified = 0usize;
    for (repo, ds) in &by_repo {
        let repo_dir = cfg.developer_root.join(repo);
        if !repo_dir.is_dir() {
            continue;
        }

        let beads = collect_beads(ds);
        if beads.iter().any(|b| b == "__oracle_typeerror_beads__") {
            stdout.push_str(
                "STOP: beads field is not iterable; oracle would TypeError. Not guessing.\n",
            );
            return VerifyDispatchRunOutput { stdout, code: 1 };
        }
        let legacy: Vec<&Value> = ds.iter().filter(|d| is_legacy(d)).collect();

        let mut closed: Vec<String> = Vec::new();
        for b in &beads {
            let st = status(&repo_dir, b);
            let counts = if cfg.rules.only_closed_status_counts {
                st.as_deref() == Some("closed")
            } else {
                true
            };
            if counts {
                closed.push(b.clone());
            }
        }

        let all_closed = if cfg.rules.named_beads_all_closed {
            !beads.is_empty() && closed.len() == beads.len()
        } else {
            !beads.is_empty()
        };

        let state = if all_closed { "VERIFIED" } else { "NO EVIDENCE" };
        if all_closed {
            verified += 1;
        } else {
            unverified += 1;
        }

        stdout.push_str(&format!(
            "  {:<18} {:<12} dispatches={} beads_closed={}/{}\n",
            repo,
            state,
            ds.len(),
            closed.len(),
            beads.len()
        ));
        if cfg.rules.legacy_idless_reported && !legacy.is_empty() {
            stdout.push_str(&format!(
                "      note: {} dispatch(es) predate bead-id ledgering and are not recoverable from the overwritten packet file\n",
                legacy.len()
            ));
        }
        for b in closed.iter().take(3) {
            stdout.push_str(&format!("      closed: {b}\n"));
        }
        if state == "NO EVIDENCE" {
            stdout.push_str(
                "      -> dispatched set is not closed. Work may be in flight, the\n",
            );
            stdout.push_str(
                "         packet may not have landed, or only part of it completed.\n",
            );
        }

        let row = OutRow {
            ts: utc_now_stamp(),
            event: if all_closed {
                "dispatch_verified".to_string()
            } else {
                "dispatch_no_evidence".to_string()
            },
            repo: repo.clone(),
            dispatches: ds.len(),
            beads_closed: closed.len(),
            beads_total: beads.len(),
            detector: if all_closed {
                "named_beads_all_closed".to_string()
            } else {
                "named_beads_incomplete".to_string()
            },
        };
        if let Ok(mut fh) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&cfg.out)
        {
            if let Ok(line) = serde_json::to_string(&row) {
                let _ = writeln!(fh, "{line}");
            }
        }
    }

    stdout.push_str(&format!(
        "\n{verified} verified, {unverified} with no evidence\n"
    ));
    if unverified > 0 {
        stdout.push_str(
            "NO EVIDENCE is a real finding, not a failure — it separates 'dispatched' from 'landed',\n",
        );
        stdout.push_str(
            "which is exactly what 14 dispatches and 0 verifications could not do.\n",
        );
    }
    VerifyDispatchRunOutput { stdout, code: 0 }
}

pub fn run_live(cfg: &VerifyDispatchConfig) -> VerifyDispatchRunOutput {
    run(cfg, &bead_status_via_br)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

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

    fn tmp_paths(tag: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "verify-dispatch-{}-{}",
            tag,
            std::process::id()
        ));
        let _ = fs::create_dir_all(&dir);
        (dir.join("ledger.jsonl"), dir.join("out.jsonl"))
    }

    fn cfg(ledger: PathBuf, out: PathBuf, now: f64) -> VerifyDispatchConfig {
        VerifyDispatchConfig {
            ledger,
            out,
            window_h: 6.0,
            developer_root: std::env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .map(|home| PathBuf::from(home).join("Developer"))
                .unwrap_or_default(),
            now,
            rules: VerifyDispatchRules::default(),
        }
    }

    fn stamp_at(now: f64) -> String {
        use chrono::TimeZone;
        chrono::Local
            .timestamp_opt(now as i64, 0)
            .single()
            .expect("timestamp")
            .naive_local()
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string()
    }

    fn row(now: f64, beads: &[&str]) -> String {
        let ts = stamp_at(now);
        let beads_json: Vec<String> = beads.iter().map(|b| format!("\"{b}\"")).collect();
        format!(
            "{{\"ts\":\"{ts}\",\"event\":\"dispatched\",\"repo\":\"control-plane\",\"pane\":\"1\",\"count\":{},\"beads\":[{}],\"invoker\":\"TEST\"}}",
            beads.len(),
            beads_json.join(",")
        )
    }

    fn status_map(pairs: &[(&str, &str)]) -> impl Fn(&Path, &str) -> Option<String> {
        let map: BTreeMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |_repo: &Path, bead: &str| map.get(bead).cloned()
    }

    #[test]
    fn missing_ledger_verdict_is_stdout_column_0() {
        let (ledger, out) = tmp_paths("missing");
        let _ = fs::remove_file(&ledger);
        let c = cfg(ledger.clone(), out, 1_700_000_000.0);
        let r = run(&c, &status_map(&[]));
        assert_eq!(r.code, 0, "rule exit_zero_always: missing ledger must not gate");
        assert!(
            r.stdout.starts_with("no controller-tick ledger at "),
            "rule stdout_verdict_column_0: missing-ledger verdict must start at column 0, got {:?}",
            r.stdout
        );
        assert!(
            !r.stdout.starts_with(' '),
            "rule stdout_verdict_column_0: leading whitespace would hide a RED from `gate > log`"
        );
    }

    #[test]
    fn no_dispatches_in_window_is_stdout_column_0() {
        let (ledger, out) = tmp_paths("emptywin");
        fs::write(&ledger, "{\"ts\":\"1999-01-01T00:00:00Z\",\"event\":\"dispatched\",\"repo\":\"control-plane\",\"beads\":[\"x\"]}\n")
            .unwrap();
        let c = cfg(ledger, out, 1_700_000_000.0);
        let r = run(&c, &status_map(&[]));
        assert_eq!(r.code, 0);
        assert!(
            r.stdout.starts_with("no dispatches in the last 6h\n"),
            "rule window_cutoff: old rows must not verify, got {:?}",
            r.stdout
        );
    }

    #[test]
    fn closed_bead_is_verified() {
        let (ledger, out) = tmp_paths("closed");
        let now = 1_700_000_000.0;
        fs::write(&ledger, row(now, &["cp-closed"]) + "\n").unwrap();
        let c = cfg(ledger, out, now);
        let r = run(&c, &status_map(&[("cp-closed", "closed")]));
        assert!(
            r.stdout.contains("VERIFIED") && r.stdout.contains("beads_closed=1/1"),
            "rule named_beads_all_closed: a dispatch naming a CLOSED bead must count it, got {:?}",
            r.stdout
        );
        assert!(r.stdout.contains("\n1 verified, 0 with no evidence\n"));
    }

    #[test]
    fn open_bead_is_no_evidence() {
        let (ledger, out) = tmp_paths("open");
        let now = 1_700_000_000.0;
        fs::write(&ledger, row(now, &["cp-open"]) + "\n").unwrap();
        let c = cfg(ledger, out, now);
        let r = run(&c, &status_map(&[("cp-open", "open")]));
        assert!(
            r.stdout.contains("NO EVIDENCE") && r.stdout.contains("beads_closed=0/1"),
            "rule only_closed_status_counts: an OPEN bead must NOT count as closed, got {:?}",
            r.stdout
        );
        assert_eq!(r.code, 0, "rule exit_zero_always: NO EVIDENCE is a finding, not a gate");
        assert!(
            r.stdout.contains("NO EVIDENCE is a real finding"),
            "rule no_evidence_named: the finding must be on stdout at column 0"
        );
        assert!(
            r.stdout
                .lines()
                .any(|l| l.starts_with("NO EVIDENCE is a real finding")),
            "rule stdout_verdict_column_0: NO EVIDENCE finding must start at column 0"
        );
    }

    #[test]
    fn partial_close_is_no_evidence() {
        let (ledger, out) = tmp_paths("partial");
        let now = 1_700_000_000.0;
        fs::write(&ledger, row(now, &["cp-closed", "cp-open"]) + "\n").unwrap();
        let c = cfg(ledger, out, now);
        let r = run(
            &c,
            &status_map(&[("cp-closed", "closed"), ("cp-open", "open")]),
        );
        assert!(
            r.stdout.contains("NO EVIDENCE") && r.stdout.contains("beads_closed=1/2"),
            "rule named_beads_all_closed: a partial close is progress, not proof the dispatched set completed, got {:?}",
            r.stdout
        );
    }

    #[test]
    fn empty_beads_list_is_legacy_and_no_evidence() {
        let (ledger, out) = tmp_paths("legacy");
        let now = 1_700_000_000.0;
        let ts = stamp_at(now);
        fs::write(
            &ledger,
            format!("{{\"ts\":\"{ts}\",\"event\":\"dispatched\",\"repo\":\"control-plane\",\"pane\":\"1\",\"count\":2,\"invoker\":\"TEST\"}}\n"),
        )
        .unwrap();
        let c = cfg(ledger, out, now);
        let r = run(&c, &status_map(&[]));
        assert!(
            r.stdout.contains("predate bead-id ledgering"),
            "rule legacy_idless_reported: a row with no ids must be REPORTED, not silently treated as zero, got {:?}",
            r.stdout
        );
        assert!(
            r.stdout.contains("NO EVIDENCE"),
            "rule named_beads_all_closed: empty named set is not VERIFIED, got {:?}",
            r.stdout
        );
    }

    #[test]
    fn non_dispatched_event_is_ignored() {
        let (ledger, out) = tmp_paths("event");
        let now = 1_700_000_000.0;
        let ts = stamp_at(now);
        fs::write(
            &ledger,
            format!("{{\"ts\":\"{ts}\",\"event\":\"tick\",\"repo\":\"control-plane\",\"beads\":[\"cp-closed\"]}}\n"),
        )
        .unwrap();
        let c = cfg(ledger, out, now);
        let r = run(&c, &status_map(&[("cp-closed", "closed")]));
        assert!(
            r.stdout.starts_with("no dispatches in the last 6h\n"),
            "rule event_is_dispatched: a non-dispatched event must not verify, got {:?}",
            r.stdout
        );
    }

    #[test]
    fn malformed_json_is_skipped() {
        let (ledger, out) = tmp_paths("malformed");
        let now = 1_700_000_000.0;
        fs::write(&ledger, format!("not-json\n{}\n", row(now, &["cp-closed"]))).unwrap();
        let c = cfg(ledger, out, now);
        let r = run(&c, &status_map(&[("cp-closed", "closed")]));
        assert!(
            r.stdout.contains("beads_closed=1/1"),
            "malformed lines must be skipped, not abort the ledger, got {:?}",
            r.stdout
        );
    }

    #[test]
    fn missing_repo_dir_is_skipped() {
        let (ledger, out) = tmp_paths("norepo");
        let now = 1_700_000_000.0;
        let ts = stamp_at(now);
        fs::write(
            &ledger,
            format!("{{\"ts\":\"{ts}\",\"event\":\"dispatched\",\"repo\":\"definitely-not-a-developer-repo-zzz\",\"beads\":[\"cp-closed\"]}}\n"),
        )
        .unwrap();
        let c = cfg(ledger, out, now);
        let r = run(&c, &status_map(&[("cp-closed", "closed")]));
        assert!(
            r.stdout.contains("VERIFYING 1 dispatch"),
            "the dispatch is in-window"
        );
        assert!(
            r.stdout.contains("\n0 verified, 0 with no evidence\n"),
            "rule skip_missing_repo: a missing Developer/ dir is skipped, not VERIFIED, got {:?}",
            r.stdout
        );
    }

    #[test]
    fn duplicate_bead_ids_are_deduped() {
        let (ledger, out) = tmp_paths("dedupe");
        let now = 1_700_000_000.0;
        let a = row(now, &["cp-closed"]);
        let b = row(now, &["cp-closed"]);
        fs::write(&ledger, format!("{a}\n{b}\n")).unwrap();
        let c = cfg(ledger, out, now);
        let r = run(&c, &status_map(&[("cp-closed", "closed")]));
        assert!(
            r.stdout.contains("dispatches=2 beads_closed=1/1"),
            "rule ledger_ids_union: duplicate ids across dispatches must count once, got {:?}",
            r.stdout
        );
    }

    #[test]
    fn disabling_named_beads_all_closed_false_passes_open_set() {
        let (ledger, out) = tmp_paths("mut-all");
        let now = 1_700_000_000.0;
        fs::write(&ledger, row(now, &["cp-open"]) + "\n").unwrap();
        let mut c = cfg(ledger, out, now);
        assert!(
            c.rules.disable("named_beads_all_closed"),
            "named_beads_all_closed must be a known rule"
        );
        let r = run(&c, &status_map(&[("cp-open", "open")]));
        assert!(
            r.stdout.contains("VERIFIED"),
            "mutation named_beads_all_closed: disabling it must false-PASS an open set, got {:?}",
            r.stdout
        );
    }

    #[test]
    fn disabling_only_closed_status_counts_false_passes_open_bead() {
        let (ledger, out) = tmp_paths("mut-status");
        let now = 1_700_000_000.0;
        fs::write(&ledger, row(now, &["cp-open"]) + "\n").unwrap();
        let mut c = cfg(ledger, out, now);
        assert!(c.rules.disable("only_closed_status_counts"));
        let r = run(&c, &status_map(&[("cp-open", "open")]));
        assert!(
            r.stdout.contains("VERIFIED") && r.stdout.contains("beads_closed=1/1"),
            "mutation only_closed_status_counts: disabling it must count an open bead as closed, got {:?}",
            r.stdout
        );
    }

    #[test]
    fn disabling_legacy_idless_reported_hides_the_note() {
        let (ledger, out) = tmp_paths("mut-legacy");
        let now = 1_700_000_000.0;
        let ts = stamp_at(now);
        fs::write(
            &ledger,
            format!("{{\"ts\":\"{ts}\",\"event\":\"dispatched\",\"repo\":\"control-plane\",\"count\":2,\"invoker\":\"TEST\"}}\n"),
        )
        .unwrap();
        let mut c = cfg(ledger, out, now);
        assert!(c.rules.disable("legacy_idless_reported"));
        let r = run(&c, &status_map(&[]));
        assert!(
            !r.stdout.contains("predate bead-id ledgering"),
            "mutation legacy_idless_reported: disabling it must hide the note, got {:?}",
            r.stdout
        );
    }

    #[test]
    fn unknown_rule_name_is_rejected() {
        let mut r = VerifyDispatchRules::default();
        assert!(
            !r.disable("no_such_rule"),
            "an unknown guard name must not report a vacuous mutation pass"
        );
    }

    #[test]
    fn every_named_rule_is_disableable() {
        // fh C75: a name in VerifyDispatchRule::ALL that `disable` does not know would make
        // the mutation harness report a vacuous pass for that rule.
        assert!(
            !VerifyDispatchRule::ALL.is_empty(),
            "C75: an empty VerifyDispatchRule::ALL is not a clean bill"
        );
        for rule in VerifyDispatchRule::ALL {
            let mut g = VerifyDispatchRules::default();
            assert!(g.all_enabled(), "default rules must start enabled");
            assert!(
                g.disable(rule.as_str()),
                "C75: VerifyDispatchRule::ALL entry {} is not disableable",
                rule.as_str()
            );
            assert!(
                !g.all_enabled(),
                "C75: disabling {} did not change the struct",
                rule.as_str()
            );
            assert_eq!(
                VerifyDispatchRule::parse(rule.as_str()),
                Some(*rule),
                "C75: as_str/parse must round-trip for {}",
                rule.as_str()
            );
        }
    }

    #[test]
    fn parse_br_status_accepts_list_or_object() {
        assert_eq!(
            parse_br_status("[{\"id\":\"x\",\"status\":\"closed\"}]").as_deref(),
            Some("closed")
        );
        assert_eq!(
            parse_br_status("{\"id\":\"x\",\"status\":\"open\"}").as_deref(),
            Some("open")
        );
        assert_eq!(parse_br_status("not-json"), None);
        assert_eq!(parse_br_status("[]"), None);
    }

    #[test]
    fn parse_ledger_ts_matches_python_mktime_local() {
        let status = oracle_status();
        let OracleStatus::Ready = status else {
            announce_skip("parse_ledger_ts_matches_python_mktime_local", &status);
            return;
        };
        let _g = ENV_LOCK.lock().unwrap();
        let ts = "2026-08-26T16:00:00Z";
        let rust = parse_ledger_ts(ts).expect("parse");
        let py = std::process::Command::new("python3")
            .args([
                "-c",
                "import time,sys; print(time.mktime(time.strptime(sys.argv[1], '%Y-%m-%dT%H:%M:%SZ')))",
                ts,
            ])
            .output()
            .expect("python");
        let py: f64 = String::from_utf8_lossy(&py.stdout)
            .trim()
            .parse()
            .expect("py float");
        assert!(
            (rust - py).abs() < 1.0,
            "rule timestamp_matches_oracle: rust {rust} vs python {py}"
        );
    }
}

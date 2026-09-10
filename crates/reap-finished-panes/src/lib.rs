#![forbid(unsafe_code)]

//! Sweep finished worker panes. The reaping path is implemented in this crate.
//! The control-plane script remains only as an external differential oracle.

use fs2::FileExt;
use pane_dispatch_ready::{classify, PaneDispatchReadyRules, PaneDispatchReadyState};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};
use subprocess_contract::BoundedOutcome;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReapFinishedPanesRule {
    SkipHumanShell,
    DeadlineReportsUnswept,
    LockNamesHolder,
    StrictReapPredicate,
}

impl ReapFinishedPanesRule {
    pub const ALL: &'static [ReapFinishedPanesRule] = &[
        ReapFinishedPanesRule::SkipHumanShell,
        ReapFinishedPanesRule::DeadlineReportsUnswept,
        ReapFinishedPanesRule::LockNamesHolder,
        ReapFinishedPanesRule::StrictReapPredicate,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            ReapFinishedPanesRule::SkipHumanShell => "skip_human_shell",
            ReapFinishedPanesRule::DeadlineReportsUnswept => "deadline_reports_unswept",
            ReapFinishedPanesRule::LockNamesHolder => "lock_names_holder",
            ReapFinishedPanesRule::StrictReapPredicate => "strict_reap_predicate",
        }
    }
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|r| r.as_str() == name)
    }
}

#[derive(Clone, Debug)]
pub struct ReapFinishedPanesRules {
    pub skip_human_shell: bool,
    pub deadline_reports_unswept: bool,
    pub lock_names_holder: bool,
    pub strict_reap_predicate: bool,
}

impl Default for ReapFinishedPanesRules {
    fn default() -> Self {
        Self {
            skip_human_shell: true,
            deadline_reports_unswept: true,
            lock_names_holder: true,
            strict_reap_predicate: true,
        }
    }
}

impl ReapFinishedPanesRules {
    pub fn disable(&mut self, name: &str) -> bool {
        let Some(rule) = ReapFinishedPanesRule::parse(name) else {
            return false;
        };
        match rule {
            ReapFinishedPanesRule::SkipHumanShell => self.skip_human_shell = false,
            ReapFinishedPanesRule::DeadlineReportsUnswept => self.deadline_reports_unswept = false,
            ReapFinishedPanesRule::LockNamesHolder => self.lock_names_holder = false,
            ReapFinishedPanesRule::StrictReapPredicate => self.strict_reap_predicate = false,
        }
        true
    }
    pub fn known_names_csv() -> String {
        ReapFinishedPanesRule::ALL
            .iter()
            .map(|r| r.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReapFinishedPanesInvoker {
    pub invoker: &'static str,
    pub proof: &'static str,
}
impl ReapFinishedPanesInvoker {
    pub const MANUAL: ReapFinishedPanesInvoker = ReapFinishedPanesInvoker {
        invoker: "MANUAL",
        proof: "unproven",
    };
    pub const SCHEDULED: ReapFinishedPanesInvoker = ReapFinishedPanesInvoker {
        invoker: "SCHEDULED",
        proof: "cron_parent",
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReapFinishedPanesAncestorRow {
    pub uid: u32,
    pub ppid: u32,
    pub comm: String,
}

pub fn parse_ancestor_rows(text: &str) -> Vec<ReapFinishedPanesAncestorRow> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let (Some(uid), Some(ppid), Some(comm)) = (it.next(), it.next(), it.next()) else {
            continue;
        };
        let (Ok(uid), Ok(ppid)) = (uid.parse::<u32>(), ppid.parse::<u32>()) else {
            continue;
        };
        out.push(ReapFinishedPanesAncestorRow {
            uid,
            ppid,
            comm: comm.to_string(),
        });
    }
    out
}

pub fn invoker_from_chain(chain: &[ReapFinishedPanesAncestorRow]) -> ReapFinishedPanesInvoker {
    for row in chain {
        if row.uid == 0 && row.ppid == 1 && row.comm == "/usr/sbin/cron" {
            return ReapFinishedPanesInvoker::SCHEDULED;
        }
    }
    ReapFinishedPanesInvoker::MANUAL
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SweepStats {
    pub reaped: u64,
    pub skipped: u64,
    pub awaiting_human: u64,
    pub unswept: u64,
    pub deadline_hit: u8,
    pub elapsed_secs: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReapPaneDecision {
    Reaped { awaiting_human: bool },
    Working,
    Changing,
    Empty,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReapPaneResult {
    Reaped {
        path: Option<PathBuf>,
        awaiting_human: bool,
        bytes: usize,
    },
    Skipped {
        reason: &'static str,
    },
    Unreadable {
        reason: String,
    },
}

/// The reaping predicate is strict: two non-empty equal captures and a FREE readiness verdict.
/// Rule `strict_reap_predicate` is the production knob over that strictness; disabling it keeps
/// only the non-empty check, so a WORKING pane is admitted for reaping. That is the mutation.
pub fn should_reap_with(
    first: &str,
    second: &str,
    ready: bool,
    rules: &ReapFinishedPanesRules,
) -> bool {
    if !rules.strict_reap_predicate {
        return !first.trim().is_empty();
    }
    !first.trim().is_empty() && first == second && ready
}
pub fn should_reap(first: &str, second: &str, ready: bool) -> bool {
    should_reap_with(first, second, ready, &ReapFinishedPanesRules::default())
}
pub fn decide_reap_with(
    first: &str,
    second: &str,
    ready: bool,
    text: &str,
    rules: &ReapFinishedPanesRules,
) -> ReapPaneDecision {
    if !should_reap_with(first, second, ready, rules) {
        if first.trim().is_empty() || second.trim().is_empty() {
            return ReapPaneDecision::Empty;
        }
        if first != second {
            return ReapPaneDecision::Changing;
        }
        return ReapPaneDecision::Working;
    }
    ReapPaneDecision::Reaped {
        awaiting_human: awaiting_human(text),
    }
}
pub fn decide_reap(first: &str, second: &str, ready: bool, text: &str) -> ReapPaneDecision {
    decide_reap_with(
        first,
        second,
        ready,
        text,
        &ReapFinishedPanesRules::default(),
    )
}

fn awaiting_human(text: &str) -> bool {
    text.lines()
        .rev()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .find(|line| !matches!(line.chars().next(), Some('│' | '─' | '└' | '├' | '›')))
        .is_some_and(|line| line.ends_with('?'))
}

pub fn require_panes<T>(panes: &[T]) -> Result<(), &'static str> {
    if panes.is_empty() {
        Err("empty pane set — refusing a vacuous reap sweep")
    } else {
        Ok(())
    }
}

/// Typed refusal when the sweep is invoked without a session. An absent scope
/// must never widen to every tmux session on the server.
pub const MISSING_SESSION_REFUSAL: &str = "SCOPE_REFUSED reason=MISSING_SESSION \
    next_action=pass --session <tmux-session> -- an unscoped sweep enumerates every \
    tmux session on this server, including repos this process does not own";

/// Keep only panes whose tmux session name equals `session`.
pub fn filter_panes_to_session(
    panes: &[(String, String)],
    session: &str,
) -> Vec<(String, String)> {
    panes
        .iter()
        .filter(|(name, _)| name == session)
        .cloned()
        .collect()
}

/// Panes that would be stolen if the filter were skipped. Known-bad when nonempty
/// after a scoped sweep.
pub fn foreign_sessions_in(panes: &[(String, String)], session: &str) -> Vec<String> {
    panes
        .iter()
        .filter(|(name, _)| name != session)
        .map(|(name, idx)| format!("{name}:{idx}"))
        .collect()
}

/// Per-repo write dir so two supervisors cannot read each other's transcripts
/// as their own. `base` is typically `~/.local/state/flywheel/reaped`.
pub fn scoped_artifact_dir(base: &Path, repo: &Path) -> PathBuf {
    let name = repo
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown-repo");
    base.join(safe_component(name))
}

pub fn resolve_pane_id(session: &str, idx: &str, timeout: Duration) -> Result<String, String> {
    let mut cmd = Command::new(tick_monitor::TMUX);
    cmd.args([
        "list-panes",
        "-s",
        "-t",
        session,
        "-F",
        "#{pane_id} #{session_name}:#{window_index}.#{pane_index}",
    ]);
    let out = spawn_timeout(cmd, timeout)
        .ok_or_else(|| "tmux list-panes timed out or failed".to_owned())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_owned());
    }
    let wanted = format!("{session}:0.{idx}");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|line| {
            let mut fields = line.split_whitespace();
            let pane_id = fields.next()?;
            (fields.next()? == wanted).then(|| pane_id.to_owned())
        })
        .ok_or_else(|| format!("no such pane {session}:0.{idx}"))
}

pub fn capture_pane(pane_id: &str, lines: usize, timeout: Duration) -> Result<String, String> {
    let mut cmd = Command::new(tick_monitor::TMUX);
    cmd.args([tick_monitor::CAPTURE_PANE, "-p", "-e", "-t", pane_id, "-S"])
        .arg(format!("-{lines}"));
    let out = spawn_timeout(cmd, timeout)
        .ok_or_else(|| {
            format!(
                "{} {} timed out or failed",
                tick_monitor::TMUX,
                tick_monitor::CAPTURE_PANE
            )
        })?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn safe_component(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn rendered_result_bytes(text: &str) -> usize {
    text.trim_end_matches('\n').len() + 1
}

pub fn write_reaped_result(
    outdir: &Path,
    ledger: &Path,
    session: &str,
    pane: &str,
    pane_id: &str,
    text: &str,
    awaiting_human: bool,
    stamp: &str,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(outdir)?;
    let path = outdir.join(format!(
        "{}.pane{}.{}.txt",
        safe_component(session),
        safe_component(pane),
        stamp
    ));
    let result_text = text.trim_end_matches('\n');
    let result_bytes = rendered_result_bytes(text);
    std::fs::write(&path, format!("{result_text}\n"))?;
    if let Some(parent) = ledger.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let row = serde_json::json!({"ts": stamp, "event": "result_reaped", "session": session, "pane": pane, "pane_id": pane_id, "awaiting_human": awaiting_human, "bytes": result_bytes, "path": path});
    let mut file = OpenOptions::new().create(true).append(true).open(ledger)?;
    writeln!(file, "{row}")?;
    Ok(path)
}

pub fn reap_pane(
    session: &str,
    idx: &str,
    settle: Duration,
    lines: usize,
    apply: bool,
    outdir: &Path,
    ledger: &Path,
    stamp: &str,
    rules: &ReapFinishedPanesRules,
) -> ReapPaneResult {
    let pane_id = match resolve_pane_id(session, idx, Duration::from_secs(15)) {
        Ok(id) => id,
        Err(reason) => return ReapPaneResult::Unreadable { reason },
    };
    let first = match capture_pane(&pane_id, lines, Duration::from_secs(15)) {
        Ok(text) => text,
        Err(reason) => return ReapPaneResult::Unreadable { reason },
    };
    std::thread::sleep(settle);
    let second = match capture_pane(&pane_id, lines, Duration::from_secs(15)) {
        Ok(text) => text,
        Err(reason) => return ReapPaneResult::Unreadable { reason },
    };
    let readiness = classify(&second, false, &PaneDispatchReadyRules::default());
    let ready = readiness.state == PaneDispatchReadyState::Free;
    match decide_reap_with(&first, &second, ready, &second, rules) {
        ReapPaneDecision::Empty => ReapPaneResult::Skipped {
            reason: "empty_capture",
        },
        ReapPaneDecision::Changing => ReapPaneResult::Skipped {
            reason: "still_changing",
        },
        ReapPaneDecision::Working => ReapPaneResult::Skipped { reason: "working" },
        ReapPaneDecision::Reaped { awaiting_human } if apply => {
            match write_reaped_result(
                outdir,
                ledger,
                session,
                idx,
                &pane_id,
                &second,
                awaiting_human,
                stamp,
            ) {
                Ok(path) => ReapPaneResult::Reaped {
                    path: Some(path),
                    awaiting_human,
                    bytes: second.len(),
                },
                Err(error) => ReapPaneResult::Unreadable {
                    reason: format!("write reaped result: {error}"),
                },
            }
        }
        ReapPaneDecision::Reaped { awaiting_human } => ReapPaneResult::Reaped {
            path: None,
            awaiting_human,
            bytes: rendered_result_bytes(&second),
        },
    }
}

pub fn consecutive_cycle_started_same_pid(heartbeat: &str) -> bool {
    let mut previous: Option<String> = None;
    let mut cycles = 0usize;
    for line in heartbeat.lines() {
        let Ok(row) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if row.get("event").and_then(|v| v.as_str()) != Some("CYCLE_STARTED") {
            continue;
        }
        let Some(pid) = row.get("pid").or_else(|| row.get("process_pid")) else {
            return false;
        };
        let current = pid.to_string();
        if previous.as_deref().is_some_and(|old| old != current) {
            return false;
        }
        previous = Some(current);
        cycles += 1;
    }
    cycles >= 2
}

/// A pane with index 0 is the human shell — never a worker, never reaped.
pub fn is_worker_pane(idx: &str, rules: &ReapFinishedPanesRules) -> bool {
    if !rules.skip_human_shell {
        return true;
    }
    idx.parse::<i64>().map(|n| n > 0).unwrap_or(false)
}

/// Count remaining worker panes as unswept when the deadline fires.
pub fn apply_deadline(
    stats: &mut SweepStats,
    started: Instant,
    deadline: Duration,
    rules: &ReapFinishedPanesRules,
) -> bool {
    if !rules.deadline_reports_unswept {
        return false;
    }
    if stats.deadline_hit == 1 || started.elapsed() >= deadline {
        stats.deadline_hit = 1;
        stats.unswept += 1;
        true
    } else {
        false
    }
}

pub fn parse_reaper_out(out: &str, rc_ok: bool) -> (&'static str, bool) {
    if rc_ok && out.starts_with("REAPED") {
        let awaiting = out.contains("awaiting_human=1");
        ("reaped", awaiting)
    } else {
        ("skipped", false)
    }
}

#[derive(Debug)]
pub struct ReapFinishedPanesRunLock {
    file: File,
    pub path: PathBuf,
}
impl Drop for ReapFinishedPanesRunLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

/// Who holds the lock is a SEPARATE verdict from whether the probe could run.
/// Collapsing both into the token "unknown" is the denied-probe-as-negative-result
/// trap: "nobody holds it" and "I could not look" have different remedies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReapLockHolder {
    /// The probe ran and named a holder.
    Named { pid: String, elapsed: String },
    /// The probe ran and found no other holder (lock held by this process tree only).
    NoHolder,
    /// The probe could NOT run: lsof was absent at every candidate path, or the
    /// bounded spawn timed out. Never reported as a pid.
    ProbeUnavailable { reason: String },
}

impl ReapLockHolder {
    /// Operator-facing detail. Never emits the string "unknown".
    pub fn detail(&self) -> String {
        match self {
            ReapLockHolder::Named { pid, elapsed } => format!("pid={pid} elapsed={elapsed}"),
            ReapLockHolder::NoHolder => "holder=none probe=ran".to_string(),
            ReapLockHolder::ProbeUnavailable { reason } => {
                format!("holder=unprobed probe_unavailable={reason}")
            }
        }
    }
    pub fn state(&self) -> &'static str {
        match self {
            ReapLockHolder::Named { .. } => "named",
            ReapLockHolder::NoHolder => "none",
            ReapLockHolder::ProbeUnavailable { .. } => "unprobed",
        }
    }
}

#[derive(Debug)]
pub enum ReapFinishedPanesLockOutcome {
    Acquired(ReapFinishedPanesRunLock),
    Busy {
        holder: ReapLockHolder,
    },
    Unusable {
        reason: String,
    },
}

pub fn acquire_lock(path: &Path) -> ReapFinishedPanesLockOutcome {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let file = match OpenOptions::new().create(true).append(true).open(path) {
        Ok(f) => f,
        Err(e) => {
            return ReapFinishedPanesLockOutcome::Unusable {
                reason: format!("open_failed: {e}"),
            }
        }
    };
    match file.try_lock_exclusive() {
        Ok(()) => ReapFinishedPanesLockOutcome::Acquired(ReapFinishedPanesRunLock {
            file,
            path: path.to_path_buf(),
        }),
        Err(_) => ReapFinishedPanesLockOutcome::Busy {
            holder: lsof_holder(path),
        },
    }
}

/// Same candidate pair as `loop-driver/src/lib.rs:462` and
/// `omp-orchestrator/src/target_directory.rs:344`: Darwin ships lsof in /usr/sbin,
/// Debian/Ubuntu (the Contabo workers) in /usr/bin. fh C47: those two sites hold a
/// PATH list, not this crate's identity-bearing contract — that contract is the TYPED
/// verdict below, which `lock_holder_pids` cannot express (it returns an empty Vec for
/// both "no holder" and "lsof missing"), and it is private to a crate this one does not
/// depend on. The list is duplicated; the verdict is defined once, here.
pub const LSOF_CANDIDATES: [&str; 2] = ["/usr/sbin/lsof", "/usr/bin/lsof"];

fn lsof_holder(path: &Path) -> ReapLockHolder {
    // Measured 2026-08-27: `lsof -t` on a held lock took 2.8–3.2s, so the 2s
    // bound killed it with empty stdout and the skip row printed pid=unknown.
    // -nP: PATH-less cron still works; we only need PIDs.
    let me = std::process::id().to_string();
    let mut ran = false;
    for bin in LSOF_CANDIDATES {
        if !Path::new(bin).is_file() {
            continue;
        }
        let mut cmd = Command::new(bin);
        cmd.args(["-nP", "-t"]).arg(path);
        let Some(out) = spawn_timeout(cmd, Duration::from_secs(10)) else {
            continue;
        };
        ran = true;
        let found = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::trim)
            .find(|p| !p.is_empty() && *p != me)
            .map(str::to_string);
        if let Some(pid) = found {
            let elapsed = ps_etime(&pid);
            return match elapsed {
                Some(elapsed) => ReapLockHolder::Named { pid, elapsed },
                // The holder IS named; only its age is unavailable. Say which.
                None => ReapLockHolder::Named {
                    pid,
                    elapsed: "etime_unavailable".to_string(),
                },
            };
        }
    }
    if ran {
        ReapLockHolder::NoHolder
    } else {
        ReapLockHolder::ProbeUnavailable {
            reason: format!("lsof_absent:{}", LSOF_CANDIDATES.join(",")),
        }
    }
}

fn ps_etime(pid: &str) -> Option<String> {
    if pid.is_empty() {
        return None;
    }
    let mut cmd = Command::new("ps");
    cmd.args(["-p", pid, "-o", "etime="]);
    let out = spawn_timeout(cmd, Duration::from_secs(2))?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

pub fn spawn_timeout(mut cmd: Command, timeout: Duration) -> Option<Output> {
    match subprocess_contract::bounded_output(&mut cmd, timeout) {
        BoundedOutcome::Completed(output) => Some(output),
        BoundedOutcome::TimedOut | BoundedOutcome::Unspawned(_) => None,
    }
}

pub fn lane_row_json(
    verdict: &str,
    detail: &str,
    inv: ReapFinishedPanesInvoker,
    ts: &str,
) -> String {
    serde_json::json!({
        "ts": ts,
        "event": "lane_run",
        "verdict": verdict,
        "detail": detail,
        "invoker": inv.invoker,
        "invoker_proof": inv.proof,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_zero_is_human_shell() {
        let r = ReapFinishedPanesRules::default();
        assert!(
            !is_worker_pane("0", &r),
            "rule skip_human_shell: pane 0 is never reaped"
        );
        assert!(is_worker_pane("1", &r));
        assert!(is_worker_pane("2", &r));
    }

    #[test]
    fn disabling_skip_human_admits_pane_zero() {
        let mut r = ReapFinishedPanesRules::default();
        assert!(r.disable("skip_human_shell"));
        assert!(
            is_worker_pane("0", &r),
            "mutation skip_human_shell: deleting it would reap the human shell"
        );
    }

    #[test]
    fn zero_deadline_counts_remainder() {
        let mut stats = SweepStats::default();
        let r = ReapFinishedPanesRules::default();
        let started = Instant::now() - Duration::from_secs(5);
        assert!(apply_deadline(
            &mut stats,
            started,
            Duration::from_secs(0),
            &r
        ));
        assert_eq!(stats.deadline_hit, 1);
        assert_eq!(stats.unswept, 1);
        apply_deadline(&mut stats, started, Duration::from_secs(0), &r);
        assert_eq!(stats.unswept, 2, "rule deadline_reports_unswept");
    }

    #[test]
    fn disabling_deadline_does_not_count_unswept() {
        let mut r = ReapFinishedPanesRules::default();
        assert!(r.disable("deadline_reports_unswept"));
        let mut stats = SweepStats::default();
        assert!(!apply_deadline(
            &mut stats,
            Instant::now() - Duration::from_secs(9),
            Duration::from_secs(0),
            &r
        ));
        assert_eq!(stats.unswept, 0);
    }

    #[test]
    fn invoker_uid_forgery_refused() {
        let chain = parse_ancestor_rows("501 1 /usr/sbin/cron\n");
        assert_eq!(invoker_from_chain(&chain), ReapFinishedPanesInvoker::MANUAL);
    }

    #[test]
    fn invoker_genuine_cron_certifies() {
        let chain = parse_ancestor_rows("501 233 /bin/sh\n0 1 /usr/sbin/cron\n");
        assert_eq!(
            invoker_from_chain(&chain),
            ReapFinishedPanesInvoker::SCHEDULED
        );
    }

    #[test]
    fn reaped_line_counts_awaiting() {
        let (k, await_h) = parse_reaper_out("REAPED pane=2 awaiting_human=1", true);
        assert_eq!(k, "reaped");
        assert!(await_h);
        let (k2, _) = parse_reaper_out("skip not finished", true);
        assert_eq!(k2, "skipped");
    }

    #[test]
    fn a_denied_probe_is_a_distinct_verdict_from_an_empty_one() {
        // "nobody holds it" and "I could not look" must not share a token.
        let named = ReapLockHolder::Named {
            pid: "4242".into(),
            elapsed: "22:22".into(),
        };
        let none = ReapLockHolder::NoHolder;
        let denied = ReapLockHolder::ProbeUnavailable {
            reason: format!("lsof_absent:{}", LSOF_CANDIDATES.join(",")),
        };
        assert_eq!(named.state(), "named");
        assert_eq!(none.state(), "none");
        assert_eq!(denied.state(), "unprobed");
        assert_ne!(none.state(), denied.state());
        assert_ne!(none.detail(), denied.detail());
        for holder in [&named, &none, &denied] {
            assert!(
                !holder.detail().contains("unknown"),
                "rule lock_names_holder: no verdict may render as unknown, got {}",
                holder.detail()
            );
        }
        assert!(denied.detail().contains("/usr/bin/lsof"), "{}", denied.detail());
        match (ReapFinishedPanesLockOutcome::Busy { holder: named }) {
            ReapFinishedPanesLockOutcome::Busy { holder } => {
                assert_eq!(holder.detail(), "pid=4242 elapsed=22:22")
            }
            ReapFinishedPanesLockOutcome::Acquired(_)
            | ReapFinishedPanesLockOutcome::Unusable { .. } => panic!("expected Busy"),
        }
    }

    #[test]
    fn spawn_timeout_kills_a_hung_child() {
        let mut cmd = Command::new("sleep");
        cmd.arg("30");
        let start = Instant::now();
        let out = spawn_timeout(cmd, Duration::from_millis(250));
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "rule bounded_waits: a hung child must not be waited on unbounded, elapsed={:?}",
            start.elapsed()
        );
        assert!(
            out.is_none(),
            "rule bounded_waits: timeout path is restrictive and returns no output"
        );
    }

    #[test]
    fn spawn_timeout_child_does_not_inherit_our_file_fd() {
        use std::os::unix::io::AsRawFd;
        let dir = std::env::temp_dir().join(format!("reap-fd-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let held = dir.join("held");
        let guard = std::fs::File::create(&held).expect("held file");
        let fd = guard.as_raw_fd();
        let mut cmd = Command::new("cat");
        cmd.arg(format!("/dev/fd/{fd}"));
        let out = spawn_timeout(cmd, Duration::from_secs(2)).expect("cat open-fd");
        assert!(
            !out.status.success(),
            "rule lock_not_inheritable: child opened our File fd {fd} (inherited, not CLOEXEC)"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn crate_does_not_widen_admission() {
        let main = include_str!("main.rs");
        let token = format!("{}_{}", "ADMISSION", "FRESH");
        assert!(
            !main.contains(&token),
            "rule no_widened_admission: the reaper does not own the standing verdict window"
        );
    }

    #[test]
    fn crate_does_not_rank_on_ntm_classifier_labels() {
        let lib = include_str!("lib.rs");
        let main = include_str!("main.rs");
        let needle_a = format!("{}{}", "robot", "-activity");
        let needle_b = format!("{}{}", "safe_to", "_dispatch");
        for src in [lib, main] {
            assert!(
                !src.contains(&needle_a) && !src.contains(&needle_b),
                "rule no_classifier_as_truth: reap enumerates tmux panes, never ntm labels"
            );
        }
    }

    #[test]
    fn crate_does_not_default_to_a_sibling_repo() {
        let lib = include_str!("lib.rs");
        let main = include_str!("main.rs");
        // The needle is assembled by `concat!` so this guard never contains the
        // contiguous home literal it exists to forbid (omp-orchestrator-npq).
        let home_prefix = concat!("/Users/", "josh", "/Developer/");
        for src in [lib, main] {
            for other in ["franken-harvest", "clutterfreespaces", "foundry"] {
                assert!(
                    !src.contains(&format!("{home_prefix}{other}")),
                    "rule no_cross_repo_default: found {other}"
                );
            }
        }
    }

    #[test]
    fn every_named_rule_is_disableable() {
        assert!(!ReapFinishedPanesRule::ALL.is_empty());
        for rule in ReapFinishedPanesRule::ALL {
            let mut g = ReapFinishedPanesRules::default();
            assert!(g.disable(rule.as_str()), "{}", rule.as_str());
        }
    }

    #[test]
    fn foreign_session_panes_are_excluded_from_the_sweep() {
        let mixed = vec![
            ("control-plane".into(), "1".into()),
            ("omp-orchestrator".into(), "3".into()),
            ("control-plane".into(), "4".into()),
            ("omp-orchestrator".into(), "8".into()),
        ];
        let scoped = filter_panes_to_session(&mixed, "omp-orchestrator");
        let stolen = foreign_sessions_in(&scoped, "omp-orchestrator");
        assert!(
            stolen.is_empty(),
            "KNOWN-BAD: a foreign pane remaining after filter is a stolen sweep: {stolen:?}"
        );
        assert_eq!(
            scoped,
            vec![
                ("omp-orchestrator".into(), "3".into()),
                ("omp-orchestrator".into(), "8".into()),
            ]
        );
        assert!(
            !foreign_sessions_in(&mixed, "omp-orchestrator").is_empty(),
            "the mixed set must contain foreign panes or the known-bad leg is vacuous"
        );
    }

    #[test]
    fn this_session_panes_are_kept() {
        let ours = vec![
            ("omp-orchestrator".into(), "5".into()),
            ("omp-orchestrator".into(), "9".into()),
        ];
        let scoped = filter_panes_to_session(&ours, "omp-orchestrator");
        assert_eq!(scoped, ours, "KNOWN-GOOD: in-session panes must be swept");
        require_panes(&scoped).expect("in-session set is not empty");
    }

    #[test]
    fn empty_in_scope_pane_set_is_an_error() {
        let mixed = vec![("control-plane".into(), "1".into())];
        let scoped = filter_panes_to_session(&mixed, "omp-orchestrator");
        assert!(
            require_panes(&scoped).is_err(),
            "ANTI-VACUITY: empty in-scope set must refuse, never nothing-to-reap"
        );
    }

    #[test]
    fn missing_session_refusal_is_typed() {
        assert!(MISSING_SESSION_REFUSAL.starts_with("SCOPE_REFUSED"));
        assert!(MISSING_SESSION_REFUSAL.contains("MISSING_SESSION"));
        assert!(!MISSING_SESSION_REFUSAL.contains("control-plane"));
    }

    /// Two repos must not share a write dir. Paths are DELIBERATELY not `$HOME`-rooted.
    ///
    /// This test previously used two `$HOME`-rooted absolute repo paths, which made
    /// `path-literal-guard`'s `zero_home_path_literals_across_crates_src` leg RED
    /// repo-wide — a real current-tree failure attributed here by pane `%7` during a
    /// grading rerun, not a defect in the crate it was grading. The invariant needs two
    /// DISTINCT repo paths with distinct basenames; it never needed real ones.
    ///
    /// AND MY FIRST FIX FAILED FOR THE FUNNIEST REASON AVAILABLE: this comment quoted the
    /// offending literals verbatim while explaining why they were removed, so the gate
    /// stayed RED on the warning itself. That is `AGENTS.md`'s recorded seventh instance
    /// of a doc comment containing the needle it warns about. The gate does not strip
    /// comments before matching, which is a known gap; the right move here was to stop
    /// writing the literal, not to widen the gate.
    #[test]
    fn artifact_dir_separates_repos() {
        let base = PathBuf::from("/tmp/reaped-base");
        let a = scoped_artifact_dir(&base, Path::new("/src/repos/omp-orchestrator"));
        let b = scoped_artifact_dir(&base, Path::new("/src/repos/control-plane"));
        assert_ne!(a, b, "two repos must not share a write dir");
        assert!(a.ends_with("omp-orchestrator"));
        assert!(b.ends_with("control-plane"));
    }
}

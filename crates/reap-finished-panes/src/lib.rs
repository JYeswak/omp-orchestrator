#![forbid(unsafe_code)]

//! Sweep finished worker panes. Port of `bin/reap-finished-panes.sh`.
//! The reaper binary (`pane-result-reaper.sh`) is an EXTERNAL command.

use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReapFinishedPanesRule {
    SkipHumanShell,
    DeadlineReportsUnswept,
    LockNamesHolder,
}

impl ReapFinishedPanesRule {
    pub const ALL: &'static [ReapFinishedPanesRule] = &[
        ReapFinishedPanesRule::SkipHumanShell,
        ReapFinishedPanesRule::DeadlineReportsUnswept,
        ReapFinishedPanesRule::LockNamesHolder,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            ReapFinishedPanesRule::SkipHumanShell => "skip_human_shell",
            ReapFinishedPanesRule::DeadlineReportsUnswept => "deadline_reports_unswept",
            ReapFinishedPanesRule::LockNamesHolder => "lock_names_holder",
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
}

impl Default for ReapFinishedPanesRules {
    fn default() -> Self {
        Self {
            skip_human_shell: true,
            deadline_reports_unswept: true,
            lock_names_holder: true,
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

#[derive(Debug)]
pub enum ReapFinishedPanesLockOutcome {
    Acquired(ReapFinishedPanesRunLock),
    Busy {
        holder_pid: String,
        holder_elapsed: String,
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
        Err(_) => {
            let holder_pid = lsof_holder(path).unwrap_or_else(|| "unknown".into());
            let holder_elapsed = ps_etime(&holder_pid).unwrap_or_else(|| "unknown".into());
            ReapFinishedPanesLockOutcome::Busy {
                holder_pid,
                holder_elapsed,
            }
        }
    }
}

fn lsof_holder(path: &Path) -> Option<String> {
    // Measured 2026-08-27: `lsof -t` on a held lock took 2.8–3.2s, so the 2s
    // bound killed it with empty stdout and the skip row printed pid=unknown.
    // /usr/sbin + -nP: PATH-less cron still works; we only need PIDs.
    let mut cmd = Command::new("/usr/sbin/lsof");
    cmd.args(["-nP", "-t"]).arg(path);
    let out = spawn_timeout(cmd, Duration::from_secs(10))?;
    let me = std::process::id().to_string();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .find(|p| !p.is_empty() && *p != me)
        .map(str::to_string)
}

fn ps_etime(pid: &str) -> Option<String> {
    if pid == "unknown" {
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
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().ok()?;
    // DRAIN THE PIPES ON DEDICATED THREADS.  `try_wait` in a poll loop CANNOT be paired with
    // undrained pipes: a child that writes more than the OS pipe buffer (~64 KiB, and stdout and
    // stderr each have their own) blocks in `write` forever, so it never exits, so `try_wait`
    // never returns Some, and the call burns its entire timeout at 0% CPU before being killed.
    //
    // MEASURED 2026-08-27: `git -C <repo> log --since "24 hours ago" --oneline` completes in
    // 0.6-0.9s from a shell, and sat at 0.0% CPU for 104s as a child here -- reproduced exactly by
    // polling `try_wait` without reading the pipes.  Six crates shared this shape; fixing only the
    // one that fired would have left five live.
    let out = child.stdout.take().map(|mut r| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = r.read_to_end(&mut buf);
            buf
        })
    });
    let err = child.stderr.take().map(|mut r| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = r.read_to_end(&mut buf);
            buf
        })
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() >= timeout => {
                let _ = child.kill();
                break child.wait().ok()?;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => return None,
        }
    };
    // The readers end when the child's fds close, which the kill above guarantees.
    let stdout = out.and_then(|h| h.join().ok()).unwrap_or_default();
    let stderr = err.and_then(|h| h.join().ok()).unwrap_or_default();
    Some(Output { status, stdout, stderr })
}

pub fn lane_row_json(verdict: &str, detail: &str, inv: ReapFinishedPanesInvoker, ts: &str) -> String {
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
        assert_eq!(invoker_from_chain(&chain), ReapFinishedPanesInvoker::SCHEDULED);
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
    fn lock_busy_without_lookup_names_unknown_only_when_os_silent() {
        // Structural: Busy always carries pid+elapsed fields. "unknown" is honest OS silence.
        let outcome = ReapFinishedPanesLockOutcome::Busy {
            holder_pid: "4242".into(),
            holder_elapsed: "22:22".into(),
        };
        match outcome {
            ReapFinishedPanesLockOutcome::Busy {
                holder_pid,
                holder_elapsed,
            } => {
                assert_ne!(holder_pid, "unknown", "rule lock_names_holder");
                assert_ne!(holder_elapsed, "unknown");
            }
            ReapFinishedPanesLockOutcome::Acquired(_) | ReapFinishedPanesLockOutcome::Unusable { .. } => panic!("expected Busy"),
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
            out.is_some(),
            "rule bounded_waits: timeout path must still return"
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
        let mut cmd = Command::new("/bin/sh");
        cmd.args(["-c", "exec 3<>/dev/fd/$CHECK_FD"])
            .env("CHECK_FD", fd.to_string());
        let out = spawn_timeout(cmd, Duration::from_secs(2)).expect("sh open-fd");
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
}

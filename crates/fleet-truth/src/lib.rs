#![forbid(unsafe_code)]

//! Ground-truth fleet inspection register, ported from `bin/fleet-truth.sh`.
//!
//! Ranks INSPECTION priority on git/bead/fleet-ops facts, never classifier state.
//! Identity-unknown ranks 999 and is never silently dropped.
//! The shell file is the differential oracle and is not edited by this crate.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Spawn a child with stdin=null (O_CLOEXEC on every other fd) and an explicit deadline.
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
    Some(Output {
        status,
        stdout,
        stderr,
    })
}

/// fh C75: one enum is the authority for mutation-rule names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FleetTruthRule {
    IdentityUnknownRanksHigh,
    GroundTruthNotClassifier,
    ZeroCommitsInspectFirst,
}

impl FleetTruthRule {
    pub const ALL: &'static [FleetTruthRule] = &[
        FleetTruthRule::IdentityUnknownRanksHigh,
        FleetTruthRule::GroundTruthNotClassifier,
        FleetTruthRule::ZeroCommitsInspectFirst,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            FleetTruthRule::IdentityUnknownRanksHigh => "identity_unknown_ranks_high",
            FleetTruthRule::GroundTruthNotClassifier => "ground_truth_not_classifier",
            FleetTruthRule::ZeroCommitsInspectFirst => "zero_commits_inspect_first",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|r| r.as_str() == name)
    }
}

#[derive(Clone, Debug)]
pub struct FleetTruthRules {
    pub identity_unknown_ranks_high: bool,
    pub ground_truth_not_classifier: bool,
    pub zero_commits_inspect_first: bool,
}

impl Default for FleetTruthRules {
    fn default() -> Self {
        Self {
            identity_unknown_ranks_high: true,
            ground_truth_not_classifier: true,
            zero_commits_inspect_first: true,
        }
    }
}

impl FleetTruthRules {
    pub fn disable(&mut self, name: &str) -> bool {
        let Some(rule) = FleetTruthRule::parse(name) else {
            return false;
        };
        match rule {
            FleetTruthRule::IdentityUnknownRanksHigh => self.identity_unknown_ranks_high = false,
            FleetTruthRule::GroundTruthNotClassifier => self.ground_truth_not_classifier = false,
            FleetTruthRule::ZeroCommitsInspectFirst => self.zero_commits_inspect_first = false,
        }
        true
    }

    pub fn known_names_csv() -> String {
        FleetTruthRule::ALL
            .iter()
            .map(|r| r.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Clone, Debug)]
pub struct Sensors {
    pub session: String,
    pub repo: String,
    pub vstate: String,
    pub commits: i64,
    pub dirty: i64,
    pub behind: i64,
    pub ctx: String,
    pub bclose: String,
    pub save_age: String,
    pub save_alert: String,
    /// Classifier label, if any. Ignored unless ground_truth_not_classifier is disabled.
    pub ntm_state: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TruthRow {
    pub score: i64,
    pub session: String,
    pub repo: String,
    pub commits: String,
    pub last_bead_close: String,
    pub dirty: String,
    pub behind: String,
    pub ctx: String,
    pub save_age: String,
    pub save_alert: String,
    pub reason: String,
}

impl TruthRow {
    pub fn pipe_line(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.score,
            self.session,
            self.repo,
            self.commits,
            self.last_bead_close,
            self.dirty,
            self.behind,
            self.ctx,
            self.save_age,
            self.save_alert,
            self.reason
        )
    }

    pub fn field_count(&self) -> usize {
        11
    }
}

pub fn truth_row(s: &Sensors, rules: &FleetTruthRules) -> TruthRow {
    if s.vstate != "OK" {
        let score = if rules.identity_unknown_ranks_high {
            999
        } else {
            0
        };
        return TruthRow {
            score,
            session: s.session.clone(),
            repo: s.repo.clone(),
            commits: "?".into(),
            last_bead_close: "?".into(),
            dirty: "?".into(),
            behind: "?".into(),
            ctx: "?".into(),
            save_age: "UNKNOWN".into(),
            save_alert: "identity_unknown".into(),
            reason: format!("{} (fail-closed: rank-high, inspect)", s.vstate),
        };
    }

    // Mutation: if the ground-truth check is deleted, a classifier IDLE/FREE
    // label would falsely look healthy. That is the known-bad.
    if !rules.ground_truth_not_classifier {
        let st = s.ntm_state.to_ascii_lowercase();
        if st == "idle" || st == "free" || st == "waiting" {
            return TruthRow {
                score: 0,
                session: s.session.clone(),
                repo: s.repo.clone(),
                commits: s.commits.to_string(),
                last_bead_close: s.bclose.clone(),
                dirty: s.dirty.to_string(),
                behind: s.behind.to_string(),
                ctx: s.ctx.clone(),
                save_age: s.save_age.clone(),
                save_alert: s.save_alert.clone(),
                reason: "classifier-healthy (mutation)".into(),
            };
        }
    }

    let mut score = 0i64;
    let mut reason = if rules.zero_commits_inspect_first && s.commits == 0 {
        score += 100;
        "0 commits in window".to_string()
    } else {
        format!("shipping ({} commits)", s.commits)
    };
    if s.dirty >= 50 && s.commits == 0 {
        score += 40;
        reason = format!("{reason}; {} dirty undelivered", s.dirty);
    }
    if s.behind >= 20 {
        score += 30;
        reason = format!("{reason}; behind {}", s.behind);
    }
    if s.ctx != "UNKNOWN" {
        if let Ok(ctx) = s.ctx.parse::<f64>() {
            if ctx >= 85.0 {
                score += 25;
                reason = format!("{reason}; ctx {}%", s.ctx);
            }
        }
    }
    if s.bclose == "UNKNOWN" {
        score += 15;
        reason = format!("{reason}; UNKNOWN bead-db");
    }
    match s.save_alert.as_str() {
        "ok" => {}
        a if a.starts_with("not_on_main:") => {
            score += 30;
            reason = format!("{reason}; fleet-ops {a}");
        }
        a if a.starts_with("save_stale:") => {
            score += 20;
            reason = format!("{reason}; fleet-ops {a}");
        }
        "save_unknown" => {
            score += 10;
            reason = format!("{reason}; fleet-ops save_unknown");
        }
        _ => {}
    }

    TruthRow {
        score,
        session: s.session.clone(),
        repo: s.repo.clone(),
        commits: s.commits.to_string(),
        last_bead_close: s.bclose.clone(),
        dirty: s.dirty.to_string(),
        behind: s.behind.to_string(),
        ctx: s.ctx.clone(),
        save_age: s.save_age.clone(),
        save_alert: s.save_alert.clone(),
        reason,
    }
}

pub fn fleet_ops_alert(branch: &str, save_age: &str, stale_hours: f64) -> String {
    if branch != "main" {
        return format!("not_on_main:{branch}");
    }
    if save_age == "UNKNOWN" {
        return "save_unknown".into();
    }
    match save_age.parse::<f64>() {
        Ok(age) if age > stale_hours => format!("save_stale:{age}h"),
        Ok(_) => "ok".into(),
        Err(_) => "save_unknown".into(),
    }
}

pub fn parse_behind(status_sb_first_line: &str) -> i64 {
    // git status -sb: "## main...origin/main [behind 42]"
    let Some(idx) = status_sb_first_line.find("behind ") else {
        return 0;
    };
    let rest = &status_sb_first_line[idx + "behind ".len()..];
    rest.chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

pub fn parse_completed_ts_utc(ts: &str) -> Option<i64> {
    let ts = ts.split('.').next().unwrap_or(ts);
    let ts = ts.trim_end_matches('Z');
    let ts = format!("{ts}Z");
    chrono::NaiveDateTime::parse_from_str(&ts, "%Y-%m-%dT%H:%M:%SZ")
        .ok()
        .map(|n| n.and_utc().timestamp())
}

pub fn hours_ago(ts: &str, now: i64) -> Option<f64> {
    let then = parse_completed_ts_utc(ts)?;
    Some((now - then) as f64 / 3600.0)
}

pub fn last_save_age_hours(ledger_text: &str, repo: &str, now: i64) -> String {
    let mut last: Option<String> = None;
    for line in ledger_text.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v.get("repo").and_then(|x| x.as_str()) != Some(repo) {
            continue;
        }
        let action = v.get("action").and_then(|x| x.as_str()).unwrap_or("");
        let result = v.get("result").and_then(|x| x.as_str()).unwrap_or("");
        let ok = (action == "commit" && result == "committed")
            || (action == "skip" && result == "clean");
        if !ok {
            continue;
        }
        if let Some(utc) = v.get("utc").and_then(|x| x.as_str()) {
            last = Some(utc.to_string());
        }
    }
    match last {
        Some(ts) => hours_ago(&ts, now)
            .map(|h| format!("{h:.1}"))
            .unwrap_or_else(|| "UNKNOWN".into()),
        None => "UNKNOWN".into(),
    }
}

pub fn repo_has_git(path: &Path) -> bool {
    // Match the shell oracle: `[ ! -d "$repo/.git" ]`. A `.git` *file* (worktree)
    // is UNKNOWN here, same as the shell. Worktrees are forbidden on this fleet.
    path.join(".git").is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Sensors {
        Sensors {
            session: "s".into(),
            repo: "/r".into(),
            vstate: "OK".into(),
            commits: 0,
            dirty: 0,
            behind: 0,
            ctx: "UNKNOWN".into(),
            bclose: "UNKNOWN".into(),
            save_age: "UNKNOWN".into(),
            save_alert: "save_unknown".into(),
            ntm_state: "ERROR".into(),
        }
    }

    #[test]
    fn identity_unknown_ranks_999() {
        let mut s = base();
        s.vstate = "UNKNOWN:no-git-at-cwd".into();
        let row = truth_row(&s, &FleetTruthRules::default());
        assert_eq!(
            row.score, 999,
            "rule identity_unknown_ranks_high: unobservable identity ranks-high, never a false PASS"
        );
        assert_eq!(row.field_count(), 11);
        assert_eq!(row.save_alert, "identity_unknown");
    }

    #[test]
    fn stale_outranks_healthy() {
        let stale = truth_row(
            &Sensors {
                dirty: 60,
                bclose: "UNKNOWN".into(),
                save_alert: "save_unknown".into(),
                ntm_state: "BUSY".into(),
                ..base()
            },
            &FleetTruthRules::default(),
        );
        let healthy = truth_row(
            &Sensors {
                commits: 1,
                dirty: 0,
                bclose: "2026-08-26T00:00:00Z".into(),
                save_alert: "ok".into(),
                save_age: "0.1".into(),
                ntm_state: "ERROR".into(),
                ..base()
            },
            &FleetTruthRules::default(),
        );
        assert!(
            stale.score > healthy.score,
            "stale {} vs healthy {}",
            stale.score,
            healthy.score
        );
        assert_eq!(
            healthy.score, 0,
            "classifier ERROR on a shipping repo must not raise inspection rank"
        );
    }

    #[test]
    fn classifier_error_does_not_establish_state() {
        let s = Sensors {
            ntm_state: "ERROR".into(),
            commits: 5,
            bclose: "2026-08-01T00:00:00Z".into(),
            save_alert: "ok".into(),
            ..base()
        };
        let row = truth_row(&s, &FleetTruthRules::default());
        assert_eq!(
            row.reason, "shipping (5 commits)",
            "rule ground_truth_not_classifier: ntm ERROR label alone never establishes inspection state"
        );
    }

    #[test]
    fn disabling_identity_unknown_false_passes() {
        let mut r = FleetTruthRules::default();
        assert!(r.disable("identity_unknown_ranks_high"));
        let mut s = base();
        s.vstate = "UNKNOWN:no-git-at-cwd".into();
        let row = truth_row(&s, &r);
        assert_eq!(row.score, 0);
    }

    #[test]
    fn disabling_ground_truth_trusts_classifier_idle() {
        let mut r = FleetTruthRules::default();
        assert!(r.disable("ground_truth_not_classifier"));
        let s = Sensors {
            ntm_state: "idle".into(),
            commits: 0,
            dirty: 60,
            ..base()
        };
        let row = truth_row(&s, &r);
        assert_eq!(
            row.score, 0,
            "mutation ground_truth_not_classifier: disabling it treats classifier idle as healthy"
        );
    }

    #[test]
    fn parse_behind_extracts_count() {
        assert_eq!(parse_behind("## main...origin/main [behind 42]"), 42);
        assert_eq!(parse_behind("## main"), 0);
    }

    #[test]
    fn every_named_rule_is_disableable() {
        assert!(!FleetTruthRule::ALL.is_empty());
        for rule in FleetTruthRule::ALL {
            let mut g = FleetTruthRules::default();
            assert!(g.disable(rule.as_str()));
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
        let dir = std::env::temp_dir().join(format!("ft-fd-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let held = dir.join("held");
        let guard = std::fs::File::create(&held).expect("held file");
        let fd = guard.as_raw_fd();
        // macOS `ls /dev/fd` lists extra dirents even for CLOEXEC fds (measured);
        // open of the raw fd is the real inherit check.
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
    fn crate_takes_no_run_lock_so_never_emits_unknown_holder() {
        let main = include_str!("main.rs");
        let token = format!("{}_{}", "holder", "pid");
        assert!(
            !main.contains(&token),
            "rule names_its_blocker: this observer takes no lock and must not emit holder_pid=unknown"
        );
    }

    #[test]
    fn crate_does_not_widen_admission() {
        let main = include_str!("main.rs");
        let token = format!("{}_{}", "ADMISSION", "FRESH");
        assert!(
            !main.contains(&token),
            "rule no_widened_admission: observers do not own the standing verdict window"
        );
    }

    #[test]
    fn crate_does_not_claim_liveness_from_a_capture() {
        let main = include_str!("main.rs");
        let live = format!("{}{}", "Working", " (");
        let worked = format!("{}{}", "Worked", " for");
        assert!(
            !main.contains(&live) && !main.contains(&worked),
            "rule no_single_capture_liveness: this crate never classifies pane liveness"
        );
    }

    #[test]
    fn pipe_line_does_not_truncate_fields() {
        let row = TruthRow {
            score: 1,
            session: "s".into(),
            repo: "/r".into(),
            commits: "0".into(),
            last_bead_close: "2026-08-26T11:12:44.572009Z-LONG".into(),
            dirty: "1".into(),
            behind: "0".into(),
            ctx: "10.0".into(),
            save_age: "0.1".into(),
            save_alert: "save_stale:30.0h-extra".into(),
            reason: "shipping".into(),
        };
        assert!(
            row.pipe_line().contains("2026-08-26T11:12:44.572009Z-LONG"),
            "rule no_silent_truncation: machine row must carry the full bead-close, not a cut"
        );
        assert!(row.pipe_line().contains("save_stale:30.0h-extra"));
    }
}

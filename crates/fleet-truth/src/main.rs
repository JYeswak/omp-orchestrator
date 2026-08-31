#![forbid(unsafe_code)]

//! Live fleet-truth binary. Verdicts to STDOUT column 0. stderr is usage only.
//! Every child is spawned with stdin=null and an explicit deadline.

use fleet_truth::{
    fleet_ops_alert, last_save_age_hours, parse_behind, repo_has_git, spawn_timeout, truth_row,
    Rules, Sensors, TruthRow,
};
use serde_json::{json, Value};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// How long an observation child (ntm, tmux, git) may take before it is killed.
///
/// LOAD-AWARE FOR THE SAME MEASURED REASON AS fleet-reconcile's observe_timeout().
/// MEASURED 2026-08-26: `ntm --robot-snapshot` took 12s at load 43 on this shared box, and load
/// ran 100+ for most of that day; fleet-truth itself timed out at 200s wall while its children
/// were killed at a fixed 30s. Both binaries gate controller-tick's OBSERVE step, so when either
/// times out the tick logs `OBSERVE FAIL-CLOSED — verdict=FAIL; no dispatches`, the standing
/// verdict freezes, and the whole fleet loop stops dispatching -- while ntm and tmux agree
/// perfectly by hand. A fixed child bound makes the loop's liveness a function of who else is
/// building on this machine.
///
/// Floor 30s preserves today's behaviour on a quiet box; +2s per unit of 1-minute load; capped
/// at 120s so a genuinely hung child is still killed. An unreadable load falls back to 0, which
/// only fails to WIDEN -- it never silently narrows. FLEET_TRUTH_CHILD_TIMEOUT overrides.
fn child_timeout() -> Duration {
    if let Ok(v) = std::env::var("FLEET_TRUTH_CHILD_TIMEOUT") {
        if let Ok(secs) = v.parse::<u64>() {
            return Duration::from_secs(secs.clamp(1, 600));
        }
    }
    let load = (|| -> Option<f64> {
        let out = Command::new("/usr/bin/uptime").output().ok()?;
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        let tail = text.rsplit("load averages:").next()?.trim().to_string();
        tail.split_whitespace().next()?.parse::<f64>().ok()
    })()
    .unwrap_or(0.0);
    Duration::from_secs(30u64.saturating_add((load * 2.0) as u64).min(120))
}

fn stdout_of(cmd: Command) -> String {
    spawn_timeout(cmd, child_timeout())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

fn say(line: &str) {
    println!("{line}");
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn derive_repo(session: &str) -> (String, String) {
    let mut cmd = Command::new("tmux");
    cmd.args(["list-panes", "-t", session, "-F", "#{pane_current_path}"]);
    let text = stdout_of(cmd);
    if text.trim().is_empty() {
        return (String::new(), "UNKNOWN:no-panes".into());
    }
    let mut counts: Vec<(usize, String)> = Vec::new();
    for line in text.lines() {
        let p = line.trim();
        if p.is_empty() {
            continue;
        }
        if let Some(e) = counts.iter_mut().find(|(_, x)| x == p) {
            e.0 += 1;
        } else {
            counts.push((1, p.to_string()));
        }
    }
    counts.sort_by_key(|a| std::cmp::Reverse(a.0));
    let repo = counts.first().map(|(_, p)| p.clone()).unwrap_or_default();
    if !repo_has_git(Path::new(&repo)) {
        return (repo, "UNKNOWN:no-git-at-cwd".into());
    }
    (repo, "OK".into())
}

fn commits_in_window(repo: &str, since: &str) -> i64 {
    let mut cmd = Command::new("git");
    cmd.args(["-C", repo, "log", "--since", since, "--oneline"]);
    stdout_of(cmd)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count() as i64
}

fn dirty_count(repo: &str) -> i64 {
    let mut cmd = Command::new("git");
    cmd.args(["-C", repo, "status", "--short"]);
    stdout_of(cmd)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count() as i64
}

fn behind_count(repo: &str) -> i64 {
    let mut cmd = Command::new("git");
    cmd.args(["-C", repo, "status", "-sb"]);
    let text = stdout_of(cmd);
    parse_behind(text.lines().next().unwrap_or(""))
}

fn branch_name(repo: &str) -> String {
    let mut cmd = Command::new("git");
    cmd.args(["-C", repo, "symbolic-ref", "--quiet", "--short", "HEAD"]);
    let t = stdout_of(cmd).trim().to_string();
    if t.is_empty() {
        "DETACHED".into()
    } else {
        t
    }
}

fn last_bead_close(repo: &str) -> String {
    if !Path::new(repo).join(".beads").is_dir() {
        return "UNKNOWN".into();
    }
    let mut cmd = Command::new("br");
    cmd.args(["list", "--status", "closed", "--json"])
        .current_dir(repo);
    let text = stdout_of(cmd);
    let v: Value = match serde_json::from_str(text.trim()) {
        Ok(v) => v,
        Err(_) => return "UNKNOWN".into(),
    };
    let rows = if let Some(arr) = v.as_array() {
        arr.clone()
    } else {
        v.get("issues")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default()
    };
    let mut ts: Vec<String> = rows
        .iter()
        .filter_map(|r| {
            r.get("closed_at")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
        })
        .filter(|s| !s.is_empty())
        .collect();
    ts.sort();
    ts.pop().unwrap_or_else(|| "NONE".into())
}

fn max_context_pct(session: &str) -> String {
    let mut cmd = Command::new("ntm");
    cmd.arg(format!("--robot-context={session}"));
    let text = stdout_of(cmd);
    let v: Value = match serde_json::from_str(text.trim()) {
        Ok(v) => v,
        Err(_) => return "UNKNOWN".into(),
    };
    let ags = match v.get("agents").and_then(|x| x.as_array()) {
        Some(a) => a,
        None => return "UNKNOWN".into(),
    };
    if ags.is_empty() {
        return "UNKNOWN".into();
    }
    // Match the python oracle: `a.get('usage_percent', 0)` — missing key is 0, not skip.
    let mut max = 0.0f64;
    for a in ags {
        let p = a
            .get("usage_percent")
            .and_then(|x| x.as_f64())
            .unwrap_or(0.0);
        if p > max {
            max = p;
        }
    }
    format!("{:.1}", (max * 10.0).round() / 10.0)
}

fn sensors_for(session: &str, since: &str, ledger: &str, stale_h: f64, now: i64) -> Sensors {
    let (repo, vstate) = derive_repo(session);
    if vstate != "OK" {
        return Sensors {
            session: session.into(),
            repo,
            vstate,
            commits: 0,
            dirty: 0,
            behind: 0,
            ctx: "?".into(),
            bclose: "?".into(),
            save_age: "UNKNOWN".into(),
            save_alert: "identity_unknown".into(),
            ntm_state: String::new(),
        };
    }
    let commits = commits_in_window(&repo, since);
    let dirty = dirty_count(&repo);
    let behind = behind_count(&repo);
    let ctx = max_context_pct(session);
    let bclose = last_bead_close(&repo);
    let save_age = last_save_age_hours(ledger, &repo, now);
    let branch = branch_name(&repo);
    let save_alert = fleet_ops_alert(&branch, &save_age, stale_h);
    Sensors {
        session: session.into(),
        repo,
        vstate,
        commits,
        dirty,
        behind,
        ctx,
        bclose,
        save_age,
        save_alert,
        ntm_state: String::new(),
    }
}

fn row_to_json(r: &TruthRow) -> Value {
    json!({
        "inspect_rank_score": r.score.to_string(),
        "session": r.session,
        "repo": r.repo,
        "commits_in_window": r.commits,
        "last_bead_close": r.last_bead_close,
        "uncommitted_count": r.dirty,
        "branch_behind": r.behind,
        "max_context_pct": r.ctx,
        "fleet_ops_save_age_hours": r.save_age,
        "fleet_ops_alert": r.save_alert,
        "reason": r.reason,
    })
}

/// `$HOME`, or `None` — never an invented directory (omp-orchestrator-npq, the same
/// mechanism as omp-idle-dispatch: home-relative paths derive from the environment,
/// never a literal).
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn main() -> ExitCode {
    // Home-relative PATH/TMUX segments come from `$HOME` when set and are omitted when
    // not: omitted is a true statement (no such directory exists), never a guess.
    let path = match home_dir() {
        Some(home) => format!(
            "/opt/homebrew/bin:{}/.local/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin",
            home.display()
        ),
        None => "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin".to_owned(),
    };
    std::env::set_var("PATH", &path);
    if std::env::var("TMUX_TMPDIR").is_err() {
        if let Some(home) = home_dir() {
            std::env::set_var("TMUX_TMPDIR", home.join(".tmux-sockets"));
        }
    }

    let mut json = false;
    let mut selftest = false;
    let mut eval_row = false;
    let mut mutation = false;
    let mut disabled: Vec<String> = Vec::new();
    let mut sessions: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--json" => json = true,
            "--selftest" => selftest = true,
            "--eval-row" => eval_row = true,
            "--mutation" => mutation = true,
            "--disable-rule" => match args.next() {
                Some(v) => disabled.push(v),
                None => {
                    eprintln!("usage error: --disable-rule requires a name");
                    return ExitCode::from(2);
                }
            },
            "-h" | "--help" => {
                eprintln!("usage: fleet-truth [sessions...] [--json|--selftest|--eval-row]");
                return ExitCode::SUCCESS;
            }
            other if other.starts_with("--") => {}
            other => sessions.push(other.to_string()),
        }
    }
    if !disabled.is_empty() && !mutation {
        eprintln!("usage error: --disable-rule requires --mutation");
        return ExitCode::from(2);
    }
    let mut rules = Rules::default();
    for name in &disabled {
        if !rules.disable(name) {
            eprintln!(
                "usage error: unknown rule {name}; known: {}",
                Rules::known_names_csv()
            );
            return ExitCode::from(2);
        }
    }

    if selftest {
        return run_selftest(&rules);
    }
    if eval_row {
        return eval_row_mode(&rules);
    }
    run_register(json, &sessions, &rules)
}

fn eval_row_mode(rules: &Rules) -> ExitCode {
    let mut buf = String::new();
    let _ = io::stdin().read_to_string(&mut buf);
    let v: Value = match serde_json::from_str(buf.trim()) {
        Ok(v) => v,
        Err(_) => {
            say("FAIL fleet-truth unparseable --eval-row JSON");
            return ExitCode::from(1);
        }
    };
    let s = Sensors {
        session: v
            .get("session")
            .and_then(|x| x.as_str())
            .unwrap_or("s")
            .into(),
        repo: v
            .get("repo")
            .and_then(|x| x.as_str())
            .unwrap_or("/r")
            .into(),
        vstate: v
            .get("vstate")
            .and_then(|x| x.as_str())
            .unwrap_or("OK")
            .into(),
        commits: v.get("commits").and_then(|x| x.as_i64()).unwrap_or(0),
        dirty: v.get("dirty").and_then(|x| x.as_i64()).unwrap_or(0),
        behind: v.get("behind").and_then(|x| x.as_i64()).unwrap_or(0),
        ctx: v
            .get("ctx")
            .and_then(|x| x.as_str())
            .unwrap_or("UNKNOWN")
            .into(),
        bclose: v
            .get("bclose")
            .and_then(|x| x.as_str())
            .unwrap_or("UNKNOWN")
            .into(),
        save_age: v
            .get("save_age")
            .and_then(|x| x.as_str())
            .unwrap_or("UNKNOWN")
            .into(),
        save_alert: v
            .get("save_alert")
            .and_then(|x| x.as_str())
            .unwrap_or("save_unknown")
            .into(),
        ntm_state: v
            .get("ntm_state")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .into(),
    };
    let row = truth_row(&s, rules);
    say(&row.pipe_line());
    ExitCode::SUCCESS
}

fn run_register(json_out: bool, sessions: &[String], rules: &Rules) -> ExitCode {
    let mut tmux_v = Command::new("tmux");
    tmux_v.arg("-V");
    let tmux_ok = spawn_timeout(tmux_v, Duration::from_secs(5))
        .map(|o| o.status.success())
        .unwrap_or(false);
    let mut git_v = Command::new("git");
    git_v.arg("--version");
    let git_ok = spawn_timeout(git_v, Duration::from_secs(5))
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !tmux_ok || !git_ok {
        let mut missing = String::new();
        if !tmux_ok {
            missing.push_str(" tmux");
        }
        if !git_ok {
            missing.push_str(" git");
        }
        say(&format!(
            "DETECTOR=tools_missing FAIL not on PATH:{missing}"
        ));
        say("FAIL_MODE=fail_closed_on_missing_tools; fail_open_and_loud_on_fleet_ops_unknown");
        return ExitCode::from(1);
    }

    let since = std::env::var("FLEET_SINCE").unwrap_or_else(|_| "2 hours ago".into());
    let ledger_path = match std::env::var("FLEET_OPS_LEDGER") {
        Ok(path) => path,
        Err(_) => match home_dir() {
            Some(home) => format!(
                "{}/.local/state/zeststream-fleet-ops/ledger.ndjson",
                home.display()
            ),
            None => {
                say("DETECTOR=ledger_default FAIL reason=HOME_unset: set FLEET_OPS_LEDGER to an absolute path");
                return ExitCode::from(1);
            }
        },
    };
    let stale_h: f64 = std::env::var("FLEET_OPS_STALE_HOURS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(24.0);
    let ledger = fs_read(&ledger_path);
    let now = now_epoch();

    let sess_list: Vec<String> = if sessions.is_empty() {
        let mut cmd = Command::new("tmux");
        cmd.args(["list-sessions", "-F", "#{session_name}"]);
        let t = stdout_of(cmd);
        if t.trim().is_empty() {
            say("DETECTOR=tmux_empty_session_list (loud, not silent). Reconcile decides whether this is fail-closed.");
        }
        t.lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()
    } else {
        sessions.to_vec()
    };

    // PARALLEL ACROSS SESSIONS.  Each session's sensor sweep costs SIX subprocess spawns (four
    // git, one br, one ntm), so a serial walk of 7 sessions is 42 spawns end to end -- measured
    // 2026-08-27 at load 85, that exceeded even a 180s bound while each individual child stayed
    // well inside its own.  PER-CHILD BOUNDS COMPOUND; widening them again would only move the
    // cliff.  The sessions are independent (distinct repos, read-only sensors, no shared mutable
    // state), so the sweep is embarrassingly parallel and the fix is structural.
    //
    // Determinism is preserved: the `sort_by_key` below restores a total order over the results,
    // so output does not depend on which thread finishes first.  Threads are joined unconditionally
    // -- a panicking sensor thread degrades to that session being dropped from the register rather
    // than poisoning the whole observation.
    let mut rows: Vec<TruthRow> = std::thread::scope(|scope| {
        let handles: Vec<_> = sess_list
            .iter()
            .map(|s| {
                let (since, ledger) = (&since, &ledger);
                scope.spawn(move || truth_row(&sensors_for(s, since, ledger, stale_h, now), rules))
            })
            .collect();
        handles
            .into_iter()
            .filter_map(|h| h.join().ok())
            .collect::<Vec<_>>()
    });
    rows.sort_by_key(|a| std::cmp::Reverse(a.score));

    if json_out {
        let arr: Vec<Value> = rows.iter().map(row_to_json).collect();
        say(&serde_json::to_string_pretty(&arr).unwrap_or_else(|_| "[]".into()));
    } else {
        say("=== GROUND-TRUTH FLEET INSPECTION REGISTER (identity-verified, fail-closed) ===");
        say(&format!("since={since}  READ-ONLY. Rank = INSPECTION priority (higher=inspect first). NOT an intervention trigger."));
        say("A stale session may need ESCALATE-TO-OWNER, not a nudge (esp. a large uncommitted diff).");
        say("");
        say(&format!(
            "{:<5} {:<16} {:<8} {:<22} {:<7} {:<7} {:<7} {:<9} {:<18} {}",
            "RANK",
            "SESSION",
            "COMMITS",
            "LAST-BEAD-CLOSE",
            "DIRTY",
            "BEHIND",
            "CTX%",
            "SAVE-AGE",
            "SAVE-ALERT",
            "REASON"
        ));
        for (i, r) in rows.iter().enumerate() {
            let bc: String = r.last_bead_close.chars().take(19).collect();
            let sa: String = r.save_alert.chars().take(18).collect();
            say(&format!(
                "{:<5} {:<16} {:<8} {:<22} {:<7} {:<7} {:<7} {:<9} {:<18} {}",
                i + 1,
                r.session,
                r.commits,
                bc,
                r.dirty,
                r.behind,
                r.ctx,
                r.save_age,
                sa,
                r.reason
            ));
        }
    }
    ExitCode::SUCCESS
}

fn fs_read(p: &str) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

fn run_selftest(rules: &Rules) -> ExitCode {
    let tmp = std::env::temp_dir().join(format!("ft-selftest-{}", std::process::id()));
    let stale = tmp.join("stale");
    let healthy = tmp.join("healthy");
    let _ = std::fs::create_dir_all(&stale);
    let _ = std::fs::create_dir_all(&healthy);
    // Minimal git repos: the scoring selftest does not spawn git; it plants sensors.
    let s = truth_row(
        &Sensors {
            session: "fix-stale".into(),
            repo: stale.to_string_lossy().into(),
            vstate: "OK".into(),
            commits: 0,
            dirty: 60,
            behind: 0,
            ctx: "UNKNOWN".into(),
            bclose: "UNKNOWN".into(),
            save_age: "UNKNOWN".into(),
            save_alert: "save_unknown".into(),
            ntm_state: "BUSY".into(),
        },
        rules,
    );
    let h = truth_row(
        &Sensors {
            session: "fix-healthy".into(),
            repo: healthy.to_string_lossy().into(),
            vstate: "OK".into(),
            commits: 1,
            dirty: 0,
            behind: 0,
            ctx: "10.0".into(),
            bclose: "2026-08-26T00:00:00Z".into(),
            save_age: "0.1".into(),
            save_alert: "ok".into(),
            ntm_state: "ERROR".into(),
        },
        rules,
    );
    let fc = truth_row(
        &Sensors {
            session: "fix-nogit".into(),
            repo: "/nonexistent-repo-xyz".into(),
            vstate: "UNKNOWN:no-git-at-cwd".into(),
            commits: 0,
            dirty: 0,
            behind: 0,
            ctx: "?".into(),
            bclose: "?".into(),
            save_age: "UNKNOWN".into(),
            save_alert: "identity_unknown".into(),
            ntm_state: String::new(),
        },
        rules,
    );
    let save_alert = fleet_ops_alert("main", "0.1", 24.0);
    say(&format!(
        "SELFTEST: stale(0-commit,60-dirty,no-beads)={}  healthy(1-commit,clean,beads)={}  fail-closed(no-git)={}  fleet-ops-save-alert={}",
        s.score, h.score, fc.score, save_alert
    ));
    let mut ok = true;
    if s.score <= h.score {
        say(&format!(
            "FAIL: stale did not outrank healthy ({} vs {})",
            s.score, h.score
        ));
        ok = false;
    }
    if fc.score != 999 {
        say(&format!(
            "FAIL: fail-closed identity did not rank-high ({})",
            fc.score
        ));
        ok = false;
    }
    if fc.field_count() != 11 {
        say(&format!(
            "FAIL: fail-closed identity row emitted {} fields, expected 11",
            fc.field_count()
        ));
        ok = false;
    }
    if save_alert != "ok" {
        say(&format!(
            "FAIL: fleet-ops ledger health did not render ok ({save_alert})"
        ));
        ok = false;
    }
    let _ = std::fs::remove_dir_all(&tmp);
    if ok {
        say("SELFTEST PASS: stale outranks healthy, fail-closed identity ranks-high, and fleet-ops ledger health renders.");
        ExitCode::SUCCESS
    } else {
        let _ = std::io::stderr().flush();
        ExitCode::from(1)
    }
}

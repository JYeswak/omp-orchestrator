#![forbid(unsafe_code)]

//! Live reap-finished-panes binary. The reaping path is implemented in the Rust crate.

use reap_finished_panes::{
    acquire_lock, apply_deadline, consecutive_cycle_started_same_pid, decide_reap,
    invoker_from_chain, is_worker_pane, lane_row_json, parse_ancestor_rows, reap_pane,
    require_panes, spawn_timeout, write_reaped_result, ReapFinishedPanesLockOutcome,
    ReapFinishedPanesRules, ReapPaneDecision, ReapPaneResult, SweepStats,
};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[path = "scheduled_lane_telemetry.rs"]
mod scheduled_lane_telemetry;

fn ts() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

fn ancestry_text() -> String {
    let mut rows = String::new();
    let mut pid = std::os::unix::process::parent_id();
    for _ in 0..12 {
        if pid <= 1 {
            break;
        }
        let mut cmd = Command::new("ps");
        cmd.args(["-p", &pid.to_string(), "-o", "uid=,ppid=,comm="]);
        let Some(out) = spawn_timeout(cmd, Duration::from_secs(2)) else {
            break;
        };
        let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if line.is_empty() {
            break;
        }
        rows.push_str(&line);
        rows.push('\n');
        let Some(next) = line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u32>().ok())
        else {
            break;
        };
        pid = next;
    }
    rows
}

fn append_line(path: &Path, line: &str) {
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
    }
}

/// `$HOME/.local/state/flywheel/<name>`, or a loud typed failure — never an invented home.
fn home_state_path(name: &str) -> String {
    match std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
    {
        Some(home) => format!("{}/.local/state/flywheel/{name}", home.display()),
        None => {
            eprintln!(
                "reap-finished-panes: HOME is unset; cannot resolve the default state path for {name}; set the corresponding env override"
            );
            std::process::exit(64);
        }
    }
}

fn main() -> ExitCode {
    let _telemetry = scheduled_lane_telemetry::Run::new("reap-finished-panes");
    let path = match std::env::var_os("HOME").filter(|v| !v.is_empty()) {
        Some(home) => format!(
            "{}/.local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin",
            PathBuf::from(&home).display()
        ),
        None => "/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin".to_owned(),
    };
    std::env::set_var("PATH", &path);
    if let Some(home) = std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
    {
        std::env::set_var("TMUX_TMPDIR", home.join(".tmux-sockets"));
    }

    let mut repo: Option<PathBuf> = None;
    let mut selftest = false;
    let mut mutation = false;
    let mut disabled: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--repo" => match args.next() {
                Some(v) => repo = Some(PathBuf::from(v)),
                None => {
                    eprintln!("usage error: --repo requires a path");
                    return ExitCode::from(2);
                }
            },
            "--selftest" => selftest = true,
            "--mutation" => mutation = true,
            "--disable-rule" => match args.next() {
                Some(v) => disabled.push(v),
                None => {
                    eprintln!("usage error: --disable-rule requires a name");
                    return ExitCode::from(2);
                }
            },
            "-h" | "--help" => {
                eprintln!("usage: reap-finished-panes [--repo PATH] [--selftest]");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("usage error: unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    let repo =
        repo.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    if !repo.is_dir() {
        eprintln!("usage error: --repo is not a directory: {}", repo.display());
        return ExitCode::from(2);
    }
    let mut rules = ReapFinishedPanesRules::default();
    if !disabled.is_empty() && !mutation {
        eprintln!("usage error: --disable-rule requires --mutation");
        return ExitCode::from(2);
    }
    for name in &disabled {
        if !rules.disable(name) {
            eprintln!(
                "usage error: unknown rule {name}; known: {}",
                ReapFinishedPanesRules::known_names_csv()
            );
            return ExitCode::from(2);
        }
    }

    let ledger = if let Ok(p) = std::env::var("REAPER_LEDGER") {
        p
    } else if selftest {
        format!(
            "{}/reap-led-st-{}.jsonl",
            std::env::temp_dir().display(),
            std::process::id()
        )
    } else {
        home_state_path("pane-result-reaper.jsonl")
    };
    let lock_path = if let Ok(p) = std::env::var("REAP_SWEEP_LOCK") {
        p
    } else if selftest {
        format!(
            "{}/reap-st-{}.lock",
            std::env::temp_dir().display(),
            std::process::id()
        )
    } else {
        home_state_path("reap-sweep.lock")
    };
    let apply = std::env::var("REAP_APPLY").unwrap_or_else(|_| "1".into()) == "1";
    let lane_ledger = if selftest {
        std::env::var("REAP_LANE_LEDGER").unwrap_or_else(|_| {
            format!(
                "{}/reap-lane-{}.jsonl",
                std::env::temp_dir().display(),
                std::process::id()
            )
        })
    } else {
        std::env::var("REAP_LANE_LEDGER")
            .unwrap_or_else(|_| home_state_path("reap-finished-panes.jsonl"))
    };
    let outdir =
        PathBuf::from(std::env::var("REAPER_OUTDIR").unwrap_or_else(|_| home_state_path("reaped")));
    let lines = std::env::var("REAPER_LINES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(160usize);

    let _guard = match acquire_lock(Path::new(&lock_path)) {
        ReapFinishedPanesLockOutcome::Acquired(g) => g,
        ReapFinishedPanesLockOutcome::Busy {
            holder_pid,
            holder_elapsed,
        } => {
            let row = format!(
                r#"{{"ts":"{}","event":"sweep_skipped","reason":"another_sweep_running","holder_pid":"{holder_pid}","holder_elapsed":"{holder_elapsed}"}}"#,
                ts()
            );
            append_line(Path::new(&ledger), &row);
            let inv = invoker_from_chain(&parse_ancestor_rows(&ancestry_text()));
            append_line(
                Path::new(&lane_ledger),
                &lane_row_json("SKIPPED", "another_sweep_running", inv, &ts()),
            );
            println!("reap-finished-panes SKIPPED another_sweep_running pid={holder_pid} elapsed={holder_elapsed}");
            return ExitCode::SUCCESS;
        }
        ReapFinishedPanesLockOutcome::Unusable { reason } => {
            eprintln!("reap-finished-panes: cannot open lock {lock_path}: {reason}");
            return ExitCode::from(2);
        }
    };
    if selftest {
        return run_selftest(&rules, &repo);
    }

    let deadline = Duration::from_secs(
        std::env::var("REAP_SWEEP_DEADLINE_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(240),
    );
    let started = Instant::now();
    let mut stats = SweepStats::default();
    let panes = pane_list();
    if let Err(reason) = require_panes(&panes) {
        eprintln!("reap-finished-panes: {reason}");
        return ExitCode::from(2);
    }
    let settle = Duration::from_secs(
        std::env::var("REAPER_SETTLE_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3),
    );
    for (session, idx) in panes {
        if !is_worker_pane(&idx, &rules) {
            continue;
        }
        if apply_deadline(&mut stats, started, deadline, &rules) {
            continue;
        }
        match reap_pane(
            &session,
            &idx,
            settle,
            lines,
            apply,
            &outdir,
            Path::new(&ledger),
            &ts(),
        ) {
            ReapPaneResult::Reaped {
                path,
                awaiting_human,
                bytes,
            } => {
                stats.reaped += 1;
                if awaiting_human {
                    stats.awaiting_human += 1;
                }
                match path {
                    Some(path) => println!("REAPED {session}:{idx} -> {} (awaiting_human={awaiting_human}, bytes={bytes})", path.display()),
                    None => println!("REPORT {session}:{idx} settled awaiting_human={awaiting_human} bytes={bytes}"),
                }
            }
            ReapPaneResult::Skipped { reason } => {
                stats.skipped += 1;
                println!("SKIPPED {session}:{idx} reason={reason}");
            }
            ReapPaneResult::Unreadable { reason } => {
                stats.skipped += 1;
                eprintln!("reap-finished-panes: {session}:{idx} unreadable: {reason}");
            }
        }
    }
    stats.elapsed_secs = started.elapsed().as_secs();
    let summary = format!(
        r#"{{"ts":"{}","event":"reap_sweep","reaped":{},"skipped":{},"awaiting_human":{},"unswept":{},"deadline_hit":{},"elapsed_secs":{}}}"#,
        ts(),
        stats.reaped,
        stats.skipped,
        stats.awaiting_human,
        stats.unswept,
        stats.deadline_hit,
        stats.elapsed_secs
    );
    append_line(Path::new(&ledger), &summary);
    println!(
        "[{}] reap sweep: reaped={} skipped={} awaiting_human={} unswept={} deadline_hit={} elapsed={}s",
        ts(),
        stats.reaped,
        stats.skipped,
        stats.awaiting_human,
        stats.unswept,
        stats.deadline_hit,
        stats.elapsed_secs
    );
    if stats.deadline_hit == 1 {
        eprintln!(
            "  WARNING: {} pane(s) unswept — sweep hit its {}s deadline",
            stats.unswept,
            deadline.as_secs()
        );
    }
    let inv = invoker_from_chain(&parse_ancestor_rows(&ancestry_text()));
    append_line(
        Path::new(&lane_ledger),
        &lane_row_json(
            "SWEPT",
            &format!(
                "reaped={} skipped={} unswept={} deadline_hit={}",
                stats.reaped, stats.skipped, stats.unswept, stats.deadline_hit
            ),
            inv,
            &ts(),
        ),
    );
    let _ = _guard;
    ExitCode::SUCCESS
}

fn pane_list() -> Vec<(String, String)> {
    if let Ok(raw) = std::env::var("REAP_PANE_LIST") {
        return raw
            .lines()
            .filter_map(|l| {
                let mut it = l.split_whitespace();
                Some((it.next()?.to_owned(), it.next()?.to_owned()))
            })
            .collect();
    }
    let mut cmd = Command::new("tmux");
    cmd.args(["list-panes", "-a", "-F", "#{session_name} #{pane_index}"]);
    spawn_timeout(cmd, Duration::from_secs(15))
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|l| {
                    let mut it = l.split_whitespace();
                    Some((it.next()?.to_owned(), it.next()?.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn run_selftest(rules: &ReapFinishedPanesRules, repo: &Path) -> ExitCode {
    let mut fail = 0;
    let root = repo.join("var/agent-tmp").join(format!(
        "reap-finished-panes-selftest-{}",
        std::process::id()
    ));
    let outdir = root.join("reaped");
    let ledger = root.join("pane-result-reaper.jsonl");
    let stamp = "2026-09-02T04:00:00Z";
    let finished_text = "finished output\nresult: committed";

    let finished = decide_reap(finished_text, finished_text, true, finished_text);
    let finished_ok = match finished {
        ReapPaneDecision::Reaped { awaiting_human } => {
            let path = write_reaped_result(
                &outdir,
                &ledger,
                "reaper-selftest",
                "1",
                "%selftest",
                finished_text,
                awaiting_human,
                stamp,
            );
            let row = path.as_ref().ok().and_then(|path| {
                let row = std::fs::read_to_string(&ledger).ok()?;
                let row: serde_json::Value = serde_json::from_str(row.trim()).ok()?;
                let expected = serde_json::json!({
                    "ts": stamp,
                    "event": "result_reaped",
                    "session": "reaper-selftest",
                    "pane": "1",
                    "pane_id": "%selftest",
                    "awaiting_human": awaiting_human,
                    "bytes": finished_text.len() + 1,
                    "path": path,
                });
                let artifact = std::fs::read_to_string(path).ok()?;
                Some(row == expected && artifact == format!("{finished_text}\n"))
            });
            row == Some(true)
        }
        other => {
            println!("FAIL selftest.finished-pane-ledger: {other:?}");
            false
        }
    };
    if finished_ok {
        println!("PASS selftest.finished-pane-ledger");
    } else {
        fail += 1;
    }

    let working = decide_reap("Working (9s)", "Working (9s)", false, "Working (9s)");
    if matches!(working, ReapPaneDecision::Working) {
        println!("PASS selftest.working-pane-not-reaped");
    } else {
        println!("FAIL selftest.working-pane-not-reaped: {working:?}");
        fail += 1;
    }
    let empty = decide_reap("", "", true, "");
    if matches!(empty, ReapPaneDecision::Empty) && require_panes::<(String, String)>(&[]).is_err() {
        println!("PASS selftest.empty-pane-set-refuses");
    } else {
        println!("FAIL selftest.empty-pane-set-refuses: {empty:?}");
        fail += 1;
    }
    let heartbeat =
        "{\"event\":\"CYCLE_STARTED\",\"pid\":4242}\n{\"event\":\"CYCLE_STARTED\",\"pid\":4242}\n";
    if consecutive_cycle_started_same_pid(heartbeat) {
        println!("PASS selftest.cycle-pid-stable");
    } else {
        println!("FAIL selftest.cycle-pid-stable");
        fail += 1;
    }
    if !is_worker_pane("0", rules) && is_worker_pane("1", rules) {
        println!("PASS selftest.skips-human-shell");
    } else {
        println!("FAIL selftest.skips-human-shell");
        fail += 1;
    }
    let _ = std::fs::remove_dir_all(&root);
    if fail == 0 {
        println!("=== SELFTEST: 0 failure(s) ===");
        ExitCode::SUCCESS
    } else {
        println!("=== SELFTEST: FAILURES ===");
        ExitCode::from(1)
    }
}

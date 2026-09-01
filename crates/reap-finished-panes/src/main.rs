#![forbid(unsafe_code)]

//! Live reap-finished-panes binary. pane-result-reaper.sh is an external command.

use reap_finished_panes::{
    acquire_lock, apply_deadline, invoker_from_chain, is_worker_pane, lane_row_json,
    parse_ancestor_rows, parse_reaper_out, spawn_timeout, ReapFinishedPanesLockOutcome, ReapFinishedPanesRules, SweepStats,
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
    // cheap UTC stamp
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

/// Marker entries that identify a repository root while walking up from the cwd.
const REPO_MARKERS: [&str; 2] = [".git", ".beads"];

/// Repository root for `bin/` helpers: `--repo` flag > `CP` env > upward `.git`/`.beads`
/// marker walk from the cwd (omp-orchestrator-npq, the omp-idle-dispatch mechanism).
/// Failures name what could not be found and the escape hatch.
fn resolve_repo_root(flag: Option<&str>) -> Result<PathBuf, String> {
    if let Some(flag) = flag {
        if flag.trim().is_empty() {
            return Err("--repo is set but empty".to_owned());
        }
        return Ok(PathBuf::from(flag));
    }
    if let Some(root) = std::env::var_os("CP").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(root));
    }
    let mut current = std::env::current_dir().map_err(|error| format!("cannot read the current directory: {error}"))?;
    loop {
        if REPO_MARKERS.iter().any(|marker| current.join(marker).exists()) {
            return Ok(current);
        }
        let Some(parent) = current.parent() else {
            return Err(format!(
                "no repository marker ({}) found at or above {}; pass --repo <PATH> or set CP",
                REPO_MARKERS.join(" or "),
                current.display()
            ));
        };
        current = parent.to_path_buf();
    }
}

/// `$HOME/.local/state/flywheel/<name>`, or a loud typed failure — never an invented home.
fn home_state_path(name: &str) -> String {
    match std::env::var_os("HOME").filter(|v| !v.is_empty()).map(PathBuf::from) {
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
    // Home-relative PATH/TMUX segments derive from `$HOME` when set and are omitted
    // when not: omitted is a true statement, never a guess (omp-orchestrator-npq).
    let path = match std::env::var_os("HOME").filter(|v| !v.is_empty()) {
        Some(home) => format!(
            "{}/.local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin",
            PathBuf::from(&home).display()
        ),
        None => "/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin".to_owned(),
    };
    std::env::set_var("PATH", &path);
    if let Some(home) = std::env::var_os("HOME").filter(|v| !v.is_empty()).map(PathBuf::from) {
        // Unconditional override, matching the original semantics exactly.
        std::env::set_var("TMUX_TMPDIR", home.join(".tmux-sockets"));
    }

    let mut selftest = false;
    let mut mutation = false;
    let mut repo_flag: Option<String> = None;
    let mut disabled: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--selftest" => selftest = true,
            "--mutation" => mutation = true,
            "--repo" => match args.next() {
                Some(v) => repo_flag = Some(v),
                None => {
                    eprintln!("usage error: --repo requires a path");
                    return ExitCode::from(2);
                }
            },
            "--disable-rule" => match args.next() {
                Some(v) => disabled.push(v),
                None => {
                    eprintln!("usage error: --disable-rule requires a name");
                    return ExitCode::from(2);
                }
            },
            "-h" | "--help" => {
                eprintln!("usage: reap-finished-panes [--selftest] [--repo <PATH>]");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("usage error: unknown argument {other}");
                return ExitCode::from(2);
            }
        }
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

    let cp = match resolve_repo_root(repo_flag.as_deref()) {
        Ok(root) => root.display().to_string(),
        Err(message) => {
            eprintln!("reap-finished-panes: {message}");
            return ExitCode::from(64);
        }
    };
    let reaper =
        std::env::var("REAPER").unwrap_or_else(|_| format!("{cp}/bin/pane-result-reaper.sh"));
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
        // cargo test / --selftest must never contend with the live */5 sweep.
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
        std::env::var("REAP_LANE_LEDGER").unwrap_or_else(|_| home_state_path("reap-finished-panes.jsonl"))
    };


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
    if !PathBuf::from(&reaper).is_file() {
        eprintln!("reap-finished-panes: missing {reaper}");
        return ExitCode::from(2);
    }

    if selftest {
        return run_selftest(&reaper, &rules);
    }

    let deadline = Duration::from_secs(
        std::env::var("REAP_SWEEP_DEADLINE_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(240),
    );
    std::env::set_var(
        "REAPER_SETTLE_SECS",
        std::env::var("REAPER_SETTLE_SECS").unwrap_or_else(|_| "3".into()),
    );

    let started = Instant::now();
    let mut stats = SweepStats::default();
    let panes = pane_list();
    for (session, idx) in panes {
        if !is_worker_pane(&idx, &rules) {
            continue;
        }
        if apply_deadline(&mut stats, started, deadline, &rules) {
            continue;
        }
        let mut cmd = Command::new(&reaper);
        cmd.arg("--session").arg(&session).arg("--pane").arg(&idx);
        if apply {
            cmd.arg("--apply");
        }
        let out = spawn_timeout(cmd, Duration::from_secs(60));
        let (text, ok) = match out {
            Some(o) => (
                String::from_utf8_lossy(&o.stdout).into_owned()
                    + &String::from_utf8_lossy(&o.stderr),
                o.status.success(),
            ),
            None => (String::new(), false),
        };
        let (kind, awaiting) = parse_reaper_out(text.trim(), ok);
        match kind {
            "reaped" => {
                stats.reaped += 1;
                if awaiting {
                    stats.awaiting_human += 1;
                }
                println!("{}", text.trim());
            }
            _ => stats.skipped += 1,
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
                Some((it.next()?.to_string(), it.next()?.to_string()))
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
                    Some((it.next()?.to_string(), it.next()?.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn run_selftest(reaper: &str, rules: &ReapFinishedPanesRules) -> ExitCode {
    let mut fail = 0;
    if PathBuf::from(reaper).is_file() {
        println!("PASS selftest.reaper-present");
    } else {
        println!("FAIL selftest.reaper-present");
        fail += 1;
    }
    let mut cmd = Command::new(reaper);
    cmd.arg("--selftest");
    match spawn_timeout(cmd, Duration::from_secs(60)) {
        Some(o) if o.status.success() => println!("PASS selftest.reaper-selftest-green"),
        _ => {
            println!("FAIL selftest.reaper-selftest-green");
            fail += 1;
        }
    }
    if !is_worker_pane("0", rules) && is_worker_pane("1", rules) {
        println!("PASS selftest.skips-human-shell");
    } else {
        println!("FAIL selftest.skips-human-shell");
        fail += 1;
    }
    // Hermetic deadline: inject panes, zero deadline, no live tmux required.
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("reap-finished-panes"));
    let lock = std::env::temp_dir().join(format!("reap-lock-st-{}", std::process::id()));
    let led = std::env::temp_dir().join(format!("reap-led-st-{}", std::process::id()));
    let lane = std::env::temp_dir().join(format!("reap-lane-st-{}", std::process::id()));
    let mut child = Command::new(&exe);
    child
        .env("REAP_SWEEP_DEADLINE_SECS", "0")
        .env("REAP_APPLY", "0")
        .env("REAP_SWEEP_LOCK", &lock)
        .env("REAPER_LEDGER", &led)
        .env("REAP_LANE_LEDGER", &lane)
        .env("REAP_PANE_LIST", "alpha 0\nalpha 1\nbeta 2\n");
    let out = spawn_timeout(child, Duration::from_secs(15));
    let text = out
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    if text.contains("deadline_hit=1") && text.contains("unswept=") && !text.contains("unswept=0") {
        println!("PASS selftest.deadline-reports-unswept (fires-on-known-bad)");
    } else {
        println!("FAIL selftest.deadline-reports-unswept out={text}");
        fail += 1;
    }
    let _ = std::fs::remove_file(&lock);
    let _ = std::fs::remove_file(&led);
    let _ = std::fs::remove_file(&lane);
    if fail == 0 {
        println!("=== SELFTEST: 0 failure(s) ===");
        ExitCode::SUCCESS
    } else {
        println!("=== SELFTEST: FAILURES ===");
        ExitCode::from(1)
    }
}

#![forbid(unsafe_code)]

//! Live fleet-reconcile binary.
//! Verdicts to STDOUT column 0 both directions. stderr is usage only.
//! Every child is spawned with stdin=null and an explicit deadline.

use fleet_reconcile::{
    classify_ft, emit_envelope, exit_for, invoker_resolve_env, parse_ancestor_rows,
    reconcile_inner, spawn_timeout, FleetReconcileRules, FAIL_MODE,
};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::Duration;

/// How long an observation child (tmux, ntm, ft) may take before it is killed.
///
/// LOAD-AWARE BECAUSE A FIXED BOUND MADE THE WHOLE FLEET LOOP DIE UNDER LOAD.
/// MEASURED 2026-08-26: `ntm --robot-snapshot` took **12s at load 43** on this box, against a
/// hard 15s bound — and load ran 100+ for most of that day. Above the bound `spawn_timeout`
/// kills the child and returns its EMPTY output, which the caller then reports as
/// `DETECTOR=ntm_snapshot_unparseable` — a claim about the DATA for what is actually a claim
/// about the CLOCK. controller-tick fail-closes on that verdict before it ever reaches its
/// publish step, so every tick logged `OBSERVE FAIL-CLOSED — verdict=FAIL; no dispatches`,
/// the standing verdict froze, and the fleet sat idle while `ntm list` and `tmux` agreed
/// perfectly at 7 sessions and the snapshot itself parsed fine by hand.
///
/// Same shape as `check.sh`'s `_stage_bound()` (600s floor + 10s per unit of 1-minute load,
/// capped): this box is shared with other projects' builds, and an observation window that
/// only holds on an idle machine is not a window. Floor 15s preserves today's behaviour on a
/// quiet box; +2s per load unit; capped at 90s so a genuinely hung child is still killed.
/// FLEET_RECONCILE_OBSERVE_TIMEOUT overrides it outright for tests.
fn observe_timeout() -> Duration {
    if let Ok(v) = std::env::var("FLEET_RECONCILE_OBSERVE_TIMEOUT") {
        if let Ok(secs) = v.parse::<u64>() {
            return Duration::from_secs(secs.clamp(1, 600));
        }
    }
    // macOS has no /proc/loadavg; `uptime` is the portable reading and is cheap. If it cannot be
    // read we fall back to load 0, i.e. today's fixed 15s floor -- an unreadable load must not
    // silently WIDEN the window, only fail to widen it.
    let load = (|| -> Option<f64> {
        let out = Command::new("/usr/bin/uptime").output().ok()?;
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        let tail = text.rsplit("load averages:").next()?.trim().to_string();
        tail.split_whitespace().next()?.parse::<f64>().ok()
    })()
    .unwrap_or(0.0);
    let secs = 15u64.saturating_add((load * 2.0) as u64).min(90);
    Duration::from_secs(secs)
}

fn say(line: &str) {
    println!("{line}");
}

fn usage() {
    eprintln!("usage: fleet-reconcile [--json|--selftest|--mutation --disable-rule NAME]");
}

fn main() -> ExitCode {
    // Home-relative PATH/TMUX segments derive from `$HOME` when set and are omitted
    // when not: omitted is a true statement, never a guess (omp-orchestrator-npq).
    let path = match std::env::var_os("HOME").filter(|v| !v.is_empty()) {
        Some(home) => format!(
            "/opt/homebrew/bin:{}/.local/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin",
            PathBuf::from(&home).display()
        ),
        None => "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin".to_owned(),
    };
    std::env::set_var("PATH", &path);
    if std::env::var("TMUX_TMPDIR").is_err() {
        if let Some(home) = std::env::var_os("HOME").filter(|v| !v.is_empty()).map(PathBuf::from) {
            std::env::set_var("TMUX_TMPDIR", home.join(".tmux-sockets"));
        }
    }

    let mut json = false;
    let mut selftest = false;
    let mut mutation = false;
    let mut disabled: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--json" => json = true,
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
                usage();
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("usage error: unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    if !disabled.is_empty() && !mutation {
        eprintln!("usage error: --disable-rule requires --mutation");
        return ExitCode::from(2);
    }
    let mut rules = FleetReconcileRules::default();
    for name in &disabled {
        if !rules.disable(name) {
            eprintln!(
                "usage error: unknown rule {name}; known: {}",
                FleetReconcileRules::known_names_csv()
            );
            return ExitCode::from(2);
        }
    }

    if selftest {
        return run_selftest(json);
    }
    run_reconcile(json, &rules)
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

fn invoker_pair() -> (String, String) {
    let inv = std::env::var("LOOP_INVOKER")
        .ok()
        .or_else(|| std::env::var("CT_INVOKER").ok())
        .unwrap_or_default();
    let proof = std::env::var("LOOP_INVOKER_PROOF")
        .ok()
        .or_else(|| std::env::var("CT_INVOKER_PROOF").ok())
        .unwrap_or_default();
    let chain = parse_ancestor_rows(&ancestry_text());
    let got = invoker_resolve_env(&inv, &proof, &chain);
    (got.invoker.to_string(), got.proof.to_string())
}

fn read_inputs() -> Result<(String, String, String, String), String> {
    if let Ok(dir) = std::env::var("FLEET_RECONCILE_FIXTURE_DIR") {
        let d = PathBuf::from(dir);
        let read = |name: &str| fs::read_to_string(d.join(name)).unwrap_or_default();
        return Ok((
            read("tmux-sessions.txt"),
            read("ntm-list.txt"),
            read("snapshot.json"),
            read("ft-state.json"),
        ));
    }
    let mut tmux = Command::new("tmux");
    tmux.args(["list-sessions", "-F", "#{session_name}"]);
    let tmux_sessions = spawn_timeout(tmux, observe_timeout())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let mut ntm_l = Command::new("ntm");
    ntm_l.arg("list");
    let ntm_list = spawn_timeout(ntm_l, observe_timeout())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let mut ntm_s = Command::new("ntm");
    ntm_s.arg("--robot-snapshot");
    let ntm_snap = spawn_timeout(ntm_s, observe_timeout())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let ft_bin = std::env::var("FT_BIN").unwrap_or_else(|_| {
        std::env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .map(|home| format!("{}/.local/bin/ft", PathBuf::from(&home).display()))
            .unwrap_or_else(|| "ft".into())
    });
    let ft_ws = std::env::var("FT_WORKSPACE").unwrap_or_else(|_| {
        std::env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .map(|home| {
                format!(
                    "{}/.local/share/frankenterm/control-plane",
                    PathBuf::from(&home).display()
                )
            })
            .unwrap_or_else(|| ".local/share/frankenterm/control-plane".into())
    });
    let ft_raw = if PathBuf::from(&ft_bin).is_file() {
        let mut ft = Command::new(&ft_bin);
        ft.args(["robot", "--format", "json", "state"])
            .env("FT_WORKSPACE", ft_ws)
            .env("RUST_LOG", "error")
            .env_remove("WEZTERM_UNIX_SOCKET");
        spawn_timeout(ft, observe_timeout())
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
    } else {
        String::new()
    };
    Ok((tmux_sessions, ntm_list, ntm_snap, ft_raw))
}

fn run_reconcile(json: bool, rules: &FleetReconcileRules) -> ExitCode {
    let (invoker, proof) = invoker_pair();
    let (tmux, list, snap, ft_raw) = match read_inputs() {
        Ok(v) => v,
        Err(e) => {
            say(&format!("FAIL fleet-reconcile {e}"));
            return ExitCode::from(1);
        }
    };
    let inner = reconcile_inner(&tmux, &list, &snap, rules);
    let ft_status = classify_ft(&ft_raw);
    let env = emit_envelope(&inner, ft_status, &invoker, &proof);
    if json {
        println!("{}", env);
    } else {
        say("=== FLEET RECONCILIATION (ntm vs tmux; FT is a separate namespace) ===");
        say(&format!("FAIL_MODE={FAIL_MODE}"));
        say(&format!(
            "DETECTOR={} VERDICT={} invoker={invoker} invoker_proof={proof} tmux={} ntm={} ft={ft_status}",
            inner.detector, inner.verdict, inner.tmux_count, inner.ntm_count
        ));
        say(&format!("detail: {}", env["detail"].as_str().unwrap_or("")));
    }
    ExitCode::from(exit_for(&inner.verdict) as u8)
}

fn write_fix(dir: &std::path::Path, tmux: &str, list: &str, snap: &str, ft: &str) {
    let _ = fs::create_dir_all(dir);
    let _ = fs::write(dir.join("tmux-sessions.txt"), tmux);
    let _ = fs::write(dir.join("ntm-list.txt"), list);
    let _ = fs::write(dir.join("snapshot.json"), snap);
    let _ = fs::write(dir.join("ft-state.json"), ft);
}

fn run_selftest(_json: bool) -> ExitCode {
    let tmp = std::env::temp_dir().join(format!("fr-selftest-{}", std::process::id()));
    let fix = tmp.join("fix");
    let _ = fs::create_dir_all(&fix);
    let mut fails = 0;
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("fleet-reconcile"));

    let mut run_leg = |name: &str, want_det: &str, want_ver: &str, want_rc: i32| {
        let mut cmd = Command::new(&exe);
        cmd.arg("--json").env("FLEET_RECONCILE_FIXTURE_DIR", &fix);
        let out = spawn_timeout(cmd, Duration::from_secs(15)).expect("selftest spawn");
        let rc = out.status.code().unwrap_or(99);
        let text = String::from_utf8_lossy(&out.stdout);
        let v: serde_json::Value =
            serde_json::from_str(text.trim()).unwrap_or(serde_json::Value::Null);
        let got_det = v.get("detector").and_then(|x| x.as_str()).unwrap_or("");
        let got_ver = v.get("verdict").and_then(|x| x.as_str()).unwrap_or("");
        let mut ok = true;
        if got_det != want_det {
            ok = false;
            say(&format!(
                "    MISSING detector: want {want_det} got {got_det}"
            ));
        }
        if got_ver != want_ver {
            ok = false;
            say(&format!("    verdict: want {want_ver} got {got_ver}"));
        }
        if rc != want_rc {
            ok = false;
            say(&format!("    rc={rc} want={want_rc} (secondary)"));
        }
        if ok {
            say(&format!("  ok   {name} detector={want_det}"));
        } else {
            say(&format!("  FAIL {name}"));
            fails += 1;
        }
    };

    write_fix(
        &fix,
        "alpha\nbeta\n",
        "  alpha: 2 panes\n  beta: 1 pane\n",
        r#"{"success":true,"summary":{"total_sessions":2},"sessions":[{"name":"alpha"},{"name":"beta"}]}"#,
        "",
    );
    run_leg(
        "known-good: ntm and tmux agree",
        "ntm_tmux_agree",
        "PASS",
        0,
    );

    write_fix(
        &fix,
        "control-plane\nalpsinsurance\n",
        "  control-plane: 3 panes\n",
        r#"{"success":true,"summary":{"total_sessions":0},"sessions":[]}"#,
        "",
    );
    run_leg(
        "known-bad: ntm empty-success with live tmux",
        "ntm_empty_success_with_live_tmux",
        "FAIL",
        1,
    );

    write_fix(
        &fix,
        "control-plane\n",
        "No tmux sessions running\n",
        r#"{"success":true,"summary":{"total_sessions":1},"sessions":[{"name":"control-plane"}]}"#,
        "",
    );
    run_leg(
        "known-bad: ntm list empty text (exit 0 cannot see this)",
        "ntm_list_empty_text",
        "FAIL",
        1,
    );

    write_fix(
        &fix,
        "alpha\nbeta\n",
        "  alpha: 1 pane\n",
        r#"{"success":true,"summary":{"total_sessions":1},"sessions":[{"name":"alpha"}]}"#,
        "",
    );
    run_leg(
        "known-bad: ntm/tmux name-set disagree",
        "ntm_tmux_disagree",
        "FAIL",
        1,
    );

    let _ = fs::remove_dir_all(&tmp);
    say("");
    if fails == 0 {
        say("SELFTEST PASS: agree verifies; empty-success, empty-text, and name-set each fire their NAMED detector.");
        ExitCode::SUCCESS
    } else {
        say(&format!("SELFTEST RED ({fails} leg(s) failed)"));
        let _ = std::io::stderr().flush();
        ExitCode::from(1)
    }
}

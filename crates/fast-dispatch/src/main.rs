#![forbid(unsafe_code)]

//! Live fast-dispatch binary.
//!
//! Verdicts go to STDOUT at column 0 in both directions. stderr is usage only.
//! Every child is spawned with stdin=null and an explicit deadline. The mkdir
//! lock lives in a value whose Drop releases it; children cannot inherit an
//! flock fd because there is no flock fd.
//!
//! WHAT IS RUST: admission, FREE-pane selection, conductor skip, session-repo
//! map, lock, bounded child runner, packet assembly, send orchestration.
//! WHAT STILL SHELLS: check.sh, pane-dispatch-ready.sh, cargo-lane-budget.sh,
//! loop-queue-filter, composer-typed.py, pane-dispatch-fence, ntm, br, tmux.

#[path = "dispatch_cli_contract.rs"]
mod dispatch_cli_contract;
use fast_dispatch::{
    admission_fresh_pass, cargo_lane_timeout_secs, classify_invoker, is_conductor_routed,
    select_free_panes, session_repo_dir, strip_ansi, wedge_reason, AdmissionConfig, FastDispatchRules,
    SelectError, CORPUS_FIRST_CONTRACT,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

#[path = "scheduled_lane_telemetry.rs"]
mod scheduled_lane_telemetry;

/// Repository root: `FD_REPO` env > upward `.git`/`.beads` marker walk from the cwd
/// (omp-orchestrator-npq, the omp-idle-dispatch mechanism). Never a hardcoded
/// checkout — a wrong root compiles fine and then silently runs the wrong repo's
/// scripts.
static CP_ROOT: std::sync::LazyLock<Result<PathBuf, String>> = std::sync::LazyLock::new(resolve_repo_root);

/// Loud accessor: resolution failure prints the typed message naming the markers,
/// the searched directory, and the escape hatch, then exits 64.
fn cp() -> PathBuf {
    match &*CP_ROOT {
        Ok(root) => root.clone(),
        Err(message) => {
            eprintln!("fast-dispatch: {message}");
            std::process::exit(64);
        }
    }
}

fn resolve_repo_root() -> Result<PathBuf, String> {
    if let Some(root) = std::env::var_os("FD_REPO").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(root));
    }
    let mut current = std::env::current_dir()
        .map_err(|error| format!("cannot read the current directory: {error}"))?;
    loop {
        if [".git", ".beads"].iter().any(|marker| current.join(marker).exists()) {
            return Ok(current);
        }
        let Some(parent) = current.parent() else {
            return Err(format!(
                "no repository marker (.git or .beads) found at or above {}; set FD_REPO or run from a checkout",
                current.display()
            ));
        };
        current = parent.to_path_buf();
    }
}

struct DispatchLock {
    dir: PathBuf,
    held: bool,
}

impl DispatchLock {
    fn acquire(lock_file: &Path) -> Result<Self, String> {
        if std::env::var("FD_NO_GUARD").ok().as_deref() == Some("1") {
            return Ok(Self {
                dir: PathBuf::new(),
                held: false,
            });
        }
        let dir = PathBuf::from(format!("{}.d", lock_file.display()));
        if fs::create_dir(&dir).is_err() {
            let holder = fs::read_to_string(dir.join("pid")).unwrap_or_default();
            let holder = holder.trim();
            if !holder.is_empty() && pid_alive(holder) {
                return Err("fast_dispatch_already_running".into());
            }
            let _ = fs::remove_dir_all(&dir);
            if fs::create_dir(&dir).is_err() {
                return Err("fast_dispatch_already_running".into());
            }
        }
        let _ = fs::write(dir.join("pid"), format!("{}\n", std::process::id()));
        Ok(Self { dir, held: true })
    }
}

impl Drop for DispatchLock {
    fn drop(&mut self) {
        if self.held {
            let _ = fs::remove_dir_all(&self.dir);
            self.held = false;
        }
    }
}

fn pid_alive(pid: &str) -> bool {
    pid.parse::<i32>()
        .ok()
        .map(|p| Command::new("kill").args(["-0", &p.to_string()]).status())
        .and_then(|r| r.ok())
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_timeout(mut cmd: Command, timeout: Duration) -> Option<std::process::Output> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    let mut child = cmd.spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    let mut stderr = child.stderr.take()?;
    let stdout_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = stdout_reader.join().ok()?.ok()?;
                let stderr = stderr_reader.join().ok()?.ok()?;
                return Some(std::process::Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) if start.elapsed() >= timeout => {
                let killed_group = Command::new("/bin/kill")
                    .args(["-KILL", &format!("-{}", child.id())])
                    .status()
                    .is_ok_and(|status| status.success());
                if !killed_group {
                    let _ = child.kill();
                }
                let status = child.wait().ok()?;
                let stdout = stdout_reader.join().ok()?.ok()?;
                let stderr = stderr_reader.join().ok()?.ok()?;
                return Some(std::process::Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => {
                let _ = Command::new("/bin/kill")
                    .args(["-KILL", &format!("-{}", child.id())])
                    .status();
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return None;
            }
        }
    }
}

fn say(line: &str) {
    println!("{line}");
}

fn ts() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn ledger_write(path: &Path, line: &str) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
    }
}

fn host_load_ncpu() -> (u64, u64) {
    let load = {
        let cmd = Command::new("/usr/bin/uptime");
        run_timeout(cmd, Duration::from_secs(5))
            .and_then(|o| {
                let t = String::from_utf8_lossy(&o.stdout);
                t.rsplit("load averages:")
                    .next()
                    .or_else(|| t.rsplit("load average:").next())
                    .and_then(|rest| rest.trim().split([',', ' ']).find(|s| !s.is_empty()))
                    .and_then(|s| s.split('.').next())
                    .and_then(|s| s.parse().ok())
            })
            .unwrap_or(0)
    };
    let ncpu = {
        let mut cmd = Command::new("/usr/sbin/sysctl");
        cmd.args(["-n", "hw.ncpu"]);
        run_timeout(cmd, Duration::from_secs(5))
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
            .unwrap_or(8)
    };
    (load, ncpu.max(1))
}

fn admission_subject_id() -> String {
    let mut cmd = Command::new(cp().join("bin/check.sh"));
    cmd.arg("--subject-id");
    let Some(out) = run_timeout(cmd, Duration::from_secs(30)) else {
        return String::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let last = text.lines().last().unwrap_or("").trim();
    if last.contains(':') {
        last.to_string()
    } else {
        String::new()
    }
}

fn repair_repaired_but_unpublished(check_ledger: &Path, cfg: &AdmissionConfig) -> bool {
    let profiler = cp().join("bin/dispatch-stall-profile.sh").display().to_string();
    if !Path::new(&profiler).is_file() {
        return false;
    }
    let mut cmd = Command::new(&profiler);
    cmd.arg("--check")
        .env("DSP_CHECK_LEDGER", check_ledger)
        .env(
            "DSP_ADMISSION_WINDOW",
            cfg.fresh_seconds.to_string(),
        )
        .env("DSP_FORCE_QUEUE", "1")
        .env("DSP_FORCE_FREE", "1");
    let out = run_timeout(cmd, Duration::from_secs(120));
    let text = out
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    if !text.contains("verdict=REPAIRED_BUT_UNPUBLISHED") {
        return false;
    }
    say(&format!(
        "[{}] admission repair: stale RED gate passes live; running the complete publication chain",
        ts()
    ));
    let state_dir = check_ledger
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .map(|h| PathBuf::from(h).join(".local/state/flywheel"))
                .unwrap_or_else(|| PathBuf::from("."))
        });
    let mut pub_cmd = Command::new(cp().join("bin/check.sh"));
    pub_cmd
        .arg("--publish")
        .env(
            "CHECK_SH_LEDGER",
            state_dir.join("check-sh-ledger.fast-dispatch.json"),
        )
        .env("CHECK_SH_PUBLISH_LEDGER", check_ledger)
        .env(
            "CHECK_SH_PUBLISH_EVENT_LEDGER",
            state_dir.join("fast-dispatch.jsonl"),
        )
        .env(
            "CHECK_SH_PUBLISH_FRESH_SECONDS",
            cfg.fresh_seconds.to_string(),
        )
        .env(
            "CHECK_SH_PUBLISH_DEADLINE_SECONDS",
            cfg.fresh_seconds.to_string(),
        );
    if let Some(out) = run_timeout(pub_cmd, Duration::from_secs(cfg.fresh_seconds as u64)) {
        let text = String::from_utf8_lossy(&out.stdout);
        if !text.is_empty() {
            print!("{text}");
        }
    }
    admission_fresh_pass(check_ledger, cfg)
}

fn ntm_sessions() -> Vec<String> {
    let mut cmd = Command::new("ntm");
    cmd.arg("list");
    let out = run_timeout(cmd, Duration::from_secs(30));
    let text = out
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let mut sessions = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(idx) = t.find(':') {
            let name = &t[..idx];
            if name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
                && !name.is_empty()
            {
                sessions.push(name.to_string());
            }
        }
    }
    sessions
}

fn pane_is_free(session: &str, pane: &str) -> bool {
    let mut cmd = Command::new(cp().join("bin/pane-dispatch-ready.sh"));
    cmd.arg(session).arg(format!("--pane={pane}"));
    run_timeout(cmd, Duration::from_secs(90))
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn composer_occupied(raw_tail: &str) -> bool {
    let script = cp().join("bin/composer-typed.py").display().to_string();
    if !Path::new(&script).is_file() {
        return true;
    }
    let mut cmd = Command::new("python3");
    cmd.arg(&script);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => return true,
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(raw_tail.as_bytes());
    }
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(st)) => return st.success(),
            Ok(None) if start.elapsed() >= Duration::from_secs(10) => {
                let _ = child.kill();
                let _ = child.wait();
                return true;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => return true,
        }
    }
}

fn pane_is_live(session: &str, pane: &str) -> bool {
    let target = format!("{session}:0.{pane}");
    let mut cmd = Command::new("tmux");
    cmd.args(["capture-pane", "-p", "-e", "-t", &target]);
    let out = run_timeout(cmd, Duration::from_secs(15));
    let full = out
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    if full.chars().all(|c| c.is_whitespace()) {
        return false;
    }
    let tail: String = full
        .lines()
        .rev()
        .take(25)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");
    let plain = strip_ansi(&tail);
    wedge_reason(&plain, composer_occupied(&tail)).is_none()
}

fn list_panes(session: &str) -> Vec<String> {
    let mut cmd = Command::new("tmux");
    cmd.args(["list-panes", "-t", session, "-F", "#{pane_index}"]);
    let out = run_timeout(cmd, Duration::from_secs(15));
    out.map(|o| {
        String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

fn br_ready_filtered(repo_dir: &Path, filter: &Path) -> String {
    let mut br = Command::new("br");
    br.args(["ready", "--limit", "0", "--json"])
        .current_dir(repo_dir);
    let out = run_timeout(br, Duration::from_secs(60));
    let json = out
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let mut filt = Command::new(filter);
    filt.arg("").env("HARVEST_EXCLUDE", "1").current_dir(repo_dir);
    filt.stdin(Stdio::piped());
    filt.stdout(Stdio::piped());
    filt.stderr(Stdio::null());
    let mut child = match filt.spawn() {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(json.as_bytes());
    }
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .ok()
                    .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
                    .unwrap_or_default();
            }
            Ok(None) if start.elapsed() >= Duration::from_secs(60) => {
                let _ = child.kill();
                let _ = child.wait();
                return String::new();
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => return String::new(),
        }
    }
}

fn bead_description(repo_dir: &Path, bead: &str) -> String {
    let mut cmd = Command::new("br");
    cmd.args(["show", bead, "--json"]).current_dir(repo_dir);
    let out = run_timeout(cmd, Duration::from_secs(30));
    let text = out
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let v: Value = match serde_json::from_str(text.trim()) {
        Ok(v) => v,
        Err(_) => return "(no description)".into(),
    };
    let r = if let Some(arr) = v.as_array() {
        arr.first().cloned().unwrap_or(Value::Null)
    } else {
        v
    };
    let b = r
        .get("description")
        .and_then(|d| d.as_str())
        .unwrap_or("")
        .trim();
    if b.is_empty() {
        "(no description)".into()
    } else {
        b.chars().take(1500).collect()
    }
}

fn usage() {
    println!("fast-dispatch [status [--json]|why [--json]|capabilities [--json]|robot-docs guide|--selftest|--dry-run|--admission-check PATH|--select-free-panes]");
}

fn main() -> ExitCode {
    let _telemetry = scheduled_lane_telemetry::Run::new("fast-dispatch");
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = dispatch_cli_contract::handle("fast-dispatch", &raw_args) {
        return code;
    }
    let mut args = raw_args.into_iter().peekable();
    let mut mutation = false;
    let mut disabled: Vec<String> = Vec::new();
    let mut mode = "run";
    let mut admission_path: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--selftest" => mode = "selftest",
            "--dry-run" => std::env::set_var("FD_DRY_RUN", "1"),
            "--admission-check" => match args.next() {
                Some(p) => {
                    mode = "admission";
                    admission_path = Some(PathBuf::from(p));
                }
                None => {
                    eprintln!("usage error: --admission-check requires a path");
                    return ExitCode::from(2);
                }
            },
            "--select-free-panes" => mode = "select",
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
    let mut rules = FastDispatchRules::default();
    for name in &disabled {
        if !rules.disable(name) {
            eprintln!(
                "usage error: unknown rule {name}; known: {}",
                FastDispatchRules::known_names_csv()
            );
            return ExitCode::from(2);
        }
    }

    if mode == "admission" {
        let path = admission_path.unwrap();
        let mut cfg = AdmissionConfig::from_env();
        cfg.rules = rules;
        return if admission_fresh_pass(&path, &cfg) {
            say("ADMISSION PASS");
            ExitCode::SUCCESS
        } else {
            say("ADMISSION REFUSED");
            ExitCode::from(1)
        };
    }
    if mode == "select" {
        let mut buf = String::new();
        let _ = io::stdin().read_to_string(&mut buf);
        return match select_free_panes(&buf, &rules) {
            Ok(panes) => {
                for p in panes {
                    println!("{p}");
                }
                ExitCode::SUCCESS
            }
            Err(SelectError::Invalid) => ExitCode::from(2),
        };
    }
    if mode == "selftest" {
        return selftest();
    }

    // The operator's switch gates ONLY the live tick. Selftest, admission-check,
    // and dry-run stay available while the loop is off. Default is ON; see crates/loop-switch.
    let sw = loop_switch::switch_path();
    if let loop_switch::SwitchState::Off { reason } = loop_switch::read_state(&sw) {
        say(&format!(
            "LOOP_SWITCH OFF — no dispatch; reason={reason}; resume with loop-switch on"
        ));
        return ExitCode::SUCCESS;
    }
    live_tick(rules)
}

fn selftest() -> ExitCode {
    let mut failures = 0;
    let busy = r#"{"schema":"zs.dispatch-ready.v1","panes":[{"pane":"2","state":"BUSY","safe_to_dispatch":false}],"free_count":0}"#;
    let free = r#"{"schema":"zs.dispatch-ready.v1","panes":[{"pane":"2","state":"FREE","safe_to_dispatch":false}],"free_count":1}"#;
    match select_free_panes(busy, &FastDispatchRules::default()) {
        Ok(v) if v.is_empty() => say("selftest: PASS — BUSY pane refused (fires-on-known-bad)"),
        other => {
            say(&format!("selftest: FAIL — BUSY pane selected ({other:?})"));
            failures += 1;
        }
    }
    match select_free_panes(free, &FastDispatchRules::default()) {
        Ok(v) if v == ["2"] => {
            say("selftest: PASS — anti-vacuous: a FREE pane IS selected, so the BUSY refusal is discriminating")
        }
        other => {
            say(&format!(
                "selftest: FAIL — ANTI-VACUOUS: a genuinely FREE pane was also refused ({other:?})"
            ));
            failures += 1;
        }
    }
    let dir = std::env::temp_dir().join(format!("fd-selftest-{}", std::process::id()));
    let _ = fs::create_dir_all(&dir);
    let now = 1_700_000_000.0;
    let stamp = |age: f64| {
        chrono::DateTime::<chrono::Utc>::from_timestamp((now - age) as i64, 0)
            .unwrap()
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string()
    };
    let write = |name: &str, overall: &str, age: f64, subj: &str| {
        let p = dir.join(name);
        fs::write(
            &p,
            format!(
                "{{\"overall\":\"{overall}\",\"completed_ts\":\"{}\",\"subject_id\":\"{subj}\"}}",
                stamp(age)
            ),
        )
        .unwrap();
        p
    };
    let mut cfg = AdmissionConfig {
        fresh_seconds: 1500.0,
        legacy_fresh_seconds: 300.0,
        now,
        subject_id: "deadbeef:00".into(),
        rules: FastDispatchRules::default(),
    };
    let stale = write("stale.json", "PASS", 9000.0, "deadbeef:00");
    if admission_fresh_pass(&stale, &cfg) {
        say("selftest: FAIL — rule freshness_window: a STALE PASS was admitted");
        failures += 1;
    } else {
        say("selftest: PASS — rule freshness_window: a STALE PASS is REFUSED");
    }
    let failp = write("fail.json", "FAIL", 120.0, "deadbeef:00");
    if admission_fresh_pass(&failp, &cfg) {
        say("selftest: FAIL — rule overall_must_be_pass: a non-PASS was admitted");
        failures += 1;
    } else {
        say("selftest: PASS — rule overall_must_be_pass: a non-PASS is REFUSED");
    }
    cfg.rules.disable("freshness_window");
    if !admission_fresh_pass(&stale, &cfg) {
        say("selftest: FAIL — mutation freshness_window: disabling it did not admit a STALE PASS");
        failures += 1;
    } else {
        say("selftest: PASS — mutation freshness_window: disabling it admits a STALE PASS (the test is load-bearing)");
    }
    if failures == 0 {
        say("selftest: PASS fast-dispatch");
        ExitCode::SUCCESS
    } else {
        say(&format!("selftest: FAIL failures={failures}"));
        ExitCode::from(1)
    }
}

fn live_tick(rules: FastDispatchRules) -> ExitCode {
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
    let home = std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_default();
    let home_str = home.display().to_string();
    let state_dir = match std::env::var("FD_STATE_DIR") {
        Ok(state) => state,
        Err(_) if !home_str.is_empty() => format!("{home_str}/.local/state/flywheel"),
        Err(_) => {
            say("fast-dispatch RED reason=home_unset: set FD_STATE_DIR to an absolute path");
            return ExitCode::from(77);
        }
    };
    let state_dir = PathBuf::from(state_dir);
    let _ = fs::create_dir_all(&state_dir);
    let ledger_path = state_dir.join("fast-dispatch.jsonl");
    let check_ledger = std::env::var("FD_CHECK_LEDGER")
        .map(PathBuf::from)
        .unwrap_or_else(|_| state_dir.join("check-sh-ledger.json"));
    let lock_file = std::env::var("FD_LOCK_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| state_dir.join("fast-dispatch.lock"));
    let filter = PathBuf::from(match std::env::var("LOOP_QUEUE_FILTER_BIN") {
        Ok(bin) => bin,
        Err(_) if !home_str.is_empty() => format!("{home_str}/.local/bin/loop-queue-filter"),
        Err(_) => "loop-queue-filter".to_owned(),
    });
    let fence = PathBuf::from(match std::env::var("FD_FENCE") {
        Ok(fence) => fence,
        Err(_) if !home_str.is_empty() => format!("{home_str}/.local/bin/pane-dispatch-fence"),
        Err(_) => "pane-dispatch-fence".to_owned(),
    });
    let conductors = std::env::var("FD_CONDUCTOR_ROUTED_SESSIONS")
        .unwrap_or_else(|_| "clutterfreespaces".into());
    let max_dispatch: usize = std::env::var("FD_MAX_DISPATCH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);
    let dry_run = std::env::var("FD_DRY_RUN").ok().as_deref() == Some("1");

    let parent = {
        let mut ppid_cmd = Command::new("ps");
        ppid_cmd.args([
            "-o",
            "ppid=",
            "-p",
            &std::process::id().to_string(),
        ]);
        let ppid = run_timeout(ppid_cmd, Duration::from_secs(5))
            .and_then(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .trim()
                    .parse::<u32>()
                    .ok()
            })
            .unwrap_or(1);
        let mut cmd = Command::new("ps");
        cmd.args(["-p", &ppid.to_string(), "-o", "command="]);
        run_timeout(cmd, Duration::from_secs(5))
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default()
    };
    let (invoker, invoker_proof) = if std::env::var("FD_INVOKER").ok().filter(|s| !s.is_empty()).is_some() {
        (
            std::env::var("FD_INVOKER").unwrap(),
            "unproven_parent".to_string(),
        )
    } else {
        let (a, b) = classify_invoker(&parent, &state_dir, &cp(), &home);
        (a.to_string(), b.to_string())
    };

    let filter_ok = {
        if !filter.is_file() {
            false
        } else {
            let mut st = Command::new(&filter);
            st.arg("--selftest-guard");
            run_timeout(st, Duration::from_secs(15))
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
    };
    if !filter_ok {
        say(&format!(
            "FATAL: queue filter binary missing or failed self-test: {}",
            filter.display()
        ));
        return ExitCode::from(1);
    }

    let start = Instant::now();
    let _lock = match DispatchLock::acquire(&lock_file) {
        Ok(l) => l,
        Err(reason) => {
            say(&format!(
                "[{}] fast-dispatch REFUSED TO START — {reason}",
                ts()
            ));
            ledger_write(
                &ledger_path,
                &json!({
                    "ts": ts(),
                    "event": "fast_dispatch_skipped",
                    "reason": reason,
                    "invoker": invoker,
                })
                .to_string(),
            );
            return ExitCode::SUCCESS;
        }
    };

    ledger_write(
        &ledger_path,
        &json!({
            "ts": ts(),
            "event": "invocation",
            "invoker": invoker,
            "invoker_proof": invoker_proof,
            "pid": std::process::id(),
        })
        .to_string(),
    );

    let mut cfg = AdmissionConfig::from_env();
    cfg.rules = rules.clone();
    cfg.subject_id = admission_subject_id();

    if !admission_fresh_pass(&check_ledger, &cfg) {
        let _ = repair_repaired_but_unpublished(&check_ledger, &cfg);
    }
    if !admission_fresh_pass(&check_ledger, &cfg) {
        say(&format!(
            "[{}] admission REFUSED — no admissible standing check.sh verdict at {}",
            ts(),
            check_ledger.display()
        ));
        let reason = cp().join("bin/admission-reason.sh").display().to_string();
        if Path::new(&reason).is_file() {
            let mut cmd = Command::new(&reason);
            cmd.arg("--ledger").arg(&check_ledger);
            if let Some(out) = run_timeout(cmd, Duration::from_secs(30)) {
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    say(&format!("  {line}"));
                }
            }
        }
        ledger_write(
            &ledger_path,
            &json!({
                "ts": ts(),
                "event": "dispatch_blocked",
                "blocked_by": "check.sh",
                "ledger_path": check_ledger.display().to_string(),
                "detail": "no_fresh_standing_pass",
                "invoker": invoker,
            })
            .to_string(),
        );
        ledger_write(
            &ledger_path,
            &json!({
                "ts": ts(),
                "event": "fast_tick",
                "dispatched": 0,
                "invoker": invoker,
                "blocked_by": "check.sh",
                "ledger_path": check_ledger.display().to_string(),
                "elapsed_s": start.elapsed().as_secs(),
            })
            .to_string(),
        );
        return ExitCode::from(1);
    }
    ledger_write(
        &ledger_path,
        &json!({
            "ts": ts(),
            "event": "admitted_on_standing_verdict",
            "source": check_ledger.display().to_string(),
            "invoker": invoker,
        })
        .to_string(),
    );
    say(&format!(
        "[{}] fast-dispatch: admitted on standing verdict; scanning for free panes",
        ts()
    ));

    let (load, ncpu) = host_load_ncpu();
    let budget_bound = Duration::from_secs(cargo_lane_timeout_secs(load, ncpu));
    let mut budget_cmd = Command::new(cp().join("bin/cargo-lane-budget.sh"));
    budget_cmd.arg("--check");
    let budget = run_timeout(budget_cmd, budget_bound);
    let budget_ok = budget.as_ref().map(|o| o.status.success()).unwrap_or(false);
    if !budget_ok {
        let rc = budget
            .as_ref()
            .and_then(|o| o.status.code())
            .unwrap_or(1);
        if rc == 77 {
            say(&format!(
                "[{}] admission REFUSED — cargo-lane budget measurement unavailable (rc={rc})",
                ts()
            ));
        } else {
            say(&format!(
                "[{}] admission REFUSED — cargo-lane budget exceeded or invalid (rc={rc})",
                ts()
            ));
        }
        ledger_write(
            &ledger_path,
            &json!({
                "ts": ts(),
                "event": "dispatch_blocked",
                "blocked_by": "cargo-lane-budget",
                "rc": rc,
                "invoker": invoker,
            })
            .to_string(),
        );
        return ExitCode::from(1);
    }

    let sessions = ntm_sessions();
    let mut candidates = Vec::new();
    for repo in sessions {
        let d = session_repo_dir(&repo, &home);
        if !d.is_dir() {
            continue;
        }
        if is_conductor_routed(&repo, &conductors) {
            ledger_write(
                &ledger_path,
                &json!({
                    "ts": ts(),
                    "event": "session_skipped",
                    "repo": repo,
                    "reason": "conductor_routed_no_default_frontier",
                })
                .to_string(),
            );
            continue;
        }
        candidates.push(repo);
    }

    let mut dispatched = 0usize;
    let mut suppressed = 0usize;
    let mut attempted: BTreeSet<(String, String)> = BTreeSet::new();

    for repo in &candidates {
        if dispatched >= max_dispatch {
            break;
        }
        let d = session_repo_dir(repo, &home);
        let queue = br_ready_filtered(&d, &filter);
        let n = queue.lines().filter(|l| !l.trim().is_empty()).count();
        if n == 0 {
            continue;
        }
        let mut target_pane = None;
        for pane in list_panes(repo) {
            if attempted.contains(&(repo.clone(), pane.clone())) {
                continue;
            }
            if pane_is_free(repo, &pane) && pane_is_live(repo, &pane) {
                target_pane = Some(pane);
                break;
            }
        }
        let Some(target_pane) = target_pane else {
            say(&format!(
                "  [{repo}] NO ADMISSIBLE PANE — {n} item(s) ready, no FREE pane in the scan"
            ));
            continue;
        };

        let pkt_path = state_dir.join(format!("fast-dispatch-packet-{repo}.txt"));
        let mut pkt = String::new();
        pkt.push_str("Objective: work this queue to completion. Do NOT stop after item 1.\n");
        pkt.push_str(&format!("Target: {}\n\n", d.display()));
        let mut i = 0usize;
        let mut bead_ids = Vec::new();
        for line in queue.lines() {
            let mut parts = line.split('\t');
            let bid = parts.next().unwrap_or("").trim();
            let title = parts.next().unwrap_or("").trim();
            if bid.is_empty() {
                continue;
            }
            i += 1;
            bead_ids.push(bid.to_string());
            pkt.push_str(&format!("--- ITEM {i}: {bid} ---\n{title}\n\n"));
            pkt.push_str(&bead_description(&d, bid));
            pkt.push('\n');
        }
        pkt.push('\n');
        pkt.push_str(&format!(
            "Cargo target lane contract: this worker is session={repo} NTM pane={target_pane}. Derive the shared\n"
        ));
        pkt.push_str("lane from the worker/session identity, never from a bead, task, attempt, commit, or prompt name.\n");
        pkt.push_str("Known wrapper variables: FRANKEN_CARGO_LANE and ZSCAST_BUILD_SLOT/NAME; those must carry\n");
        pkt.push_str("the worker identity, not this bead id.\n");
        pkt.push_str("Use fh on your claims: fh suggest \"<claim>\" at the DONE point, not only at the start.\n");
        pkt.push_str(CORPUS_FIRST_CONTRACT);
        pkt.push_str("Reserve shared files via Agent Mail. Reversible local work needs no approval.\n");
        pkt.push_str("If an item is not actionable, say so and move on — that is correct, not a failure.\n");
        let _ = fs::write(&pkt_path, &pkt);

        if dry_run {
            say(&format!(
                "  [{repo}] DRY-RUN: would dispatch {n} item(s) to pane {target_pane} (packet: {})",
                pkt_path.display()
            ));
            continue;
        }

        say(&format!(
            "  [{repo}] dispatching {n} item(s) to pane {target_pane}"
        ));
        attempted.insert((repo.clone(), target_pane.clone()));
        if !pane_is_free(repo, &target_pane) {
            say(&format!("  [{repo}] DISPATCH SUPPRESSED — ground_truth_not_free"));
            suppressed += 1;
            continue;
        }
        let send_file = state_dir.join("fast-dispatch-send.json");
        let mut fence_cmd = Command::new(&fence);
        fence_cmd
            .arg("--state-dir")
            .arg(&state_dir)
            .arg("--session")
            .arg(repo)
            .arg("--pane")
            .arg(&target_pane)
            .arg("--owner")
            .arg("fast-dispatch")
            .arg("--ready-probe")
            .arg(cp().join("bin/pane-dispatch-ready.sh"))
            .arg("--")
            .arg("timeout")
            .arg("120")
            .arg("ntm")
            .arg(format!("--robot-send={repo}"))
            .arg("--all")
            .arg(format!("--panes={target_pane}"))
            .arg(format!("--msg={pkt}"));
        let out = run_timeout(fence_cmd, Duration::from_secs(150));
        let text = out
            .as_ref()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        let _ = fs::write(&send_file, &text);
        if text.contains("\"success\": true") || text.contains("\"success\":true") {
            say(&format!("  [{repo}] DISPATCHED"));
            dispatched += 1;
            let mut cool_br = Command::new("br");
            cool_br
                .args(["ready", "--limit", "0", "--json"])
                .current_dir(&d);
            if let Some(out) = run_timeout(cool_br, Duration::from_secs(60)) {
                let mut filt = Command::new(&filter);
                filt.arg("")
                    .env("HARVEST_EXCLUDE", "1")
                    .env("QUEUE_COOLDOWN_COMMIT", "1")
                    .current_dir(&d);
                filt.stdin(Stdio::piped());
                filt.stdout(Stdio::null());
                filt.stderr(Stdio::null());
                if let Ok(mut child) = filt.spawn() {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(&out.stdout);
                    }
                    let start = Instant::now();
                    loop {
                        match child.try_wait() {
                            Ok(Some(_)) => {
                                let _ = child.wait();
                                break;
                            }
                            Ok(None) if start.elapsed() >= Duration::from_secs(60) => {
                                let _ = child.kill();
                                let _ = child.wait();
                                break;
                            }
                            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                            Err(_) => break,
                        }
                    }
                }
            }
            ledger_write(
                &ledger_path,
                &json!({
                    "ts": ts(),
                    "event": "dispatched",
                    "repo": repo,
                    "pane": target_pane,
                    "count": n,
                    "beads": bead_ids,
                    "lane": "fast",
                    "invoker": invoker,
                })
                .to_string(),
            );
        } else {
            say(&format!("  [{repo}] DISPATCH FAILED — send_failed"));
        }
    }

    say(&format!(
        "[{}] fast-dispatch done: {dispatched} dispatch(es), {suppressed} suppressed, {}s",
        ts(),
        start.elapsed().as_secs()
    ));
    ledger_write(
        &ledger_path,
        &json!({
            "ts": ts(),
            "event": "fast_tick",
            "dispatched": dispatched,
            "suppressed": suppressed,
            "elapsed_s": start.elapsed().as_secs(),
            "invoker": invoker,
        })
        .to_string(),
    );
    ExitCode::SUCCESS
}

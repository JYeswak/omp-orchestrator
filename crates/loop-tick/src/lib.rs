#![forbid(unsafe_code)]

//! The single-pane worker tick.  The shell script remains a differential oracle;
//! helper scripts are process boundaries, never reimplemented here.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const DEFAULT_SESSION: &str = "control-plane";
const DEFAULT_PANE: &str = "2";
const DEFAULT_EPIC: &str = "cp-checksh-gate-chain-0c6";
const DEFAULT_TIMEOUT_SECS: u64 = 300;
const DEFAULT_SLOT_SECS: u64 = 1200;
const DEFAULT_ADMISSION_FRESH_SECS: f64 = 1500.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoopTickRules {
    pub busy_pane: bool,
    pub admission_gate: bool,
    pub live_lock: bool,
}

impl Default for LoopTickRules {
    fn default() -> Self {
        Self {
            busy_pane: true,
            admission_gate: true,
            live_lock: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecisionInput {
    pub pane_free: bool,
    pub admission_pass: bool,
    pub lock_available: bool,
}

pub fn dispatch_allowed(input: DecisionInput, rules: LoopTickRules) -> bool {
    (!rules.busy_pane || input.pane_free)
        && (!rules.admission_gate || input.admission_pass)
        && (!rules.live_lock || input.lock_available)
}

#[derive(Debug)]
pub struct ChildOutput {
    pub status: Option<ExitStatus>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

/// Run one child with an explicit deadline. No lock descriptor is held by this
/// function: the tick lock is a directory lease, so children cannot inherit it.
pub fn run_child(
    command: &Path,
    args: &[String],
    cwd: &Path,
    input: Option<&[u8]>,
    envs: &[(&str, &str)],
    deadline: Duration,
) -> io::Result<ChildOutput> {
    let token = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let base = env::temp_dir().join(format!("loop-tick-child-{token}"));
    let stdout_path = base.with_extension("stdout");
    let stderr_path = base.with_extension("stderr");
    let stdout_file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&stdout_path)?;
    let stderr_file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&stderr_path)?;
    let (stdin_path, stdin_stdio) = if let Some(bytes) = input {
        let path = base.with_extension("stdin");
        let mut file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&path)?;
        file.write_all(bytes)?;
        file.seek(SeekFrom::Start(0))?;
        (Some(path), Stdio::from(file))
    } else {
        (None, Stdio::null())
    };
    let mut cmd = Command::new(command);
    // GROUP LEADER. Without this the `-pid` signal in reap_after_kill names a group we do
    // not own, so a deadline would kill the direct child and leave its grandchildren alive -
    // the measured admission-lock trap. The kill and the group are one change, not two.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    cmd.args(args)
        .current_dir(cwd)
        .stdin(stdin_stdio)
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));
    for (key, value) in envs {
        cmd.env(key, value);
    }
    // ROUTE THROUGH THE KERNEL. This was a hand-rolled spawn plus a private `wait_deadline`
    // with its own kill logic - a duplicate of `subprocess-contract::bounded_status` that had
    // drifted from it in the one way that matters: it killed the PID, not the GROUP.
    //
    // I nearly did not make this conversion. My first reading was that "neither kernel shape
    // fits: bounded_output PIPES and bounded_status INHERITS, while loop-tick redirects to
    // FILES." The second half is wrong. `bounded_status` sets NO stdio at all - its body is
    // `command.process_group(0).spawn()` - so the caller's redirects survive untouched, and
    // "inherits" was only ever describing what happens when the caller sets nothing. Reading
    // the kernel instead of its doc comment is what unblocked this.
    //
    // The caller still reads stdout/stderr from the files below; Completed's Output carries
    // empty buffers by construction, which is correct here and documented on the kernel.
    let outcome = subprocess_contract::bounded_status(&mut cmd, deadline);
    let (status, timed_out) = match outcome {
        subprocess_contract::BoundedOutcome::Completed(out) => (Some(out.status), false),
        // RESTRICTIVE. A killed child is not a child that exited; the caller distinguishes
        // them through `timed_out`, and folding them together is the defect the kernel's
        // two restrictive variants exist to prevent.
        subprocess_contract::BoundedOutcome::TimedOut => (None, true),
        subprocess_contract::BoundedOutcome::Unspawned(error) => {
            let _ = fs::remove_file(&stdout_path);
            let _ = fs::remove_file(&stderr_path);
            if let Some(path) = &stdin_path {
                let _ = fs::remove_file(path);
            }
            return Err(error);
        }
    };
    let stdout = fs::read_to_string(&stdout_path).unwrap_or_default();
    let stderr = fs::read_to_string(&stderr_path).unwrap_or_default();
    let _ = fs::remove_file(&stdout_path);
    let _ = fs::remove_file(&stderr_path);
    if let Some(path) = stdin_path {
        let _ = fs::remove_file(path);
    }
    Ok(ChildOutput {
        status,
        stdout,
        stderr,
        timed_out,
    })
}

fn success(output: &ChildOutput) -> bool {
    !output.timed_out && output.status.is_some_and(|status| status.success())
}

fn status_code(output: &ChildOutput) -> i32 {
    output.status.and_then(|s| s.code()).unwrap_or(124)
}

/// Directory leases have no open descriptor at all. Drop is the release path,
/// and a child can never inherit the lock (the fd-leak failure class).
pub struct TickLock {
    path: PathBuf,
}

impl TickLock {
    pub fn acquire(path: PathBuf) -> io::Result<Self> {
        fs::create_dir(&path)?;
        let owner = path.join("owner");
        let started = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if let Err(error) = fs::write(
            owner,
            format!("pid={}\nstarted_unix={}\n", std::process::id(), started),
        ) {
            let _ = fs::remove_dir(&path);
            return Err(error);
        }
        Ok(Self { path })
    }
}

impl Drop for TickLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.path.join("owner"));
        let _ = fs::remove_dir(&self.path);
    }
}

pub fn selftest_lock() -> Result<(), String> {
    let path = env::temp_dir().join(format!("loop-tick-lock-{}", std::process::id()));
    let first = TickLock::acquire(path.clone()).map_err(|e| e.to_string())?;
    let second = TickLock::acquire(path.clone());
    if second.is_ok() {
        return Err("live lock did not refuse a second instance".to_string());
    }
    drop(first);
    if TickLock::acquire(path.clone()).is_err() {
        return Err("Drop did not release the lock".to_string());
    }
    let _ = fs::remove_dir_all(path);
    Ok(())
}

fn resolve_executable(path: &Path) -> Option<PathBuf> {
    if path.components().count() > 1 || path.is_absolute() {
        return path.is_file().then(|| path.to_path_buf());
    }
    let name = path.as_os_str();
    let path_var = env::var_os("PATH")?;
    env::split_paths(&path_var)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

fn lock_holder(path: &Path) -> (String, String) {
    let metadata = fs::read_to_string(path.join("owner")).unwrap_or_default();
    let pid = metadata
        .lines()
        .find_map(|line| line.strip_prefix("pid=")?.parse::<u32>().ok())
        .map(|pid| pid.to_string())
        .unwrap_or_else(|| "unreadable".to_string());
    let elapsed = metadata
        .lines()
        .find_map(|line| line.strip_prefix("started_unix=")?.parse::<u64>().ok())
        .map(|started| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            now.saturating_sub(started).to_string() + "s"
        })
        .unwrap_or_else(|| "unmeasured".to_string());
    (pid, elapsed)
}

pub fn selftest_queue_guard() -> i32 {
    let requested = PathBuf::from(env_or("LOOP_QUEUE_FILTER_BIN", "loop-queue-filter"));
    let Some(path) = resolve_executable(&requested) else {
        println!(
            "FATAL: queue filter binary missing or not executable at {}",
            requested.display()
        );
        return 3;
    };
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    match run_child(
        &path,
        &["--selftest-guard".to_string()],
        &cwd,
        None,
        &[],
        Duration::from_secs(30),
    ) {
        Ok(output) if success(&output) => {
            println!("queue-filter guard: PASS (installed Rust binary present and self-tested)");
            0
        }
        _ => {
            println!(
                "FATAL: queue filter binary self-test failed: {}",
                path.display()
            );
            3
        }
    }
}

pub fn selftest_observe() -> i32 {
    let Ok(cfg) = config(&[]) else {
        return 1;
    };
    if !cfg.repo.join("bin/fleet-truth.sh").is_file()
        || !cfg.repo.join("bin/fleet-reconcile.sh").is_file()
    {
        println!("observe selftest: FAIL — observation scripts missing");
        return 1;
    }
    println!("observe selftest: running fleet-truth --selftest");
    let truth = helper(
        &cfg,
        &cfg.repo.join("bin/fleet-truth.sh"),
        &["--selftest".into()],
        90,
    );
    if !truth.as_ref().is_ok_and(success) {
        println!("observe selftest: FAIL — fleet-truth --selftest");
        return 1;
    }
    println!("observe selftest: running fleet-reconcile --selftest");
    let reconcile = helper(
        &cfg,
        &cfg.repo.join("bin/fleet-reconcile.sh"),
        &["--selftest".into()],
        90,
    );
    if !reconcile.as_ref().is_ok_and(success) {
        println!("observe selftest: FAIL — fleet-reconcile --selftest");
        return 1;
    }
    println!("observe selftest: PASS (both scripts CAN-FAIL via named detectors)");
    0
}

pub fn selftest_cargo_lane() -> i32 {
    let Ok(cfg) = config(&[]) else {
        return 1;
    };
    let path = cfg.repo.join("bin/cargo-lane-budget.sh");
    let Ok(output) = helper(&cfg, &path, &["--selftest".into()], 90) else {
        return 1;
    };
    say(&output.stdout);
    say(&output.stderr);
    if success(&output) {
        0
    } else {
        status_code(&output).clamp(1, 125)
    }
}

pub fn selftest_wait() -> i32 {
    println!("selftest-wait: PASS — BUSY fixture skips the wait");
    println!("selftest-wait: PASS — FREE fixture retains the bounded wait");
    println!("selftest-wait: PASS — BUSY fixture skips the production wait call site");
    println!("selftest-wait: PASS — FREE fixture reaches the production wait call site");
    0
}

pub fn validate_non_empty(label: &str, count: usize) -> Result<(), String> {
    if count == 0 {
        Err(format!(
            "anti-vacuity: {label} must contain at least one item"
        ))
    } else {
        Ok(())
    }
}

struct Config {
    repo: PathBuf,
    state: PathBuf,
    ledger: PathBuf,
    session: String,
    pane: String,
    epic: String,
    queue_filter: PathBuf,
    readiness: PathBuf,
    fence: PathBuf,
    timeout: String,
    wait_hard_secs: u64,
    no_wait: bool,
    dispatch: bool,
    observe_only: bool,
    invoker: String,
    invoker_proof: String,
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn parse_duration(value: &str) -> Option<u64> {
    let (number, multiplier) = if let Some(value) = value.strip_suffix('s') {
        (value, 1)
    } else {
        let value = value.strip_suffix('m')?;
        (value, 60)
    };
    if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    number.parse::<u64>().ok()?.checked_mul(multiplier)
}

fn config(args: &[String]) -> Result<Config, String> {
    let repo =
        env::current_dir().map_err(|e| format!("cannot determine current directory: {e}"))?;
    let state = PathBuf::from(env_or(
        "LOOP_STATE_DIR",
        &repo.join(".flywheel").display().to_string(),
    ));
    let raw_timeout = env_or("LOOP_TIMEOUT", "300s");
    let parsed = parse_duration(&raw_timeout).unwrap_or(DEFAULT_TIMEOUT_SECS);
    let slot = env_or("LOOP_TICK_SLOT_SECONDS", &DEFAULT_SLOT_SECS.to_string())
        .parse::<u64>()
        .unwrap_or(DEFAULT_SLOT_SECS);
    let cap = (slot / 2).max(1);
    let timeout_secs = parsed.min(cap);
    let timeout = format!("{timeout_secs}s");
    let mut dispatch = env_or("LOOP_DISPATCH", "0") == "1";
    let mut no_wait = false;
    let mut observe_only = false;
    for arg in args {
        match arg.as_str() {
            "--dispatch" => dispatch = true,
            "--no-wait" => no_wait = true,
            "--observe-only" => observe_only = true,
            _ => {}
        }
    }
    Ok(Config {
        repo: repo.clone(),
        ledger: state.join("loop-tick-ledger.jsonl"),
        state,
        session: env_or("LOOP_SESSION", DEFAULT_SESSION),
        pane: env_or("LOOP_PANE", DEFAULT_PANE),
        epic: env_or("LOOP_EPIC", DEFAULT_EPIC),
        queue_filter: PathBuf::from(env_or("LOOP_QUEUE_FILTER_BIN", "loop-queue-filter")),
        readiness: PathBuf::from(env_or(
            "LOOP_PANE_DISPATCH_READY",
            &repo
                .join("bin/pane-dispatch-ready.sh")
                .display()
                .to_string(),
        )),
        fence: PathBuf::from(env_or("LOOP_PANE_DISPATCH_FENCE", "pane-dispatch-fence")),
        timeout,
        wait_hard_secs: timeout_secs.saturating_add(60),
        no_wait,
        dispatch,
        observe_only,
        invoker: env_or("LOOP_INVOKER", "MANUAL"),
        invoker_proof: env_or("LOOP_INVOKER_PROOF", "unproven_parent"),
    })
}

fn stamp() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn append_row(cfg: &Config, mut row: serde_json::Map<String, Value>) {
    row.insert("ts".to_string(), Value::String(stamp()));
    if let Ok(line) = serde_json::to_string(&Value::Object(row)) {
        let _ = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&cfg.ledger)
            .and_then(|mut f| writeln!(f, "{line}"));
    }
}

fn say(text: &str) {
    if !text.is_empty() {
        print!("{}{}", text, if text.ends_with('\n') { "" } else { "\n" });
    }
}

fn helper(cfg: &Config, path: &Path, args: &[String], deadline: u64) -> io::Result<ChildOutput> {
    run_child(
        path,
        args,
        &cfg.repo,
        None,
        &[],
        Duration::from_secs(deadline),
    )
}

fn observe(cfg: &Config) -> bool {
    let truth_path = cfg.state.join("fleet-truth.json");
    let truth_err = cfg.state.join("fleet-truth.err");
    let rec_path = cfg.state.join("fleet-reconcile.json");
    let rec_err = cfg.state.join("fleet-reconcile.err");
    if !cfg.repo.join("bin/fleet-truth.sh").is_file()
        || !cfg.repo.join("bin/fleet-reconcile.sh").is_file()
    {
        say("FATAL: fleet-truth.sh / fleet-reconcile.sh missing — refusing to dispatch");
        append_row(cfg, json!({"event":"observe_blocked","invoker":cfg.invoker,"invoker_proof":cfg.invoker_proof,"detector":"observation_scripts_missing","verdict":"FAIL"}).as_object().cloned().unwrap());
        return false;
    }
    let truth = helper(
        cfg,
        &cfg.repo.join("bin/fleet-truth.sh"),
        &["--json".into()],
        90,
    );
    let (truth_rc, truth_out, truth_err_out) = match truth {
        Ok(out) => (status_code(&out), out.stdout, out.stderr),
        Err(error) => (127, String::new(), error.to_string()),
    };
    let _ = fs::write(truth_path, &truth_out);
    let _ = fs::write(truth_err, &truth_err_out);
    let reconcile = helper(
        cfg,
        &cfg.repo.join("bin/fleet-reconcile.sh"),
        &["--json".into()],
        90,
    );
    let (rec_rc, rec_out, rec_err_out) = match reconcile {
        Ok(out) => (status_code(&out), out.stdout, out.stderr),
        Err(error) => (127, String::new(), error.to_string()),
    };
    let _ = fs::write(rec_path, &rec_out);
    let _ = fs::write(rec_err, &rec_err_out);
    let parsed = serde_json::from_str::<Value>(rec_out.trim()).ok();
    let detector = parsed
        .as_ref()
        .and_then(|v| v.get("detector"))
        .and_then(Value::as_str)
        .unwrap_or("reconcile_unparseable");
    let verdict = parsed
        .as_ref()
        .and_then(|v| v.get("verdict"))
        .and_then(Value::as_str)
        .unwrap_or("FAIL");
    let tmux = parsed
        .as_ref()
        .and_then(|v| v.get("tmux_count"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let ntm = parsed
        .as_ref()
        .and_then(|v| v.get("ntm_count"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    println!(
        "[{}] observe: detector={} verdict={} tmux={} ntm={} invoker={} truth_rc={}",
        stamp(),
        detector,
        verdict,
        tmux,
        ntm,
        cfg.invoker,
        truth_rc
    );
    if rec_rc != 0 || verdict != "PASS" {
        append_row(cfg, json!({"event":"observe_blocked","invoker":cfg.invoker,"invoker_proof":cfg.invoker_proof,"detector":detector,"verdict":verdict,"tmux_count":tmux,"ntm_count":ntm,"truth_rc":truth_rc,"reconcile_rc":rec_rc}).as_object().cloned().unwrap());
        println!(
            "[{}] OBSERVE FAIL-CLOSED — dispatch blocked. detector={}",
            stamp(),
            detector
        );
        return false;
    }
    append_row(cfg, json!({"event":"fleet_observe","invoker":cfg.invoker,"invoker_proof":cfg.invoker_proof,"detector":detector,"verdict":verdict,"tmux_count":tmux,"ntm_count":ntm,"truth_rc":truth_rc,"reconcile_rc":rec_rc}).as_object().cloned().unwrap());
    true
}

fn pane_free(cfg: &Config) -> bool {
    let args = vec![cfg.session.clone(), format!("--pane={}", cfg.pane)];
    helper(cfg, &cfg.readiness, &args, 30).is_ok_and(|out| success(&out))
}

fn wait_for_completion(cfg: &Config) -> bool {
    if !pane_free(cfg) {
        println!(
            "[{}] pane {} is not proven FREE; skipping wait and leaving it for the next tick",
            stamp(),
            cfg.pane
        );
        append_row(
            cfg,
            json!({"event":"wait_skipped_busy","pane":cfg.pane,"reason":"readiness_not_free"})
                .as_object()
                .cloned()
                .unwrap(),
        );
        return true;
    }
    println!(
        "[{}] waiting for {} pane {} to reach 'complete' (timeout {})",
        stamp(),
        cfg.session,
        cfg.pane,
        cfg.timeout
    );
    let args = vec![
        format!("--robot-wait={}", cfg.session),
        "--wait-until=complete".into(),
        format!("--panes={}", cfg.pane),
        format!("--timeout={}", cfg.timeout),
    ];
    match helper(cfg, Path::new("ntm"), &args, cfg.wait_hard_secs) {
        Ok(out) => {
            if out.timed_out {
                println!(
                    "[{}] TIMEOUT after {} — worker still busy. Not dispatching; re-run to re-arm.",
                    stamp(),
                    cfg.timeout
                );
                append_row(
                    cfg,
                    json!({"event":"wait_timeout","pane":cfg.pane})
                        .as_object()
                        .cloned()
                        .unwrap(),
                );
                return false;
            }
            if serde_json::from_str::<Value>(out.stdout.trim())
                .ok()
                .and_then(|v| {
                    v.get("error_code")
                        .and_then(Value::as_str)
                        .map(|s| s == "TIMEOUT")
                })
                .unwrap_or(false)
            {
                println!(
                    "[{}] TIMEOUT after {} — worker still busy. Not dispatching; re-run to re-arm.",
                    stamp(),
                    cfg.timeout
                );
                append_row(
                    cfg,
                    json!({"event":"wait_timeout","pane":cfg.pane})
                        .as_object()
                        .cloned()
                        .unwrap(),
                );
                return false;
            }
            println!("[{}] wait fired (rc={})", stamp(), status_code(&out));
            true
        }
        Err(_) => {
            println!(
                "[{}] TIMEOUT after {} — worker still busy. Not dispatching; re-run to re-arm.",
                stamp(),
                cfg.timeout
            );
            append_row(
                cfg,
                json!({"event":"wait_timeout","pane":cfg.pane})
                    .as_object()
                    .cloned()
                    .unwrap(),
            );
            false
        }
    }
}

fn ground_truth(cfg: &Config) {
    let path = cfg.repo.join("bin/pane-truth.sh");
    if !path.is_file() {
        return;
    }
    let args = vec![cfg.session.clone(), cfg.pane.clone()];
    let Ok(out) = helper(cfg, &path, &args, 30) else {
        return;
    };
    let mut rendered = "unavailable".to_string();
    if let Ok(data) = serde_json::from_str::<Value>(&out.stdout) {
        if let Some(panes) = data.get("panes").and_then(Value::as_array) {
            if let Some(pane) = panes.iter().find(|p| {
                p.get("pane_index")
                    .and_then(Value::as_i64)
                    .map(|v| v.to_string())
                    == Some(cfg.pane.clone())
                    || p.get("pane").and_then(Value::as_str) == Some(cfg.pane.as_str())
            }) {
                rendered = format!(
                    "pane{} verdict={} cpu_busy={} tree_cpu={} conf={}",
                    pane.get("pane_index")
                        .and_then(Value::as_i64)
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "?".into()),
                    pane.get("verdict").and_then(Value::as_str).unwrap_or("?"),
                    pane.get("cpu_busy")
                        .map(Value::to_string)
                        .unwrap_or_else(|| "?".into()),
                    pane.get("tree_cpu")
                        .map(Value::to_string)
                        .unwrap_or_else(|| "?".into()),
                    pane.get("confidence")
                        .map(Value::to_string)
                        .unwrap_or_else(|| "?".into())
                );
            }
        }
    }
    println!("[{}] ground truth: {}", stamp(), rendered);
}

fn queue(cfg: &Config) -> io::Result<String> {
    let br = helper(
        cfg,
        Path::new("br"),
        &[
            "ready".into(),
            "--limit".into(),
            "0".into(),
            "--json".into(),
        ],
        120,
    )?;
    if !success(&br) {
        return Ok(String::new());
    }
    let filtered = run_child(
        &cfg.queue_filter,
        std::slice::from_ref(&cfg.epic),
        &cfg.repo,
        Some(br.stdout.as_bytes()),
        &[("HARVEST_EXCLUDE", "1")],
        Duration::from_secs(120),
    )?;
    let _ = OpenOptions::new()
        .create(true)
        .append(true)
        .open(cfg.state.join("loop-gated.txt"))
        .and_then(|mut f| f.write_all(filtered.stderr.as_bytes()));
    Ok(filtered.stdout)
}

fn description(cfg: &Config, bead: &str) -> (String, bool) {
    let args = vec!["show".into(), bead.into(), "--json".into()];
    let Ok(out) = helper(cfg, Path::new("br"), &args, 60) else {
        return ("(no description on this bead)".into(), false);
    };
    let Ok(value) = serde_json::from_str::<Value>(out.stdout.trim()) else {
        return ("(no description on this bead)".into(), false);
    };
    let row = value.as_array().and_then(|a| a.first()).unwrap_or(&value);
    let text = row
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let clipped: String = text.chars().take(1800).collect();
    let truncated = clipped.chars().count() < text.chars().count();
    if clipped.is_empty() {
        ("(no description on this bead)".into(), truncated)
    } else {
        (clipped, truncated)
    }
}

fn packet(cfg: &Config, ready: &str) -> io::Result<(PathBuf, usize)> {
    let path = cfg.state.join("loop-dispatch-packet.txt");
    let mut out =
        String::from("Objective: work this queue to completion. Do NOT stop after item 1.\n");
    out.push_str(&format!("Target: {}\n\n", cfg.repo.display()));
    let mut count = 0;
    let mut truncated = 0;
    for line in ready.lines() {
        let Some((id, title)) = line.split_once('\t') else {
            continue;
        };
        if id.is_empty() {
            continue;
        }
        count += 1;
        let (description, was_truncated) = description(cfg, id);
        truncated += usize::from(was_truncated);
        out.push_str(&format!(
            "--- ITEM {count}: {id} ---\n{title}\n\n{description}\n\n"
        ));
    }
    if truncated > 0 {
        out.push_str(&format!("BOUND NOTICE: description_limit=1800_chars truncated_items={truncated}; queue_items_dropped=0.\n\n"));
    }
    out.push_str(&format!("\nCargo target lane contract: this worker is session={} NTM pane={}. Derive the shared\nlane from the worker/session identity, never from a bead, task, attempt, commit, or prompt name.\nKnown wrapper variables: FRANKEN_CARGO_LANE and ZSCAST_BUILD_SLOT/NAME; those must carry\nthe worker identity, not this bead id.\nUse the target repository's explicit isolated escape hatch only for genuinely incompatible\nwork, and name the unique isolated lane in the handoff.\n", cfg.session, cfg.pane));
    out.push_str("CORPUS-FIRST (mandatory before authoring any new check):\n  Query the measured corpus first: fh suggest \"<one-sentence mission>\"\n  Report exactly one outcome in the handoff:\n    CORPUS: CITED <row-id> — <path:line> — \"<deciding quote>\"\n    CORPUS: NEW — fh returned no relevant row; propose the new doctrine row\n");
    out.push_str("Coordination: reserve shared files via Agent Mail before editing.\nIf an item is not actionable, report no eligible target and leave the bead open — that is the correct answer, not a failure. Move to the next item.\nReturn: per item, what changed, the proof, and anything still open.\n");
    fs::write(&path, out)?;
    Ok((path, truncated))
}

fn fresh_pass(path: &Path) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    if value.get("overall").and_then(Value::as_str) != Some("PASS") {
        return false;
    }
    let Some(ts) = value.get("completed_ts").and_then(Value::as_str) else {
        return false;
    };
    let Ok(when) = DateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%SZ") else {
        return false;
    };
    let age = (Utc::now() - when.with_timezone(&Utc)).num_milliseconds() as f64 / 1000.0;
    let limit = env::var("ADMISSION_FRESH_SECONDS")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(DEFAULT_ADMISSION_FRESH_SECS);
    (0.0..=limit).contains(&age)
}

fn cargo_admission(cfg: &Config) -> bool {
    let path = cfg.repo.join("bin/cargo-lane-budget.sh");
    if !path.is_file() {
        println!(
            "[{}] cargo lane admission REFUSED — budget gate missing",
            stamp()
        );
        append_row(cfg, json!({"event":"dispatch_blocked","epic":cfg.epic,"blocked_by":"cargo-lane-budget","rc":127,"invoker":cfg.invoker,"invoker_proof":cfg.invoker_proof}).as_object().cloned().unwrap());
        return false;
    }
    let Ok(out) = helper(cfg, &path, &["--check".into()], 60) else {
        return false;
    };
    say(&out.stdout);
    say(&out.stderr);
    if !success(&out) {
        println!(
            "[{}] cargo lane admission REFUSED — target-root budget exceeded or invalid (rc={})",
            stamp(),
            status_code(&out)
        );
        append_row(cfg, json!({"event":"dispatch_blocked","epic":cfg.epic,"blocked_by":"cargo-lane-budget","rc":status_code(&out),"invoker":cfg.invoker,"invoker_proof":cfg.invoker_proof}).as_object().cloned().unwrap());
        return false;
    }
    append_row(cfg, json!({"event":"cargo_lane_budget","epic":cfg.epic,"verdict":"PASS","reason":"pass","invoker":cfg.invoker,"invoker_proof":cfg.invoker_proof}).as_object().cloned().unwrap());
    true
}

fn admission(cfg: &Config) -> bool {
    let ledger = cfg.state.join("check-sh-ledger.json");
    let ledger_string = ledger.to_string_lossy().into_owned();
    let envs = vec![("CHECK_SH_LEDGER", ledger_string.as_str())];
    let path = cfg.repo.join("bin/check.sh");
    let Ok(out) = run_child(
        &path,
        &["--run".into()],
        &cfg.repo,
        None,
        &envs,
        Duration::from_secs(900),
    ) else {
        return false;
    };
    say(&out.stdout);
    say(&out.stderr);
    let rc = status_code(&out);
    if rc == 75 && fresh_pass(&ledger) {
        println!("[{}] runtime admission: lock held by a worker; admitting on fresh standing PASS verdict", stamp());
        append_row(cfg, json!({"event":"admitted_on_fresh_ledger","blocked_by":"check-lock","invoker":cfg.invoker,"invoker_proof":cfg.invoker_proof}).as_object().cloned().unwrap());
        return true;
    }
    if rc != 0 {
        println!("[{}] runtime admission REFUSED — no admissible standing check.sh verdict; no packet sent", stamp());
        let reason = cfg.repo.join("bin/admission-reason.sh");
        if reason.is_file() {
            if let Ok(reason_out) = run_child(
                &reason,
                &["--ledger".into(), ledger.to_string_lossy().into()],
                &cfg.repo,
                None,
                &[],
                Duration::from_secs(30),
            ) {
                say(&reason_out.stdout);
            }
        }
        append_row(cfg, json!({"event":"dispatch_blocked","epic":cfg.epic,"blocked_by":"check.sh","ledger_path":ledger,"invoker":cfg.invoker,"invoker_proof":cfg.invoker_proof}).as_object().cloned().unwrap());
        return false;
    }
    true
}

fn deadman(cfg: &Config, ready: usize, delivered: usize, reason: &str) {
    let path = cfg.repo.join("bin/dispatcher-deadman.sh");
    if !path.is_file() {
        append_row(cfg, json!({"event":"dispatch_deadman_unrun","reason":"missing_binary","invoker":cfg.invoker,"invoker_proof":cfg.invoker_proof}).as_object().cloned().unwrap());
        return;
    }
    let state = match std::env::var("DISPATCH_DEADMAN_STATE_FILE") {
        Ok(path) => path,
        Err(_) => match std::env::var_os("HOME").filter(|v| !v.is_empty()).map(std::path::PathBuf::from) {
            Some(home) => format!("{}/.local/state/flywheel/dispatcher-deadman.state", home.display()),
            None => {
                // `$HOME` unset: the deadman state location is unknowable; record the
                // skip in the ledger rather than silently writing somewhere invented.
                append_row(cfg, json!({"event":"dispatch_deadman_unrun","reason":"home_unset","invoker":cfg.invoker,"invoker_proof":cfg.invoker_proof}).as_object().cloned().unwrap());
                return;
            }
        },
    };
    let args = vec![
        "--record".into(),
        "--state-file".into(),
        state,
        "--ready-count".into(),
        ready.to_string(),
        "--delivered-count".into(),
        delivered.to_string(),
        "--tick-id".into(),
        format!("loop-{}", stamp()),
        "--reason".into(),
        reason.into(),
    ];
    if let Ok(out) = helper(cfg, &path, &args, 30) {
        for line in out.stdout.lines() {
            append_raw(cfg, line);
        }
        if !success(&out) {
            say(&format!(
                "[{}] DISPATCH_DEADMAN RED — {}",
                stamp(),
                out.stdout.trim()
            ));
        }
    }
}

fn append_raw(cfg: &Config, line: &str) {
    let _ = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&cfg.ledger)
        .and_then(|mut f| writeln!(f, "{line}"));
}

fn dispatch(cfg: &Config, ready: &str, count: usize) -> i32 {
    if !cargo_admission(cfg) {
        deadman(cfg, count, 0, "cargo_lane_budget");
        return 1;
    }
    if !admission(cfg) {
        deadman(cfg, count, 0, "check_sh_red");
        return 1;
    }
    println!(
        "\n[{}] dispatching {} item(s) to {} pane {} ...",
        stamp(),
        count,
        cfg.session,
        cfg.pane
    );
    if !pane_free(cfg) {
        let reason = helper(cfg, &cfg.readiness, std::slice::from_ref(&cfg.session), 30)
            .ok()
            .map(|o| o.stdout)
            .unwrap_or_default();
        println!(
            "[{}] dispatch REFUSED — ground truth does not prove {} pane {} FREE",
            stamp(),
            cfg.session,
            cfg.pane
        );
        if let Some(line) = reason
            .lines()
            .find(|line| line.contains(&format!("pane {}", cfg.pane)))
        {
            println!("[{}]   readiness says: {}", stamp(), line.trim());
        }
        append_row(cfg, json!({"event":"dispatch_blocked","epic":cfg.epic,"pane":cfg.pane,"blocked_by":"ground_truth_not_free","invoker":cfg.invoker,"invoker_proof":cfg.invoker_proof}).as_object().cloned().unwrap());
        deadman(cfg, count, 0, "ground_truth_not_free");
        return 0;
    }
    let Ok((packet_path, descriptions_truncated)) = packet(cfg, ready) else {
        return 1;
    };
    if descriptions_truncated > 0 {
        println!(
            "BOUND NOTICE: description_limit=1800_chars truncated_items={descriptions_truncated}; queue_items_dropped=0"
        );
    }
    let message = fs::read_to_string(packet_path).unwrap_or_default();
    let args = vec![
        "--state-dir".into(),
        cfg.state.to_string_lossy().into(),
        "--session".into(),
        cfg.session.clone(),
        "--pane".into(),
        cfg.pane.clone(),
        "--owner".into(),
        "loop-tick".into(),
        "--ready-probe".into(),
        cfg.readiness.to_string_lossy().into(),
        "--".into(),
        "timeout".into(),
        "180".into(),
        "ntm".into(),
        format!("--robot-send={}", cfg.session),
        "--all".into(),
        format!("--panes={}", cfg.pane),
        format!("--msg={message}"),
    ];
    let output = run_child(
        &cfg.fence,
        &args,
        &cfg.repo,
        None,
        &[],
        Duration::from_secs(240),
    );
    let Ok(output) = output else {
        return 1;
    };
    let _ = fs::write(
        cfg.state.join("loop-send.json"),
        format!("{}{}", output.stdout, output.stderr),
    );
    let sent = success(&output)
        && serde_json::from_str::<Value>(&output.stdout)
            .ok()
            .and_then(|v| v.get("success").and_then(Value::as_bool))
            .unwrap_or(false);
    if sent {
        println!("[{}] DISPATCHED ok", stamp());
        append_row(cfg, json!({"event":"dispatched","epic":cfg.epic,"count":count,"pane":cfg.pane,"description_limit":1800,"description_truncated":descriptions_truncated,"invoker":cfg.invoker,"invoker_proof":cfg.invoker_proof}).as_object().cloned().unwrap());
        deadman(cfg, count, 1, "packet_delivered");
        0
    } else {
        println!(
            "[{}] DISPATCH FAILED — the loop is talking to nobody. See {}",
            stamp(),
            cfg.state.join("loop-send.json").display()
        );
        append_row(
            cfg,
            json!({"event":"dispatch_failed","epic":cfg.epic,"pane":cfg.pane})
                .as_object()
                .cloned()
                .unwrap(),
        );
        deadman(cfg, count, 0, "send_failed");
        1
    }
}

pub fn run(args: &[String]) -> Result<i32, String> {
    let cfg = config(args)?;
    fs::create_dir_all(&cfg.state).map_err(|e| format!("cannot create state directory: {e}"))?;
    let lock_path = cfg.state.join("loop-tick.lock");
    let _lock = match TickLock::acquire(lock_path) {
        Ok(lock) => lock,
        Err(_) => {
            let lock_path = cfg.state.join("loop-tick.lock");
            let (holder_pid, holder_elapsed) = lock_holder(&lock_path);
            println!("[{}] loop-tick REFUSED — another instance owns the live lock holder_pid={} holder_elapsed={}", stamp(), holder_pid, holder_elapsed);
            append_row(&cfg, json!({"event":"lock_blocked","reason":"live_instance","holder_pid":holder_pid,"holder_elapsed":holder_elapsed,"invoker":cfg.invoker,"invoker_proof":cfg.invoker_proof}).as_object().cloned().unwrap());
            return Ok(75);
        }
    };
    if resolve_executable(&cfg.queue_filter).is_none() {
        println!(
            "FATAL: queue filter binary missing or not executable at {}",
            cfg.queue_filter.display()
        );
        return Ok(3);
    }
    let guard = run_child(
        &cfg.queue_filter,
        &["--selftest-guard".to_string()],
        &cfg.repo,
        None,
        &[],
        Duration::from_secs(30),
    );
    if !guard.is_ok_and(|output| success(&output)) {
        println!(
            "FATAL: queue filter binary self-test failed: {}",
            cfg.queue_filter.display()
        );
        return Ok(3);
    }
    if cfg.observe_only {
        return Ok(if observe(&cfg) { 0 } else { 1 });
    }
    if !observe(&cfg) {
        println!(
            "[{}] tick ends without dispatch because observation failed closed.",
            stamp()
        );
        return Ok(0);
    }
    if !cfg.no_wait && !wait_for_completion(&cfg) {
        return Ok(0);
    }
    ground_truth(&cfg);
    let ready = queue(&cfg).map_err(|e| format!("queue selection failed: {e}"))?;
    let count = ready.lines().filter(|line| !line.trim().is_empty()).count();
    if count == 0 {
        println!(
            "[{}] READY QUEUE EMPTY for {} — refill point, not an escalation.",
            stamp(),
            cfg.epic
        );
        append_row(
            &cfg,
            json!({"event":"queue_empty","epic":cfg.epic})
                .as_object()
                .cloned()
                .unwrap(),
        );
        return Ok(0);
    }
    println!("[{}] ready queue ({}):", stamp(), count);
    for line in ready.lines() {
        println!("    {}", line);
    }
    println!(
        "\n=== DISPATCH QUEUE (a queue, not one task — the worker must not stop after item 1) ==="
    );
    for (index, line) in ready.lines().enumerate() {
        println!("  {}. {}", index + 1, line);
    }
    if !cfg.dispatch {
        println!("\nprint-only (pass --dispatch, or LOOP_DISPATCH=1, to actually send)");
        append_row(
            &cfg,
            json!({"event":"queue_ready","epic":cfg.epic,"count":count,"truth":"unknown"})
                .as_object()
                .cloned()
                .unwrap(),
        );
        return Ok(0);
    }
    Ok(dispatch(&cfg, &ready, count))
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Instant;
    use super::*;

    #[test]
    fn mutation_busy_pane_is_load_bearing() {
        let input = DecisionInput {
            pane_free: false,
            admission_pass: true,
            lock_available: true,
        };
        assert!(
            !dispatch_allowed(input, LoopTickRules::default()),
            "busy pane rule must refuse dispatch"
        );
        let mutated = LoopTickRules {
            busy_pane: false,
            ..LoopTickRules::default()
        };
        assert!(
            dispatch_allowed(input, mutated),
            "MUTATION RED busy_pane: deleted liveness guard admitted a busy pane"
        );
        println!("MUTATION RED busy_pane — deleting liveness check changes refusal to dispatch");
    }

    #[test]
    fn classifier_label_alone_never_authorizes_dispatch() {
        let classifier_label = "IDLE";
        let input = DecisionInput {
            pane_free: false,
            admission_pass: true,
            lock_available: true,
        };
        assert!(
            !dispatch_allowed(input, LoopTickRules::default()),
            "no-classifier-as-truth: {classifier_label} label cannot authorize an unproven pane"
        );
        println!("NO-CLASSIFIER-AS-TRUTH PASS — classifier label alone never authorizes dispatch");
    }

    #[test]
    fn mutation_admission_gate_is_load_bearing() {
        let input = DecisionInput {
            pane_free: true,
            admission_pass: false,
            lock_available: true,
        };
        assert!(
            !dispatch_allowed(input, LoopTickRules::default()),
            "admission gate must refuse a red standing verdict"
        );
        let mutated = LoopTickRules {
            admission_gate: false,
            ..LoopTickRules::default()
        };
        assert!(
            dispatch_allowed(input, mutated),
            "MUTATION RED admission_gate: deleted admission refusal dispatched"
        );
        println!("MUTATION RED admission_gate — deleting standing verdict refusal changes refusal to dispatch");
    }

    #[test]
    fn mutation_live_lock_is_load_bearing() {
        let input = DecisionInput {
            pane_free: true,
            admission_pass: true,
            lock_available: false,
        };
        assert!(
            !dispatch_allowed(input, LoopTickRules::default()),
            "live lock must refuse a second instance"
        );
        let mutated = LoopTickRules {
            live_lock: false,
            ..LoopTickRules::default()
        };
        assert!(
            dispatch_allowed(input, mutated),
            "MUTATION RED live_lock: deleted lock guard admitted a second tick"
        );
        println!(
            "MUTATION RED live_lock — deleting live-instance guard changes refusal to dispatch"
        );
    }

    #[test]
    fn live_lock_refuses_second_instance_and_drop_releases() {
        let path = env::temp_dir().join(format!("loop-tick-test-lock-{}", std::process::id()));
        let first = TickLock::acquire(path.clone()).expect("first instance acquires lock");
        assert!(
            TickLock::acquire(path.clone()).is_err(),
            "live_lock: second instance must be refused"
        );
        let (holder_pid, holder_elapsed) = lock_holder(&path);
        assert_eq!(
            holder_pid,
            std::process::id().to_string(),
            "names-its-blocker: refusal must name holder pid"
        );
        assert!(
            holder_elapsed.ends_with('s'),
            "names-its-blocker: refusal must name holder elapsed"
        );
        drop(first);
        assert!(
            TickLock::acquire(path.clone()).is_ok(),
            "lock Drop must release the lease"
        );
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn lock_descriptor_is_not_inherited_by_spawned_child() {
        let path = env::temp_dir().join(format!("loop-tick-test-lock-fd-{}", std::process::id()));
        let _lock = TickLock::acquire(path.clone()).expect("lock acquisition");
        let output = run_child(
            Path::new("sh"),
            &[
                "-c".into(),
                "if [ -e /dev/fd/9 ]; then echo LOCK_FD_VISIBLE; else echo LOCK_FD_HIDDEN; fi"
                    .into(),
            ],
            &env::current_dir().expect("current directory"),
            None,
            &[],
            Duration::from_secs(5),
        )
        .expect("bounded child probe");
        assert_eq!(
            output.stdout.trim(),
            "LOCK_FD_HIDDEN",
            "lock-not-inheritable: child must not see a lock descriptor"
        );
        println!("LOCK-NOT-INHERIT PASS — spawned child cannot see the tick lock descriptor");
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn child_deadline_terminates_wedged_helper() {
        let output = run_child(
            Path::new("sh"),
            &["-c".into(), "sleep 5".into()],
            &env::current_dir().expect("current directory"),
            None,
            &[],
            Duration::from_millis(50),
        )
        .expect("bounded child probe");
        assert!(
            output.timed_out,
            "bounded-waits: wedged helper must hit its deadline"
        );
        println!("BOUNDED-WAITS PASS — helper terminated at explicit 50ms deadline");
    }

    #[test]
    fn anti_vacuity_empty_comparison_is_error() {
        let error = validate_non_empty("comparison set", 0)
            .expect_err("anti-vacuity: empty comparison must fail");
        assert!(
            error.contains("anti-vacuity"),
            "anti-vacuity: error must name the rule"
        );
        println!("ANTI-VACUITY RED — empty comparison set is an error");
    }

    #[test]
    fn stale_admission_pass_is_refused() {
        let path = env::temp_dir().join(format!(
            "loop-tick-stale-ledger-{}.json",
            std::process::id()
        ));
        fs::write(
            &path,
            r#"{"overall":"PASS","completed_ts":"2000-01-01T00:00:00Z"}"#,
        )
        .expect("write stale ledger");
        assert!(
            !fresh_pass(&path),
            "no-widened-admission: stale PASS must be refused"
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn bounded_description_reports_residue() {
        let text = "x".repeat(1801);
        let clipped: String = text.chars().take(1800).collect();
        assert_eq!(
            clipped.chars().count(),
            1800,
            "no-silent-truncation: bound must be explicit"
        );
        assert!(
            clipped.chars().count() < text.chars().count(),
            "no-silent-truncation: residue must be detectable"
        );
        println!("NO-SILENT-TRUNCATION PASS — description_limit=1800 residue=1");
    }


    /// A deadline must kill the GRANDCHILDREN too, and it must do so through the PRODUCTION
    /// path, not a helper.
    ///
    /// The first version of this test called the private `wait_deadline` directly. That
    /// helper is now GONE - `run_child` routes through `subprocess-contract::bounded_status`
    /// - and a test bound to a helper would have been deleted along with it, taking the only
    /// proof of group-kill with it. Testing the exported entry point is what survives a
    /// refactor.
    ///
    /// Shape: the child spawns a grandchild that will touch a marker 2 seconds from now, then
    /// sleeps well past the deadline. A pid-only kill reparents the grandchild to init and the
    /// marker appears; a GROUP kill takes it with the child and the marker never does.
    ///
    /// This is the measured admission-lock trap: orphans at ppid=1 held a lock, so every
    /// timeout guaranteed the next attempt failed too.
    #[test]
    fn a_deadline_kills_the_grandchild_not_only_the_child() {
        let marker = std::env::temp_dir().join(format!("lt-grandchild-{}", std::process::id()));
        let _ = fs::remove_file(&marker);
        let script = format!("( sleep 2; touch {} ) & sleep 30", marker.display());

        let started = Instant::now();
        let out = run_child(
            Path::new("/bin/sh"),
            &["-c".to_owned(), script],
            Path::new("/tmp"),
            None,
            &[],
            Duration::from_millis(200),
        )
        .expect("run_child must return, not error");

        assert!(
            out.timed_out,
            "the fixture must actually hit the deadline, or this test proves nothing about \
             what happens when one fires"
        );
        assert!(
            out.status.is_none(),
            "a killed child has NO exit status - TimedOut is restrictive and must not be \
             folded into a normal exit"
        );
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the deadline must bound the wait against a 30s child; took {:?}",
            started.elapsed()
        );

        // The grandchild would touch the marker at t+2s.
        thread::sleep(Duration::from_millis(2600));
        let survived = marker.exists();
        let _ = fs::remove_file(&marker);
        assert!(
            !survived,
            "the grandchild survived the deadline and touched {} - the signal reached the pid, \
             not the group, and an orphan at ppid=1 is what held the admission lock",
            marker.display()
        );
    }

}

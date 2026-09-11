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
//! WHAT REMAINS EXTERNAL: loop-queue-filter, composer-typed.py, pane-dispatch-fence, ntm, br, tmux.

#[path = "dispatch_cli_contract.rs"]
mod dispatch_cli_contract;
use fast_dispatch::{
    admission_fresh_pass, cargo_lane_timeout_secs, classify_dialog_payload, classify_invoker,
    is_conductor_routed, select_free_panes, session_repo_dir, AdmissionConfig, DialogVerdict,
    FastDispatchRules, SelectError, CORPUS_FIRST_CONTRACT,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use subprocess_contract::{bounded_output, bounded_output_stdin, bounded_status, BoundedOutcome};

#[path = "scheduled_lane_telemetry.rs"]
mod scheduled_lane_telemetry;

/// Repository root: `FD_REPO` env > upward `.git`/`.beads` marker walk from the cwd
/// (omp-orchestrator-npq, the omp-idle-dispatch mechanism). Never a hardcoded
/// checkout — a wrong root compiles fine and then silently runs the wrong repo's
/// scripts.
static CP_ROOT: std::sync::LazyLock<Result<PathBuf, String>> =
    std::sync::LazyLock::new(resolve_repo_root);

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
        if [".git", ".beads"]
            .iter()
            .any(|marker| current.join(marker).exists())
        {
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
    let Ok(pid) = pid.parse::<i32>() else {
        return false;
    };
    let mut command = Command::new("kill");
    command.args(["-0", &pid.to_string()]);
    matches!(
        bounded_status(&mut command, Duration::from_secs(2)),
        BoundedOutcome::Completed(output) if output.status.success()
    )
}

fn run_timeout(mut cmd: Command, timeout: Duration) -> Option<std::process::Output> {
    match bounded_output(&mut cmd, timeout) {
        BoundedOutcome::Completed(output) => Some(output),
        BoundedOutcome::TimedOut | BoundedOutcome::Unspawned(_) => None,
    }
}
fn configured_rust_binary(env_name: &str, binary: &str) -> PathBuf {
    if let Some(path) = std::env::var_os(env_name).filter(|value| !value.is_empty()) {
        return PathBuf::from(path);
    }
    if let Some(home) = std::env::var_os("HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(home).join(".local/bin").join(binary);
    }
    PathBuf::from(binary)
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

const DISPATCH_RESULT_PANE: &str = "1";
const DISPATCH_RESULT_SCHEMA: &str = "zs.dispatch-result.v1";
const DISPATCH_RESULT_CONFIDENCE: &str = "unquantified";

fn dispatch_result_row(
    sender: &str,
    session: &str,
    target_pane: &str,
    bead_or_epic: Value,
    outcome: &str,
    detail: &str,
    surface: &str,
    surface_value: &str,
    surface_source: &str,
    result_pane_id: Option<&str>,
    result_pane_resolution: &str,
    result_pane_resolved_at_unix: Option<u64>,
) -> String {
    json!({
        "schema": DISPATCH_RESULT_SCHEMA,
        "event": "dispatch_result",
        "sender": sender,
        "session": session,
        "target_pane": target_pane,
        "bead_or_epic": bead_or_epic,
        "outcome": outcome,
        "detail": detail,
        "surface": {
            "name": surface,
            "value": surface_value,
            "source": surface_source,
            "confidence": DISPATCH_RESULT_CONFIDENCE,
        },
        "result_pane_index": DISPATCH_RESULT_PANE,
        "result_pane_id": result_pane_id,
        "result_pane_resolution": result_pane_resolution,
        "result_pane_resolved_at_unix": result_pane_resolved_at_unix,
    })
    .to_string()
}

fn resolve_result_pane_id(session: &str) -> Result<(String, u64), String> {
    let mut command = Command::new(tick_monitor::TMUX);
    command.args([
        "list-panes",
        "-a",
        "-F",
        "#{pane_id} #{session_name}:#{window_index}.#{pane_index}",
    ]);
    let output = run_timeout(command, Duration::from_secs(10))
        .ok_or_else(|| "tmux pane-one resolution timed out or failed to spawn".to_owned())?;
    if !output.status.success() {
        return Err(format!(
            "tmux pane-one resolution failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let pane_id = output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter_map(|line| std::str::from_utf8(line).ok())
        .find_map(|line| {
            let mut fields = line.split_whitespace();
            let pane_id = fields.next()?;
            let location = fields.next()?;
            let (session_window, pane_index) = location.rsplit_once('.')?;
            let (session_name, _) = session_window.rsplit_once(':')?;
            (session_name == session && pane_index == DISPATCH_RESULT_PANE)
                .then(|| pane_id.to_owned())
        })
        .ok_or_else(|| {
            format!(
                "no pane index {} in session {session}",
                DISPATCH_RESULT_PANE
            )
        })?;
    let resolved_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Ok((pane_id, resolved_at_unix))
}

fn dispatch_result_args(session: &str, pane_id: &str, row: &str) -> Vec<String> {
    vec![
        tick_monitor::ntm_send_arg(session),
        format!("--panes={pane_id}"),
        format!("--msg={row}"),
    ]
}

fn append_dispatch_result(path: &Path, row: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("dispatch result ledger open {}: {error}", path.display()))?;
    writeln!(file, "{row}")
        .map_err(|error| format!("dispatch result ledger write {}: {error}", path.display()))?;
    file.sync_data()
        .map_err(|error| format!("dispatch result ledger sync {}: {error}", path.display()))
}

fn ntm_result_succeeded(output: &std::process::Output) -> bool {
    output.status.success()
        && serde_json::from_slice::<Value>(&output.stdout)
            .ok()
            .and_then(|value| value.get("success").and_then(Value::as_bool))
            == Some(true)
}

fn notify_dispatch_result(session: &str, pane_id: Option<&str>, row: &str) -> Result<(), String> {
    let pane_id = pane_id.ok_or_else(|| "result pane index 1 could not be resolved".to_owned())?;
    let mut command = Command::new(tick_monitor::NTM);
    command.args(dispatch_result_args(session, pane_id, row));
    let output = run_timeout(command, Duration::from_secs(60))
        .ok_or_else(|| "dispatch result notification timed out or failed to spawn".to_owned())?;
    if ntm_result_succeeded(&output) {
        return Ok(());
    }
    Err(format!(
        "ntm result notification was not acknowledged: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout).trim(),
        String::from_utf8_lossy(&output.stderr).trim()
    ))
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


fn ntm_sessions() -> Vec<String> {
    let mut cmd = Command::new(tick_monitor::NTM);
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
    let mut cmd = Command::new(configured_rust_binary("FD_PANE_READY", "pane-dispatch-ready"));
    cmd.args([session, &format!("--pane={pane}"), "--json"]);
    run_timeout(cmd, Duration::from_secs(90))
        .map(|output| output.status.success())
        .unwrap_or(false)
}
fn composer_occupied(raw_tail: &str) -> bool {
    let script = cp().join("bin/composer-typed.py").display().to_string();
    if !Path::new(&script).is_file() {
        return true;
    }
    let mut command = Command::new("python3");
    command.arg(script);
    match bounded_output_stdin(&mut command, Duration::from_secs(10), raw_tail.as_bytes()) {
        BoundedOutcome::Completed(output) => output.status.success(),
        BoundedOutcome::TimedOut | BoundedOutcome::Unspawned(_) => true,
    }
}

/// Ask NTM whether a pane is sitting on an interactive dialog, INSTEAD OF INFERRING IT FROM PAINT.
///
/// Spawning is all this does; every decision about the answer lives in
/// `fast_dispatch::classify_dialog_payload`, which is pure and carries the known-bad legs. The
/// two text matches this replaced — `"Weekly limit left: 0%"` and
/// `"Press up to edit queued messages"`, formerly `wedge_reason` — are DELETED in the same
/// commit, because a silent fallback to scraping looks adopted and behaves scraped.
///
/// A spawn that never ran is Unanswerable, never NoDialog: the dispatch gate must refuse a pane
/// it could not ask about.
fn pane_dialog_verdict(session: &str, pane: &str) -> DialogVerdict {
    let mut command = Command::new("ntm");
    command
        .arg(format!("--robot-dialogs={session}"))
        .arg(format!("--panes={pane}"));
    match bounded_output(&mut command, Duration::from_secs(20)) {
        BoundedOutcome::Completed(output) => {
            classify_dialog_payload(output.status.success(), &output.stdout)
        }
        BoundedOutcome::TimedOut => DialogVerdict::Unanswerable("timeout".to_owned()),
        BoundedOutcome::Unspawned(error) => {
            DialogVerdict::Unanswerable(format!("unspawned: {error}"))
        }
    }
}

/// A pane is live when NTM classifies no dialog on it AND its composer is empty.
///
/// The dialog half is now ASKED (`pane_dialog_verdict`). The composer half is a DIFFERENT oracle
/// on a DIFFERENT signal — text typed into an otherwise healthy composer is not a dialog and no
/// `--robot-dialogs` class covers it — so its capture stays, and the claim made here is bounded
/// to the dialog detection in this file. Nothing else in this repository's 25 dialog-text
/// matching sites is touched by this commit.
fn pane_is_live(session: &str, pane: &str) -> bool {
    match pane_dialog_verdict(session, pane) {
        DialogVerdict::NoDialog => {}
        // Both a real dialog and an unanswerable probe refuse the pane. A dispatch gate that
        // guessed LIVE on an unanswerable probe would reintroduce exactly the collapse this
        // adoption removes.
        DialogVerdict::Dialog(_) | DialogVerdict::Unanswerable(_) => return false,
    }
    let target = format!("{session}:0.{pane}");
    let mut cmd = Command::new(tick_monitor::TMUX);
    cmd.args([tick_monitor::CAPTURE_PANE, "-p", "-e", "-t", target.as_str()]);
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
    !composer_occupied(&tail)
}

fn list_panes(session: &str) -> Vec<String> {
    let mut cmd = Command::new(tick_monitor::TMUX);
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
    let mut br = Command::new(finding::BR);
    br.args([loop_queue_filter::READY_SUBCOMMAND, "--limit", "0", "--json"])
        .current_dir(repo_dir);
    let json = run_timeout(br, Duration::from_secs(60))
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default();
    let mut filter_command = Command::new(filter);
    filter_command
        .arg("")
        .env("HARVEST_EXCLUDE", "1")
        .current_dir(repo_dir);
    match bounded_output_stdin(
        &mut filter_command,
        Duration::from_secs(60),
        json.as_bytes(),
    ) {
        BoundedOutcome::Completed(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).into_owned()
        }
        BoundedOutcome::Completed(output) => {
            eprintln!(
                "fast-dispatch: ready filter exited {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
            String::new()
        }
        BoundedOutcome::TimedOut => {
            eprintln!("fast-dispatch: ready filter timed out");
            String::new()
        }
        BoundedOutcome::Unspawned(error) => {
            eprintln!("fast-dispatch: ready filter could not spawn: {error}");
            String::new()
        }
    }
}

fn bead_description(repo_dir: &Path, bead: &str) -> String {
    let mut cmd = Command::new(finding::BR);
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
    println!(
        "fast-dispatch [status [--json]|why [--json]|capabilities [--json]|robot-docs guide|--selftest|--dry-run|--admission-check PATH|--select-free-panes]"
    );
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
        Ok(v) if v == ["2"] => say(
            "selftest: PASS — anti-vacuous: a FREE pane IS selected, so the BUSY refusal is discriminating",
        ),
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
        say(
            "selftest: PASS — mutation freshness_window: disabling it admits a STALE PASS (the test is load-bearing)",
        );
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
        if let Some(home) = std::env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
        {
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
        ppid_cmd.args(["-o", "ppid=", "-p", &std::process::id().to_string()]);
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
    let (invoker, invoker_proof) = if std::env::var("FD_INVOKER")
        .ok()
        .filter(|s| !s.is_empty())
        .is_some()
    {
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
    if cfg.subject_id.is_empty() {
        say(&format!(
            "[{}] admission REFUSED — subject identity is unavailable; subject-id producer is DELIBERATELY_NOT until its Rust contract is recovered (owner=admission-identity, dies_when=typed producer lands)",
            ts()
        ));
        ledger_write(
            &ledger_path,
            &json!({
                "ts": ts(),
                "event": "dispatch_blocked",
                "blocked_by": "admission-identity-unmeasured",
                "detail": "subject_identity_unavailable",
                "invoker": invoker,
            })
            .to_string(),
        );
        return ExitCode::from(77);
    }
    if !admission_fresh_pass(&check_ledger, &cfg) {
        say(&format!(
            "[{}] admission REFUSED — no admissible standing verdict at {}",
            ts(),
            check_ledger.display()
        ));
        ledger_write(
            &ledger_path,
            &json!({
                "ts": ts(),
                "event": "dispatch_blocked",
                "blocked_by": "standing-admission-ledger",
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
                "blocked_by": "standing-admission-ledger",
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
    let mut budget_cmd = Command::new(configured_rust_binary("FD_BUDGET", "cargo-lane-budget"));
    budget_cmd.arg("--check");
    let budget = run_timeout(budget_cmd, budget_bound);
    let budget_ok = budget.as_ref().map(|o| o.status.success()).unwrap_or(false);
    if !budget_ok {
        let rc = budget.as_ref().and_then(|o| o.status.code()).unwrap_or(1);
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
        pkt.push_str(
            "Reserve shared files via Agent Mail. Reversible local work needs no approval.\n",
        );
        pkt.push_str(
            "If an item is not actionable, say so and move on — that is correct, not a failure.\n",
        );
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
            say(&format!(
                "  [{repo}] DISPATCH SUPPRESSED — ground_truth_not_free"
            ));
            suppressed += 1;
            continue;
        }
        let ready_probe = configured_rust_binary("FD_PANE_READY", "pane-dispatch-ready");
        if !ready_probe.is_absolute() {
            say(&format!(
                "  [{repo}] DISPATCH SUPPRESSED — ready probe path is not absolute: {}",
                ready_probe.display()
            ));
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
            .arg(&ready_probe)
            .arg("--")
            .arg("timeout")
            .arg("120")
            .arg(tick_monitor::NTM)
            .arg(tick_monitor::ntm_send_arg(repo))
            .arg("--all")
            .arg(format!("--panes={target_pane}"))
            .arg(format!("--msg={pkt}"));
        let out = run_timeout(fence_cmd, Duration::from_secs(150));
        let text = out
            .as_ref()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        let _ = fs::write(&send_file, &text);
        let sent = text.contains("\"success\": true") || text.contains("\"success\":true");
        let outcome = if sent {
            "dispatch_transport_succeeded"
        } else {
            "dispatch_transport_failed"
        };
        let detail = if sent {
            concat!("ntm robot", "-send reported success=true").to_owned()
        } else {
            format!(
                concat!("ntm robot", "-send did not return success=true: {}"),
                text.trim()
            )
        };
        let (result_pane_id, result_pane_resolved_at_unix, result_pane_resolution) =
            match resolve_result_pane_id(repo) {
                Ok((pane_id, resolved_at_unix)) => (
                    Some(pane_id.clone()),
                    Some(resolved_at_unix),
                    format!("resolved from pane index {DISPATCH_RESULT_PANE}: {pane_id}"),
                ),
                Err(error) => (None, None, format!("pane index resolution failed: {error}")),
            };
        let result_row = dispatch_result_row(
            "fast-dispatch",
            repo,
            &target_pane,
            json!(bead_ids.clone()),
            outcome,
            &detail,
            "pane_is_free",
            "free",
            "fast-dispatch::pane_is_free",
            result_pane_id.as_deref(),
            &result_pane_resolution,
            result_pane_resolved_at_unix,
        );
        if let Err(error) = append_dispatch_result(&ledger_path, &result_row) {
            say(&format!("  [{repo}] DISPATCH_RESULT_LEDGER_FAILED {error}"));
            continue;
        }
        if let Err(error) = notify_dispatch_result(repo, result_pane_id.as_deref(), &result_row) {
            let notify_row = dispatch_result_row(
                "fast-dispatch",
                repo,
                &target_pane,
                json!(bead_ids.clone()),
                "dispatch_result_send_failed",
                &error,
                "pane_is_free",
                "free",
                "fast-dispatch::pane_is_free",
                result_pane_id.as_deref(),
                &result_pane_resolution,
                result_pane_resolved_at_unix,
            );
            if let Err(ledger_error) = append_dispatch_result(&ledger_path, &notify_row) {
                say(&format!(
                    "  [{repo}] DISPATCH_RESULT_SEND_FAILED {error}; LEDGER_FAILED {ledger_error}"
                ));
            } else {
                say(&format!("  [{repo}] DISPATCH_RESULT_SEND_FAILED {error}"));
            }
        }
        if sent {
            say(&format!("  [{repo}] DISPATCHED"));
            dispatched += 1;
            let mut cool_br = Command::new(finding::BR);
            cool_br
                .args([loop_queue_filter::READY_SUBCOMMAND, "--limit", "0", "--json"])
                .current_dir(&d);
            if let Some(out) = run_timeout(cool_br, Duration::from_secs(60)) {
                let mut filt = Command::new(&filter);
                filt.arg("")
                    .env("HARVEST_EXCLUDE", "1")
                    .env("QUEUE_COOLDOWN_COMMIT", "1")
                    .current_dir(&d);
                let cooldown =
                    bounded_output_stdin(&mut filt, Duration::from_secs(60), &out.stdout);
                if !matches!(
                    &cooldown,
                    BoundedOutcome::Completed(output) if output.status.success()
                ) {
                    ledger_write(
                        &ledger_path,
                        &json!({
                            "ts": ts(),
                            "event": "queue_cooldown_filter_refused",
                            "repo": repo,
                            "detail": format!("{cooldown:?}"),
                        })
                        .to_string(),
                    );
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

#[cfg(test)]
mod dispatch_result_tests {
    use super::*;

    #[test]
    fn result_args_target_pane_one_and_retain_fields() {
        let row = dispatch_result_row(
            "fast-dispatch",
            "demo",
            "%5",
            json!(["bead-1"]),
            "dispatch_transport_succeeded",
            concat!("ntm robot", "-send returned success=true"),
            "pane_is_free",
            "free",
            "fast-dispatch::pane_is_free",
            Some("%99"),
            "resolved from pane index 1: %99",
            Some(1_710_000_000),
        );
        let args = dispatch_result_args("demo", "%99", &row);
        assert_eq!(args[0], tick_monitor::ntm_send_arg("demo"));
        assert_eq!(args[1], "--panes=%99");
        let value: Value = serde_json::from_str(args[2].strip_prefix("--msg=").unwrap()).unwrap();
        assert_eq!(value["sender"], "fast-dispatch");
        assert_eq!(value["target_pane"], "%5");
        assert_eq!(value["result_pane_id"], "%99");
        assert_eq!(value["surface"]["confidence"], "unquantified");
        assert_eq!(value["result_pane_resolved_at_unix"], 1_710_000_000);
    }

    #[test]
    fn failed_result_keeps_failure_outcome_for_ledger_and_notification() {
        let row = dispatch_result_row(
            "fast-dispatch",
            "demo",
            "%5",
            json!(["bead-1"]),
            "dispatch_transport_failed",
            concat!("ntm robot", "-send did not return success=true"),
            "pane_is_free",
            "free",
            "fast-dispatch::pane_is_free",
            Some("%99"),
            "resolved from pane index 1: %99",
            Some(1_710_000_000),
        );
        let value: Value = serde_json::from_str(&row).unwrap();
        assert_eq!(value["outcome"], "dispatch_transport_failed");
        assert_eq!(value["surface"]["name"], "pane_is_free");
    }
}

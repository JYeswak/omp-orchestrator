#![forbid(unsafe_code)]
//! `fleet-monitor` — ONE stateless fleet-wide attention tick.
//!
//! SCOPE OF THIS PORT (stated plainly, because a partial port that hides its seams is worse than
//! no port). RUST owns the parts that decide things and the parts that broke:
//!
//!   * the single-instance run lock, its holder naming, and the run deadline   (requirements C, D)
//!   * invoker lineage proof                                                    (uid+ppid, not argv)
//!   * the fleet attention wait, cursor persistence, and wake reason
//!   * pane liveness classification and the idle/ready scan
//!   * the admission verdict refresh — the publisher INVOCATION contract        (requirement A/B)
//!   * every ledger row and every stdout verdict line
//!
//! STILL SHELL, invoked as external binaries with stable CLI contracts (expected and fine per the
//! brief): `ntm`, `br`, `tmux`, `git`, `check.sh --publish`, `loop-queue-filter`, and the auxiliary
//! report lanes this tick rides (`storage-trend.sh`, `ci-orphan-reaper.sh`,
//! `agent-mail-log-cap.sh`, `tracked-conformance-scratch-gate.sh`, `charter-align.py`,
//! `mission-grade.sh`, `harvest-adopt.sh`, `selftest-sweep.sh`, `controller-tick.sh`). Those lanes
//! are pass-through: this binary runs them, records rc, and never reimplements their logic.
//!
//! THE GIT TOPOLOGY CENSUS remains in the shell for this pass. It is ~250 lines of report-only
//! observation that has not failed, and porting it in the same commit as the wedge fix would mix a
//! behaviour-preserving repair with a large rewrite. `--topology-only` delegates to the shell.

#[path = "dispatch_cli_contract.rs"]
mod dispatch_cli_contract;
use ntm_fleet_monitor::{parse_activity_json, ActivityError, ActivitySnapshot};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitCode, Output, Stdio};
use std::time::Duration;

#[path = "scheduled_lane_telemetry.rs"]
mod scheduled_lane_telemetry;

use fleet_monitor::{
    attention_end_cursor, attention_wake_reason, invoker_from_chain, json_escape, lock,
    ntm_list_census_line, ntm_list_is_empty, observe_scan_set, pane_liveness, parse_ancestor_rows,
    publish_failure_detail, publish_invocation, raw_open_count, safe_panes, FleetMonitorInvoker, LivenessState,
    ObserveRules, ObserveScan, RunDeadline, EXIT_CANNOT_OBSERVE,
};

const USAGE: &str = "usage: fleet-monitor [status [--json]|why [--json]|capabilities [--json]|robot-docs guide|--all|--self] [--dispatch|--report-only] [--selftest] [--topology-only]";

struct Cfg {
    scope_self: bool,
    dispatch: bool,
    cp_bin: PathBuf,
    state_dir: PathBuf,
    ledger: PathBuf,
    cursor_f: PathBuf,
    developer_root: PathBuf,
    timeout: String,
    ntm_bin: PathBuf,
    tmux_bin: PathBuf,
    tmux_tmpdir: PathBuf,
    ntm_activity_timeout: Duration,
    self_repo: String,
    invoker: FleetMonitorInvoker,
    deadline: RunDeadline,
    aux_lane_timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuxLaneOutcome {
    Completed { code: i32 },
    Signalled { signal: Option<i32> },
    DeadlineReached { code: Option<i32>, seconds: u64 },
    SpawnFailed { kind: std::io::ErrorKind },
}

#[derive(Debug)]
enum NtmActivityOutcome {
    Completed {
        code: Option<i32>,
        signal: Option<i32>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    TimedOut {
        seconds: u64,
        stdout_bytes: usize,
        stderr_bytes: usize,
    },
    SpawnFailed {
        kind: io::ErrorKind,
    },
    IoFailed {
        kind: io::ErrorKind,
    },
    OutputLimitExceeded {
        stream: &'static str,
        bytes: usize,
    },
    NoPanes,
    PaneEnumerationFailed {
        code: Option<i32>,
        signal: Option<i32>,
    },
    Malformed {
        detail: String,
        stdout_bytes: usize,
    },
}

impl AuxLaneOutcome {
    fn ledger_rc(self) -> i32 {
        match self {
            Self::Completed { code } => code,
            Self::Signalled { .. } | Self::DeadlineReached { code: None, .. } | Self::SpawnFailed { .. } => -1,
            Self::DeadlineReached { code: Some(code), .. } => code,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Completed { .. } => "completed",
            Self::Signalled { .. } => "signalled",
            Self::DeadlineReached { .. } => "deadline_reached",
            Self::SpawnFailed { .. } => "spawn_failed",
        }
    }

    fn is_failure(self) -> bool {
        !matches!(self, Self::Completed { code: 0 })
    }
}

fn env_or(k: &str, d: &str) -> String {
    std::env::var(k).unwrap_or_else(|_| d.to_string())
}

fn configured_ntm_bin() -> PathBuf {
    if let Some(path) = std::env::var_os("FLEET_NTM_BIN") {
        return PathBuf::from(path);
    }
    let installed = std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(|home| PathBuf::from(home).join(".local/bin/ntm"));
    if installed.as_ref().is_some_and(|path| path.is_file()) {
        installed.unwrap()
    } else {
        PathBuf::from("ntm")
    }
}

fn configured_tmux_bin() -> PathBuf {
    let installed = PathBuf::from("/opt/homebrew/bin/tmux");
    if installed.is_file() {
        installed
    } else {
        PathBuf::from("tmux")
    }
}

fn configured_tmux_tmpdir() -> PathBuf {
    if let Some(path) = std::env::var_os("FLEET_TMUX_TMPDIR").filter(|v| !v.is_empty()) {
        return PathBuf::from(path);
    }
    match std::env::var_os("HOME").filter(|v| !v.is_empty()).map(PathBuf::from) {
        Some(home) => home.join(".tmux-sockets"),
        None => {
            eprintln!("fleet-monitor: HOME is unset; cannot resolve the default tmux socket dir; set FLEET_TMUX_TMPDIR");
            std::process::exit(64);
        }
    }
}

/// Repository bin dir via upward `.git`/`.beads` marker walk from the cwd
/// (omp-orchestrator-npq, the omp-idle-dispatch mechanism). Loud typed failure
/// when no marker exists above the cwd — never a hardcoded checkout.
fn discovered_repo_bin() -> PathBuf {
    let mut current = std::env::current_dir().unwrap_or_else(|error| {
        eprintln!("fleet-monitor: cannot read the current directory: {error}");
        std::process::exit(64);
    });
    loop {
        if [".git", ".beads"].iter().any(|marker| current.join(marker).exists()) {
            return current.join("bin");
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => {
                eprintln!(
                    "fleet-monitor: no repository marker (.git or .beads) found at or above {}; set CP_BIN",
                    current.display()
                );
                std::process::exit(64);
            }
        }
    }
}

fn ts() -> String {
    // UTC ISO-8601 without pulling a date dependency: ask the system `date`, exactly as the shell
    // did, so the two implementations stamp rows identically.
    Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}

/// Every verdict line goes to STDOUT at column 0 (fh G1). stderr carries usage errors only.
fn say(msg: &str) {
    println!("{msg}");
    let _ = std::io::stdout().flush();
}

fn append_ledger(ledger: &Path, row: &str) {
    if let Some(p) = ledger.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(ledger) {
        let _ = writeln!(f, "{row}");
    }
}

impl Cfg {
    /// A ledger row carrying provenance on EVERY shape — a lane with 30+ row shapes must not have
    /// provable and unprovable rows.
    fn row(&self, event: &str, extra: &str) -> String {
        let base = format!(
            r#"{{"ts":"{}","invoker":"{}","invoker_proof":"{}","event":"{}""#,
            ts(),
            self.invoker.invoker,
            self.invoker.proof,
            event
        );
        if extra.is_empty() {
            format!("{base}}}")
        } else {
            format!("{base},{extra}}}")
        }
    }
    fn log(&self, event: &str, extra: &str) {
        append_ledger(&self.ledger, &self.row(event, extra));
    }
}

/// Walk the process ancestry and classify provenance by VERIFIED LINEAGE.
fn invoker_detect() -> FleetMonitorInvoker {
    let mut rows = String::new();
    let mut pid = std::os::unix::process::parent_id();
    for _ in 0..12 {
        if pid <= 1 {
            break;
        }
        let Ok(out) =
            Command::new("ps").arg("-p").arg(pid.to_string()).arg("-o").arg("uid=,ppid=,comm=").output()
        else {
            break;
        };
        let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if line.is_empty() {
            break;
        }
        rows.push_str(&line);
        rows.push('\n');
        let Some(next) = line.split_whitespace().nth(1).and_then(|s| s.parse::<u32>().ok()) else {
            break;
        };
        pid = next;
    }
    invoker_from_chain(&parse_ancestor_rows(&rows))
}

fn main() -> ExitCode {
    let _telemetry = scheduled_lane_telemetry::Run::new("fleet-monitor");
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = dispatch_cli_contract::handle("fleet-monitor", &raw_args) {
        return code;
    }
    let mut scope_self = false;
    let mut dispatch = false;
    let mut selftest = false;
    let mut topology_only = std::env::var("FLEET_TOPOLOGY_ONLY").ok().as_deref() == Some("1");

    for a in raw_args {
        match a.as_str() {
            "--self" => scope_self = true,
            "--all" => scope_self = false,
            "--dispatch" => dispatch = true,
            "--report-only" => dispatch = false,
            "--selftest" => selftest = true,
            "--topology-only" => topology_only = true,
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }

    let cp_bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(discovered_repo_bin);
    // The installed binary lives in bin/, so CP_BIN is its directory; when run from cargo target/
    // fall back to the repo's bin/ so the auxiliary lanes still resolve.
    let cp_bin = if cp_bin.join("check.sh").exists() {
        cp_bin
    } else {
        match std::env::var_os("CP_BIN").filter(|v| !v.is_empty()) {
            Some(path) => PathBuf::from(path),
            None => discovered_repo_bin(),
        }
    };

    let home = std::env::var_os("HOME").filter(|v| !v.is_empty()).map(PathBuf::from);
    let state_dir = match std::env::var_os("FLEET_STATE_DIR").filter(|v| !v.is_empty()) {
        Some(path) => PathBuf::from(path),
        None => match &home {
            Some(home) => home.join(".local/state/flywheel"),
            None => {
                eprintln!("fleet-monitor: HOME is unset; cannot resolve the default state dir; set FLEET_STATE_DIR");
                std::process::exit(64);
            }
        },
    };
    let developer_root = match std::env::var_os("FLEET_DEVELOPER_ROOT").filter(|v| !v.is_empty()) {
        Some(path) => PathBuf::from(path),
        None => match &home {
            Some(home) => home.join("Developer"),
            None => {
                eprintln!("fleet-monitor: HOME is unset; cannot resolve the default developer root; set FLEET_DEVELOPER_ROOT");
                std::process::exit(64);
            }
        },
    };
    let self_repo = cp_bin
        .parent()
        .and_then(|p| p.file_name())
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "control-plane".to_string());

    let cfg = Cfg {
        scope_self,
        dispatch,
        ledger: state_dir.join("fleet-monitor.jsonl"),
        cursor_f: state_dir.join("fleet-monitor.cursor"),
        timeout: env_or("FLEET_TIMEOUT", "300s"),
        ntm_bin: configured_ntm_bin(),
        tmux_bin: configured_tmux_bin(),
        tmux_tmpdir: configured_tmux_tmpdir(),
        ntm_activity_timeout: Duration::from_secs(
            env_or("FLEET_NTM_ACTIVITY_TIMEOUT_SECONDS", "30")
                .parse::<u64>()
                .ok()
                .filter(|seconds| *seconds > 0)
                .unwrap_or(30),
        ),
        developer_root,
        self_repo,
        invoker: invoker_detect(),
        // REQUIREMENT C: the run's own wall-clock deadline. A wedged instance held the lock for
        // 2h21m at 0.0% CPU today and silenced the lane; a bounded run cannot do that.
        deadline: RunDeadline::new(Duration::from_secs(
            env_or("FLEET_RUN_DEADLINE_SECONDS", "1500").parse().unwrap_or(1500),
        )),
        aux_lane_timeout: Duration::from_secs(
            env_or("FLEET_AUX_LANE_TIMEOUT_SECONDS", "120")
                .parse::<u64>()
                .ok()
                .filter(|seconds| *seconds > 0)
                .unwrap_or(120),
        ),
        state_dir,
        cp_bin,
    };
    let _ = std::fs::create_dir_all(&cfg.state_dir);

    if selftest {
        return selftest_run(&cfg);
    }

    // ── SINGLE-INSTANCE RUN LOCK ───────────────────────────────────────────────────────────
    // REFUSAL EXITS 0. A skipped tick is the NORMAL outcome under load; a nonzero exit would make
    // it a cron error and train the operator to ignore the signal. The skip is a TYPED ROW.
    let lock_path = cfg.state_dir.join("fleet-monitor.run.lock");
    let _guard = match lock::acquire(&lock_path, &lock::OsHolderLookup) {
        lock::LockOutcome::Acquired(g) => g,
        lock::LockOutcome::Busy { holder_pid, holder_elapsed } => {
            say(&format!(
                "[{}] CONCURRENT_RUN_SKIPPED — fleet-monitor already running (pid={holder_pid} elapsed={holder_elapsed}); refusing to stack. rc=0",
                ts()
            ));
            cfg.log(
                "concurrent_run_skipped",
                &format!(
                    r#""script":"fleet-monitor","holder_pid":"{}","holder_elapsed":"{}","lock":"{}""#,
                    json_escape(&holder_pid),
                    json_escape(&holder_elapsed),
                    json_escape(&lock_path.display().to_string())
                ),
            );
            return ExitCode::SUCCESS;
        }
        lock::LockOutcome::Unusable { reason } => {
            say(&format!(
                "[{}] CONCURRENT_RUN_SKIPPED — run lock {} unusable; refusing to run unserialized (fail closed). rc=0",
                ts(),
                lock_path.display()
            ));
            cfg.log(
                "concurrent_run_skipped",
                &format!(
                    r#""script":"fleet-monitor","holder_pid":"unknown","holder_elapsed":"unknown","lock":"{}","reason":"lock_unusable","detail":"{}""#,
                    json_escape(&lock_path.display().to_string()),
                    json_escape(&reason)
                ),
            );
            return ExitCode::SUCCESS;
        }
    };

    let repos = match resolve_repos(&cfg) {
        Ok(repos) => repos,
        Err(reason) => {
            say(&format!(
                "[{}] CANNOT_OBSERVE code={EXIT_CANNOT_OBSERVE} reason={reason}",
                ts()
            ));
            cfg.log(
                "cannot_observe",
                &format!(r#""reason":"{}","code":{EXIT_CANNOT_OBSERVE}"#, json_escape(reason)),
            );
            return ExitCode::from(EXIT_CANNOT_OBSERVE as u8);
        }
    };

    // Git topology stays in the shell this pass (see module docs) and is a bounded diagnostic.
    if topology_only {
        let rc = run_shell_lane(&cfg, "fleet-monitor.sh", &[], &[("FLEET_TOPOLOGY_ONLY", "1")]);
        return if rc == 0 { ExitCode::SUCCESS } else { ExitCode::from(rc as u8) };
    }

    // ── AUXILIARY REPORT LANES (pass-through) ──────────────────────────────────────────────
    aux_lane(&cfg, "ci-orphan-reaper.sh", &["--report"], "ci_orphan_reaper_report");
    aux_lane(&cfg, "agent-mail-log-cap.sh", &["--apply"], "agent_mail_log_cap");

    // ── ADMISSION VERDICT REFRESH ──────────────────────────────────────────────────────────
    admission_refresh(&cfg);

    aux_lane(&cfg, "storage-trend.sh", &[], "storage_trend");

    if cfg.deadline.expired() {
        say(&format!("[{}] RUN_DEADLINE reached before the attention wait — ending this tick early so the next slot re-observes fresher state", ts()));
        cfg.log("run_deadline_reached", r#""phase":"pre_attention""#);
        return ExitCode::SUCCESS;
    }

    // ── 1. BLOCK on the whole fleet ────────────────────────────────────────────────────────
    let wake = attention_wait(&cfg);
    say(&format!("[{}] fleet wait returned: wake_reason={wake}", ts()));

    // ── 2. IDLE PANES WITH READY WORK ──────────────────────────────────────────────────────
    let (found, liveness_blocked) = idle_scan(&cfg, &repos);

    if found == 0 {
        if liveness_blocked > 0 {
            say(&format!(
                "[{}] no live idle-pane/ready-work pairs; liveness blocked {liveness_blocked} candidate pane(s).",
                ts()
            ));
            cfg.log("fleet_liveness_blocked", &format!(r#""panes":{liveness_blocked}"#));
        } else {
            say(&format!(
                "[{}] no idle-pane/ready-work pairs. Fleet is either busy or genuinely drained.",
                ts()
            ));
            cfg.log("fleet_clear", "");
        }
    }

    // ── 3. DISPATCH HANDOFF ────────────────────────────────────────────────────────────────
    // This surface NEVER sends directly and never bypasses readiness, cargo, check, or the pane
    // fence: it hands the decision to controller-tick.sh, the existing admission-gated sender.
    if cfg.dispatch && found > 0 {
        dispatch_handoff(&cfg, found);
    }

    // ── 4. TRAILING REPORT LANES ───────────────────────────────────────────────────────────
    for repo in &repos {
        let _ = run_capture(
            Command::new("timeout")
                .arg("120")
                .arg("python3")
                .arg(cfg.cp_bin.join("charter-align.py"))
                .arg(repo),
        );
    }
    aux_lane(&cfg, "mission-grade.sh", &["--due"], "daily_mission_grade");
    aux_lane(&cfg, "harvest-adopt.sh", &["--due"], "stamp_adoption");

    ExitCode::SUCCESS
}

fn resolve_repos(cfg: &Cfg) -> Result<Vec<String>, &'static str> {
    if let Ok(list) = std::env::var("FLEET_REPOS") {
        return Ok(list.split_whitespace().map(str::to_string).collect());
    }
    if cfg.scope_self {
        return Ok(vec![cfg.self_repo.clone()]);
    }
    let out = run_capture(
        Command::new(&cfg.ntm_bin)
            .env("TMUX_TMPDIR", &cfg.tmux_tmpdir)
            .arg("list"),
    );
    match observe_scan_set(&out, &cfg.developer_root, ObserveRules::from_env()) {
        ObserveScan::CannotObserve { reason } => Err(reason),
        ObserveScan::Repos(repos) => Ok(repos),
    }
}

fn run_capture(cmd: &mut Command) -> String {
    cmd.output()
        .map(|o| {
            let mut s = String::from_utf8_lossy(&o.stdout).to_string();
            s.push_str(&String::from_utf8_lossy(&o.stderr));
            s
        })
        .unwrap_or_default()
}

const NTM_ACTIVITY_OUTPUT_LIMIT: usize = 16 * 1024 * 1024;

#[derive(Debug)]
enum PipeReadOutcome {
    Complete(Vec<u8>),
    LimitExceeded { stream: &'static str, bytes: usize },
    Failed { kind: io::ErrorKind },
}

fn read_pipe<R: Read>(reader: R, stream: &'static str) -> PipeReadOutcome {
    let mut bytes = Vec::new();
    match reader
        .take((NTM_ACTIVITY_OUTPUT_LIMIT + 1) as u64)
        .read_to_end(&mut bytes)
    {
        Ok(_) if bytes.len() > NTM_ACTIVITY_OUTPUT_LIMIT => {
            PipeReadOutcome::LimitExceeded { stream, bytes: bytes.len() }
        }
        Ok(_) => PipeReadOutcome::Complete(bytes),
        Err(error) => PipeReadOutcome::Failed { kind: error.kind() },
    }
}

fn join_pipe(
    reader: std::thread::JoinHandle<PipeReadOutcome>,
) -> Result<Vec<u8>, NtmActivityOutcome> {
    match reader.join() {
        Ok(PipeReadOutcome::Complete(bytes)) => Ok(bytes),
        Ok(PipeReadOutcome::LimitExceeded { stream, bytes }) => {
            Err(NtmActivityOutcome::OutputLimitExceeded { stream, bytes })
        }
        Ok(PipeReadOutcome::Failed { kind }) => Err(NtmActivityOutcome::IoFailed { kind }),
        Err(_) => Err(NtmActivityOutcome::IoFailed {
            kind: io::ErrorKind::Other,
        }),
    }
}

/// Stop the probe's process group, then reap the direct child.
///
/// NTM is an external process and may retain descendants that inherit the
/// capture pipes. Killing only the direct child would leave the reader joins
/// unbounded. `process_group(0)` gives this invocation its own group; the
/// platform `kill` utility is used here because this crate forbids unsafe code.
fn terminate_and_reap(child: &mut Child) -> Result<(), NtmActivityOutcome> {
    let group = Command::new("/bin/kill")
        .args(["-KILL", &format!("-{}", child.id())])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| NtmActivityOutcome::IoFailed { kind: error.kind() })?;
    if !group.success() {
        let _ = child.kill();
        return child
            .wait()
            .map(|_| ())
            .map_err(|error| NtmActivityOutcome::IoFailed { kind: error.kind() });
    }
    child
        .wait()
        .map(|_| ())
        .map_err(|error| NtmActivityOutcome::IoFailed { kind: error.kind() })
}

fn join_pipes(
    stdout_reader: std::thread::JoinHandle<PipeReadOutcome>,
    stderr_reader: std::thread::JoinHandle<PipeReadOutcome>,
) -> Result<(Vec<u8>, Vec<u8>), NtmActivityOutcome> {
    let stdout = join_pipe(stdout_reader);
    let stderr = join_pipe(stderr_reader);
    match (stdout, stderr) {
        (Ok(stdout), Ok(stderr)) => Ok((stdout, stderr)),
        (Err(outcome), _) | (_, Err(outcome)) => Err(outcome),
    }
}

fn pane_selector(cfg: &Cfg, repo: &str) -> Result<String, NtmActivityOutcome> {
    let target = format!("{repo}:0");
    let output = Command::new(&cfg.tmux_bin)
        .env("TMUX_TMPDIR", &cfg.tmux_tmpdir)
        .args(["list-panes", "-t", &target, "-F", "#{pane_index}"])
        .output()
        .map_err(|error| NtmActivityOutcome::SpawnFailed { kind: error.kind() })?;
    if !output.status.success() {
        return Err(NtmActivityOutcome::PaneEnumerationFailed {
            code: output.status.code(),
            signal: std::os::unix::process::ExitStatusExt::signal(&output.status),
        });
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let panes: Vec<_> = text
        .lines()
        .map(str::trim)
        .filter(|pane| !pane.is_empty() && pane.chars().all(|c| c.is_ascii_digit()))
        .collect();
    if panes.is_empty() {
        return Err(NtmActivityOutcome::NoPanes);
    }
    Ok(panes.join(","))
}

/// Run the NTM activity probe with a real wall bound and both pipes drained concurrently.
/// `Command::output()` is not sufficient here: a chatty probe can fill one pipe while the parent
/// polls the child, recreating the pipe deadlock this monitor is meant to report rather than hide.
fn run_ntm_activity(cfg: &Cfg, repo: &str) -> NtmActivityOutcome {
    let selectors = match pane_selector(cfg, repo) {
        Ok(selectors) => selectors,
        Err(outcome) => return outcome,
    };
    let mut command = Command::new(&cfg.ntm_bin);
    command
        .arg(format!("--robot-activity={repo}"))
        .arg("--panes")
        .arg(selectors)
        .env("TMUX_TMPDIR", &cfg.tmux_tmpdir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.process_group(0);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => return NtmActivityOutcome::SpawnFailed { kind: error.kind() },
    };
    let Some(stdout) = child.stdout.take() else {
        let _ = terminate_and_reap(&mut child);
        return NtmActivityOutcome::IoFailed {
            kind: io::ErrorKind::Other,
        };
    };
    let Some(stderr) = child.stderr.take() else {
        let _ = terminate_and_reap(&mut child);
        return NtmActivityOutcome::IoFailed {
            kind: io::ErrorKind::Other,
        };
    };
    let stdout_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        match stdout
            .take((NTM_ACTIVITY_OUTPUT_LIMIT + 1) as u64)
            .read_to_end(&mut bytes)
        {
            Ok(_) if bytes.len() > NTM_ACTIVITY_OUTPUT_LIMIT => {
                PipeReadOutcome::LimitExceeded { stream: "stdout", bytes: bytes.len() }
            }
            Ok(_) => PipeReadOutcome::Complete(bytes),
            Err(error) => PipeReadOutcome::Failed { kind: error.kind() },
        }
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        match stderr
            .take((NTM_ACTIVITY_OUTPUT_LIMIT + 1) as u64)
            .read_to_end(&mut bytes)
        {
            Ok(_) if bytes.len() > NTM_ACTIVITY_OUTPUT_LIMIT => {
                PipeReadOutcome::LimitExceeded { stream: "stderr", bytes: bytes.len() }
            }
            Ok(_) => PipeReadOutcome::Complete(bytes),
            Err(error) => PipeReadOutcome::Failed { kind: error.kind() },
        }
    });

    let deadline = std::time::Instant::now() + cfg.ntm_activity_timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Ok(None) => {
                if let Err(outcome) = terminate_and_reap(&mut child) {
                    return outcome;
                }
                let (stdout, stderr) = match join_pipes(stdout_reader, stderr_reader) {
                    Ok(pipes) => pipes,
                    Err(outcome) => return outcome,
                };
                return NtmActivityOutcome::TimedOut {
                    seconds: cfg.ntm_activity_timeout.as_secs().max(1),
                    stdout_bytes: stdout.len(),
                    stderr_bytes: stderr.len(),
                };
            }
            Err(error) => {
                if let Err(outcome) = terminate_and_reap(&mut child) {
                    return outcome;
                }
                let _ = join_pipes(stdout_reader, stderr_reader);
                return NtmActivityOutcome::IoFailed { kind: error.kind() };
            }
        }
    };
    let (stdout, stderr) = match join_pipes(stdout_reader, stderr_reader) {
        Ok(pipes) => pipes,
        Err(outcome) => return outcome,
    };
    NtmActivityOutcome::Completed {
        code: status.code(),
        signal: std::os::unix::process::ExitStatusExt::signal(&status),
        stdout,
        stderr,
    }
}

fn typed_safe_panes(activity: &str) -> Result<(Vec<String>, ActivitySnapshot), ActivityError> {
    let typed = parse_activity_json(activity)?;
    let omp_panes: std::collections::BTreeSet<_> = typed
        .agents
        .iter()
        .filter(|agent| agent.kind.is_omp_family())
        .map(|agent| agent.pane.clone())
        .collect();
    let mut panes: Vec<_> = safe_panes(activity)
        .into_iter()
        .filter(|pane| !omp_panes.contains(pane))
        .collect();
    panes.extend(
        typed
            .omp_agents()
            .filter(|agent| agent.capture_eligible())
            .map(|agent| agent.pane.clone()),
    );
    panes.sort();
    panes.dedup();
    Ok((panes, typed))
}

fn record_omp_readiness(cfg: &Cfg, repo: &str, snapshot: &ActivitySnapshot) {
    for agent in snapshot.omp_agents() {
        say(&format!(
            "  OMP         {repo} pane {} agent={} readiness={} freshness={} safe_to_dispatch={} dispatchable={}",
            agent.pane,
            agent.kind.as_str(),
            agent.readiness.as_str(),
            agent.freshness.as_str(),
            agent.safe_to_dispatch,
            agent.dispatchable()
        ));
        cfg.log(
            "omp_readiness",
            &format!(
                r#""repo":"{}","pane":"{}","agent":"{}","readiness":"{}","freshness":"{}","safe_to_dispatch":{},"dispatchable":{}"#,
                json_escape(repo),
                json_escape(&agent.pane),
                json_escape(agent.kind.as_str()),
                agent.readiness.as_str(),
                agent.freshness.as_str(),
                agent.safe_to_dispatch,
                agent.dispatchable()
            ),
        );
    }
}

fn record_ntm_activity_failure(cfg: &Cfg, repo: &str, outcome: &NtmActivityOutcome) {
    let (kind, detail) = match outcome {
        NtmActivityOutcome::Completed {
            code,
            signal,
            stdout,
            stderr,
        } => (
            "completed_nonzero",
            format!(
                r#""code":{},"signal":{},"stdout_bytes":{},"stderr_bytes":{}"#,
                code.map_or_else(|| "null".to_string(), |value| value.to_string()),
                signal.map_or_else(|| "null".to_string(), |value| value.to_string()),
                stdout.len(),
                stderr.len()
            ),
        ),
        NtmActivityOutcome::TimedOut {
            seconds,
            stdout_bytes,
            stderr_bytes,
        } => (
            "timeout",
            format!(r#""seconds":{seconds},"stdout_bytes":{stdout_bytes},"stderr_bytes":{stderr_bytes}"#),
        ),
        NtmActivityOutcome::SpawnFailed { kind } => (
            "spawn_failed",
            format!(r#""error_kind":"{:?}""#, kind),
        ),
        NtmActivityOutcome::IoFailed { kind } => (
            "io_failed",
            format!(r#""error_kind":"{:?}""#, kind),
        ),
        NtmActivityOutcome::OutputLimitExceeded { stream, bytes } => (
            "output_limit_exceeded",
            format!(r#""stream":"{stream}","bytes":{bytes}"#),
        ),
        NtmActivityOutcome::NoPanes => ("no_panes", String::new()),
        NtmActivityOutcome::PaneEnumerationFailed { code, signal } => (
            "pane_enumeration_failed",
            format!(
                r#""code":{},"signal":{}"#,
                code.map_or_else(|| "null".to_string(), |value| value.to_string()),
                signal.map_or_else(|| "null".to_string(), |value| value.to_string())
            ),
        ),
        NtmActivityOutcome::Malformed {
            detail,
            stdout_bytes,
        } => (
            "malformed",
            format!(
                r#""detail":"{}","stdout_bytes":{}"#,
                json_escape(detail),
                stdout_bytes
            ),
        ),
    };
    say(&format!("  OMP         {repo}: readiness unavailable ({kind})"));
    cfg.log(
        "omp_readiness_unproven",
        &format!(r#""repo":"{}","reason":"{}",{}"#, json_escape(repo), kind, detail),
    );
}

/// Run an auxiliary shell lane and record its rc. Pass-through: never reimplements its logic.
fn aux_lane(cfg: &Cfg, script: &str, args: &[&str], event: &str) {
    let bin = cfg.cp_bin.join(script);
    if !bin.exists() {
        say(&format!("[{}] {event} UNRUN: {} is absent", ts(), bin.display()));
        cfg.log(&format!("{event}_unrun"), "");
        return;
    }
    let wrapper = cfg.cp_bin.join("lib/scheduled-lane-run.sh");
    if !wrapper.is_file() {
        say(&format!(
            "[{}] {event} UNRUN: auxiliary deadline wrapper is absent at {}",
            ts(),
            wrapper.display()
        ));
        cfg.log(&format!("{event}_unrun"), "\"reason\":\"auxiliary_deadline_wrapper_missing\"");
        return;
    }
    let Some(seconds) = aux_lane_budget_seconds(cfg.deadline.remaining(), cfg.aux_lane_timeout) else {
        say(&format!("[{}] {event} skipped — fleet run deadline exhausted", ts()));
        cfg.log(&format!("{event}_skipped_deadline"), "\"reason\":\"run_deadline_exhausted\"");
        return;
    };
    let lane = format!("fleet-monitor-aux-{event}");
    let out = Command::new(&wrapper)
        .arg("--lane")
        .arg(&lane)
        .arg("--deadline")
        .arg(seconds.to_string())
        .arg("--")
        .arg(&bin)
        .args(args)
        .env("FLEET_STATE_DIR", &cfg.state_dir)
        .output();
    let outcome = classify_aux_lane_outcome(&out, seconds);
    if let Ok(o) = &out {
        let mut text = String::from_utf8_lossy(&o.stdout).to_string();
        text.push_str(&String::from_utf8_lossy(&o.stderr));
        if !text.trim().is_empty() {
            print!("{text}");
            let _ = std::io::stdout().flush();
        }
    }
    let extra = match outcome {
        AuxLaneOutcome::Completed { .. } | AuxLaneOutcome::DeadlineReached { .. } => {
            format!(r#""rc":{},"outcome":"{}""#, outcome.ledger_rc(), outcome.label())
        }
        AuxLaneOutcome::Signalled { signal } => format!(
            r#""rc":{},"outcome":"{}","signal":{}"#,
            outcome.ledger_rc(),
            outcome.label(),
            signal.map_or_else(|| "null".to_string(), |value| value.to_string())
        ),
        AuxLaneOutcome::SpawnFailed { kind } => format!(
            r#""rc":{},"outcome":"{}","error_kind":"{:?}""#,
            outcome.ledger_rc(),
            outcome.label(),
            kind
        ),
    };
    cfg.log(event, &extra);
    if let AuxLaneOutcome::DeadlineReached { seconds, .. } = outcome {
        cfg.log(
            "aux_lane_deadline_reached",
            &format!(r#""event_name":"{}","deadline_seconds":{}"#, json_escape(event), seconds),
        );
    }
    if outcome.is_failure() {
        say(&format!("[{}] {event} rc={}", ts(), outcome.ledger_rc()));
    }
}

fn classify_aux_lane_outcome(result: &std::io::Result<Output>, seconds: u64) -> AuxLaneOutcome {
    match result {
        Err(error) => AuxLaneOutcome::SpawnFailed { kind: error.kind() },
        Ok(output) => {
            let mut text = String::from_utf8_lossy(&output.stdout).to_string();
            text.push_str(&String::from_utf8_lossy(&output.stderr));
            if text.lines().any(|line| line.starts_with("DEADLINE lane=")) {
                return AuxLaneOutcome::DeadlineReached {
                    code: output.status.code(),
                    seconds,
                };
            }
            match output.status.code() {
                Some(code) => AuxLaneOutcome::Completed { code },
                None => AuxLaneOutcome::Signalled {
                    signal: std::os::unix::process::ExitStatusExt::signal(&output.status),
                },
            }
        }
    }
}

fn aux_lane_budget_seconds(remaining: Duration, configured: Duration) -> Option<u64> {
    let seconds = remaining.as_secs().min(configured.as_secs());
    (seconds > 0).then_some(seconds)
}

fn run_shell_lane(cfg: &Cfg, script: &str, args: &[&str], envs: &[(&str, &str)]) -> i32 {
    let mut c = Command::new("/bin/bash");
    c.arg(cfg.cp_bin.join(script)).args(args);
    for (k, v) in envs {
        c.env(k, v);
    }
    c.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    c.status().ok().and_then(|s| s.code()).unwrap_or(-1)
}

/// ⛔ THE PUBLISH INVOCATION. NO OUTER TIMEOUT — see the crate docs. `check.sh --publish` owns the
/// private-run + complete-candidate + atomic-promotion contract and its own deadline; an outer
/// kill produces no ledger row at all and was measured to kill healthy in-budget runs.
fn admission_refresh(cfg: &Cfg) {
    let check_sh = cfg.cp_bin.join("check.sh");
    if !check_sh.exists() {
        say(&format!("[{}] ADMISSION_VERDICT refresh UNRUN: {} is absent", ts(), check_sh.display()));
        return;
    }
    let fresh: u64 = env_or("FM_STANDING_FRESH_SECONDS", "1500").parse().unwrap_or(1500);
    let deadline: u64 = env_or("FM_ADMISSION_REFRESH_DEADLINE_SECONDS", &fresh.to_string())
        .parse()
        .unwrap_or(fresh);
    let inv = publish_invocation(&check_sh, &cfg.state_dir, &cfg.ledger, fresh, deadline);

    let mut cmd = Command::new(&inv.bin);
    cmd.args(&inv.args);
    for (k, v) in &inv.env {
        cmd.env(k, v);
    }
    let out = cmd.output();
    let rc = out.as_ref().map(|o| o.status.code().unwrap_or(-1)).unwrap_or(-1);
    let text = out
        .as_ref()
        .map(|o| {
            let mut s = String::from_utf8_lossy(&o.stdout).to_string();
            s.push_str(&String::from_utf8_lossy(&o.stderr));
            s
        })
        .unwrap_or_default();

    cfg.log("admission_verdict_refresh", &format!(r#""rc":{rc}"#));
    match rc {
        0 => say(&format!("[{}] ADMISSION_VERDICT refreshed PASS", ts())),
        75 => say(&format!(
            "[{}] ADMISSION_VERDICT refresh skipped — lock held by another run (standing verdict unchanged)",
            ts()
        )),
        _ => {
            say(&format!("[{}] ADMISSION_VERDICT refresh rc={rc}", ts()));
            // KEEP THE REASON, NOT JUST THE VERDICT.
            let detail = publish_failure_detail(&text);
            if !detail.trim().is_empty() {
                say(&detail);
            }
        }
    }
}

fn attention_wait(cfg: &Cfg) -> String {
    let cursor = std::fs::read_to_string(&cfg.cursor_f).unwrap_or_default().trim().to_string();
    let mut cmd = Command::new("timeout");
    cmd.arg("400")
        .arg(&cfg.ntm_bin)
        .env("TMUX_TMPDIR", &cfg.tmux_tmpdir)
        .arg("--robot-attention")
        .arg("--attention-condition=action_required");
    if !cursor.is_empty() {
        cmd.arg(format!("--attention-cursor={cursor}"));
    }
    // --timeout is mandatory: --attention-timeout is IGNORED by the wait and it silently falls
    // back to its own 5m default.
    cmd.arg(format!("--timeout={}", cfg.timeout));
    let att = run_capture(&mut cmd);

    let _ = std::fs::write(cfg.state_dir.join("fleet-attention.json"), &att);
    let next = attention_end_cursor(&att);
    if !next.is_empty() {
        let _ = std::fs::write(&cfg.cursor_f, &next);
    }
    attention_wake_reason(&att)
}

fn idle_scan(cfg: &Cfg, repos: &[String]) -> (u64, u64) {
    say(&format!("[{}] scanning for idle panes beside ready work:", ts()));
    let mut found = 0u64;
    let mut liveness_blocked = 0u64;

    for repo in repos {
        if cfg.deadline.expired() {
            cfg.log("run_deadline_reached", r#""phase":"idle_scan""#);
            say(&format!("[{}] RUN_DEADLINE reached mid-scan — remaining repos deferred to the next slot", ts()));
            break;
        }
        let d = cfg.developer_root.join(repo);
        if !d.is_dir() {
            continue;
        }
        let activity = run_ntm_activity(cfg, repo);
        let panes = match &activity {
            NtmActivityOutcome::Completed { code: Some(0), stdout, .. } => {
                let activity = String::from_utf8_lossy(stdout);
                match typed_safe_panes(&activity) {
                    Ok((panes, snapshot)) => {
                        record_omp_readiness(cfg, repo, &snapshot);
                        panes
                    }
                    Err(error) => {
                        record_ntm_activity_failure(
                            cfg,
                            repo,
                            &NtmActivityOutcome::Malformed {
                                detail: error.to_string(),
                                stdout_bytes: stdout.len(),
                            },
                        );
                        say(&format!("  OMP         {repo}: typed readiness refused ({error})"));
                        Vec::new()
                    }
                }
            }
            outcome => {
                record_ntm_activity_failure(cfg, repo, outcome);
                Vec::new()
            }
        };

        let mut idle = 0u64;
        let mut busy = 0u64;
        let mut wedged = 0u64;
        let mut unproven = 0u64;
        for pane in &panes {
            let text = run_capture(
                Command::new(&cfg.tmux_bin)
                    .env("TMUX_TMPDIR", &cfg.tmux_tmpdir)
                    .arg("capture-pane")
                    .arg("-p")
                    .arg("-t")
                    .arg(format!("{repo}:0.{pane}"))
                    .arg("-S")
                    .arg("-20"),
            );
            let l = pane_liveness(&text);
            match l.state {
                LivenessState::Live => idle += 1,
                LivenessState::Busy => {
                    busy += 1;
                    liveness_blocked += 1;
                    say(&format!("  BUSY        {repo} pane {pane}: {}", l.reason));
                    cfg.log(
                        "pane_busy",
                        &format!(
                            r#""repo":"{}","pane":"{}","reason":"{}""#,
                            json_escape(repo),
                            json_escape(pane),
                            l.reason
                        ),
                    );
                }
                LivenessState::Wedged => {
                    wedged += 1;
                    liveness_blocked += 1;
                    say(&format!("  WEDGED      {repo} pane {pane}: {}", l.reason));
                    cfg.log(
                        "pane_wedged",
                        &format!(
                            r#""repo":"{}","pane":"{}","reason":"{}""#,
                            json_escape(repo),
                            json_escape(pane),
                            l.reason
                        ),
                    );
                }
                LivenessState::Unproven => {
                    unproven += 1;
                    liveness_blocked += 1;
                    say(&format!("  UNPROVEN    {repo} pane {pane}: {}", l.reason));
                    cfg.log(
                        "pane_liveness_unproven",
                        &format!(
                            r#""repo":"{}","pane":"{}","reason":"{}""#,
                            json_escape(repo),
                            json_escape(pane),
                            l.reason
                        ),
                    );
                }
            }
        }
        if idle == 0 {
            continue;
        }

        // COUNT THROUGH THE POLICY FILTER, not raw `br ready`. Two copies of the policy is two
        // policies: they drift, and a monitor is exactly where drift stays invisible.
        let br_json = run_capture(
            Command::new("timeout")
                .arg("90")
                .arg("br")
                .arg("ready")
                .arg("--limit")
                .arg("0")
                .arg("--json")
                .current_dir(&d),
        );
        let raw = raw_open_count(&br_json);
        let ready = queue_filter_count(cfg, &d, &br_json);
        let parked = raw.saturating_sub(ready);

        if ready > 0 {
            if parked > 0 || busy > 0 || wedged > 0 || unproven > 0 {
                say(&format!(
                    "  ACTIONABLE  {repo}: {idle} live idle pane(s), {ready} dispatchable ({parked} parked, {busy} busy, {wedged} wedged, {unproven} unproven)"
                ));
            } else {
                say(&format!("  ACTIONABLE  {repo}: {idle} live idle pane(s), {ready} dispatchable"));
            }
            cfg.log(
                "idle_with_work",
                &format!(
                    r#""repo":"{}","idle_panes":{idle},"dispatchable":{ready},"parked":{parked},"wedged":{wedged},"unproven":{unproven}"#,
                    json_escape(repo)
                ),
            );
            found += 1;
        } else if parked > 0 {
            // NOT "queue empty". A queue holding only parked work is BLOCKED, and calling that
            // drained is how a starved lane looks healthy.
            say(&format!(
                "  BLOCKED     {repo}: {idle} idle pane(s), 0 dispatchable, {parked} parked on a blocker"
            ));
            cfg.log(
                "queue_all_parked",
                &format!(r#""repo":"{}","parked":{parked}"#, json_escape(repo)),
            );
        } else {
            say(&format!("  ok          {repo}: {idle} idle pane(s), queue genuinely empty"));
        }
    }
    (found, liveness_blocked)
}

/// Count dispatchable beads through THE SHARED policy filter (installed binary preferred, the
/// python original as fallback) — never a second copy of the policy.
fn queue_filter_count(cfg: &Cfg, repo_dir: &Path, br_json: &str) -> u64 {
    let installed = std::env::var_os("LOOP_QUEUE_FILTER_BIN")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .map(|home| PathBuf::from(home).join(".local/bin/loop-queue-filter"))
        });
    let mut cmd = match installed {
        Some(path) if path.is_file() => {
            let mut c = Command::new(&path);
            c.arg("--count").arg("");
            c
        }
        _ => {
            let mut c = Command::new("python3");
            c.arg(cfg.cp_bin.join("loop-queue-filter.py")).arg("--count").arg("");
            c
        }
    };
    cmd.current_dir(repo_dir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    let Ok(mut child) = cmd.spawn() else { return 0 };
    if let Some(mut si) = child.stdin.take() {
        let _ = si.write_all(br_json.as_bytes());
    }
    let Ok(out) = child.wait_with_output() else { return 0 };
    String::from_utf8_lossy(&out.stdout).trim().parse::<u64>().unwrap_or(0)
}

fn dispatch_handoff(cfg: &Cfg, found: u64) {
    let dispatch_log = cfg.state_dir.join("fleet-monitor-dispatch.log");
    say(&format!("[{}] actionable fleet state found — invoking gated controller tick", ts()));
    let t = env_or("FLEET_DISPATCH_TIMEOUT", "900");
    let rc = Command::new("timeout")
        .arg(&t)
        .arg("/bin/bash")
        .arg(cfg.cp_bin.join("controller-tick.sh"))
        .stdout(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&dispatch_log)
                .map(Stdio::from)
                .unwrap_or_else(|_| Stdio::null()),
        )
        .stderr(Stdio::null())
        .status()
        .ok()
        .and_then(|s| s.code())
        .unwrap_or(-1);
    cfg.log(
        "dispatch_handoff",
        &format!(
            r#""actionable_repos":{found},"rc":{rc},"log":"{}""#,
            json_escape(&dispatch_log.display().to_string())
        ),
    );
    if rc == 0 {
        say(&format!("[{}] controller tick returned rc=0; inspect its ledger for delivered panes", ts()));
    } else {
        say(&format!("[{}] controller tick FAILED rc={rc} — see {}", ts(), dispatch_log.display()));
    }
}

fn selftest_run(cfg: &Cfg) -> ExitCode {
    // A --selftest that only checks its own logic is a claim. Check the things whose ABSENCE has
    // actually broken this lane: the publisher, the policy filter, and the lock directory.
    let check_sh = cfg.cp_bin.join("check.sh");
    if !check_sh.exists() {
        say(&format!("selftest: FAIL — publisher missing at {}", check_sh.display()));
        return ExitCode::FAILURE;
    }
    let inv = publish_invocation(&check_sh, &cfg.state_dir, &cfg.ledger, 1500, 1500);
    let env: std::collections::HashMap<_, _> = inv.env.iter().cloned().collect();
    if env["CHECK_SH_LEDGER"] == env["CHECK_SH_PUBLISH_LEDGER"] {
        say("selftest: FAIL — the private run ledger must differ from the published one");
        return ExitCode::FAILURE;
    }
    // KNOWN-BAD PROBE: the classifier must refuse to call an unmarked pane live.
    if pane_liveness("nothing here").state != LivenessState::Unproven {
        say("selftest: FAIL — liveness classifier called an unmarked pane live");
        return ExitCode::FAILURE;
    }
    if pane_liveness("Press up to edit queued messages").state != LivenessState::Wedged {
        say("selftest: FAIL — liveness classifier missed a wedged pane");
        return ExitCode::FAILURE;
    }
    if pane_liveness("⠋ Working").state != LivenessState::Busy
        || pane_liveness("⠋ Working").reason != "omp_working_marker"
        || pane_liveness("history\n╰─").reason != "omp_prompt_footer"
        || pane_liveness("⠋ 2m\n⎋ Awaiting work\n╰─").state != LivenessState::Busy
    {
        say("selftest: FAIL — liveness classifier missed the OMP dialect");
        return ExitCode::FAILURE;
    }

    // L4 / C95: empty ntm list is CANNOT_OBSERVE, never fleet_clear.
    let planted_empty = "No tmux sessions running\n";
    if ntm_list_census_line(planted_empty) != "CANNOT_OBSERVE|empty_ntm_list" {
        say("selftest: FAIL — empty_ntm_list detector did not fire on ntm's empty-fleet banner");
        return ExitCode::FAILURE;
    }
    say("selftest: PASS — named detector empty_ntm_list RED on planted No-tmux-sessions listing");

    let live_list = run_capture(
        Command::new(&cfg.ntm_bin)
            .env("TMUX_TMPDIR", &cfg.tmux_tmpdir)
            .arg("list"),
    );
    if ntm_list_is_empty(&live_list) {
        say("selftest: FAIL — live ntm list is an empty scan set (TMUX_TMPDIR / ntm projection)");
        return ExitCode::FAILURE;
    }
    if !live_list.contains(':') {
        say("selftest: FAIL — live ntm list has zero session rows (empty scan set is ERROR, never PASS)");
        return ExitCode::FAILURE;
    }
    say("selftest: PASS — live ntm list is listed, not empty_ntm_list");

    // MUTATION: disable the observe-path guard. The planted empty listing must
    // collapse to a drained scan (Repos []) — that is the identity merge this
    // detector exists to stop. Restore is env-scoped; production empty_scan stays on.
    let mutated = observe_scan_set(
        planted_empty,
        &cfg.developer_root,
        ObserveRules { empty_scan: false },
    );
    if matches!(mutated, ObserveScan::Repos(ref r) if r.is_empty()) {
        say("MUTATION RED empty_ntm_list: disabling the guard treats the planted empty ntm list as a drained fleet");
    } else {
        say(&format!(
            "selftest: FAIL — mutating empty_scan off must collapse planted empty list to drained, got {mutated:?}"
        ));
        return ExitCode::FAILURE;
    }

    say(&format!(
        "selftest: PASS (publisher={}, invoker={}/{})",
        check_sh.display(),
        cfg.invoker.invoker,
        cfg.invoker.proof
    ));
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::{
        aux_lane_budget_seconds, classify_aux_lane_outcome, typed_safe_panes, AuxLaneOutcome,
    };
    use std::io;
    use std::process::Command;
    use std::time::Duration;

    #[test]
    fn rule_aux_lane_budget_is_capped_and_nonzero() {
        assert_eq!(
            aux_lane_budget_seconds(Duration::from_secs(30), Duration::from_secs(120)),
            Some(30)
        );
        assert_eq!(
            aux_lane_budget_seconds(Duration::from_secs(300), Duration::from_secs(120)),
            Some(120)
        );
        assert_eq!(
            aux_lane_budget_seconds(Duration::ZERO, Duration::from_secs(120)),
            None
        );
        assert_eq!(
            aux_lane_budget_seconds(Duration::from_secs(120), Duration::ZERO),
            None
        );
    }

    #[test]
    fn rule_aux_lane_outcomes_preserve_completion_deadline_signal_and_spawn_failure() {
        let completed = Command::new("/bin/sh").arg("-c").arg("exit 0").output().unwrap();
        assert_eq!(
            classify_aux_lane_outcome(&Ok(completed), 2),
            AuxLaneOutcome::Completed { code: 0 }
        );

        let deadline = Command::new("/bin/sh")
            .arg("-c")
            .arg("printf 'DEADLINE lane=x exceeded_s=2\\n'; exit 143")
            .output()
            .unwrap();
        assert_eq!(
            classify_aux_lane_outcome(&Ok(deadline), 2),
            AuxLaneOutcome::DeadlineReached { code: Some(143), seconds: 2 }
        );

        let signalled = Command::new("/bin/sh").arg("-c").arg("kill -TERM $$").output().unwrap();
        assert!(matches!(
            classify_aux_lane_outcome(&Ok(signalled), 2),
            AuxLaneOutcome::Signalled { signal: Some(15) }
        ));

        assert!(matches!(
            classify_aux_lane_outcome(&Err(io::Error::from(io::ErrorKind::NotFound)), 2),
            AuxLaneOutcome::SpawnFailed { kind: io::ErrorKind::NotFound }
        ));
    }

    #[test]
    fn rule_typed_omp_readiness_overrides_legacy_safe_hint() {
        let healthy = r#"{"success":true,"agents":[{"pane":3,"agent_type":"omp-claude","state":"IDLE","observation_state":"idle","safe_to_dispatch":true,"capture_provenance":"live","observation_freshness":"fresh"}]}"#;
        let (panes, snapshot) = typed_safe_panes(healthy).expect("healthy OMP activity parses");
        assert_eq!(panes, vec!["3"]);
        assert_eq!(snapshot.dispatchable_omp_count(), 1);

        let conflict = healthy.replace("\"state\":\"IDLE\"", "\"state\":\"THINKING\"");
        let (panes, snapshot) = typed_safe_panes(&conflict).expect("conflict activity parses");
        assert!(panes.is_empty(), "conflicting OMP must not remain a legacy candidate");
        assert_eq!(snapshot.dispatchable_omp_count(), 0);

        let unknown_state = healthy.replace("\"state\":\"IDLE\"", "\"state\":\"UNKNOWN\"");
        let (panes, _) = typed_safe_panes(&unknown_state).expect("unknown idle OMP parses");
        assert_eq!(panes, vec!["3"], "pane capture remains the final liveness gate");

        let unknown = healthy.replace("omp-claude", "omp-future");
        let (panes, _) = typed_safe_panes(&unknown).expect("unknown OMP activity parses");
        assert!(panes.is_empty(), "unknown OMP plugins must not fall through legacy safety");

        assert!(typed_safe_panes(r#"{"success":true,"agents":[]}"#).is_err());
    }
}

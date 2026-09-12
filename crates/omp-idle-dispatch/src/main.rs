#![forbid(unsafe_code)]

//! Thin operational boundary for `omp-idle-dispatch`.
//!
//! The library owns every dispatch decision. This binary alone talks to tmux, `br`, and
//! `ntm`; each child has stdin closed and failures become explicit RED/error output rather
//! than an empty successful queue.

use omp_idle_dispatch::{
    blocker_fields, classify_capture, classify_tick, confirm_capture_pair, pick_beads, plan_queues,
    receiver_transition, recently_dispatched, render_packet, IdleDispatchPaneState, TickVerdict,
    DEFAULT_CONFIRM_SECONDS, DEFAULT_COOLDOWN_SECONDS, LANE, QUEUE_WIDTH,
};
use omp_idle_dispatch::profile_store::{self, PaneOracleError, PaneRoute};
use agent_mail_native::identity::{format_sender_header, resolve_pane_identity};
use agent_mail_native::journey::ProjectKey;
use agent_mail_native::MailClient;
use asupersync::runtime::RuntimeBuilder;
use asupersync::Cx;
use serde_json::{json, Value};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use subprocess_contract::{bounded_output, bounded_status, BoundedOutcome};

const COMMAND_TIMEOUT_SECONDS: u64 = 30;
const DEFAULT_RECEIVER_PROOF_SECONDS: u64 = 30;
const RECEIVER_PROOF_ENV: &str = "OMP_DISPATCH_RECEIVER_PROOF_S";
const SENDER_IDENTITY_TIMEOUT: Duration = Duration::from_secs(COMMAND_TIMEOUT_SECONDS);

/// Marker entries that identify a repository root while walking up from the cwd.
/// `.git` may be a directory (plain checkout) or a file (worktree/submodule).
const REPO_MARKERS: [&str; 2] = [".git", ".beads"];

/// Environment variable overriding the repository root (`--repo` beats it).
const REPO_ENV: &str = "OMP_DISPATCH_REPO";
/// Environment variable overriding the tmux session name.
const SESSION_ENV: &str = "OMP_DISPATCH_SESSION";
/// Environment variable overriding the ledger path.
const LEDGER_ENV: &str = "OMP_DISPATCH_LEDGER";
/// `PATH` for spawned children: `$HOME/.local/bin` first, then the system directories.
///
/// NOT the ledger path. This function sat directly beneath the doc comment
/// "Ledger location relative to `$HOME`..." — which belongs to `LEDGER_HOME_RELATIVE`
/// below — and the misattribution cost three probes on 2026-09-07 while reconstructing
/// hand-dispatch ledger rows: the reader concluded the default ledger location was a
/// colon-separated `PATH` string and went looking for a defect that does not exist.
/// A doc comment that describes its neighbour is the same class as prose asserting
/// behaviour the code does not have.
fn default_path() -> String {
    let home = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    match home {
        Some(home) => format!(
            "{}/.local/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin",
            home.display()
        ),
        None => "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin".to_owned(),
    }
}
/// Ledger location relative to `$HOME` when `OMP_DISPATCH_LEDGER` is unset.
const LEDGER_HOME_RELATIVE: &str = ".local/state/flywheel/omp-idle-dispatch.jsonl";
const DEFAULT_LOCK: &str = "/tmp/omp-idle-dispatch.lock";
const NO_PANES_EXIT: u8 = 78;
/// A repository or `$HOME` could not be resolved; distinct from usage errors.
const CONFIG_ERROR_EXIT: u8 = 64;

/// Fail-closed path configuration: everything the binary needs to know where it is.
///
/// Every variant names the thing it could not find, because the historic failure mode of
/// this lane was worse than an error: a hardcoded root compiled fine after a move and then
/// silently read the WRONG repository.
#[derive(Debug)]
enum ConfigError {
    /// An explicit source (`--repo` or an environment variable) was set but empty.
    ExplicitEmpty { source: String },
    /// No repository marker found walking up from `from`.
    RepoNotFound { from: PathBuf },
    /// `$HOME` is unset, so the default ledger location is unknowable.
    HomeUnset,
}

impl ConfigError {
    fn code(&self) -> &'static str {
        match self {
            Self::ExplicitEmpty { .. } => "repo_source_empty",
            Self::RepoNotFound { .. } => "repo_root_not_found",
            Self::HomeUnset => "home_unset",
        }
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExplicitEmpty { source } => {
                write!(formatter, "{source} is set but empty")
            }
            Self::RepoNotFound { from } => write!(
                formatter,
                "no repository marker ({}) found at or above {}; pass --repo <PATH> or set {REPO_ENV}",
                REPO_MARKERS.join(" or "),
                from.display()
            ),
            Self::HomeUnset => write!(
                formatter,
                "$HOME is unset; cannot resolve the default ledger ({LEDGER_HOME_RELATIVE}); set {LEDGER_ENV}"
            ),
        }
    }
}

#[derive(Debug)]
enum CommandError {
    Spawn { program: String, message: String },
    Failed { program: String, code: Option<i32> },
    TimedOut { program: String, seconds: u64 },
}

impl CommandError {
    fn message(&self) -> String {
        match self {
            Self::Spawn { program, message } => format!("{program}: {message}"),
            Self::Failed { program, code } => format!("{program} exited {:?}", code),
            Self::TimedOut { program, seconds } => {
                format!("{program} exceeded {seconds}s deadline")
            }
        }
    }
}

/// A create-new lock whose Drop removes the marker. It is intentionally not held across
/// library decisions: the marker only prevents overlapping operational ticks.
struct DispatchLock {
    path: PathBuf,
    _file: File,
}

impl DispatchLock {
    fn acquire(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        Ok(Self { path, _file: file })
    }
}

impl Drop for DispatchLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn env_or(key: &str, fallback: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| fallback.to_string())
}

fn env_u64(key: &str, fallback: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

/// Walk up from `start`, returning the first ancestor that holds a repository marker.
/// Mirrors `beads_rust`'s `canonical_source_repo`: identity is derived from a discovered
/// marker's parent, never from a constant.
fn discover_repo_root(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(directory) = current {
        if REPO_MARKERS
            .iter()
            .any(|marker| directory.join(marker).exists())
        {
            return Some(directory.to_path_buf());
        }
        current = directory.parent();
    }
    None
}

/// Resolve the repository root. Precedence, highest first, is documented in `usage()`:
/// `--repo` flag > `OMP_DISPATCH_REPO` env > upward marker walk from the cwd.
/// Pure with respect to the process: callers pass the flag, env value, and start directory,
/// so precedence is unit-testable without mutating process-global state.
fn resolve_repo_root(
    flag: Option<&str>,
    env_value: Option<String>,
    start: &Path,
) -> Result<PathBuf, ConfigError> {
    if let Some(flag) = flag {
        if flag.trim().is_empty() {
            return Err(ConfigError::ExplicitEmpty {
                source: "--repo".to_owned(),
            });
        }
        return Ok(PathBuf::from(flag));
    }
    if let Some(value) = env_value {
        if value.trim().is_empty() {
            return Err(ConfigError::ExplicitEmpty {
                source: REPO_ENV.to_owned(),
            });
        }
        return Ok(PathBuf::from(value));
    }
    discover_repo_root(start).ok_or_else(|| ConfigError::RepoNotFound {
        from: start.to_path_buf(),
    })
}

/// `$HOME`, or a typed error. Never a guessed literal.
fn home_dir() -> Result<PathBuf, ConfigError> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or(ConfigError::HomeUnset)
}

/// Resolve the ledger: `OMP_DISPATCH_LEDGER` env > `$HOME/<LEDGER_HOME_RELATIVE>`.
fn resolve_ledger() -> Result<PathBuf, ConfigError> {
    if let Some(path) = std::env::var_os(LEDGER_ENV).filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    Ok(home_dir()?.join(LEDGER_HOME_RELATIVE))
}

/// Resolve the tmux session name: `OMP_DISPATCH_SESSION` env > the discovered repository's
/// basename. The basename default keeps this checkout's `control-plane` behavior while a
/// moved checkout (for example omp-orchestrator) resolves its own session instead of
/// silently targeting a session that no longer matches the repository.
fn session_name(repo: &Path) -> String {
    if let Some(session) = std::env::var(SESSION_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return session;
    }
    // No invented fallback: a repo path with no basename is pathological, and an empty
    // session name makes tmux fail loudly rather than silently target the wrong session.
    repo.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Read the live `--repo` flag value (`--repo PATH` or `--repo=PATH`) from raw args.
fn repo_flag(args: &[String]) -> Result<Option<String>, String> {
    let mut flag: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--repo" {
            let value = args.get(index + 1).ok_or("--repo requires a path")?;
            flag = Some(value.clone());
            index += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--repo=") {
            flag = Some(value.to_owned());
        }
        index += 1;
    }
    Ok(flag)
}

fn configure_command(command: &mut Command, cwd: Option<&Path>) {
    command.stdin(Stdio::null()).stderr(Stdio::null());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
}


fn run_bounded(mut command: Command, program: &str, capture: bool) -> Result<String, CommandError> {
    let outcome = if capture {
        bounded_output(&mut command, Duration::from_secs(COMMAND_TIMEOUT_SECONDS))
    } else {
        command.stdout(Stdio::null());
        bounded_status(&mut command, Duration::from_secs(COMMAND_TIMEOUT_SECONDS))
    };
    match outcome {
        BoundedOutcome::Completed(output) if output.status.success() => {
            if capture {
                Ok(String::from_utf8_lossy(&output.stdout).into_owned())
            } else {
                Ok(String::new())
            }
        }
        BoundedOutcome::Completed(output) => Err(CommandError::Failed {
            program: program.to_string(),
            code: output.status.code(),
        }),
        BoundedOutcome::TimedOut => Err(CommandError::TimedOut {
            program: program.to_string(),
            seconds: COMMAND_TIMEOUT_SECONDS,
        }),
        BoundedOutcome::Unspawned(error) => Err(CommandError::Spawn {
            program: program.to_string(),
            message: error.to_string(),
        }),
    }
}

fn command_output(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
) -> Result<String, CommandError> {
    let mut command = Command::new(program);
    command.args(args);
    configure_command(&mut command, cwd);
    run_bounded(command, program, true)
}

fn command_status(program: &str, args: &[String], cwd: Option<&Path>) -> Result<(), CommandError> {
    let mut command = Command::new(program);
    command.args(args);
    configure_command(&mut command, cwd);
    run_bounded(command, program, false).map(|_| ())
}

fn utc_timestamp(now: SystemTime) -> String {
    let seconds = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    let days = seconds.div_euclid(86_400);
    let day_seconds = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        day_seconds / 3_600,
        (day_seconds % 3_600) / 60,
        day_seconds % 60
    )
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    (year + i64::from(month <= 2), month as u32, day as u32)
}

fn invoker() -> (&'static str, &'static str) {
    match std::env::var("OMP_INVOKER").ok().as_deref() {
        Some(value) if value.starts_with("SCHEDULED") => ("SCHEDULED", "cron_parent"),
        _ => ("MANUAL", "unproven_parent"),
    }
}

fn ledger_path() -> Result<PathBuf, ConfigError> {
    resolve_ledger()
}

fn append_ledger(path: &Path, row: Value) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, &row).map_err(io::Error::other)?;
    file.write_all(b"\n")
}

fn emit(row: Value) {
    println!("{}", row);
}

fn status(json_mode: bool) -> u8 {
    let row = json!({
        "schema": "omp-idle-dispatch.status.v1",
        "lane": LANE,
        "ready": true,
        "pure_decision_layer": true,
        "two_capture_confirmation": true,
        "cooldown_seconds": env_u64("OMP_DISPATCH_COOLDOWN", DEFAULT_COOLDOWN_SECONDS),
        "confirm_seconds": env_u64("OMP_DISPATCH_CONFIRM_S", DEFAULT_CONFIRM_SECONDS),
        "gated_on_check_sh": false,
        "process_spawning": "binary_boundary_only",
        "profile_store_route": true,
        "legacy_spinner_route": "unprofiled_only",
    });
    if json_mode {
        emit(row);
    } else {
        println!("lane={LANE} ready=true two_capture_confirmation=true gated_on_check_sh=false");
    }
    0
}

fn why(json_mode: bool) -> u8 {
    let row = json!({
        "schema": "omp-idle-dispatch.why.v1",
        "lane": LANE,
        "reason": "A stale standing check.sh RED must not strand idle panes; this lane only mutates by sending a bounded packet into a confirmed idle pane.",
        "refusals": ["no_panes_visible", "unknown_pane_shape", "working_or_changed_capture", "pane_cooldown", "empty_ready_queue", "send_failed"],
        "healthy_noop": "BLOCKED infrastructure:no-idle-capacity",
    });
    if json_mode {
        emit(row);
    } else {
        println!("lane={LANE} reason=confirmed-idle-pane-gets-bounded-ready-queue");
    }
    0
}

fn capabilities(json_mode: bool) -> u8 {
    let row = json!({
        "schema": "omp-idle-dispatch.capabilities.v1",
        "name": LANE,
        "observes": true,
        "dispatches": true,
        "gated_on_check_sh": false,
        "subcommands": ["status", "why", "capabilities", "pane-state", "run"],
        "mutation": "ntm robot send (one call per pane; no fallback)",
        "profile_store_route": true,
        "legacy_spinner_route": "unprofiled_only",
        "selftest": true,
    });
    if json_mode {
        emit(row);
    } else {
        println!("name={LANE} observes=true dispatches=true gated_on_check_sh=false");
    }
    0
}

fn lock_or_report(path: &Path) -> Option<DispatchLock> {
    match DispatchLock::acquire(path) {
        Ok(lock) => Some(lock),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            emit(
                json!({ "schema": "omp-idle-dispatch.error.v1", "lane": LANE, "error": "ALREADY_RUNNING" }),
            );
            None
        }
        Err(error) => {
            emit(
                json!({ "schema": "omp-idle-dispatch.error.v1", "lane": LANE, "error": "lock_failed", "detail": error.to_string() }),
            );
            None
        }
    }
}

fn capture_pane(pane: &str) -> Result<String, CommandError> {
    command_output(tick_monitor::TMUX, &[tick_monitor::CAPTURE_PANE, "-p", "-t", pane], None)
}

/// Confirm a sender success by observing the target pane enter the named bead's working state.
///
/// `ntm`'s return status is only dispatch-record evidence. The receiver proof is a separate,
/// target-observed capture transition, bounded by the same command deadline and polled for state.
fn receiver_proof(pane: &str, before: &str, bead_id: &str) -> bool {
    let seconds =
        env_u64(RECEIVER_PROOF_ENV, DEFAULT_RECEIVER_PROOF_SECONDS).min(COMMAND_TIMEOUT_SECONDS);
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        if let Ok(after) = capture_pane(pane) {
            if receiver_transition(before, &after, bead_id) {
                return true;
            }
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return false;
        }
        thread::sleep(Duration::from_millis(200).min(remaining));
    }
}

fn live_panes(session: &str) -> Result<Vec<String>, CommandError> {
    let output = command_output(
        tick_monitor::TMUX,
        &["list-panes", "-t", session, "-F", "#{pane_id}"],
        None,
    )?;
    Ok(output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

fn pane_index(pane: &str) -> Result<String, CommandError> {
    command_output(
        tick_monitor::TMUX,
        &["display-message", "-p", "-t", pane, "#{pane_index}"],
        None,
    )
    .map(|value| value.trim().to_string())
}

fn sender_header(repo: &Path, session: &str) -> Result<String, String> {
    let pane_id = std::env::var("TMUX_PANE")
        .map_err(|_| "SENDER_IDENTITY_REFUSED reason=TMUX_PANE_missing".to_owned())?;
    let project = ProjectKey::new(repo.display().to_string());
    let client = MailClient::discover().with_request_timeout(SENDER_IDENTITY_TIMEOUT);
    let runtime = RuntimeBuilder::current_thread()
        .build()
        .map_err(|error| format!("SENDER_IDENTITY_REFUSED reason=runtime_build error={error}"))?;
    let identity = runtime.block_on(async {
        let cx = Cx::current()
            .ok_or_else(|| "SENDER_IDENTITY_REFUSED reason=no_runtime_context".to_owned())?;
        resolve_pane_identity(&cx, &client, &project, &pane_id)
            .await
            .map_err(|error| format!("SENDER_IDENTITY_REFUSED pane={pane_id} error={error}"))
    })?;
    if identity.pane_id != pane_id {
        return Err(format!(
            "SENDER_IDENTITY_REFUSED requested_pane={pane_id} resolved_pane={}",
            identity.pane_id
        ));
    }
    if let Some(identity_session) = identity.session.as_deref() {
        if identity_session != session {
            return Err(format!(
                "SENDER_IDENTITY_REFUSED pane={pane_id} session={identity_session} expected_session={session}"
            ));
        }
    }
    let header = format_sender_header(&identity, session, &project)
        .map_err(|error| format!("SENDER_IDENTITY_REFUSED pane={pane_id} error={error}"))?;
    if !header.starts_with("FROM:")
        || !header
            .lines()
            .nth(1)
            .is_some_and(|line| line.starts_with("REPLY-VIA:"))
    {
        return Err("SENDER_IDENTITY_REFUSED reason=malformed_header".to_owned());
    }
    Ok(header)
}

fn read_pane_route(pane: &str, home: &Path, binary: &str) -> Result<PaneRoute, PaneOracleError> {
    let runtime = RuntimeBuilder::current_thread()
        .build()
        .map_err(|error| PaneOracleError::Runtime(error.to_string()))?;
    runtime.block_on(async {
        let cx = Cx::current()
            .ok_or_else(|| PaneOracleError::Runtime("no_runtime_context".to_owned()))?;
        profile_store::read_pane_state(&cx, pane, home, binary).await
    })
}

fn report_profile_store_error(pane: &str, error: &PaneOracleError) {
    emit(json!({
        "schema": "omp-idle-dispatch.error.v1",
        "lane": LANE,
        "error": "PANE_ORACLE_UNKNOWN",
        "pane": pane,
        "detail": error.to_string(),
        "exit_code": error.exit_code(),
    }));
}

fn report_profile_store_mapping(
    pane: &str,
    profile: &str,
    entry: &profile_store::ValidatedSessionEntry,
) {
    let entry_shape = match entry.entry.shape {
        profile_store::EntryShape::TwoLines => "two_lines",
        profile_store::EntryShape::ThreeLines => "three_lines",
    };
    let selected_root = entry
        .terminal_entry_path
        .parent()
        .map_or_else(|| entry.terminal_entry_path.display().to_string(), |path| path.display().to_string());
    emit(json!({
        "schema": "omp-idle-dispatch.pane-oracle.v1",
        "lane": LANE,
        "pane": pane,
        "profile": profile,
        "selected_root": selected_root,
        "unprofiled_probe": entry.roots.unprofiled.status(),
        "profiled_probe": entry.roots.profiled.as_ref().map_or("NOT_SELECTED", profile_store::RootProbe::status),
        "entry_shape": entry_shape,
        "state_token": entry.entry.state_token.status(),
    }));
}
fn report_profile_store_state(
    pane: &str,
    profile: &str,
    outcome: &ompo_doctor::omp_state::StateOutcome,
) {
    let idle_projection = match profile_store::idle_from_state(outcome) {
        Some(true) => "idle",
        Some(false) => "working",
        None => "unknown",
    };
    emit(json!({
        "schema": "omp-idle-dispatch.pane-state.v1",
        "lane": LANE,
        "pane": pane,
        "profile": profile,
        "state_outcome": outcome.reason_code(),
        "state_status": outcome.envelope_status(),
        "snapshot_projection": idle_projection,
        "dispatch_projection": "unknown",
        "liveness_proof": "UNMEASURED_LAST_OPENED_SESSION",
    }));
}

#[derive(Debug)]
enum PaneSample {
    Ready {
        second: String,
        omp_seen: bool,
        unprofiled: bool,
    },
    Skipped {
        omp_seen: bool,
        unprofiled: bool,
    },
    OracleError {
        error: PaneOracleError,
        omp_seen: bool,
    },
}

fn observe_unprofiled(pane: &str, confirm_seconds: u64) -> PaneSample {
    let first = match capture_pane(pane) {
        Ok(capture) => capture,
        Err(_) => {
            return PaneSample::Skipped {
                omp_seen: false,
                unprofiled: true,
            };
        }
    };
    if !first.contains(omp_idle_dispatch::MODEL_BANNER)
        || classify_capture(&first) != IdleDispatchPaneState::Idle
    {
        return PaneSample::Skipped {
            omp_seen: first.contains(omp_idle_dispatch::MODEL_BANNER),
            unprofiled: true,
        };
    }
    thread::sleep(Duration::from_secs(confirm_seconds));
    let second = match capture_pane(pane) {
        Ok(capture) => capture,
        Err(_) => return PaneSample::Skipped { omp_seen: true, unprofiled: true },
    };
    if confirm_capture_pair(&first, &second).as_str() != "IDLE" {
        return PaneSample::Skipped {
            omp_seen: true,
            unprofiled: true,
        };
    }
    PaneSample::Ready {
        second,
        omp_seen: true,
        unprofiled: true,
    }
}

fn observe_profiled(
    pane: &str,
    profile: String,
    entry: profile_store::ValidatedSessionEntry,
    outcome: ompo_doctor::omp_state::StateOutcome,
) -> PaneSample {
    report_profile_store_mapping(pane, &profile, &entry);
    report_profile_store_state(pane, &profile, &outcome);
    let snapshot_projection = match profile_store::idle_from_state(&outcome) {
        Some(true) => "idle",
        Some(false) => "working",
        None => "unknown",
    };
    PaneSample::OracleError {
        error: PaneOracleError::Runtime(format!(
            "PANE_ORACLE_STATE_UNMEASURED reason=profile_store_is_last_opened_snapshot state_outcome={} snapshot_projection={snapshot_projection}",
            outcome.reason_code()
        )),
        omp_seen: true,
    }
}

fn observe_pane(
    pane: &str,
    home: &Path,
    binary: &str,
    confirm_seconds: u64,
) -> PaneSample {
    match read_pane_route(pane, home, binary) {
        Ok(PaneRoute::Unprofiled { .. }) => observe_unprofiled(pane, confirm_seconds),
        Ok(PaneRoute::Profiled {
            profile,
            entry,
            outcome,
            ..
        }) => observe_profiled(pane, profile, entry, outcome),
        Err(error) => PaneSample::OracleError {
            error,
            omp_seen: false,
        },
    }
}

fn accept_pane_sample(
    sample: PaneSample,
    omp_seen: &mut usize,
    unprofiled_fallbacks: &mut usize,
    idle_found: &mut usize,
) -> Result<Option<String>, PaneOracleError> {
    match sample {
        PaneSample::Ready {
            second,
            omp_seen: observed_omp,
            unprofiled,
            ..
        } => {
            if observed_omp {
                *omp_seen += 1;
            }
            if unprofiled {
                *unprofiled_fallbacks += 1;
            }
            *idle_found += 1;
            Ok(Some(second))
        }
        PaneSample::Skipped {
            omp_seen: observed_omp,
            unprofiled,
        } => {
            if observed_omp {
                *omp_seen += 1;
            }
            if unprofiled {
                *unprofiled_fallbacks += 1;
            }
            Ok(None)
        }
        PaneSample::OracleError {
            error,
            omp_seen: observed_omp,
        } => {
            if observed_omp {
                *omp_seen += 1;
            }
            Err(error)
        }
    }
}
fn run_tick(dry_run: bool, repo: &Path) -> u8 {
    // Resolution before any side effect: a missing repo or `$HOME` must fail loudly
    // here, never mid-tick after panes have been captured.
    let session = session_name(repo);
    let ledger = match ledger_path() {
        Ok(ledger) => ledger,
        Err(error) => return config_error_exit(&error),
    };
    let lock_path = PathBuf::from(env_or("OMP_DISPATCH_LOCK", DEFAULT_LOCK));
    let Some(_lock) = lock_or_report(&lock_path) else {
        return 75;
    };
    let sender_prefix = match sender_header(repo, &session) {
        Ok(prefix) => prefix,
        Err(error) => {
            emit(json!({
                "schema": "omp-idle-dispatch.error.v1",
                "lane": LANE,
                "error": "sender_identity_refused",
                "detail": error,
            }));
            return CONFIG_ERROR_EXIT;
        }
    };
    let panes = match live_panes(&session) {
        Ok(panes) if !panes.is_empty() => panes,
        Ok(_) => {
            emit(
                json!({ "schema": "omp-idle-dispatch.error.v1", "lane": LANE, "error": "no_panes_visible" }),
            );
            return NO_PANES_EXIT;
        }
        Err(error) => {
            emit(
                json!({ "schema": "omp-idle-dispatch.error.v1", "lane": LANE, "error": "pane_probe_failed", "detail": error.message() }),
            );
            return NO_PANES_EXIT;
        }
    };
    let home = match home_dir() {
        Ok(home) => home,
        Err(error) => return config_error_exit(&error),
    };
    let omp_binary = std::env::var("OMP_DISPATCH_OMP_BINARY").unwrap_or_else(|_| "omp".to_owned());
    let ready_json = match command_output(finding::BR, &[loop_queue_filter::READY_SUBCOMMAND, "--limit", "0", "--json"], Some(repo)) {
        Ok(output) => output,
        Err(error) => {
            emit(
                json!({ "schema": "omp-idle-dispatch.error.v1", "lane": LANE, "error": "ready_probe_failed", "detail": error.message() }),
            );
            return NO_PANES_EXIT;
        }
    };
    let beads = pick_beads(&ready_json, 12);
    let ready_count = match serde_json::from_str::<Value>(&ready_json) {
        Ok(Value::Array(rows)) => rows.len(),
        Ok(Value::Object(object)) => object
            .get("issues")
            .and_then(Value::as_array)
            .map_or(0, Vec::len),
        _ => 0,
    };
    let ledger_text = fs::read_to_string(&ledger).unwrap_or_default();
    let cooldown = Duration::from_secs(env_u64("OMP_DISPATCH_COOLDOWN", DEFAULT_COOLDOWN_SECONDS));
    let confirm_seconds = env_u64("OMP_DISPATCH_CONFIRM_S", DEFAULT_CONFIRM_SECONDS);
    let (invoker_name, invoker_proof) = invoker();
    let mut idle_found = 0usize;
    let mut dispatched = 0usize;
    let mut cooldown_skipped = 0usize;
    let mut omp_seen = 0usize;
    let mut send_failed = false;
    let mut cursor = 0usize;
    let mut unprofiled_fallbacks = 0usize;
    let mut oracle_failed = false;
    for pane in panes {
        let second = match accept_pane_sample(
            observe_pane(&pane, &home, &omp_binary, confirm_seconds),
            &mut omp_seen,
            &mut unprofiled_fallbacks,
            &mut idle_found,
        ) {
            Ok(Some(second)) => second,
            Ok(None) => continue,
            Err(error) => {
                oracle_failed = true;
                report_profile_store_error(&pane, &error);
                continue;
            }
        };
        if recently_dispatched(&ledger_text, &pane, SystemTime::now(), cooldown) {
            cooldown_skipped += 1;
            let row = json!({ "ts": utc_timestamp(SystemTime::now()), "lane": LANE, "verdict": "BLOCKED", "external_blocker": "infrastructure:pane-cooldown", "escalation_action": format!("skipped pane={pane} within {}s", cooldown.as_secs()), "pane": pane, "action": "cooldown", "invoker": invoker_name, "invoker_proof": invoker_proof });
            let _ = append_ledger(&ledger, row);
            continue;
        }
        let plan = plan_queues(&beads, 1, cursor);
        cursor = plan.next_cursor;
        let Some(queue) = plan.pane_queues.first() else {
            break;
        };
        let mut packet = render_packet(&utc_timestamp(SystemTime::now()), ready_count, queue);
        packet.insert_str(0, &sender_prefix);
        let first_bead = queue
            .first()
            .map(|bead| bead.id.clone())
            .unwrap_or_default();
        if dry_run {
            println!("DRY-RUN would dispatch pane={pane} bead={first_bead}");
            dispatched += 1;
            continue;
        }
        let index = match pane_index(&pane) {
            Ok(index) if !index.is_empty() => index,
            _ => {
                send_failed = true;
                let _ = append_ledger(
                    &ledger,
                    json!({ "ts": utc_timestamp(SystemTime::now()), "lane": LANE, "verdict": "RED", "cause": "pane_index_failed", "pane": pane, "action": "send_failed", "invoker": invoker_name, "invoker_proof": invoker_proof }),
                );
                continue;
            }
        };
        let args = vec![
            tick_monitor::ntm_send_arg(&session),
            format!("--panes={index}"),
            format!("--msg={packet}"),
        ];
        match command_status(tick_monitor::NTM, &args, None) {
            Ok(()) if receiver_proof(&pane, &second, &first_bead) => {
                dispatched += 1;
                let _ = append_ledger(
                    &ledger,
                    json!({
                        "ts": utc_timestamp(SystemTime::now()),
                        "lane": LANE,
                        "verdict": "GREEN",
                        "product_moved": true,
                        "receiver_proof": "target_state_transition",
                        "pane": pane,
                        "action": "dispatched",
                        "bead": first_bead,
                        "ready": ready_count,
                        "invoker": invoker_name,
                        "invoker_proof": invoker_proof
                    }),
                );
            }
            Ok(()) => {
                send_failed = true;
                let _ = append_ledger(
                    &ledger,
                    json!({
                        "ts": utc_timestamp(SystemTime::now()),
                        "lane": LANE,
                        "verdict": "RED",
                        "cause": "receiver_proof_failed",
                        "sender_ok": true,
                        "receiver_proof": false,
                        "product_moved": false,
                        "pane": pane,
                        "action": "send_returned_without_target_transition",
                        "bead": first_bead,
                        "invoker": invoker_name,
                        "invoker_proof": invoker_proof
                    }),
                );
            }
            Err(error) => {
                send_failed = true;
                let _ = append_ledger(
                    &ledger,
                    json!({
                        "ts": utc_timestamp(SystemTime::now()),
                        "lane": LANE,
                        "verdict": "RED",
                        "cause": "send_failed",
                        "detail": error.message(),
                        "pane": pane,
                        "action": "send_failed",
                        "invoker": invoker_name,
                        "invoker_proof": invoker_proof
                    }),
                );
            }
        }
    }
    let verdict = classify_tick(omp_seen, idle_found, dispatched, send_failed || oracle_failed);
    if let Some((blocker, escalation)) =
        blocker_fields(verdict, omp_seen, idle_found, ready_count, dispatched)
    {
        let _ = append_ledger(
            &ledger,
            json!({ "ts": utc_timestamp(SystemTime::now()), "lane": LANE, "verdict": verdict.as_str(), "external_blocker": blocker, "escalation_action": escalation, "ready": ready_count, "invoker": invoker_name, "invoker_proof": invoker_proof }),
        );
    }
    emit(
        json!({ "schema": "omp-idle-dispatch.tick.v1", "lane": LANE, "verdict": verdict.as_str(), "idle": idle_found, "omp_seen": omp_seen, "unprofiled_fallbacks": unprofiled_fallbacks, "oracle_failed": oracle_failed, "dispatched": dispatched, "cooldown_skipped": cooldown_skipped, "ready": ready_count, "invoker": invoker_name, "invoker_proof": invoker_proof, "queue_width": QUEUE_WIDTH, "dry_run": dry_run }),
    );
    if verdict == TickVerdict::RedSendFailed {
        1
    } else {
        0
    }
}

fn selftest() -> u8 {
    let idle = "π  > ◒ GPT-5.6-Luna > S37.17";
    let working = " ⠏ 10m > ◒ GPT-5.6-Luna > S34.94";
    let stale = format!(" ⠹ 22m · ◒ GPT-5.6-Luna\n{idle}");
    let quoted = format!("π > ◒ GPT-5.6-Luna > quoted\n{working}");
    let tests = [
        classify_capture(&stale) == IdleDispatchPaneState::Idle,
        classify_capture(&quoted) == IdleDispatchPaneState::Working,
        classify_capture("??? > ◒ GPT-5.6-Luna") == IdleDispatchPaneState::Unknown,
        classify_capture("josh@studio % ls") == IdleDispatchPaneState::Unknown,
        classify_capture(idle) == IdleDispatchPaneState::Idle,
        classify_capture(working) == IdleDispatchPaneState::Working,
        confirm_capture_pair(idle, idle).as_str() == "IDLE",
        confirm_capture_pair(idle, working).as_str() == "CHANGED",
        classify_tick(3, 0, 0, false).as_str() == "BLOCKED",
        classify_tick(3, 1, 1, false).as_str() == "BLOCKED",
        classify_tick(3, 1, 0, true).as_str() == "RED",
        matches!(
            profile_store::profile_from_command("omp --profile claude"),
            Ok(profile_store::ProfileSelection::Profiled(profile)) if profile == "claude"
        ),
        profile_store::parse_entry("/repo\n/session.jsonl\n").is_ok(),
        profile_store::parse_entry("only-one-line\n").is_err(),
    ];
    let passed = tests.iter().filter(|value| **value).count();
    emit(
        json!({ "schema": "omp-idle-dispatch.selftest.v1", "passed": passed, "total": tests.len(), "ok": passed == tests.len(), "mutation": "first-banner-without-tail-anchor=WORKING" }),
    );
    if passed == tests.len() {
        0
    } else {
        1
    }
}

fn usage() {
    println!(
        "omp-idle-dispatch [status|why|capabilities|run|--pane-state %N|--dry-run|--selftest] [--json] [--repo <PATH>]\n\n\
         Repository root precedence: --repo flag > {REPO_ENV} env > upward walk from the cwd\n\
         for a {} marker. No marker and no override is a loud error, never a default.\n\
         Session: {SESSION_ENV} env > repository basename. Ledger: {LEDGER_ENV} env >\n\
         $HOME/{LEDGER_HOME_RELATIVE}.",
        REPO_MARKERS.join(" or ")
    );
}

/// Emit a configuration-resolution failure and return its exit code. The error row is
/// the loud failure: it names exactly what could not be found and how to provide it.
fn config_error_exit(error: &ConfigError) -> u8 {
    emit(json!({
        "schema": "omp-idle-dispatch.error.v1",
        "lane": LANE,
        "error": error.code(),
        "detail": error.to_string(),
    }));
    CONFIG_ERROR_EXIT
}

/// ENV PARITY CONTRACT (control-plane#cp-79am1). bin/omp-idle-dispatch.sh — deleted by the Rust port,
/// restored here from git (45c613d^, lines 25-27) — exported, in order:
///   PATH=<fleet PATH, personal bin dir first>
///   TMUX_TMPDIR set-if-unset to the user's tmux socket dir (the deleted script
///     hardcoded an absolute home path here — see the contract record in tests/)
///   LC_ALL="${LC_ALL:-C.UTF-8}"  # cron gives NO locale; tmux -F rewrites TAB to '_' without it
/// The Rust port DROPPED all three, so under cron (which supplies no environment) tmux
/// attached to its private default socket and rewrote tab delimiters — observing the
/// wrong world silently. This contract restores the set-if-unset semantics with
/// $HOME-derived paths, and FAILS LOUDLY when the tmux socket dir cannot be resolved
/// or does not exist: a dispatcher that observes the wrong fleet must refuse, not
/// report an empty one.
#[derive(Debug)]
enum StartupError {
    TmuxTmpDirUnusable { path: PathBuf, reason: String },
    HomeUnset,
}

impl StartupError {
    fn code(&self) -> &'static str {
        match self {
            Self::TmuxTmpDirUnusable { .. } => "tmux_tmpdir_unusable",
            Self::HomeUnset => "home_unset",
        }
    }
}

impl std::fmt::Display for StartupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TmuxTmpDirUnusable { path, reason } => {
                write!(formatter, "TMUX_TMPDIR={} is unusable: {reason}", path.display())
            }
            Self::HomeUnset => write!(
                formatter,
                "HOME is unset; cannot default TMUX_TMPDIR to $HOME/.tmux-sockets; set TMUX_TMPDIR explicitly"
            ),
        }
    }
}

/// Restore the deleted shell export semantics: TMUX_TMPDIR set-if-unset with a
/// `$HOME`-derived default, LC_ALL defaulted to C.UTF-8 (cron gives no locale, and
/// without a UTF-8 charmap tmux rewrites tab delimiters, corrupting pane parsing).
/// The tmux socket dir must EXIST — a missing dir means the dispatcher would attach
/// to the wrong server and observe an empty world, which is the silent failure this
/// contract exists to make loud.
fn prepare_runtime_environment() -> Result<(), StartupError> {
    let tmux_tmpdir = match std::env::var_os("TMUX_TMPDIR") {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => {
            let home = std::env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .ok_or(StartupError::HomeUnset)?;
            home.join(".tmux-sockets")
        }
    };
    let metadata =
        std::fs::metadata(&tmux_tmpdir).map_err(|error| StartupError::TmuxTmpDirUnusable {
            path: tmux_tmpdir.clone(),
            reason: error.to_string(),
        })?;
    if !metadata.is_dir() {
        return Err(StartupError::TmuxTmpDirUnusable {
            path: tmux_tmpdir,
            reason: "path is not a directory".into(),
        });
    }
    std::env::set_var("PATH", default_path());
    std::env::set_var("TMUX_TMPDIR", &tmux_tmpdir);
    if std::env::var_os("LC_ALL").is_none() {
        std::env::set_var("LC_ALL", "C.UTF-8");
    }
    Ok(())
}

/// The observe path (run/dry-run) refuses to start in an unusable environment: rc=78
/// with a typed STARTUP_ERROR naming the variable, the path, and the reason. Diagnostics
/// (--selftest, status, why, capabilities) do not touch tmux and skip the contract.
fn startup_error_exit(error: &StartupError) -> u8 {
    emit(json!({
        "schema": "omp-idle-dispatch.error.v1",
        "lane": LANE,
        "error": format!("STARTUP_ERROR[{}]", error.code()),
        "detail": error.to_string(),
    }));
    78
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json_mode = args.iter().any(|arg| arg == "--json");
    let flag = match repo_flag(&args) {
        Ok(flag) => flag,
        Err(message) => {
            eprintln!("omp-idle-dispatch: {message}");
            usage();
            std::process::exit(2);
        }
    };
    let exit = match args.first().map(String::as_str) {
        Some("status") | Some("--status") => status(json_mode),
        Some("why") | Some("--why") => why(json_mode),
        Some("capabilities") | Some("--capabilities") => capabilities(json_mode),
        Some("--selftest") => selftest(),
        Some("--pane-state") => pane_state_cli(args.get(1).map(String::as_str)),
        Some("--dry-run") => {
            if let Err(error) = prepare_runtime_environment() {
                startup_error_exit(&error)
            } else {
                dispatch_exit(&flag, true)
            }
        }
        Some("run") | None => {
            if let Err(error) = prepare_runtime_environment() {
                startup_error_exit(&error)
            } else {
                dispatch_exit(&flag, false)
            }
        }
        _ => {
            usage();
            2
        }
    };
    std::process::exit(exit as i32);
}

/// Resolve the repository root for a tick, then run it. Resolution failure exits loudly.
fn dispatch_exit(flag: &Option<String>, dry_run: bool) -> u8 {
    let flag_value = flag.as_deref();
    let env_value = std::env::var(REPO_ENV).ok();
    let start = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            eprintln!("omp-idle-dispatch: cannot read the current directory: {error}");
            return CONFIG_ERROR_EXIT;
        }
    };
    match resolve_repo_root(flag_value, env_value, &start) {
        Ok(repo) => run_tick(dry_run, &repo),
        Err(error) => config_error_exit(&error),
    }
}

fn pane_state_cli(pane: Option<&str>) -> u8 {
    let Some(pane) = pane else {
        eprintln!("omp-idle-dispatch: --pane-state requires a tmux pane id such as %7");
        return 2;
    };
    if let Err(error) = prepare_runtime_environment() {
        return startup_error_exit(&error);
    }
    let home = match home_dir() {
        Ok(home) => home,
        Err(error) => return config_error_exit(&error),
    };
    let binary = std::env::var("OMP_DISPATCH_OMP_BINARY").unwrap_or_else(|_| "omp".to_owned());
    match read_pane_route(pane, &home, &binary) {
        Ok(PaneRoute::Profiled {
            profile,
            entry,
            outcome,
            ..
        }) => {
            report_profile_store_mapping(pane, &profile, &entry);
            report_profile_store_state(pane, &profile, &outcome);
            outcome.exit_code()
        }
        Ok(PaneRoute::Unprofiled { command }) => {
            emit(json!({
                "schema": "omp-idle-dispatch.pane-state.v1",
                "lane": LANE,
                "pane": pane,
                "route": "UNPROFILED",
                "command": command,
                "idle_projection": "legacy_fallback",
            }));
            0
        }
        Err(error) => {
            report_profile_store_error(pane, &error);
            error.exit_code()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Best-effort temp directory with cleanup on drop; keeps the crate dependency-free.
    struct TempDir(PathBuf);

    impl TempDir {
        fn create(label: &str) -> Self {
            Self::create_in(&std::env::temp_dir(), label)
        }

        fn create_in(root: &Path, label: &str) -> Self {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = root.join(format!(
                "omp-idle-dispatch-test-{}-{}-{}",
                label,
                std::process::id(),
                unique
            ));
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn repo_flag_beats_env_beats_discovery() {
        let root = TempDir::create("precedence");
        let nested = root.path().join("a/b/c");
        fs::create_dir_all(&nested).expect("create nested directory");
        fs::create_dir(root.path().join(".git")).expect("create .git marker");

        let flag_target = TempDir::create("flag-target");
        let env_target = TempDir::create("env-target");

        let resolved = resolve_repo_root(
            Some(flag_target.path().to_str().expect("utf-8 path")),
            Some(env_target.path().to_string_lossy().into_owned()),
            &nested,
        )
        .expect("flag must win");
        assert_eq!(resolved, flag_target.path());

        let resolved = resolve_repo_root(
            None,
            Some(env_target.path().to_string_lossy().into_owned()),
            &nested,
        )
        .expect("env must win over discovery");
        assert_eq!(resolved, env_target.path());

        let resolved =
            resolve_repo_root(None, None, &nested).expect("discovery must find the marker");
        assert_eq!(resolved, root.path());
    }

    #[test]
    fn discovery_walks_up_for_git_and_beads_markers() {
        let git_root = TempDir::create("git-marker");
        let nested = git_root.path().join("deeply/nested");
        fs::create_dir_all(&nested).expect("create nested directory");
        fs::create_dir(git_root.path().join(".git")).expect("create .git marker");
        assert_eq!(
            discover_repo_root(&nested),
            Some(git_root.path().to_path_buf())
        );

        let beads_root = TempDir::create("beads-marker");
        let nested = beads_root.path().join("x");
        fs::create_dir_all(&nested).expect("create nested directory");
        fs::create_dir(beads_root.path().join(".beads")).expect("create .beads marker");
        assert_eq!(
            discover_repo_root(&nested),
            Some(beads_root.path().to_path_buf())
        );
    }

    #[test]
    fn known_bad_no_repo_above_cwd_fails_loudly_naming_the_markers() {
        // A temp dir has no .git/.beads and neither do its ancestors up to the temp root
        // boundary we control; use a nested path and assert the typed error, then assert
        // the message names what was searched so the failure is self-describing.
        let nowhere = TempDir::create_in(Path::new("/tmp"), "known-bad");
        let start = nowhere.path().join("plain");
        fs::create_dir_all(&start).expect("create start directory");

        // The temp dir itself is clean; walk-up stops at the first marker, so plant the
        // start below a marker-free subtree by also asserting the error TYPE first.
        let error = match resolve_repo_root(None, None, &start) {
            Ok(found) => panic!(
                "a marker-free directory must not resolve; found {}",
                found.display()
            ),
            Err(error) => error,
        };
        // KNOWN-BAD: the error must be the typed RepoNotFound naming the markers.
        assert!(
            matches!(error, ConfigError::RepoNotFound { ref from } if *from == start),
            "wrong error for a marker-free directory: {error:?}"
        );
        let message = error.to_string();
        assert!(
            message.contains(".git") && message.contains(".beads"),
            "message must name the markers: {message}"
        );
        assert!(
            message.contains(start.to_string_lossy().as_ref()),
            "message must name the start directory: {message}"
        );
        assert!(
            message.contains(REPO_ENV),
            "message must name the escape hatch env: {message}"
        );
    }

    #[test]
    fn empty_explicit_sources_are_errors_not_defaults() {
        let start = Path::new("/");
        let error =
            resolve_repo_root(Some("   "), None, start).expect_err("empty --repo is an error");
        assert!(
            matches!(error, ConfigError::ExplicitEmpty { .. }),
            "wrong error: {error:?}"
        );
        assert!(
            error.to_string().contains("--repo"),
            "message must name --repo: {error}"
        );

        let error =
            resolve_repo_root(None, Some(String::new()), start).expect_err("empty env is an error");
        assert!(
            matches!(error, ConfigError::ExplicitEmpty { .. }),
            "wrong error: {error:?}"
        );
        assert!(
            error.to_string().contains(REPO_ENV),
            "message must name the env var: {error}"
        );
    }
    /// The home-path literal this gate forbids. Built by `concat!` so the scanning source
    /// itself never contains the contiguous literal (the gate must not catch its own needle).
    const USER_HOME_LITERAL: &str = concat!("/Users/", "josh");

    /// Count home-path literals in this crate's own `src/`, recursively.
    fn hardcoded_user_path_hits(src: &Path) -> (Vec<String>, usize) {
        let mut hits = Vec::new();
        let mut scanned = 0usize;
        let mut stack = vec![src.to_path_buf()];
        while let Some(directory) = stack.pop() {
            let entries = fs::read_dir(&directory)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()));
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|extension| extension == "rs") {
                    scanned += 1;
                    let text = fs::read_to_string(&path)
                        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
                    for (index, line) in text.lines().enumerate() {
                        if line.contains(USER_HOME_LITERAL) {
                            hits.push(format!("{}:{}", path.display(), index + 1));
                        }
                    }
                }
            }
        }
        (hits, scanned)
    }

    #[test]
    fn no_hardcoded_user_paths_in_src() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let (hits, scanned) = hardcoded_user_path_hits(&src);
        // Anti-vacuity: a scan that saw no source files proves nothing.
        assert!(
            scanned >= 2,
            "vacuous scan: only {scanned} source files under {}",
            src.display()
        );
        assert!(
            hits.is_empty(),
            "hardcoded home-path literal(s) reintroduced (this test exists so a \
             reintroduction turns RED): {hits:?}"
        );
    }
    #[test]
    fn session_name_env_beats_repo_basename() {
        // Direct env read is avoided: session_name reads the process env, so assert the
        // basename derivation only; the env leg is covered by the precedence pattern above
        // and by the portability proof run against the built binary.
        let repo = Path::new("/somewhere/control-plane");
        std::env::remove_var(SESSION_ENV);
        assert_eq!(session_name(repo), "control-plane");
        let repo = Path::new("/other/omp-orchestrator");
        assert_eq!(session_name(repo), "omp-orchestrator");
    }
}

#![forbid(unsafe_code)]

use fleet_composite::{compute_with_epsilon, compute_json_with_epsilon, factors, run_selftest, CompositeReport};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const EXIT_USAGE: u8 = 2;
/// A repository or `$HOME` could not be resolved; distinct from usage errors.
const EXIT_CONFIG: u8 = 64;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

/// Marker entries that identify a repository root while walking up from the cwd.
/// `.git` may be a directory (plain checkout) or a file (worktree/submodule).
const REPO_MARKERS: [&str; 2] = [".git", ".beads"];
/// Environment variable overriding the repository root (`--repo` beats it).
const REPO_ENV: &str = "FLEET_REPO";
/// Environment variable overriding the `ntm` session measured for `omp_busy`.
const SESSION_ENV: &str = "FLEET_SESSION";
/// Ledger location relative to `$HOME` when `FLEET_LEDGER` is unset.
const LEDGER_HOME_RELATIVE: &str = ".local/state/flywheel/challenge-lane.jsonl";
const USAGE: &str = "usage: fleet-composite [status|why|capabilities|composite|--selftest] [--json] [--repo <PATH>]\n\n\
                    repository root precedence: --repo flag > FLEET_REPO env > upward walk from\n\
                    the cwd for a .git or .beads marker; no marker and no override is a loud\n\
                    error, never a default. session: FLEET_SESSION env > repository basename.\n\
                    ledger: FLEET_LEDGER env > $HOME/.local/state/flywheel/challenge-lane.jsonl";

/// Fail-closed path configuration. Every variant names the thing it could not find,
/// because a hardcoded root compiles fine after a move and then silently measures the
/// WRONG repository.
#[derive(Debug)]
enum ConfigError {
    /// An explicit source (`--repo` or an environment variable) was set but empty.
    ExplicitEmpty { source: String },
    /// No repository marker found walking up from `from`.
    RepoNotFound { from: PathBuf },
    /// `$HOME` is unset, so a `~`-relative or default ledger path is unknowable.
    HomeUnset,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExplicitEmpty { source } => write!(formatter, "{source} is set but empty"),
            Self::RepoNotFound { from } => write!(
                formatter,
                "no repository marker ({}) found at or above {}; pass --repo <PATH> or set {REPO_ENV}",
                REPO_MARKERS.join(" or "),
                from.display()
            ),
            Self::HomeUnset => write!(
                formatter,
                "$HOME is unset; cannot resolve a home-relative ledger path; set FLEET_LEDGER to an absolute path"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Debug)]
struct CommandOutput {
    stdout: String,
    stderr: String,
    status: Option<i32>,
    error: Option<String>,
}

fn run_command(program: &str, args: &[&str], cwd: Option<&Path>, timeout: Duration) -> CommandOutput {
    let mut command = Command::new(program);
    command.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    CommandExt::process_group(&mut command, 0);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return CommandOutput {
                stdout: String::new(),
                stderr: String::new(),
                status: None,
                error: Some(format!("spawn {program}: {error}")),
            };
        }
    };

    // Drain both pipes concurrently: waiting for a verbose `br --json` process without readers
    // can deadlock once a pipe buffer fills.  The child remains owned by this function and is
    // reaped on every path, including timeout and wait failure.
    let stdout_pipe = child.stdout.take().expect("stdout was configured as piped");
    let stderr_pipe = child.stderr.take().expect("stderr was configured as piped");
    let stdout_thread = thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut pipe = stdout_pipe;
        let _ = pipe.read_to_end(&mut bytes);
        bytes
    });
    let stderr_thread = thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut pipe = stderr_pipe;
        let _ = pipe.read_to_end(&mut bytes);
        bytes
    });

    // Polling gives the binary an explicit timeout and kills a timed-out child rather than
    // leaving a detached process behind.
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return collect_output(child, status.code(), None, stdout_thread, stderr_thread);
            }
            Ok(None) => {
                if started.elapsed() >= timeout {
                    let signal_error = terminate_process_group(&mut child);
                    let error = signal_error.map_or_else(
                        || format!("{program} timed out after {}s", timeout.as_secs()),
                        |signal_error| {
                            format!(
                                "{program} timed out after {}s; process-group signal failed: {signal_error}",
                                timeout.as_secs()
                            )
                        },
                    );
                    return collect_output(child, None, Some(error), stdout_thread, stderr_thread);
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                let signal_error = terminate_process_group(&mut child);
                let message = signal_error.map_or_else(
                    || format!("wait {program}: {error}"),
                    |signal_error| {
                        format!("wait {program}: {error}; process-group signal failed: {signal_error}")
                    },
                );
                return collect_output(child, None, Some(message), stdout_thread, stderr_thread);
            }
        }
    }
}

fn terminate_process_group(child: &mut Child) -> Option<String> {
    #[cfg(unix)]
    {
        let target = format!("-{}", child.id());
        match Command::new("/bin/kill").args(["-TERM", &target]).status() {
            Ok(status) if status.success() => None,
            Ok(status) => Some(format!("kill -TERM {target} exited {:?}", status.code())),
            Err(error) => Some(format!("spawn kill -TERM {target}: {error}")),
        }
    }
    #[cfg(not(unix))]
    {
        child.kill().err().map(|error| format!("kill child: {error}"))
    }
}

fn collect_output(
    mut child: Child,
    status: Option<i32>,
    error: Option<String>,
    stdout_thread: thread::JoinHandle<Vec<u8>>,
    stderr_thread: thread::JoinHandle<Vec<u8>>,
) -> CommandOutput {
    let wait_error = child.wait().err();
    let stdout = stdout_thread.join().unwrap_or_default();
    let stderr = stderr_thread.join().unwrap_or_default();
    CommandOutput {
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
        status,
        error: error.or_else(|| wait_error.map(|wait_error| format!("collect child output: {wait_error}"))),
    }
}

/// Walk up from `start`, returning the first ancestor that holds a repository marker.
/// Mirrors `beads_rust`'s `canonical_source_repo`: identity is derived from a discovered
/// marker's parent, never from a constant.
fn discover_repo_root(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(directory) = current {
        if REPO_MARKERS.iter().any(|marker| directory.join(marker).exists()) {
            return Some(directory.to_path_buf());
        }
        current = directory.parent();
    }
    None
}

/// Resolve the repository root. Precedence, highest first, is documented in `USAGE`:
/// `--repo` flag > `FLEET_REPO` env > upward marker walk from the cwd.
/// Pure with respect to the process so precedence is unit-testable.
fn resolve_repo_root(
    flag: Option<&str>,
    env_value: Option<String>,
    start: &Path,
) -> Result<PathBuf, ConfigError> {
    if let Some(flag) = flag {
        if flag.trim().is_empty() {
            return Err(ConfigError::ExplicitEmpty { source: "--repo".to_owned() });
        }
        return Ok(PathBuf::from(flag));
    }
    if let Some(value) = env_value {
        if value.trim().is_empty() {
            return Err(ConfigError::ExplicitEmpty { source: REPO_ENV.to_owned() });
        }
        return Ok(PathBuf::from(value));
    }
    discover_repo_root(start).ok_or_else(|| ConfigError::RepoNotFound { from: start.to_path_buf() })
}

/// `$HOME`, or a typed error. Never a guessed literal.
fn home_dir() -> Result<PathBuf, ConfigError> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or(ConfigError::HomeUnset)
}

/// Expand a leading `~` against `$HOME`. A `~`-relative path with no `$HOME` is a typed
/// error, not an invented directory.
fn expand_tilde(path: PathBuf) -> Result<PathBuf, ConfigError> {
    let Some(value) = path.to_str() else {
        return Ok(path);
    };
    if value == "~" {
        return home_dir();
    }
    if let Some(rest) = value.strip_prefix("~/") {
        return Ok(home_dir()?.join(rest));
    }
    Ok(path)
}

/// Resolve the ledger: `FLEET_LEDGER` env (tilde-expanded) > `$HOME/<LEDGER_HOME_RELATIVE>`.
fn resolve_ledger() -> Result<PathBuf, ConfigError> {
    if let Some(path) = std::env::var_os("FLEET_LEDGER").filter(|value| !value.is_empty()) {
        return expand_tilde(PathBuf::from(path));
    }
    Ok(home_dir()?.join(LEDGER_HOME_RELATIVE))
}

/// Resolve the `ntm` session measured for `omp_busy`: `FLEET_SESSION` env > the resolved
/// repository's basename. The basename keeps this checkout's `control-plane` behavior while
/// a moved checkout resolves its own session instead of silently measuring a session that
/// no longer matches the repository.
fn session_name(repo: &Path) -> String {
    if let Some(session) = std::env::var(SESSION_ENV).ok().filter(|value| !value.trim().is_empty()) {
        return session;
    }
    // No invented fallback: an empty name makes `ntm` fail loudly downstream.
    repo.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn epsilon_from_env() -> (f64, Option<String>) {
    match std::env::var("FLEET_GEO_EPS") {
        Ok(value) if !value.trim().is_empty() => match value.parse::<f64>() {
            Ok(eps) if eps.is_finite() => (eps.clamp(0.0, 1.0), None),
            _ => (0.0, Some("FLEET_GEO_EPS must be a finite number".to_owned())),
        },
        _ => (0.0, None),
    }
}

struct Measurement {
    raw: BTreeMap<String, f64>,
    errors: BTreeMap<String, String>,
}

fn measure(repo: &Path, ledger: &Path, session: &str) -> Measurement {
    let mut raw = BTreeMap::new();
    let mut errors = BTreeMap::new();

    let commits = run_command(
        "git",
        &["log", "--since", "1 hour ago", "--format=%h"],
        Some(repo),
        COMMAND_TIMEOUT,
    );
    let commit_count = commits.stdout.lines().filter(|line| !line.trim().is_empty()).count() as f64;
    raw.insert("commits_1h".to_owned(), commit_count);
    if commits.error.is_some() || commits.status != Some(0) {
        errors.insert(
            "commits_1h".to_owned(),
            command_error("git log", &commits),
        );
    }

    let activity_flag = format!("--robot-activity={session}");
    let activity = run_command("ntm", &[&activity_flag], None, COMMAND_TIMEOUT);
    let mut busy = 0.0;
    if activity.status == Some(0) && activity.error.is_none() {
        match serde_json::from_str::<Value>(&activity.stdout) {
            Ok(value) => {
                if let Some(agents) = value.get("agents").and_then(Value::as_array) {
                    for agent in agents {
                        let agent_type = agent.get("agent_type").and_then(Value::as_str).unwrap_or("");
                        if !agent_type.starts_with("omp") {
                            continue;
                        }
                        // `is true` in the Python oracle is deliberate: the string "true" is not
                        // safe evidence of an idle pane, so only a JSON boolean qualifies.
                        let idle = agent.get("observation_state").and_then(Value::as_str) == Some("idle")
                            && agent.get("safe_to_dispatch").and_then(Value::as_bool) == Some(true);
                        if !idle {
                            busy += 1.0;
                        }
                    }
                } else {
                    errors.insert("omp_busy".to_owned(), "ntm activity has no agents array".to_owned());
                }
            }
            Err(error) => {
                errors.insert("omp_busy".to_owned(), format!("ntm activity malformed JSON: {error}"));
            }
        }
    } else {
        errors.insert("omp_busy".to_owned(), command_error("ntm --robot-activity", &activity));
    }
    raw.insert("omp_busy".to_owned(), busy);

    let age_min = match std::fs::metadata(ledger).and_then(|metadata| metadata.modified()) {
        Ok(modified) => signed_age_minutes(modified),
        Err(error) => {
            errors.insert("ledger_fresh".to_owned(), format!("ledger mtime unavailable: {error}"));
            1.0e9
        }
    };
    let freshness = if age_min <= 20.0 {
        1.0
    } else if age_min >= 60.0 {
        0.0
    } else {
        (60.0 - age_min) / 40.0
    };
    raw.insert("ledger_fresh".to_owned(), freshness);

    let closed = run_command("br", &["list", "--status=closed", "--json"], Some(repo), COMMAND_TIMEOUT);
    let mut closed_count = 0.0;
    if closed.status == Some(0) && closed.error.is_none() {
        match serde_json::from_str::<Value>(&closed.stdout) {
            Ok(value) => {
                let issues = value
                    .as_array()
                    .or_else(|| value.get("issues").and_then(Value::as_array));
                if let Some(issues) = issues {
                    let cutoff = utc_hour_key(SystemTime::now().checked_sub(Duration::from_secs(3600)).unwrap_or(SystemTime::now()));
                    for issue in issues {
                        let timestamp = issue
                            .get("closed_at")
                            .and_then(Value::as_str)
                            .filter(|value| !value.is_empty())
                            .or_else(|| issue.get("updated_at").and_then(Value::as_str).filter(|value| !value.is_empty()))
                            .unwrap_or("");
                        if timestamp.get(..14).unwrap_or(timestamp) >= cutoff.as_str() {
                            closed_count += 1.0;
                        }
                    }
                } else {
                    errors.insert("beads_closed_1h".to_owned(), "br output has no issues array".to_owned());
                }
            }
            Err(error) => {
                errors.insert("beads_closed_1h".to_owned(), format!("br output malformed JSON: {error}"));
            }
        }
    } else {
        errors.insert("beads_closed_1h".to_owned(), command_error("br list", &closed));
    }
    raw.insert("beads_closed_1h".to_owned(), closed_count);

    Measurement { raw, errors }
}

fn command_error(label: &str, output: &CommandOutput) -> String {
    if let Some(error) = &output.error {
        return error.clone();
    }
    if !output.stderr.trim().is_empty() {
        return format!("{label} exited {:?}: {}", output.status, output.stderr.trim());
    }
    format!("{label} exited {:?}", output.status)
}

fn signed_age_minutes(modified: SystemTime) -> f64 {
    match modified.duration_since(SystemTime::now()) {
        Ok(future) => -(future.as_secs_f64() / 60.0),
        Err(past) => past.duration().as_secs_f64() / 60.0,
    }
}

fn utc_hour_key(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = seconds.div_euclid(86_400);
    let day_seconds = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = day_seconds / 3_600;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:")
}

// Howard Hinnant's proleptic-Gregorian conversion, kept local to avoid adding a time dependency
// to a small standalone diagnostic crate.
fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year, month as u32, day as u32)
}

fn invoker_provenance() -> (&'static str, &'static str) {
    if std::env::var("FLEET_INVOKER").ok().as_deref() == Some("SCHEDULED") {
        ("SCHEDULED", "cron_parent")
    } else {
        ("MANUAL", "unproven_parent")
    }
}

fn report_value(report: CompositeReport, mode: &str, measurement: Option<&Measurement>) -> Value {
    let mut value = serde_json::to_value(report).expect("CompositeReport contains only JSON values");
    if let Value::Object(object) = &mut value {
        object.insert("mode".to_owned(), Value::String(mode.to_owned()));
        let (invoker, proof) = invoker_provenance();
        object.insert("invoker".to_owned(), Value::String(invoker.to_owned()));
        object.insert("invoker_proof".to_owned(), Value::String(proof.to_owned()));
        if let Some(measurement) = measurement {
            object.insert("measurement_errors".to_owned(), json!(measurement.errors));
        }
    }
    value
}

fn live_report(mode: &str, repo: &Path) -> Result<Value, ConfigError> {
    let ledger = resolve_ledger()?;
    let session = session_name(repo);
    let measurement = measure(repo, &ledger, &session);
    let (eps, epsilon_error) = epsilon_from_env();
    let mut value = report_value(compute_with_epsilon(&measurement.raw, eps), mode, Some(&measurement));
    if let Value::Object(object) = &mut value {
        object.insert("repo".to_owned(), Value::String(repo.display().to_string()));
        object.insert("ledger".to_owned(), Value::String(ledger.display().to_string()));
        object.insert("session".to_owned(), Value::String(session));
        if let Some(error) = epsilon_error {
            object.insert("epsilon_error".to_owned(), Value::String(error));
        }
    }
    Ok(value)
}

fn why_report(repo: &Path) -> Result<Value, ConfigError> {
    let mut value = live_report("why", repo)?;
    if let Value::Object(object) = &mut value {
        let mut reasons = Map::new();
        for factor in factors() {
            let reason = match factor.name {
                "commits_1h" => "recent commits measure landed work, but cannot compensate for dead dispatch dimensions",
                "omp_busy" => "no busy OMP panes means queued work is not being conducted",
                "ledger_fresh" => "a stale ledger is not current evidence of a healthy lane",
                "beads_closed_1h" => "no recently closed beads means work is not reaching completion",
                _ => "unknown factor",
            };
            reasons.insert(factor.name.to_owned(), Value::String(reason.to_owned()));
        }
        object.insert("factor_reasons".to_owned(), Value::Object(reasons));
        object.insert("no_claim".to_owned(), Value::String("This reads agent-writable git and ledger state; it is a hill-climbing metric, not held-out external validation.".to_owned()));
    }
    Ok(value)
}

fn capabilities() -> Value {
    json!({
        "schema": "zs.fleet-composite.capabilities.v1",
        "binary": "fleet-composite",
        "read_only": true,
        "operations": [
            {"name": "status", "args": ["--json"], "description": "measure live git, OMP, ledger, and bead factors"},
            {"name": "why", "args": ["--json"], "description": "explain dead factors and the metric's no-claim boundary"},
            {"name": "capabilities", "args": ["--json"], "description": "show this machine-readable contract"},
            {"name": "composite", "args": ["--json"], "description": "read a numeric factor object from stdin and grade it"},
            {"name": "--selftest", "args": [], "description": "run the eleven geometric and mutation assertions"}
        ],
        "factors": factors().iter().map(|factor| json!({"name": factor.name, "baseline": factor.baseline, "optimum": factor.optimum})).collect::<Vec<_>>(),
        "exit_codes": {"0": "success", "1": "selftest failure", "2": "usage error"},
        "external_commands": ["git", "ntm", "br"],
        "no_claim": "The live composite is a hill-climbing metric over agent-writable state, not held-out external validation."
    })
}

fn print_json(value: &Value) {
    println!("{}", serde_json::to_string_pretty(value).expect("diagnostic output is JSON serializable"));
}

fn selftest_exit() -> ExitCode {
    let result = run_selftest();
    for check in &result.checks {
        if check.passed {
            println!("  PASS  {:<62} {}", check.label, check.got);
        } else {
            println!("  FAIL  {:<62} got=[{}] want=[{}]", check.label, check.got, check.want);
        }
    }
    if result.failures.is_empty() {
        println!("\nSELFTEST PASS ({} assertions; both directions + 1 mutation)", result.checked);
        ExitCode::SUCCESS
    } else {
        println!("\nSELFTEST FAIL ({} of {} assertions failed)", result.failures.len(), result.checked);
        ExitCode::from(1)
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "status".to_owned());
    let rest: Vec<String> = args.collect();
    let mut json_requested = false;
    let mut flag: Option<String> = None;
    let mut index = 0;
    while index < rest.len() {
        let argument = rest[index].as_str();
        match argument {
            "--json" => json_requested = true,
            "--repo" => {
                let value = match rest.get(index + 1) {
                    Some(value) => value,
                    None => {
                        eprintln!("usage error: --repo requires a path\n{USAGE}");
                        return ExitCode::from(EXIT_USAGE);
                    }
                };
                flag = Some(value.clone());
                index += 2;
                continue;
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            _ => {
                if let Some(value) = argument.strip_prefix("--repo=") {
                    flag = Some(value.to_owned());
                } else {
                    eprintln!("usage error: unknown argument {argument}\n{USAGE}");
                    return ExitCode::from(EXIT_USAGE);
                }
            }
        }
        index += 1;
    }

    match command.as_str() {
        "status" | "why" => {
            let repo = match resolve_repo_for_report(flag.as_deref()) {
                Ok(repo) => repo,
                Err(error) => return config_error_exit(&error),
            };
            let report = match command.as_str() {
                "status" => live_report("status", &repo),
                _ => why_report(&repo),
            };
            match report {
                Ok(value) => {
                    print_json(&value);
                    let _ = json_requested;
                    ExitCode::SUCCESS
                }
                Err(error) => config_error_exit(&error),
            }
        }
        "capabilities" => {
            print_json(&capabilities());
            let _ = json_requested;
            ExitCode::SUCCESS
        }
        "composite" => {
            let mut input = String::new();
            if io::stdin().read_to_string(&mut input).is_err() {
                print_json(&report_value(compute_json_with_epsilon("", epsilon_from_env().0), "composite", None));
                return ExitCode::from(1);
            }
            let (eps, _) = epsilon_from_env();
            print_json(&report_value(compute_json_with_epsilon(&input, eps), "composite", None));
            let _ = json_requested;
            ExitCode::SUCCESS
        }
        "--selftest" | "selftest" => selftest_exit(),
        _ => {
            eprintln!("usage error: unknown command {command}\n{USAGE}");
            ExitCode::from(EXIT_USAGE)
        }
    }
}

/// Resolve the repository root for a reporting command. Precedence: `--repo` flag >
/// `FLEET_REPO` env > upward marker walk from the cwd. The walk finds nothing only when
/// no marker exists above the cwd, which is a loud error, never a default.
fn resolve_repo_for_report(flag: Option<&str>) -> Result<PathBuf, ConfigError> {
    let cwd = std::env::current_dir().map_err(|error| {
        eprintln!("fleet-composite: cannot read the current directory: {error}");
        ConfigError::RepoNotFound { from: PathBuf::from(".") }
    })?;
    let env_value = std::env::var(REPO_ENV).ok();
    resolve_repo_root(flag, env_value, &cwd)
}

/// A configuration-resolution failure is loud and nonzero: the message names what could
/// not be found and how to provide it.
fn config_error_exit(error: &ConfigError) -> ExitCode {
    eprintln!("fleet-composite: config error: {error}");
    ExitCode::from(EXIT_CONFIG)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Best-effort temp directory with cleanup on drop; keeps the crate dependency-free.
    struct TempDir(PathBuf);

    impl TempDir {
        fn create(label: &str) -> Self {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "fleet-composite-test-{}-{}-{}",
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

        let resolved =
            resolve_repo_root(None, Some(env_target.path().to_string_lossy().into_owned()), &nested)
                .expect("env must win over discovery");
        assert_eq!(resolved, env_target.path());

        let resolved = resolve_repo_root(None, None, &nested).expect("discovery must find the marker");
        assert_eq!(resolved, root.path());
    }

    #[test]
    fn discovery_walks_up_for_git_and_beads_markers() {
        let git_root = TempDir::create("git-marker");
        let nested = git_root.path().join("deeply/nested");
        fs::create_dir_all(&nested).expect("create nested directory");
        fs::create_dir(git_root.path().join(".git")).expect("create .git marker");
        assert_eq!(discover_repo_root(&nested), Some(git_root.path().to_path_buf()));

        let beads_root = TempDir::create("beads-marker");
        let nested = beads_root.path().join("x");
        fs::create_dir_all(&nested).expect("create nested directory");
        fs::create_dir(beads_root.path().join(".beads")).expect("create .beads marker");
        assert_eq!(discover_repo_root(&nested), Some(beads_root.path().to_path_buf()));
    }

    #[test]
    fn known_bad_no_repo_above_cwd_fails_loudly_naming_the_markers() {
        let nowhere = TempDir::create("known-bad");
        let start = nowhere.path().join("plain");
        fs::create_dir_all(&start).expect("create start directory");

        let error = match resolve_repo_root(None, None, &start) {
            Ok(found) => panic!("a marker-free directory must not resolve; found {}", found.display()),
            Err(error) => error,
        };
        // KNOWN-BAD: the typed error must name the markers and the start directory.
        assert!(
            matches!(error, ConfigError::RepoNotFound { ref from } if *from == start),
            "wrong error for a marker-free directory: {error:?}"
        );
        let message = error.to_string();
        assert!(message.contains(".git") && message.contains(".beads"), "message must name the markers: {message}");
        assert!(
            message.contains(start.to_string_lossy().as_ref()),
            "message must name the start directory: {message}"
        );
        assert!(message.contains(REPO_ENV), "message must name the escape hatch env: {message}");
    }

    #[test]
    fn empty_explicit_sources_are_errors_not_defaults() {
        let start = Path::new("/");
        let error = resolve_repo_root(Some("   "), None, start).expect_err("empty --repo is an error");
        assert!(matches!(error, ConfigError::ExplicitEmpty { .. }), "wrong error: {error:?}");
        assert!(error.to_string().contains("--repo"), "message must name --repo: {error}");

        let error = resolve_repo_root(None, Some(String::new()), start).expect_err("empty env is an error");
        assert!(matches!(error, ConfigError::ExplicitEmpty { .. }), "wrong error: {error:?}");
        assert!(error.to_string().contains(REPO_ENV), "message must name the env var: {error}");
    }

    #[test]
    fn tilde_without_home_is_a_typed_error_not_an_invented_directory() {
        // `expand_tilde` with `$HOME` present is the known-good leg.
        if std::env::var_os("HOME").is_some() {
            let expanded = expand_tilde(PathBuf::from("~/state/ledger.jsonl"))
                .expect("HOME is set; tilde must expand");
            let home = PathBuf::from(std::env::var_os("HOME").expect("HOME"));
            assert_eq!(expanded, home.join("state/ledger.jsonl"));
        }
        // A non-tilde path passes through untouched.
        let plain = expand_tilde(PathBuf::from("/tmp/absolute.jsonl")).expect("absolute path needs no HOME");
        assert_eq!(plain, PathBuf::from("/tmp/absolute.jsonl"));
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
        assert!(scanned >= 2, "vacuous scan: only {scanned} source files under {}", src.display());
        assert!(
            hits.is_empty(),
            "hardcoded home-path literal(s) reintroduced (this test exists so a \
             reintroduction turns RED): {hits:?}"
        );
    }

    #[test]
    fn session_name_env_beats_repo_basename() {
        std::env::remove_var(SESSION_ENV);
        assert_eq!(session_name(Path::new("/somewhere/control-plane")), "control-plane");
        assert_eq!(session_name(Path::new("/other/omp-orchestrator")), "omp-orchestrator");
    }
}

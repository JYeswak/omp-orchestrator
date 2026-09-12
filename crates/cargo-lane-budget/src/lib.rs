#![forbid(unsafe_code)]

//! Cargo lane population and admission budget.
//!
//! The missing shell oracle is DELIBERATELY_NOT:
//! owner=n7mjb; dies_when=an independent Rust reference algorithm or a recovered external oracle
//! is specified. This crate owns the Rust decision logic. Derive the lane ceiling from the
//! measured tmux session count, count lane markers with a bounded walk, and use APFS
//! `Capacity Not Allocated` for the shared-container bound. tmux, diskutil, df, and
//! find-shaped filesystem inspection are external boundaries; no shell or Python is used for decisions.

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use subprocess_contract::{bounded_output, BoundedOutcome};

pub const DEFAULT_LANES_PER_SESSION: u64 = 22;
pub const DEFAULT_HEADROOM: u64 = 29;
pub const DEFAULT_FLOOR: u64 = 96;
pub const DEFAULT_MIN_FREE_GIB: f64 = 10.0;
pub const DEFAULT_AGGREGATE_FLOOR_BYTES: u64 = 53_687_091_200;
pub const DEFAULT_AGGREGATE_CONTAINER: &str = "disk3";
pub const DEFAULT_SCAN_DEPTH: usize = 6;
pub const DEFAULT_SCAN_RETRIES: u32 = 4;
pub const DEFAULT_SCAN_DEADLINE_SECS: u64 = 45;
pub const DEFAULT_CACHE_MAX_AGE_SECS: u64 = 2_700;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Pass,
    Red,
    Unknown,
    Config,
}

impl Status {
    pub const fn code(self) -> i32 {
        match self {
            Self::Pass => 0,
            Self::Red => 78,
            Self::Unknown => 77,
            Self::Config => 1,
        }
    }
}

#[derive(Debug)]
pub struct Report {
    pub status: Status,
    pub lines: Vec<String>,
}

impl Report {
    fn pass(lines: Vec<String>) -> Self {
        Self {
            status: Status::Pass,
            lines,
        }
    }

    fn red(lines: Vec<String>) -> Self {
        Self {
            status: Status::Red,
            lines,
        }
    }

    fn unknown(lines: Vec<String>) -> Self {
        Self {
            status: Status::Unknown,
            lines,
        }
    }

    fn config(lines: Vec<String>) -> Self {
        Self {
            status: Status::Config,
            lines,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub lane_budget: Option<u64>,
    pub lanes_per_session: u64,
    pub headroom: u64,
    pub floor: u64,
    pub tmux_bin: String,
    pub scan_roots: Option<String>,
    pub scan_depth: usize,
    pub scan_deadline_secs: u64,
    pub scan_retries: u32,
    pub cache_max_age_secs: u64,
    pub cache_file: PathBuf,
    pub ledger: PathBuf,
    pub byte_root: PathBuf,
    pub min_free_gib: f64,
    pub aggregate_bound: bool,
    pub aggregate_container: String,
    pub aggregate_floor_bytes: u64,
    pub aggregate_fixture: Option<PathBuf>,
    pub diskutil_bin: String,
}

fn env_string(name: &str) -> Option<String> {
    env::var(name).ok().filter(|s| !s.is_empty())
}

fn parse_u64(name: &str, default: u64) -> Result<u64, String> {
    match env_string(name) {
        None => Ok(default),
        Some(value) => value.parse::<u64>().map_err(|_| format!("{name}={value}")),
    }
}

fn parse_bool(name: &str, default: bool) -> Result<bool, String> {
    match env_string(name) {
        None => Ok(default),
        Some(value) => match value.as_str() {
            "0" => Ok(false),
            "1" => Ok(true),
            _ => Err(format!("{name}={value}")),
        },
    }
}

pub fn config_from_env() -> Result<Config, String> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let cache_file = env_string("CARGO_LANE_CACHE_FILE")
        .or_else(|| env_string("CARGO_LANE_OFFLOAD_CACHE_FILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/state/flywheel/cargo-lane-offload-count.tsv"));
    let lane_budget = env_string("CARGO_LANE_BUDGET")
        .map(|v| {
            v.parse::<u64>()
                .map_err(|_| format!("CARGO_LANE_BUDGET={v}"))
        })
        .transpose()?;
    let lanes_per_session = parse_u64("CARGO_LANE_LANES_PER_SESSION", DEFAULT_LANES_PER_SESSION)?;
    let headroom = parse_u64("CARGO_LANE_BUDGET_HEADROOM", DEFAULT_HEADROOM)?;
    let floor = parse_u64("CARGO_LANE_BUDGET_FLOOR", DEFAULT_FLOOR)?;
    let scan_depth = parse_u64("CARGO_LANE_SCAN_DEPTH", DEFAULT_SCAN_DEPTH as u64)? as usize;
    let scan_deadline_secs = parse_u64(
        "CARGO_LANE_SCAN_RETRY_DEADLINE_SECONDS",
        DEFAULT_SCAN_DEADLINE_SECS,
    )?;
    let scan_retries = parse_u64("CARGO_LANE_SCAN_RETRIES", DEFAULT_SCAN_RETRIES as u64)? as u32;
    let cache_max_age_secs = parse_u64(
        "CARGO_LANE_OFFLOAD_CACHE_MAX_AGE_SECONDS",
        DEFAULT_CACHE_MAX_AGE_SECS,
    )?;
    let min_free_gib = env_string("CARGO_LANE_MIN_FREE_GIB")
        .map(|v| {
            v.parse::<f64>()
                .map_err(|_| format!("CARGO_LANE_MIN_FREE_GIB={v}"))
        })
        .transpose()?
        .unwrap_or(DEFAULT_MIN_FREE_GIB);
    let aggregate_bound = parse_bool("CARGO_LANE_AGGREGATE_BOUND", true)?;
    let aggregate_floor_bytes = parse_u64(
        "CARGO_LANE_AGGREGATE_FLOOR_BYTES",
        DEFAULT_AGGREGATE_FLOOR_BYTES,
    )?;
    Ok(Config {
        lane_budget,
        lanes_per_session,
        headroom,
        floor,
        tmux_bin: env_string("CARGO_LANE_TMUX_BIN").unwrap_or_else(|| "tmux".into()),
        scan_roots: env_string("CARGO_LANE_SCAN_ROOTS"),
        scan_depth,
        scan_deadline_secs,
        scan_retries,
        cache_max_age_secs,
        cache_file,
        ledger: env_string("CARGO_LANE_BUDGET_LEDGER")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state/flywheel/cargo-lane-budget.jsonl")),
        byte_root: env_string("CARGO_LANE_BYTE_CHECK_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/")),
        min_free_gib,
        aggregate_bound,
        aggregate_container: env_string("CARGO_LANE_AGGREGATE_CONTAINER")
            .unwrap_or_else(|| DEFAULT_AGGREGATE_CONTAINER.into()),
        aggregate_floor_bytes,
        aggregate_fixture: env_string("CARGO_LANE_AGGREGATE_FIXTURE").map(PathBuf::from),
        diskutil_bin: env_string("CARGO_LANE_AGGREGATE_DISKUTIL")
            .unwrap_or_else(|| "/usr/sbin/diskutil".into()),
    })
}

#[derive(Debug)]
enum RunError {
    Spawn(String),
    Io(String),
    Timeout,
    Exit(i32),
}

#[derive(Debug)]
struct RunOutput {
    stdout: String,
}

/// Run one external boundary with a wall-clock deadline.
///
/// DRAIN THE PIPES ON DEDICATED THREADS. `try_wait` in a poll loop CANNOT be paired with
/// undrained pipes: a child that writes past the OS pipe buffer (~64 KiB, and stdout and
/// stderr each have their own) blocks in `write` forever, so it never exits, so `try_wait`
/// never returns `Some`, and the call burns its entire timeout at 0% CPU before being killed.
/// The pre-extraction source read the pipes only inside the exited arm — which is exactly
fn run_bounded(program: &str, args: &[String], timeout: Duration) -> Result<RunOutput, RunError> {
    let mut command = Command::new(program);
    command.args(args);
    match bounded_output(&mut command, timeout) {
        BoundedOutcome::Completed(output) => {
            let result = RunOutput {
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            };
            if output.status.success() {
                Ok(result)
            } else {
                Err(RunError::Exit(output.status.code().unwrap_or(1)))
            }
        }
        BoundedOutcome::TimedOut => Err(RunError::Timeout),
        BoundedOutcome::Unspawned(error) => Err(RunError::Spawn(format!("{program}: {error}"))),
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn derive_budget(sessions: u64, lanes_per_session: u64, headroom: u64, floor: u64) -> u64 {
    std::cmp::max(
        floor,
        sessions
            .saturating_mul(lanes_per_session)
            .saturating_add(headroom),
    )
}

pub fn resolve_lane_identity(task: Option<&str>, session: &str, isolated: bool) -> Option<String> {
    if session.is_empty() {
        return None;
    }
    let raw = if isolated {
        format!("{}-{}", session, task?)
    } else {
        session.to_string()
    };
    let sanitized: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '%' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    (!sanitized.is_empty()).then_some(sanitized)
}

pub fn root_set(config: &Config) -> Vec<PathBuf> {
    let extra = env_string("CARGO_LANE_OFFLOAD_EXTRA_ROOT")
        .or_else(|| env_string("TMP_TARGET_OFFLOAD_EXTRA_ROOT"))
        .unwrap_or_else(|| {
            "/Volumes/ZestData/zeststream-offload-20260609/build-cache/franken-harvest-cargo-targets"
                .into()
        });
    let roots: Vec<String> = if let Some(override_roots) = &config.scan_roots {
        override_roots.split(':').map(str::to_string).collect()
    } else {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        vec![
            env_string("CARGO_LANE_OFFLOAD_ROOT")
                .or_else(|| env_string("TMP_TARGET_OFFLOAD_ROOT"))
                .unwrap_or_else(|| {
                    "/Volumes/ZestData/zeststream-offload-20260609/build-cache/cargo-targets".into()
                }),
            env_string("CARGO_LANE_PRIVATE_TMP_ROOT")
                .or_else(|| env_string("TMP_TARGET_PRIVATE_TMP_ROOT"))
                .unwrap_or_else(|| "/private/tmp".into()),
            env_string("CARGO_LANE_CACHE_ROOT")
                .or_else(|| env_string("TMP_TARGET_CACHE_ROOT"))
                .unwrap_or_else(|| home.join("Library/Caches").display().to_string()),
            env_string("CARGO_LANE_TMP_ROOT")
                .or_else(|| env_string("TMP_TARGET_TMP_ROOT"))
                .or_else(|| env::var("TMPDIR").ok())
                .unwrap_or_else(|| "/tmp".into()),
        ]
    };
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for root in roots {
        if !root.is_empty() && seen.insert(root.clone()) {
            result.push(PathBuf::from(root));
        }
    }
    if env_string("CARGO_LANE_ROOTS_SELFTEST").as_deref() == Some("1") {
        if let Some(extra_root) = env_string("CARGO_LANE_ROOTS_TEST_APPEND") {
            if seen.insert(extra_root.clone()) {
                result.push(PathBuf::from(extra_root));
            }
        }
    }
    let _ = extra; // The excluded Franken-harvest root is reported, never scanned.
    result
}

#[derive(Debug, Default)]
struct ScanResult {
    markers: Vec<PathBuf>,
    incomplete: bool,
    unmeasured: usize,
}

fn scan_dir(path: &Path, depth: usize, limit: Instant, result: &mut ScanResult) {
    if Instant::now() >= limit {
        result.incomplete = true;
        result.unmeasured += 1;
        return;
    }
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => {
            result.incomplete = true;
            result.unmeasured += 1;
            return;
        }
    };
    for entry in entries.flatten() {
        if Instant::now() >= limit {
            result.incomplete = true;
            result.unmeasured += 1;
            return;
        }
        let entry_path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            result.incomplete = true;
            result.unmeasured += 1;
            continue;
        };
        if !file_type.is_dir() {
            if entry.file_name() == "CACHEDIR.TAG" {
                result.markers.push(entry_path);
            }
            continue;
        }
        if depth > 0 && !file_type.is_symlink() {
            scan_dir(&entry_path, depth - 1, limit, result);
        }
    }
}

fn read_cache(path: &Path, root: &Path, max_age: u64) -> Option<(u64, u64)> {
    let text = fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let mut fields = line.split('\t');
        let epoch = fields.next()?.parse::<u64>().ok()?;
        let count = fields.next()?.parse::<u64>().ok()?;
        let cached_root = fields.next()?;
        if cached_root == root.to_string_lossy() {
            let now = now_secs();
            if now >= epoch && now - epoch <= max_age {
                return Some((count, now - epoch));
            }
        }
    }
    None
}

fn write_cache(path: &Path, root: &Path, count: u64) {
    let mut rows: HashMap<String, (u64, u64)> = HashMap::new();
    if let Ok(text) = fs::read_to_string(path) {
        for line in text.lines() {
            let mut fields = line.split('\t');
            let (Some(epoch), Some(value), Some(key)) =
                (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            if let (Ok(epoch), Ok(value)) = (epoch.parse(), value.parse()) {
                rows.insert(key.to_string(), (epoch, value));
            }
        }
    }
    rows.insert(root.to_string_lossy().into_owned(), (now_secs(), count));
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    if let Ok(mut file) = File::create(&tmp) {
        for (key, (epoch, value)) in rows {
            let _ = writeln!(file, "{epoch}\t{value}\t{key}");
        }
        let _ = fs::rename(tmp, path);
    }
}

fn session_count(config: &Config) -> Result<u64, String> {
    let args = vec![
        "list-sessions".into(),
        "-F".into(),
        "#{session_name}".into(),
    ];
    let output = run_bounded(&config.tmux_bin, &args, Duration::from_secs(5))
        .map_err(|e| format_run_error(e, "tmux list-sessions"))?;
    let count = output
        .stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count() as u64;
    (count > 0)
        .then_some(count)
        .ok_or_else(|| "empty session list".into())
}

fn format_run_error(error: RunError, operation: &str) -> String {
    match error {
        RunError::Spawn(detail) => format!("{operation}: spawn failed: {detail}"),
        RunError::Io(detail) => format!("{operation}: I/O failed: {detail}"),
        RunError::Timeout => format!("{operation}: timeout"),
        RunError::Exit(code) => format!("{operation}: exit={code}"),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregateStats {
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub quota_used_bytes: u64,
    pub quota_sum_bytes: u64,
    pub allowed_bytes: u64,
    pub headroom_bytes: i128,
    pub quota_count: usize,
}

fn number_after(line: &str, label: &str) -> Option<u64> {
    let rest = line.split_once(label)?.1.trim();
    let number = rest.split_whitespace().next()?;
    number.parse::<u64>().ok()
}

/// Parse the authoritative APFS container measurement.  `df` is deliberately
/// absent here: its TOTAL/AVAIL columns are per-volume quota views and double-count
/// shared container bytes.  Only `Capacity Not Allocated` is the free denominator.
pub fn parse_aggregate(text: &str, floor_bytes: u64) -> Result<AggregateStats, String> {
    if text.trim().is_empty() {
        return Err("diskutil_empty".into());
    }
    let total_bytes = text
        .lines()
        .find_map(|line| number_after(line, "Size (Capacity Ceiling):"))
        .filter(|v| *v > 0)
        .ok_or_else(|| "container_total_absent".to_string())?;
    let free_bytes = text
        .lines()
        .find_map(|line| number_after(line, "Capacity Not Allocated:"))
        .ok_or_else(|| "container_free_absent".to_string())?;
    let mut quota_used_bytes: u64 = 0;
    let mut quota_sum_bytes: u64 = 0;
    let mut quota_count = 0;
    let mut pending_quota = None;
    for line in text.lines() {
        if let Some(quota) = number_after(line, "Capacity Quota:") {
            pending_quota = Some(quota);
        }
        if let Some(consumed) = number_after(line, "Capacity Consumed:") {
            if let Some(quota) = pending_quota.take() {
                quota_sum_bytes = quota_sum_bytes.saturating_add(quota);
                quota_used_bytes = quota_used_bytes.saturating_add(consumed);
                quota_count += 1;
            }
        }
    }
    if quota_count == 0 {
        return Err("no_quotas_parsed".into());
    }
    let available_growth = free_bytes.saturating_sub(floor_bytes);
    let allowed_bytes = quota_used_bytes.saturating_add(available_growth);
    Ok(AggregateStats {
        total_bytes,
        free_bytes,
        quota_used_bytes,
        quota_sum_bytes,
        allowed_bytes,
        headroom_bytes: allowed_bytes as i128 - quota_sum_bytes as i128,
        quota_count,
    })
}

fn free_gib(root: &Path) -> Result<f64, String> {
    let args = vec!["-Pk".into(), root.display().to_string()];
    let output =
        run_bounded("df", &args, Duration::from_secs(5)).map_err(|e| format_run_error(e, "df"))?;
    let row = output
        .stdout
        .lines()
        .nth(1)
        .ok_or_else(|| "df_row_absent".to_string())?;
    let free_blocks = row
        .split_whitespace()
        .nth(3)
        .ok_or_else(|| "df_free_absent".to_string())?
        .parse::<u64>()
        .map_err(|_| "df_free_unparseable".to_string())?;
    Ok(free_blocks as f64 / 1_048_576.0)
}

fn aggregate_stats(config: &Config) -> Result<AggregateStats, String> {
    let text = if let Some(fixture) = &config.aggregate_fixture {
        fs::read_to_string(fixture).map_err(|e| format!("fixture_read: {e}"))?
    } else {
        let args = vec![
            "apfs".into(),
            "list".into(),
            config.aggregate_container.clone(),
        ];
        run_bounded(&config.diskutil_bin, &args, Duration::from_secs(30))
            .map_err(|e| format_run_error(e, "diskutil apfs list"))?
            .stdout
    };
    parse_aggregate(&text, config.aggregate_floor_bytes)
}

#[derive(Serialize)]
struct LedgerRow<'a> {
    ts: u64,
    event: &'a str,
    verdict: &'a str,
    rc: i32,
    reason: &'a str,
    observed: u64,
    budget: u64,
    warn_at: u64,
    warn: bool,
    count_headroom: u64,
    budget_source: &'a str,
    budget_sessions: Option<u64>,
    free_gib: Option<f64>,
    min_free_gib: f64,
    aggregate_free_bytes: Option<u64>,
    aggregate_headroom_bytes: Option<i128>,
}

fn record_ledger(config: &Config, row: LedgerRow<'_>) {
    if let Some(parent) = config.ledger.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(serialized) = serde_json::to_string(&row) {
        if let Ok(mut file) = File::options()
            .create(true)
            .append(true)
            .open(&config.ledger)
        {
            let _ = writeln!(file, "{serialized}");
        }
    }
}

fn verdict_name(status: Status) -> &'static str {
    match status {
        Status::Pass => "PASS",
        Status::Red => "RED",
        Status::Unknown => "UNKNOWN",
        Status::Config => "RED",
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CountAssessment {
    status: Status,
    warn: bool,
    warn_at: u64,
    headroom: u64,
}

fn assess_count(observed: u64, budget: u64, configured_headroom: u64) -> CountAssessment {
    let warn_at = budget.saturating_sub(configured_headroom).max(1);
    if observed >= budget {
        CountAssessment {
            status: Status::Red,
            warn: false,
            warn_at,
            headroom: budget.saturating_sub(observed),
        }
    } else {
        CountAssessment {
            status: Status::Pass,
            warn: observed >= warn_at,
            warn_at,
            headroom: budget - observed,
        }
    }
}

pub fn check(config: &Config) -> Report {
    if config.lanes_per_session == 0
        || config.floor == 0
        || config.headroom > config.lanes_per_session * 2
    {
        return Report::config(vec![format!(
            "CARGO_LANE_BUDGET RED configuration lanes_per_session={} headroom={} floor={}",
            config.lanes_per_session, config.headroom, config.floor
        )]);
    }
    let (budget, source, sessions) = if let Some(override_budget) = config.lane_budget {
        (override_budget, "override", None)
    } else {
        match session_count(config) {
            Ok(sessions) => (
                derive_budget(
                    sessions,
                    config.lanes_per_session,
                    config.headroom,
                    config.floor,
                ),
                "derived",
                Some(sessions),
            ),
            Err(detail) => {
                return Report::unknown(vec![format!(
                    "CARGO_LANE_BUDGET UNKNOWN budget_underived detail={detail} tmux_bin={}",
                    config.tmux_bin
                )]);
            }
        }
    };
    let roots = root_set(config);
    let mut seen_markers = HashSet::new();
    let mut lines = Vec::new();
    let mut incomplete = false;
    for root in &roots {
        if !root.is_dir() {
            continue;
        }
        let deadline = Instant::now() + Duration::from_secs(config.scan_deadline_secs);
        let mut scan = ScanResult::default();
        scan_dir(root, config.scan_depth, deadline, &mut scan);
        let mut markers: Vec<PathBuf> = scan.markers;
        markers.sort();
        markers.dedup();
        let mut count = 0_u64;
        for marker in markers {
            if seen_markers.insert(marker) {
                count += 1;
            }
        }
        if scan.incomplete {
            if let Some((cached, age)) =
                read_cache(&config.cache_file, root, config.cache_max_age_secs)
            {
                count = cached;
                lines.push(format!(
                    "CARGO_LANE_BUDGET root={} markers={} estimated=1 cache_age_seconds={age}",
                    root.display(),
                    count
                ));
            } else {
                incomplete = true;
                lines.push(format!(
                    "CARGO_LANE_BUDGET UNKNOWN root={} coverage=incomplete measured_subtrees=0 unmeasured_subtrees={} first_unmeasured={}",
                    root.display(), scan.unmeasured, root.display()
                ));
            }
        } else {
            write_cache(&config.cache_file, root, count);
            lines.push(format!(
                "CARGO_LANE_BUDGET root={} markers={} estimated=0",
                root.display(),
                count
            ));
        }
    }
    let observed = seen_markers.len() as u64;
    let count = assess_count(observed, budget, config.headroom);
    lines.push(format!(
        "CARGO_LANE_BUDGET budget_derivation source={source} sessions={} lanes_per_session={} headroom={} floor={} budget={budget} warn_at={}",
        sessions.map_or_else(|| "none".into(), |s| s.to_string()),
        config.lanes_per_session,
        config.headroom,
        config.floor,
        count.warn_at
    ));
    lines.push(format!(
        "CARGO_LANE_BUDGET observed={observed} budget={budget} count_headroom={}",
        count.headroom
    ));
    if count.status == Status::Red {
        lines.push(format!(
            "CARGO_LANE_BUDGET RED lane_roots_exceed_budget over={}",
            observed - budget
        ));
        record_ledger(
            config,
            LedgerRow {
                ts: now_secs(),
                event: "cargo_lane_budget",
                verdict: "RED",
                rc: Status::Red.code(),
                reason: "lane_roots_exceed_budget",
                observed,
                budget,
                warn_at: count.warn_at,
                warn: count.warn,
                count_headroom: count.headroom,
                budget_source: source,
                budget_sessions: sessions,
                free_gib: None,
                min_free_gib: config.min_free_gib,
                aggregate_free_bytes: None,
                aggregate_headroom_bytes: None,
            },
        );
        return Report::red(lines);
    }
    if incomplete {
        lines.push(format!(
            "CARGO_LANE_BUDGET UNKNOWN scan_incomplete observed={observed} budget={budget}"
        ));
        record_ledger(
            config,
            LedgerRow {
                ts: now_secs(),
                event: "cargo_lane_budget",
                verdict: "UNKNOWN",
                rc: Status::Unknown.code(),
                reason: "scan_incomplete",
                observed,
                budget,
                warn_at: count.warn_at,
                warn: count.warn,
                count_headroom: count.headroom,
                budget_source: source,
                budget_sessions: sessions,
                free_gib: None,
                min_free_gib: config.min_free_gib,
                aggregate_free_bytes: None,
                aggregate_headroom_bytes: None,
            },
        );
        return Report::unknown(lines);
    }
    let free = match free_gib(&config.byte_root) {
        Ok(free) => free,
        Err(detail) => {
            lines.push(format!(
                "CARGO_LANE_BUDGET UNKNOWN free_space_unreadable root={} detail={detail}",
                config.byte_root.display()
            ));
            return Report::unknown(lines);
        }
    };
    lines.push(format!(
        "CARGO_LANE_BUDGET free_gib={free:.1} min_free_gib={:.1} root={}",
        config.min_free_gib,
        config.byte_root.display()
    ));
    if free < config.min_free_gib {
        lines.push(format!(
            "CARGO_LANE_BUDGET RED free_space_below_floor free_gib={free:.1} min_gib={:.1} lane_roots={observed}",
            config.min_free_gib
        ));
        return Report::red(lines);
    }
    let mut aggregate = None;
    if config.aggregate_bound {
        match aggregate_stats(config) {
            Ok(stats) if stats.quota_sum_bytes > stats.allowed_bytes => {
                let over = stats.quota_sum_bytes - stats.allowed_bytes;
                lines.push(format!(
                    "CARGO_LANE_BUDGET RED aggregate_quota_overcommit container={} quota_sum_bytes={} allowed_bytes={} over_bytes={} free_bytes={}",
                    config.aggregate_container,
                    stats.quota_sum_bytes,
                    stats.allowed_bytes,
                    over,
                    stats.free_bytes
                ));
                return Report::red(lines);
            }
            Ok(stats) => {
                lines.push(format!(
                    "CARGO_LANE_BUDGET aggregate_bound container={} quotas={} quota_sum_bytes={} allowed_bytes={} headroom_bytes={} free_bytes={}",
                    config.aggregate_container,
                    stats.quota_count,
                    stats.quota_sum_bytes,
                    stats.allowed_bytes,
                    stats.headroom_bytes,
                    stats.free_bytes
                ));
                aggregate = Some(stats);
            }
            Err(detail) => {
                lines.push(format!(
                    "CARGO_LANE_BUDGET UNKNOWN aggregate_bound_unreadable container={} detail={detail}",
                    config.aggregate_container
                ));
                return Report::unknown(lines);
            }
        }
    }
    if count.warn {
        lines.push(format!(
            "CARGO_LANE_BUDGET WARN lane_roots_near_budget lane_roots={observed} budget={budget} warn_at={} headroom={}",
            count.warn_at, count.headroom
        ));
    }
    lines.push(format!(
        "CARGO_LANE_BUDGET PASS lane_roots={observed} budget={budget}"
    ));
    record_ledger(
        config,
        LedgerRow {
            ts: now_secs(),
            event: "cargo_lane_budget",
            verdict: "PASS",
            rc: Status::Pass.code(),
            reason: "pass",
            observed,
            budget,
            warn_at: count.warn_at,
            warn: count.warn,
            count_headroom: count.headroom,
            budget_source: source,
            budget_sessions: sessions,
            free_gib: Some(free),
            min_free_gib: config.min_free_gib,
            aggregate_free_bytes: aggregate.as_ref().map(|a| a.free_bytes),
            aggregate_headroom_bytes: aggregate.as_ref().map(|a| a.headroom_bytes),
        },
    );
    Report::pass(lines)
}

pub fn packet_contract(session: &str, pane: &str) -> Option<String> {
    let lane = resolve_lane_identity(None, session, false)?;
    Some(format!(
        "Cargo target lane contract (control-plane#cp-xxu9):\n  worker identity: session={session} (pane={pane} is a volatile routing handle); shared lane key={lane}\n  shared work: derive the target lane from the worker/session identity, never from a bead,\n    task, attempt, commit, or human-readable prompt name.\n  isolation escape hatch: only genuinely incompatible work may set the target repo's\n    *_LANE_ISOLATED=1 (or equivalent) and supply an explicit unique task lane.\n  before building: do not invent a new target root; reuse the session lane and report any\n    wrapper that ignores this contract to its owning repository."
    ))
}

/// Exit code for a RED selftest.
///
/// Deliberately the EXISTING `1`, not a newly allocated number. `XC-001` already carries
/// "a gate refused", and a RED selftest is exactly that meaning — so nothing new lands in
/// the `1`-`4` legacy band, which the registry's band rule closes to new semantics ("do not
/// add meanings; new semantics go to an unallocated band"). The failure COUNT deliberately
/// does not become the code: 256 distinct meanings is not a contract.
pub const SELFTEST_RED_EXIT: u8 = 1;

/// Collapse a selftest failure count into a process exit code **without narrowing**.
///
/// THE DEFECT THIS REPLACES (bead `omp-orchestrator-n34x`): `main.rs` forwarded the count as
/// `ExitCode::from(selftest() as u8)`. In Rust `as` between integers WRAPS, it does not
/// saturate, so `256 as u8 == 0` — the process would exit SUCCESS while its own stdout said
/// `SELFTEST RED cargo-lane-budget failures=256`. Latent only because both `return` arms were
/// literals; the obvious next edit (return the already-computed count, which is strictly more
/// informative) activates it. Every multiple of 256 is an activation value.
///
/// This function is total over `i32` and contains no numeric conversion at all: the codes are
/// `u8` literals. Truncation is not fixed here, it is inexpressible.
///
/// On the domain `selftest` can actually return — `{0, 1}` — this agrees with the old cast
/// byte for byte, which is what makes the change safe in a crate that has no equivalence
/// oracle in this repo (EE-P4). It diverges only where the old cast was wrong.
pub fn selftest_exit_code(failures: i32) -> u8 {
    if failures == 0 {
        0
    } else {
        SELFTEST_RED_EXIT
    }
}

/// The selftest summary line and its exit code, derived from ONE value.
///
/// The whole defect in `n34x` is that the emitted line and the exit status could DISAGREE, so
/// an exit-code-only assertion could not see it (`AGENTS.md` gate rule 7). Producing both from
/// a single argument makes disagreement unrepresentable rather than merely untested.
pub fn selftest_summary(failures: i32) -> (String, u8) {
    if failures == 0 {
        (
            "SELFTEST PASS cargo-lane-budget".to_owned(),
            selftest_exit_code(failures),
        )
    } else {
        (
            format!("SELFTEST RED cargo-lane-budget failures={failures}"),
            selftest_exit_code(failures),
        )
    }
}

pub fn selftest() -> i32 {
    let mut failures = 0;
    if derive_budget(3, 22, 29, 96) == 96 && derive_budget(7, 22, 29, 96) == 183 {
        println!("SELFTEST PASS derived-budget sessions=3=>96 sessions=7=>183");
    } else {
        println!("SELFTEST RED derived-budget");
        failures += 1;
    }
    let fixture = "Size (Capacity Ceiling): 1000 B\nCapacity Not Allocated: 600 B\nCapacity Quota: 400 B\nCapacity Consumed: 100 B\n";
    match parse_aggregate(fixture, 100) {
        Ok(stats) if stats.free_bytes == 600 && stats.allowed_bytes == 600 => {
            println!("SELFTEST PASS container-vs-df uses Capacity Not Allocated=600");
        }
        _ => {
            println!("SELFTEST RED container-vs-df predicate");
            failures += 1;
        }
    }
    if resolve_lane_identity(Some("task"), "session", false)
        == resolve_lane_identity(Some("other"), "session", false)
    {
        println!("SELFTEST PASS shared-lane identity");
    } else {
        println!("SELFTEST RED shared-lane identity");
        failures += 1;
    }
    let (summary, code) = selftest_summary(failures);
    println!("{summary}");
    i32::from(code)
}

pub fn print_report(report: &Report) -> i32 {
    for line in &report.lines {
        println!("{line}");
    }
    report.status.code()
}

pub fn status_name(status: Status) -> &'static str {
    verdict_name(status)
}

pub fn read_to_string(path: &Path) -> io::Result<String> {
    fs::read_to_string(path)
}
#[cfg(test)]
mod tests {
    use super::{
        assess_count, check, selftest, selftest_exit_code, selftest_summary, Config, Status,
        SELFTEST_RED_EXIT,
    };
    use std::{fs, path::PathBuf};

    /// THE CLOSED CODE SPACE `XC-PT-SELFTEST` CLAIMS.
    ///
    /// `main.rs:24` forwards this function's return value as the process exit code
    /// (`ExitCode::from(selftest() as u8)`), which the exit-code registry gate records as
    /// a pass-through site. §6 declares its code space CLOSED at {0,1} rather than the
    /// 0..=255-of-a-child default, on the grounds that the two `return` arms below are
    /// literal `0` and literal `1` and the local `failures` counter is only interpolated
    /// into the printed message. This test is what makes that a checked claim.
    ///
    /// **THE HAZARD THIS GUARDS, and it is not hypothetical.** `as u8` NARROWS an `i32`.
    /// The obvious next edit is to return `failures` instead of `1` — the count is already
    /// computed and already printed one line above the return. At `failures == 256` the
    /// cast truncates to **0**, so `SELFTEST RED cargo-lane-budget failures=256` prints on
    /// stdout while the process exits SUCCESS. That is a vacuous green arriving through a
    /// cast rather than through a missing check, and this assertion fails the moment the
    /// return value leaves {0,1}.
    #[test]
    fn selftest_returns_only_a_documented_exit_code() {
        let code = selftest();
        assert!(
            code == 0 || code == 1,
            "selftest() returned {code}, outside the {{0,1}} space XC-PT-SELFTEST declares. \
             main.rs narrows this through `as u8`, so a value of 256 would exit SUCCESS \
             while printing RED."
        );
        assert!(
            u8::try_from(code).is_ok(),
            "selftest() returned {code}, which `ExitCode::from(selftest() as u8)` at \
             main.rs:24 would TRUNCATE rather than refuse"
        );
    }

    /// KNOWN-BAD LEG, FIRING AT THE EXACT ACTIVATION VALUE — bead `omp-orchestrator-n34x`.
    ///
    /// The defect is invisible at every value below 256 and at every value that is not a
    /// multiple of it, which is precisely why it survived review. A leg at `failures = 1`
    /// proves nothing. This one first PINS THE MECHANISM — `256_i32 as u8 == 0`, the wrapping
    /// the old `main.rs:24` performed — and then asserts the replacement refuses it.
    #[test]
    fn exit_code_does_not_truncate_at_256() {
        // The mechanism, stated as an assertion rather than as prose: this is what the old
        // `ExitCode::from(selftest() as u8)` did with a count of 256.
        assert_eq!(
            256_i32 as u8, 0,
            "the premise of this bead: `as` wraps, so 256 became a SUCCESS exit"
        );

        // The replacement, at 256 and at every other multiple that would have wrapped.
        for failures in [256, 512, 768, 65_536, 16_777_216] {
            assert_ne!(
                selftest_exit_code(failures),
                0,
                "failures={failures} truncated to a SUCCESS exit; the n34x defect is back"
            );
            assert_eq!(selftest_exit_code(failures), SELFTEST_RED_EXIT);
        }

        // i32::MIN is the other value `as u8` mishandles silently.
        assert_ne!(selftest_exit_code(i32::MIN), 0);
        assert_ne!(selftest_exit_code(-1), 0);
    }

    /// KNOWN-GOOD LEG: zero failures still exits 0, and on the domain `selftest` can actually
    /// return the new conversion agrees with the old cast BYTE FOR BYTE. That equivalence is
    /// what makes this change safe in a crate with no equivalence oracle in this repo (EE-P4):
    /// it diverges from the old behaviour only where the old behaviour was wrong.
    #[test]
    fn exit_code_agrees_with_the_old_cast_on_the_reachable_domain() {
        assert_eq!(selftest_exit_code(0), 0);
        for reachable in [0_i32, 1] {
            assert_eq!(
                selftest_exit_code(reachable),
                reachable as u8,
                "behaviour changed on a value selftest can actually return"
            );
        }
    }

    /// ASSERT THE MESSAGE, NOT JUST THE CODE — `AGENTS.md` gate rule 7.
    ///
    /// The entire defect is that the emitted line and the exit status DISAGREE, so an
    /// exit-code-only assertion is blind to it. This reads both, at 256 specifically, and
    /// pins that the RED line still names the true count even though the code does not carry
    /// it — the count belongs in the message, never in the exit status.
    #[test]
    fn the_line_and_the_status_cannot_disagree() {
        let (line, code) = selftest_summary(256);
        assert!(
            line.contains("SELFTEST RED cargo-lane-budget failures=256"),
            "the RED line must still name the real count, got {line:?}"
        );
        assert_ne!(
            code, 0,
            "stdout says RED failures=256 while the exit code says SUCCESS — the n34x defect"
        );

        let (pass_line, pass_code) = selftest_summary(0);
        assert!(
            pass_line.contains("SELFTEST PASS cargo-lane-budget"),
            "got {pass_line:?}"
        );
        assert_eq!(pass_code, 0);

        // Anti-vacuity: a PASS line must never be paired with a nonzero code either.
        for failures in [0, 1, 2, 255, 256] {
            let (line, code) = selftest_summary(failures);
            assert_eq!(
                line.contains("SELFTEST RED"),
                code != 0,
                "line/status disagreement at failures={failures}: {line:?} with code {code}"
            );
        }
    }

    #[test]
    fn count_warn_band_has_both_edges() {
        let below = assess_count(1, 4, 2);
        assert_eq!(below.status, Status::Pass);
        assert!(!below.warn);
        assert_eq!(below.headroom, 3);

        let warn = assess_count(3, 4, 2);
        assert_eq!(warn.status, Status::Pass);
        assert!(warn.warn);
        assert_eq!(warn.warn_at, 2);
        assert_eq!(warn.headroom, 1);

        let equal = assess_count(4, 4, 2);
        assert_eq!(equal.status, Status::Red);
        assert!(!equal.warn);
        assert_eq!(equal.headroom, 0);
    }
    #[test]
    fn check_emits_warn_and_ledger_provenance() {
        let root =
            std::env::temp_dir().join(format!("cargo-lane-budget-warn-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for name in ["one", "two", "three"] {
            let lane = root.join(name);
            fs::create_dir_all(&lane).unwrap();
            fs::write(
                lane.join("CACHEDIR.TAG"),
                "Signature: 8a477f597d28d172789f06886806bc55\n",
            )
            .unwrap();
        }
        let ledger = root.join("ledger.jsonl");
        let config = Config {
            lane_budget: Some(4),
            lanes_per_session: 22,
            headroom: 2,
            floor: 1,
            tmux_bin: "tmux".into(),
            scan_roots: Some(root.display().to_string()),
            scan_depth: 2,
            scan_deadline_secs: 10,
            scan_retries: 1,
            cache_max_age_secs: 60,
            cache_file: root.join("cache"),
            ledger: ledger.clone(),
            byte_root: PathBuf::from("/"),
            min_free_gib: 0.0,
            aggregate_bound: false,
            aggregate_container: "disk3".into(),
            aggregate_floor_bytes: 1,
            aggregate_fixture: None,
            diskutil_bin: "diskutil".into(),
        };
        let report = check(&config);
        assert_eq!(report.status, Status::Pass);
        assert!(report.lines.iter().any(|line| line
            .contains("WARN lane_roots_near_budget lane_roots=3 budget=4 warn_at=2 headroom=1")));
        let row: serde_json::Value =
            serde_json::from_str(fs::read_to_string(ledger).unwrap().lines().last().unwrap())
                .unwrap();
        assert_eq!(row["observed"], 3);
        assert_eq!(row["budget"], 4);
        assert_eq!(row["warn_at"], 2);
        assert_eq!(row["warn"], true);
        assert_eq!(row["count_headroom"], 1);
        let _ = fs::remove_dir_all(root);
    }
}

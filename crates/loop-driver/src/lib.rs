#![forbid(unsafe_code)]

//! Deadline-bounded driver for `bin/loop-tick.sh`.
//!
//! `bin/loop-driver.sh` remains the differential oracle. The live Rust path owns
//! its advisory lock directly: the locked `File` is private to an RAII type,
//! Rust opens it close-on-exec, and `Drop` releases it.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const EXIT_CONCURRENT: i32 = 75;
pub const EXIT_DEADLINE: i32 = 124;
pub const SCHEDULE_INTERVAL_SECONDS: u64 = 1_200;
/// Two missed 20-minute slots is the backstop: a healthy tick is already
/// child-bounded to `DEFAULT_DEADLINE_SECONDS`. This bound kills the *holder*.
pub const WALL_BOUND_INTERVALS: u64 = 2;
pub const DEFAULT_WALL_BOUND_SECONDS: u64 = SCHEDULE_INTERVAL_SECONDS * WALL_BOUND_INTERVALS;
pub const DEFAULT_LIVENESS_GAP_MS: u64 = 2_000;
// The shell oracle and the cutover cron row both use this path. Keeping the
// interop path stable means a shell invocation during deployment cannot overlap
// a Rust invocation by accidentally taking a different lock.
pub const DEFAULT_LOCK_PATH: &str = "/tmp/control-plane-loop-driver.lock";
// The 13,33,53 schedule is 1,200 seconds. Reserve the final minute for cron to
// reap this process before the next slot, so a bounded run cannot overlap it.
pub const DEFAULT_DEADLINE_SECONDS: u64 = SCHEDULE_INTERVAL_SECONDS - 60;
/// Cron PATH with the personal bin segment derived from `$HOME` when set and omitted
/// when not: omitted is a true statement, never a guess (omp-orchestrator-npq).
fn cron_path() -> String {
    match std::env::var_os("HOME").filter(|v| !v.is_empty()) {
        Some(home) => format!(
            "/opt/homebrew/bin:{}/.local/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin",
            std::path::PathBuf::from(&home).display()
        ),
        None => "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin".to_owned(),
    }
}
const OS_PROBE_BUDGET: Duration = Duration::from_secs(1);

#[derive(Debug, Eq, PartialEq)]
pub struct LoopDriverRunOutput {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

impl LoopDriverRunOutput {
    fn success() -> Self {
        Self {
            stdout: String::new(),
            stderr: String::new(),
            code: 0,
        }
    }

    fn verdict(line: String, code: i32) -> Self {
        Self {
            stdout: format!("{line}\n"),
            stderr: String::new(),
            code,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LoopDriverConfig {
    pub repo: PathBuf,
    pub log: PathBuf,
    pub lock_path: PathBuf,
    pub state_dir: PathBuf,
    pub ledger_threshold_check: PathBuf,
    pub p6_rearm_check: PathBuf,
    pub loop_tick_bin: PathBuf,
    pub session: String,
    pub tmux_tmpdir: PathBuf,
    pub deadline: Duration,
}

impl LoopDriverConfig {
    pub fn from_env() -> Result<Self, String> {
        let home = std::env::var_os("HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| {
                "HOME is unset; cannot resolve the default state dir or log; set LOOP_DRIVER_STATE_DIR and LOOP_DRIVER_LOG".to_owned()
            })?;
        // Repository root: `LOOP_REPO` env > upward `.git`/`.beads` marker walk from the
        // cwd — the omp-orchestrator-npq mechanism. Never a constant, because a wrong
        // root compiles fine and then silently runs the wrong repo's scripts.
        let repo = match std::env::var_os("LOOP_REPO").filter(|v| !v.is_empty()) {
            Some(repo) => PathBuf::from(repo),
            None => {
                let mut current = std::env::current_dir()
                    .map_err(|error| format!("cannot read the current directory: {error}"))?;
                loop {
                    if [".git", ".beads"].iter().any(|marker| current.join(marker).exists()) {
                        break;
                    }
                    let Some(parent) = current.parent() else {
                        return Err(format!(
                            "no repository marker (.git or .beads) found at or above {}; set LOOP_REPO or run from a checkout",
                            current.display()
                        ));
                    };
                    current = parent.to_path_buf();
                }
                current
            }
        };
        let state_dir = std::env::var_os("LOOP_DRIVER_STATE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state/flywheel"));
        let log = std::env::var_os("LOOP_DRIVER_LOG")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state/flywheel/control-plane-loop.log"));
        let lock_path = std::env::var_os("LOOP_DRIVER_LOCK")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_LOCK_PATH));
        let deadline_seconds = match std::env::var("LOOP_DRIVER_DEADLINE_SECONDS") {
            Ok(value) => value
                .parse::<u64>()
                .map_err(|_| "LOOP_DRIVER_DEADLINE_SECONDS must be an integer".to_owned())?,
            Err(_) => DEFAULT_DEADLINE_SECONDS,
        };
        if deadline_seconds == 0 || deadline_seconds > SCHEDULE_INTERVAL_SECONDS {
            return Err(format!(
                "LOOP_DRIVER_DEADLINE_SECONDS must be between 1 and {SCHEDULE_INTERVAL_SECONDS}"
            ));
        }
        // Session default is the resolved repository's basename: this checkout keeps
        // today's behavior, and a moved checkout targets its own session instead of a
        // name that no longer matches the repository.
        let session_default = repo
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Self {
            ledger_threshold_check: std::env::var_os("LOOP_LEDGER_THRESHOLD_CHECK")
                .map(PathBuf::from)
                .unwrap_or_else(|| repo.join("bin/tick-ledger-threshold-check.sh")),
            p6_rearm_check: std::env::var_os("LOOP_P6_REARM_CHECK")
                .map(PathBuf::from)
                .unwrap_or_else(|| repo.join("bin/p6-rearm-check.sh")),
            loop_tick_bin: std::env::var_os("LOOP_TICK_BIN")
                .map(PathBuf::from)
                .unwrap_or_else(|| repo.join("bin/loop-tick.sh")),
            session: std::env::var("LOOP_SESSION").unwrap_or(session_default),
            tmux_tmpdir: std::env::var_os("TMUX_TMPDIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".tmux-sockets")),
            repo,
            log,
            lock_path,
            state_dir,
            deadline: Duration::from_secs(deadline_seconds),
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LoopDriverRules {
    record_tick_output: bool,
    pub wall_bound: bool,
}

impl Default for LoopDriverRules {
    fn default() -> Self {
        Self {
            record_tick_output: true,
            wall_bound: true,
        }
    }
}

impl LoopDriverRules {
    pub fn disable(&mut self, name: &str) -> bool {
        match name {
            "differential_tick_log" => {
                self.record_tick_output = false;
                true
            }
            "wall_bound" => {
                self.wall_bound = false;
                true
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LockRules {
    pub enforce_single_instance: bool,
    pub recover_dead_holder: bool,
    /// When false, a WEDGED holder is reported but never killed. Mutation target.
    pub wall_bound: bool,
    pub wall_bound_secs: u64,
    pub liveness_gap: Duration,
}

impl Default for LockRules {
    fn default() -> Self {
        Self::from_env()
    }
}

impl LockRules {
    pub fn from_env() -> Self {
        let disabled = std::env::var_os("LOOP_DRIVER_DISABLE_WALL_BOUND").is_some();
        let wall_bound_secs = std::env::var("LOOP_DRIVER_WALL_BOUND_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_WALL_BOUND_SECONDS)
            .max(1);
        let liveness_gap_ms = std::env::var("LOOP_DRIVER_LIVENESS_GAP_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_LIVENESS_GAP_MS)
            .max(50);
        Self {
            enforce_single_instance: true,
            recover_dead_holder: true,
            wall_bound: !disabled,
            wall_bound_secs,
            liveness_gap: Duration::from_millis(liveness_gap_ms),
        }
    }

    pub fn disable(&mut self, name: &str) -> bool {
        match name {
            "wall_bound" => {
                self.wall_bound = false;
                true
            }
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LockMetadata {
    pub pid: u32,
    pub started_unix_ms: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HolderLiveness {
    Live,
    Wedged,
}

impl HolderLiveness {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Live => "LIVE",
            Self::Wedged => "WEDGED",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HolderView {
    pub pid: u32,
    pub elapsed: String,
    pub elapsed_secs: u64,
    pub liveness: HolderLiveness,
}

#[derive(Debug)]
pub enum LockError {
    LiveInstance {
        holder_pid: Option<u32>,
        holder_elapsed: Option<String>,
        liveness: Option<HolderLiveness>,
    },
    DeadHolderRecoveryDisabled {
        holder_pid: u32,
    },
    Io(std::io::Error),
}

impl std::fmt::Display for LockError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LiveInstance {
                holder_pid,
                holder_elapsed,
                liveness,
            } => write!(
                formatter,
                "live instance holds lock (pid={} elapsed={} liveness={})",
                holder_pid.map_or_else(|| "unresolved".to_owned(), |pid| pid.to_string()),
                holder_elapsed.as_deref().unwrap_or("unavailable"),
                liveness.map_or("unresolved", HolderLiveness::as_str)
            ),
            Self::DeadHolderRecoveryDisabled { holder_pid } => {
                write!(
                    formatter,
                    "dead holder metadata requires recovery (pid={holder_pid})"
                )
            }
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

pub struct InstanceGuard {
    file: File,
    recovered_dead_holder: Option<u32>,
    recovered_wedged: Option<HolderView>,
    wall_bound_secs: u64,
    lock_path: PathBuf,
}

impl InstanceGuard {
    pub fn acquire(path: &Path, rules: LockRules) -> Result<Self, LockError> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(LockError::Io)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)
            .map_err(LockError::Io)?;

        let mut recovered_wedged = None;
        if rules.enforce_single_instance {
            match File::try_lock(&file) {
                Ok(()) => {}
                Err(std::fs::TryLockError::WouldBlock) => {
                    recovered_wedged = refuse_or_reap_wedge(path, &file, &rules)?;
                }
                Err(std::fs::TryLockError::Error(error)) => return Err(LockError::Io(error)),
            }
        }

        let old = read_metadata(&mut file);
        let current_pid = std::process::id();
        let recovered_dead_holder = match old {
            Some(metadata) if metadata.pid != current_pid && !process_is_alive(metadata.pid) => {
                if rules.recover_dead_holder {
                    Some(metadata.pid)
                } else {
                    let _ = File::unlock(&file);
                    return Err(LockError::DeadHolderRecoveryDisabled {
                        holder_pid: metadata.pid,
                    });
                }
            }
            _ => None,
        };

        let metadata = LockMetadata {
            pid: current_pid,
            started_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_millis()),
        };
        file.set_len(0).map_err(LockError::Io)?;
        file.seek(SeekFrom::Start(0)).map_err(LockError::Io)?;
        serde_json::to_writer(&mut file, &metadata)
            .map_err(|error| LockError::Io(std::io::Error::other(error)))?;
        file.write_all(b"\n").map_err(LockError::Io)?;
        file.flush().map_err(LockError::Io)?;

        Ok(Self {
            file,
            recovered_dead_holder,
            recovered_wedged,
            wall_bound_secs: rules.wall_bound_secs,
            lock_path: path.to_path_buf(),
        })
    }

    #[must_use]
    pub fn recovered_dead_holder(&self) -> Option<u32> {
        self.recovered_dead_holder
    }

    #[must_use]
    pub fn recovered_wedged(&self) -> Option<&HolderView> {
        self.recovered_wedged.as_ref()
    }

    #[must_use]
    pub fn wedged_kill_line(&self) -> Option<String> {
        let view = self.recovered_wedged.as_ref()?;
        Some(format!(
            "LOOP_DRIVER_HOLDER_WEDGED code={EXIT_DEADLINE} holder_pid={} holder_elapsed={} holder_liveness=WEDGED bound_seconds={} killed=1 lock={}",
            view.pid,
            view.elapsed,
            self.wall_bound_secs,
            self.lock_path.display()
        ))
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        let _ = File::unlock(&self.file);
    }
}

fn read_metadata(file: &mut File) -> Option<LockMetadata> {
    let mut text = String::new();
    file.seek(SeekFrom::Start(0)).ok()?;
    file.read_to_string(&mut text).ok()?;
    serde_json::from_str(text.trim()).ok()
}

#[derive(Clone, Debug)]
struct ProcessRow {
    pid: u32,
    ppid: u32,
    etimes: u64,
    cpu: f64,
    cputime_ms: u64,
}

fn lock_holder_pids(path: &Path) -> Vec<u32> {
    let me = std::process::id();
    let parse = |stdout: &[u8]| -> Vec<u32> {
        String::from_utf8_lossy(stdout)
            .lines()
            .filter_map(|line| line.trim().parse::<u32>().ok())
            .filter(|pid| *pid != me)
            .collect()
    };
    // Direct output() — the bounded run_command path was dropping lsof pids
    // on this box (WouldBlock with empty lsof). lsof of one file is instant.
    // -nP skips DNS/port names; FAR uses the same pair on this box.
    for bin in ["/usr/sbin/lsof", "/usr/bin/lsof"] {
        for args in [vec!["-nP", "-t"], vec!["-t"]] {
            let mut cmd = Command::new(bin);
            cmd.args(&args).arg(path);
            if let Ok(output) = cmd.output() {
                let pids = parse(&output.stdout);
                if !pids.is_empty() {
                    return pids;
                }
            }
        }
    }
    Vec::new()
}

fn pgrep_children(pid: u32) -> Vec<u32> {
    let output = Command::new("/usr/bin/pgrep")
        .args(["-P", &pid.to_string()])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse().ok())
        .collect()
}

fn parse_ps_line(line: &str) -> Option<ProcessRow> {
    let mut fields = line.split_whitespace();
    Some(ProcessRow {
        pid: fields.next()?.parse().ok()?,
        ppid: fields.next()?.parse().ok()?,
        etimes: parse_etime(fields.next()?),
        cpu: fields.next()?.parse().ok()?,
        cputime_ms: parse_cputime(fields.next().unwrap_or("0:00.00")),
    })
}

fn ps_row_for(pid: u32) -> Option<ProcessRow> {
    // Darwin `ps` has `etime` ([[dd-]hh:]mm:ss), not Linux `etimes` (integer
    // seconds). Asking for etimes= here used to fail the whole snapshot, so
    // every holder looked like 0% CPU and was labelled WEDGED.
    let output = Command::new("/bin/ps")
        .args([
            "-p",
            &pid.to_string(),
            "-o",
            "pid=,ppid=,etime=,%cpu=,time=",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_ps_line(&String::from_utf8_lossy(&output.stdout))
}

fn process_table() -> Vec<ProcessRow> {
    let output = Command::new("/bin/ps")
        .args(["-axo", "pid=,ppid=,etime=,%cpu=,time="])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_ps_line)
        .collect()
}

/// Darwin `etime` is [[dd-]hh:]mm:ss — never a raw second count.
fn parse_etime(raw: &str) -> u64 {
    let t = raw.trim();
    if t.is_empty() {
        return 0;
    }
    let (days, rest) = match t.split_once('-') {
        Some((d, r)) => (d.parse::<u64>().unwrap_or(0), r),
        None => (0, t),
    };
    let parts: Vec<&str> = rest.split(':').collect();
    let hms = match parts.as_slice() {
        [ss] => ss.parse::<u64>().unwrap_or(0),
        [mm, ss] => mm.parse::<u64>().unwrap_or(0) * 60 + ss.parse::<u64>().unwrap_or(0),
        [hh, mm, ss] => {
            hh.parse::<u64>().unwrap_or(0) * 3_600
                + mm.parse::<u64>().unwrap_or(0) * 60
                + ss.parse::<u64>().unwrap_or(0)
        }
        _ => 0,
    };
    days.saturating_mul(86_400).saturating_add(hms)
}

/// Among lock holders, walk descendants and name the deepest leaf (the
/// process actually blocked — a stuck git, not the sleeping flock parent).
fn deepest_from_rows(candidates: &[u32], rows: &[ProcessRow]) -> Option<u32> {
    let mut best = None;
    let mut best_depth = 0usize;
    let mut frontier: Vec<u32> = candidates.to_vec();
    frontier.sort_unstable();
    let mut seen = std::collections::BTreeSet::new();
    let mut depth = 0usize;
    while !frontier.is_empty() && depth < 64 {
        let mut next = Vec::new();
        for pid in frontier {
            if !seen.insert(pid) {
                continue;
            }
            if depth >= best_depth {
                best = Some(pid);
                best_depth = depth;
            }
            let mut kids: Vec<u32> = rows
                .iter()
                .filter(|row| row.ppid == pid)
                .map(|row| row.pid)
                .collect();
            if kids.is_empty() {
                kids = pgrep_children(pid);
            }
            kids.sort_unstable();
            next.extend(kids);
        }
        frontier = next;
        depth += 1;
    }
    best.or_else(|| candidates.first().copied())
}

fn deepest_holder_pid(candidates: &[u32]) -> Option<u32> {
    deepest_from_rows(candidates, &process_table())
}

fn parse_cputime(raw: &str) -> u64 {
    // macOS `time=` is [[hh:]mm:]ss.ss
    let t = raw.trim();
    let parts = t.split(':');
    let collected: Vec<&str> = parts.collect();
    let (h, m, s) = match collected.as_slice() {
        [ss] => (0, 0, *ss),
        [mm, ss] => (0, mm.parse().unwrap_or(0), *ss),
        [hh, mm, ss] => (hh.parse().unwrap_or(0), mm.parse().unwrap_or(0), *ss),
        _ => (0, 0, "0"),
    };
    let seconds: f64 = s.parse().unwrap_or(0.0);
    ((h as f64 * 3600.0 + m as f64 * 60.0 + seconds) * 1000.0) as u64
}

/// Max CPU / cputime in the descendant tree. A sleeping lock holder with a
/// burning child is LIVE; a sleeping holder whose git child is stuck in write
/// is 0% across the tree and WEDGED.
fn tree_activity_from_rows(root: u32, rows: &[ProcessRow]) -> (f64, u64) {
    let mut cpu: f64 = 0.0;
    let mut cputime = 0u64;
    let mut stack = vec![root];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(p) = stack.pop() {
        if !seen.insert(p) {
            continue;
        }
        if let Some(row) = rows.iter().find(|row| row.pid == p) {
            cpu = cpu.max(row.cpu);
            cputime = cputime.max(row.cputime_ms);
            stack.extend(rows.iter().filter(|row| row.ppid == p).map(|row| row.pid));
        } else if let Some(row) = ps_row_for(p) {
            cpu = cpu.max(row.cpu);
            cputime = cputime.max(row.cputime_ms);
            stack.extend(pgrep_children(p));
        }
    }
    (cpu, cputime)
}

fn combined_tree_activity(roots: &[u32], rows: &[ProcessRow]) -> (f64, u64) {
    let mut cpu: f64 = 0.0;
    let mut cputime = 0u64;
    for root in roots {
        let (c, t) = tree_activity_from_rows(*root, rows);
        cpu = cpu.max(c);
        cputime = cputime.max(t);
    }
    (cpu, cputime)
}

/// Two captures, gap apart. LIVE = still alive AND (tree CPU sampled > 0.05
/// OR accumulated cputime grew). WEDGED = still alive AND 0 CPU across the
/// lock-holder tree. A pipe-deadlocked git leaf matches WEDGED; a busy-loop
/// fixture matches LIVE.
pub fn classify_liveness(
    first: (u64, f64, usize, u64),
    second: (u64, f64, usize, u64),
    still_alive: bool,
) -> Option<HolderLiveness> {
    if !still_alive {
        return None;
    }
    let advancing = second.0 >= first.0;
    if !advancing {
        return None;
    }
    let cpu = first.1.max(second.1);
    let cpu_grew = second.3 > first.3;
    if cpu > 0.05 || cpu_grew {
        Some(HolderLiveness::Live)
    } else {
        Some(HolderLiveness::Wedged)
    }
}

fn peek_metadata(file: &File) -> Option<LockMetadata> {
    let mut clone = file.try_clone().ok()?;
    read_metadata(&mut clone)
}

fn holder_view(path: &Path, file: &File, gap: Duration) -> Option<HolderView> {
    let mut candidates = lock_holder_pids(path);
    if candidates.is_empty() {
        std::thread::sleep(Duration::from_millis(50));
        candidates = lock_holder_pids(path);
    }
    if candidates.is_empty() {
        if let Some(meta) = peek_metadata(file) {
            if process_is_alive(meta.pid) {
                candidates.push(meta.pid);
            }
        }
    }
    if candidates.is_empty() {
        return None;
    }
    let first_rows = process_table();
    let pid =
        deepest_from_rows(&candidates, &first_rows).or_else(|| deepest_holder_pid(&candidates))?;
    let first_elapsed = first_rows
        .iter()
        .find(|row| row.pid == pid)
        .map(|row| row.etimes)
        .or_else(|| ps_row_for(pid).map(|row| row.etimes))
        .unwrap_or(0);
    let (cpu1, ct1) = combined_tree_activity(&candidates, &first_rows);
    std::thread::sleep(gap);
    let still_alive = process_is_alive(pid) || candidates.iter().copied().any(process_is_alive);
    if !still_alive {
        return None;
    }
    let second_rows = process_table();
    let second_elapsed = second_rows
        .iter()
        .find(|row| row.pid == pid)
        .map(|row| row.etimes)
        .or_else(|| ps_row_for(pid).map(|row| row.etimes))
        .unwrap_or(first_elapsed);
    let (cpu2, ct2) = combined_tree_activity(&candidates, &second_rows);
    let liveness = classify_liveness(
        (first_elapsed, cpu1, 0, ct1),
        (second_elapsed, cpu2, 0, ct2),
        still_alive,
    )
    .unwrap_or(HolderLiveness::Wedged);
    // Holder age is etime, not the probe gap. Inflating with the gap made a
    // 0s parse failure look like bound_seconds=1 and authorized a kill.
    let elapsed_secs = second_elapsed.max(first_elapsed);
    Some(HolderView {
        pid,
        elapsed: format!("{elapsed_secs}s"),
        elapsed_secs,
        liveness,
    })
}

fn terminate_process_tree(pid: u32) {
    if pid <= 1 {
        return;
    }
    let killing_self = pid == std::process::id();
    if killing_self {
        // Children first, then the caller exits.
    } else {
        // continue
    }
    let children = pgrep_children(pid);
    for child in children {
        if child != std::process::id() {
            terminate_process_tree(child);
        }
    }
    if killing_self {
        return;
    }
    let _ = Command::new("/bin/kill")
        .args(["-TERM", &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(Duration::from_millis(50));
    let _ = Command::new("/bin/kill")
        .args(["-KILL", &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

pub fn wall_bound_line(bound: Duration, lock: &Path) -> String {
    format!(
        "LOOP_DRIVER_WALL_BOUND code={EXIT_DEADLINE} bound_seconds={} elapsed_seconds={} lock={}",
        bound.as_secs(),
        bound.as_secs(),
        lock.display()
    )
}

/// Self-imposed wall bound: after N intervals this process kills its children
/// and exits 124 with a typed row. Does not widen the lock.
pub fn arm_wall_watchdog(bound: Duration, lock: PathBuf, log: PathBuf) {
    let pid = std::process::id();
    std::thread::spawn(move || {
        std::thread::sleep(bound);
        let line = wall_bound_line(bound, &lock);
        let _ = append(&log, &format!("[{}] driver: {line}\n", timestamp()));
        println!("{line}");
        let _ = std::io::stdout().flush();
        terminate_process_tree(pid);
        std::process::exit(EXIT_DEADLINE);
    });
}

/// A WEDGED holder older than the wall bound is reaped, then we retry the
/// kernel lock. Two concurrent LIVE runs remain impossible.
fn refuse_or_reap_wedge(
    path: &Path,
    file: &File,
    rules: &LockRules,
) -> Result<Option<HolderView>, LockError> {
    let view = holder_view(path, file, rules.liveness_gap);
    if let Some(view) = view {
        let old_enough = view.elapsed_secs >= rules.wall_bound_secs;
        if view.liveness == HolderLiveness::Wedged && rules.wall_bound && old_enough {
            let roots = lock_holder_pids(path);
            for pid in &roots {
                terminate_process_tree(*pid);
            }
            terminate_process_tree(view.pid);
            std::thread::sleep(Duration::from_millis(150));
            match File::try_lock(file) {
                Ok(()) => return Ok(Some(view)),
                Err(std::fs::TryLockError::WouldBlock) => {}
                Err(std::fs::TryLockError::Error(error)) => return Err(LockError::Io(error)),
            }
        }
        return Err(LockError::LiveInstance {
            holder_pid: Some(view.pid),
            holder_elapsed: Some(view.elapsed),
            liveness: Some(view.liveness),
        });
    }
    let (pid, elapsed) = os_holder(path);
    Err(LockError::LiveInstance {
        holder_pid: pid,
        holder_elapsed: elapsed,
        liveness: None,
    })
}

/// Ask the OS which process has the shared lock file open. Used when two-capture
/// cannot complete; still prefers lsof pids over the word "unknown".
fn os_holder(path: &Path) -> (Option<u32>, Option<String>) {
    let candidates = lock_holder_pids(path);
    let Some(pid) = deepest_holder_pid(&candidates) else {
        return (None, None);
    };
    let elapsed = ps_row_for(pid).map(|row| format!("{}s", row.etimes));
    (Some(pid), elapsed)
}

fn process_is_alive(pid: u32) -> bool {
    let mut command = Command::new("/bin/kill");
    command
        .args(["-0", &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    run_command(command, Deadline::new(OS_PROBE_BUDGET))
        .is_ok_and(|result| !result.timed_out && result.code() == 0)
}

pub fn lock_refusal(path: &Path, error: &LockError) -> LoopDriverRunOutput {
    match error {
        LockError::LiveInstance {
            holder_pid,
            holder_elapsed,
            liveness,
        } => {
            let pid = holder_pid.map_or_else(|| "unresolved".to_owned(), |pid| pid.to_string());
            let elapsed = holder_elapsed.as_deref().unwrap_or("unavailable");
            let live = liveness.map_or("unresolved", HolderLiveness::as_str);
            let killed_note = if liveness == &Some(HolderLiveness::Wedged) {
                " killed=0"
            } else {
                ""
            };
            LoopDriverRunOutput::verdict(
                format!(
                    "LOOP_DRIVER_REFUSED code={EXIT_CONCURRENT} reason=live_instance holder_pid={pid} holder_elapsed={elapsed} holder_liveness={live}{killed_note} lock={}",
                    path.display()
                ),
                EXIT_CONCURRENT,
            )
        }
        other => LoopDriverRunOutput::verdict(
            format!(
                "LOOP_DRIVER_REFUSED code=78 reason=lock_unusable lock={} detail={other}",
                path.display()
            ),
            78,
        ),
    }
}

#[derive(Clone, Copy)]
struct Deadline {
    started: Instant,
    budget: Duration,
}

impl Deadline {
    fn new(budget: Duration) -> Self {
        Self {
            started: Instant::now(),
            budget,
        }
    }

    fn expired(self) -> bool {
        self.started.elapsed() >= self.budget
    }

    fn elapsed(self) -> Duration {
        self.started.elapsed()
    }
}

struct ChildResult {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    timed_out: bool,
}

impl ChildResult {
    fn code(&self) -> i32 {
        if self.timed_out {
            EXIT_DEADLINE
        } else {
            self.status.code().unwrap_or(1)
        }
    }

    fn combined(&self) -> String {
        let mut bytes = self.stdout.clone();
        bytes.extend_from_slice(&self.stderr);
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

fn run_command(mut command: Command, deadline: Deadline) -> Result<ChildResult, std::io::Error> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let stdout_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        bytes
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });

    let (status, timed_out) = loop {
        if let Some(status) = child.try_wait()? {
            break (status, false);
        }
        if deadline.expired() {
            let _ = child.kill();
            break (child.wait()?, true);
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    Ok(ChildResult {
        status,
        stdout: stdout_reader.join().unwrap_or_default(),
        stderr: stderr_reader.join().unwrap_or_default(),
        timed_out,
    })
}

fn deadline_output(deadline: Deadline, phase: &str) -> LoopDriverRunOutput {
    LoopDriverRunOutput::verdict(
        format!(
            "LOOP_DRIVER_DEADLINE_EXCEEDED code={EXIT_DEADLINE} phase={phase} elapsed_seconds={} deadline_seconds={}",
            deadline.elapsed().as_secs(),
            deadline.budget.as_secs()
        ),
        EXIT_DEADLINE,
    )
}

pub fn deadline_probe(deadline: Duration, child_runtime: Duration) -> LoopDriverRunOutput {
    let clock = Deadline::new(deadline);
    let mut command = Command::new("/bin/sleep");
    command.arg(child_runtime.as_secs().to_string());
    match run_command(command, clock) {
        Ok(result) if result.timed_out => deadline_output(clock, "deadline_probe"),
        Ok(result) => LoopDriverRunOutput::verdict(
            format!(
                "LOOP_DRIVER_DEADLINE_PROBE_COMPLETED code={}",
                result.code()
            ),
            result.code(),
        ),
        Err(error) => LoopDriverRunOutput {
            stdout: String::new(),
            stderr: format!("usage error: cannot spawn deadline probe: {error}\n"),
            code: 2,
        },
    }
}

fn timestamp() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn append(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(text.as_bytes())
}

fn append_driver_log(config: &LoopDriverConfig, message: &str) {
    let _ = append(
        &config.log,
        &format!("[{}] driver: {message}\n", timestamp()),
    );
}

fn command_env(command: &mut Command, config: &LoopDriverConfig) {
    command
        .env("PATH", cron_path())
        .env("TMUX_TMPDIR", &config.tmux_tmpdir)
        .env("RUST_LOG", "error");
}

fn command_exists(name: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    cron_path().split(':').any(|directory| {
        let candidate = Path::new(directory).join(name);
        fs::metadata(candidate)
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    })
}

#[derive(Clone, Debug)]
struct Invocation {
    invoker: &'static str,
    proof: &'static str,
    parent_pid: u32,
}

fn parse_ps_row(text: &str) -> Option<(u32, u32, String)> {
    let mut fields = text.split_whitespace();
    let uid = fields.next()?.parse().ok()?;
    let ppid = fields.next()?.parse().ok()?;
    let command = fields.collect::<Vec<_>>().join(" ");
    if command.is_empty() {
        return None;
    }
    Some((uid, ppid, command))
}

fn ps_row(
    pid: u32,
    config: &LoopDriverConfig,
    deadline: Deadline,
) -> Result<Option<(u32, u32, String)>, LoopDriverRunOutput> {
    let mut command = Command::new("/bin/ps");
    command.args(["-p", &pid.to_string(), "-o", "uid=,ppid=,comm="]);
    command_env(&mut command, config);
    match run_command(command, deadline) {
        Ok(result) if result.timed_out => Err(deadline_output(deadline, "invoker_lineage")),
        Ok(result) if result.code() == 0 => {
            Ok(parse_ps_row(&String::from_utf8_lossy(&result.stdout)))
        }
        Ok(_) | Err(_) => Ok(None),
    }
}

fn detect_invocation(config: &LoopDriverConfig, deadline: Deadline) -> Result<Invocation, LoopDriverRunOutput> {
    let current = ps_row(std::process::id(), config, deadline)?;
    let parent_pid = current.as_ref().map_or(0, |(_, ppid, _)| *ppid);
    let mut pid = parent_pid;
    for _ in 0..12 {
        if pid <= 1 {
            break;
        }
        let Some((uid, ppid, command)) = ps_row(pid, config, deadline)? else {
            break;
        };
        if uid == 0 && ppid == 1 && command == "/usr/sbin/cron" {
            return Ok(Invocation {
                invoker: "SCHEDULED",
                proof: "cron_parent",
                parent_pid,
            });
        }
        pid = ppid;
    }
    Ok(Invocation {
        invoker: "MANUAL",
        proof: "unproven_parent",
        parent_pid,
    })
}

fn write_lane_row(config: &LoopDriverConfig, invocation: &Invocation) {
    let row = serde_json::json!({
        "ts": timestamp(),
        "event": "lane_run",
        "invoker": invocation.invoker,
        "invoker_proof": invocation.proof,
    });
    let _ = append(
        &config.state_dir.join("loop-driver.jsonl"),
        &format!("{row}\n"),
    );
}

fn run_helper(
    executable: &Path,
    args: &[&str],
    config: &LoopDriverConfig,
    deadline: Deadline,
) -> Result<ChildResult, LoopDriverRunOutput> {
    let mut command = Command::new(executable);
    command.args(args).current_dir(&config.repo);
    command_env(&mut command, config);
    match run_command(command, deadline) {
        Ok(result) if result.timed_out => Err(deadline_output(deadline, "preflight")),
        Ok(result) => Ok(result),
        Err(error) => Err(LoopDriverRunOutput::verdict(
            format!(
                "LOOP_DRIVER_REFUSED code=1 reason=spawn_failed command={} detail={error}",
                executable.display()
            ),
            1,
        )),
    }
}

fn session_visible(config: &LoopDriverConfig, deadline: Deadline) -> Result<bool, LoopDriverRunOutput> {
    for attempt in 1..=3 {
        let mut command = Command::new("ntm");
        command.arg("list");
        command_env(&mut command, config);
        let result = match run_command(command, deadline) {
            Ok(result) if result.timed_out => return Err(deadline_output(deadline, "ntm_list")),
            Ok(result) => result,
            Err(_) => continue,
        };
        let prefix = format!("{}:", config.session);
        if String::from_utf8_lossy(&result.stdout)
            .lines()
            .any(|line| line.trim_start().starts_with(&prefix))
        {
            return Ok(true);
        }
        if attempt < 3 {
            if deadline.expired() {
                return Err(deadline_output(deadline, "ntm_retry"));
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    }

    let mut command = Command::new("tmux");
    command.args(["has-session", "-t", &config.session]);
    command_env(&mut command, config);
    match run_command(command, deadline) {
        Ok(result) if result.timed_out => Err(deadline_output(deadline, "tmux_has_session")),
        Ok(result) if result.code() == 0 => {
            append_driver_log(
                config,
                &format!(
                    "ntm did not see '{}' in 3 tries but tmux HAS it — ntm projection fault (upstream ntm#254), proceeding",
                    config.session
                ),
            );
            Ok(true)
        }
        Ok(_) | Err(_) => Ok(false),
    }
}

pub fn classify_tick_failure(rc: i32, output: &str, deadline_seconds: u64) -> String {
    if rc == EXIT_DEADLINE {
        return format!("TICK TIMED OUT after {deadline_seconds}s (cron-slot bound)");
    }
    if let Some(detail) = output.lines().rev().find(|line| {
        line.contains("CARGO_LANE_BUDGET")
            && (line.contains("scan_unavailable") || line.contains("scan_incomplete"))
    }) {
        return format!("TICK REFUSED — cargo-lane-budget measurement unavailable: {detail}");
    }
    if let Some(detail) = output.lines().rev().find(|line| {
        line.contains("runtime admission REFUSED") || line.contains("dispatch_blocked")
    }) {
        return format!("TICK REFUSED — admission: {detail}");
    }
    if output.contains("DISPATCH FAILED") {
        return "TICK FAILED — dispatch delivery failed; see tick output above".to_owned();
    }
    format!("TICK FAILED rc={rc} — refusal reason not classified; see tick output above")
}

pub fn selftest_failure_reason() -> LoopDriverRunOutput {
    let budget = classify_tick_failure(
        1,
        "CARGO_LANE_BUDGET UNKNOWN scan_unavailable root=/private/tmp rc=124\n",
        SCHEDULE_INTERVAL_SECONDS,
    );
    let delivery = classify_tick_failure(
        1,
        "DISPATCH FAILED — the loop is talking to nobody\n",
        SCHEDULE_INTERVAL_SECONDS,
    );
    if budget.contains("cargo-lane-budget measurement unavailable")
        && !budget.contains("talking to nobody")
        && delivery.contains("dispatch delivery failed")
        && !delivery.contains("talking to nobody")
    {
        LoopDriverRunOutput::verdict(
            "PASS selftest.failure-reason-cargo-budget\nPASS selftest.failure-reason-delivery"
                .to_owned(),
            0,
        )
    } else {
        LoopDriverRunOutput::verdict("FAIL selftest.failure-reason-classification".to_owned(), 1)
    }
}

pub fn invoker_from_rows(rows: &[(u32, u32, &str)]) -> (&'static str, &'static str) {
    if rows
        .iter()
        .any(|(uid, ppid, command)| *uid == 0 && *ppid == 1 && *command == "/usr/sbin/cron")
    {
        ("SCHEDULED", "cron_parent")
    } else {
        ("MANUAL", "unproven_parent")
    }
}

pub fn selftest_invoker() -> LoopDriverRunOutput {
    let good = [
        vec![(501, 233, "/bin/sh"), (0, 1, "/usr/sbin/cron")],
        vec![
            (501, 900, "/bin/bash"),
            (501, 233, "/bin/sh"),
            (0, 1, "/usr/sbin/cron"),
        ],
    ];
    let bad = [
        vec![(501, 1, "/usr/sbin/cron")],
        vec![(0, 9999, "/usr/sbin/cron")],
        vec![(0, 1, "/usr/sbin/crond")],
        vec![(0, 1, "/sbin/launchd")],
        vec![(0, 233, "/bin/sh"), (501, 1, "/usr/sbin/cron")],
        Vec::new(),
    ];
    let good_pass = good
        .iter()
        .all(|rows| invoker_from_rows(rows) == ("SCHEDULED", "cron_parent"));
    let bad_pass = bad
        .iter()
        .all(|rows| invoker_from_rows(rows) == ("MANUAL", "unproven_parent"));
    if good_pass && bad_pass {
        LoopDriverRunOutput::verdict(
            "selftest: PASS — lineage accepts only (uid 0 AND ppid 1 AND /usr/sbin/cron) and fails closed"
                .to_owned(),
            0,
        )
    } else {
        LoopDriverRunOutput::verdict("selftest: FAIL — invoker lineage".to_owned(), 1)
    }
}

pub fn run_live(config: &LoopDriverConfig, rules: &LoopDriverRules) -> LoopDriverRunOutput {
    let deadline = Deadline::new(config.deadline);
    let mut lock_rules = LockRules::from_env();
    if !rules.wall_bound {
        lock_rules.wall_bound = false;
    }
    let guard = match InstanceGuard::acquire(&config.lock_path, lock_rules) {
        Ok(guard) => guard,
        Err(error) => return lock_refusal(&config.lock_path, &error),
    };
    if let Some(line) = guard.wedged_kill_line() {
        append_driver_log(config, &line);
        println!("{line}");
    }
    if lock_rules.wall_bound {
        arm_wall_watchdog(
            Duration::from_secs(lock_rules.wall_bound_secs),
            config.lock_path.clone(),
            config.log.clone(),
        );
    }

    let invocation = match detect_invocation(config, deadline) {
        Ok(invocation) => invocation,
        Err(output) => return output,
    };
    write_lane_row(config, &invocation);

    let missing = ["timeout", "ntm", "br", "python3", "tmux"]
        .into_iter()
        .filter(|tool| !command_exists(tool))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        append_driver_log(
            config,
            &format!("PREFLIGHT FAILED — not on PATH: {}", missing.join(" ")),
        );
        append_driver_log(config, &format!("PATH={}", cron_path()));
        append_driver_log(config, "refusing to run; a partial tick is worse than none");
        return LoopDriverRunOutput::verdict(
            format!(
                "LOOP_DRIVER_REFUSED code=1 reason=preflight_missing_tools tools={}",
                missing.join(",")
            ),
            1,
        );
    }

    if !config.ledger_threshold_check.is_file() {
        append_driver_log(
            config,
            &format!(
                "LEDGER_THRESHOLD_UNRUN — missing:{}",
                config.ledger_threshold_check.display()
            ),
        );
        return LoopDriverRunOutput::verdict(
            format!(
                "LOOP_DRIVER_REFUSED code=1 reason=ledger_threshold_missing path={}",
                config.ledger_threshold_check.display()
            ),
            1,
        );
    }
    let threshold = match run_helper(
        &config.ledger_threshold_check,
        &["--check"],
        config,
        deadline,
    ) {
        Ok(result) => result,
        Err(output) => return output,
    };
    append_driver_log(
        config,
        &format!("ledger-threshold {}", threshold.combined().trim_end()),
    );
    if threshold.code() != 0 {
        append_driver_log(
            config,
            &format!("LEDGER_THRESHOLD_REFUSED rc={}", threshold.code()),
        );
        return LoopDriverRunOutput::verdict(
            format!(
                "LOOP_DRIVER_REFUSED code=1 reason=ledger_threshold rc={}",
                threshold.code()
            ),
            1,
        );
    }

    if !config.p6_rearm_check.is_file() {
        append_driver_log(
            config,
            &format!(
                "P6_REARM_UNRUN — missing:{}",
                config.p6_rearm_check.display()
            ),
        );
    } else {
        let p6 = match run_helper(&config.p6_rearm_check, &["--check"], config, deadline) {
            Ok(result) => result,
            Err(output) => return output,
        };
        append_driver_log(
            config,
            &format!("p6-rearm rc={} {}", p6.code(), p6.combined().trim_end()),
        );
    }

    let visible = match session_visible(config, deadline) {
        Ok(visible) => visible,
        Err(output) => return output,
    };
    if !visible {
        append_driver_log(
            config,
            &format!(
                "PREFLIGHT FAILED — session '{}' not visible to ntm OR tmux (both surfaces agree it is gone)",
                config.session
            ),
        );
        append_driver_log(
            config,
            &format!(
                "TMUX_TMPDIR={} (wrong socket dir shows an empty fleet)",
                config.tmux_tmpdir.display()
            ),
        );
        return LoopDriverRunOutput::verdict(
            format!(
                "LOOP_DRIVER_REFUSED code=1 reason=session_not_visible session={}",
                config.session
            ),
            1,
        );
    }

    if !config.repo.is_dir() {
        append_driver_log(config, &format!("cannot cd {}", config.repo.display()));
        return LoopDriverRunOutput::verdict(
            format!(
                "LOOP_DRIVER_REFUSED code=1 reason=repo_unreadable path={}",
                config.repo.display()
            ),
            1,
        );
    }
    append_driver_log(
        config,
        &format!(
            "tick start invoker={} invoker_proof={} ppid={}",
            invocation.invoker, invocation.proof, invocation.parent_pid
        ),
    );

    let chokepoint = config.repo.join("loop-kit/loop-start-chokepoint.sh");
    if !chokepoint.is_file() {
        append_driver_log(
            config,
            &format!(
                "REFUSING — charter chokepoint missing/unreadable: {}",
                chokepoint.display()
            ),
        );
        return LoopDriverRunOutput::verdict(
            format!(
                "LOOP_DRIVER_REFUSED code=1 reason=chokepoint_missing path={}",
                chokepoint.display()
            ),
            1,
        );
    }
    let mut choke_command = Command::new("/bin/bash");
    choke_command
        .arg(&chokepoint)
        .arg("--repo")
        .arg(&config.repo)
        .current_dir(&config.repo);
    command_env(&mut choke_command, config);
    let choke = match run_command(choke_command, deadline) {
        Ok(result) if result.timed_out => return deadline_output(deadline, "chokepoint"),
        Ok(result) => result,
        Err(error) => {
            return LoopDriverRunOutput::verdict(
                format!("LOOP_DRIVER_REFUSED code=1 reason=chokepoint_spawn detail={error}"),
                1,
            )
        }
    };
    let _ = append(&config.log, &choke.combined());
    if choke.code() != 0 {
        append_driver_log(
            config,
            "REFUSING — charter chokepoint denied loop entry (unsigned/latched charter)",
        );
        return LoopDriverRunOutput::verdict(
            "LOOP_DRIVER_REFUSED code=1 reason=chokepoint_denied".to_owned(),
            1,
        );
    }

    let mut tick_command = if config
        .loop_tick_bin
        .extension()
        .is_some_and(|ext| ext == "sh")
    {
        let mut command = Command::new("/bin/bash");
        command.arg(&config.loop_tick_bin);
        command
    } else {
        Command::new(&config.loop_tick_bin)
    };
    tick_command
        .arg("--dispatch")
        .current_dir(&config.repo)
        .env("LOOP_INVOKER", invocation.invoker)
        .env("LOOP_INVOKER_PROOF", invocation.proof);
    command_env(&mut tick_command, config);
    let tick_result = match run_command(tick_command, deadline) {
        Ok(result) => result,
        Err(error) => {
            return LoopDriverRunOutput::verdict(
                format!("LOOP_DRIVER_REFUSED code=1 reason=tick_spawn detail={error}"),
                1,
            )
        }
    };
    if rules.record_tick_output {
        let _ = append(&config.log, &tick_result.combined());
    }
    if tick_result.timed_out {
        append_driver_log(
            config,
            &classify_tick_failure(
                EXIT_DEADLINE,
                &tick_result.combined(),
                config.deadline.as_secs(),
            ),
        );
        return deadline_output(deadline, "loop_tick");
    }
    if tick_result.code() == 0 {
        append_driver_log(config, "tick ok");
    } else {
        append_driver_log(
            config,
            &classify_tick_failure(
                tick_result.code(),
                &tick_result.combined(),
                config.deadline.as_secs(),
            ),
        );
    }
    LoopDriverRunOutput::success()
}

fn unique_lock(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "loop-driver-{label}-{}-{nonce}.lock",
        std::process::id()
    ))
}

fn spawn_holder(
    exe: &Path,
    lock: &Path,
    hold: &str,
    extra_env: &[(&str, &str)],
) -> std::process::Child {
    let mut cmd = Command::new(exe);
    cmd.args([hold, "30"])
        .env("LOOP_DRIVER_LOCK", lock)
        .env("LOOP_DRIVER_WALL_BOUND_SECONDS", "3600")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("spawn holder");
    let mut line = String::new();
    if let Some(stdout) = child.stdout.as_mut() {
        use std::io::BufRead;
        let mut reader = std::io::BufReader::new(stdout);
        let _ = reader.read_line(&mut line);
    }
    assert!(
        line.contains("LOCK_HELD"),
        "holder did not acquire: {line:?}"
    );
    child
}

fn probe(
    exe: &Path,
    lock: &Path,
    extra_env: &[(&str, &str)],
    extra_args: &[&str],
) -> (i32, String) {
    let mut cmd = Command::new(exe);
    cmd.arg("--lock-probe")
        .args(extra_args)
        .env("LOOP_DRIVER_LOCK", lock)
        .env("LOOP_DRIVER_LIVENESS_GAP_MS", "1500")
        .env("LOOP_DRIVER_WALL_BOUND_SECONDS", "1")
        .stdin(Stdio::null());
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    let output = cmd.output().expect("probe");
    (
        output.status.code().unwrap_or(99),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

/// Plants a CPU-burning LIVE holder and a sleeping WEDGED holder.
/// Leg (a) LIVE is load-bearing: without it, always-crying-wedge would pass.
pub fn selftest_holder_liveness(exe: &Path) -> LoopDriverRunOutput {
    let mut fails = 0u32;
    let mut lines = Vec::new();

    // (a) genuinely working holder
    let live_lock = unique_lock("live");
    let mut live = spawn_holder(exe, &live_lock, "--hold-lock-working", &[]);
    std::thread::sleep(Duration::from_millis(1500));
    let (rc, out) = probe(exe, &live_lock, &[], &[]);
    let live_still = Command::new("/bin/kill")
        .args(["-0", &live.id().to_string()])
        .status()
        .is_ok_and(|s| s.success());
    if rc != EXIT_CONCURRENT || !out.contains("holder_liveness=LIVE") || !live_still {
        fails += 1;
        lines.push(format!(
            "selftest: FAIL — working holder must report LIVE and stay alive rc={rc} out={out:?} alive={live_still}"
        ));
    } else {
        lines.push("selftest: PASS — working holder reports LIVE and is not killed".into());
    }
    let _ = live.kill();
    let _ = live.wait();

    // (b) wedged holder older than bound is WEDGED+killed
    let wedge_lock = unique_lock("wedge");
    let mut wedge = spawn_holder(exe, &wedge_lock, "--hold-lock", &[]);
    std::thread::sleep(Duration::from_millis(1500));
    let (rc, out) = probe(exe, &wedge_lock, &[], &[]);
    std::thread::sleep(Duration::from_millis(200));
    let wedge_alive = Command::new("/bin/kill")
        .args(["-0", &wedge.id().to_string()])
        .status()
        .is_ok_and(|s| s.success());
    let wedged_named =
        out.contains("holder_liveness=WEDGED") || out.contains("LOOP_DRIVER_HOLDER_WEDGED");
    let killed = !wedge_alive || out.contains("killed=1") || out.contains("LOCK_ACQUIRED");
    if !wedged_named || !killed {
        fails += 1;
        lines.push(format!(
            "selftest: FAIL — wedged holder must report WEDGED and be killed rc={rc} out={out:?} alive={wedge_alive}"
        ));
    } else {
        lines.push("selftest: PASS — wedged holder reports WEDGED and is killed".into());
    }
    let _ = wedge.kill();
    let _ = wedge.wait();

    // (1) self-imposed wall bound kills own tree
    let wall_lock = unique_lock("wall");
    let mut wall = Command::new(exe);
    wall.args(["--hold-lock", "30"])
        .env("LOOP_DRIVER_LOCK", &wall_lock)
        .env("LOOP_DRIVER_WALL_BOUND_SECONDS", "1")
        .env("LOOP_DRIVER_WATCHDOG", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut wall_child = wall.spawn().expect("wall holder");
    let start = Instant::now();
    let (status, stdout) = loop {
        if let Some(status) = wall_child.try_wait().ok().flatten() {
            let mut buf = Vec::new();
            if let Some(mut s) = wall_child.stdout.take() {
                let _ = s.read_to_end(&mut buf);
            }
            break (
                status.code().unwrap_or(99),
                String::from_utf8_lossy(&buf).into_owned(),
            );
        }
        if start.elapsed() > Duration::from_secs(5) {
            let _ = wall_child.kill();
            let _ = wall_child.wait();
            break (99, String::new());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    if status != EXIT_DEADLINE || !stdout.contains("LOOP_DRIVER_WALL_BOUND") {
        fails += 1;
        lines.push(format!(
            "selftest: FAIL — wall bound must kill own tree with typed row rc={status} out={stdout:?}"
        ));
    } else {
        lines.push("selftest: PASS — run older than N intervals kills its own tree (LOOP_DRIVER_WALL_BOUND)".into());
    }

    // Mutation: bound off, wedge leg does not kill
    let mut_lock = unique_lock("mut");
    let mut mutant_holder = spawn_holder(exe, &mut_lock, "--hold-lock", &[]);
    std::thread::sleep(Duration::from_millis(1500));
    let (rc, out) = probe(
        exe,
        &mut_lock,
        &[("LOOP_DRIVER_DISABLE_WALL_BOUND", "1")],
        &["--mutation", "--disable-rule", "wall_bound"],
    );
    std::thread::sleep(Duration::from_millis(100));
    let mutant_alive = Command::new("/bin/kill")
        .args(["-0", &mutant_holder.id().to_string()])
        .status()
        .is_ok_and(|s| s.success());
    if mutant_alive && (out.contains("WEDGED") || rc == EXIT_CONCURRENT) {
        lines.push(
            "MUTATION RED wall_bound: disabling the bound leaves the planted wedge alive".into(),
        );
    } else {
        fails += 1;
        lines.push(format!(
            "selftest: FAIL — mutating the bound off must leave the wedge alive rc={rc} out={out:?} alive={mutant_alive}"
        ));
    }
    let _ = mutant_holder.kill();
    let _ = mutant_holder.wait();

    let body = lines.join("\n");
    if fails == 0 {
        LoopDriverRunOutput::verdict(format!("{body}\nSELFTEST PASS holder-liveness"), 0)
    } else {
        LoopDriverRunOutput::verdict(
            format!("{body}\nSELFTEST FAIL holder-liveness failures={fails}"),
            1,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deepest_walk_names_the_blocked_leaf() {
        assert_eq!(deepest_from_rows(&[10], &[]), Some(10));
        assert_eq!(deepest_from_rows(&[], &[]), None);
        let rows = [
            ProcessRow {
                pid: 2,
                ppid: 1,
                etimes: 10,
                cpu: 0.0,
                cputime_ms: 0,
            },
            ProcessRow {
                pid: 3,
                ppid: 2,
                etimes: 9,
                cpu: 0.0,
                cputime_ms: 0,
            },
        ];
        assert_eq!(
            deepest_from_rows(&[1], &rows),
            Some(3),
            "holder_pid must name the deepest descendant, not the flock parent"
        );
    }

    #[test]
    fn etime_parses_darwin_formats() {
        assert_eq!(parse_etime("00:01"), 1);
        assert_eq!(parse_etime("01:23"), 83);
        assert_eq!(parse_etime("05:36:09"), 5 * 3_600 + 36 * 60 + 9);
        assert_eq!(
            parse_etime("05-05:36:09"),
            5 * 86_400 + 5 * 3_600 + 36 * 60 + 9
        );
        assert_eq!(parse_etime("2-00:00:00"), 2 * 86_400);
    }

    #[test]
    fn liveness_busy_leaf_is_live_idle_leaf_is_wedged() {
        let busy_a = (10, 0.0, 0, 10);
        let busy_b = (12, 12.0, 0, 400);
        assert_eq!(
            classify_liveness(busy_a, busy_b, true),
            Some(HolderLiveness::Live)
        );
        let idle_a = (22639, 0.0, 0, 0);
        let idle_b = (22659, 0.0, 0, 0);
        assert_eq!(
            classify_liveness(idle_a, idle_b, true),
            Some(HolderLiveness::Wedged)
        );
        assert_eq!(classify_liveness(idle_a, idle_b, false), None);
    }

    #[test]
    fn lineage_requires_all_three_cron_fields_on_one_row() {
        assert_eq!(
            invoker_from_rows(&[(0, 233, "/bin/sh"), (501, 1, "/usr/sbin/cron")]),
            ("MANUAL", "unproven_parent"),
            "invoker lineage: split evidence must fail closed"
        );
        assert_eq!(
            invoker_from_rows(&[(0, 1, "/usr/sbin/cron")]),
            ("SCHEDULED", "cron_parent"),
            "invoker lineage: a genuine cron master must remain accepted"
        );
    }

    #[test]
    fn deadline_is_derived_below_the_cron_interval() {
        assert_eq!(DEFAULT_DEADLINE_SECONDS, SCHEDULE_INTERVAL_SECONDS - 60);
    }

    #[test]
    fn failure_reason_names_cargo_measurement_not_delivery() {
        let output = classify_tick_failure(
            1,
            "CARGO_LANE_BUDGET UNKNOWN scan_unavailable root=/tmp rc=124",
            SCHEDULE_INTERVAL_SECONDS,
        );
        assert!(output.contains("cargo-lane-budget measurement unavailable"));
        assert!(!output.contains("talking to nobody"));
    }
}

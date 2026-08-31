//! Three repeatable monitors behind one loop-enforcement choke point.
//!
//! WHY THIS CRATE EXISTS, stated so nobody re-derives it:
//!
//! 1. `/loop-enforcement` ships its choke point as `emit_tick.sh` + `tick_guard.sh`.
//!    Both are `.sh`, and this repo's `no-shell-gate` refuses tracked `.sh`/`.py`
//!    (proven to bite 2026-08-31: staged probes -> exit 101). Per AGENTS.md, "if you
//!    reach for a shell script, you have found a missing crate." This is that crate.
//!
//! 2. `pane-truth` cannot see an OMP v18 working pane. Measured 2026-08-31 07:30Z:
//!    `claims_busy` false on 6 of 6 panes, and pane %1409 -- braille spinner, advancing
//!    timer, 16.5% tree CPU -- was reported IDLE. Its detector matches
//!    `esc to interrupt` and a GERUND regex needing `Word...(Nh|m|s`; v18 renders
//!    `<braille> 12m  . <model> . <cwd>` -- no word, no ellipsis, no paren. Every
//!    fixture in that crate is the Claude format, so its green selftest AND its
//!    mutation leg are vacuous for OMP panes (fh C38). Tracked as
//!    `omp-orchestrator-pane-truth-omp-v18-blind-lre`; this crate does not fork
//!    pane-truth, it implements the v18 contract AGENTS.md actually specifies.
//!
//! 3. The two-capture liveness rule cannot be satisfied inside one invocation.
//!    pane-truth reports `liveness_two_capture: false` on every pane for exactly that
//!    reason. Here the prior capture lives in a state file, so tick N-1 and tick N form
//!    the pair. IDLE IS NEVER REPORTED FROM A SINGLE OBSERVATION.
//!
//! NO-CLAIM BOUNDARY: this crate classifies and records. It sends nothing, closes no
//! bead, and dispatches no work. `Unproven` is a first-class outcome and is excluded
//! from idle capacity -- an unrecognised capture is never an idle pane.

pub mod lifecycle;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// json
// ---------------------------------------------------------------------------

/// Escape a string for embedding in JSON output.
pub fn esc(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 2);
    for ch in input.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// proc -- the shared subprocess helper
// ---------------------------------------------------------------------------

/// The typed result of running one subprocess.
///
/// `TimedOut` is deliberately NOT `Completed { code: non-zero }`. AGENTS.md: "A timeout
/// is not a verdict" -- an empty buffer from a killed child must never map to the token a
/// genuinely failing subject produces. A caller matching on `Completed` structurally
/// cannot read a timeout as an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Completed {
        code: Option<i32>,
        stdout: String,
        stderr: String,
    },
    TimedOut {
        after_ms: u64,
        group_killed: bool,
    },
    SpawnFailed {
        message: String,
    },
}

impl Outcome {
    /// stdout only when the process genuinely completed. A timeout yields None, so a
    /// caller cannot accidentally treat a killed child's empty buffer as output.
    pub fn stdout_if_completed(&self) -> Option<&str> {
        match self {
            Outcome::Completed { stdout, .. } => Some(stdout),
            _ => None,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Outcome::Completed { .. } => "completed",
            Outcome::TimedOut { .. } => "timed_out",
            Outcome::SpawnFailed { .. } => "spawn_failed",
        }
    }
}

/// Run `argv` with a deadline, draining BOTH pipes concurrently and killing the whole
/// process GROUP on timeout.
///
/// Two measured failures this shape exists to prevent, both from AGENTS.md:
///   - "Kill the process GROUP, never the pid": orphaned grandchildren (`ppid=1`) held the
///     admission lock, so every timeout guaranteed the next attempt also failed -- the
///     failure created the condition for its own repetition.
///   - "Drain both pipes": undrained stdout+stderr deadlocks past ~64 KiB. The tell is 0%
///     CPU with no children, and widening the timeout only hides it longer.
///
/// `process_group(0)` makes the child a group leader, so its pgid equals its pid and one
/// signal to `-pid` reaches every descendant.
pub fn run(argv: &[&str], timeout: Duration) -> Outcome {
    if argv.is_empty() {
        return Outcome::SpawnFailed {
            message: "empty argv".to_owned(),
        };
    }
    let mut cmd = Command::new(argv[0]);
    cmd.args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return Outcome::SpawnFailed {
                message: format!("{}: {e}", argv[0]),
            }
        }
    };
    let pid = child.id();

    // Drain both pipes on their own threads BEFORE waiting. This is the deadlock fix:
    // a single-threaded read of stdout blocks while stderr fills its buffer.
    let out_pipe = child.stdout.take();
    let err_pipe = child.stderr.take();
    let out_thread = thread::spawn(move || read_all(out_pipe));
    let err_thread = thread::spawn(move || read_all(err_pipe));

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let status = child.wait();
        let _ = tx.send(status.ok().and_then(|s| s.code()));
    });

    let (timed_out, code) = match rx.recv_timeout(timeout) {
        Ok(code) => (false, code),
        Err(_) => {
            kill_group(pid);
            (true, None)
        }
    };

    let stdout = out_thread.join().unwrap_or_default();
    let stderr = err_thread.join().unwrap_or_default();

    if timed_out {
        Outcome::TimedOut {
            after_ms: timeout.as_millis() as u64,
            group_killed: true,
        }
    } else {
        Outcome::Completed {
            code,
            stdout,
            stderr,
        }
    }
}

fn read_all<R: Read>(pipe: Option<R>) -> String {
    let mut buf = Vec::new();
    if let Some(mut p) = pipe {
        let _ = p.read_to_end(&mut buf);
    }
    String::from_utf8_lossy(&buf).into_owned()
}

/// Signal the process GROUP (`-pid`), TERM then KILL. Dependency-free: this crate has no
/// `libc`, so the signal goes through `/bin/kill`, which accepts a negative pgid.
fn kill_group(pid: u32) {
    let neg = format!("-{pid}");
    let _ = Command::new("/bin/kill")
        .args(["-TERM", &neg])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    thread::sleep(Duration::from_millis(300));
    let _ = Command::new("/bin/kill")
        .args(["-KILL", &neg])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

// ---------------------------------------------------------------------------
// pane -- the OMP v18 status-line contract
// ---------------------------------------------------------------------------

/// What one pane's LAST status line proves.
///
/// `Unproven` is not a soft idle. It is excluded from idle capacity entirely, because
/// "I could not read this pane" and "this pane is free" are opposite conditions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneState {
    /// Braille spinner AND an elapsed timer on the last line.
    Working { timer_secs: u64 },
    /// The `pi` prompt glyph, no spinner.
    Idle,
    /// A queued-but-unsubmitted marker: accepts packets, runs nothing.
    Wedged,
    /// Not an agent pane, or an unrecognised capture.
    Unproven,
}

impl PaneState {
    pub fn label(&self) -> &'static str {
        match self {
            PaneState::Working { .. } => "WORKING",
            PaneState::Idle => "IDLE",
            PaneState::Wedged => "WEDGED",
            PaneState::Unproven => "UNPROVEN",
        }
    }
}

/// U+2800..U+28FF. A spinner frame.
pub fn is_braille(c: char) -> bool {
    matches!(c as u32, 0x2800..=0x28FF)
}

/// Parse an elapsed timer token (`27s`, `12m`, `1h`) into seconds.
///
/// Requires a LOWERCASE unit so `1.3M` (a token budget) and `S0.25` (a spend counter)
/// cannot be mistaken for elapsed time -- both appear on every live v18 status line.
pub fn parse_timer(line: &str) -> Option<u64> {
    let bytes: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i < bytes.len() {
                let unit = bytes[i];
                let after_ok = i + 1 >= bytes.len() || !bytes[i + 1].is_alphanumeric();
                if after_ok {
                    let n: u64 = bytes[start..i].iter().collect::<String>().parse().ok()?;
                    match unit {
                        's' => return Some(n),
                        'm' => return Some(n * 60),
                        'h' => return Some(n * 3600),
                        _ => {}
                    }
                }
            }
        } else {
            i += 1;
        }
    }
    None
}

/// The last non-blank, non-decoration line of a capture.
///
/// Anchoring on the LAST line is load-bearing. A whole-buffer scan matches a stale
/// spinner still sitting in scrollback -- measured 2026-08-31, one pane scored working
/// AND idle simultaneously while genuinely idle. It also blocks the false positive the
/// pane-truth bead warns about: a braille character inside quoted prose is not pane
/// state.
pub fn last_status_line(capture: &str) -> &str {
    capture
        .lines()
        .rev()
        .map(str::trim_end)
        .find(|l| {
            let t = l.trim();
            !t.is_empty()
                && !t.chars().all(|c| {
                    matches!(c, '-' | '=' | '_' | '\u{2500}' | '\u{2570}' | '\u{2502}' | '\u{256d}')
                })
        })
        .unwrap_or("")
}

/// Classify a capture by the OMP v18 contract: spinner + timer = Working, `pi` = Idle.
pub fn classify(capture: &str) -> PaneState {
    if capture.contains("Press up to edit queued messages")
        || capture.contains("Messages to be submitted after next tool call")
    {
        return PaneState::Wedged;
    }
    let line = last_status_line(capture);
    let spinner = line.chars().any(is_braille);
    let timer = parse_timer(line);
    match (spinner, timer) {
        (true, Some(secs)) => PaneState::Working { timer_secs: secs },
        _ if line.contains('\u{03c0}') => PaneState::Idle,
        _ => PaneState::Unproven,
    }
}

/// Hash of the capture with animated elements removed.
///
/// THE SPINNER TRAP: a hash over the raw frame changes every time the spinner advances,
/// so a dead pane still produces a changing hash and a busy-detector built on it reports
/// false-BUSY forever. Stripping braille frames and elapsed timers leaves only content
/// that real work changes.
pub fn stable_hash(capture: &str) -> u64 {
    let mut cleaned = String::with_capacity(capture.len());
    for ch in capture.chars() {
        if is_braille(ch) {
            continue;
        }
        cleaned.push(ch);
    }
    let stripped: String = cleaned
        .split_whitespace()
        .filter(|tok| parse_timer(tok).is_none())
        .collect::<Vec<_>>()
        .join(" ");
    let mut h = DefaultHasher::new();
    stripped.hash(&mut h);
    h.finish()
}

// ---------------------------------------------------------------------------
// liveness -- two captures, across ticks
// ---------------------------------------------------------------------------

/// The verdict from comparing this tick's observation against the previous tick's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Liveness {
    /// Timer advanced, or stable content changed. Positive proof of life.
    Live,
    /// `pi` glyph on both captures, far enough apart.
    ConfirmedIdle,
    /// WORKING on the previous capture, `pi` on this one: it just finished.
    ///
    /// Deliberately NOT dispatchable and deliberately NOT `Live`. Filed as
    /// `omp-orchestrator-transition-to-idle-misread-oco`: this case used to fall through a
    /// `_ => Live` catch-all, because "it moved, so it is not frozen" is true and is not
    /// the question a dispatcher asks. The operator spotted a freed worker my classifier
    /// had hidden. It stays out of `is_dispatchable` because one idle capture is still one
    /// capture -- the NEXT tick sees (Idle, Idle) and yields `ConfirmedIdle`. Naming it
    /// separately is what makes a just-freed worker VISIBLE without buying a slot by
    /// weakening the two-capture rule.
    NewlyIdle,
    /// Timer and stable hash both static across a sufficient gap.
    Frozen,
    /// Accepts input, submits nothing.
    Wedged,
    /// One capture only, gap too short, or unreadable. NOT idle.
    Unproven { why: &'static str },
}

impl Liveness {
    pub fn label(&self) -> &'static str {
        match self {
            Liveness::Live => "LIVE",
            Liveness::ConfirmedIdle => "CONFIRMED_IDLE",
            Liveness::NewlyIdle => "NEWLY_IDLE",
            Liveness::Frozen => "FROZEN",
            Liveness::Wedged => "WEDGED",
            Liveness::Unproven { .. } => "UNPROVEN",
        }
    }
    /// Only a two-capture confirmed idle may receive work.
    pub fn is_dispatchable(&self) -> bool {
        matches!(self, Liveness::ConfirmedIdle)
    }
    /// Free capacity a conductor should be AWARE of, whether or not it may be filled yet.
    /// `NewlyIdle` belongs here and not in `is_dispatchable`: report it, confirm it, then
    /// fill it.
    pub fn is_free_capacity(&self) -> bool {
        matches!(self, Liveness::ConfirmedIdle | Liveness::NewlyIdle)
    }
}

/// Minimum gap between the two captures.
///
/// 75s, not 30s: measured, a lane deep in a long tool call has a STATIC timer and
/// changing output, and a 30s window called two live panes frozen. The asymmetry decides
/// the number -- a missed freeze costs idle minutes, a false freeze destroys work in
/// flight.
pub const MIN_GAP_SECS: u64 = 75;

/// One pane's observation at a point in time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub pane_id: String,
    pub state: PaneState,
    pub hash: u64,
    pub at: u64,
}

pub fn liveness(prev: Option<&Observation>, now: &Observation) -> Liveness {
    if matches!(now.state, PaneState::Wedged) {
        return Liveness::Wedged;
    }
    if matches!(now.state, PaneState::Unproven) {
        return Liveness::Unproven {
            why: "capture_unrecognised",
        };
    }
    let Some(prev) = prev else {
        return Liveness::Unproven {
            why: "no_prior_capture",
        };
    };
    if prev.pane_id != now.pane_id {
        return Liveness::Unproven {
            why: "pane_id_mismatch",
        };
    }
    let gap = now.at.saturating_sub(prev.at);
    if gap < MIN_GAP_SECS {
        return Liveness::Unproven {
            why: "gap_too_short",
        };
    }
    // Exhaustive on purpose. The `_ => Live` catch-all this replaces is what hid a freed
    // worker (bead -oco): every unlisted pair silently became "motion", which is true and
    // useless. If a new PaneState is added, this match must fail to compile rather than
    // absorb it into Live.
    match (&prev.state, &now.state) {
        (PaneState::Working { timer_secs: a }, PaneState::Working { timer_secs: b }) => {
            if b > a || prev.hash != now.hash {
                Liveness::Live
            } else {
                Liveness::Frozen
            }
        }
        // Two idle captures across the floor. Content may differ -- a pane can render
        // while idle -- and that does not make it busy.
        (PaneState::Idle, PaneState::Idle) => Liveness::ConfirmedIdle,
        // It just finished. Visible as free capacity, not yet fillable.
        (PaneState::Working { .. }, PaneState::Idle) => Liveness::NewlyIdle,
        // It picked work up. Unambiguously alive.
        (PaneState::Idle, PaneState::Working { .. }) => Liveness::Live,
        // Anything involving Wedged/Unproven on EITHER side is not a liveness claim.
        // `now` being Wedged/Unproven already returned above, so this is a prior-side
        // Wedged/Unproven paired with a readable now: motion, but not a verdict we will
        // act on, so report it as unproven rather than inventing Live.
        (PaneState::Wedged | PaneState::Unproven, _) => Liveness::Unproven {
            why: "prior_capture_unusable",
        },
        (_, PaneState::Wedged | PaneState::Unproven) => Liveness::Unproven {
            why: "capture_unrecognised",
        },
    }
}

// ---------------------------------------------------------------------------
// guard -- loop-enforcement, compiled to exit codes
// ---------------------------------------------------------------------------

pub const MODES: [&str; 10] = [
    "DISPATCH",
    "RESEARCH",
    "PLANNING",
    "BEAD_POLISH",
    "PRD_TO_BEADS",
    "L1_REMEDIATION",
    "L2_REMEDIATION",
    "L3_REMEDIATION",
    "HOLD_ESCALATED",
    "BLOCKED",
];

/// Unchanged by the BLOCKED class and evaluated on BLOCKED ticks exactly as on every
/// other mode: honest blocking is expressible only as STRUCTURE, never as prose.
pub const FORBIDDEN: [&str; 6] = [
    "standing by",
    "queue empty",
    "blocked on josh",
    "wait_josh",
    "no state change",
    "session done",
];

pub const BLOCKER_CLASSES: [&str; 8] = [
    "upstream",
    "dependency",
    "external-service",
    "credential",
    "data",
    "rate-limit",
    "infrastructure",
    "joshua-decision",
];

const REMEDIATION: [&str; 4] = [
    "L1_REMEDIATION",
    "L2_REMEDIATION",
    "L3_REMEDIATION",
    "HOLD_ESCALATED",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reject {
    UnknownMode(String),
    ForbiddenPhrase(String),
    MissingBlocker,
    BadBlockerClass(String),
    BlockerNameTooShort,
    JoshuaDecisionNeedsBead,
    NoEscalationArtifact,
    EscalationRequired { blocker: String, streak: u32 },
}

impl Reject {
    /// A distinct exit code per rejection reason. "The gate said no" and "the gate could
    /// not run" must never print the same thing.
    pub fn code(&self) -> i32 {
        match self {
            Reject::UnknownMode(_) => 4,
            Reject::ForbiddenPhrase(_) => 5,
            Reject::MissingBlocker
            | Reject::BadBlockerClass(_)
            | Reject::BlockerNameTooShort
            | Reject::JoshuaDecisionNeedsBead
            | Reject::NoEscalationArtifact => 6,
            Reject::EscalationRequired { .. } => 7,
        }
    }
    pub fn message(&self) -> String {
        match self {
            Reject::UnknownMode(m) => format!(
                "mode {m:?} is not in the enum; fabricated mode strings hard-reject. valid: {}",
                MODES.join(", ")
            ),
            Reject::ForbiddenPhrase(p) => {
                format!("forbidden phrase {p:?}: a blocked tick must be structure, not prose")
            }
            Reject::MissingBlocker => {
                "verdict BLOCKED requires external_blocker=<class>:<name>".to_owned()
            }
            Reject::BadBlockerClass(c) => format!(
                "blocker class {c:?} not allowed; valid: {}",
                BLOCKER_CLASSES.join(", ")
            ),
            Reject::BlockerNameTooShort => "blocker name must be >=3 chars".to_owned(),
            Reject::JoshuaDecisionNeedsBead => {
                "joshua-decision: MUST NAME A BEAD; an unnamed human gate is 'blocked on Josh' in a new hat".to_owned()
            }
            Reject::NoEscalationArtifact => {
                "BLOCKED requires escalation_action or auto_filed_bead".to_owned()
            }
            Reject::EscalationRequired { blocker, streak } => format!(
                "{streak} consecutive ticks on blocker {blocker:?}: this tick needs escalation_action, auto_filed_bead, or a *_REMEDIATION/HOLD_ESCALATED mode. a repeated block is a bead, not a rhythm"
            ),
        }
    }
}

/// One tick, as submitted to the choke point.
#[derive(Debug, Clone, Default)]
pub struct Tick {
    pub mode: String,
    pub verdict: String,
    pub external_blocker: Option<String>,
    pub escalation_action: Option<String>,
    pub auto_filed_bead: Option<String>,
    pub note: String,
}

impl Tick {
    fn all_text(&self) -> String {
        let mut s = format!("{} {} {}", self.mode, self.verdict, self.note);
        for opt in [
            &self.external_blocker,
            &self.escalation_action,
            &self.auto_filed_bead,
        ] {
            if let Some(v) = opt {
                s.push(' ');
                s.push_str(v);
            }
        }
        s.to_lowercase()
    }
    fn has_escalation(&self) -> bool {
        self.escalation_action.as_ref().is_some_and(|s| !s.trim().is_empty())
            || self.auto_filed_bead.as_ref().is_some_and(|s| !s.trim().is_empty())
            || REMEDIATION.contains(&self.mode.as_str())
    }
}

/// Validate a tick. `prior_blocker`/`prior_streak` come from the state file, so the
/// 3-identical-blocker ladder survives across invocations.
pub fn validate(tick: &Tick, prior_blocker: &str, prior_streak: u32) -> Result<u32, Reject> {
    if !MODES.contains(&tick.mode.as_str()) {
        return Err(Reject::UnknownMode(tick.mode.clone()));
    }
    let text = tick.all_text();
    for phrase in FORBIDDEN {
        if text.contains(phrase) {
            return Err(Reject::ForbiddenPhrase(phrase.to_owned()));
        }
    }

    let blocked = tick.verdict.eq_ignore_ascii_case("BLOCKED") || tick.mode == "BLOCKED";
    let blocker = tick.external_blocker.clone().unwrap_or_default();

    if blocked {
        if blocker.trim().is_empty() {
            return Err(Reject::MissingBlocker);
        }
        let (class, name) = blocker.split_once(':').ok_or(Reject::MissingBlocker)?;
        if !BLOCKER_CLASSES.contains(&class) {
            return Err(Reject::BadBlockerClass(class.to_owned()));
        }
        if name.trim().len() < 3 {
            return Err(Reject::BlockerNameTooShort);
        }
        if class == "joshua-decision" && !name.contains('-') {
            return Err(Reject::JoshuaDecisionNeedsBead);
        }
        if !tick.has_escalation() {
            return Err(Reject::NoEscalationArtifact);
        }
    }

    // Streak on the blocker identity, whatever the verdict.
    let streak = if !blocker.is_empty() && blocker == prior_blocker {
        prior_streak + 1
    } else if blocker.is_empty() {
        0
    } else {
        1
    };

    if streak >= 3 && !tick.has_escalation() {
        return Err(Reject::EscalationRequired {
            blocker,
            streak,
        });
    }
    Ok(streak)
}

// ---------------------------------------------------------------------------
// state
// ---------------------------------------------------------------------------

/// Prior-tick state. TSV on purpose: this crate has no serde, and a flat line format is
/// parseable in std without a hand-rolled JSON reader that could silently mis-read.
/// Output is JSON; state is TSV. The asymmetry is deliberate.
#[derive(Debug, Default, Clone)]
pub struct State {
    pub last_tick: u64,
    pub last_blocker: String,
    pub blocker_streak: u32,
    pub red_streak: u32,
    pub panes: Vec<Observation>,
    pub commits: Vec<(String, String)>,
}

pub fn state_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_owned());
    Path::new(&home)
        .join(".local/state/omp-orchestrator")
        .join("tick-monitor.tsv")
}

pub fn load(path: &Path) -> State {
    let mut st = State::default();
    let Ok(text) = std::fs::read_to_string(path) else {
        return st;
    };
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        match f.as_slice() {
            ["last_tick", v] => st.last_tick = v.parse().unwrap_or(0),
            ["last_blocker", v] => st.last_blocker = (*v).to_owned(),
            ["blocker_streak", v] => st.blocker_streak = v.parse().unwrap_or(0),
            ["red_streak", v] => st.red_streak = v.parse().unwrap_or(0),
            ["commit", repo, sha] => st.commits.push(((*repo).to_owned(), (*sha).to_owned())),
            ["pane", id, label, timer, hash, at] => {
                let state = match *label {
                    "WORKING" => PaneState::Working {
                        timer_secs: timer.parse().unwrap_or(0),
                    },
                    "IDLE" => PaneState::Idle,
                    "WEDGED" => PaneState::Wedged,
                    _ => PaneState::Unproven,
                };
                st.panes.push(Observation {
                    pane_id: (*id).to_owned(),
                    state,
                    hash: hash.parse().unwrap_or(0),
                    at: at.parse().unwrap_or(0),
                });
            }
            _ => {}
        }
    }
    st
}

pub fn save(path: &Path, st: &State) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut out = String::new();
    out.push_str(&format!("last_tick\t{}\n", st.last_tick));
    out.push_str(&format!("last_blocker\t{}\n", st.last_blocker));
    out.push_str(&format!("blocker_streak\t{}\n", st.blocker_streak));
    out.push_str(&format!("red_streak\t{}\n", st.red_streak));
    for (repo, sha) in &st.commits {
        out.push_str(&format!("commit\t{repo}\t{sha}\n"));
    }
    for p in &st.panes {
        let timer = match &p.state {
            PaneState::Working { timer_secs } => *timer_secs,
            _ => 0,
        };
        out.push_str(&format!(
            "pane\t{}\t{}\t{}\t{}\t{}\n",
            p.pane_id,
            p.state.label(),
            timer,
            p.hash,
            p.at
        ));
    }
    // Write-then-rename so a crash mid-write cannot leave a truncated state file that
    // reads as "no prior capture" and silently downgrades every pane to Unproven.
    let tmp = path.with_extension("tsv.tmp");
    std::fs::write(&tmp, out)?;
    std::fs::rename(&tmp, path)
}

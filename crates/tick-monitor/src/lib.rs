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
        // Enumerated, not wildcarded: both non-Completed arms are RESTRICTIVE
        // terminals, and a wildcard here would silently swallow a fourth variant
        // into "no output" — the same shape as reading a killed child's empty
        // buffer as a real answer, which is what this method exists to prevent.
        match self {
            Outcome::Completed { stdout, .. } => Some(stdout),
            Outcome::TimedOut { .. } => None,
            Outcome::SpawnFailed { .. } => None,
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
    /// An Ask/approval dialog is open ABOVE the status line. ALIVE and blocked on an
    /// answer -- the opposite of dead. `timer_secs` is the pane's turn timer, which keeps
    /// ADVANCING while the pane waits, which is precisely why this needed its own state.
    Dialog { timer_secs: u64 },
    /// Not an agent pane, or an unrecognised capture.
    Unproven,
}

impl PaneState {
    pub fn label(&self) -> &'static str {
        match self {
            PaneState::Working { .. } => "WORKING",
            PaneState::Idle => "IDLE",
            PaneState::Wedged => "WEDGED",
            PaneState::Dialog { .. } => "DIALOG",
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
                    matches!(
                        c,
                        '-' | '=' | '_' | '\u{2500}' | '\u{2570}' | '\u{2502}' | '\u{256d}'
                    )
                })
        })
        .unwrap_or("")
}

/// The model-name contract: every OMP/agent status line names its model.
pub const MODEL_MARKERS: &[&str] = &["Opus 5", "GLM 5.3", "GPT-5.6", "GPT-5.5"];

/// Verbatim footer markers from an OMP v18 Ask dialog (captured from `%1372`, 2026-08-31).
const DIALOG_FOOTER: &[&str] = &["Enter select", "Esc cancel", "\u{2191}/\u{2193} move"];

/// True when an Ask/approval dialog is open above the status line.
///
/// MEASURED 2026-08-31 on `%1372`: on OMP v18 the dialog renders ABOVE the status line and
/// the status line remains last, WITH an advancing spinner and timer. So the failure is not
/// "covered status line -> unreadable"; it is that a pane blocked on a human answer reads as
/// healthy WORKING work indefinitely. Same class as `Wedged`: looks busy, is not.
///
/// POSITIONAL, not shape-based. My own pane carries BOX-FRAMED lines containing `Esc cancel`
/// -- OMP renders tool-call blocks inside frames and my commands quoted the marker -- so
/// `framed && contains(marker)` self-pollutes exactly like the grader-reply citation bug.
/// The discriminator that survives is ADJACENCY: the footer sits within the three non-blank
/// lines immediately above the status line.
pub fn dialog_open(capture: &str) -> bool {
    let nb: Vec<&str> = capture
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .collect();
    // No model line at all: not an agent pane. NOT a dialog claim, and NOT death -- death
    // is decided only by absence from `tmux list-panes`, never by an unreadable capture.
    let Some(status_ix) = nb
        .iter()
        .rposition(|l| MODEL_MARKERS.iter().any(|m| l.contains(m)))
    else {
        return false;
    };
    let lo = status_ix.saturating_sub(3);
    nb[lo..status_ix].iter().any(|l| {
        let t = l.trim();
        t.starts_with('\u{2502}') && DIALOG_FOOTER.iter().any(|m| t.contains(m))
    })
}

/// Classify a capture by the OMP v18 contract: spinner + timer = Working, `pi` = Idle.
pub fn classify(capture: &str) -> PaneState {
    if capture.contains("Press up to edit queued messages")
        || capture.contains("Messages to be submitted after next tool call")
    {
        return PaneState::Wedged;
    }
    // BEFORE the spinner branch, like `Wedged`: a dialog pane HAS a live spinner and timer,
    // so checking spinner first would score it WORKING and hide a pane awaiting an answer.
    if dialog_open(capture) {
        return PaneState::Dialog {
            timer_secs: parse_timer(last_status_line(capture)).unwrap_or(0),
        };
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
        if is_braille(ch) || ch == 'π' {
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
    /// An Ask dialog is open: ALIVE, and blocked on an answer rather than on work.
    ///
    /// Not dispatchable (it cannot accept a packet) and not free capacity (it is occupied),
    /// but it MUST be surfaced. Measured 2026-08-31: `%1372` sat 26+ minutes on the
    /// arc-keepalive install approval reading as `WORKING`/`LIVE`, so the escalation it was
    /// waiting on was invisible to the conductor while looking perfectly healthy.
    Dialog { timer_secs: u64 },
    /// The capture succeeded and the pane is PRESENT, but no model-name line was found --
    /// and it carried one last tick. Alive, and unreadable rather than idle or dead.
    ///
    /// Distinct from `Dialog` on purpose. A covered status line is an OBSERVATION failure;
    /// a dialog is a pane WAITING FOR AN ANSWER. Distinct from `Unproven` because that gets
    /// dropped from capacity, which is how a live pane goes untended.
    Obscured,
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
            Liveness::Dialog { .. } => "DIALOG",
            Liveness::Obscured => "OBSCURED",
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
    /// Alive but blocked on an ANSWER, not on work. The conductor must act; a dispatcher
    /// must not. Kept separate from `is_free_capacity` on purpose: answering is the action,
    /// not filling.
    pub fn needs_answer(&self) -> bool {
        matches!(self, Liveness::Dialog { .. })
    }
    /// Alive, but the conductor must LOOK rather than fill. `Dialog` needs an answer;
    /// `Obscured` needs a deeper capture. Both used to vanish into `Unproven` and be
    /// dropped from capacity, which is precisely how a live pane goes untended.
    pub fn needs_attention(&self) -> bool {
        matches!(self, Liveness::Dialog { .. } | Liveness::Obscured)
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
    // Alive and awaiting an answer. Returned BEFORE the two-capture machinery on purpose:
    // its timer advances while blocked, so the (Working, Working) arm would call it Live.
    if let PaneState::Dialog { timer_secs } = now.state {
        return Liveness::Dialog { timer_secs };
    }
    let Some(prev) = prev else {
        return Liveness::Unproven {
            why: "no_prior_capture",
        };
    };
    // MUST precede any use of `prev.state`: comparing a prior observation from a DIFFERENT
    // pane is not evidence about this one.
    if prev.pane_id != now.pane_id {
        return Liveness::Unproven {
            why: "pane_id_mismatch",
        };
    }
    // AN UNREADABLE CAPTURE FROM A PANE THAT WAS AN AGENT LAST TICK IS NOT A NON-EVENT.
    //
    // This check used to sit ABOVE the `prev` resolution and returned a flat `Unproven`,
    // which the conductor drops from capacity -- and dropping is exactly how a live pane
    // goes untended. The discriminator is the PRIOR observation, not the shape of this
    // capture: a pane that carried a model-name line last tick and carries none now is
    // alive and OBSCURED, while a pane that never had one is a shell.
    //
    // Deliberately NOT `Dialog`. Measured false positive on another watcher, %1414 at
    // 09:38Z: a box-drawing region briefly covered the status line of a pane that was
    // mid-work at 26/26, and the watcher reported DIALOG. A covered status line is an
    // OBSERVATION failure; a dialog is a pane WAITING FOR AN ANSWER. Claiming the second
    // from evidence for the first invents a blocker that does not exist.
    if matches!(now.state, PaneState::Unproven) {
        return match prev.state {
            PaneState::Working { .. } | PaneState::Idle | PaneState::Dialog { .. } => {
                Liveness::Obscured
            }
            PaneState::Wedged | PaneState::Unproven => Liveness::Unproven {
                why: "capture_unrecognised",
            },
        };
    }
    let gap = now.at.saturating_sub(prev.at);

    // POSITIVE PROOF IS HOISTED ABOVE THE GAP FLOOR, and the asymmetry is the point.
    //
    // Measured defect, recorded in 05-actions A1 and flagged again by the held-out
    // operator-at-3am lens: the floor returned `Unproven { gap_too_short }` BEFORE the
    // `(Working, Working)` arm could compare timers and hashes, so a pane that had
    // demonstrably moved was reported unproven whenever two captures landed inside 75
    // seconds. The section's own words: "positive proof of life is discarded.
    // Correctly reasoned, incorrectly implemented."
    //
    // WHY THE FLOOR EXISTS AT ALL, so this does not read as removing a guard: the
    // ABSENCE of change over a short window proves nothing — a genuinely working pane
    // may render nothing for two seconds, and calling that Frozen would be a false
    // accusation. The floor protects the NEGATIVE verdict.
    //
    // The PRESENCE of change needs no such protection. A turn timer that advanced, or a
    // content hash that differs, cannot occur in a dead pane at any gap. The floor now
    // guards only the direction it was reasoned for.
    if let (
        PaneState::Working { timer_secs: before },
        PaneState::Working { timer_secs: after },
    ) = (&prev.state, &now.state)
    {
        if after > before || prev.hash != now.hash {
            return Liveness::Live;
        }
    }

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
        // Prior was blocked awaiting an answer; `now` is readable, so the dialog was
        // answered and the pane resumed. Real motion -- but a timer that advanced across a
        // human's thinking time is not evidence about WORK, so it buys no verdict. `now`
        // being Dialog already returned above; this arm covers the resumed cases.
        (PaneState::Dialog { .. }, _) => Liveness::Unproven {
            why: "prior_awaiting_answer",
        },
        // `now` being Dialog returned above, so that half is unreachable here -- but the
        // match is exhaustive by design, so it must be spelled rather than wildcarded. A
        // wildcard is exactly what let the WORKING->IDLE transition fall into `Live`.
        (_, PaneState::Wedged | PaneState::Unproven | PaneState::Dialog { .. }) => {
            Liveness::Unproven {
                why: "capture_unrecognised",
            }
        }
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
        self.escalation_action
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty())
            || self
                .auto_filed_bead
                .as_ref()
                .is_some_and(|s| !s.trim().is_empty())
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
        return Err(Reject::EscalationRequired { blocker, streak });
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
    /// The pid that owns this ledger.
    ///
    /// # Why this exists (measured 2026-08-31)
    ///
    /// Two `tick-monitor watch` processes ran against this one file: pid 36597 at
    /// `--interval 90` and pid 40931 at `--interval 45`. `save()` truncates and
    /// rewrites wholesale, so the 90s writer kept stamping `last_tick` while the
    /// 45s reader computed `now - last_tick`. The gap decayed 15s per tick —
    /// 75, 66, 51, 36, 22, 6 — until it fell under [`MIN_GAP_SECS`] and the
    /// two-capture liveness rule self-disabled.
    ///
    /// **Measured impact: 2 of 11 ticks (18%) yielded a usable liveness verdict.**
    /// The other 82% reported `gap_too_short` when the real inter-observation gap
    /// was a healthy 75s every single time. Because `free_capacity` requires
    /// `Confirmed` idle, an idle worker stayed invisible for roughly four ticks —
    /// the exact "idle worker beside a ready queue" failure this crate exists to
    /// prevent, caused by the monitor watching for it.
    ///
    /// A pid is a sound ownership token *here* — unlike the marker-file pid in
    /// AGENTS.md C112, this one names the process that does the writing, so it
    /// dies with the thing it owns.
    pub owner_pid: u32,
}

// ---------------------------------------------------------------------------
// idle-capacity escalation
// ---------------------------------------------------------------------------

/// Events emitted by the persistent idle-capacity streak tracker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapacityAlarmEvent {
    None { consecutive_ticks: u32 },
    Fire { consecutive_ticks: u32 },
}

/// Tracks free capacity independently of the no-value/stall streak. A fully occupied
/// fleet resets the alarm; it can never fire merely because ticks continue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacityAlarm {
    threshold: u32,
    consecutive_free_ticks: u32,
    fired: bool,
}

impl CapacityAlarm {
    pub fn new(threshold: u32) -> Self {
        Self {
            threshold: threshold.max(1),
            consecutive_free_ticks: 0,
            fired: false,
        }
    }

    pub fn observe(&mut self, free_capacity: bool) -> CapacityAlarmEvent {
        if !free_capacity {
            self.consecutive_free_ticks = 0;
            self.fired = false;
            return CapacityAlarmEvent::None {
                consecutive_ticks: 0,
            };
        }
        self.consecutive_free_ticks = self.consecutive_free_ticks.saturating_add(1);
        if !self.fired && self.consecutive_free_ticks >= self.threshold {
            self.fired = true;
            CapacityAlarmEvent::Fire {
                consecutive_ticks: self.consecutive_free_ticks,
            }
        } else {
            CapacityAlarmEvent::None {
                consecutive_ticks: self.consecutive_free_ticks,
            }
        }
    }

    pub fn consecutive_free_ticks(&self) -> u32 {
        self.consecutive_free_ticks
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapacityEscalationReceipt {
    pub urgent_path: PathBuf,
    pub notification_observed: bool,
    pub consecutive_ticks: u32,
}

/// Escalate through the real macOS notification surface. The urgent artifact is written
/// first so a notification failure cannot erase the durable signal. Tests use the sibling
/// `_with_notifier` seam with `/bin/echo`; production always uses `/usr/bin/osascript`.
pub fn escalate_idle_capacity(
    urgent_path: &Path,
    tick: u64,
    consecutive_ticks: u32,
    observation: &str,
) -> Result<CapacityEscalationReceipt, String> {
    escalate_idle_capacity_with_notifier(
        urgent_path,
        tick,
        consecutive_ticks,
        observation,
        Path::new("/usr/bin/osascript"),
    )
}

pub fn escalate_idle_capacity_with_notifier(
    urgent_path: &Path,
    tick: u64,
    consecutive_ticks: u32,
    observation: &str,
    notifier: &Path,
) -> Result<CapacityEscalationReceipt, String> {
    if consecutive_ticks == 0 {
        return Err("idle-capacity escalation requires a positive streak".to_owned());
    }
    if let Some(parent) = urgent_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "cannot create urgent directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let urgent = format!(
        "URGENT: persistent idle capacity\nconsecutive_ticks: {consecutive_ticks}\ntick: {tick}\nobservation: {observation}\naction: macOS notification requested via osascript\n"
    );
    std::fs::write(urgent_path, urgent).map_err(|error| {
        format!(
            "cannot write urgent artifact {}: {error}",
            urgent_path.display()
        )
    })?;

    let notification = format!(
        "display notification {} with title \"OMP idle capacity\" sound name \"Sosumi\"",
        applescript_string(&format!(
            "{consecutive_ticks} consecutive ticks; dispatch or grade now (tick {tick})"
        ))
    );
    let executable = notifier.to_str().ok_or_else(|| {
        format!(
            "notification executable is not UTF-8: {}",
            notifier.display()
        )
    })?;
    let argv = [executable, "-e", notification.as_str()];
    match run(&argv, Duration::from_secs(10)) {
        Outcome::Completed { code: Some(0), .. } => Ok(CapacityEscalationReceipt {
            urgent_path: urgent_path.to_owned(),
            notification_observed: true,
            consecutive_ticks,
        }),
        Outcome::Completed { code, stderr, .. } => Err(format!(
            "notification command exited {:?}: {}",
            code,
            stderr.trim()
        )),
        Outcome::TimedOut { after_ms, .. } => {
            Err(format!("notification command timed out after {after_ms}ms"))
        }
        Outcome::SpawnFailed { message } => {
            Err(format!("notification command failed to spawn: {message}"))
        }
    }
}

fn applescript_string(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 2);
    out.push('\"');
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('\"');
    out
}

/// Where a watcher keeps its state, **scoped to the session it watches**.
///
/// # Why this takes an argument (measured 2026-08-31)
///
/// This used to return one fixed path with no session component — and the
/// directory was hardcoded to `omp-orchestrator`, so a watcher on any of the
/// other seven live sessions wrote its state into a folder named after a
/// session it was not watching.
///
/// Eight tmux sessions were live at the time. Every watcher started without an
/// explicit `--state` therefore contended for **one** file. The damage is not a
/// crash: `save()` truncates and rewrites wholesale, so watchers silently
/// overwrite each other's `last_tick`, the observation gap decays on the wrong
/// cadence, and the two-capture liveness rule reports `gap_too_short` instead of
/// an error. Measured on the one collision found by hand: **2 of 11 ticks (18%)
/// yielded a usable verdict.**
///
/// [`check_ownership`] makes that collision *visible*, but refusing is the wrong
/// outcome at scale — eight sessions **should** have eight watchers. Scoping the
/// path is what makes them independent; ownership then only fires when two
/// watchers genuinely target the same session, which is a real conflict.
///
/// A session name reaches the filesystem here, so it is sanitised: anything
/// outside `[A-Za-z0-9._-]` becomes `_`. `tmux` permits `/` and `..` in session
/// names, and an unsanitised name would escape the state directory.
pub fn state_path(session: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_owned());
    let safe: String = session
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '_' })
        .collect();
    // Collapse any traversal-shaped run. `../../etc/passwd` sanitises to
    // `.._.._etc_passwd`, which is a harmless single segment — but leaving a
    // literal `..` in a path this code joins invites a later refactor to split
    // on it. Distinctness is preserved: two different names stay different.
    // ORDER MATTERS. Collapsing first turns ".." into "__", which is neither
    // empty nor all-dots, so the degenerate check below stops firing. Caught by
    // an_empty_or_dotted_session_still_yields_a_usable_path within a minute of
    // being written — the fallback must be decided on the ORIGINAL shape.
    let safe = if safe.is_empty() || safe.chars().all(|c| c == '.') {
        "unnamed".to_owned()
    } else {
        safe.replace("..", "__")
    };
    Path::new(&home)
        .join(".local/state/omp-orchestrator/sessions")
        .join(safe)
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
            ["owner_pid", v] => st.owner_pid = v.parse().unwrap_or(0),
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
                    // Without this arm the writer emits DIALOG and the next tick reads it
                    // back as Unproven -- a silent downgrade that loses the prior-side
                    // "was awaiting an answer" fact one tick after it was established.
                    "DIALOG" => PaneState::Dialog {
                        timer_secs: timer.parse().unwrap_or(0),
                    },
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

/// True when `pid` names a process that currently exists.
///
/// `kill(pid, 0)` is the portable liveness probe: it performs permission and
/// existence checks and delivers nothing. A dead owner must not hold the ledger
/// forever, so a stale pid is reclaimable.
fn pid_is_live(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    // SAFETY-FREE: no unsafe here — shell out is not needed; /proc is absent on
    // macOS, so use the same check `kill -0` performs via std.
    std::path::Path::new("/proc").join(pid.to_string()).exists()
        || std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
}

/// Refuse to write a ledger a different LIVE process owns.
///
/// Fail-closed: two writers silently corrupt the gap arithmetic that the
/// two-capture liveness rule depends on, and the corruption reads as
/// `gap_too_short` rather than as an error. Better to refuse and say why.
pub fn check_ownership(path: &Path, my_pid: u32) -> Result<(), String> {
    let existing = load(path).owner_pid;
    if existing == 0 || existing == my_pid || !pid_is_live(existing) {
        return Ok(());
    }
    Err(format!(
        "LEDGER CONTENDED: {} is owned by live pid {existing}, not this process ({my_pid}).\n\
         Two watchers on one ledger make `last_tick` advance on the wrong cadence, and the \
         resulting gap decay disables the two-capture liveness rule silently — it reports \
         gap_too_short, never an error. Stop the other watcher or give this one its own \
         --state path.",
        path.display()
    ))
}

pub fn save(path: &Path, st: &State) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut out = String::new();
    out.push_str(&format!("owner_pid\t{}\n", st.owner_pid));
    out.push_str(&format!("last_tick\t{}\n", st.last_tick));
    out.push_str(&format!("last_blocker\t{}\n", st.last_blocker));
    out.push_str(&format!("blocker_streak\t{}\n", st.blocker_streak));
    out.push_str(&format!("red_streak\t{}\n", st.red_streak));
    for (repo, sha) in &st.commits {
        out.push_str(&format!("commit\t{repo}\t{sha}\n"));
    }
    for p in &st.panes {
        // Both timer-bearing states must be written. A `_ => 0` catch-all silently zeroed
        // DIALOG's timer -- caught by the round-trip leg, not by review.
        let timer = match &p.state {
            PaneState::Working { timer_secs } | PaneState::Dialog { timer_secs } => *timer_secs,
            PaneState::Idle | PaneState::Wedged | PaneState::Unproven => 0,
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

/// Panes observed on a previous tick that are ABSENT from the live pane list: the only
/// evidence of death this crate accepts.
///
/// DEAD and DIALOG demand opposite responses -- a dead pane needs respawning, a prompting
/// pane needs an ANSWER -- so conflating them destroys live work. Measured 2026-08-31: a
/// fleet watcher scored `%1413` GONE when it had merely opened an Ask dialog.
///
/// The caller MUST pass a list it actually obtained. An empty list from a FAILED
/// `tmux list-panes` would make this report the entire fleet dead, which is why
/// `pane_ids` returns `Err` on a timeout instead of an empty vector: a timeout is not an
/// empty fleet, and absence of evidence is not evidence of absence.
pub fn vanished(prior: &[Observation], live_ids: &[String]) -> Vec<String> {
    if live_ids.is_empty() {
        // ANTI-VACUITY: refuse to declare a fleet-wide death from a scan that found
        // nothing. Callers get an empty answer, never a mass obituary.
        return Vec::new();
    }
    prior
        .iter()
        .map(|o| o.pane_id.clone())
        .filter(|id| !live_ids.iter().any(|l| l == id))
        .collect()
}

// ---------------------------------------------------------------------------
// repo discovery -- no hardcoded roots, ever
// ---------------------------------------------------------------------------

/// Why a repository set could not be resolved. Every variant names what it looked for,
/// because the historic failure of this lane was worse than an error: a hardcoded root
/// COMPILES after a move and then silently reads the WRONG repo (bead -7ai, -npq).
#[derive(Debug, PartialEq, Eq)]
pub enum RepoError {
    /// An explicit source was set but empty.
    ExplicitEmpty { source: &'static str },
    /// No marker found walking up from `from`.
    NotFound { from: String, markers: &'static str },
}

impl std::fmt::Display for RepoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoError::ExplicitEmpty { source } => {
                write!(
                    f,
                    "{source} is set but empty; refusing to guess a repository"
                )
            }
            RepoError::NotFound { from, markers } => write!(
                f,
                "no repository marker ({markers}) found walking up from {from}; \
                 pass --repo <path> or set {}",
                REPOS_ENV
            ),
        }
    }
}

/// Colon-separated repository list, honoured when no `--repo` flag is given.
pub const REPOS_ENV: &str = "OMP_LIFECYCLE_REPOS";
const MARKERS: [&str; 2] = [".git", ".beads"];

/// Resolve the repository set: explicit flags > env > upward marker walk > typed error.
///
/// Precedence is documented because it is load-bearing (-7ai acceptance 2). There is NO
/// silent cwd fallback: a tool that defaults to "wherever I happen to be" is the wrong-repo
/// defect wearing a different hat.
pub fn resolve_repos(explicit: &[&str]) -> Result<Vec<String>, RepoError> {
    if !explicit.is_empty() {
        if explicit.iter().any(|s| s.trim().is_empty()) {
            return Err(RepoError::ExplicitEmpty { source: "--repo" });
        }
        return Ok(explicit.iter().map(|s| s.to_string()).collect());
    }
    if let Ok(v) = std::env::var(REPOS_ENV) {
        if v.trim().is_empty() {
            return Err(RepoError::ExplicitEmpty { source: REPOS_ENV });
        }
        return Ok(v
            .split(':')
            .filter(|s| !s.trim().is_empty())
            .map(str::to_owned)
            .collect());
    }
    let start = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| ".".to_owned());
    let mut dir = std::path::PathBuf::from(&start);
    loop {
        if MARKERS.iter().any(|m| dir.join(m).exists()) {
            return Ok(vec![dir.display().to_string()]);
        }
        if !dir.pop() {
            return Err(RepoError::NotFound {
                from: start,
                markers: ".git/.beads",
            });
        }
    }
}

#[cfg(test)]
mod repo_tests {
    use super::*;

    #[test]
    fn explicit_flags_win() {
        assert_eq!(
            resolve_repos(&["/a", "/b"]).unwrap(),
            vec!["/a".to_owned(), "/b".to_owned()]
        );
    }

    #[test]
    fn an_empty_explicit_value_is_a_typed_error_not_a_guess() {
        assert_eq!(
            resolve_repos(&[""]),
            Err(RepoError::ExplicitEmpty { source: "--repo" })
        );
    }

    #[test]
    fn the_marker_walk_finds_this_repo_and_names_what_it_sought() {
        // Running under cargo, cwd is inside the repo, so the walk must succeed.
        let got = resolve_repos(&[]).expect("marker walk should find this repo");
        assert_eq!(got.len(), 1);
        let p = std::path::Path::new(&got[0]);
        assert!(
            p.join(".git").exists() || p.join(".beads").exists(),
            "resolved {got:?} carries no marker"
        );
    }

    #[test]
    fn the_error_message_names_the_markers_and_the_escape_hatch() {
        let e = RepoError::NotFound {
            from: "/tmp/nowhere".into(),
            markers: ".git/.beads",
        };
        let s = e.to_string();
        assert!(
            s.contains(".git/.beads"),
            "must name what it looked for: {s}"
        );
        assert!(s.contains("--repo"), "must name the escape hatch: {s}");
        assert!(s.contains(REPOS_ENV), "must name the env override: {s}");
    }
}

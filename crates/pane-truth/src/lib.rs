#![forbid(unsafe_code)]

//! Ground-truth tmux pane state. NTM labels are never consulted.
//! A positive liveness claim requires a prior capture at least 75 seconds old
//! whose rendered content changed; CPU and explicit status markers remain the
//! shell oracle's independent signals.
//!
//! OMP v18 status-line contract (bead omp-orchestrator-pane-truth-omp-v18-blind-lre,
//! measured 2026-08-31): a WORKING pane renders a braille spinner frame followed
//! by a bare elapsed timer on its LAST status line (`⠸ 56m · ◉ GLM 5.3 · …`);
//! IDLE renders the `π` prompt glyph. The detector below is ported from
//! crates/tick-monitor (commit 7b2219f, 41 selftest legs) and keeps its four
//! measured traps: lowercase-unit-only timers (`1.3M` is a token budget,
//! `S0.25` a spend counter — both sit on every live v18 line), LAST-line
//! anchoring (a braille character in scrollback prose is not pane state),
//! spinner-stripped content hashing (a raw-frame hash changes every animation
//! step, so a dead pane reads changing forever), and the persisted prior
//! capture that makes two-capture liveness reachable across invocations.
//!
//! NO-CLAIM: this fixes the STATUS-LINE detector. Single-capture liveness is
//! still not sound — the two-capture rule stands, and `liveness_two_capture:
//! false` means UNPROVEN, never idle. A verdict of IDLE at low CPU with no
//! prior capture remains the weakest cell in the ladder; a consumer must not
//! treat one observation as proof of life or of idleness.

use chrono::{SecondsFormat, Utc};
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const TWO_CAPTURE_MIN_SECS: i64 = 75;
const CHILD_DEADLINE: Duration = Duration::from_secs(15);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneTruthRule {
    TwoCaptureLiveness,
    BusyMarkers,
    AwaitingInput,
    ClassifierIndependence,
}

impl PaneTruthRule {
    pub const ALL: [PaneTruthRule; 4] = [
        PaneTruthRule::TwoCaptureLiveness,
        PaneTruthRule::BusyMarkers,
        PaneTruthRule::AwaitingInput,
        PaneTruthRule::ClassifierIndependence,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            PaneTruthRule::TwoCaptureLiveness => "two_capture_liveness",
            PaneTruthRule::BusyMarkers => "busy_markers",
            PaneTruthRule::AwaitingInput => "awaiting_input",
            PaneTruthRule::ClassifierIndependence => "classifier_independence",
        }
    }
}

#[derive(Clone, Debug)]
pub struct PaneTruthRules {
    pub two_capture_liveness: bool,
    pub busy_markers: bool,
    pub awaiting_input: bool,
    pub classifier_independence: bool,
}

impl Default for PaneTruthRules {
    fn default() -> Self {
        Self {
            two_capture_liveness: true,
            busy_markers: true,
            awaiting_input: true,
            classifier_independence: true,
        }
    }
}

impl PaneTruthRules {
    pub fn disable(&mut self, name: &str) -> bool {
        match name {
            "two_capture_liveness" => self.two_capture_liveness = false,
            "busy_markers" => self.busy_markers = false,
            "awaiting_input" => self.awaiting_input = false,
            "classifier_independence" => self.classifier_independence = false,
            _ => return false,
        }
        true
    }

    pub fn known_names_csv() -> String {
        PaneTruthRule::ALL
            .iter()
            .map(|r| r.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Clone, Debug)]
pub struct ExternalOutput {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

/// Run a child with closed stdin and a hard wall deadline.
pub fn run_external(mut command: Command, deadline: Duration) -> Result<ExternalOutput, String> {
    let temp = std::env::temp_dir().join(format!(
        "pane-truth-child-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&temp).map_err(|e| format!("temp directory failed: {e}"))?;
    let stdout_path = temp.join("stdout");
    let stderr_path = temp.join("stderr");
    let stdout_file = File::create(&stdout_path).map_err(|e| format!("stdout file failed: {e}"))?;
    let stderr_file = File::create(&stderr_path).map_err(|e| format!("stderr file failed: {e}"))?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));
    let mut child = command.spawn().map_err(|e| format!("spawn failed: {e}"))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let result = child_result(status.code(), &stdout_path, &stderr_path, false);
                let _ = fs::remove_dir_all(&temp);
                return Ok(result);
            }
            Ok(None) if started.elapsed() >= deadline => {
                let _ = child.kill();
                reap_bounded(&mut child);
                let result = child_result(None, &stdout_path, &stderr_path, true);
                let _ = fs::remove_dir_all(&temp);
                return Ok(result);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => {
                let _ = child.kill();
                reap_bounded(&mut child);
                let _ = fs::remove_dir_all(&temp);
                return Err(format!("wait failed: {e}"));
            }
        }
    }
}

fn reap_bounded(child: &mut std::process::Child) {
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => return,
        }
    }
}

fn child_result(
    status: Option<i32>,
    stdout_path: &std::path::Path,
    stderr_path: &std::path::Path,
    timed_out: bool,
) -> ExternalOutput {
    ExternalOutput {
        status,
        stdout: fs::read_to_string(stdout_path).unwrap_or_default(),
        stderr: fs::read_to_string(stderr_path).unwrap_or_default(),
        timed_out,
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct PaneRow {
    pub pane_index: u32,
    pub pane_id: String,
    pub pane_pid: u32,
    pub tree_cpu: f64,
    pub cpu_busy: bool,
    pub claims_busy: bool,
    pub claims_done: bool,
    pub awaiting_input: bool,
    pub verdict: String,
    pub confidence: String,
    pub last_line: String,
    pub liveness_two_capture: bool,
}

fn tail_lines(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    if lines.len() <= n {
        text.to_string()
    } else {
        lines[lines.len() - n..].join("\n")
    }
}

// ---------------------------------------------------------------------------
// OMP v18 status-line contract (bead omp-orchestrator-pane-truth-omp-v18-blind-lre).
// Ported from crates/tick-monitor (commit 7b2219f, 41 selftest legs): the
// detector whose absence made every v18 working pane read IDLE. The four
// measured traps are preserved verbatim: lowercase-unit-only timers,
// LAST-line anchoring, spinner-stripped hashing, and the persisted prior
// capture that makes two-capture liveness reachable across invocations.
// ---------------------------------------------------------------------------

/// U+2800..U+28FF — a braille spinner frame.
pub fn is_braille(c: char) -> bool {
    matches!(c as u32, 0x2800..=0x28FF)
}

/// Parse an elapsed timer token (`27s`, `12m`, `1h`) into seconds.
///
/// Requires a LOWERCASE unit so `1.3M` (a token budget) and `S0.25` (a spend
/// counter) cannot be mistaken for elapsed time — both appear on every live
/// v18 status line, so a unit check that accepted them would read an idle
/// pane as working forever.
pub fn parse_timer(line: &str) -> Option<u64> {
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            if i < chars.len() {
                let unit = chars[i];
                let after_ok = i + 1 >= chars.len() || !chars[i + 1].is_alphanumeric();
                if after_ok {
                    let n: u64 = chars[start..i].iter().collect::<String>().parse().ok()?;
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
/// Anchoring on the LAST line is load-bearing, both directions: a
/// whole-buffer scan matches a stale spinner still sitting in scrollback,
/// and a braille character inside quoted prose is not pane state. Decoration
/// lines (the `╰─` codex frame border, box-drawing rules) are skipped so the
/// status line is found behind them.
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

/// v18 WORKING: a braille spinner AND an elapsed timer TOGETHER on the last
/// status line. The `π` idle glyph must not match here.
///
/// THIS FUNCTION IS THE MUTATION TARGET: deleting its call site in
/// `claims_busy` must turn the v18 fixtures RED while the claude-marker
/// fixtures stay GREEN, so the leg names WHICH detector fired (fh C31).
pub fn v18_working(text: &str) -> bool {
    let line = last_status_line(text);
    line.chars().any(is_braille) && parse_timer(line).is_some()
}

// ---------------------------------------------------------------------------
// Verbatim live captures — OMP v18 status lines. Fixture policy for this
// crate: fixtures are NEVER hand-written; every fixture names its capture
// date and the pane and model that produced it. These are `tmux capture-pane`
// bytes from the live omp-orchestrator session.
/// Captured 2026-08-31T08:14Z, pane %1409 (omp-glm, GLM 5.3), mid-turn.
/// Braille ⠸ + bare timer 58m; `1.3M` token budget and `$22.82` spend counter
/// must NOT parse as elapsed time.
const V18_WORKING_GLM_1409: &str = " ⠸ 58m  · ◉ GLM 5.3 · 📁 ~/Developer/omp-orchestrator · ⑂ main *2 ?2 · ◫ 34.2%/1.3M ⟲ · $22.82";

/// Captured 2026-08-31T08:14Z, pane %1413 (omp-codex, GPT-5.6-Luna),
/// mid-turn. The trailing `╰─` decoration line proves the LAST-line anchor:
/// the status line is second-to-last in the buffer.
const V18_WORKING_CODEX_1413: &str = " ⠸ 4m  > ◕ GPT-5.6-Luna > 📁 ~/Developer/omp-orchestrator > ⑂ main *2 ?2 > S0.60 ▶────────24%──────────────────╎┃────1M─\n╰─";

/// Captured 2026-08-31T08:14Z, pane %1414 (omp-codex, GPT-5.6-Luna),
/// mid-turn. Same shape as %1413 with different counters.
const V18_WORKING_CODEX_1414: &str = " ⠼ 4m  > ◕ GPT-5.6-Luna > 📁 ~/Developer/omp-orchestrator > ⑂ main *2 ?2 > S1.39 ▶───────────────44%─────────╎┃────1M─\n╰─";

/// Captured 2026-08-31T08:14Z, pane %1397 (omp-claude, Opus 5), the wave
/// orchestrator between turns. The claude-variant v18 WORKING line: spinner
/// ⠼ + bare timer 1m; `S41.10` spend counter must not parse as time.
const V18_WORKING_CLAUDE_1397: &str =
    " ⠼ 1m  · ◕ Opus 5 · 📁 ~/Developer/omp-orchestrator · ⑂ main *2 ?2 · ◫ 57.1%/1M ⟲ · S41.10";

/// Captured 2026-08-31T08:18:44Z, pane %1409 (omp-glm, GLM 5.3), idle at
/// capture: the `π` prompt glyph, no spinner, no timer. Both-directions leg.
const V18_IDLE_GLM_1409: &str =
    " π  · ◉ GLM 5.3 · 📁 ~/Developer/omp-orchestrator · ⑂ main *4 ?2 · ◫ 34.7%/1.3M ⟲ · $23.66";

/// From the bead record (captured 2026-08-31 07:30Z, omp-glm GLM 5.3 pane):
/// the exact line the blindness was measured against. The bead's own
/// transcription renders the context glyph as ⟫; the live glyph is ◫
/// (verified byte-wise against the file captures above), so this const is
/// byte-faithful to the LIVE payload, not to the transcription.
const V18_WORKING_GLM_BEAD_0730Z: &str =
    " ⠙ 12m  · ◕ GLM 5.3 · 📁 ~/Developer/omp-orchestrator · ⑂ main *1 ?6 · ◫ 13.0%/1.3M ⟲ · $1.30";

/// The Claude Code shape: explicit background/wait markers, anywhere in the
/// rendered buffer. Its own function so the two detectors are individually
/// attributable (fh C31).
fn claude_busy_markers(text: &str) -> bool {
    if text.contains("esc to interrupt")
        || text.contains("Waiting for background terminal")
        || text.contains("ctrl+b ctrl+b")
        || text.contains("to run in background")
    {
        return true;
    }
    static WAIT: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new(r"Waiting for [0-9]+ background agents? to finish").unwrap()
    });
    static GERUND: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"[A-Za-z]+(?:…|\.\.\.)\s*\([0-9]+[hms]").unwrap());
    WAIT.is_match(text) || GERUND.is_match(text)
}
fn claims_busy(text: &str, rules: &PaneTruthRules) -> bool {
    if !rules.busy_markers {
        return false;
    }
    claude_busy_markers(text) || v18_working(text)
}

fn awaiting_input(text: &str, rules: &PaneTruthRules) -> bool {
    if !rules.awaiting_input {
        return false;
    }
    let tail = tail_lines(text, 12);
    static OPTION: OnceLock<Regex> = OnceLock::new();
    let option = OPTION
        .get_or_init(|| Regex::new(r"(?m)^\s*❯?\s*[0-9]+\.\s").unwrap())
        .is_match(&tail);
    static FOOTER: OnceLock<Regex> = OnceLock::new();
    let footer = FOOTER
        .get_or_init(|| Regex::new(r"(?i)enter to (select|confirm)|press enter to confirm|esc to (go back|cancel)|\(y/n\)").unwrap())
        .is_match(&tail);
    option && footer
}

fn content_hash(text: &str) -> u64 {
    // SPINNER TRAP (bead omp-orchestrator-pane-truth-omp-v18-blind-lre, ported
    // from tick-monitor's stable_hash): a hash over the raw frame changes every
    // time the spinner advances, so a dead pane still produces a changing hash
    // and a liveness rule built on it never reports frozen. Braille frames and
    // timer tokens are stripped first; only content real work changes remains.
    // Rows recorded before this change hold raw-frame hashes, so one comparison
    // across the boundary may spuriously read "changed"; that artifact ages out
    // of the ledger after one observation.
    let mut cleaned = String::with_capacity(text.len());
    for ch in text.chars() {
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
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    stripped.hash(&mut hasher);
    hasher.finish()
}

fn timer_seconds(text: &str) -> Option<i64> {
    // Claude shape first: "Working (2s - esc to interrupt)".
    static TIMER: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"\(([0-9]+)([hms])[^)]*\)").unwrap());
    if let Some(captures) = TIMER.captures(text) {
        let value: i64 = captures.get(1)?.as_str().parse().ok()?;
        return Some(match captures.get(2)?.as_str() {
            "h" => value * 3_600,
            "m" => value * 60,
            _ => value,
        });
    }
    // OMP v18 shape: a bare elapsed timer on the LAST status line
    // ("⠸ 56m · ◉ GLM 5.3 · …") — lowercase unit only, last line only.
    parse_timer(last_status_line(text)).map(|secs| secs as i64)
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Two-capture liveness: true only when a prior ledger row exists, is at least
/// TWO_CAPTURE_MIN_SECS old, and something moved. A `false` here means
/// UNPROVEN — not idle, and never safe to dispatch on by itself.
fn liveness_from_history(history: Option<&Value>, text: &str, now: i64, rules: &PaneTruthRules) -> bool {
    if !rules.two_capture_liveness {
        return false;
    }
    let Some(previous) = history else {
        return false;
    };
    let Some(ts) = previous.get("capture_epoch").and_then(Value::as_i64) else {
        return false;
    };
    if now - ts < TWO_CAPTURE_MIN_SECS {
        return false;
    }
    let changed_hash = previous
        .get("content_hash")
        .and_then(Value::as_u64)
        .is_some_and(|hash| hash != content_hash(text));
    let changed_timer = previous
        .get("timer_seconds")
        .and_then(Value::as_i64)
        .zip(timer_seconds(text))
        .is_some_and(|(old, current)| old != current);
    // OR, not AND (bead omp-orchestrator-pane-truth-omp-v18-blind-lre): a lane
    // deep in one long tool call holds a STATIC timer while its output keeps
    // changing, and requiring both to change called live panes dead. Neither
    // moving is the only honest negative.
    changed_hash || changed_timer
}

/// Classify one pane from rendered text, CPU, and the prior capture ledger row.
pub fn classify_snapshot(
    text: &str,
    tree_cpu: f64,
    history: Option<&Value>,
    now: i64,
    rules: &PaneTruthRules,
) -> PaneRow {
    let busy_marker = claims_busy(text, rules);
    let claims_done = text.contains("Worked for") && !busy_marker;
    let awaiting = awaiting_input(text, rules);
    let cpu_busy = tree_cpu
        >= std::env::var("CPU_BUSY_THRESHOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.3f64);
    let liveness = liveness_from_history(history, text, now, rules);
    let classifier_word =
        text.contains("ERROR") || text.contains("THINKING") || text.contains("idle");
    let busy = if rules.classifier_independence && classifier_word {
        busy_marker
    } else {
        busy_marker || classifier_word
    };
    let floor = std::env::var("BUSY_SILENT_FLOOR")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(25.0f64);
    let (verdict, confidence) = if awaiting {
        ("AWAITING_INPUT", "high")
    } else if (cpu_busy || liveness) && busy {
        ("WORKING", "high")
    } else if !cpu_busy && claims_done {
        ("DONE", "high")
    } else if !cpu_busy && !busy {
        ("IDLE", "medium")
    } else if cpu_busy && !busy {
        if tree_cpu >= floor {
            ("BUSY_SILENT", "low")
        } else {
            ("IDLE", "medium")
        }
    } else if tree_cpu < 0.05 {
        ("STALLED_CANDIDATE", "low")
    } else {
        ("WORKING", "medium")
    };
    let last_line = text
        .lines()
        .rfind(|line| line.contains("esc to interrupt") || line.contains("Worked for"))
        .unwrap_or_else(|| last_status_line(text))
        .to_string();
    PaneRow {
        pane_index: 0,
        pane_id: String::new(),
        pane_pid: 0,
        tree_cpu,
        cpu_busy,
        claims_busy: busy,
        claims_done,
        awaiting_input: awaiting,
        verdict: verdict.to_string(),
        confidence: confidence.to_string(),
        last_line,
        liveness_two_capture: liveness,
    }
}

fn tree_cpu(pid: u32) -> Result<f64, String> {
    let mut command = Command::new("ps");
    command.args(["-eo", "pid,ppid,pcpu"]);
    let output = run_external(command, CHILD_DEADLINE)?;
    if output.timed_out || output.status != Some(0) {
        return Err("ps timeout/unreadable".into());
    }
    let mut rows = HashMap::<u32, (u32, f64)>::new();
    for line in output.stdout.lines() {
        let mut fields = line.split_whitespace();
        let Some(id) = fields.next().and_then(|v| v.parse().ok()) else {
            continue;
        };
        let Some(parent) = fields.next().and_then(|v| v.parse().ok()) else {
            continue;
        };
        let Some(cpu) = fields.next().and_then(|v| v.parse().ok()) else {
            continue;
        };
        rows.insert(id, (parent, cpu));
    }
    let mut descendants = HashSet::from([pid]);
    let mut changed = true;
    while changed {
        changed = false;
        for (&child, &(parent, _)) in &rows {
            if !descendants.contains(&child) && descendants.contains(&parent) {
                descendants.insert(child);
                changed = true;
            }
        }
    }
    Ok(descendants
        .iter()
        .filter(|id| **id != pid)
        .filter_map(|id| rows.get(id).map(|(_, cpu)| *cpu))
        .sum())
}

fn capture(session: &str, index: u32) -> Result<String, String> {
    let target = format!("{session}:0.{index}");
    let mut command = Command::new("tmux");
    command.args(["capture-pane", "-p", "-t", &target]);
    let output = run_external(command, CHILD_DEADLINE)?;
    if output.timed_out || output.status != Some(0) {
        return Err("tmux capture-pane timeout/unreadable".into());
    }
    Ok(output.stdout.replace('\r', ""))
}

fn history_path() -> PathBuf {
    std::env::var_os("PANE_TRUTH_HISTORY")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join(".local/state/flywheel/pane-truth.jsonl")
        })
}

fn previous_rows(path: &PathBuf) -> Vec<Value> {
    fs::read_to_string(path)
        .ok()
        .map(|text| {
            text.lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect()
        })
        .unwrap_or_default()
}

fn append_history(path: &PathBuf, row: &PaneRow, text: &str, epoch: i64) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(
            file,
            "{}",
            json!({
                "capture_epoch": epoch,
                "pane_index": row.pane_index,
                "pane_pid": row.pane_pid,
                "content_hash": content_hash(text),
                "timer_seconds": timer_seconds(text)
            })
        );
    }
}

pub fn run_live(session: &str, rules: &PaneTruthRules) -> i32 {
    let mut command = Command::new("tmux");
    command.args([
        "list-panes",
        "-t",
        session,
        "-F",
        "#{pane_index}|#{pane_pid}|#{pane_id}",
    ]);
    let list = run_external(command, CHILD_DEADLINE);
    let Ok(list) = list else {
        // L4 (bead omp-orchestrator-pane-truth-omp-v18-blind-lre): a scan that
        // enumerated NOTHING is an ERROR, never a pass — a deliverable never
        // checked reports identically to one that passed. Nonzero exit is the
        // error channel; consumers that only parse the JSON see the error row.
        println!("{}", json!({"error":"tmux_unreadable","session":session}));
        return 4;
    };
    if list.timed_out {
        println!("{}", json!({"error":"tmux_unreadable","session":session}));
        return 4;
    }
    if list.status != Some(0) {
        println!("{}", json!({"error":"no_session","session":session}));
        return 4;
    }
    let history_file = history_path();
    let previous = previous_rows(&history_file);
    let epoch = now_epoch();
    let mut panes = Vec::new();
    for line in list.stdout.lines() {
        let fields: Vec<&str> = line.split('|').collect();
        if fields.len() != 3 {
            continue;
        }
        let Some(index) = fields[0].parse().ok() else {
            continue;
        };
        let Some(pid) = fields[1].parse().ok() else {
            continue;
        };
        let text = capture(session, index).unwrap_or_default();
        let cpu = tree_cpu(pid).unwrap_or(0.0);
        let prior = previous.iter().rev().find(|row| {
            row.get("pane_index").and_then(Value::as_u64) == Some(index as u64)
                && row.get("pane_pid").and_then(Value::as_u64) == Some(pid as u64)
        });
        let mut row = classify_snapshot(&text, cpu, prior, epoch, rules);
        row.pane_index = index;
        row.pane_id = fields[2].to_string();
        row.pane_pid = pid;
        append_history(&history_file, &row, &text, epoch);
        panes.push(row);
    }
    if panes.is_empty() {
        // L4: an empty pane set is an ERROR, never a pass, and never a
        // schema-valid "panes":[] that a consumer could read as "all clear".
        println!(
            "{}",
            json!({"error":"empty_pane_set","session":session,"captured_at":now_iso()})
        );
        return 4;
    }
    println!(
        "{}",
        json!({
            "schema":"pane-truth.v1",
            "session":session,
            "captured_at":now_iso(),
            "cpu_busy_threshold":std::env::var("CPU_BUSY_THRESHOLD").ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.3),
            "panes":panes
        })
    );
    0
}

pub fn selftest(rules: &PaneTruthRules) -> i32 {
    let text = "claude\nWorking (2s - esc to interrupt)\n❯ ";
    let now = 10_000;
    let prior = json!({
        "capture_epoch": now - 75,
        "content_hash": content_hash("claude\nWorking (1s - esc to interrupt)\n❯ "),
        "timer_seconds": 1
    });
    let baseline = classify_snapshot(text, 0.0, Some(&prior), now, rules);
    let mut no_motion = rules.clone();
    no_motion.two_capture_liveness = false;
    let mutated = classify_snapshot(text, 0.0, Some(&prior), now, &no_motion);
    if baseline.verdict == "WORKING" && mutated.verdict != "WORKING" {
        println!("MUTATION RED two_capture_liveness: deleting the >=75s prior-capture guard turns changed-pane WORKING into {}", mutated.verdict);
    } else {
        println!(
            "SELFTEST FAIL two_capture_liveness baseline={} mutated={}",
            baseline.verdict, mutated.verdict
        );
        return 1;
    }
    let mut no_busy = rules.clone();
    no_busy.busy_markers = false;
    let busy_mutation = classify_snapshot(text, 0.0, Some(&prior), now, &no_busy);
    if busy_mutation.verdict != baseline.verdict {
        println!(
            "MUTATION RED busy_markers: deleting rendered-work markers changes the verdict to {}",
            busy_mutation.verdict
        );
    } else {
        println!("SELFTEST FAIL busy_markers mutation did not change verdict");
        return 1;
    }
    let prompt = "claude\n  1. Continue\nEnter to select\n❯ ";
    let awaiting = classify_snapshot(prompt, 0.0, None, now, rules);
    let mut no_prompt = rules.clone();
    no_prompt.awaiting_input = false;
    let awaiting_mutation = classify_snapshot(prompt, 0.0, None, now, &no_prompt);
    if awaiting.verdict == "AWAITING_INPUT" && awaiting_mutation.verdict != "AWAITING_INPUT" {
        println!("MUTATION RED awaiting_input: deleting the selection guard changes AWAITING_INPUT to {}", awaiting_mutation.verdict);
    } else {
        println!("SELFTEST FAIL awaiting_input mutation did not change verdict");
        return 1;
    }
    // ── OMP v18 legs (bead omp-orchestrator-pane-truth-omp-v18-blind-lre) ──
    // The claude fixtures above are the CONTROL: the busy_markers rule-flip
    // must keep flipping them. The v18 fixtures below are the payload this
    // tool actually runs against — a leg that only ever passed on claude-shaped
    // text was the vacuity (fh C38) this bead exists to end.
    let v18_base = classify_snapshot(V18_WORKING_GLM_1409, 16.5, None, now, rules);
    if v18_base.claims_busy && v18_base.verdict == "WORKING" {
        println!("v18 known-good: GLM working line (⠸ 58m) claims_busy=true verdict=WORKING");
    } else {
        println!(
            "SELFTEST FAIL v18 working line: claims_busy={} verdict={}",
            v18_base.claims_busy, v18_base.verdict
        );
        return 1;
    }
    // Acceptance 1 names THIS line first: the verbatim 07:30Z capture the bead
    // measured the blindness against.
    if claims_busy(V18_WORKING_GLM_BEAD_0730Z, rules) {
        println!("v18 known-good: the bead's 07:30Z line (⠙ 12m) claims_busy=true");
    } else {
        println!("SELFTEST FAIL v18 bead line: the 07:30Z capture must claim busy");
        return 1;
    }
    let mut no_busy_v18 = rules.clone();
    no_busy_v18.busy_markers = false;
    let v18_flip = classify_snapshot(V18_WORKING_GLM_1409, 16.5, None, now, &no_busy_v18);
    if !v18_flip.claims_busy && v18_flip.verdict != "WORKING" {
        println!("MUTATION RED busy_markers (v18 payload): the rule-flip turns the v18 working line {} — the defect, reproduced on the real payload", v18_flip.verdict);
    } else {
        println!("SELFTEST FAIL v18 busy_markers rule-flip did not change the v18 verdict");
        return 1;
    }
    let v18_idle = classify_snapshot(V18_IDLE_GLM_1409, 0.0, None, now, rules);
    if !v18_idle.claims_busy && v18_idle.verdict == "IDLE" {
        println!("v18 both-directions: the π idle line reads IDLE, not busy");
    } else {
        println!(
            "SELFTEST FAIL v18 idle line: claims_busy={} verdict={}",
            v18_idle.claims_busy, v18_idle.verdict
        );
        return 1;
    }
    println!("SELFTEST PASS pane-truth fixtures=6 (two-capture, rendered markers, input prompt, v18 working, v18 busy-flip, v18 idle)");
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_capture_requires_seventy_five_seconds() {
        let prior = json!({"capture_epoch": 900, "content_hash": content_hash("claude\nWorking (0s - esc to interrupt)"), "timer_seconds": 0});
        let row = classify_snapshot(
            "claude\nWorking (1s - esc to interrupt)",
            0.0,
            Some(&prior),
            974,
            &PaneTruthRules::default(),
        );
        assert_eq!(
            row.verdict, "STALLED_CANDIDATE",
            "rule two_capture_liveness: a 74s-old capture cannot prove motion"
        );
        let row = classify_snapshot(
            "claude\nWorking (1s - esc to interrupt)",
            0.0,
            Some(&prior),
            975,
            &PaneTruthRules::default(),
        );
        assert_eq!(
            row.verdict, "WORKING",
            "rule two_capture_liveness: changed content after 75s proves motion"
        );
    }

    #[test]
    fn classifier_words_are_not_truth() {
        let row = classify_snapshot(
            "claude\nERROR idle THINKING\n❯ ",
            0.0,
            None,
            100,
            &PaneTruthRules::default(),
        );
        assert_eq!(
            row.verdict, "IDLE",
            "rule classifier_independence: NTM words in rendered prose do not decide"
        );
    }

    #[test]
    fn bounded_child_has_deadline() {
        let started = Instant::now();
        let mut command = Command::new("sleep");
        command.arg("30");
        let out = run_external(command, Duration::from_millis(200)).unwrap();
        assert!(
            out.timed_out && started.elapsed() < Duration::from_secs(3),
            "rule bounded_waits: sleep was not bounded"
        );
    }

    // ── OMP v18 fixture legs (bead omp-orchestrator-pane-truth-omp-v18-blind-lre) ──
    // Each leg names its detector so a source-level deletion of the v18 branch
    // turns exactly these RED while the claude legs stay GREEN (fh C31).

    #[test]
    fn v18_glm_working_lines_claim_busy_via_status_line() {
        assert!(
            claims_busy(V18_WORKING_GLM_1409, &PaneTruthRules::default()),
            "v18 detector: the GLM working capture (⠸ 58m) must claim busy"
        );
        assert!(
            claims_busy(V18_WORKING_GLM_BEAD_0730Z, &PaneTruthRules::default()),
            "v18 detector: the bead's recorded 07:30Z GLM line (⠙ 12m) must claim busy"
        );
    }

    #[test]
    fn v18_codex_working_line_claims_busy_behind_decoration() {
        assert!(
            claims_busy(V18_WORKING_CODEX_1413, &PaneTruthRules::default()),
            "v18 detector: the codex status line sits behind the ╰─ border; the anchor must reach it"
        );
        assert!(claims_busy(V18_WORKING_CODEX_1414, &PaneTruthRules::default()));
    }

    #[test]
    fn v18_claude_working_line_claims_busy_via_status_line() {
        assert!(
            claims_busy(V18_WORKING_CLAUDE_1397, &PaneTruthRules::default()),
            "v18 detector: the Opus working capture (⠼ 1m) must claim busy"
        );
    }

    #[test]
    fn claude_marker_fixture_still_claims_busy() {
        // THE CONTROL LEG: deleting the v18 branch must leave this GREEN.
        assert!(
            claims_busy(
                "claude\nWorking (2s - esc to interrupt)\n❯ ",
                &PaneTruthRules::default()
            ),
            "claude markers: the Claude Code shape keeps its own detector"
        );
    }

    #[test]
    fn v18_idle_pi_line_is_not_busy() {
        let rules = PaneTruthRules::default();
        assert!(
            !claims_busy(V18_IDLE_GLM_1409, &rules),
            "v18 detector: the π idle glyph must not claim busy"
        );
        let row = classify_snapshot(V18_IDLE_GLM_1409, 0.0, None, 100, &rules);
        assert_eq!(
            row.verdict, "IDLE",
            "both directions: the idle capture reads IDLE"
        );
    }

    #[test]
    fn v18_braille_in_scrollback_prose_is_not_pane_state() {
        // Trap 2: a braille character inside quoted prose is not pane state.
        // The LAST line here is the π idle prompt; working-shaped prose is above.
        let capture = "agent quoted: \"the pane showed ⠸ 12m mid-run\"\n π  · ◕ Opus 5 · S41.10";
        assert!(
            !claims_busy(capture, &PaneTruthRules::default()),
            "last-line anchor: scrollback braille must not read as a working spinner"
        );
    }

    #[test]
    fn v18_counters_are_not_timers() {
        // Trap 1: `1.3M` is a token budget, `S0.25` a spend counter — both sit
        // on every live v18 line. Neither may parse as elapsed time.
        assert_eq!(parse_timer("1.3M"), None, "token budget is not a timer");
        assert_eq!(parse_timer("S0.25"), None, "spend counter is not a timer");
        assert_eq!(
            parse_timer(" ⠸ 56m  · ⟫ 34.2%/1.3M ⟲ · $22.82"),
            Some(56 * 60),
            "the bare elapsed timer is the FIRST lowercase-unit token on the line"
        );
        let no_timer_line = " ⠸ spinning · 1.3M · S0.25";
        assert!(
            !claims_busy(no_timer_line, &PaneTruthRules::default()),
            "trap 1: spinner without a real timer must not claim busy"
        );
    }

    #[test]
    fn decoration_line_is_not_the_status_line() {
        let line = last_status_line(V18_WORKING_CODEX_1413);
        assert!(
            line.chars().any(is_braille) && parse_timer(line).is_some(),
            "the ╰─ border must be skipped so the status line is found"
        );
    }

    #[test]
    fn stable_hash_ignores_spinner_frames_and_timer_tokens() {
        // Trap 3: a hash over the raw frame changes with every spinner step, so
        // a dead pane reads changing forever. Frames and timer tokens strip out.
        let a = " ⠸ 12m · ◕ GLM 5.3 · some output line";
        let b = " ⠼ 12m · ◕ GLM 5.3 · some output line";
        assert_eq!(
            content_hash(a),
            content_hash(b),
            "trap 3: spinner frames must not move the content hash"
        );
        let d = " ⠼ 13m · ◕ GLM 5.3 · some output line";
        assert_eq!(
            content_hash(a),
            content_hash(d),
            "trap 3: timer tokens are animated state; the timer channel carries them"
        );
        let c = " ⠼ 12m · ◕ GLM 5.3 · NEW output appeared";
        assert_ne!(
            content_hash(a),
            content_hash(c),
            "trap 3: real content changes must still move the hash"
        );
    }

    #[test]
    fn v18_two_capture_liveness_is_reachable() {
        // Acceptance 5: with a persisted prior row ≥75s old, motion is provable
        // on the v18 payload — the field is no longer always-false.
        let now = 100_000;
        let prior = json!({
            "capture_epoch": now - 75,
            "content_hash": content_hash(V18_WORKING_GLM_BEAD_0730Z),
            "timer_seconds": timer_seconds(V18_WORKING_GLM_BEAD_0730Z),
        });
        let row = classify_snapshot(
            V18_WORKING_GLM_1409,
            16.5,
            Some(&prior),
            now,
            &PaneTruthRules::default(),
        );
        assert!(
            row.liveness_two_capture,
            "timer advanced (12m → 58m) across a 75s gap: v18 liveness is reachable"
        );
        assert_eq!(row.verdict, "WORKING");

        // OR semantics: a lane deep in one long tool call holds a STATIC timer
        // while content moves — that pane is live.
        let static_timer_prior = json!({
            "capture_epoch": now - 75,
            "content_hash": content_hash(V18_WORKING_GLM_BEAD_0730Z),
            "timer_seconds": timer_seconds(V18_WORKING_GLM_1409),
        });
        let moved = classify_snapshot(
            V18_WORKING_GLM_1409,
            0.0,
            Some(&static_timer_prior),
            now,
            &PaneTruthRules::default(),
        );
        assert!(
            moved.liveness_two_capture,
            "OR semantics: content moved while the timer held"
        );

        // Neither channel moves: not proven live, never idle-by-default.
        let frozen_prior = json!({
            "capture_epoch": now - 75,
            "content_hash": content_hash(V18_WORKING_GLM_1409),
            "timer_seconds": timer_seconds(V18_WORKING_GLM_1409),
        });
        let frozen = classify_snapshot(
            V18_WORKING_GLM_1409,
            0.0,
            Some(&frozen_prior),
            now,
            &PaneTruthRules::default(),
        );
        assert!(
            !frozen.liveness_two_capture,
            "nothing moved across the gap: UNPROVEN, not live"
        );
    }

    #[test]
    fn v18_working_captures_agree_with_spinner_timer_ground_truth() {
        // Acceptance 3 at fixture level: every live working capture reads
        // WORKING at the measured tree_cpu (16.5 = %1409 at 07:30Z). The
        // live-session 6-of-6 run is recorded in the bead.
        let rules = PaneTruthRules::default();
        for capture in [
            V18_WORKING_GLM_1409,
            V18_WORKING_GLM_BEAD_0730Z,
            V18_WORKING_CODEX_1413,
            V18_WORKING_CODEX_1414,
            V18_WORKING_CLAUDE_1397,
        ] {
            let row = classify_snapshot(capture, 16.5, None, 100_000, &rules);
            assert_eq!(
                row.verdict, "WORKING",
                "v18 working capture must read WORKING, got {} for: {capture}",
                row.verdict
            );
        }
    }
}

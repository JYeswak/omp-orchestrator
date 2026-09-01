#![forbid(unsafe_code)]
//! THE OBSERVE LANE — fleet-wide attention wait, idle/ready scan, and the ONLY SCHEDULED WRITER
//! of the standing admission verdict.
//!
//! WHY THIS ROW MATTERS. When this lane stops, every dispatch lane refuses on a stale verdict and
//! the whole fleet idles. Measured 2026-08-26: it stopped firing at 14:43 (its cron slots starved
//! by twelve stacked `loop-driver.sh` instances), the standing verdict froze at 08:23 FAIL, and
//! `fast-dispatch` refused 147 of 191 ticks (77%) on `no_fresh_standing_pass`. The monitor is not
//! a report; it is the heartbeat that keeps admission young.
//!
//! ── WHAT THIS PORT CHANGES, AND WHAT IT DELIBERATELY DOES NOT ──────────────────────────────
//!
//! IT DOES NOT CHANGE WHICH PANES ARE CONSIDERED IDLE. Mis-classifying a busy pane as idle causes
//! double-dispatch to a working agent, which is worse than the wedge this fixes. `safe_to_dispatch`
//! selection and the `pane_liveness` LIVE/BUSY/WEDGED/UNPROVEN classifier are ported token-for-token
//! from the shell, and the differential harness grades that classifier against the original.
//!
//! IT DOES NOT CHANGE THE PUBLISH-INVOCATION CONTRACT. `check.sh --publish` owns the
//! private-run + complete-candidate + atomic-promotion contract; this lane only invokes it with
//! the same five environment variables and records the rc. In particular:
//!
//!   ⛔ THERE IS NO OUTER TIMEOUT ON THE PUBLISHER CALL, AND THAT IS DELIBERATE.
//!   A fixed 600s outer timeout was tried and REMOVED because it contradicted check.sh's
//!   load-scaled per-stage bound (up to 1800s): the publisher killed a healthy in-budget
//!   close-evidence run before it could publish anything. The publisher's own
//!   CHECK_SH_PUBLISH_DEADLINE_SECONDS is the bound, and it produces a COMPLETE FAIL row naming
//!   the timed-out and skipped gates. An outer kill produces no row at all. Reinstating an outer
//!   timeout here would re-break the lane in the exact way already measured and fixed.
//!
//! The run-wide deadline (see `RunDeadline`) is NOT that timeout: it bounds the whole tick so a
//! hung run cannot silence the lane forever, and it is checked BETWEEN lanes, never as a kill
//! wrapped around the publisher.
//!
//! ── THE VERDICT CONTRACT (fh G1, frankengraphdb `gate_verdict.sh`) ─────────────────────────
//! Every verdict line goes to STDOUT at column 0 in BOTH directions; stderr carries usage errors
//! only. Measured in this repo: `gate-catalog-check --catalog /nonexistent.toml 2>/dev/null`
//! prints NOTHING and exits 1 — a RED invisible to `gate > log` (bead cp-ehcor). This crate does
//! not reproduce that shape.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub mod lock;

pub use lock::{HolderLookup, LockOutcome, RunLock};

/// Provenance of the invocation, proven by VERIFIED PROCESS LINEAGE, never by a string.
///
/// SCHEDULED requires an ancestor that is (a) named `/usr/sbin/cron`, (b) uid 0, and (c) a direct
/// child of launchd (ppid 1). The argv/command string is NEVER matched: on 2026-08-22 a
/// cron-shaped argv[0] was FORGED from a detached shell in this very system. uid is kernel truth;
/// a non-root forger can fake the name but never the uid+ppid pair.
///
/// FAIL DIRECTION: unreadable or missing ancestry => MANUAL/unproven, never SCHEDULED.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FleetMonitorInvoker {
    pub invoker: &'static str,
    pub proof: &'static str,
}

impl FleetMonitorInvoker {
    pub const MANUAL: FleetMonitorInvoker = FleetMonitorInvoker { invoker: "MANUAL", proof: "unproven" };
    pub const SCHEDULED: FleetMonitorInvoker = FleetMonitorInvoker { invoker: "SCHEDULED", proof: "cron_parent" };
}

/// One ancestor row: `uid ppid comm`, nearest ancestor first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetMonitorAncestorRow {
    pub uid: u32,
    pub ppid: u32,
    pub comm: String,
}

/// PURE lineage classifier — the shell's `invoker_from_chain`, kept pure for the same reason:
/// both directions prove hermetically without spawning cron.
pub fn invoker_from_chain(chain: &[FleetMonitorAncestorRow]) -> FleetMonitorInvoker {
    for row in chain {
        if row.uid == 0 && row.ppid == 1 && row.comm == "/usr/sbin/cron" {
            return FleetMonitorInvoker::SCHEDULED;
        }
    }
    FleetMonitorInvoker::MANUAL
}

/// Parse `ps -o uid=,ppid=,comm=` output into ancestor rows. Unparseable rows are DROPPED rather
/// than guessed at: a row we cannot read must never become a SCHEDULED claim.
pub fn parse_ancestor_rows(text: &str) -> Vec<FleetMonitorAncestorRow> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let (Some(uid), Some(ppid), Some(comm)) = (it.next(), it.next(), it.next()) else {
            continue;
        };
        let (Ok(uid), Ok(ppid)) = (uid.parse::<u32>(), ppid.parse::<u32>()) else {
            continue;
        };
        out.push(FleetMonitorAncestorRow { uid, ppid, comm: comm.to_string() });
    }
    out
}

/// FleetMonitorLiveness of a candidate pane, decided from its captured text.
///
/// ⛔ THIS IS THE MOST SAFETY-CRITICAL CLASSIFIER IN THE CRATE, AND IT IS PORTED TOKEN-FOR-TOKEN.
/// Mis-classifying a busy pane as idle causes double-dispatch to a working agent. Note that this
/// function does NOT decide "is the agent busy" on its own — it runs only over panes ntm already
/// reported `safe_to_dispatch`, and it asks a narrower question: can we PROVE this pane is a live
/// agent at a prompt? Hence `Working (` and `Worked for` are LIVE (footers of an agent that is
/// present), while BUSY is an explicit OMP in-flight marker that must never become idle capacity.
/// The WEDGED markers are the two states where a pane is present but cannot accept work: an
/// unsubmitted queued message, and a pending restart.
///
/// THE DIALECTS ARE LOAD-BEARING. Codex panes never print `Ready`/`Working (`; an idle one shows
/// a past-tense rule `- Worked for 6m 00s -`. OMP panes expose a prompt footer (`╰─` or `❯`) and
/// a working marker (`⟨esc⟩` or a braille spinner). Measured 2026-08-21: 439 UNPROVEN
/// classifications and 3 of 5 live zeststream-cast panes misread, so the monitor reported "no
/// idle-pane/ready-work pairs" while two Codex workers sat idle beside 109 dispatchable beads.
/// Measured 2026-08-30: NTM reported the live OMP panes as ERROR/UNKNOWN while their fresh tails
/// showed the OMP prompt; without these markers both usable panes were excluded. UNPROVEN
/// excludes a pane from idle capacity, so a dialect the detector does not speak reads exactly
/// like a dead pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetMonitorLiveness {
    pub state: LivenessState,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LivenessState {
    Live,
    Busy,
    Wedged,
    Unproven,
}

impl LivenessState {
    pub fn as_str(self) -> &'static str {
        match self {
            LivenessState::Live => "LIVE",
            LivenessState::Busy => "BUSY",
            LivenessState::Wedged => "WEDGED",
            LivenessState::Unproven => "UNPROVEN",
        }
    }
}

/// Classify pane text. Mirrors `pane_liveness()` in `bin/fleet-monitor.sh` branch for branch,
/// including the ORDER of the branches — an earlier branch wins, exactly as the shell's `elif`
/// chain does.
pub fn pane_liveness(text: &str) -> FleetMonitorLiveness {
    if text.is_empty() {
        return FleetMonitorLiveness { state: LivenessState::Unproven, reason: "empty_capture" };
    }
    // WEDGED FIRST: a pane present but unable to accept work.
    if text.contains("Press up to edit queued messages") {
        return FleetMonitorLiveness { state: LivenessState::Wedged, reason: "queued_unsubmitted" };
    }
    if text.contains("Restart to update") {
        return FleetMonitorLiveness { state: LivenessState::Wedged, reason: "restart_required" };
    }
    // OMP leaves an old prompt footer in the capture while work is in flight. Its spinner,
    // elapsed-status row, interrupt hint, or active status line therefore MUST outrank every
    // prompt/footer marker below; otherwise the monitor invents idle capacity beside live work.
    if omp_busy_re().is_match(text) {
        return FleetMonitorLiveness { state: LivenessState::Busy, reason: "omp_working_marker" };
    }
    // `grep -Eiq '(^|[[:space:]│])Ready([[:space:]│]|$)'` — case-insensitive, bounded by
    // whitespace or the box-drawing glyph the TUIs frame their footer with.
    if ready_footer_re().is_match(text) {
        return FleetMonitorLiveness { state: LivenessState::Live, reason: "ready_footer" };
    }
    if working_footer_re().is_match(text) {
        return FleetMonitorLiveness { state: LivenessState::Live, reason: "working_footer" };
    }
    if codex_worked_re().is_match(text) {
        return FleetMonitorLiveness { state: LivenessState::Live, reason: "codex_worked_footer" };
    }
    if omp_prompt_re().is_match(text) {
        return FleetMonitorLiveness { state: LivenessState::Live, reason: "omp_prompt_footer" };
    }
    if text.contains("Esc:cancel") {
        return FleetMonitorLiveness { state: LivenessState::Live, reason: "grok_prompt_footer" };
    }
    FleetMonitorLiveness { state: LivenessState::Unproven, reason: "no_ready_or_working_marker" }
}

fn ready_footer_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)(^|[\s│])Ready([\s│]|$)").expect("static regex"))
}

fn working_footer_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)Working \(").expect("static regex"))
}

fn codex_worked_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)Worked for [0-9]+").expect("static regex"))
}

fn omp_busy_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"(?m)(⟨esc⟩|^\s*[⠋⠙⠹⠸⠼⠦⠧⠇⠏]\s+(?:Working(?:\s|$)|[0-9]+[smhd](?:\s|$))|^\s*⎋\s+\S)",
        )
            .expect("static regex")
    })
}

fn omp_prompt_re() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?m)^\s*(?:╰─|❯)\s*$").expect("static regex"))
}

/// Extract `safe_to_dispatch` pane indices from an `ntm --robot-activity` payload.
///
/// Accepts both shapes the shell accepted (`panes` or `agents`, `pane` or `pane_index`) because
/// ntm has shipped both and a monitor that understands only one silently reports an empty fleet.
pub fn safe_panes(activity_json: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(activity_json) else {
        return Vec::new();
    };
    let list = v
        .get("panes")
        .or_else(|| v.get("agents"))
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for p in list {
        if p.get("safe_to_dispatch").and_then(|b| b.as_bool()) != Some(true) {
            continue;
        }
        let pane = p.get("pane").filter(|x| !x.is_null()).or_else(|| p.get("pane_index"));
        match pane {
            Some(serde_json::Value::Number(n)) => out.push(n.to_string()),
            Some(serde_json::Value::String(s)) => out.push(s.clone()),
            _ => {}
        }
    }
    out
}

/// `end_cursor` from an attention payload, or empty when absent/unparseable.
pub fn attention_end_cursor(att: &str) -> String {
    serde_json::from_str::<serde_json::Value>(att)
        .ok()
        .and_then(|d| {
            d.get("cursor_info")
                .and_then(|c| c.get("end_cursor"))
                .and_then(|c| c.as_str().map(str::to_string))
        })
        .unwrap_or_default()
}

/// `wake_reason` from an attention payload, defaulting to `none` exactly as the shell did.
pub fn attention_wake_reason(att: &str) -> String {
    serde_json::from_str::<serde_json::Value>(att)
        .ok()
        .and_then(|d| d.get("wake_reason").and_then(|w| w.as_str().map(str::to_string)))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "none".to_string())
}

/// Count of rows with `status == "open"` in a `br ready --json` payload (the RAW count, before the
/// policy filter). Unparseable input counts ZERO, matching the shell's `except: print(0)`.
pub fn raw_open_count(br_json: &str) -> u64 {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(br_json) else {
        return 0;
    };
    let rows = if let Some(a) = v.as_array() {
        a.clone()
    } else {
        v.get("issues").and_then(|x| x.as_array()).cloned().unwrap_or_default()
    };
    rows.iter().filter(|r| r.get("status").and_then(|s| s.as_str()) == Some("open")).count() as u64
}

/// JSON string escaping for ledger rows (the shell's `topology_json_escape`).
pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

/// A whole-run wall-clock deadline.
///
/// WHY A RUN DEADLINE EXISTS AT ALL (requirement C). On 2026-08-26 a wedged instance from 14:03
/// held the run lock for 2h21m at 0.0% CPU, and every later firing skipped behind it. A lock alone
/// converts one hung run into an indefinitely silent lane. The deadline is checked BETWEEN lanes
/// so a hung run releases the lock and the next cron slot re-observes fresher state.
///
/// It is NOT a kill wrapped around the publisher — see the module docs.
#[derive(Debug, Clone)]
pub struct RunDeadline {
    start: Instant,
    budget: Duration,
}

impl RunDeadline {
    pub fn new(budget: Duration) -> Self {
        RunDeadline { start: Instant::now(), budget }
    }
    pub fn expired(&self) -> bool {
        self.start.elapsed() >= self.budget
    }
    pub fn remaining(&self) -> Duration {
        self.budget.saturating_sub(self.start.elapsed())
    }
}

/// Repos discovered from `ntm list`, filtered to those that exist under `developer_root`.
///
/// DISCOVERY IS LIVE, NOT A HARDCODED LIST — a monitor whose blind spot grows as the fleet grows
/// is worse than none.
pub fn discover_repos(ntm_list_output: &str, developer_root: &Path) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for line in ntm_list_output.lines() {
        let t = line.trim();
        let Some((name, _)) = t.split_once(':') else { continue };
        let name = name.trim();
        if name.is_empty()
            || !name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        {
            continue;
        }
        if !developer_root.join(name).is_dir() {
            continue;
        }
        if seen.insert(name.to_string()) {
            out.push(name.to_string());
        }
    }
    out
}

/// `ntm list` exits 0 while printing this when its projection is empty (measured:
/// `bin/fleet-monitor.sh --selftest` C31). Exit-code-only checks are a dead detector.
pub const NTM_LIST_EMPTY_BANNER: &str = "No tmux sessions";

/// True when the listing is an empty scan set: blank, or the ntm empty-fleet banner.
/// Distinct from "sessions listed, none currently idle" — that is a drained fleet.
pub fn ntm_list_is_empty(text: &str) -> bool {
    let t = text.trim();
    t.is_empty() || t.to_ascii_lowercase().contains(&NTM_LIST_EMPTY_BANNER.to_ascii_lowercase())
}

/// Shell-identical census line so the differential grades both sides over the same bytes.
pub fn ntm_list_census_line(text: &str) -> String {
    if ntm_list_is_empty(text) {
        "CANNOT_OBSERVE|empty_ntm_list".to_string()
    } else {
        "OK|listed".to_string()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ObserveRules {
    /// When false, an empty ntm list is treated as a drained fleet (mutation target).
    pub empty_scan: bool,
}

impl Default for ObserveRules {
    fn default() -> Self {
        Self { empty_scan: true }
    }
}

impl ObserveRules {
    pub fn from_env() -> Self {
        Self {
            empty_scan: std::env::var_os("FLEET_MONITOR_DISABLE_EMPTY_SCAN").is_none(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserveScan {
    CannotObserve { reason: &'static str },
    Repos(Vec<String>),
}

/// Live observe census. An empty `ntm list` is CANNOT_OBSERVE, never `fleet_clear`.
pub fn observe_scan_set(ntm_list_output: &str, developer_root: &Path, rules: ObserveRules) -> ObserveScan {
    if ntm_list_is_empty(ntm_list_output) {
        if !rules.empty_scan {
            return ObserveScan::Repos(Vec::new());
        }
        return ObserveScan::CannotObserve {
            reason: "empty_ntm_list",
        };
    }
    ObserveScan::Repos(discover_repos(ntm_list_output, developer_root))
}

pub const EXIT_CANNOT_OBSERVE: i32 = 78;

/// The environment handed to `check.sh --publish`. Held as data so the differential can assert the
/// Rust and shell callers build the SAME invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishInvocation {
    pub bin: PathBuf,
    pub env: Vec<(String, String)>,
    pub args: Vec<String>,
}

/// Build the publisher invocation. THE CONTRACT IS FIXED — five environment variables, the
/// `--publish` flag, and NO outer timeout.
pub fn publish_invocation(
    check_sh_bin: &Path,
    state_dir: &Path,
    ledger: &Path,
    fresh_seconds: u64,
    deadline_seconds: u64,
) -> PublishInvocation {
    let live = state_dir.join("check-sh-ledger.json");
    let private = state_dir.join("check-sh-ledger.fleet-monitor.json");
    PublishInvocation {
        bin: check_sh_bin.to_path_buf(),
        env: vec![
            ("CHECK_SH_LEDGER".into(), private.display().to_string()),
            ("CHECK_SH_PUBLISH_LEDGER".into(), live.display().to_string()),
            ("CHECK_SH_PUBLISH_EVENT_LEDGER".into(), ledger.display().to_string()),
            ("CHECK_SH_PUBLISH_FRESH_SECONDS".into(), fresh_seconds.to_string()),
            ("CHECK_SH_PUBLISH_DEADLINE_SECONDS".into(), deadline_seconds.to_string()),
        ],
        args: vec!["--publish".into()],
    }
}

/// Keep the diagnostic lines that say WHY, then the verdict block — both bounded.
///
/// `tail -8` alone kept exactly the seven gate lines plus FAIL and threw away everything that said
/// why, so two runs logged a bare "UNRUN close-evidence" while the gate had already printed rc +
/// stderr on the line above. A scan refusal that cannot say why cannot be fixed.
pub fn publish_failure_detail(output: &str) -> String {
    let mut lines: Vec<&str> = Vec::new();
    let reasons: Vec<&str> = output
        .lines()
        .filter(|l| {
            l.contains("NOTE")
                || l.contains("exit:")
                || l.contains("rc=")
                || l.contains("stderr=")
                || l.contains("scan_unavailable")
        })
        .collect();
    lines.extend(reasons.iter().rev().take(6).rev());
    let tail: Vec<&str> = output.lines().collect();
    lines.extend(tail.iter().rev().take(8).rev());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── LINEAGE ────────────────────────────────────────────────────────────────────────────
    #[test]
    fn rule_scheduled_requires_uid0_ppid1_cron() {
        let chain = vec![
            FleetMonitorAncestorRow { uid: 501, ppid: 4242, comm: "/bin/bash".into() },
            FleetMonitorAncestorRow { uid: 0, ppid: 1, comm: "/usr/sbin/cron".into() },
        ];
        assert_eq!(
            invoker_from_chain(&chain),
            FleetMonitorInvoker::SCHEDULED,
            "RULE lineage_scheduled: a uid=0 ppid=1 /usr/sbin/cron ancestor must prove SCHEDULED"
        );
    }

    #[test]
    fn rule_forged_cron_argv_is_not_scheduled() {
        // The measured attack: a cron-SHAPED name from a non-root detached shell.
        let chain = vec![FleetMonitorAncestorRow { uid: 501, ppid: 1, comm: "/usr/sbin/cron".into() }];
        assert_eq!(
            invoker_from_chain(&chain),
            FleetMonitorInvoker::MANUAL,
            "RULE lineage_forge_refused: a non-root /usr/sbin/cron ancestor must NOT prove SCHEDULED"
        );
    }

    #[test]
    fn rule_empty_chain_is_manual_not_scheduled() {
        assert_eq!(
            invoker_from_chain(&[]),
            FleetMonitorInvoker::MANUAL,
            "RULE lineage_fail_direction: unreadable ancestry must degrade to MANUAL, never SCHEDULED"
        );
    }

    #[test]
    fn rule_unparseable_ps_rows_are_dropped() {
        let rows = parse_ancestor_rows("0 1 /usr/sbin/cron\ngarbage\n501 x /bin/sh\n");
        assert_eq!(rows.len(), 1, "RULE ps_parse_strict: only fully-parseable rows survive");
        assert_eq!(invoker_from_chain(&rows), FleetMonitorInvoker::SCHEDULED);
    }

    // ── PANE LIVENESS (double-dispatch safety) ─────────────────────────────────────────────
    #[test]
    fn rule_wedged_markers_outrank_a_ready_footer() {
        // The shell's elif chain checks the two WEDGED states BEFORE any liveness footer, so a
        // pane showing both is wedged. Reversing this order would hand work to a pane that cannot
        // submit it.
        assert_eq!(
            pane_liveness("Press up to edit queued messages\n│ Ready │").reason,
            "queued_unsubmitted",
            "RULE liveness_wedged_first: an unsubmitted queued message outranks a Ready footer"
        );
        assert_eq!(
            pane_liveness("Restart to update\n│ Ready │").reason,
            "restart_required",
            "RULE liveness_wedged_first: a pending restart outranks a Ready footer"
        );
    }

    #[test]
    fn rule_ready_footer_needs_a_boundary_not_a_substring() {
        // `Already`/`Readying` must NOT read as a Ready footer: the shell bounds the match with
        // whitespace or the box glyph, and a bare substring match would invent idle capacity.
        assert_eq!(
            pane_liveness("Already done").state,
            LivenessState::Unproven,
            "RULE liveness_ready_boundary: 'Already' must not match the Ready footer"
        );
        assert_eq!(
            pane_liveness("│ Ready │").reason,
            "ready_footer",
            "RULE liveness_ready_boundary: the box-framed Ready footer matches"
        );
    }

    #[test]
    fn rule_working_and_worked_footers_are_live() {
        // Counter-intuitive but correct, and ported deliberately: these run only over panes ntm
        // already called safe_to_dispatch, and they prove an agent is PRESENT.
        assert_eq!(
            pane_liveness("Working (27s)").reason,
            "working_footer",
            "RULE liveness_working_footer: 'Working (' proves a live agent, matching the shell"
        );
        assert_eq!(
            pane_liveness("- Worked for 6m 00s -").reason,
            "codex_worked_footer",
            "RULE liveness_codex_dialect: a Codex past-tense footer is LIVE — not speaking this \
             dialect misread 3 of 5 live panes and idled workers beside 109 dispatchable beads"
        );
    }

    #[test]
    fn rule_omp_prompt_is_live_but_active_work_is_busy() {
        assert_eq!(
            pane_liveness("⠋ Working").reason,
            "omp_working_marker",
            "RULE liveness_omp_working: OMP's spinner marker proves in-flight work"
        );
        assert_eq!(
            pane_liveness("⠋ Working").state,
            LivenessState::Busy,
            "RULE liveness_omp_busy: an OMP spinner is never idle capacity"
        );
        assert_eq!(
            pane_liveness("⟨esc⟩").reason,
            "omp_working_marker",
            "RULE liveness_omp_interrupt: OMP's interrupt hint proves in-flight work"
        );
        assert_eq!(
            pane_liveness("⟨esc⟩").state,
            LivenessState::Busy,
            "RULE liveness_omp_interrupt_busy: an OMP interrupt hint is never idle capacity"
        );
        assert_eq!(
            pane_liveness("⠋ 2m · ◒ GPT-5.6\n⎋ Awaiting isolated reproduction\n╰─").state,
            LivenessState::Busy,
            "RULE liveness_omp_status_beats_footer: active status outranks a stale prompt footer"
        );
        assert_eq!(
            pane_liveness("history\n╰─\n").reason,
            "omp_prompt_footer",
            "RULE liveness_omp_prompt: OMP's boxed prompt proves an interactive client"
        );
        assert_eq!(
            pane_liveness("history\n❯\n").reason,
            "omp_prompt_footer",
            "RULE liveness_omp_claude_prompt: OMP Claude's prompt proves an interactive client"
        );
    }

    #[test]
    fn rule_empty_capture_is_unproven() {
        assert_eq!(
            pane_liveness("").reason,
            "empty_capture",
            "RULE liveness_empty: an empty capture is UNPROVEN, never LIVE"
        );
    }

    #[test]
    fn rule_no_marker_is_unproven_not_live() {
        assert_eq!(
            pane_liveness("just some scrollback").state,
            LivenessState::Unproven,
            "RULE liveness_unproven: absence of a marker is UNPROVEN, never LIVE"
        );
    }

    // ── ACTIVITY PARSING ───────────────────────────────────────────────────────────────────
    #[test]
    fn rule_only_safe_to_dispatch_panes_selected() {
        let j = r#"{"panes":[{"pane":1,"safe_to_dispatch":true},
                              {"pane":2,"safe_to_dispatch":false},
                              {"pane":3}]}"#;
        assert_eq!(
            safe_panes(j),
            vec!["1".to_string()],
            "RULE activity_safe_only: only safe_to_dispatch=true panes are candidates"
        );
    }

    #[test]
    fn rule_agents_shape_is_understood() {
        let j = r#"{"agents":[{"pane_index":4,"safe_to_dispatch":true}]}"#;
        assert_eq!(
            safe_panes(j),
            vec!["4".to_string()],
            "RULE activity_both_shapes: the agents/pane_index shape must parse too"
        );
    }

    #[test]
    fn rule_unparseable_activity_yields_no_panes() {
        assert!(
            safe_panes("not json").is_empty(),
            "RULE activity_fail_closed: unparseable activity yields NO dispatch candidates"
        );
    }

    // ── QUEUE COUNTING ─────────────────────────────────────────────────────────────────────
    #[test]
    fn rule_raw_count_counts_only_open() {
        let j = r#"[{"status":"open"},{"status":"closed"},{"status":"open"}]"#;
        assert_eq!(raw_open_count(j), 2, "RULE raw_open: only status=open rows count");
    }

    #[test]
    fn rule_unparseable_queue_counts_zero() {
        assert_eq!(
            raw_open_count("boom"),
            0,
            "RULE raw_open_fail_closed: unparseable br output counts zero, never inflates"
        );
    }

    // ── ATTENTION ──────────────────────────────────────────────────────────────────────────
    #[test]
    fn rule_wake_reason_defaults_to_none() {
        assert_eq!(
            attention_wake_reason("{}"),
            "none",
            "RULE wake_default: a payload without wake_reason reports none"
        );
        assert_eq!(attention_wake_reason("garbage"), "none");
    }

    #[test]
    fn rule_end_cursor_extracted() {
        let j = r#"{"cursor_info":{"end_cursor":"67890"}}"#;
        assert_eq!(attention_end_cursor(j), "67890", "RULE cursor: end_cursor is read for resume");
        assert_eq!(attention_end_cursor("{}"), "", "RULE cursor_absent: missing cursor is empty");
    }

    // ── PUBLISH CONTRACT ───────────────────────────────────────────────────────────────────
    #[test]
    fn rule_publish_uses_private_ledger_and_publishes_to_live() {
        let inv = publish_invocation(
            Path::new("/cp/bin/check.sh"),
            Path::new("/state"),
            Path::new("/state/fleet-monitor.jsonl"),
            1500,
            1500,
        );
        let env: std::collections::HashMap<_, _> = inv.env.iter().cloned().collect();
        assert_eq!(
            env["CHECK_SH_LEDGER"], "/state/check-sh-ledger.fleet-monitor.json",
            "RULE publish_private_ledger: the run must write its own ledger, never the live one — \
             a failing refresh must not destroy a fresher passing verdict"
        );
        assert_eq!(
            env["CHECK_SH_PUBLISH_LEDGER"], "/state/check-sh-ledger.json",
            "RULE publish_live_target: promotion targets the live standing verdict"
        );
        assert_eq!(inv.args, vec!["--publish".to_string()], "RULE publish_flag");
    }

    #[test]
    fn rule_publish_detail_keeps_the_reason_not_just_the_verdict() {
        let out = "gate a PASS\nNOTE close-evidence stage_timeout=900s exit:124\nUNRUN close-evidence\nFAIL";
        let detail = publish_failure_detail(out);
        assert!(
            detail.contains("stage_timeout=900s"),
            "RULE publish_detail_reason: the line that says WHY must survive transit, not just the verdict"
        );
    }

    // ── DISCOVERY ──────────────────────────────────────────────────────────────────────────
    #[test]
    fn rule_discovery_filters_to_existing_repos() {
        let tmp = std::env::temp_dir().join(format!("fm-disc-{}", std::process::id()));
        std::fs::create_dir_all(tmp.join("realrepo")).unwrap();
        let listing = "  realrepo: 3 windows\n  ghostrepo: 1 window\n";
        let repos = discover_repos(listing, &tmp);
        assert_eq!(
            repos,
            vec!["realrepo".to_string()],
            "RULE discovery_existing_only: a session without a repo directory is not a repo"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn rule_empty_ntm_list_is_cannot_observe_not_fleet_clear() {
        assert!(
            ntm_list_is_empty(""),
            "RULE empty_ntm_list: blank listing is an empty scan set"
        );
        assert!(
            ntm_list_is_empty("No tmux sessions running\n"),
            "RULE empty_ntm_list: ntm's exit-0 empty banner is an empty scan set"
        );
        assert_eq!(
            ntm_list_census_line("No tmux sessions running"),
            "CANNOT_OBSERVE|empty_ntm_list"
        );
        let tmp = std::env::temp_dir().join(format!("fm-empty-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        assert_eq!(
            observe_scan_set("No tmux sessions running", &tmp, ObserveRules { empty_scan: true }),
            ObserveScan::CannotObserve {
                reason: "empty_ntm_list"
            },
            "RULE empty_ntm_list: live observe must not log fleet_clear on an empty ntm list"
        );
        assert_eq!(
            observe_scan_set("No tmux sessions running", &tmp, ObserveRules { empty_scan: false }),
            ObserveScan::Repos(Vec::new()),
            "RULE empty_ntm_list_mutation: disabling the guard collapses empty list to drained"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn rule_live_ntm_list_fixture_is_listed_not_empty() {
        // Capture command: `ntm list` (see tests/fixtures/ntm-list-live.txt).
        let live = include_str!("../tests/fixtures/ntm-list-live.txt");
        assert!(
            !ntm_list_is_empty(live),
            "RULE empty_ntm_list_good: a captured live ntm list with sessions is not empty_ntm_list"
        );
        assert_eq!(ntm_list_census_line(live), "OK|listed");
        assert!(
            live.contains("control-plane:"),
            "RULE native_fixture: captured ntm list names the control-plane session"
        );
    }

    #[test]
    fn rule_live_activity_fixture_selects_only_safe_panes() {
        // Capture command: `ntm --robot-activity=control-plane`.
        let live = include_str!("../tests/fixtures/ntm-activity-control-plane.json");
        let panes = safe_panes(live);
        assert_eq!(
            panes,
            vec!["3".to_string()],
            "RULE native_activity: captured robot-activity selects the one safe_to_dispatch pane"
        );
    }

    // ── DEADLINE ───────────────────────────────────────────────────────────────────────────
    #[test]
    fn rule_run_deadline_expires() {
        let d = RunDeadline::new(Duration::from_millis(0));
        assert!(
            d.expired(),
            "RULE run_deadline: a zero budget is immediately expired so a hung run cannot silence the lane"
        );
        let d = RunDeadline::new(Duration::from_secs(600));
        assert!(!d.expired(), "RULE run_deadline_headroom: a fresh budget is not expired");
    }

    #[test]
    fn rule_json_escape_quotes_and_backslashes() {
        assert_eq!(
            json_escape(r#"a"b\c"#),
            r#"a\"b\\c"#,
            "RULE json_escape: a ledger row must stay parseable when a repo name carries a quote"
        );
    }
}

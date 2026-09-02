#![forbid(unsafe_code)]

//! Pane readiness classifier, ported from `bin/pane-dispatch-ready.sh`.
//!
//! FAIL CLOSED: anything not positively proven FREE is BUSY/QUOTA/NO_AGENT/UNREADABLE.
//! Never ranks on an NTM busy/error label. Work evidence is the agent's rendered line.
//! A FREE result is provisional until a second capture agrees (content hash). Busy
//! markers short-circuit immediately.
use regex::Regex;
use omp_types::{
    CaptureSnapshot, DispatchAdmissibility, EvidenceGrade, PaneLiveness, PaneObservation,
    UnknownReason,
};
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use subprocess_contract::{bounded_output, bounded_output_stdin, BoundedOutcome};

pub const BUSY_RE: &str = r"Working \([0-9]|esc to interrupt|Pursuing goal|Thinking…|Sautéed for|Infusing…|Warping…|Warping\.\.\.|Flummoxing…|Flummoxing\.\.\.|ctrl \+ t to view transcript";
// `(?i)` because a live Codex pane renders its model as "GPT-5.6-Luna" with a capital
// GPT. The lowercase-only `gpt-` made every Codex pane in the fleet classify NO_AGENT,
// so refill-idle-panes refused to dispatch to any of them while the ready queue sat
// 425 deep (measured 2026-09-02, zeststream-cast: 2 CONFIRMED_IDLE panes, 0 dispatched).
// Case-insensitivity is safe here: these are agent IDENTITY markers, not verdict
// tokens, and the fixture in main.rs pins the real capitalised form.
pub const AGENT_RE: &str = r"(?i)claude|codex|opus|gpt-|bypass permissions|dangerously";
pub const QUOTA_RE: &str = r"You've hit your usage limit|hit usage limits|Weekly limit left: 0%|purchasing more credits|purchase more credits";
pub const DEFAULT_BUSY_TAIL: usize = 6;
/// Window for the PROMPT-PRESENT check only -- never for busy detection.
/// Codex's footer (status line + box border + trailing blanks) puts its ready prompt
/// 6-7 lines from the end, outside the 6-line busy tail. 12 clears the tallest footer
/// measured on this fleet with margin, and cannot manufacture a BUSY->FREE flip: the
/// busy markers are evaluated first and return early.
pub const DEFAULT_PROMPT_TAIL: usize = 12;
pub const DEFAULT_QUOTA_TAIL: usize = 8;
pub const TWO_CAPTURE_MIN_SECS: u64 = omp_types::MIN_TWO_CAPTURE_INTERVAL_SECS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneDispatchReadyRule {
    TwoCaptureLiveness,
    TwoCaptureInterval,
    BusyMarkersLoadBearing,
    TailOnlyBusy,
    QuotaBeforeBusy,
    ComposerFailClosed,
}

impl PaneDispatchReadyRule {
    pub const ALL: &'static [PaneDispatchReadyRule] = &[
        PaneDispatchReadyRule::TwoCaptureLiveness,
        PaneDispatchReadyRule::TwoCaptureInterval,
        PaneDispatchReadyRule::BusyMarkersLoadBearing,
        PaneDispatchReadyRule::TailOnlyBusy,
        PaneDispatchReadyRule::QuotaBeforeBusy,
        PaneDispatchReadyRule::ComposerFailClosed,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            PaneDispatchReadyRule::TwoCaptureLiveness => "two_capture_liveness",
            PaneDispatchReadyRule::TwoCaptureInterval => "two_capture_interval",
            PaneDispatchReadyRule::BusyMarkersLoadBearing => "busy_markers_load_bearing",
            PaneDispatchReadyRule::TailOnlyBusy => "tail_only_busy",
            PaneDispatchReadyRule::QuotaBeforeBusy => "quota_before_busy",
            PaneDispatchReadyRule::ComposerFailClosed => "composer_fail_closed",
        }
    }
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|r| r.as_str() == name)
    }
}

#[derive(Clone, Debug)]
pub struct PaneDispatchReadyRules {
    pub two_capture_liveness: bool,
    pub two_capture_interval: bool,
    pub busy_markers_load_bearing: bool,
    pub tail_only_busy: bool,
    pub quota_before_busy: bool,
    pub composer_fail_closed: bool,
}

impl Default for PaneDispatchReadyRules {
    fn default() -> Self {
        Self {
            two_capture_liveness: true,
            two_capture_interval: true,
            busy_markers_load_bearing: true,
            tail_only_busy: true,
            quota_before_busy: true,
            composer_fail_closed: true,
        }
    }
}

impl PaneDispatchReadyRules {
    pub fn disable(&mut self, name: &str) -> bool {
        let Some(rule) = PaneDispatchReadyRule::parse(name) else {
            return false;
        };
        match rule {
            PaneDispatchReadyRule::TwoCaptureLiveness => self.two_capture_liveness = false,
            PaneDispatchReadyRule::TwoCaptureInterval => self.two_capture_interval = false,
            PaneDispatchReadyRule::BusyMarkersLoadBearing => self.busy_markers_load_bearing = false,
            PaneDispatchReadyRule::TailOnlyBusy => self.tail_only_busy = false,
            PaneDispatchReadyRule::QuotaBeforeBusy => self.quota_before_busy = false,
            PaneDispatchReadyRule::ComposerFailClosed => self.composer_fail_closed = false,
        }
        true
    }
    pub fn known_names_csv() -> String {
        PaneDispatchReadyRule::ALL
            .iter()
            .map(|r| r.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneDispatchReadyState {
    Free,
    Busy,
    /// A packet ARRIVED and was parked unsubmitted. Deliberately not folded into `Busy`:
    /// a wedged pane is not working, and the operator's action is to submit or clear the
    /// queued message, not to wait. Folding it into `Busy` would tell a caller "come back
    /// later" about a condition that never clears on its own.
    Wedged,
    QuotaBlocked,
    NoAgent,
    Unreadable,
}

impl PaneDispatchReadyState {
    /// The one hand-listed thing in this file. Kept honest behaviourally rather than by a
    /// pinned integer: `state_registry_covers_every_state_the_classifier_emits` asserts
    /// every state `classify` actually produces over the fixture corpus appears here.
    pub const ALL: &'static [PaneDispatchReadyState] = &[
        PaneDispatchReadyState::Free,
        PaneDispatchReadyState::Busy,
        PaneDispatchReadyState::Wedged,
        PaneDispatchReadyState::QuotaBlocked,
        PaneDispatchReadyState::NoAgent,
        PaneDispatchReadyState::Unreadable,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            PaneDispatchReadyState::Free => "FREE",
            PaneDispatchReadyState::Busy => "BUSY",
            PaneDispatchReadyState::Wedged => "WEDGED",
            PaneDispatchReadyState::QuotaBlocked => "QUOTA_BLOCKED",
            PaneDispatchReadyState::NoAgent => "NO_AGENT",
            PaneDispatchReadyState::Unreadable => "UNREADABLE",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "FREE" => Some(PaneDispatchReadyState::Free),
            "BUSY" => Some(PaneDispatchReadyState::Busy),
            "WEDGED" => Some(PaneDispatchReadyState::Wedged),
            "QUOTA_BLOCKED" => Some(PaneDispatchReadyState::QuotaBlocked),
            "NO_AGENT" => Some(PaneDispatchReadyState::NoAgent),
            "UNREADABLE" => Some(PaneDispatchReadyState::Unreadable),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaneDispatchReadyVerdict {
    pub state: PaneDispatchReadyState,
    pub reason: String,
}

impl PaneDispatchReadyVerdict {
    pub fn pipe_line(&self) -> String {
        format!("{}|{}", self.state.as_str(), self.reason)
    }
}

fn re(pat: &str) -> &'static Regex {
    static BUSY: OnceLock<Regex> = OnceLock::new();
    static AGENT: OnceLock<Regex> = OnceLock::new();
    static QUOTA: OnceLock<Regex> = OnceLock::new();
    match pat {
        "busy" => BUSY.get_or_init(|| Regex::new(BUSY_RE).expect("BUSY_RE")),
        "agent" => AGENT.get_or_init(|| Regex::new(AGENT_RE).expect("AGENT_RE")),
        _ => QUOTA.get_or_init(|| Regex::new(QUOTA_RE).expect("QUOTA_RE")),
    }
}

fn tail_lines(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    if lines.len() <= n {
        return text.to_string();
    }
    lines[lines.len() - n..].join("\n")
}

fn first_match(re: &Regex, text: &str) -> Option<String> {
    re.find(text).map(|m| m.as_str().to_string())
}

/// POSIX `grep -q '❯\|›\|^[[:space:]]*[>$][[:space:]]'` — a bare `>` with
/// nothing after it is NOT a prompt (the shell requires a following space).
/// Matching the oracle here is load-bearing: a more-permissive marker would
/// admit panes the live callers currently refuse.
/// Drop ANSI SGR sequences (`ESC [ … m`) so a glyph test can reach the glyph.
/// Deliberately narrow: only CSI-with-final-`m`, which is all tmux `-e` emits for
/// styling. Anything else is left in place rather than guessed at.
fn strip_sgr(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            // Consume the parameter/intermediate bytes up to the final byte.
            for f in chars.by_ref() {
                if f.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

fn has_prompt_marker(tail: &str) -> bool {
    for line in tail.lines() {
        if line.contains('❯') || line.contains('›') {
            return true;
        }
        let t = line.trim_start_matches(|c: char| c.is_whitespace());
        if t.starts_with('>') || t.starts_with('$') {
            let rest = &t[1..];
            if rest.starts_with(|c: char| c.is_whitespace()) {
                return true;
            }
        }
        // Codex renders its ready prompt as a STATUS LINE, not a leading glyph:
        //     " π  > ◕ GPT-5.6-Luna > 📁 ~/Developer/… > ⑂ main *134 ?367 > S161 …"
        // The `>` are mid-line separators, so the shell-oracle clauses above cannot
        // see it and every idle Codex pane read BUSY "no prompt marker" forever
        // (measured 2026-09-02: 2 CONFIRMED_IDLE panes, ready queue 425 deep).
        //
        // Anchored on the leading `π` idle glyph specifically. Codex swaps it for a
        // spinner (⠙/⠼) plus an elapsed timer while working, so this clause cannot
        // match a busy pane -- which is what keeps the fail-closed contract intact.
        // The live callers capture with `tmux capture-pane -e`, so this line arrives
        // as "\x1b[0m\x1b[48;2;…m \x1b[38;2;…mπ…" -- the glyph is preceded by SGR
        // escapes that no whitespace trim can remove. Strip them for THIS clause only;
        // the shell-oracle clauses above keep their exact byte-for-byte behaviour.
        let bare = strip_sgr(line);
        let b = bare.trim_start_matches(|c: char| c.is_whitespace());
        if b.starts_with('π') {
            let rest = b.trim_start_matches('π');
            if rest.starts_with(|c: char| c.is_whitespace()) {
                return true;
            }
        }
    }
    false
}

/// Classify captured pane text. `buffer_changed` is the second-capture motion bit.
pub fn classify(
    text: &str,
    buffer_changed: bool,
    rules: &PaneDispatchReadyRules,
) -> PaneDispatchReadyVerdict {
    if text.is_empty() {
        return PaneDispatchReadyVerdict {
            state: PaneDispatchReadyState::Unreadable,
            reason:
                "empty capture — pane blank or capture-pane failed; state unknowable (fail closed)"
                    .into(),
        };
    }
    if !re("agent").is_match(text) {
        return PaneDispatchReadyVerdict {
            state: PaneDispatchReadyState::NoAgent,
            reason: "no agent process rendering in this pane (bare shell)".into(),
        };
    }
    let qtail = tail_lines(text, DEFAULT_QUOTA_TAIL);
    if rules.quota_before_busy {
        if let Some(hit) = first_match(re("quota"), &qtail) {
            return PaneDispatchReadyVerdict {
                state: PaneDispatchReadyState::QuotaBlocked,
                reason: format!(
                    "provider quota exhausted ({hit}) — not busy, not free; needs spend, not a dispatch"
                ),
            };
        }
    }
    // PR-L1. CONSULT the existing authority; do NOT write a fourth detector. The marker was
    // already recognised in three other crates while the classifier whose only job is "can
    // this pane SAFELY receive a dispatch" did not ask, so the fix is a dependency edge, not
    // a new regex. `tick_monitor::classify` is strictly richer than the single anchor: it
    // recognises TWO parked-packet footers, and `receiver-receipt` -- whose contract forbids
    // I/O -- already consumes it, so this is a precedented edge onto a pure classifier.
    //
    // ORDER: after quota (a spend problem outranks everything) and BEFORE busy, for the same
    // reason tick-monitor checks Wedged before its own spinner branch -- a wedged pane can
    // still render a live spinner, so a busy-first order scores it BUSY and tells the caller
    // to come back later about a condition that never clears without an operator.
    if tick_monitor::classify(text) == tick_monitor::PaneState::Wedged {
        return PaneDispatchReadyVerdict {
            state: PaneDispatchReadyState::Wedged,
            reason: "a packet arrived and was PARKED unsubmitted — an operator must submit or \
                     clear the queued message; waiting will not clear it"
                .into(),
        };
    }
    let tail = if rules.tail_only_busy {
        tail_lines(text, DEFAULT_BUSY_TAIL)
    } else {
        text.to_string()
    };
    if rules.busy_markers_load_bearing {
        if let Some(hit) = first_match(re("busy"), &tail) {
            return PaneDispatchReadyVerdict {
                state: PaneDispatchReadyState::Busy,
                reason: format!("agent is working: {hit}"),
            };
        }
    }
    if buffer_changed {
        return PaneDispatchReadyVerdict {
            state: PaneDispatchReadyState::Busy,
            reason: "pane buffer changed between captures — rendered work is in flight".into(),
        };
    }
    // The prompt check gets its OWN, wider window. The 6-line busy tail is deliberately
    // tight so a stale spinner high in the scrollback cannot read as work-in-flight --
    // widening it would weaken BUSY detection. But Codex draws a taller footer than
    // Claude (status line, box border, then trailing blanks), so its ready prompt lands
    // 6-7 lines from the end and fell outside the busy window entirely.
    // Measured 2026-09-02 across this fleet: distance-from-end was 2, 6, 6 and 7 on the
    // four agent panes -- straddling the boundary, so ANY single tight window mis-reads
    // some panes. A wider window is safe HERE because this check only ever proves a
    // prompt is PRESENT; the busy markers above have already had their say and return
    // early, so nothing downstream can be talked out of BUSY by a wider look-back.
    let prompt_tail = tail_lines(text, DEFAULT_PROMPT_TAIL);
    if !has_prompt_marker(&prompt_tail) {
        return PaneDispatchReadyVerdict {
            state: PaneDispatchReadyState::Busy,
            reason: "no prompt marker in the live region — free-prompt not PROVEN (fail closed)"
                .into(),
        };
    }
    PaneDispatchReadyVerdict {
        state: PaneDispatchReadyState::Free,
        reason: "agent present, composer holds no typed text (bare prompt or autosuggestion)"
            .into(),
    }
}

/// Apply composer-typed.py outcome. rc 0 = typed = BUSY, 1 = FREE, else fail-closed BUSY.
pub fn apply_composer_rc(
    v: PaneDispatchReadyVerdict,
    rc: i32,
    _composer_path: &str,
    rules: &PaneDispatchReadyRules,
) -> PaneDispatchReadyVerdict {
    if v.state != PaneDispatchReadyState::Free {
        return v;
    }
    match rc {
        0 => PaneDispatchReadyVerdict {
            state: PaneDispatchReadyState::Busy,
            reason: "operator text typed in the composer, not ours to overwrite".into(),
        },
        1 => v,
        _ if rules.composer_fail_closed => PaneDispatchReadyVerdict {
            state: PaneDispatchReadyState::Busy,
            reason: format!("composer discriminator failed to evaluate (rc={rc}) — fail closed"),
        },
        _ => v,
    }
}

pub fn missing_composer(path: &str) -> PaneDispatchReadyVerdict {
    PaneDispatchReadyVerdict {
        state: PaneDispatchReadyState::Busy,
        reason: format!("composer discriminator missing at {path} (fail closed)"),
    }
}

/// Confirm a provisional FREE with a second capture. The canonical K0 evidence
/// type enforces the minimum interval and meaningful motion; this function
/// converts its typed refusal into an UNREADABLE, fail-closed readiness verdict.
pub fn confirm_free(
    first: PaneDispatchReadyVerdict,
    next_text: &str,
    previous: CaptureSnapshot,
    current: CaptureSnapshot,
    rules: &PaneDispatchReadyRules,
) -> PaneDispatchReadyVerdict {
    if first.state != PaneDispatchReadyState::Free {
        return first;
    }
    if !rules.two_capture_liveness {
        return first;
    }
    if !rules.two_capture_interval {
        return classify(next_text, false, rules);
    }
    match PaneObservation::from_two_captures(
        "pane-dispatch-ready",
        previous,
        current,
        DispatchAdmissibility::Unknown,
    ) {
        Ok(observation) => {
            let moved = matches!(
                observation.evidence(),
                EvidenceGrade::TwoCapture {
                    timer_changed: true,
                    ..
                } | EvidenceGrade::TwoCapture {
                    content_hash_changed: true,
                    ..
                }
            );
            classify(next_text, moved, rules)
        }
        Err(error) => PaneDispatchReadyVerdict {
            state: PaneDispatchReadyState::Unreadable,
            reason: format!("TWO_CAPTURE_UNPROVEN: {error} (fail closed)"),
        },
    }
}

pub fn spawn_timeout(mut cmd: Command, timeout: Duration) -> BoundedOutcome {
    bounded_output(&mut cmd, timeout)
}

pub fn sha_text(s: &str) -> String {
    let mut cmd = Command::new("shasum");
    match bounded_output_stdin(&mut cmd, Duration::from_secs(5), s.as_bytes()) {
        BoundedOutcome::Completed(output) if output.status.success() => output
            .stdout
            .split(|byte| byte.is_ascii_whitespace())
            .find(|part| !part.is_empty())
            .map(|part| String::from_utf8_lossy(part).into_owned())
            .unwrap_or_default(),
        BoundedOutcome::Completed(_) => String::new(),
        BoundedOutcome::TimedOut => {
            eprintln!("pane-dispatch-ready: shasum timed out before its deadline");
            String::new()
        }
        BoundedOutcome::Unspawned(error) => {
            eprintln!("pane-dispatch-ready: shasum could not spawn: {error}");
            String::new()
        }
    }
}
fn timer_piece(token: &str) -> Option<String> {
    let trimmed = token.trim_matches(|character: char| {
        !character.is_ascii_digit() && !matches!(character, 'h' | 'm' | 's')
    });
    if trimmed.is_empty()
        || !trimmed.chars().any(|character| character.is_ascii_digit())
        || !matches!(trimmed.chars().last(), Some('h' | 'm' | 's'))
    {
        return None;
    }
    Some(trimmed.to_owned())
}

fn timer_token(text: &str) -> Option<String> {
    for line in text.lines().rev() {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        for (index, token) in tokens.iter().enumerate() {
            let Some(first) = timer_piece(token) else {
                continue;
            };
            let mut value = first;
            if let Some(second) = tokens.get(index + 1).and_then(|token| timer_piece(token)) {
                value.push(' ');
                value.push_str(&second);
            }
            return Some(value);
        }
    }
    None
}

fn spinner_stripped_content(text: &str) -> String {
    text.chars()
        .filter(|character| !('⠀'..='⣿').contains(character))
        .collect()
}

/// Build the canonical K0 capture evidence from one raw pane capture.
/// Braille spinner glyphs are removed before hashing; the elapsed timer token
/// remains a separate motion signal.
pub fn capture_snapshot(captured_at_secs: u64, text: &str) -> CaptureSnapshot {
    CaptureSnapshot::new(
        captured_at_secs,
        PaneLiveness::Unknown(UnknownReason::Unclassified),
        timer_token(text),
        sha_text(&spinner_stripped_content(text)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r() -> PaneDispatchReadyRules {
        PaneDispatchReadyRules::default()
    }
    fn snapshot(at_secs: u64, hash: &str) -> CaptureSnapshot {
        CaptureSnapshot::new(at_secs, PaneLiveness::Idle, None, hash.to_owned())
    }

    fn st(text: &str) -> PaneDispatchReadyState {
        classify(text, false, &r()).state
    }

    #[test]
    fn working_timer_is_busy() {
        let t = "claude\n• Working (38m 29s • esc to interrupt)";
        assert_eq!(
            st(t),
            PaneDispatchReadyState::Busy,
            "rule busy_markers_load_bearing"
        );
    }

    #[test]
    fn empty_is_unreadable() {
        assert_eq!(st(""), PaneDispatchReadyState::Unreadable);
    }

    #[test]
    fn bare_shell_is_no_agent() {
        // Assembled by `concat!` so this source never contains the contiguous home
        // literal the repo-wide gate forbids (omp-orchestrator-npq).
        assert_eq!(
            st(concat!(
                "josh@Studio repo % pwd",
                "\n/Users/",
                "josh",
                "/Developer/x"
            )),
            PaneDispatchReadyState::NoAgent
        );
    }

    #[test]
    fn whitespace_only_is_no_agent() {
        assert_eq!(st(" "), PaneDispatchReadyState::NoAgent);
    }

    #[test]
    fn agent_no_prompt_is_busy() {
        assert_eq!(
            st("claude\nsome output with no prompt and no timer"),
            PaneDispatchReadyState::Busy
        );
    }

    #[test]
    fn empty_prompt_is_free() {
        assert_eq!(
            st("Opus 5 (1M context) │ bypass permissions\n❯ "),
            PaneDispatchReadyState::Free
        );
    }

    #[test]
    fn quota_banner_is_quota_blocked() {
        let t = "  Opus 5 (1M context) | control-plane\n■ You've hit your usage limit. try again later.\n❯ ";
        assert_eq!(
            st(t),
            PaneDispatchReadyState::QuotaBlocked,
            "rule quota_before_busy"
        );
    }

    #[test]
    fn busy_only_in_scrollback_is_free() {
        let t = "esc to interrupt appeared here long ago\n  Opus 5 (1M context) | control-plane\nf1\nf2\nf3\nf4\nf5\nf6\nf7\n❯ a suggestion";
        assert_eq!(st(t), PaneDispatchReadyState::Free, "rule tail_only_busy");
    }

    #[test]
    fn two_capture_motion_is_busy() {
        let first = classify("Opus 5 │ bypass permissions\n❯ ", false, &r());
        assert_eq!(first.state, PaneDispatchReadyState::Free);
        let v = confirm_free(
            first,
            "Opus 5 │ bypass permissions\n❯ ",
            snapshot(0, "aaa"),
            snapshot(TWO_CAPTURE_MIN_SECS, "bbb"),
            &r(),
        );
        assert_eq!(
            v.state,
            PaneDispatchReadyState::Busy,
            "rule two_capture_liveness: hash change is BUSY"
        );
    }

    #[test]
    fn disabling_two_capture_false_passes_motion() {
        let mut rules = r();
        assert!(rules.disable("two_capture_liveness"));
        let first = classify("Opus 5 │ bypass permissions\n❯ ", false, &rules);
        let v = confirm_free(
            first,
            "Opus 5 │ bypass permissions\n❯ ",
            snapshot(0, "aaa"),
            snapshot(TWO_CAPTURE_MIN_SECS, "bbb"),
            &rules,
        );
        assert_eq!(
            v.state,
            PaneDispatchReadyState::Free,
            "mutation two_capture_liveness: a single capture treats a generating pane as FREE"
        );
    }

    #[test]
    fn classifier_label_never_consulted() {
        let t = "Opus 5 │ bypass permissions\nERROR waiting idle THINKING\n❯ ";
        assert_eq!(
            st(t),
            PaneDispatchReadyState::Free,
            "rule no_classifier_as_truth: ntm ERROR/idle/THINKING words in scrollback do not decide"
        );
    }

    #[test]
    fn ntm_error_word_does_not_establish_busy() {
        let t = "claude\nsome output discussing an ERROR in JSON\nand more filler\nlines here\npadding\npad\n❯ ";
        assert_eq!(st(t), PaneDispatchReadyState::Free);
    }

    #[test]
    fn sauteed_infusing_warping_flummoxing_transcript_are_busy() {
        assert_eq!(
            st("claude\n✻ Sautéed for 3m 9s · 4 monitors still running"),
            PaneDispatchReadyState::Busy
        );
        assert_eq!(
            st("claude\n✽ Infusing… (21s · ↓ 443 tokens)"),
            PaneDispatchReadyState::Busy
        );
        assert_eq!(
            st("claude\n✻ Warping… (47s · ↓ 1.7k tokens)"),
            PaneDispatchReadyState::Busy
        );
        assert_eq!(
            st("claude\n✻ Flummoxing… (51s · ↓ 1.3k tokens)"),
            PaneDispatchReadyState::Busy
        );
        assert_eq!(
            st("codex\n… +43 lines (ctrl + t to view transcript)"),
            PaneDispatchReadyState::Busy
        );
    }

    #[test]
    fn spawn_timeout_kills_a_hung_child() {
        let mut cmd = Command::new("sleep");
        cmd.arg("30");
        let start = Instant::now();
        let out = spawn_timeout(cmd, Duration::from_millis(250));
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "rule bounded_waits: a hung child must not be waited on unbounded, elapsed={:?}",
            start.elapsed()
        );
        assert!(
            matches!(out, BoundedOutcome::TimedOut),
            "rule bounded_waits: timeout path must still return a restrictive timeout outcome"
        );
    }

    #[test]
    fn spawn_timeout_child_does_not_inherit_our_file_fd() {
        use std::os::unix::io::AsRawFd;
        let dir = std::env::temp_dir().join(format!("pdr-fd-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let held = dir.join("held");
        let guard = std::fs::File::create(&held).expect("held file");
        let fd = guard.as_raw_fd();
        let mut cmd = Command::new("/bin/sh");
        cmd.args(["-c", "exec 3<>/dev/fd/$CHECK_FD"])
            .env("CHECK_FD", fd.to_string());
        let out = spawn_timeout(cmd, Duration::from_secs(2));
        let BoundedOutcome::Completed(out) = out else {
            panic!("rule lock_not_inheritable: fd probe did not complete");
        };
        assert!(
            !out.status.success(),
            "rule lock_not_inheritable: child opened our File fd {fd} (inherited, not CLOEXEC)"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn crate_takes_no_run_lock_so_never_emits_unknown_holder() {
        let main = include_str!("main.rs");
        let token = format!("{}_{}", "holder", "pid");
        assert!(
            !main.contains(&token),
            "rule names_its_blocker: this observer takes no lock and must not emit holder_pid=unknown"
        );
    }

    #[test]
    fn crate_does_not_widen_admission() {
        let main = include_str!("main.rs");
        let token = format!("{}_{}", "ADMISSION", "FRESH");
        assert!(
            !main.contains(&token),
            "rule no_widened_admission: readiness does not own the standing verdict window"
        );
    }

    #[test]
    fn crate_does_not_default_to_a_sibling_repo() {
        let lib = include_str!("lib.rs");
        let main = include_str!("main.rs");
        // The needle is assembled by `concat!` so this guard never contains the
        // contiguous home literal it exists to forbid (omp-orchestrator-npq).
        let home_prefix = concat!("/Users/", "josh", "/Developer/");
        for src in [lib, main] {
            for other in ["franken-harvest", "clutterfreespaces", "foundry"] {
                assert!(
                    !src.contains(&format!("{home_prefix}{other}")),
                    "rule no_cross_repo_default: found {other}"
                );
            }
        }
    }

    #[test]
    fn every_named_rule_is_disableable() {
        assert!(!PaneDispatchReadyRule::ALL.is_empty());
        for rule in PaneDispatchReadyRule::ALL {
            let mut g = PaneDispatchReadyRules::default();
            assert!(g.disable(rule.as_str()), "{}", rule.as_str());
        }
    }

    #[test]
    fn disabling_busy_markers_lets_prompt_through() {
        let mut rules = r();
        assert!(rules.disable("busy_markers_load_bearing"));
        let t =
            "Opus 5 (1M context) │ bypass permissions\n• Working (38m 29s • esc to interrupt)\n❯ ";
        assert_eq!(
            classify(t, false, &rules).state,
            PaneDispatchReadyState::Free,
            "mutation busy_markers_load_bearing: markers are what blocked FREE"
        );
        assert_eq!(classify(t, false, &r()).state, PaneDispatchReadyState::Busy);
    }
}

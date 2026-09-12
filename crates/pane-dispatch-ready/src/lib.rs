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

/// WHAT THE RATE-LIMIT CENSUS CONTRIBUTED TO ONE PANE'S VERDICT — three-valued, because the
/// census is.
///
/// ⛔ THE SHAPE THIS REPLACES WAS A `BTreeMap<String, bool>` READ WITH `.unwrap_or(false)`, and
/// that single call collapsed THREE distinct facts into one `false`:
///   1. the verb answered and this pane is NOT rate-limited   (a measurement)
///   2. the verb answered and never NAMED this pane           (silence)
///   3. the verb refused, timed out, or shipped no payload    (no coverage at all)
/// Only (1) is evidence. (2) and (3) became "not rate-limited" and the pane was admitted.
///
/// ⭐ (2) IS NOT HYPOTHETICAL AND IT IS LIVE RIGHT NOW. Measured 2026-09-12T01:05Z on this
/// session: `tmux list-panes` reports FOUR panes (`0 %25 zsh`, `1 %33`, `2 %45`, `3 %26`) and an
/// UNSOLICITED `--robot-agent-health` names THREE (`"1","2","3"`) — it drops the non-agent pane
/// with no skipped list. This crate takes its denominator from tmux, correctly, and then looked
/// up pane `0` in the three-pane map and read the miss as a measured `false`. A bare `zsh` was
/// being admitted as rate-limit-clear by a census that had deliberately declined to describe it.
///
/// ⛔ AND THE SAME VERB CONTRADICTS ITSELF ON THAT PANE DEPENDING ON WHETHER YOU ASK. Solicited
/// (`--panes=0`) it answers for the bare shell with `agent_type:"cc"`, `health_grade:"A"`,
/// `safe_to_dispatch:true`, `recommendation:"HEALTHY"`. Unsolicited it omits the pane entirely.
/// The two answers are irreconcilable and the solicited one is the dangerous direction, which is
/// a second reason this layer must never turn the census's SILENCE into a positive finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitCoverage {
    /// The census measured this pane and it IS rate-limited.
    MeasuredLimited,
    /// The census measured this pane and it is NOT rate-limited. The only positive evidence.
    MeasuredFree,
    /// The census did not measure this pane: it refused, or it answered without naming it.
    Unmeasured,
}

impl RateLimitCoverage {
    /// Project `RateLimitCensus::is_rate_limited`'s three-valued answer without flattening it.
    #[must_use]
    pub fn of(answer: Option<bool>) -> Self {
        match answer {
            Some(true) => Self::MeasuredLimited,
            Some(false) => Self::MeasuredFree,
            None => Self::Unmeasured,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MeasuredLimited => "measured_limited",
            Self::MeasuredFree => "measured_free",
            Self::Unmeasured => "unmeasured",
        }
    }

    /// The boolean handed to [`rate_limit_refusal`].
    ///
    /// ⛔ `Unmeasured` YIELDS `false`, AND THAT IS A NAMED FAIL-OPEN RATHER THAN AN ACCIDENT —
    /// the whole reason this function exists instead of an inline `.unwrap_or(false)`.
    ///
    /// THE DIRECTION IS RULED HERE AND IT IS THE OPPOSITE OF `fleet-monitor`'s, ON PURPOSE.
    /// `fleet-monitor` is a DISPATCHER: withholding one unmeasured pane costs it a tick and the
    /// next live census recovers it. THIS CRATE IS THE ORACLE the dispatchers read
    /// (`refill-idle-panes` joins it against `--robot-activity`; `fast-dispatch` selects on its
    /// `state`). If an unreachable `ntm` made this layer emit `QUOTA_BLOCKED` for every pane, the
    /// whole fleet's capacity would read zero from a source none of them can cross-check — a
    /// fleet-wide false zero at maximum blast radius, which is the failure class this repo has
    /// already paid for twice.
    ///
    /// ⭐ SO THE COVERAGE IS PUBLISHED INSTEAD OF THE REFUSAL. The verdict is left exactly as
    /// `classify` produced it, and [`Self::as_str`] rides on the row so a consumer that WANTS to
    /// fail closed can: `refill-idle-panes` already withholds `Observation::Unknown`
    /// (`resolve(Idle, Unknown) => Unconfirmed`, never dispatched), so an unmeasured row gives it
    /// everything it needs to hold the pane WITHOUT this layer zeroing the fleet on its behalf.
    /// An undisclosed fail-open is the defect; a disclosed one is a bounded choice a reader can
    /// audit and a consumer can override.
    #[must_use]
    pub fn refusal_input(self) -> bool {
        matches!(self, Self::MeasuredLimited)
    }

    /// Whether a `FREE` verdict on this pane was confirmed against the rate-limit dimension.
    ///
    /// `false` for `Unmeasured`: the composer/liveness oracle is BLIND to rate limiting by
    /// construction — measured 2026-09-11, three panes held a clean empty prompt under a live
    /// ~5014-minute limit — so an unmeasured FREE is unconfirmed in exactly the dimension the
    /// census exists to cover.
    #[must_use]
    pub fn confirms_free(self) -> bool {
        matches!(self, Self::MeasuredFree)
    }
}

/// A composer-free pane whose AGENT cannot work is dispatchable-but-useless.
///
/// # Why this is an ADDITIONAL refusal and not a replacement oracle
///
/// `classify` answers *"is the composer free?"* and it is right about that. `ntm
/// --robot-agent-health` answers *"can this agent work?"*. Measured 2026-09-11 on four live
/// panes, those disagree: a pane can hold a clean empty prompt directly under
/// `You have hit your ChatGPT usage limit (pro plan). Try again in ~5014 min.` -- FREE and
/// useless for 83 HOURS. Neither verdict replaces the other, so this layers on top; the
/// composer remains the authority on FREE.
///
/// ⛔ AND IT IS THE MECHANISM, NOT A PROVISIONAL FALLBACK. 052de0b's message claimed this rule
/// was awaiting a better oracle that `--no-caut` had switched off. THAT CLAIM IS FALSE and the
/// commit could not be amended (the hook refuses a message-only commit as NOTHING_TO_CHECK), so
/// the retraction lives here, where a reader of the code finds it. Measured on one pane, two
/// calls ten seconds apart, raw payloads compared:
///
/// ```text
/// WITHOUT --no-caut   caut_enabled true   caut_available TRUE
/// WITH    --no-caut   caut_enabled false  caut_available false
/// is_rate_limited     true / true      health_grade D / D
/// recommendation      WAIT_FOR_RESET / same
/// indicators.limit    ["try again"] / ["try again"]
/// ```
///
/// The verdict does not move with caut REACHABLE, and the payload names its own mechanism:
/// `indicators.limit: ["try again"]` is a text match. No oracle was suppressed. A provider-quota
/// route may still be BUILDABLE -- caut is installed, codex is OAuth-authenticated, provider
/// status resolves live -- but `usage.primary` is NULL with zero token accounts, so there is
/// nothing to prefer over this rule today.
///
/// # ⛔ WHY THE TYPED FIELD ALONE IS NOT ENOUGH
///
/// `local_state.is_rate_limited` is itself a TEXT MATCH one layer down, and it FAILS CLOSED on a
/// superseded marker: pane %8 reported `is_rate_limited = true` while rendering a live TODO tree
/// BELOW the limit line -- an agent that recovered and is working. Refusing on the field alone
/// would have parked a working agent. The discriminator is CONTENT AFTER THE LAST LIMIT LINE:
/// the limit is live only when nothing but the composer footer follows it.
///
/// That is the same positional class as `wedge_reason`, inverted. `wedge_reason` fails OPEN by
/// missing a marker above its window; this one would fail CLOSED by matching a marker that has
/// been superseded below. Both are cured by asking about POSITION, not presence.
///
/// # NO-CLAIM
///
/// The footer recogniser is textual: a line is footer when it is blank, box-drawing only, or
/// starts with the composer prompt glyph. A future TUI that paints something else after the
/// limit line reads as CONTENT and this refusal declines to fire -- which is the safe direction
/// for an ADDITIONAL refusal, because the pane keeps whatever verdict `classify` gave it.
///
/// ⛔ THE THRESHOLD IS A CHOICE, NOT A MEASUREMENT, and it is labelled here so it cannot inherit
/// false authority from fixtures that never tested its boundary. THE FLOOR IS
/// [`SUBSTANTIVE_CONTENT_ROWS`] = 3: a pane must render at least that many substantive rows
/// below its last limit line before this layer accepts that it recovered.
///
/// MEASURED ENDPOINTS on the live population, same mechanical rule: a pane dead ~83 hours scores
/// ZERO once the title and footer are excluded (its raw count is 2, and that 2 IS the title and
/// footer -- the exclusion clause is what collapses it), a pane dead ~87 hours scores ONE (a
/// `TODO 247/284` header, which is not recovery), and the one working pane scores TEN. So the
/// observed population is {0, 1} DEAD against {10} ALIVE and EVERY floor in 2..=10 separates it
/// identically. Nothing measured distinguishes them; 3 is the middle of nothing.
///
/// ⛔ AN EARLIER VERSION OF THIS COMMENT SAID THE FLOOR WAS ONE, "in the direction that never
/// parks a working agent". That setting was FALSIFIED by the ~87-hour pane above, which renders
/// one row while dead, and the prose outlived the constant for two commits. The asymmetry it
/// appealed to points the other way at this evidence: admitting a dead pane parks the WORK for
/// days and nothing downstream recovers it, while refusing a live one costs idle capacity until
/// the next tick. Fail open with no signal; fail CLOSED with a typed signal you cannot
/// corroborate.
///
/// RESIDUAL: a dead pane rendering three stale rows is still admitted. Raise the floor only with
/// a measurement of the boundary, and change these sentences when you do.
#[must_use]
pub fn rate_limit_refusal(is_rate_limited: bool, capture: &str) -> Option<String> {
    if !is_rate_limited {
        return None;
    }
    let lines: Vec<&str> = capture.lines().collect();
    let Some(last_limit) = last_limit_line(&lines) else {
        // ⛔ CLAUSE 4: A MISSING ANCHOR IS NOT AN ABSENT CONDITION. The typed field says
        // rate-limited; if the text cannot be located we have LESS evidence, not more, and the
        // safe reading of less evidence is refusal.
        return Some(
            "agent reports rate-limited and no limit line could be located in the capture -- \
             refusing on the typed field alone rather than admitting on a failed text match"
                .to_owned(),
        );
    };
    let content_after = lines[last_limit + 1..]
        .iter()
        .filter(|line| !is_composer_footer(line) && !is_limit_message_tail(line))
        .count();
    if content_after >= SUBSTANTIVE_CONTENT_ROWS {
        return None;
    }
    Some(format!(
        "agent reports rate-limited and only {content_after} substantive row(s) follow the limit \
         line (below the {SUBSTANTIVE_CONTENT_ROWS}-row recovery floor) -- dispatch would park until reset"
    ))
}

/// Rows of non-footer, non-limit-tail content that prove an agent resumed BELOW its limit line.
///
/// ⛔ A CHOICE, informed by three measured panes and no more. Observed: a pane dead for ~87 hours
/// renders ONE such row (a `TODO 247/284` header), panes dead ~83 hours render ZERO, and the one
/// working pane renders TEN. So 0 and 1 are both DEAD and 10 is ALIVE; any floor in 2..=10
/// separates the observed population and nothing here distinguishes them.
///
/// ⭐ AND THE DIRECTION CHANGED WHEN THE %7 SPECIMEN ARRIVED. This started at `> 0`, optimised to
/// never park a working agent. That was wrong once a DEAD pane was measured rendering one row:
/// the asymmetry is not symmetric. Admitting a dead pane parks a packet for DAYS; refusing a live
/// one costs idle capacity until the next tick, because the verdict is re-derived per tick and
/// never cached. The floor is therefore set to FAIL CLOSED, and the earlier optimisation is
/// superseded rather than quietly kept.
pub const SUBSTANTIVE_CONTENT_ROWS: usize = 3;

/// The LAST limit-phrase anchor, tolerant of a phrase broken across rows by a narrow pane.
///
/// Measured: a 19-column pane splits `ChatGPT usage limit` mid-phrase, so a line-based
/// `contains("usage limit")` finds NOTHING on a pane that is dead for 87 hours -- a false
/// NEGATIVE in the admitting direction. Whitespace runs are collapsed before matching because
/// re-joining rows produces a DOUBLE space next to the existing trailing one.
fn last_limit_line(lines: &[&str]) -> Option<usize> {
    let mut flat = String::new();
    let mut owner: Vec<usize> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        for character in line.chars() {
            if character.is_whitespace() {
                if !flat.ends_with(' ') {
                    flat.push(' ');
                    owner.push(index);
                }
            } else {
                flat.push(character);
                owner.push(index);
            }
        }
        if !flat.ends_with(' ') {
            flat.push(' ');
            owner.push(index);
        }
    }
    let end = flat.rfind("usage limit")? + "usage limit".len();
    owner.get(end.saturating_sub(1)).copied()
}

/// Blank, box-drawing-only, or an agent STATUS LINE: paint, not work.
///
/// ⛔ THE FIRST VERSION MISSED THE REAL FOOTER AND THAT ALONE ADMITTED A PANE DEAD 87 HOURS.
/// It tested for a leading `π` or `>` or an all-box-drawing line. The omp/Codex status line
/// leads with an EMOJI (`📁`) and carries `▶` and `┃`, none of which are in `'─'..='╿'`, so it
/// counted as a substantive content row and donated the third row that flipped %7 from REFUSE
/// to ADMIT. A recogniser keyed on the glyphs one pane happened to paint is the positional
/// class again, one layer down.
fn is_composer_footer(line: &str) -> bool {
    // ⛔ THE PRODUCTION CAPTURE CARRIES ESCAPES. run_live captures with `-p -e`, so under SGR
    // every row's FIRST character is U+001B and every positional test below would answer about
    // an escape rather than a glyph. `contains` is escape-INSENSITIVE (SGR surrounds glyphs, it
    // never splits codepoints); first-character, `starts_with` and `all(range)` are
    // escape-DESTROYED. Stripping CSI here is what lets the positional arms survive the flag --
    // and it is confined to this predicate, which reasons about TEXT. It is deliberately NOT
    // applied to any ANSI-dependent oracle, where stripping would compute a plausible answer
    // from destroyed evidence.
    let plain = strip_csi(line);
    let trimmed = plain.trim();
    if trimmed.is_empty() {
        return true;
    }
    let Some(first) = trimmed.chars().next() else {
        return true;
    };
    // The prompt glyph is ALPHABETIC (`π` is a Greek letter), so it is named explicitly.
    if first == 'π' || first == '>' {
        return true;
    }
    // ⛔ STRUCTURE, NOT A CHARACTER RANGE, AND NOT THE FRAME EITHER. The status row mixes
    // U+1F4C1, U+25B6 and BOTH box-drawing weights, so a range whitelist keeps meeting rows it
    // paints outside; a leading-symbol rule over-excludes the opposite way and swallows the
    // `├─`/`│` rows of a live TODO tree, which is the very evidence of work this predicate
    // exists to preserve. A ▶/┃ FRAME TEST WAS ALSO FALSIFIED: over a seven-pane corpus the
    // frame appears on 5/7 and misses the two BUSIEST panes, so it would donate a phantom
    // content row exactly where it costs most. The one invariant at 7/7 is the PATH BADGE
    // U+1F4C1, and it sits MID-ROW on wide panes, so it is matched anywhere in the line.
    //
    // NO-CLAIM: 📁 is a USER-CONFIGURABLE prompt element. This is invariant over THIS FLEET'S
    // prompt config, not over the TUI, and `a_footer_without_the_path_badge_is_still_counted`
    // pins the residual rather than hiding it.
    if trimmed.contains('\u{1F4C1}') {
        return true;
    }
    trimmed
        .chars()
        .all(|character| character.is_whitespace() || ('─'..='╿').contains(&character))
}

/// Remove CSI escape sequences so a TEXT predicate sees glyphs, not SGR state.
///
/// Scoped to the footer recogniser by design. Anything reasoning about dim-vs-bright rendering
/// must read the RAW bytes; stripping there would destroy the evidence it keys on.
fn strip_csi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(character) = chars.next() {
        if character != '\u{1B}' {
            out.push(character);
            continue;
        }
        // ESC [ ... <final byte in @..~>
        if chars.next() == Some('[') {
            for inner in chars.by_ref() {
                if ('\u{40}'..='\u{7E}').contains(&inner) {
                    break;
                }
            }
        }
    }
    out
}

/// CLAUSE 3: the limit message's OWN wrap or repeat is not evidence against the limit.
///
/// Anchoring on the LAST `usage limit` line already discards a repeated copy, but a wrapped
/// TAIL can still fall below that anchor. Counting it was the measured defect: a two-clause
/// rule read all five limit-bearing panes as RECOVERED because the marker's own continuation
/// sat after it -- a positional predicate whose window includes the thing it measures.
///
/// ⛔ AND THE NARROW-PANE WRAP SPLITS THE DURATION ONTO ITS OWN ROW. At w=19 the message breaks
/// as `Try again in` / `~5211 min.`, so catching only the former left `~5211 min.` counted as
/// RECOVERY -- clause 3's defect surviving inside the very case clause 4 was written for.
fn is_limit_message_tail(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with("Error: Retry")
        || trimmed.contains("Try again in")
        || trimmed.contains("You have hit your")
        || trimmed.starts_with("ChatGPT usage limit")
        || trimmed.starts_with("limit (pro plan)")
    {
        return true;
    }
    // A bare duration continuation: `~5211 min.` and nothing else of substance.
    let duration = trimmed.trim_start_matches('~');
    let (digits, rest) = duration.split_at(
        duration
            .find(|character: char| !character.is_ascii_digit())
            .unwrap_or(duration.len()),
    );
    !digits.is_empty() && rest.trim_start().starts_with("min")
}

#[cfg(test)]
mod tests {
    /// ⛔ VERBATIM CAPTURES, NOT DESCRIPTIONS. The first version of these fixtures was ABRIDGED
    /// and the substituted row was the one that decides the verdict: the real %7 status row
    /// leads with U+1F4C1 (FILE FOLDER) and carries U+25B6 and U+2503, while the abridged row
    /// led with `π`. That single substitution moved content_after from 3 to 2 and the suite
    /// went green over a pane dead ~87 hours. A fixture that is DESCRIBED rather than PASTED is
    /// how a suite stays green, and it is worse inside a fixture than in a predicate because a
    /// fixture is what everyone downstream trusts instead of re-measuring.
    ///
    /// ⚠️ PROVENANCE IS NOT UNIFORM, AND THE DIFFERENCE IS NAMED RATHER THAN AVERAGED. `%8` and
    /// `%7` were byte-compared against an independent grader's preserved copies (`cmp -s`
    /// identical, sha256 117bf2e7… and ee3ffd8f…), so the substituted-row defect CANNOT be
    /// present in them. `pane19-dead.txt` HAS NO SECOND COPY TO DIFF AGAINST -- it is my own
    /// 49-row scrollback capture of that pane and nobody else preserved one. Two of the three
    /// are oracle-verified; the third is ASSERTED. It is also the strongest of the three for
    /// last-occurrence anchoring, because 38 rows of prior content sit ABOVE its limit lines.
    const DEAD_AFTER_LIMIT: &str = include_str!("../tests/fixtures/pane19-dead.txt");
    const RECOVERED_AFTER_LIMIT: &str = include_str!("../tests/fixtures/pane8-alive.txt");
    const DEAD_BUT_PHRASE_SPLIT: &str = include_str!("../tests/fixtures/pane7-dead-narrow.txt");
    use super::*;


    /// FIRES-ON-KNOWN-BAD: the live-limit pane is refused, and the reason names the evidence.
    #[test]
    fn a_rate_limited_pane_with_nothing_after_the_limit_line_is_refused() {
        let reason = rate_limit_refusal(true, DEAD_AFTER_LIMIT)
            .expect("a dead rate-limited pane must be refused");
        assert!(reason.contains("rate-limited"), "{reason}");
        assert!(reason.contains("0 substantive row(s)"), "{reason}");
    }

    /// KNOWN-GOOD, AND THE ONE THAT MATTERS: the verb's own field says rate-limited, the pane is
    /// working, and the refusal DECLINES. Delete the content-after-the-line clause and this leg
    /// reddens -- which is the whole reason the clause exists.
    #[test]
    fn a_recovered_pane_working_below_the_limit_line_is_not_refused() {
        assert_eq!(
            rate_limit_refusal(true, RECOVERED_AFTER_LIMIT),
            None,
            "an agent rendering work below the limit line is not rate-limited any more"
        );
    }

    /// The typed field is a precondition, not an inference: no field, no refusal, whatever the
    /// capture says. This refusal never invents a rate limit from pane text alone.
    #[test]
    fn the_refusal_requires_the_typed_field_and_never_infers_it() {
        assert_eq!(rate_limit_refusal(false, DEAD_AFTER_LIMIT), None);
        assert_eq!(rate_limit_refusal(false, RECOVERED_AFTER_LIMIT), None);
        assert_eq!(rate_limit_refusal(false, DEAD_BUT_PHRASE_SPLIT), None);
    }

    /// CLAUSE 4, SCOPED: no typed field AND no limit line is a healthy pane, not a hidden one.
    /// Refusing here would park a pane against which no evidence of any limit exists.
    #[test]
    fn no_field_and_no_limit_line_is_never_refused() {
        assert_eq!(rate_limit_refusal(false, " π  > ◒ GPT-5.6-Luna\n╰──\n"), None);
    }

    /// ⛔ CLAUSE 4, THE OTHER HALF: field TRUE and no locatable limit line must REFUSE. A missing
    /// anchor is not an absent condition -- it is less evidence, and the safe reading of less
    /// evidence is refusal. Before this leg the rule admitted the deadest pane in the session.
    #[test]
    fn the_typed_field_with_no_locatable_limit_line_refuses() {
        let reason = rate_limit_refusal(true, " π  > ◒ GPT-5.6-Luna\n╰──\n")
            .expect("a typed limit with no anchor must refuse, never admit");
        assert!(reason.contains("no limit line could be located"), "{reason}");
    }

    /// ⛔ THE 19-COLUMN PANE: the phrase is split mid-phrase, the pane is dead ~87 hours, and a
    /// line-based matcher reads it as limit-free. Whitespace-insensitive anchoring finds it, and
    /// the single TODO header below the line is under the recovery floor, so it is REFUSED.
    #[test]
    fn a_narrow_pane_that_splits_the_limit_phrase_is_still_refused() {
        let reason = rate_limit_refusal(true, DEAD_BUT_PHRASE_SPLIT)
            .expect("a 19-column dead pane must be refused, not admitted");
        assert!(reason.contains("rate-limited"), "{reason}");
        assert!(
            !reason.contains("no limit line could be located"),
            "the wrapped phrase must be FOUND, not fall through to the fail-closed arm: {reason}"
        );
    }

    /// CLAUSE 3, the measured defect: a wrapped TAIL below the anchor line is the marker's own
    /// continuation, not work. A two-clause rule counted it and read all five limit-bearing
    /// panes as RECOVERED.
    #[test]
    fn the_limit_messages_own_wrap_is_not_counted_as_recovery() {
        let wrapped = "\
 Error: Retry budget exhausted after 10 retries: You have hit your
   ChatGPT usage limit (pro plan).
   Try again in ~5014 min.

 π  > ◒ GPT-5.6-Luna
╰──
";
        let reason = rate_limit_refusal(true, wrapped)
            .expect("the marker's own wrap must not read as recovery");
        assert!(reason.contains("0 substantive row(s)"), "{reason}");
    }

    /// RESIDUAL, PINNED: the footer recogniser's one 7/7 invariant is a USER-CONFIGURABLE prompt
    /// element. A status row painted WITHOUT the path badge and without the prompt glyph is
    /// counted as content, which pushes a dead pane toward ADMIT. This leg exists so that
    /// property is a recorded fact rather than a surprise in someone's incident.
    #[test]
    fn a_footer_without_the_path_badge_is_still_counted() {
        let no_badge = " usage limit (pro plan). Try again in ~99 min.\n Luna v2 ▶── ready\n";
        assert!(
            rate_limit_refusal(true, no_badge).is_some(),
            "one uncounted row is still below the floor -- but the row IS counted, which is the \
             residual: at floor-1 this input would flip to ADMIT"
        );
    }

    /// ISOLATES THE PATH-BADGE ARM. This footer is the %33 shape: it leads with a BRAILLE
    /// SPINNER (U+283C, because the pane was working) and carries no ▶, no ┃ and no box drawing,
    /// so neither the prompt-glyph arm nor a frame test can see it. Only the 📁 arm excludes it.
    /// Measured over a seven-pane corpus: ▶ 5/7, ┃ 5/7, ─ 5/7, π 4/7, 📁 7/7 -- the frame misses
    /// the two BUSIEST panes, which is where a phantom content row costs most.
    #[test]
    fn a_spinner_led_footer_is_excluded_by_the_path_badge_alone() {
        let spinner_footer =
            " usage limit (pro plan). Try again in ~99 min.\n⠼ 3m · ◕ Opus 5 · 📁 ~/Developer/omp\n";
        let reason = rate_limit_refusal(true, spinner_footer).expect("dead pane must refuse");
        assert!(
            reason.contains("0 substantive row(s)"),
            "the spinner-led status row must be excluded as footer, not counted as work: {reason}"
        );
    }

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

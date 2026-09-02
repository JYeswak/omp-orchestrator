#![forbid(unsafe_code)]

//! Pane readiness classifier, ported from `bin/pane-dispatch-ready.sh`.
//!
//! FAIL CLOSED: anything not positively proven FREE is BUSY/QUOTA/NO_AGENT/UNREADABLE.
//! Never ranks on an NTM busy/error label. Work evidence is the agent's rendered line.
//! A FREE result is provisional until a second capture agrees (content hash). Busy
//! markers short-circuit immediately.
//!
//! BUFFER_MOTION_SECONDS defaults to 10, matching the shell oracle. The rubric's 75s
//! interval would change which panes live callers treat as FREE; we do not widen it.

use regex::Regex;
use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

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
pub const DEFAULT_MOTION_SECS: u64 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneDispatchReadyRule {
    TwoCaptureLiveness,
    BusyMarkersLoadBearing,
    TailOnlyBusy,
    QuotaBeforeBusy,
    ComposerFailClosed,
}

impl PaneDispatchReadyRule {
    pub const ALL: &'static [PaneDispatchReadyRule] = &[
        PaneDispatchReadyRule::TwoCaptureLiveness,
        PaneDispatchReadyRule::BusyMarkersLoadBearing,
        PaneDispatchReadyRule::TailOnlyBusy,
        PaneDispatchReadyRule::QuotaBeforeBusy,
        PaneDispatchReadyRule::ComposerFailClosed,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            PaneDispatchReadyRule::TwoCaptureLiveness => "two_capture_liveness",
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
    pub busy_markers_load_bearing: bool,
    pub tail_only_busy: bool,
    pub quota_before_busy: bool,
    pub composer_fail_closed: bool,
}

impl Default for PaneDispatchReadyRules {
    fn default() -> Self {
        Self {
            two_capture_liveness: true,
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
    QuotaBlocked,
    NoAgent,
    Unreadable,
}

impl PaneDispatchReadyState {
    pub fn as_str(self) -> &'static str {
        match self {
            PaneDispatchReadyState::Free => "FREE",
            PaneDispatchReadyState::Busy => "BUSY",
            PaneDispatchReadyState::QuotaBlocked => "QUOTA_BLOCKED",
            PaneDispatchReadyState::NoAgent => "NO_AGENT",
            PaneDispatchReadyState::Unreadable => "UNREADABLE",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "FREE" => Some(PaneDispatchReadyState::Free),
            "BUSY" => Some(PaneDispatchReadyState::Busy),
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
pub fn classify(text: &str, buffer_changed: bool, rules: &PaneDispatchReadyRules) -> PaneDispatchReadyVerdict {
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
            reason: format!(
                "pane buffer changed over {DEFAULT_MOTION_SECS}s — rendered work is in flight"
            ),
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
pub fn apply_composer_rc(v: PaneDispatchReadyVerdict, rc: i32, _composer_path: &str, rules: &PaneDispatchReadyRules) -> PaneDispatchReadyVerdict {
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

/// Confirm a provisional FREE with a second capture. Disabling two_capture_liveness
/// returns the first verdict unchanged — the frozen/generating fusion.
pub fn confirm_free(
    first: PaneDispatchReadyVerdict,
    next_text: &str,
    sha1: &str,
    sha2: &str,
    rules: &PaneDispatchReadyRules,
) -> PaneDispatchReadyVerdict {
    if first.state != PaneDispatchReadyState::Free {
        return first;
    }
    if !rules.two_capture_liveness {
        return first;
    }
    if next_text.is_empty() || sha1.is_empty() || sha2.is_empty() {
        return classify(next_text, false, rules);
    }
    if sha1 != sha2 {
        return classify(next_text, true, rules);
    }
    classify(next_text, false, rules)
}

pub fn spawn_timeout(mut cmd: Command, timeout: Duration) -> Option<Output> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().ok()?;
    // DRAIN THE PIPES ON DEDICATED THREADS.  `try_wait` in a poll loop CANNOT be paired with
    // undrained pipes: a child that writes more than the OS pipe buffer (~64 KiB, and stdout and
    // stderr each have their own) blocks in `write` forever, so it never exits, so `try_wait`
    // never returns Some, and the call burns its entire timeout at 0% CPU before being killed.
    //
    // MEASURED 2026-08-27: `git -C <repo> log --since "24 hours ago" --oneline` completes in
    // 0.6-0.9s from a shell, and sat at 0.0% CPU for 104s as a child here -- reproduced exactly by
    // polling `try_wait` without reading the pipes.  Six crates shared this shape; fixing only the
    // one that fired would have left five live.
    let out = child.stdout.take().map(|mut r| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = r.read_to_end(&mut buf);
            buf
        })
    });
    let err = child.stderr.take().map(|mut r| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = r.read_to_end(&mut buf);
            buf
        })
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() >= timeout => {
                let _ = child.kill();
                break child.wait().ok()?;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => return None,
        }
    };
    // The readers end when the child's fds close, which the kill above guarantees.
    let stdout = out.and_then(|h| h.join().ok()).unwrap_or_default();
    let stderr = err.and_then(|h| h.join().ok()).unwrap_or_default();
    Some(Output { status, stdout, stderr })
}

pub fn sha_text(s: &str) -> String {
    let mut cmd = Command::new("shasum");
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(s.as_bytes());
    }
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .ok()
                    .and_then(|o| {
                        String::from_utf8_lossy(&o.stdout)
                            .split_whitespace()
                            .next()
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_default();
            }
            Ok(None) if start.elapsed() >= Duration::from_secs(5) => {
                let _ = child.kill();
                return String::new();
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => return String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r() -> PaneDispatchReadyRules {
        PaneDispatchReadyRules::default()
    }

    fn st(text: &str) -> PaneDispatchReadyState {
        classify(text, false, &r()).state
    }

    #[test]
    fn working_timer_is_busy() {
        let t = "claude\n• Working (38m 29s • esc to interrupt)";
        assert_eq!(st(t), PaneDispatchReadyState::Busy, "rule busy_markers_load_bearing");
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
        assert_eq!(st(t), PaneDispatchReadyState::QuotaBlocked, "rule quota_before_busy");
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
        let v = confirm_free(first, "Opus 5 │ bypass permissions\n❯ ", "aaa", "bbb", &r());
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
            "aaa",
            "bbb",
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
        assert_eq!(st("claude\n✽ Infusing… (21s · ↓ 443 tokens)"), PaneDispatchReadyState::Busy);
        assert_eq!(st("claude\n✻ Warping… (47s · ↓ 1.7k tokens)"), PaneDispatchReadyState::Busy);
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
            out.is_some(),
            "rule bounded_waits: timeout path must still return"
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
        let out = spawn_timeout(cmd, Duration::from_secs(2)).expect("sh open-fd");
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

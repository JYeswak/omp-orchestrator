#![no_main]
//! INVARIANTS for the pane classifier — the reading every dispatch decision is built on:
//!
//! 1. `classify` never panics on any bytes (a capture is attacker-shaped: scrollback, ANSI, quoted
//!    prose, half-drawn box borders).
//! 2. Precedence is total and fixed: 402 > Wedged > Dialog > (spinner+timer) > `π` > Unproven.
//!    A parked-packet footer ANYWHERE in the capture wins over a live spinner (bead 46y7: a
//!    wedged pane still renders a spinner); a spinner in scrollback never makes a pane Working
//!    (only the LAST status line is read).
//! 3. `stable_hash` is invariant under spinner animation and timer advance: two captures that
//!    differ only in braille frames / elapsed tokens hash equal. This is the receipt discriminator
//!    (receiver-receipt: "spinner-stripped content hash"); if animation leaks into it, a dead
//!    pane reads busy forever.
//! 4. `parse_timer` never accepts an uppercase unit (`1.3M` budget, `S0.25` spend) and is
//!    monotone in the digits it accepts.
//!
//! Structure-aware: the input is a small grammar of status-line fragments rather than raw bytes,
//! so the fuzzer spends its budget inside the classifier's arms (testing-fuzzing rule 4).

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use tick_monitor::{classify, last_status_line, parse_timer, stable_hash, PaneState};

const MAX_LINES: usize = 24;

#[derive(Arbitrary, Debug, Clone)]
enum Fragment {
    Spinner(u8),
    Timer { n: u16, unit: u8 },
    Pi,
    Model(u8),
    WedgedFooter(bool),
    DialogFooter(u8),
    Provider402(bool),
    Border,
    Prose(Vec<u8>),
    Raw(Vec<u8>),
}

fn render(f: &Fragment) -> String {
    match f {
        Fragment::Spinner(i) => char::from_u32(0x2800 + u32::from(*i)).map(|c| c.to_string()).unwrap_or_default(),
        Fragment::Timer { n, unit } => format!("{n}{}", ['s', 'm', 'h', 'S', 'M', 'H', 'x', 'd'][usize::from(*unit % 8)]),
        Fragment::Pi => "π".to_owned(),
        Fragment::Model(i) => ["◕ Opus 5", "◉ GLM 5.3", "◕ GPT-5.6-Luna", "GPT-5.5", "◕ GPT-4"][usize::from(*i % 5)].to_owned(),
        Fragment::WedgedFooter(alt) => if *alt { "Messages to be submitted after next tool call" } else { "Press up to edit queued messages" }.to_owned(),
        Fragment::DialogFooter(i) => ["│ Enter select", "│ Esc cancel", "│ ↑/↓ move", "Esc cancel"][usize::from(*i % 4)].to_owned(),
        Fragment::Provider402(cross) => if *cross {
            "✘ 402 This request requires more credits, or fewer max_tokens"
        } else {
            "Error: 402 This request requires more credits, or fewer max_tokens"
        }.to_owned(),
        Fragment::Border => "─────────────".to_owned(),
        Fragment::Prose(b) => String::from_utf8_lossy(b).chars().take(40).collect(),
        Fragment::Raw(b) => String::from_utf8_lossy(b).chars().take(40).collect(),
    }
}

#[derive(Arbitrary, Debug)]
struct Input {
    lines: Vec<Vec<Fragment>>,
    /// A second frame: same content, spinner index and timer bumped — must hash equal.
    bump_spinner: u8,
    bump_timer: u16,
}

fn render_capture(lines: &[Vec<Fragment>], spin_bump: u8, timer_bump: u16) -> String {
    let mut out = String::new();
    for line in lines.iter().take(MAX_LINES) {
        for f in line {
            let rendered = match f {
                Fragment::Spinner(i) => render(&Fragment::Spinner(i.wrapping_add(spin_bump) % 0xFF)),
                Fragment::Timer { n, unit } => render(&Fragment::Timer { n: n.saturating_add(timer_bump), unit: *unit }),
                other => render(other),
            };
            out.push_str(&rendered);
            out.push(' ');
        }
        out.push('\n');
    }
    out
}

fn precedence_model(capture: &str) -> PaneState {
    if capture.lines().any(|l| {
        let l = l.trim_start();
        l.starts_with("Error: 402 This request requires more credits, or fewer max_tokens")
            || l.starts_with("✘ 402 This request requires more credits, or fewer max_tokens")
    }) {
        return PaneState::ProviderError402;
    }
    if capture.contains("Press up to edit queued messages") || capture.contains("Messages to be submitted after next tool call") {
        return PaneState::Wedged;
    }
    // Dialog adjacency is the classifier's own definition; the model defers to it rather than
    // re-deriving a positional rule (that would be a second parser to keep in sync). Everything
    // BELOW dialog is modelled exactly.
    if tick_monitor::dialog_open(capture) {
        return PaneState::Dialog { timer_secs: parse_timer(last_status_line(capture)).unwrap_or(0) };
    }
    let line = last_status_line(capture);
    let spinner = line.chars().any(tick_monitor::is_braille);
    match (spinner, parse_timer(line)) {
        (true, Some(secs)) => PaneState::Working { timer_secs: secs },
        _ if line.contains('π') => PaneState::Idle,
        _ => PaneState::Unproven,
    }
}

fuzz_target!(|input: Input| {
    let a = render_capture(&input.lines, 0, 0);
    // Contract 1: never panics, on any shape.
    let state = classify(&a);
    // Contract 2: precedence equals the model's.
    assert_eq!(state, precedence_model(&a), "classify diverged from the precedence model on:\n{a}");
    // Contract 2b: a wedged footer anywhere beats a live spinner on the last line.
    if a.contains("Press up to edit queued messages") || a.contains("Messages to be submitted after next tool call") {
        assert!(matches!(state, PaneState::Wedged | PaneState::ProviderError402), "parked packet read as {state:?}");
    }
    // Contract 2c: Unproven is never produced from a last line carrying both a spinner and a
    // lowercase timer (that pair IS the working contract).
    let last = last_status_line(&a);
    if last.chars().any(tick_monitor::is_braille) && parse_timer(last).is_some() {
        assert!(!matches!(state, PaneState::Unproven), "spinner+timer read as Unproven: {last:?}");
    }
    // Contract 3: stable_hash ignores animation — bump every spinner frame and every timer
    // and the hash must not move. (Prose and borders untouched, so content is identical.)
    let b = render_capture(&input.lines, input.bump_spinner, input.bump_timer);
    assert_eq!(stable_hash(&a), stable_hash(&b), "animation leaked into stable_hash:\n{a}\n---\n{b}");
    // Contract 4: parse_timer never accepts an uppercase unit token on its own.
    for tok in ["12M", "3H", "5S", "1.3M", "S0.25"] {
        assert_eq!(parse_timer(tok), None, "uppercase unit accepted: {tok}");
    }
    // Contract 4b: a lowercase timer token parses to its own seconds, whatever surrounds it.
    if let Some(Fragment::Timer { n, unit }) = input.lines.first().and_then(|l| l.first()) {
        let u = ['s', 'm', 'h', 'S', 'M', 'H', 'x', 'd'][usize::from(*unit % 8)];
        let expected = match u { 's' => Some(u64::from(*n)), 'm' => Some(u64::from(*n) * 60), 'h' => Some(u64::from(*n) * 3600), _ => None };
        assert_eq!(parse_timer(&format!("{n}{u}")), expected, "parse_timer({n}{u})");
    }
});

#![forbid(unsafe_code)]

//! Proptest floor for classify precedence: 402 > Wedged > Dialog > spinner+timer > π > Unproven.
//! Restates `fuzz/fuzz_targets/tick_monitor_classify.rs`.

use proptest::prelude::*;
use tick_monitor::{
    classify, dialog_open, is_braille, last_status_line, parse_timer, stable_hash, PaneState,
};

const MAX_LINES: usize = 8;

#[derive(Debug, Clone)]
enum Fragment {
    Spinner(u8),
    Timer { n: u16, unit: u8 },
    Pi,
    WedgedFooter(bool),
    DialogFooter(u8),
    Provider402(bool),
    Border,
    Prose,
}

fn render(f: &Fragment) -> String {
    match f {
        Fragment::Spinner(i) => char::from_u32(0x2800 + u32::from(*i) % 256)
            .map(|c| c.to_string())
            .unwrap_or_default(),
        Fragment::Timer { n, unit } => {
            format!(
                "{n}{}",
                ['s', 'm', 'h', 'S', 'M', 'H', 'x', 'd'][usize::from(*unit % 8)]
            )
        }
        Fragment::Pi => "π".to_owned(),
        Fragment::WedgedFooter(alt) => {
            if *alt {
                "Messages to be submitted after next tool call".to_owned()
            } else {
                "Press up to edit queued messages".to_owned()
            }
        }
        Fragment::DialogFooter(i) => {
            ["│ Enter select", "│ Esc cancel", "│ ↑/↓ move", "Esc cancel"][usize::from(*i % 4)]
                .to_owned()
        }
        Fragment::Provider402(cross) => {
            if *cross {
                "✘ 402 This request requires more credits, or fewer max_tokens".to_owned()
            } else {
                "Error: 402 This request requires more credits, or fewer max_tokens".to_owned()
            }
        }
        Fragment::Border => "─────────────".to_owned(),
        Fragment::Prose => "status".to_owned(),
    }
}

fn fragment_strategy() -> impl Strategy<Value = Fragment> {
    prop_oneof![
        (0u8..=40).prop_map(Fragment::Spinner),
        (0u16..120, 0u8..8).prop_map(|(n, unit)| Fragment::Timer { n, unit }),
        Just(Fragment::Pi),
        proptest::bool::ANY.prop_map(Fragment::WedgedFooter),
        (0u8..4).prop_map(Fragment::DialogFooter),
        proptest::bool::ANY.prop_map(Fragment::Provider402),
        Just(Fragment::Border),
        Just(Fragment::Prose),
    ]
}

fn render_capture(lines: &[Vec<Fragment>], spin_bump: u8, timer_bump: u16) -> String {
    let mut out = String::new();
    for line in lines.iter().take(MAX_LINES) {
        for f in line {
            let rendered = match f {
                Fragment::Spinner(i) => render(&Fragment::Spinner(i.wrapping_add(spin_bump))),
                Fragment::Timer { n, unit }
                    if matches!(
                        ['s', 'm', 'h', 'S', 'M', 'H', 'x', 'd'][usize::from(*unit % 8)],
                        's' | 'm' | 'h'
                    ) =>
                {
                    render(&Fragment::Timer {
                        n: n.saturating_add(timer_bump),
                        unit: *unit,
                    })
                }
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
    if capture.contains("Press up to edit queued messages")
        || capture.contains("Messages to be submitted after next tool call")
    {
        return PaneState::Wedged;
    }
    if dialog_open(capture) {
        return PaneState::Dialog {
            timer_secs: parse_timer(last_status_line(capture)).unwrap_or(0),
        };
    }
    let line = last_status_line(capture);
    let spinner = line.chars().any(is_braille);
    match (spinner, parse_timer(line)) {
        (true, Some(secs)) => PaneState::Working { timer_secs: secs },
        _ if line.contains('π') => PaneState::Idle,
        _ => PaneState::Unproven,
    }
}

proptest! {
    #[test]
    fn classify_precedence_is_total_and_stable_hash_ignores_animation(
        lines in prop::collection::vec(
            prop::collection::vec(fragment_strategy(), 0..=4),
            0..=MAX_LINES,
        ),
        bump_spinner in 0u8..8,
        bump_timer in 0u16..8,
    ) {
        let a = render_capture(&lines, 0, 0);
        let state = classify(&a);
        prop_assert_eq!(state.clone(), precedence_model(&a));
        if a.contains("Press up to edit queued messages")
            || a.contains("Messages to be submitted after next tool call")
        {
            prop_assert!(matches!(state, PaneState::Wedged | PaneState::ProviderError402));
        }
        let last = last_status_line(&a);
        if last.chars().any(is_braille) && parse_timer(last).is_some() {
            prop_assert!(!matches!(state, PaneState::Unproven));
        }
        let b = render_capture(&lines, bump_spinner, bump_timer);
        prop_assert_eq!(stable_hash(&a), stable_hash(&b));
        for tok in ["12M", "3H", "5S", "1.3M", "S0.25"] {
            prop_assert_eq!(parse_timer(tok), None);
        }
    }
}

#[test]
fn planted_402_beats_spinner() {
    let capture = "Error: 402 This request requires more credits, or fewer max_tokens\n⠙ 4s\n";
    assert_eq!(classify(capture), PaneState::ProviderError402);
}

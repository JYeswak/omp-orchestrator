//! Behaviour-preserving legs for `apply_composer_rc` / `missing_composer` /
//! the prompt-marker clause inside `classify`.
//!
//! These are pub in lib.rs but had ZERO tests naming them. A Free pane whose
//! composer discriminator is missing or returns a non-0/1 rc is fail-closed
//! BUSY — the silent-skip direction. Pin that without changing the functions.

use pane_dispatch_ready::{apply_composer_rc, classify, missing_composer, PaneDispatchReadyRules, PaneDispatchReadyState, PaneDispatchReadyVerdict};

fn free_prompt() -> PaneDispatchReadyVerdict {
    classify("Opus 5 │ bypass permissions\n❯ ", false, &PaneDispatchReadyRules::default())
}

#[test]
fn apply_composer_rc_typed_text_is_busy() {
    let v = apply_composer_rc(free_prompt(), 0, "bin/composer-typed.py", &PaneDispatchReadyRules::default());
    assert_eq!(v.state, PaneDispatchReadyState::Busy, "rc 0 means typed composer, not ours");
}

#[test]
fn apply_composer_rc_empty_composer_keeps_free() {
    let v = apply_composer_rc(free_prompt(), 1, "bin/composer-typed.py", &PaneDispatchReadyRules::default());
    assert_eq!(
        v.state,
        PaneDispatchReadyState::Free,
        "rc 1 means empty composer, still FREE"
    );
}

#[test]
fn apply_composer_rc_unknown_rc_is_fail_closed_busy() {
    let v = apply_composer_rc(free_prompt(), 2, "bin/composer-typed.py", &PaneDispatchReadyRules::default());
    assert_eq!(
        v.state,
        PaneDispatchReadyState::Busy,
        "unknown composer rc must not admit a dispatch"
    );
}

#[test]
fn apply_composer_rc_does_not_override_a_busy_verdict() {
    let busy = classify(
        "Opus 5 │ bypass permissions\n• Working (12s • esc to interrupt)\n❯ ",
        false,
        &PaneDispatchReadyRules::default(),
    );
    assert_eq!(busy.state, PaneDispatchReadyState::Busy);
    let v = apply_composer_rc(busy, 1, "bin/composer-typed.py", &PaneDispatchReadyRules::default());
    assert_eq!(
        v.state,
        PaneDispatchReadyState::Busy,
        "an empty composer cannot un-busy a working pane"
    );
}

#[test]
fn missing_composer_is_fail_closed_busy() {
    let v = missing_composer("/no/such/composer-typed.py");
    assert_eq!(v.state, PaneDispatchReadyState::Busy);
    assert!(
        v.reason.contains("missing"),
        "reason must name the missing discriminator, got {:?}",
        v.reason
    );
}

/// `>` with no following space is NOT a prompt (POSIX grep requires the space).
/// A more-permissive marker would FREE panes the live callers currently refuse.
#[test]
fn bare_gt_without_space_is_not_a_prompt() {
    let v = classify("Opus 5 │ bypass permissions\n>", false, &PaneDispatchReadyRules::default());
    assert_ne!(
        v.state,
        PaneDispatchReadyState::Free,
        "bare '>' without a following space must not classify FREE"
    );
}

#[test]
fn gt_with_space_is_a_prompt_and_can_be_free() {
    let v = classify("Opus 5 │ bypass permissions\n> ", false, &PaneDispatchReadyRules::default());
    assert_eq!(v.state, PaneDispatchReadyState::Free);
}

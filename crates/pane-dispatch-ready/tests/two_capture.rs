//! K5 acceptance 6: two-capture liveness must be time-bounded and motion-backed.

#![forbid(unsafe_code)]

use omp_types::{CaptureSnapshot, PaneLiveness, MIN_TWO_CAPTURE_INTERVAL_SECS};
use pane_dispatch_ready::{classify, confirm_free, PaneDispatchReadyRules, PaneDispatchReadyState};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::fs;

const IDLE_PROMPT: &str = "Opus 5 │ bypass permissions\n❯ ";

fn snapshot(at_secs: u64, timer: &str, content_hash: &str) -> CaptureSnapshot {
    CaptureSnapshot::new(
        at_secs,
        PaneLiveness::Idle,
        Some(timer.to_owned()),
        content_hash.to_owned(),
    )
}

fn confirm(
    first_at_secs: u64,
    second_at_secs: u64,
    first_timer: &str,
    second_timer: &str,
    first_hash: &str,
    second_hash: &str,
    rules: &PaneDispatchReadyRules,
) -> pane_dispatch_ready::PaneDispatchReadyVerdict {
    let first = classify(IDLE_PROMPT, false, rules);
    assert_eq!(first.state, PaneDispatchReadyState::Free);
    confirm_free(
        first,
        IDLE_PROMPT,
        snapshot(first_at_secs, first_timer, first_hash),
        snapshot(second_at_secs, second_timer, second_hash),
        rules,
    )
}
fn sha256_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn replace_once(bytes: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
    let position = bytes
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("interval guard source needle must exist");
    let mut output = Vec::with_capacity(bytes.len() + replacement.len() - needle.len());
    output.extend_from_slice(&bytes[..position]);
    output.extend_from_slice(replacement);
    output.extend_from_slice(&bytes[position + needle.len()..]);
    output
}

#[test]
fn zero_interval_frozen_payload_is_unreadable_and_text_named() {
    assert_eq!(MIN_TWO_CAPTURE_INTERVAL_SECS, 75);
    let verdict = confirm(100, 100, "27s", "27s", "same", "same", &rules());
    println!("{}", verdict.pipe_line());
    assert_eq!(verdict.state, PaneDispatchReadyState::Unreadable);
    assert!(verdict.reason.contains("TWO_CAPTURE_UNPROVEN"));
    assert!(verdict.reason.contains("0 seconds"));
    assert!(verdict.pipe_line().contains("TWO_CAPTURE_UNPROVEN"));
}

#[test]
fn insufficient_interval_refuses_even_when_timer_moves() {
    let verdict = confirm(100, 174, "27s", "28s", "same", "same", &rules());
    println!("{}", verdict.pipe_line());
    assert_eq!(verdict.state, PaneDispatchReadyState::Unreadable);
    assert!(verdict.reason.contains("74 seconds"));
    assert!(verdict.reason.contains("75 seconds"));
}

#[test]
fn minimum_interval_with_timer_motion_is_busy() {
    let verdict = confirm(100, 175, "27s", "28s", "same", "same", &rules());
    println!("{}", verdict.pipe_line());
    assert_eq!(verdict.state, PaneDispatchReadyState::Busy);
}

#[test]
fn minimum_interval_with_spinner_stripped_content_motion_is_busy() {
    let verdict = confirm(100, 175, "27s", "27s", "hash-one", "hash-two", &rules());
    println!("{}", verdict.pipe_line());
    assert_eq!(verdict.state, PaneDispatchReadyState::Busy);
}

#[test]
fn interval_guard_mutation_restores_source_byte_identically() {
    let before = include_bytes!("../src/lib.rs").to_vec();
    let before_sha = sha256_hex(&before);
    let mutated = replace_once(
        &before,
        b"if !rules.two_capture_interval {",
        b"if false                         {",
    );
    let mutated_sha = sha256_hex(&mutated);
    assert_ne!(before_sha, mutated_sha);

    let path = std::env::temp_dir().join(format!(
        "pane-dispatch-ready-two-capture-{}",
        std::process::id()
    ));
    fs::write(&path, &mutated).expect("write mutated source fixture");
    let mut mutated_rules = rules();
    assert!(mutated_rules.disable("two_capture_interval"));
    let mutation_verdict = confirm(100, 100, "27s", "27s", "same", "same", &mutated_rules);
    assert_eq!(mutation_verdict.state, PaneDispatchReadyState::Free);

    fs::write(&path, &before).expect("restore source fixture");
    let after = fs::read(&path).expect("read restored source fixture");
    let after_sha = sha256_hex(&after);
    println!(
        "MUTATION RED interval_guard before={before_sha} mutated={mutated_sha} restored={after_sha} byte_identical={}",
        before == after
    );
    assert_eq!(
        before, after,
        "interval guard restore must be byte-identical"
    );
    assert_eq!(
        before_sha, after_sha,
        "interval guard restore digest must match"
    );
    fs::remove_file(path).expect("remove source fixture");
}
#[test]
fn mutation_disabling_interval_check_reopens_frozen_payload() {
    let mut mutated_rules = rules();
    assert!(mutated_rules.disable("two_capture_interval"));
    let mutated = confirm(100, 100, "27s", "27s", "same", "same", &mutated_rules);
    println!(
        "MUTATION two_capture_interval disabled -> {}",
        mutated.pipe_line()
    );
    assert_eq!(mutated.state, PaneDispatchReadyState::Free);

    let restored = confirm(100, 100, "27s", "27s", "same", "same", &rules());
    println!("MUTATION RESTORED -> {}", restored.pipe_line());
    assert_eq!(restored.state, PaneDispatchReadyState::Unreadable);
}

fn rules() -> PaneDispatchReadyRules {
    PaneDispatchReadyRules::default()
}

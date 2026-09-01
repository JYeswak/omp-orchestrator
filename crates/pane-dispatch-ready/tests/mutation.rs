//! Fires-on-known-bad mutation legs. Each names its rule on a column-0 RED line.
//! A nonzero exit alone is not evidence.

use pane_dispatch_ready::{classify, confirm_free, PaneDispatchReadyRules, PaneDispatchReadyState};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_pane-dispatch-ready"))
}

fn composer() -> PathBuf {
    if let Some(path) = std::env::var_os("COMPOSER_TYPED") {
        return PathBuf::from(path);
    }
    let output = Command::new("which")
        .arg("composer-typed")
        .output()
        .expect("which composer-typed");
    let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    PathBuf::from(path)
}

fn eval_bin(args: &[&str], input: &str) -> String {
    let mut child = Command::new(rust_bin())
        .args(args)
        .env("COMPOSER_TYPED", composer())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn state_of(line: &str) -> &str {
    line.split('|').next().unwrap_or("")
}

/// pdr-001: two captures. A hash change is BUSY; deleting the rule treats a
/// generating pane as FREE — the frozen/Working (27s) fusion.
#[test]
fn mutation_two_capture_liveness() {
    let prompt = "Opus 5 │ bypass permissions\n❯ ";
    let mut off = PaneDispatchReadyRules::default();
    assert!(off.disable("two_capture_liveness"));
    let first_off = classify(prompt, false, &off);
    let v_off = confirm_free(first_off, prompt, "aaa", "bbb", &off);
    assert_eq!(
        v_off.state,
        PaneDispatchReadyState::Free,
        "disabled two_capture_liveness must keep the first FREE, got {:?}",
        v_off.state
    );
    println!(
        "MUTATION two_capture_liveness disabled -> FREE (single capture treats a generating pane as free)"
    );

    let on = PaneDispatchReadyRules::default();
    let first_on = classify(prompt, false, &on);
    let v_on = confirm_free(first_on, prompt, "aaa", "bbb", &on);
    assert_eq!(
        v_on.state,
        PaneDispatchReadyState::Busy,
        "rule two_capture_liveness: hash change is BUSY, got {:?}",
        v_on.state
    );
    println!(
        "MUTATION RED two_capture_liveness: BUSY (hash change across captures; Working (Ns) vs frozen cannot pass on one sample)"
    );
}

/// pdr-002: busy markers are the only thing standing between a timer+prompt
/// pane and FREE. Deleting them is a double-dispatch.
#[test]
fn mutation_busy_markers_load_bearing() {
    let input =
        "Opus 5 (1M context) │ bypass permissions\n• Working (38m 29s • esc to interrupt)\n❯ ";
    let off = eval_bin(
        &[
            "--eval",
            "--mutation",
            "--disable-rule",
            "busy_markers_load_bearing",
        ],
        input,
    );
    assert_eq!(
        state_of(&off),
        "FREE",
        "disabled busy markers must let the prompt through, got {off}"
    );
    println!("MUTATION busy_markers_load_bearing disabled -> FREE (timer+prompt classified free)");

    let on = eval_bin(&["--eval"], input);
    assert_eq!(
        state_of(&on),
        "BUSY",
        "rule busy_markers_load_bearing: timer+prompt is BUSY, got {on}"
    );
    println!("MUTATION RED busy_markers_load_bearing: BUSY (Working (Ns) blocks FREE)");
}

/// pdr-003: quota is a spend decision, not BUSY. Deleting the pre-busy check
/// would send the operator to debug the wrong thing AND could FREE a pane
/// that cannot execute.
#[test]
fn mutation_quota_before_busy() {
    let input = "  Opus 5 (1M context) | control-plane\n■ You've hit your usage limit. try again later.\n❯ ";
    let off = eval_bin(
        &[
            "--eval",
            "--mutation",
            "--disable-rule",
            "quota_before_busy",
        ],
        input,
    );
    assert_ne!(
        state_of(&off),
        "QUOTA_BLOCKED",
        "disabled quota_before_busy must not report QUOTA_BLOCKED, got {off}"
    );
    println!(
        "MUTATION quota_before_busy disabled -> {} (quota banner no longer a spend decision)",
        state_of(&off)
    );

    let on = eval_bin(&["--eval"], input);
    assert_eq!(
        state_of(&on),
        "QUOTA_BLOCKED",
        "rule quota_before_busy: exhausted pane is QUOTA_BLOCKED, got {on}"
    );
    println!("MUTATION RED quota_before_busy: QUOTA_BLOCKED (needs spend, not a dispatch)");
}

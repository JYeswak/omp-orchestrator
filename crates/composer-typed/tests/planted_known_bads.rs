//! REAL pane capture + planted typed/dim lines, through the top-level binary.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_composer-typed"))
}

fn rc(input: &str) -> i32 {
    let mut child = Command::new(rust_bin())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("top-level binary");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait().unwrap().code().unwrap_or(99)
}

#[test]
fn planted_real_capture_goes_through_binary() {
    let fx = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/codex-generating-busy-at-line64.txt");
    let body = std::fs::read_to_string(&fx).expect("real capture fixture");
    let got = rc(&body);
    assert!(
        got == 0 || got == 1,
        "binary must return the oracle's 0/1, got {got}"
    );
    println!("PLANTED real codex capture -> rc={got} via CARGO_BIN_EXE");
}

#[test]
fn planted_typed_line_is_typed_through_binary() {
    let body = "  Opus 5 (1M context) | control-plane\n❯ bought credits - resume the fleet\n";
    assert_eq!(rc(body), 0, "planted typed line must be TYPED");
    println!("PLANTED typed operator text -> rc=0 via CARGO_BIN_EXE");
}

#[test]
fn planted_dim_suggestion_is_free_through_binary() {
    let esc = "\u{1b}";
    let body = format!("  Opus 5\n{esc}[39m❯ {esc}[2ma suggestion{esc}[0m\n");
    assert_eq!(rc(&body), 1, "planted dim suggestion must be FREE");
    println!("PLANTED dim autosuggestion -> rc=1 via CARGO_BIN_EXE");
}

/// 6q5 FIRES-ON-KNOWN-BAD: an empty composer must classify as NOT typed.
/// The dispatch path calls is_typed() on the captured pane content; if the
/// composer is empty (the packet never arrived), is_typed must return false
/// so the dispatcher classifies it as non-delivery rather than success.
/// This test pins the fail-closed-on-empty behavior that the COMPOSER_EMPTY
/// error in omp-orchestrator's send_and_verify depends on.
#[test]
fn empty_composer_is_not_typed() {
    let rules = composer_typed::Rules::default();
    assert!(
        !composer_typed::is_typed("", &rules),
        "an empty composer must classify as not-typed — the dispatch path depends on this to detect non-delivery"
    );
}

/// 6q5: a composer holding the dispatched objective line IS typed.
/// Proves the positive case the dispatch path relies on.
#[test]
fn dispatched_objective_line_is_typed() {
    let rules = composer_typed::Rules::default();
    let objective = "Opus 5 │ bypass permissions\n❯ Objective: complete bead omp-orchestrator-6q5. Target repository: /tmp/test";
    assert!(
        composer_typed::is_typed(objective, &rules),
        "a dispatched objective line must classify as typed"
    );
}

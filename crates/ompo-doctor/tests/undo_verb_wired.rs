#![forbid(unsafe_code)]

//! f3maq's remaining half: `undo` was DECLARED, COMPILED and TESTED, and still
//! not reachable from the binary.
//!
//! The three lib-level parity legs (`compare_verb_parity`,
//! `every_verb_appears_in_usage`) couple `VERBS`, the usage text and the verb
//! table to each other — but all three are SOURCE facts. None of them launches
//! the binary, so all three were green while `ompo undo` exited as an unknown
//! verb. That is the built-not-wired shape at binary granularity, and the only
//! instrument that sees it is running the shipped binary.
//!
//! # The discriminator
//!
//! Before the verb table arm existed, `undo` fell through to the unknown-verb
//! arm, which prints `UAD_UNKNOWN_VERB`. So the known-bad is not "a nonzero
//! exit" — `undo` with no scope is ALSO a nonzero exit, and correctly so. The
//! discriminator is WHICH refusal: an unknown-verb refusal names the verb set,
//! a real `undo` refusal names a scope.

use std::process::Command;

fn run(args: &[&str]) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ompo"))
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("ANTI-VACUITY: ompo did not execute: {error}"));
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    (output.status.code(), text)
}

/// The verb is REACHABLE: `ompo undo --help` succeeds and prints undo's own
/// usage, which no other verb emits.
#[test]
fn the_undo_verb_is_reachable_from_the_shipped_binary() {
    let (code, text) = run(&["undo", "--help"]);
    println!("READBACK undo --help exit={code:?}");
    assert!(
        !text.contains("UAD_UNKNOWN_VERB"),
        "undo is still an unknown verb to the binary: {text}"
    );
    assert_eq!(code, Some(0), "undo --help must succeed: {text}");
    assert!(
        text.contains("ompo undo <scope>"),
        "undo's own usage must be what answered: {text}"
    );
    // undo's stated law, printed by its own help. If this line ever moves, the
    // help being served is not undo's.
    assert!(
        text.contains("--dry-run"),
        "undo's help must state its default: {text}"
    );
}

/// A REAL undo refusal, not an unknown-verb refusal. Both are nonzero exits, so
/// the exit code alone cannot tell "the verb is unwired" from "the verb ran and
/// refused for cause" — the same exit-code-is-not-a-cause trap 8nuh's premise
/// was built on.
#[test]
fn a_bare_undo_refuses_as_undo_and_not_as_an_unknown_verb() {
    let (code, text) = run(&["undo"]);
    println!("READBACK bare undo exit={code:?}");
    assert!(
        !text.contains("UAD_UNKNOWN_VERB"),
        "a bare undo must refuse AS undo: {text}"
    );
    assert!(
        code.is_some_and(|code| code != 0),
        "a bare undo names no scope and must refuse: {text}"
    );
    assert!(
        text.contains("undo"),
        "the refusal must name what refused: {text}"
    );
}

/// KNOWN-GOOD CONTROL, so the legs above cannot pass against a binary that
/// accepts everything: a genuinely absent verb STILL refuses as unknown, and
/// the unknown-verb path is therefore still live and still the fallthrough.
#[test]
fn a_genuinely_unknown_verb_still_refuses_as_unknown() {
    let (code, text) = run(&["definitely-not-a-verb"]);
    assert!(
        text.contains("UAD_UNKNOWN_VERB"),
        "the unknown-verb arm must remain the single authority: {text}"
    );
    assert!(code.is_some_and(|code| code != 0), "{text}");
    // And it must advertise the verb set that now contains undo, which is what
    // ties this control back to the wiring under test.
    assert!(
        text.contains("undo"),
        "the advertised verb set must include the newly wired verb: {text}"
    );
}

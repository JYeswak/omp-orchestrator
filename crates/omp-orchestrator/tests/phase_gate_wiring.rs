//! `omp-orchestrator-block-non-arc-behind-s0-f3g5` item 8: the phase gate has a PRODUCTION CALLER
//! on the supervisor's dispatch path, and it is REACHED.
//!
//! # Why this is a source-predicate proof and what that costs
//!
//! `apply_phase_gate`'s only production caller sits inside `run_cycle`, which is invoked from the
//! supervisor loop at `main.rs:6045`. `HD-0016` stopped that supervisor, so a live `PHASE_GATE`
//! heartbeat row is **unobtainable today** — not because the caller is unreachable, but because the
//! process that reaches it is deliberately down. Item 8 is explicit that a compiling call site is
//! not a wiring proof (`blocker-taxonomy`: 662 LOC, 19 green tests, zero callers), so this target
//! follows the in-tree convention for exactly this situation — `tests/gate_wiring_wave2.rs` and
//! `tests/gate_wiring_wave3.rs` — and proves what is provable while naming what is not.
//!
//! **What it proves:** the call exists on the dispatch candidate path, between the ranked order and
//! the queue observation that consumes it; it is wired to the OFF constant rather than a literal;
//! its outcome is written to the heartbeat so the row is observable the moment the dispatcher runs;
//! and the refusal arm returns an error rather than falling through to dispatch.
//!
//! **What it does not prove:** that a tick has executed it. That is the live row, and it arrives
//! with the restart.

use std::path::Path;
use text_structure::code_only;

fn supervisor_source() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main.rs");
    std::fs::read_to_string(&path).expect("the supervisor source must be readable")
}

// HANDROLLED STRIPPER REMOVED 2026-09-07 -- `text-structure::code_only` already existed, and
// `no-shell-gate/tests/text_structure_lint.rs` names this exact file for it:
//   RAW_TEXT_MATCH_IN_GATE crates/omp-orchestrator/tests/phase_gate_wiring.rs:local matcher
// The local copy compared TWO-CHARACTER STRINGS and, worse, had NO STRING-LITERAL state, so a
// block-comment opener inside any literal in the scanned file would blank the tail and report LESS
// wiring -- indistinguishable from a real absence. The kernel tracks literals (raw strings
// included) and counts nested block comments. KERNEL-ONLY: a handroll is not merely redundant, it
// is usually worse.

/// POSITIVE CONTROL FIRST, and it is now a control on the SHARED kernel rather than on a local
/// copy. A needle that cannot match returns zero from a working probe and from a broken one, so
/// every assertion below is meaningless until the stripper is shown to preserve code AND to remove
/// comment text. This is the standing rule — point the instrument at a guaranteed-absent subject
/// and read what it returns — expressed as a test.
#[test]
fn the_comment_stripper_preserves_code_and_removes_comments() {
    let source = supervisor_source();
    let code = code_only(&source);
    assert!(
        code.contains("async fn run_cycle"),
        "the stripper must preserve real code — positive control failed"
    );
    assert!(
        !code.contains("FALSE IS AN OBSERVATION"),
        "the stripper must remove comment text; that phrase exists only in a comment"
    );
    assert!(
        source.contains("FALSE IS AN OBSERVATION"),
        "…and it must exist in the unstripped source, or the negative control is vacuous"
    );
}

/// ITEM 8 — the caller exists in CODE, not in a comment, and is wired to the OFF constant.
#[test]
fn the_dispatch_path_calls_the_phase_gate_with_the_shipped_constant() {
    let source = supervisor_source();
    let code = code_only(&source);
    assert!(
        code.contains("loop_queue_filter::phase_gate::apply_phase_gate"),
        "no production call to apply_phase_gate survives comment stripping"
    );
    // Wired to the CONSTANT. A literal `false` would land the same behaviour today and silently
    // diverge the moment the shipped default changes.
    assert!(
        code.contains("loop_queue_filter::phase_gate::PHASE_GATE_ENABLED"),
        "the caller must pass the shipped constant, never a literal"
    );
    assert!(
        !code.contains("apply_phase_gate(\n        &bead_ids,\n        arc_census_queue_scoped,\n        false,\n        true,"),
        "the caller must not force the gate on"
    );
}

/// ITEM 8 — the outcome is OBSERVABLE. `gate_active=false` is the fact that distinguishes a
/// disabled gate from a released phase, and it reaches the heartbeat rather than only stdout.
#[test]
fn the_gate_outcome_is_written_to_the_heartbeat() {
    let source = supervisor_source();
    let code = code_only(&source);
    for status in [
        "\"PHASE_GATE\"",
        "\"PHASE_GATE_WITHHELD\"",
        "\"PHASE_GATE_REFUSED\"",
    ] {
        assert!(
            code.contains(status),
            "{status} must be written to the heartbeat so the row is observable on the first tick"
        );
    }
    assert!(
        code.contains("write_heartbeat(config, tick, \"PHASE_GATE\""),
        "the decision row must go through write_heartbeat, not println alone — stdout is not a \
         durable observation"
    );
}

/// ITEM 8 — the REFUSAL arm must not fall through to dispatch. A vacuity refusal that returned the
/// unfiltered set would convert the gate's own anti-vacuity codes into a silent passthrough.
#[test]
fn a_gate_refusal_returns_an_error_rather_than_dispatching() {
    let source = supervisor_source();
    let code = code_only(&source);
    let code: &str = code.as_ref();
    let refusal = code
        .find("PHASE_GATE_REFUSED")
        .expect("the refusal arm must exist");
    let mut end = (refusal + 400).min(code.len());
    while !code.is_char_boundary(end) {
        end += 1;
    }
    let tail = &code[refusal..end];
    assert!(
        tail.contains("return Err("),
        "the refusal arm must return an error; found instead: {tail}"
    );
}

/// The caller must sit BETWEEN the ranked order and the queue observation that consumes it —
/// otherwise it filters a set nobody dispatches, which is a call site rather than a gate.
#[test]
fn the_caller_precedes_the_queue_observation_that_consumes_the_set() {
    let source = supervisor_source();
    let code = code_only(&source);
    let gate = code
        .find("apply_phase_gate")
        .expect("apply_phase_gate call site");
    let observation = code
        .find("observation.queue = QueueState")
        .expect("the queue observation that consumes the candidate set");
    assert!(
        gate < observation,
        "the gate must run BEFORE the candidate set is observed and dispatched: gate at {gate}, \
         observation at {observation}"
    );
}

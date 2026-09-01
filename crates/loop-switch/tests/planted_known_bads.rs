//! PLANTED-KNOWN-BAD FIXTURES for the loop switch.
//!
//! The switch decides whether the entire fleet dispatches, so it can block every unit of work in
//! the system. That makes it load-bearing: it ships its own fixtures, and each one encodes a way
//! the switch could silently betray the operator's intent.
//!
//! The governing requirement (Joshua, 2026-08-27): *"it should stay on until i say so or session
//! resets."* Every test below is a way that promise could break.

use std::path::PathBuf;

use loop_switch::{read_state, status_json, turn_off, turn_on, SwitchState};

/// Each test gets its own switch path so they cannot race. Uses the process id and a distinct
/// label rather than a shared temp name -- two tests sharing a switch file would make one of them
/// pass for the wrong reason.
fn scratch(label: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("loop-switch-test-{}-{label}", std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

/// THE DEFAULT MUST BE ON. If a fresh machine, a cleared state directory, or a new session read
/// as OFF, the fleet would sit idle with nobody having asked it to -- the exact complaint this
/// switch answers.
#[test]
fn a_missing_switch_file_means_the_loop_is_on() {
    let p = scratch("missing");
    assert!(!p.exists(), "fixture must start with no switch file");
    assert_eq!(read_state(&p), SwitchState::On);
    assert!(read_state(&p).is_on());
}

/// PLANTED KNOWN-BAD: the operator turned the loop off. It must STAY off across independent
/// reads -- a switch that forgets is worse than no switch, because the operator believes the
/// fleet is stopped while it dispatches.
#[test]
fn off_persists_across_reads_and_keeps_the_reason() {
    let p = scratch("persist");
    turn_off(&p, "joshua stopped the fleet for a demo").expect("write switch");
    for _ in 0..3 {
        match read_state(&p) {
            SwitchState::Off { reason } => {
                assert!(
                    reason.contains("joshua stopped the fleet for a demo"),
                    "the recorded reason must survive: {reason}"
                );
            }
            SwitchState::On => panic!("switch forgot it was OFF between reads"),
        }
    }
    let _ = std::fs::remove_file(&p);
}

/// An empty reason must still be OFF. The state lives in the file's EXISTENCE, never its content;
/// a switch that read empty-as-on would turn the fleet back on behind the operator.
#[test]
fn an_empty_reason_is_still_off() {
    let p = scratch("empty");
    std::fs::write(&p, "").expect("write empty switch");
    assert!(
        !read_state(&p).is_on(),
        "an empty switch file must still stop the loop -- existence is the state"
    );
    let _ = std::fs::remove_file(&p);
}

/// `on` must be idempotent. If resuming a loop that is already running errored, an operator
/// recovering a stuck fleet would be one confusing failure away from leaving it stopped.
#[test]
fn turning_on_an_already_on_loop_succeeds() {
    let p = scratch("idempotent");
    turn_on(&p).expect("turning on a loop that is already on must succeed");
    assert!(read_state(&p).is_on());
}

/// The full round trip, which is what the operator actually does.
#[test]
fn off_then_on_returns_to_running() {
    let p = scratch("roundtrip");
    turn_off(&p, "maintenance").expect("off");
    assert!(!read_state(&p).is_on(), "must be OFF after `off`");
    turn_on(&p).expect("on");
    assert!(read_state(&p).is_on(), "must be ON after `on`");
    assert!(!p.exists(), "`on` must actually remove the file, not just report success");
}

/// The status payload is read by other lanes; its schema and its `running` bit must agree with
/// the state. A status that said `running: true` while OFF would let a lane dispatch into a fleet
/// the operator had stopped.
#[test]
fn status_json_agrees_with_the_state_in_both_directions() {
    let p = scratch("status");
    let on = status_json(&p);
    assert_eq!(on["schema"], "zs.loop-switch.v1");
    assert_eq!(on["running"], true);
    assert_eq!(on["state"], "ON");
    assert!(on["reason"].is_null(), "an ON loop has no stop reason");

    turn_off(&p, "because I said so").expect("off");
    let off = status_json(&p);
    assert_eq!(off["running"], false);
    assert_eq!(off["state"], "OFF");
    assert!(
        off["reason"].as_str().unwrap().contains("because I said so"),
        "status must carry the reason the operator recorded"
    );
    let _ = std::fs::remove_file(&p);
}

/// The switch records WHEN it was thrown. Without a timestamp, a stale switch found weeks later
/// is indistinguishable from one thrown a minute ago, and nobody dares turn the fleet back on.
#[test]
fn the_recorded_reason_carries_a_timestamp() {
    let p = scratch("stamp");
    turn_off(&p, "stamped").expect("off");
    let SwitchState::Off { reason } = read_state(&p) else {
        panic!("must be OFF");
    };
    assert!(
        reason.starts_with("20") && reason.contains('T'),
        "the reason must begin with an RFC3339 stamp so staleness is readable: {reason}"
    );
    let _ = std::fs::remove_file(&p);
}

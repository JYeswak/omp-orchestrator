//! A NON-FILE AT THE SWITCH PATH IS NOT A SWITCH.
//!
//! Measured 2026-09-02 (skill-loop pass 5, rigor-atlas): `read_state` gated on `Path::exists()`,
//! which is true for a DIRECTORY. `read_to_string` on a directory errors, and that error was
//! classified as "switch file present but unreadable; honouring the stop" -- so
//! `mkdir ~/.local/state/flywheel/loop-switch.off`, with no human intent and no `turn_off`, turned
//! the whole fleet OFF. Only `turn_off` (or a human) writes a regular FILE there; nothing that
//! expresses stop-intent creates a directory. The header meanwhile claimed the switch "cannot fail
//! INTO the off state" -- refuted by its own code, in two places.
//!
//! Known-bad leg: a directory at the path must read ON. Known-good leg: a regular file at the
//! path still reads OFF, so the fix does not weaken the stop.

use loop_switch::{read_state, status_json, SwitchState};
use std::path::PathBuf;

fn scratch(label: &str) -> PathBuf {
    let base = std::env::var_os("ZS_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join(format!("loop-switch-non-file-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

#[test]
fn a_directory_at_the_switch_path_is_not_a_stop() {
    let dir = scratch("dir");
    let p = dir.join("loop-switch.off");
    std::fs::create_dir_all(&p).expect("plant a directory where the switch file lives");
    assert!(p.exists(), "precondition: the path exists (that is the whole trap)");
    assert!(p.is_dir(), "precondition: it is a directory, not a file");

    let state = read_state(&p);
    assert_eq!(
        state,
        SwitchState::On,
        "a directory expresses no stop-intent; it must read ON, not `present but unreadable`"
    );
    let json = status_json(&p);
    assert_eq!(json["state"], "ON", "status_json must agree: {json}");
}

#[test]
fn a_regular_file_at_the_switch_path_still_stops() {
    // Known-good leg: the fix must not weaken the stop.
    let dir = scratch("file");
    let p = dir.join("loop-switch.off");
    std::fs::write(&p, "2026-09-02T00:00:00Z stopped by hand").expect("plant the switch file");
    match read_state(&p) {
        SwitchState::Off { reason } => assert!(reason.contains("stopped by hand"), "{reason}"),
        SwitchState::On => panic!("a regular switch file must still read OFF"),
    }
}

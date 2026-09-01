#![forbid(unsafe_code)]
//! `loop-switch` — the fleet loop's on/off switch and its status verb.
//!
//! Exit dictionary:
//!   0  the command succeeded; for `status`, the loop is ON
//!   1  usage error
//!   3  `status` succeeded and the loop is OFF (distinguishable without parsing)
//!   4  the switch could not be written or removed

use std::path::PathBuf;
use std::process::ExitCode;

use loop_switch::{read_state, status_json, switch_path, turn_off, turn_on, SwitchState};

const USAGE: &str = "\
loop-switch — turn the fleet dispatch loop on or off, and see which it is

USAGE:
    loop-switch status [--json]     is the loop running? (exit 0 = ON, 3 = OFF)
    loop-switch off \"<reason>\"      stop the loop; the reason is recorded and shown
    loop-switch on                  resume the loop (idempotent)
    loop-switch path                print the switch file path

The switch is a FILE, so it survives crashes, reboots, and context resets. The loop stays in
whatever state you last set until you change it. Default is ON: a missing switch file means
running, so nothing but a deliberate `off` can stop the fleet.

ENV:
    FLEET_LOOP_SWITCH   override the switch path (default: ~/.local/state/flywheel/loop-switch.off)
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path: PathBuf = switch_path();
    let verb = args.first().map(String::as_str).unwrap_or("status");

    match verb {
        "status" => {
            let json = args.iter().any(|a| a == "--json");
            let state = read_state(&path);
            if json {
                println!("{}", status_json(&path));
            } else {
                match &state {
                    SwitchState::On => {
                        println!("loop is ON — dispatch runs on schedule");
                        println!("  switch: {} (absent = on)", path.display());
                    }
                    SwitchState::Off { reason } => {
                        println!("loop is OFF");
                        println!("  reason: {reason}");
                        println!("  switch: {}", path.display());
                        println!("  resume: loop-switch on");
                    }
                }
            }
            if state.is_on() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(3)
            }
        }
        "off" => {
            let reason = args[1..].join(" ");
            match turn_off(&path, &reason) {
                Ok(()) => {
                    println!("loop is now OFF — no lane will dispatch until `loop-switch on`");
                    if let SwitchState::Off { reason } = read_state(&path) {
                        println!("  reason: {reason}");
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("could not write the switch at {}: {e}", path.display());
                    ExitCode::from(4)
                }
            }
        }
        "on" => match turn_on(&path) {
            Ok(()) => {
                // RE-MEASURE THE SUBJECT: an exit code is a claim about a process, not the world.
                match read_state(&path) {
                    SwitchState::On => {
                        println!("loop is now ON — dispatch resumes on the next scheduled tick");
                        ExitCode::SUCCESS
                    }
                    SwitchState::Off { reason } => {
                        eprintln!("removal reported success but the switch is STILL OFF: {reason}");
                        ExitCode::from(4)
                    }
                }
            }
            Err(e) => {
                eprintln!("could not remove the switch at {}: {e}", path.display());
                ExitCode::from(4)
            }
        },
        "path" => {
            println!("{}", path.display());
            ExitCode::SUCCESS
        }
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("unknown verb: {other}\n");
            eprint!("{USAGE}");
            ExitCode::from(1)
        }
    }
}

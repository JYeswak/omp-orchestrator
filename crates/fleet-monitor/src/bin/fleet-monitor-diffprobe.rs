#![forbid(unsafe_code)]
//! Differential probe: exposes the two PURE classifiers so the shell original and the Rust port can
//! be graded over IDENTICAL bytes. Not installed; a test fixture for bin/fleet-monitor-differential.sh.
//!
//! It prints the SHELL'S EXACT OUTPUT FORMAT (`STATE|reason`, `INVOKER proof`) so the comparator
//! diffs verdict strings rather than a translation layer that could mask a divergence.
//!
//! FLEET_MONITOR_DIFFPROBE_MUTATE exists for the anti-vacuity leg (fh C86): it deliberately breaks
//! a classifier so the harness can prove it can SEE a disagreement before trusting that it saw none.

use std::io::Read;

use fleet_monitor::{
    invoker_from_chain, ntm_list_census_line, pane_liveness, parse_ancestor_rows, LivenessState,
};

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let mutate = std::env::var("FLEET_MONITOR_DIFFPROBE_MUTATE").unwrap_or_default();

    match mode.as_str() {
        "liveness" => {
            // The shell captures pane text without a trailing newline; strip exactly one so both
            // sides classify the same bytes.
            let text = input.strip_suffix('\n').unwrap_or(&input);
            if mutate == "drop_ready" {
                // MUTATION: the Ready-footer leg removed. Must change the verdict.
                println!("UNPROVEN|no_ready_or_working_marker");
                return;
            }
            if mutate == "drop_omp" {
                // MUTATION: the OMP prompt/working legs removed. Must change an OMP fixture.
                if text.contains('╰')
                    || text.contains('❯')
                    || text.contains("⟨esc⟩")
                    || text.contains('⎋')
                    || text.contains('⠋')
                    || text.contains('⠹')
                {
                    println!("UNPROVEN|no_ready_or_working_marker");
                    return;
                }
            }
            let l = pane_liveness(text);
            let state = match l.state {
                LivenessState::Live => "LIVE",
                LivenessState::Busy => "BUSY",
                LivenessState::Wedged => "WEDGED",
                LivenessState::Unproven => "UNPROVEN",
            };
            println!("{state}|{}", l.reason);
        }
        "invoker" => {
            let inv = invoker_from_chain(&parse_ancestor_rows(&input));
            println!("{} {}", inv.invoker, inv.proof);
        }
        "census" => {
            let text = input.strip_suffix('\n').unwrap_or(&input);
            if mutate == "drop_empty_scan" {
                println!("OK|listed");
                return;
            }
            println!("{}", ntm_list_census_line(text));
        }
        _ => {
            eprintln!("usage: fleet-monitor-diffprobe liveness|invoker|census  (input on stdin)");
            std::process::exit(2);
        }
    }
}

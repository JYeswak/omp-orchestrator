#![forbid(unsafe_code)]

//! `ntm-fleet-monitor` — classify a proposed fleet action. Does not send.

use ntm_fleet_monitor::{
    classify, classify_named, render_wave, selftest, Approved, Intent, TypedAction, Wave,
    NO_CLAIM_BOUNDARY,
};
use std::process::ExitCode;

const USAGE: &str = "\
ntm-fleet-monitor — typed fleet-loop actions (classifies; does not send)

USAGE:
    ntm-fleet-monitor --selftest
    ntm-fleet-monitor classify --action <kebab> [--pane-dispatchable] [--two-captures] [--packet-complete] [--finding-has-bead]
    ntm-fleet-monitor waves

Waves print one line per default phase action under fail-closed facts (no pane
proven idle, no two captures). That is the NON-APPROVAL / refuse-closed standing
plan. Pass facts to `classify` to open Autonomous waves.
";

fn parse_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name || a == &format!("--{name}"))
}

fn flag_value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == name {
            return it.next().map(String::as_str);
        }
        if let Some(v) = a.strip_prefix(&format!("{name}=")) {
            return Some(v);
        }
    }
    None
}

fn render_authorized(wave: Wave) -> String {
    match Approved::authorize(wave) {
        Ok(approved) => render_wave(approved.wave()),
        Err(_) => render_wave(wave),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--help") | Some("-h") | None => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some("--selftest") => match selftest() {
            Ok(()) => {
                println!("SELFTEST: 0 failure(s)");
                println!("NO-CLAIM: {NO_CLAIM_BOUNDARY}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("SELFTEST FAIL: {e}");
                ExitCode::from(2)
            }
        },
        Some("waves") => {
            let facts = Intent::new(TypedAction::ObserveScan);
            for a in ntm_fleet_monitor::ALL_ACTIONS {
                let w = classify(Intent { action: a, ..facts });
                println!("{}", render_authorized(w));
            }
            ExitCode::SUCCESS
        }
        Some("classify") => {
            let name = match flag_value(&args, "--action") {
                Some(n) => n,
                None => {
                    eprintln!("missing --action");
                    eprint!("{USAGE}");
                    return ExitCode::from(2);
                }
            };
            let intent = Intent {
                action: TypedAction::parse(name).unwrap_or(TypedAction::ObserveScan),
                pane_dispatchable: parse_flag(&args, "--pane-dispatchable"),
                two_captures: parse_flag(&args, "--two-captures"),
                packet_complete: parse_flag(&args, "--packet-complete"),
                finding_has_bead: parse_flag(&args, "--finding-has-bead"),
            };
            let w = classify_named(name, intent);
            println!("{}", render_authorized(w));
            if Approved::authorize(w).is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        _ => {
            eprint!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

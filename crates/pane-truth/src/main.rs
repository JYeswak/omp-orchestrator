#![forbid(unsafe_code)]

use pane_truth::{run_live, run_live_exit_code, selftest, selftest_exit_code, PaneTruthRules};
use std::process::ExitCode;
#[used]
static BUILD_ID_MARKER: &[u8] = concat!("build_id=", env!("OMP_BUILD_ID")).as_bytes();
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--version") {
        println!("pane-truth 0.1.0 build_id={}", env!("OMP_BUILD_ID"));
        return ExitCode::SUCCESS;
    }
    let mut session = "control-plane".to_string();
    let mut self_test = false;
    let mut mutation = false;
    let mut disabled = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--selftest" => self_test = true,
            "--mutation" => mutation = true,
            "--disable-rule" => match args.next() {
                Some(rule) => disabled.push(rule),
                None => {
                    eprintln!("usage error: --disable-rule requires a name");
                    return ExitCode::from(2);
                }
            },
            "-h" | "--help" => {
                eprintln!("usage: pane-truth [session] [--selftest]");
                return ExitCode::SUCCESS;
            }
            value if value.starts_with('-') => {
                eprintln!("usage error: unknown argument {value}");
                return ExitCode::from(2);
            }
            value => session = value.to_string(),
        }
    }
    if !disabled.is_empty() && !mutation {
        eprintln!("usage error: --disable-rule requires --mutation");
        return ExitCode::from(2);
    }
    let mut rules = PaneTruthRules::default();
    for name in disabled {
        if !rules.disable(&name) {
            eprintln!(
                "usage error: unknown rule {name}; known: {}",
                PaneTruthRules::known_names_csv()
            );
            return ExitCode::from(2);
        }
    }
    if self_test {
        // NOT `selftest(&rules) as u8` (bead omp-orchestrator-e4wp): `as` WRAPS between
        // integers, so a count of 256 truncated to 0 and this oracle would print RED while
        // exiting SUCCESS. `selftest_exit_code` is total over i32 and returns u8 literals.
        return ExitCode::from(selftest_exit_code(selftest(&rules)));
    }
    // Same narrowing, second site in the same function. Fixing only the one that was reported
    // would have left this one live — the failure mode `admission-reason` records as "six
    // crates shared this shape; fixing only the one that fired would have left five live".
    // `run_live_exit_code` preserves 0 and 4 exactly and refuses to turn anything
    // unrepresentable into a SUCCESS exit.
    ExitCode::from(run_live_exit_code(run_live(&session, &rules)))
}

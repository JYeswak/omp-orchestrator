#![forbid(unsafe_code)]

use pane_truth::{run_live, selftest, PaneTruthRules};
use std::process::ExitCode;

fn main() -> ExitCode {
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
        return ExitCode::from(selftest(&rules) as u8);
    }
    ExitCode::from(run_live(&session, &rules) as u8)
}

//! Verify that the committed `HEAD` tree compiles from a clean archive export.

#![forbid(unsafe_code)]

use no_shell_gate::head_compiles::{self, GateConfig};
use std::path::PathBuf;
use std::process::ExitCode;

fn usage() -> &'static str {
    "usage: head-compiles-gate [--repo PATH] [--receipt PATH]\n\nVerifies git archive HEAD in a private export with cargo check --workspace --all-targets.\nThe verdict is about committed-tree compilation, not tests, CI parity, or worktree cleanliness."
}

fn main() -> ExitCode {
    let mut repo = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut receipt = None;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--repo" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    eprintln!("ARGUMENT_ERROR --repo requires PATH\n{}", usage());
                    return ExitCode::from(2);
                };
                repo = PathBuf::from(value);
            }
            "--receipt" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    eprintln!("ARGUMENT_ERROR --receipt requires PATH\n{}", usage());
                    return ExitCode::from(2);
                };
                receipt = Some(PathBuf::from(value));
            }
            "--help" | "-h" => {
                println!("{}", usage());
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("ARGUMENT_ERROR unknown argument {other}\n{}", usage());
                return ExitCode::from(2);
            }
        }
        index += 1;
    }

    let config = GateConfig { repo, receipt };
    match head_compiles::run(&config) {
        Ok(receipt) => {
            print!("{}\n", receipt.text);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(error.exit_code())
        }
    }
}

#![forbid(unsafe_code)]

use ompo_start::append_s1_foundation;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("docs/plan/FOUNDATION.jsonl"));
    match append_s1_foundation(&path) {
        Ok(true) => {
            println!("FOUNDATION_APPENDED {}", path.display());
            ExitCode::SUCCESS
        }
        Ok(false) => {
            println!("FOUNDATION_ALREADY_PRESENT {}", path.display());
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("FOUNDATION_APPEND_FAILED {err}");
            ExitCode::from(1)
        }
    }
}

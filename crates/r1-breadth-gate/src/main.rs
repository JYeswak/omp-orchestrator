#![forbid(unsafe_code)]

use r1_breadth_gate::check_repo;
use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let root = env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| ".".into()));
    match check_repo(&root) {
        Ok(report) => {
            print!("{}", report.render());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

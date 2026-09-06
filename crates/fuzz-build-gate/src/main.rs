#![forbid(unsafe_code)]

use fuzz_build_gate::{run_gate, DEFAULT_JOBS, REQUIRED_WORKER};
use std::path::PathBuf;
use std::process::ExitCode;

fn usage() {
    eprintln!("usage: fuzz-build-gate [--repo PATH] [--worker NAME] [--jobs N]");
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let index = args.iter().position(|arg| arg == name)?;
    args.get(index + 1).map(String::as_str)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        usage();
        return ExitCode::SUCCESS;
    }
    let repo = flag(&args, "--repo")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let worker = flag(&args, "--worker").unwrap_or(REQUIRED_WORKER);
    let jobs = flag(&args, "--jobs")
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_JOBS);
    match run_gate(&repo, worker, jobs) {
        Ok(report) => {
            println!(
                "fuzz-build-gate: PASS targets={} regression_inputs={}",
                report.targets, report.regression_inputs
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("fuzz-build-gate: {error}");
            ExitCode::from(1)
        }
    }
}

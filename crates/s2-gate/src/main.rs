#![forbid(unsafe_code)]

use s2_gate::{admit, readback_ok};
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let root = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let inception = root.join(".omp-orchestrator/inception.json");
    let metric = readback_ok(&inception);
    match admit(metric) {
        Ok(()) => {
            println!("S2_OK L5-METRIC-READBACK-OK=1");
            ExitCode::SUCCESS
        }
        Err(refuse) => {
            eprintln!("{refuse}");
            ExitCode::from(1)
        }
    }
}

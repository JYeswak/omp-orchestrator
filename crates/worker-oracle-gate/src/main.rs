#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;
use worker_oracle_gate::{admission_check, classify_target, CensusReport, TargetVerdict};


fn usage() {
    eprintln!("usage: worker-oracle-gate census [--repo PATH] | target TARGET [--repo PATH] [--worker NAME]");
}

fn report(report: CensusReport) -> ExitCode {
    println!("WORKER_ORACLE_CENSUS PASS ledger_targets={} ledger_host_bound={} marker_matches={} grep_host_bound={}", report.ledger_targets, report.ledger_host_bound, report.marker_matches, report.source_host_bound);
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().map(String::as_str).unwrap_or("census");
    let mut repo = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut worker = std::env::var("RCH_WORKER").unwrap_or_else(|_| "local".to_owned());
    let mut index = usize::from(mode != "census");
    while index < args.len() {
        match args[index].as_str() {
            "--repo" => { index += 1; let Some(path) = args.get(index) else { usage(); return ExitCode::from(2); }; repo = PathBuf::from(path); }
            "--worker" => { index += 1; let Some(name) = args.get(index) else { usage(); return ExitCode::from(2); }; worker = name.clone(); }
            "--help" | "-h" => { usage(); return ExitCode::SUCCESS; }
            _ => {}
        }
        index += 1;
    }
    if mode == "census" {
        return match admission_check(&repo) { Ok(census_report) => report(census_report), Err(error) => { eprintln!("{error}"); ExitCode::from(1) } };
    }
    if mode != "target" { usage(); return ExitCode::from(2); }
    let Some(target) = args.get(1) else { usage(); return ExitCode::from(2); };
    match classify_target(&repo, target, &worker) {
        Ok(TargetVerdict::TreePure { target }) => { println!("TREE_PURE PASS target={target}"); ExitCode::SUCCESS }
        Ok(TargetVerdict::HostBoundLocal { target }) => { println!("HOST_BOUND PASS target={target} worker=local"); ExitCode::SUCCESS }
        Ok(TargetVerdict::HostBoundRemote { target, worker }) => { println!("HOST_BOUND PASS target={target} worker={worker}"); ExitCode::SUCCESS }
        Err(error) => { eprintln!("{error}"); ExitCode::from(1) }
    }
}

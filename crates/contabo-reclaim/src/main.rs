#![forbid(unsafe_code)]

use asupersync::runtime::RuntimeBuilder;
use asupersync::Cx;
use contabo_reclaim::probe::Config;
use contabo_reclaim::{
    run_all_workers, worker_by_id, FleetReport, ReclaimError, ReclaimMode, ReclaimReport,
    WorkerSelection,
};
use std::path::PathBuf;
use std::process::ExitCode;

fn usage() -> &'static str {
    "usage: contabo-reclaim (--worker contabo-N | --all-workers) --base ABSOLUTE_PATH [--apply] [--json] (or CONTABO_RECLAIM_BASE)"
}

fn parse_args() -> Result<(WorkerSelection, PathBuf, ReclaimMode, bool), ReclaimError> {
    let mut selection = None;
    let mut base = None;
    let mut mode = ReclaimMode::DryRun;
    let mut json = false;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--worker" => {
                if selection.is_some() {
                    return Err(ReclaimError::MultipleWorkers);
                }
                index += 1;
                let worker = args.get(index).ok_or(ReclaimError::MissingSelection)?;
                selection = Some(WorkerSelection::One(worker_by_id(worker)?));
            }
            value if value.starts_with("--worker=") => {
                if selection.is_some() {
                    return Err(ReclaimError::MultipleWorkers);
                }
                selection = Some(WorkerSelection::One(worker_by_id(&value[9..])?));
            }
            "--all-workers" => {
                if selection.is_some() {
                    return Err(ReclaimError::MultipleWorkers);
                }
                selection = Some(WorkerSelection::All);
            }
            "--base" => {
                index += 1;
                base = Some(PathBuf::from(args.get(index).ok_or_else(|| {
                    ReclaimError::InvalidBase {
                        detail: "--base requires an absolute path".to_owned(),
                    }
                })?));
            }
            value if value.starts_with("--base=") => base = Some(PathBuf::from(&value[7..])),
            "--apply" => mode = ReclaimMode::Apply,
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                std::process::exit(0);
            }
            other => {
                return Err(ReclaimError::InvalidBase {
                    detail: format!("unknown argument={other}; {}", usage()),
                })
            }
        }
        index += 1;
    }
    let selection = selection.ok_or(ReclaimError::MissingSelection)?;
    let base = base
        .or_else(|| std::env::var_os("CONTABO_RECLAIM_BASE").map(PathBuf::from))
        .ok_or_else(|| ReclaimError::InvalidBase {
            detail: "BASE_REQUIRED: pass --base or CONTABO_RECLAIM_BASE".to_owned(),
        })?;
    Ok((selection, base, mode, json))
}

enum CliReport {
    Single(ReclaimReport),
    Fleet(FleetReport),
}

fn print_error(error: &ReclaimError, json: bool) {
    if json {
        let value = serde_json::json!({
            "schema": "contabo-reclaim/report-v1",
            "outcome": "ERROR",
            "error": error.to_string(),
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&value)
                .unwrap_or_else(|_| "{\"outcome\":\"ERROR\"}".to_owned())
        );
    } else {
        eprintln!("{error}");
    }
}

fn print_single_human(report: &ReclaimReport) {
    println!(
        "CONTABO_RECLAIM outcome={:?} worker={} host={} mode={:?} bytes={} directories={} active_build_ids={:?} detail={}",
        report.outcome,
        report.worker,
        report.host,
        report.mode,
        report.bytes,
        report.directories,
        report.active_build_ids,
        report.detail
    );
    for guard in &report.guards {
        println!("GUARD {guard}");
    }
    for candidate in &report.candidates {
        println!("CANDIDATE {candidate}");
    }
    for refusal in &report.refused {
        println!("{refusal}");
    }
}

fn print_fleet_human(report: &FleetReport) {
    println!(
        "CONTABO_RECLAIM_FLEET outcome={:?} mode={:?} workers={} deferred_active_workers={} incomplete_workers={} detail={}",
        report.outcome,
        report.mode,
        report.workers.len(),
        report.deferred_active_workers,
        report.incomplete_workers,
        report.detail
    );
    for (index, worker) in report.workers.iter().enumerate() {
        println!(
            "WORKER index={} outcome={:?} worker={} host={} active_build_ids={:?} bytes={} directories={} detail={}",
            index,
            worker.outcome,
            worker.worker,
            worker.host,
            worker.active_build_ids,
            worker.bytes,
            worker.directories,
            worker.detail
        );
        for guard in &worker.guards {
            println!("WORKER_GUARD index={} {guard}", index);
        }
        for refusal in &worker.refused {
            println!("WORKER_REFUSAL index={} {refusal}", index);
        }
    }
}

fn main() -> ExitCode {
    let (selection, base, mode, json) = match parse_args() {
        Ok(value) => value,
        Err(error) => {
            print_error(&error, false);
            return ExitCode::from(2);
        }
    };
    let runtime = match RuntimeBuilder::current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            let error = ReclaimError::Runtime {
                detail: error.to_string(),
            };
            print_error(&error, json);
            return ExitCode::from(2);
        }
    };
    let result = runtime.block_on(async {
        let cx = Cx::current().ok_or_else(|| ReclaimError::Runtime {
            detail: "no ambient Cx".to_owned(),
        })?;
        match selection {
            WorkerSelection::One(worker) => {
                let config = Config { worker, base, mode };
                contabo_reclaim::run(&cx, &config)
                    .await
                    .map(CliReport::Single)
            }
            WorkerSelection::All => run_all_workers(&cx, &base, mode)
                .await
                .map(CliReport::Fleet),
        }
    });
    match result {
        Ok(CliReport::Single(report)) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report)
                        .unwrap_or_else(|_| "{\"outcome\":\"ERROR\"}".to_owned())
                );
            } else {
                print_single_human(&report);
            }
            ExitCode::from(report.outcome.exit_code())
        }
        Ok(CliReport::Fleet(report)) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report)
                        .unwrap_or_else(|_| "{\"outcome\":\"ERROR\"}".to_owned())
                );
            } else {
                print_fleet_human(&report);
            }
            ExitCode::from(report.exit_code())
        }
        Err(error) => {
            print_error(&error, json);
            ExitCode::from(2)
        }
    }
}

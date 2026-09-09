#![forbid(unsafe_code)]

use asupersync::runtime::RuntimeBuilder;
use asupersync::Cx;
use contabo_reclaim::probe::Config;
use contabo_reclaim::{worker_by_id, ReclaimError, ReclaimMode};
use std::path::PathBuf;
use std::process::ExitCode;

fn usage() -> &'static str {
    "usage: contabo-reclaim --worker contabo-N --base ABSOLUTE_PATH [--apply] [--json] (or CONTABO_RECLAIM_BASE)"
}

fn parse_args() -> Result<(Config, bool), ReclaimError> {
    let mut worker = None;
    let mut base = None;
    let mut mode = ReclaimMode::DryRun;
    let mut json = false;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--worker" => {
                if worker.is_some() {
                    return Err(ReclaimError::MultipleWorkers);
                }
                index += 1;
                worker = Some(args.get(index).ok_or(ReclaimError::MissingWorker)?.clone());
            }
            value if value.starts_with("--worker=") => {
                if worker.is_some() {
                    return Err(ReclaimError::MultipleWorkers);
                }
                worker = Some(value[9..].to_owned());
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
    let worker = worker.ok_or(ReclaimError::MissingWorker)?;
    let base = base
        .or_else(|| std::env::var_os("CONTABO_RECLAIM_BASE").map(PathBuf::from))
        .ok_or_else(|| ReclaimError::InvalidBase {
            detail: "BASE_REQUIRED: pass --base or CONTABO_RECLAIM_BASE".to_owned(),
        })?;
    Ok((
        Config {
            worker: worker_by_id(&worker)?,
            base,
            mode,
        },
        json,
    ))
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

fn main() -> ExitCode {
    let (config, json) = match parse_args() {
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
        contabo_reclaim::run(&cx, &config).await
    });
    match result {
        Ok(report) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report)
                        .unwrap_or_else(|_| "{\"outcome\":\"ERROR\"}".to_owned())
                );
            } else {
                println!(
                    "CONTABO_RECLAIM outcome={:?} worker={} host={} mode={:?} bytes={} directories={} detail={}",
                    report.outcome,
                    report.worker,
                    report.host,
                    report.mode,
                    report.bytes,
                    report.directories,
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
            ExitCode::from(report.outcome.exit_code())
        }
        Err(error) => {
            print_error(&error, json);
            ExitCode::from(2)
        }
    }
}

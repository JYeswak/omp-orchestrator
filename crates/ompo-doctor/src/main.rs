#![forbid(unsafe_code)]

use ompo_doctor::{current_repo, run_doctor};
use std::path::PathBuf;
use std::process::ExitCode;

fn usage() {
    eprintln!(
        "usage: ompo doctor [--repo PATH] [--scope system] [--json]\n\
         probe tools, emit LifecycleEvent rows, and read back the journal"
    );
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let command = args.next();
    if matches!(command.as_deref(), Some("--help") | Some("-h")) {
        usage();
        return ExitCode::SUCCESS;
    }
    if command.as_deref() != Some("doctor") {
        usage();
        return ExitCode::from(2);
    }

    let mut repo = match current_repo() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("ompo doctor: {error}");
            return ExitCode::from(1);
        }
    };
    let mut scope = "system".to_owned();
    let mut json = false;
    let rest: Vec<_> = args.collect();
    let mut index = 0;
    while index < rest.len() {
        match rest[index].as_str() {
            "--json" => json = true,
            "--repo" => {
                index += 1;
                let Some(path) = rest.get(index) else {
                    usage();
                    return ExitCode::from(2);
                };
                repo = PathBuf::from(path);
            }
            "--scope" => {
                index += 1;
                let Some(value) = rest.get(index) else {
                    usage();
                    return ExitCode::from(2);
                };
                scope = value.clone();
            }
            "--help" | "-h" => {
                usage();
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("ompo doctor: unknown argument {other}");
                usage();
                return ExitCode::from(2);
            }
        }
        index += 1;
    }

    match run_doctor(&repo, &scope) {
        Ok(summary) if json => match serde_json::to_string(&summary) {
            Ok(value) => {
                println!("{value}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("ompo doctor: cannot encode report: {error}");
                ExitCode::from(1)
            }
        },
        Ok(summary) => {
            println!(
                "OMPO_DOCTOR scope={} probes={} lifecycle_events={} readback_lines={} journal={}",
                summary.scope,
                summary.probe_count,
                summary.event_count,
                summary.readback_lines,
                summary.lifecycle_journal.display()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("ompo doctor: {error}");
            ExitCode::from(1)
        }
    }
}

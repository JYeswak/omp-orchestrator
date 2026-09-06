#![forbid(unsafe_code)]

use input_manifest::{
    audit_ledgers, audit_ledgers_at, scan_gate_unwired, CargoInputBound, CargoTestResult,
    CensusMode,
};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("cargo-test") => cargo_test(args.collect()),
        Some("census") => census(args.collect()),
        Some("ledger-audit") => ledger_audit(args.collect()),
        _ => {
            eprintln!(
                "usage: input-manifest cargo-test [--head N|--all] [--source CMD]\n\
                 input-manifest census --repo ROOT --recursive|--non-recursive"
            );
            ExitCode::from(2)
        }
    }
}

fn cargo_test(args: Vec<String>) -> ExitCode {
    let mut bound = CargoInputBound::All;
    let mut source = "cargo test".to_owned();
    let mut input_file = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--all" => bound = CargoInputBound::All,
            "--head" => {
                index += 1;
                let Some(value) = args.get(index).and_then(|value| value.parse().ok()) else {
                    eprintln!("MANIFEST_REFUSED --head requires a positive integer");
                    return ExitCode::from(2);
                };
                bound = CargoInputBound::Head { lines: value };
            }
            "--source" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    eprintln!("MANIFEST_REFUSED --source requires text");
                    return ExitCode::from(2);
                };
                source = value.clone();
            }
            "--input-file" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    eprintln!("MANIFEST_REFUSED --input-file requires a path");
                    return ExitCode::from(2);
                };
                input_file = Some(PathBuf::from(value));
            }
            other => {
                eprintln!("MANIFEST_REFUSED unknown cargo-test argument {other}");
                return ExitCode::from(2);
            }
        }
        index += 1;
    }

    let input = match input_file {
        Some(path) => match fs::read_to_string(&path) {
            Ok(input) => input,
            Err(error) => {
                eprintln!(
                    "MANIFEST_REFUSED input unread path={}: {error}",
                    path.display()
                );
                return ExitCode::from(2);
            }
        },
        None => {
            let mut input = String::new();
            if let Err(error) = io::stdin().read_to_string(&mut input) {
                eprintln!("MANIFEST_REFUSED stdin unread: {error}");
                return ExitCode::from(2);
            }
            input
        }
    };
    match CargoTestResult::extract(&input, bound, source) {
        Ok(result) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&result).expect("serialize result")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn ledger_audit(args: Vec<String>) -> ExitCode {
    let mut root = PathBuf::from(".");
    let mut ledger_dir = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--repo" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    eprintln!("MANIFEST_REFUSED --repo requires a path");
                    return ExitCode::from(2);
                };
                root = PathBuf::from(value);
            }
            "--ledger-dir" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    eprintln!("MANIFEST_REFUSED --ledger-dir requires a path");
                    return ExitCode::from(2);
                };
                ledger_dir = Some(PathBuf::from(value));
            }
            other => {
                eprintln!("MANIFEST_REFUSED unknown ledger-audit argument {other}");
                return ExitCode::from(2);
            }
        }
        index += 1;
    }
    let result = match ledger_dir {
        Some(base) => audit_ledgers_at(&root, &base),
        None => audit_ledgers(&root),
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&result).expect("serialize ledger audit")
    );
    if result.error.is_some() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
fn census(args: Vec<String>) -> ExitCode {
    let mut root = PathBuf::from(".");
    let mut mode = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--repo" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    eprintln!("MANIFEST_REFUSED --repo requires a path");
                    return ExitCode::from(2);
                };
                root = PathBuf::from(value);
            }
            "--recursive" => mode = Some(CensusMode::Recursive),
            "--non-recursive" => {
                mode = Some(CensusMode::NonRecursiveGlob {
                    pattern: "crates/omp-orchestrator/src/*.rs".to_owned(),
                })
            }
            other => {
                eprintln!("MANIFEST_REFUSED unknown census argument {other}");
                return ExitCode::from(2);
            }
        }
        index += 1;
    }
    let Some(mode) = mode else {
        eprintln!("MANIFEST_REFUSED census requires --recursive or --non-recursive");
        return ExitCode::from(2);
    };
    let result = scan_gate_unwired(&root, mode);
    println!(
        "{}",
        serde_json::to_string_pretty(&result).expect("serialize census")
    );
    if result.error.is_some() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

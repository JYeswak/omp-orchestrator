#![forbid(unsafe_code)]

//! Robot-facing entrypoint for the six-clause porting gate.

use asupersync::runtime::RuntimeBuilder;
use asupersync::Cx;
use porting_gate::{check_candidate, GateStatus, SCHEMA_VERSION};
use std::path::PathBuf;
use std::process::ExitCode;

fn usage() -> &'static str {
    "usage: porting-gate --repo PATH --crate NAME"
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut repo = None;
    let mut candidate = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--repo" => {
                index += 1;
                repo = args.get(index).map(PathBuf::from);
            }
            "--crate" => {
                index += 1;
                candidate = args.get(index).cloned();
            }
            "--help" => {
                // `{usage()}` is not a valid inline format arg — Rust only accepts
                // identifiers there, never a call. It is a compile error, not a
                // lint, so this crate did not build at all.
                println!("{}", usage());
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("PORTING_GATE_ERROR unknown argument {other}\n{}", usage());
                return ExitCode::from(2);
            }
        }
        index += 1;
    }
    let (Some(repo), Some(candidate)) = (repo, candidate) else {
        eprintln!("PORTING_GATE_ERROR missing --repo or --crate\n{}", usage());
        return ExitCode::from(2);
    };

    let runtime = match RuntimeBuilder::current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("PORTING_GATE_ERROR runtime={error}");
            return ExitCode::from(2);
        }
    };
    let result = runtime.block_on(async move {
        let cx = Cx::current().ok_or_else(|| "no runtime context".to_owned())?;
        check_candidate(&cx, &repo, &candidate)
            .await
            .map_err(|error| error.to_string())
    });
    match result {
        Ok(report) => {
            println!(
                "{}",
                serde_json::json!({
                    "schema_version": SCHEMA_VERSION,
                    "command": "porting-gate",
                    "candidate": report.crate_name,
                    "status": format!("{:?}", report.status).to_uppercase(),
                    "clauses": report.clauses,
                })
            );
            if report.status == GateStatus::Pass {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(error) => {
            eprintln!("PORTING_GATE_ERROR {error}");
            ExitCode::from(2)
        }
    }
}

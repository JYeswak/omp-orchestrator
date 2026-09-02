#![forbid(unsafe_code)]

use asupersync::Cx;
use preregistration_gate::{SCHEMA_VERSION, collect_repository_inputs, validate_pre_write};
use serde_json::{Value, json};
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn repo_from_args(args: impl IntoIterator<Item = String>) -> Result<PathBuf, String> {
    let mut values = args.into_iter();
    let mut repo = env::current_dir().map_err(|error| format!("CONFIG_ERROR cwd: {error}"))?;
    while let Some(value) = values.next() {
        match value.as_str() {
            "--repo" => {
                repo = PathBuf::from(
                    values
                        .next()
                        .ok_or_else(|| "CONFIG_ERROR --repo requires a path".to_owned())?,
                );
            }
            "--help" | "-h" => {
                return Err(
                    "usage: preregistration-gate [--repo PATH] (exit 0=pass, 1=refusal, 2=error)"
                        .to_owned(),
                );
            }
            other => return Err(format!("CONFIG_ERROR unknown argument {other}")),
        }
    }
    Ok(repo)
}

fn print_error(status: &str, error: impl Into<String>) -> ExitCode {
    println!(
        "{}",
        json!({
            "schema_version": SCHEMA_VERSION,
            "status": status,
            "data": Value::Null,
            "error": error.into(),
        })
    );
    if status == "REFUSED" {
        ExitCode::from(1)
    } else {
        ExitCode::from(2)
    }
}

#[asupersync::main]
async fn main() -> ExitCode {
    let repo = match repo_from_args(env::args().skip(1)) {
        Ok(repo) => repo,
        Err(error) => return print_error("ERROR", error),
    };
    let cx = match Cx::current() {
        Some(cx) => cx,
        None => return print_error("ERROR", "PREREGISTRATION_ERROR no asupersync context"),
    };
    let inputs = match collect_repository_inputs(&cx, &repo).await {
        Ok(inputs) => inputs,
        Err(error) => return print_error("ERROR", error),
    };
    match validate_pre_write(
        &inputs.registry,
        &inputs.base_revision,
        &inputs.committed_revisions,
        &inputs.changed_paths,
        &inputs.evidence_rows,
    ) {
        Ok(report) => {
            println!(
                "{}",
                json!({
                    "schema_version": SCHEMA_VERSION,
                    "status": "OK",
                    "data": report,
                    "error": Value::Null,
                })
            );
            ExitCode::SUCCESS
        }
        Err(error) => print_error("REFUSED", error.to_string()),
    }
}

#![forbid(unsafe_code)]

use asupersync::runtime::RuntimeBuilder;
use asupersync::types::Budget;
use omp_inventory_map::{
    CRATE_VERSION, EXPECTED_OMP_VERSION, InventoryMap, ProbeConfig, ProbeState, SCHEMA_VERSION,
    collect_inventory,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug)]
struct Arguments {
    command: String,
    config: ProbeConfig,
}

impl Arguments {
    fn parse(raw: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut values = raw.into_iter();
        let mut command = "doctor".to_owned();
        let mut config = ProbeConfig::default();
        let mut first = true;
        while let Some(value) = values.next() {
            if value == "--json" {
                continue;
            }
            if first && matches!(value.as_str(), "doctor" | "health" | "version") {
                command = value;
                first = false;
                continue;
            }
            first = false;
            match value.as_str() {
                "--repo" => {
                    config.repo_root = PathBuf::from(
                        values
                            .next()
                            .ok_or_else(|| "CONFIG_ERROR --repo requires a path".to_owned())?,
                    );
                }
                "--omp" => {
                    config.omp_program = PathBuf::from(
                        values
                            .next()
                            .ok_or_else(|| "CONFIG_ERROR --omp requires a path".to_owned())?,
                    );
                }
                "--cargo" => {
                    config.cargo_program = PathBuf::from(
                        values
                            .next()
                            .ok_or_else(|| "CONFIG_ERROR --cargo requires a path".to_owned())?,
                    );
                }
                "--find" => {
                    config.find_program = PathBuf::from(
                        values
                            .next()
                            .ok_or_else(|| "CONFIG_ERROR --find requires a path".to_owned())?,
                    );
                }
                other => return Err(format!("CONFIG_ERROR unknown argument {other}")),
            }
        }
        Ok(Self { command, config })
    }
}

#[derive(Serialize)]
struct RobotEnvelope<T: Serialize> {
    schema_version: &'static str,
    command: String,
    status: &'static str,
    data: Option<T>,
    error: Option<String>,
}

fn print_json<T: Serialize>(envelope: &RobotEnvelope<T>) -> Result<(), String> {
    let encoded = serde_json::to_string_pretty(envelope).map_err(|error| error.to_string())?;
    println!("{encoded}");
    Ok(())
}

fn version() -> Result<(), String> {
    print_json(&RobotEnvelope {
        schema_version: SCHEMA_VERSION,
        command: "version".to_owned(),
        status: "OK",
        data: Some(json!({
            "crate_version": CRATE_VERSION,
            "schema_version": SCHEMA_VERSION,
            "expected_omp_version": EXPECTED_OMP_VERSION,
        })),
        error: None,
    })
}

fn map_status(map: &InventoryMap) -> &'static str {
    match map.state {
        ProbeState::Known => "OK",
        ProbeState::Unknown => "UNKNOWN",
    }
}

fn collect(command: String, config: ProbeConfig) -> ExitCode {
    let runtime = match RuntimeBuilder::current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            let envelope = RobotEnvelope::<Value> {
                schema_version: SCHEMA_VERSION,
                command,
                status: "ERROR",
                data: None,
                error: Some(format!("RUNTIME_ERROR {error}")),
            };
            let _ = print_json(&envelope);
            return ExitCode::from(1);
        }
    };
    let cx = runtime.request_cx_with_budget(Budget::INFINITE);
    match runtime.block_on(async { collect_inventory(&cx, &config).await }) {
        Ok(map) => {
            let status = map_status(&map);
            let code = if status == "OK" { 0 } else { 2 };
            let envelope = RobotEnvelope {
                schema_version: SCHEMA_VERSION,
                command,
                status,
                data: Some(map),
                error: None,
            };
            if print_json(&envelope).is_err() {
                ExitCode::from(1)
            } else {
                ExitCode::from(code)
            }
        }
        Err(error) => {
            let envelope = RobotEnvelope::<Value> {
                schema_version: SCHEMA_VERSION,
                command,
                status: "ERROR",
                data: None,
                error: Some(error.to_string()),
            };
            let _ = print_json(&envelope);
            ExitCode::from(1)
        }
    }
}

fn main() -> ExitCode {
    let arguments = match Arguments::parse(env::args().skip(1)) {
        Ok(arguments) => arguments,
        Err(error) => {
            let envelope = RobotEnvelope::<Value> {
                schema_version: SCHEMA_VERSION,
                command: "doctor".to_owned(),
                status: "ERROR",
                data: None,
                error: Some(error),
            };
            let _ = print_json(&envelope);
            return ExitCode::from(1);
        }
    };
    if arguments.command == "version" {
        return if version().is_ok() {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        };
    }
    collect(arguments.command, arguments.config)
}

#![forbid(unsafe_code)]

use asupersync::Cx;
use omp_rpc_session::{
    NO_CLAIM_BOUNDARY, OMP_RPC_SCHEMA_VERSION, OMP_SURFACE, OmpCommand, RpcRequest,
    RpcSessionConfig, run_session,
};
use serde_json::{Value, json};
use std::path::PathBuf;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_BINARY: &str = "omp";

struct QuickOptions {
    binary: PathBuf,
    state_only: bool,
    session: Option<String>,
    session_dir: Option<PathBuf>,
}

fn print_json(value: Value) {
    println!("{value}");
}

fn binary_from_args(args: &[String]) -> Result<PathBuf, String> {
    let mut binary = std::env::var_os("OMP_BINARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_BINARY));
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--binary" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("--binary requires a path".to_owned());
                };
                binary = PathBuf::from(value);
            }
            "--help" | "-h" => return Err(usage()),
            unknown => return Err(format!("unknown argument {unknown}; {}", usage())),
        }
        index += 1;
    }
    Ok(binary)
}

fn quick_options(args: &[String]) -> Result<QuickOptions, String> {
    let mut options = QuickOptions {
        binary: std::env::var_os("OMP_BINARY")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_BINARY)),
        state_only: false,
        session: None,
        session_dir: None,
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--state" => options.state_only = true,
            "--binary" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("--binary requires a path".to_owned());
                };
                options.binary = PathBuf::from(value);
            }
            "--session" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("--session requires an existing session id or path".to_owned());
                };
                if value.trim().is_empty() {
                    return Err("--session requires a non-empty existing session id or path".to_owned());
                }
                options.session = Some(value.clone());
            }
            "--session-dir" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("--session-dir requires a path".to_owned());
                };
                if value.trim().is_empty() {
                    return Err("--session-dir requires a non-empty path".to_owned());
                }
                options.session_dir = Some(PathBuf::from(value));
            }
            "--help" | "-h" => return Err(usage()),
            unknown => return Err(format!("unknown argument {unknown}; {}", usage())),
        }
        index += 1;
    }
    Ok(options)
}

fn usage() -> String {
    "usage: omp-rpc-session <doctor|health|version|quick> [--binary PATH] [--state] [--session ID|PATH] [--session-dir PATH]".to_owned()
}
fn binary_present(binary: &std::path::Path) -> bool {
    if binary.is_file() {
        return true;
    }
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|directory| directory.join(binary).is_file())
}

fn doctor(binary: &std::path::Path) -> Value {
    let exists = binary_present(binary);
    json!({
        "schema": OMP_RPC_SCHEMA_VERSION,
        "kind": "doctor",
        "version": VERSION,
        "surface": OMP_SURFACE,
        "ok": exists,
        "binary": binary,
        "binaryPresent": exists,
        "transport": "direct-process-group",
        "stdin": "bounded-one-shot-request-sequence",
        "noClaim": NO_CLAIM_BOUNDARY
    })
}

#[asupersync::main]
async fn main() {
    let mut argv = std::env::args().skip(1);
    let command = argv.next().unwrap_or_else(|| "health".to_owned());
    let rest = argv.collect::<Vec<_>>();
    match command.as_str() {
        "version" | "--version" => {
            print_json(json!({
                "schema": OMP_RPC_SCHEMA_VERSION,
                "kind": "version",
                "version": VERSION,
                "surface": OMP_SURFACE,
                "noClaim": NO_CLAIM_BOUNDARY
            }));
        }
        "doctor" => match binary_from_args(&rest) {
            Ok(binary) => print_json(doctor(&binary)),
            Err(error) => print_json(json!({
                "schema": OMP_RPC_SCHEMA_VERSION,
                "kind": "doctor",
                "version": VERSION,
                "surface": OMP_SURFACE,
                "ok": false,
                "error": error,
                "noClaim": NO_CLAIM_BOUNDARY
            })),
        },
        "health" => match binary_from_args(&rest) {
            Ok(binary) => {
                let result = doctor(&binary);
                print_json(json!({
                    "schema": OMP_RPC_SCHEMA_VERSION,
                    "kind": "health",
                    "version": VERSION,
                    "surface": OMP_SURFACE,
                    "ok": result.get("ok").and_then(Value::as_bool).unwrap_or(false),
                    "binary": binary,
                    "noClaim": NO_CLAIM_BOUNDARY
                }));
            }
            Err(error) => print_json(json!({
                "schema": OMP_RPC_SCHEMA_VERSION,
                "kind": "health",
                "version": VERSION,
                "surface": OMP_SURFACE,
                "ok": false,
                "error": error,
                "noClaim": NO_CLAIM_BOUNDARY
            })),
        },
        "quick" => {
            let options = match quick_options(&rest) {
                Ok(options) => options,
                Err(error) => {
                    print_json(json!({
                        "schema": OMP_RPC_SCHEMA_VERSION,
                        "kind": "quick",
                        "version": VERSION,
                        "surface": OMP_SURFACE,
                        "ok": false,
                        "error": error,
                        "noClaim": NO_CLAIM_BOUNDARY
                    }));
                    std::process::exit(2);
                }
            };
            let mut command = OmpCommand::new(options.binary);
            if let Some(path) = options.session_dir {
                command = command.arg("--session-dir").arg(path);
            }
            if let Some(session) = options.session {
                command = match command.resume(session) {
                    Ok(command) => command,
                    Err(error) => {
                        print_json(json!({
                            "schema": OMP_RPC_SCHEMA_VERSION,
                            "kind": "quick",
                            "version": VERSION,
                            "surface": OMP_SURFACE,
                            "ok": false,
                            "error": error.to_string(),
                            "noClaim": NO_CLAIM_BOUNDARY
                        }));
                        std::process::exit(2);
                    }
                };
            }
            let config = if options.state_only {
                RpcSessionConfig::with_command(command).with_requests(RpcRequest::state_sequence())
            } else {
                RpcSessionConfig::with_command(command)
            };
            let cx = match Cx::current() {
                Some(cx) => cx,
                None => {
                    print_json(json!({
                        "schema": OMP_RPC_SCHEMA_VERSION,
                        "kind": "quick",
                        "version": VERSION,
                        "surface": OMP_SURFACE,
                        "ok": false,
                        "error": "asupersync main did not provide a current Cx",
                        "noClaim": NO_CLAIM_BOUNDARY
                    }));
                    std::process::exit(3);
                }
            };
            match run_session(&cx, &config).await {
                Ok(report) => print_json(report.to_json()),
                Err(error) => {
                    print_json(json!({
                        "schema": OMP_RPC_SCHEMA_VERSION,
                        "kind": "quick",
                        "version": VERSION,
                        "surface": OMP_SURFACE,
                        "ok": false,
                        "error": error.to_string(),
                        "noClaim": NO_CLAIM_BOUNDARY
                    }));
                    std::process::exit(4);
                }
            }
        }
        unknown => print_json(json!({
            "schema": OMP_RPC_SCHEMA_VERSION,
            "kind": "error",
            "version": VERSION,
            "surface": OMP_SURFACE,
            "ok": false,
            "error": format!("unknown command {unknown}; {}", usage()),
            "noClaim": NO_CLAIM_BOUNDARY
        })),
    }
}

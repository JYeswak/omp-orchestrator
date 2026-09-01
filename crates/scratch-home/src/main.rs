#![forbid(unsafe_code)]

use scratch_home::{ScratchError, ScratchRoot, ENV_VAR, SCHEMA_VERSION};
use serde_json::json;
use std::process::{Command, ExitCode};
use std::time::Duration;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn usage() -> &'static str {
    "usage: scratch-home <resolve|pane-env|job|reap|spawn|version> ...\n\n  resolve SESSION [--base PATH]\n  pane-env SESSION [--base PATH]\n  job SESSION PANE_OR_AGENT JOB OWNER [--base PATH]\n  reap SESSION AGE_SECS [--apply] [--base PATH]\n  spawn SESSION [--base PATH] [--ntm PATH] [-- NTM_ARGS...]"
}

fn json_error(error: impl ToString) -> serde_json::Value {
    json!({
        "schema": SCHEMA_VERSION,
        "version": VERSION,
        "ok": false,
        "error": error.to_string(),
    })
}

fn print_json(value: serde_json::Value) {
    println!("{value}");
}

fn root_from(base: Option<String>) -> Result<ScratchRoot, ScratchError> {
    match base {
        Some(path) => ScratchRoot::new(path),
        None => ScratchRoot::default(),
    }
}

fn take_base(args: &mut Vec<String>) -> Result<Option<String>, String> {
    let mut base = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--base" {
            if base.is_some() {
                return Err("--base may be provided once".to_owned());
            }
            let Some(value) = args.get(index + 1) else {
                return Err("--base requires a path".to_owned());
            };
            base = Some(value.clone());
            args.drain(index..=index + 1);
        } else {
            index += 1;
        }
    }
    Ok(base)
}

fn resolve(args: &[String]) -> Result<(), String> {
    let mut positional = args.to_vec();
    let base = take_base(&mut positional)?;
    if positional.len() != 1 {
        return Err(usage().to_owned());
    }
    let root = root_from(base).map_err(|error| error.to_string())?;
    let path = root
        .ensure_session(&positional[0])
        .map_err(|error| error.to_string())?;
    println!("{}", path.display());
    Ok(())
}

fn pane_env(args: &[String]) -> Result<(), String> {
    let mut positional = args.to_vec();
    let base = take_base(&mut positional)?;
    if positional.len() != 1 {
        return Err(usage().to_owned());
    }
    let root = root_from(base).map_err(|error| error.to_string())?;
    println!(
        "{}",
        root.pane_env(&positional[0])
            .map_err(|error| error.to_string())?
    );
    Ok(())
}

fn create_job(args: &[String]) -> Result<(), String> {
    let mut positional = args.to_vec();
    let base = take_base(&mut positional)?;
    if positional.len() != 4 {
        return Err(usage().to_owned());
    }
    let root = root_from(base).map_err(|error| error.to_string())?;
    let path = root
        .create_job(
            &positional[0],
            &positional[1],
            &positional[2],
            &positional[3],
        )
        .map_err(|error| error.to_string())?;
    println!("{}", path.display());
    Ok(())
}

fn reap(args: &[String]) -> Result<(), String> {
    let mut positional = Vec::new();
    let mut base = None;
    let mut apply = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--base" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("--base requires a path".to_owned());
                };
                base = Some(value.clone());
                index += 2;
            }
            "--apply" => {
                apply = true;
                index += 1;
            }
            value => {
                positional.push(value.to_owned());
                index += 1;
            }
        }
    }
    if positional.len() != 2 {
        return Err(usage().to_owned());
    }
    let age_secs = positional[1]
        .parse::<u64>()
        .map_err(|_| "AGE_SECS must be a non-negative integer".to_owned())?;
    let root = root_from(base).map_err(|error| error.to_string())?;
    let report = root
        .reap(&positional[0], Duration::from_secs(age_secs))
        .map_err(|error| error.to_string())?;
    let removed = if apply {
        root.apply(&report).map_err(|error| error.to_string())?
    } else {
        0
    };
    print_json(json!({
        "schema": SCHEMA_VERSION,
        "version": VERSION,
        "ok": true,
        "session": positional[0],
        "age_secs": age_secs,
        "apply": apply,
        "removed": removed,
        "auto_reapable": report.auto_reapable(),
        "candidates": report.candidates,
        "unknown": report.unknown,
        "no_claim": "Unknown owner attribution is never auto-reaped; dry-run is the default.",
    }));
    Ok(())
}

fn spawn(args: &[String]) -> Result<ExitCode, String> {
    let separator = args.iter().position(|arg| arg == "--");
    let (prefix, trailing) = match separator {
        Some(index) => (&args[..index], &args[index + 1..]),
        None => (args, &[][..]),
    };
    let mut ntm = "ntm".to_owned();
    let mut base = None;
    let mut session = None;
    let mut passthrough = Vec::new();
    let mut index = 0;
    while index < prefix.len() {
        match prefix[index].as_str() {
            "--base" => {
                let Some(value) = prefix.get(index + 1) else {
                    return Err("--base requires a path".to_owned());
                };
                base = Some(value.clone());
                index += 2;
            }
            "--ntm" => {
                let Some(value) = prefix.get(index + 1) else {
                    return Err("--ntm requires a path".to_owned());
                };
                ntm = value.clone();
                index += 2;
            }
            value if session.is_none() => {
                session = Some(value.to_owned());
                index += 1;
            }
            value => {
                passthrough.push(value.to_owned());
                index += 1;
            }
        }
    }
    let Some(session) = session else {
        return Err(usage().to_owned());
    };
    passthrough.extend(trailing.iter().cloned());
    let root = root_from(base).map_err(|error| error.to_string())?;
    let mut command = Command::new(ntm);
    command.args(
        root.ntm_spawn_args(&session)
            .map_err(|error| error.to_string())?,
    );
    command.args(passthrough);
    let status = command
        .status()
        .map_err(|error| format!("ntm spawn failed: {error}"))?;
    Ok(if status.success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let command = args
        .first()
        .cloned()
        .unwrap_or_else(|| "resolve".to_owned());
    if !args.is_empty() {
        args.remove(0);
    }
    let result = match command.as_str() {
        "resolve" => resolve(&args).map(|()| ExitCode::SUCCESS),
        "pane-env" => pane_env(&args).map(|()| ExitCode::SUCCESS),
        "job" => create_job(&args).map(|()| ExitCode::SUCCESS),
        "reap" => reap(&args).map(|()| ExitCode::SUCCESS),
        "spawn" => spawn(&args),
        "version" | "--version" => {
            print_json(
                json!({"schema": SCHEMA_VERSION, "version": VERSION, "ok": true, "env": ENV_VAR}),
            );
            Ok(ExitCode::SUCCESS)
        }
        "help" | "--help" | "-h" => {
            println!("{}", usage());
            Ok(ExitCode::SUCCESS)
        }
        _ => Err(format!("unknown command {command}; {}", usage())),
    };
    match result {
        Ok(code) => code,
        Err(error) => {
            print_json(json_error(error));
            ExitCode::from(2)
        }
    }
}

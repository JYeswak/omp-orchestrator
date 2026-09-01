#![forbid(unsafe_code)]

use omp_orchestrator::target_directory::{
    reap_target_in_scope, resolve_target_directory, ReapDecision, ResolvedTarget,
    TargetDirectoryError,
};
use serde_json::{json, Value};
use std::env;
use std::path::PathBuf;

fn main() {
    match run(env::args().skip(1).collect()) {
        Ok(value) => println!("{value}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}

fn run(args: Vec<String>) -> Result<Value, String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage().to_owned());
    };
    if matches!(command, "help" | "--help" | "-h") {
        return Ok(json!({"usage": usage()}));
    }

    let mut options = Options::default();
    options.parse(&args[1..])?;
    let repo_root = options
        .repo
        .take()
        .unwrap_or(env::current_dir().map_err(|error| {
            format!("CARGO_TARGET_POLICY_ERROR code=current-dir-unavailable reason={error}")
        })?);
    let registered_roots = options.registered_roots();

    match command {
        "resolve" | "report" => {
            if !options.paths.is_empty() {
                return Err("CARGO_TARGET_POLICY_ERROR code=unexpected-path-for-resolve".to_owned());
            }
            let resolved =
                resolve_target_directory(&repo_root, options.target.as_deref(), &registered_roots)
                    .map_err(format_target_error)?;
            Ok(resolved_json(&resolved))
        }
        "reap" => {
            let paths = options.reap_paths();
            if paths.is_empty() {
                return Err(format_target_error(TargetDirectoryError::EmptyScan));
            }
            let mut decisions = Vec::with_capacity(paths.len());
            for path in paths {
                let decision =
                    reap_target_in_scope(&path, &repo_root, &registered_roots, options.apply)
                        .map_err(format_target_error)?;
                decisions.push(decision_json(&decision));
            }
            Ok(json!({
                "command": "reap",
                "apply": options.apply,
                "repo_root": repo_root,
                "registered_roots": registered_roots,
                "decisions": decisions,
            }))
        }
        _ => Err(format!(
            "CARGO_TARGET_POLICY_ERROR code=unknown-command command={command}\n{}",
            usage()
        )),
    }
}

#[derive(Default)]
struct Options {
    repo: Option<PathBuf>,
    target: Option<PathBuf>,
    roots: Vec<PathBuf>,
    paths: Vec<PathBuf>,
    apply: bool,
}

impl Options {
    fn parse(&mut self, args: &[String]) -> Result<(), String> {
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--repo" => {
                    self.repo = Some(next_path(args, &mut index, "repo")?);
                }
                "--target" => {
                    self.target = Some(next_path(args, &mut index, "target")?);
                }
                "--registered-root" => {
                    self.roots
                        .push(next_path(args, &mut index, "registered-root")?);
                }
                "--apply" => self.apply = true,
                "--dry-run" => self.apply = false,
                "--help" | "-h" => return Err(usage().to_owned()),
                "--" => {
                    self.paths
                        .extend(args[index + 1..].iter().map(PathBuf::from));
                    break;
                }
                argument if argument.starts_with('-') => {
                    return Err(format!(
                        "CARGO_TARGET_POLICY_ERROR code=unknown-option option={argument}"
                    ));
                }
                argument => self.paths.push(PathBuf::from(argument)),
            }
            index += 1;
        }
        Ok(())
    }

    fn registered_roots(&self) -> Vec<PathBuf> {
        let mut roots = self.roots.clone();
        if let Some(root) = env::var_os("FRANKEN_CARGO_TARGET_ROOT") {
            roots.push(PathBuf::from(root));
        }
        roots
    }

    fn reap_paths(&self) -> Vec<PathBuf> {
        if self.paths.is_empty() {
            self.target.clone().into_iter().collect()
        } else {
            self.paths.clone()
        }
    }
}

fn next_path(args: &[String], index: &mut usize, option: &str) -> Result<PathBuf, String> {
    *index += 1;
    args.get(*index).map(PathBuf::from).ok_or_else(|| {
        format!("CARGO_TARGET_POLICY_ERROR code=missing-option-value option=--{option}")
    })
}

fn resolved_json(resolved: &ResolvedTarget) -> Value {
    json!({
        "command": "resolve",
        "path": resolved.path,
        "owner": {
            "owner": resolved.owner.owner,
            "target": resolved.owner.target,
            "toolchain": resolved.owner.toolchain,
            "pid": resolved.owner.pid,
            "created_epoch": resolved.owner.created_epoch,
        },
    })
}

fn decision_json(decision: &ReapDecision) -> Value {
    match decision {
        ReapDecision::Candidate { path, owner } => {
            json!({"decision": "candidate", "path": path, "owner": owner})
        }
        ReapDecision::Removed { path, bytes_before } => {
            json!({"decision": "removed", "path": path, "bytes_before": bytes_before})
        }
        ReapDecision::SkippedMissing { path } => {
            json!({"decision": "skipped-missing", "path": path})
        }
        ReapDecision::SkippedUnowned { path } => {
            json!({"decision": "skipped-unowned", "path": path})
        }
        ReapDecision::SkippedActive { path, reason } => {
            json!({"decision": "skipped-active", "path": path, "reason": reason})
        }
    }
}

fn format_target_error(error: TargetDirectoryError) -> String {
    format!("CARGO_TARGET_POLICY_ERROR code={error}")
}

fn usage() -> &'static str {
    "usage: omp-target-dir resolve|report [--repo PATH] [--target PATH] [--registered-root PATH...]\n       omp-target-dir reap [--apply|--dry-run] [--repo PATH] [--registered-root PATH...] [--target PATH|PATH...]\n\nEnvironment: FRANKEN_CARGO_TARGET_ROOT adds a registered root. Reaping is dry-run unless --apply is explicit."
}

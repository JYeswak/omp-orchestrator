#![forbid(unsafe_code)]

//! Fuzz-build and crash-regression admission gate.
//!
//! The fuzz workspace is intentionally separate from the root workspace. This crate is the
//! typed boundary that makes its two obligations inspectable: every declared target is part of
//! the build report, and every checked-in crash/seed input has an owning Rust regression test.
//! The binary additionally runs the fuzz workspace build on the one approved remote lane.
//!
//! Restored 2026-09-06 for bead omp-orchestrator-untracked-crate-breaks-workspace-ycwh.
//! Provenance of the pre-restore bytes is UNKNOWN (0 files in HEAD). Escaping is written
//! correctly in this one pass; do not patch quote-by-quote.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use subprocess_contract::{bounded_output, BoundedOutcome};

pub const REQUIRED_WORKER: &str = "contabo-3";
pub const DEFAULT_JOBS: usize = 2;
const BUILD_DEADLINE: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzTarget {
    pub name: String,
    pub owner_crate: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegressionInput {
    pub target: String,
    pub path: String,
    pub owner_crate: String,
    pub test_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractReport {
    pub targets: usize,
    pub regression_inputs: usize,
}

#[derive(Debug)]
pub enum GateError {
    Io { path: PathBuf, detail: String },
    MissingManifest(PathBuf),
    NoTargets(PathBuf),
    UnknownTargetOwner { target: String },
    NoRegressionInputs(PathBuf),
    MissingRegressionTest {
        input: String,
        owner_crate: String,
        expected_test: String,
    },
    WrongPlacement { actual: String },
    BuildFailed { targets: Vec<String>, detail: String },
    BuildTimedOut,
    BuildUnspawned(String),
}

impl fmt::Display for GateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, detail } => {
                write!(f, "FUZZ_BUILD_GATE_IO path={} detail={detail}", path.display())
            }
            Self::MissingManifest(path) => {
                write!(
                    f,
                    "FUZZ_BUILD_GATE_REFUSED missing_manifest={}",
                    path.display()
                )
            }
            Self::NoTargets(path) => {
                write!(
                    f,
                    "FUZZ_BUILD_GATE_REFUSED no_targets manifest={}",
                    path.display()
                )
            }
            Self::UnknownTargetOwner { target } => {
                write!(
                    f,
                    "FUZZ_BUILD_GATE_REFUSED target={target} reason=owner_not_mapped"
                )
            }
            Self::NoRegressionInputs(path) => {
                write!(
                    f,
                    "FUZZ_BUILD_GATE_REFUSED no_regression_inputs root={}",
                    path.display()
                )
            }
            Self::MissingRegressionTest {
                input,
                owner_crate,
                expected_test,
            } => write!(
                f,
                "FUZZ_BUILD_GATE_RED artifact={input} owner={owner_crate} reason=missing_regression_test expected={expected_test}"
            ),
            Self::WrongPlacement { actual } => write!(
                f,
                "FUZZ_BUILD_GATE_REFUSED placement={} required={REQUIRED_WORKER}",
                if actual.is_empty() { "missing" } else { actual }
            ),
            Self::BuildFailed { targets, detail } => write!(
                f,
                "FUZZ_BUILD_GATE_RED targets=[{}] reason=compile_failed detail={detail}",
                targets.join(",")
            ),
            Self::BuildTimedOut => f.write_str("FUZZ_BUILD_GATE_REFUSED reason=build_timeout"),
            Self::BuildUnspawned(detail) => {
                write!(
                    f,
                    "FUZZ_BUILD_GATE_REFUSED reason=build_unspawned detail={detail}"
                )
            }
        }
    }
}

impl std::error::Error for GateError {}

fn io_error(path: &Path, error: impl fmt::Display) -> GateError {
    GateError::Io {
        path: path.to_owned(),
        detail: error.to_string(),
    }
}

fn quoted_value(line: &str, key: &str) -> Option<String> {
    let (left, right) = line.split_once('=')?;
    if left.trim() != key {
        return None;
    }
    let value = right.trim();
    Some(value.strip_prefix('"')?.strip_suffix('"')?.to_owned())
}

/// Path-dep rows in the fuzz workspace look like `foo = { path = "../crates/foo" }`.
fn is_crates_path_dep(rhs: &str) -> bool {
    rhs.contains(r##"path = "../crates/"##)
}
fn dependency_crates(manifest: &str) -> Vec<String> {
    let mut in_dependencies = false;
    let mut names = Vec::new();
    for raw in manifest.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_dependencies = line == "[dependencies]";
            continue;
        }
        if !in_dependencies || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, rhs)) = line.split_once('=') else {
            continue;
        };
        if is_crates_path_dep(rhs) {
            names.push(name.trim().to_owned());
        }
    }
    names
}

fn owner_for_target(name: &str, dependencies: &[String]) -> Option<String> {
    dependencies
        .iter()
        .filter_map(|crate_name| {
            let prefix = crate_name.replace('-', "_");
            (name == prefix || name.starts_with(&(prefix.clone() + "_")))
                .then_some((prefix.len(), crate_name.clone()))
        })
        .max_by_key(|(length, _)| *length)
        .map(|(_, name)| name)
}

pub fn parse_targets(manifest: &str) -> Result<Vec<FuzzTarget>, GateError> {
    let dependencies = dependency_crates(manifest);
    let mut in_bin = false;
    let mut names = Vec::new();
    let mut current = None;
    for raw in manifest.lines() {
        let line = raw.trim();
        if line == "[[bin]]" {
            if let Some(name) = current.take() {
                names.push(name);
            }
            in_bin = true;
            continue;
        }
        if line.starts_with('[') {
            if in_bin {
                if let Some(name) = current.take() {
                    names.push(name);
                }
                in_bin = false;
            }
            continue;
        }
        if in_bin && current.is_none() {
            current = quoted_value(line, "name");
        }
    }
    if in_bin {
        if let Some(name) = current {
            names.push(name);
        }
    }
    if names.is_empty() {
        return Err(GateError::NoTargets(PathBuf::from("fuzz/Cargo.toml")));
    }
    names
        .into_iter()
        .map(|name| {
            let owner_crate = owner_for_target(&name, &dependencies)
                .ok_or_else(|| GateError::UnknownTargetOwner { target: name.clone() })?;
            Ok(FuzzTarget { name, owner_crate })
        })
        .collect()
}

fn collect_files(
    directory: &Path,
    root: &Path,
    predicate: &dyn Fn(&Path) -> bool,
    output: &mut Vec<PathBuf>,
) -> Result<(), GateError> {
    if !directory.is_dir() {
        return Ok(());
    }
    let entries = fs::read_dir(directory).map_err(|error| io_error(directory, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| io_error(directory, error))?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| io_error(&path, error))?;
        if file_type.is_dir() {
            collect_files(&path, root, predicate, output)?;
        } else if file_type.is_file() && predicate(&path) {
            output.push(
                path.strip_prefix(root)
                    .map_err(|error| io_error(&path, error))?
                    .to_owned(),
            );
        }
    }
    Ok(())
}

fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn regression_test_name(target: &str, path: &str) -> String {
    let file = path.rsplit('/').next().unwrap_or(path);
    let raw = format!("regression_{target}_{file}");
    raw.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

pub fn discover_inputs(
    repo_root: &Path,
    targets: &[FuzzTarget],
) -> Result<Vec<RegressionInput>, GateError> {
    let mut inputs = Vec::new();
    for target in targets {
        let artifact_dir = repo_root.join("fuzz/artifacts").join(&target.name);
        let corpus_dir = repo_root.join("fuzz/corpus").join(&target.name);
        let mut paths = Vec::new();
        collect_files(&artifact_dir, repo_root, &|_| true, &mut paths)?;
        collect_files(
            &corpus_dir,
            repo_root,
            &|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("seed-"))
            },
            &mut paths,
        )?;
        paths.sort();
        for path in paths {
            let path = slash_path(&path);
            inputs.push(RegressionInput {
                test_name: regression_test_name(&target.name, &path),
                target: target.name.clone(),
                path,
                owner_crate: target.owner_crate.clone(),
            });
        }
    }
    if inputs.is_empty() {
        return Err(GateError::NoRegressionInputs(repo_root.join("fuzz")));
    }
    Ok(inputs)
}

fn test_sources(
    repo_root: &Path,
    owner_crate: &str,
) -> Result<Vec<(String, String)>, GateError> {
    let tests_root = repo_root.join("crates").join(owner_crate).join("tests");
    let mut paths = Vec::new();
    collect_files(
        &tests_root,
        repo_root,
        &|path| path.extension().and_then(|e| e.to_str()) == Some("rs"),
        &mut paths,
    )?;
    let mut sources = Vec::new();
    for relative in paths {
        let absolute = repo_root.join(&relative);
        let source = fs::read_to_string(&absolute).map_err(|error| io_error(&absolute, error))?;
        sources.push((slash_path(&relative), source));
    }
    Ok(sources)
}

fn source_has_regression(source: &str, input: &RegressionInput) -> bool {
    let function_marker = format!("fn {}", input.test_name);
    source.contains(&function_marker)
        && source.split("include_bytes!").skip(1).any(|tail| {
            tail.split(')').next().unwrap_or_default().contains(&input.path)
        })
}

pub fn check_regression_contract(repo_root: &Path) -> Result<ContractReport, GateError> {
    let manifest_path = repo_root.join("fuzz/Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            GateError::MissingManifest(manifest_path.clone())
        } else {
            io_error(&manifest_path, error)
        }
    })?;
    let targets = parse_targets(&manifest)?;
    let inputs = discover_inputs(repo_root, &targets)?;
    let mut source_cache: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for input in &inputs {
        let sources = if let Some((_, sources)) = source_cache
            .iter()
            .find(|(owner, _)| owner == &input.owner_crate)
        {
            sources.clone()
        } else {
            let sources = test_sources(repo_root, &input.owner_crate)?;
            source_cache.push((input.owner_crate.clone(), sources.clone()));
            sources
        };
        if !sources
            .iter()
            .any(|(_, source)| source_has_regression(source, input))
        {
            return Err(GateError::MissingRegressionTest {
                input: input.path.clone(),
                owner_crate: input.owner_crate.clone(),
                expected_test: input.test_name.clone(),
            });
        }
    }
    Ok(ContractReport {
        targets: targets.len(),
        regression_inputs: inputs.len(),
    })
}

pub fn validate_placement(placement: &str) -> Result<(), GateError> {
    if placement.trim() == REQUIRED_WORKER {
        Ok(())
    } else {
        Err(GateError::WrongPlacement {
            actual: placement.trim().to_owned(),
        })
    }
}

fn summarize_output(stdout: &[u8], stderr: &[u8]) -> String {
    let mut detail = String::from_utf8_lossy(stdout).to_string();
    if !stderr.is_empty() {
        if !detail.is_empty() {
            detail.push('\n');
        }
        detail.push_str(&String::from_utf8_lossy(stderr));
    }
    detail.chars().take(2000).collect()
}

pub fn run_gate(repo_root: &Path, worker: &str, jobs: usize) -> Result<ContractReport, GateError> {
    validate_placement(worker)?;
    let report = check_regression_contract(repo_root)?;
    let manifest = repo_root.join("fuzz/Cargo.toml");
    let mut command = std::process::Command::new("cargo");
    command
        .arg("check")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--jobs")
        .arg(jobs.to_string());
    match bounded_output(&mut command, BUILD_DEADLINE) {
        BoundedOutcome::Completed(output) if output.status.success() => Ok(report),
        BoundedOutcome::Completed(output) => Err(GateError::BuildFailed {
            targets: Vec::new(),
            detail: summarize_output(&output.stdout, &output.stderr),
        }),
        BoundedOutcome::TimedOut => Err(GateError::BuildTimedOut),
        BoundedOutcome::Unspawned(error) => Err(GateError::BuildUnspawned(error.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FUZZ_MANIFEST: &str = r#"
[dependencies]
ack-stage = { path = "../crates/ack-stage" }
receiver-receipt = { path = "../crates/receiver-receipt" }

[[bin]]
name = "ack_stage_verdict_lattice"
path = "fuzz_targets/ack_stage_verdict_lattice.rs"

[[bin]]
name = "receiver_receipt_ack_wait"
path = "fuzz_targets/receiver_receipt_ack_wait.rs"
"#;

    #[test]
    fn parse_targets_maps_path_deps_under_crates() {
        let targets = parse_targets(FUZZ_MANIFEST).expect("fixture manifest");
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].owner_crate, "ack-stage");
        assert_eq!(targets[1].owner_crate, "receiver-receipt");
    }

    #[test]
    fn nested_quote_specimen_does_not_lex_as_identifiers() {
        // The production defect was `rhs.contains("path = "../crates/")`.
        let rhs = r#" { path = "../crates/ack-stage" } "#;
        assert!(is_crates_path_dep(rhs));
        assert!(!is_crates_path_dep(r#" { path = "../elsewhere" } "#));
    }

    #[test]
    fn placement_refuses_anything_but_contabo_3() {
        assert!(validate_placement("contabo-3").is_ok());
        assert!(validate_placement("contabo-4").is_err());
    }
}

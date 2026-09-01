#![forbid(unsafe_code)]

//! Fail-closed admission for a crate arriving from control-plane.
//!
//! The gate is intentionally candidate-scoped. Existing crates are not
//! grandfathered when the gate is invoked, but the gate never turns a missing
//! observation into a pass. Clause 4 performs the expensive repository-green
//! check from a clean `git archive HEAD` extraction; the other clauses inspect
//! the candidate and its declared repository evidence.

use asupersync::process::Command;
use asupersync::Cx;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use subprocess_contract::run_output;

pub const SCHEMA_VERSION: &str = "porting-gate/v1";
pub const CLAUSE_CODES: [&str; 6] = [
    "CLAUSE_1_WIRED",
    "CLAUSE_2_SURFACE_DECLARED",
    "CLAUSE_3_ASUPERSYNC",
    "CLAUSE_4_REPOSITORY_GREEN",
    "CLAUSE_5_NO_SH_PY",
    "CLAUSE_6_INVENTORY_FIELDS",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ClauseState {
    Pass,
    Refused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateStatus {
    Pass,
    Refused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClauseResult {
    pub code: &'static str,
    pub state: ClauseState,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortingFacts {
    pub wired: bool,
    pub surface_declared: bool,
    pub asupersync_conformant: bool,
    pub repository_green: bool,
    pub no_shell_or_python: bool,
    pub inventory_fields_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortingReport {
    pub schema_version: &'static str,
    pub crate_name: String,
    pub status: GateStatus,
    pub clauses: Vec<ClauseResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateError {
    EmptyCandidates,
    InvalidCandidate(String),
    Io(String),
    Metadata(String),
}

impl fmt::Display for GateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCandidates => f.write_str(
                "ANTI_VACUITY: zero crates examined — a porting gate that checks nothing proves nothing",
            ),
            Self::InvalidCandidate(name) => write!(f, "INVALID_CANDIDATE: {name}"),
            Self::Io(detail) => write!(f, "PORTING_GATE_IO: {detail}"),
            Self::Metadata(detail) => write!(f, "PORTING_GATE_METADATA: {detail}"),
        }
    }
}

impl std::error::Error for GateError {}

/// Evaluate the six landing clauses against already-measured facts.
pub fn assess(crate_name: &str, facts: PortingFacts) -> PortingReport {
    let checks = [
        (
            CLAUSE_CODES[0],
            facts.wired,
            "production caller exists outside the candidate",
        ),
        (
            CLAUSE_CODES[1],
            facts.surface_declared,
            "OMP-SURFACE-MAP.toml declares the candidate",
        ),
        (
            CLAUSE_CODES[2],
            facts.asupersync_conformant,
            "ASUPERSYNC-CONFORMANCE.md and manifest contract pass",
        ),
        (
            CLAUSE_CODES[3],
            facts.repository_green,
            "candidate suite passes from git archive extraction",
        ),
        (
            CLAUSE_CODES[4],
            facts.no_shell_or_python,
            "candidate carries no .sh or .py file",
        ),
        (
            CLAUSE_CODES[5],
            facts.inventory_fields_complete,
            "inventory row has all required evidence fields",
        ),
    ];
    let clauses = checks
        .into_iter()
        .map(|(code, passed, detail)| ClauseResult {
            code,
            state: if passed {
                ClauseState::Pass
            } else {
                ClauseState::Refused
            },
            detail: detail.to_owned(),
        })
        .collect::<Vec<_>>();
    let status = if clauses
        .iter()
        .all(|clause| clause.state == ClauseState::Pass)
    {
        GateStatus::Pass
    } else {
        GateStatus::Refused
    };
    PortingReport {
        schema_version: SCHEMA_VERSION,
        crate_name: crate_name.to_owned(),
        status,
        clauses,
    }
}

/// Evaluate a non-empty candidate list without silently passing an empty scan.
pub fn assess_many(candidates: Vec<(String, PortingFacts)>) -> Result<Vec<PortingReport>, GateError> {
    if candidates.is_empty() {
        return Err(GateError::EmptyCandidates);
    }
    Ok(candidates
        .into_iter()
        .map(|(name, facts)| assess(&name, facts))
        .collect())
}

fn valid_candidate(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn read_tree(root: &Path) -> Result<Vec<(PathBuf, String)>, GateError> {
    let mut stack = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).map_err(|error| GateError::Io(error.to_string()))?;
        for entry in entries {
            let path = entry
                .map_err(|error| GateError::Io(error.to_string()))?
                .path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let text = fs::read_to_string(&path).unwrap_or_default();
                files.push((path, text));
            }
        }
    }
    Ok(files)
}

fn candidate_files(root: &Path, name: &str) -> Result<Vec<(PathBuf, String)>, GateError> {
    let candidate = root.join("crates").join(name);
    if !candidate.is_dir() {
        return Err(GateError::InvalidCandidate(name.to_owned()));
    }
    read_tree(&candidate)
}

fn wired_from_metadata(metadata: &Value, root: &Path, name: &str) -> bool {
    let dependency_wired = metadata
        .get("packages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|package| package.get("name").and_then(Value::as_str) != Some(name))
        .any(|package| {
            package
                .get("dependencies")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .any(|dependency| dependency.get("name").and_then(Value::as_str) == Some(name))
        });
    if dependency_wired {
        return true;
    }
    [".github", "launchd"]
        .into_iter()
        .map(|directory| root.join(directory))
        .filter(|directory| directory.is_dir())
        .filter_map(|directory| read_tree(&directory).ok())
        .flatten()
        .any(|(_, text)| text.contains(name))
}

fn surface_block<'a>(map: &'a str, name: &str) -> Option<&'a str> {
    let marker = format!("[crates.{name}]");
    let start = map.find(&marker)? + marker.len();
    let rest = &map[start..];
    let end = rest.find("\n[").unwrap_or(rest.len());
    Some(&rest[..end])
}

fn surface_fields_complete(root: &Path, name: &str) -> Result<bool, GateError> {
    let map = fs::read_to_string(root.join("OMP-SURFACE-MAP.toml"))
        .map_err(|error| GateError::Io(error.to_string()))?;
    let Some(block) = surface_block(&map, name) else {
        return Ok(false);
    };
    let required = [
        "classification",
        "omp_surface",
        "inputs",
        "outputs",
        "what_must_be_true",
        "negative_evidence",
    ];
    if required
        .iter()
        .any(|field| !block.lines().any(|line| line.trim_start().starts_with(&format!("{field} ="))))
    {
        return Ok(false);
    }
    let classification = block
        .lines()
        .find_map(|line| line.trim().strip_prefix("classification = "))
        .unwrap_or_default();
    if classification.trim_matches('"') == "b" {
        return Ok(block.lines().any(|line| {
            line.trim_start().starts_with("omp_alternative =")
                && !line.trim_end().ends_with("= \"\"")
        }));
    }
    Ok(matches!(classification.trim_matches('"'), "a" | "c"))
}

fn asupersync_conformant(root: &Path, name: &str, manifest: &str) -> Result<bool, GateError> {
    let table = fs::read_to_string(root.join("ASUPERSYNC-CONFORMANCE.md"))
        .map_err(|error| GateError::Io(error.to_string()))?;
    let row_exists = table.lines().any(|line| {
        line.split_whitespace().next() == Some(name)
    });
    let unsafe_forbid = manifest.contains("unsafe_code = \"forbid\"");
    let asupersync = manifest.contains("asupersync");
    let source_has_command = candidate_files(root, name)?
        .into_iter()
        .any(|(_, text)| text.contains("Command::new"));
    let subprocess_contract = manifest.contains("subprocess-contract");
    Ok(row_exists && unsafe_forbid && asupersync && (!source_has_command || subprocess_contract))
}

fn no_shell_or_python(files: &[(PathBuf, String)]) -> bool {
    files.iter().all(|(path, _)| {
        !matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("sh") | Some("py")
        )
    })
}

fn archive_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "porting-gate-archive-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ))
}

async fn archive_green(cx: &Cx, root: &Path, name: &str) -> Result<bool, GateError> {
    cx.checkpoint()
        .map_err(|_| GateError::Metadata("cancelled before archive check".to_owned()))?;
    let archive_root = archive_path();
    fs::create_dir_all(&archive_root).map_err(|error| GateError::Io(error.to_string()))?;
    let archive_file = archive_root.join("source.tar");

    let mut archive = Command::new("git");
    archive.args(["archive", "--format=tar", "HEAD"]);
    archive.current_dir(root);
    let archive_output = run_output(cx, archive)
        .await
        .map_err(|error| GateError::Metadata(format!("git archive: {error}")))?;
    if !archive_output.status.success() {
        let _ = fs::remove_dir_all(&archive_root);
        return Ok(false);
    }
    fs::write(&archive_file, archive_output.stdout)
        .map_err(|error| GateError::Io(error.to_string()))?;

    let mut extract = Command::new("tar");
    extract.args(["-xf", &archive_file.display().to_string()]);
    extract.current_dir(&archive_root);
    let extract_output = run_output(cx, extract)
        .await
        .map_err(|error| GateError::Metadata(format!("tar extract: {error}")))?;
    if !extract_output.status.success() {
        let _ = fs::remove_dir_all(&archive_root);
        return Ok(false);
    }

    let target_dir = archive_root.join("target");
    let mut test = Command::new("cargo");
    test.args(["test", "--quiet", "-p", name]);
    test.current_dir(&archive_root);
    test.env("CARGO_TARGET_DIR", &target_dir);
    let test_output = run_output(cx, test)
        .await
        .map_err(|error| GateError::Metadata(format!("archive cargo test: {error}")))?;
    let passed = test_output.status.success();
    let _ = fs::remove_dir_all(&archive_root);
    Ok(passed)
}

/// Run the complete gate for one candidate crate.
pub async fn check_candidate(cx: &Cx, root: &Path, name: &str) -> Result<PortingReport, GateError> {
    if !valid_candidate(name) {
        return Err(GateError::InvalidCandidate(name.to_owned()));
    }
    let files = candidate_files(root, name)?;
    let manifest_path = root.join("crates").join(name).join("Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path)
        .map_err(|error| GateError::Io(error.to_string()))?;

    let mut cargo = Command::new("cargo");
    cargo.args(["metadata", "--no-deps", "--format-version", "1"]);
    cargo.current_dir(root);
    let metadata_output = run_output(cx, cargo)
        .await
        .map_err(|error| GateError::Metadata(format!("cargo metadata: {error}")))?;
    if !metadata_output.status.success() {
        return Err(GateError::Metadata(
            String::from_utf8_lossy(&metadata_output.stderr).into_owned(),
        ));
    }
    let metadata: Value = serde_json::from_slice(&metadata_output.stdout)
        .map_err(|error| GateError::Metadata(error.to_string()))?;

    let wired = wired_from_metadata(&metadata, root, name);
    let surface_declared = surface_block(
        &fs::read_to_string(root.join("OMP-SURFACE-MAP.toml"))
            .map_err(|error| GateError::Io(error.to_string()))?,
        name,
    )
    .is_some();
    let asupersync_ok = asupersync_conformant(root, name, &manifest)?;
    let repository_green = archive_green(cx, root, name).await?;
    let no_shell = no_shell_or_python(&files);
    let inventory_fields = surface_fields_complete(root, name)?;

    Ok(assess(
        name,
        PortingFacts {
            wired,
            surface_declared,
            asupersync_conformant: asupersync_ok,
            repository_green,
            no_shell_or_python: no_shell,
            inventory_fields_complete: inventory_fields,
        },
    ))
}

#![forbid(unsafe_code)]

//! Worker-oracle classification for tests whose verdict depends on checkout identity.
//!
//! The ledger is the single target list. This crate never claims that an RCH sync is complete;
//! it refuses a HOST_BOUND target when the worker lacks the surface that target reads.

use serde_json::Value;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub const LEDGER_RELATIVE_PATH: &str = "crates/worker-oracle-gate/src/TEST-ORACLE.jsonl";

/// The machine ledger is kept in Rust so an RCH source transfer cannot silently drop it.
/// The census reads this one string; there is no second target list.
const TEST_ORACLE_LEDGER: &str = r#"{"target":"no-shell-gate::findings_ledger","classification":"HOST_BOUND","source":"crates/no-shell-gate/tests/findings_ledger.rs","reason":"reads git history and fixed-pointer commits","surface_marker":"git show"}
{"target":"omp-orchestrator::gate_wiring_wave3","classification":"HOST_BOUND","source":"crates/omp-orchestrator/tests/gate_wiring_wave3.rs","reason":"census derives reachability from host git and hook state","surface_marker":"census_gates"}
{"target":"no-shell-gate::cross_section_authority","classification":"HOST_BOUND","source":"crates/no-shell-gate/tests/cross_section_authority.rs","reason":"reads the tracked cross-section authority registry","surface_marker":"CROSS-SECTION-AUTHORITY"}
{"target":"no-shell-gate::numbers","classification":"HOST_BOUND","source":"crates/no-shell-gate/tests/numbers.rs","reason":"re-runs host paths and environment-backed figures","surface_marker":"NUMBERS.toml"}
{"target":"omp-types::claim_strength_laws","classification":"TREE_PURE","source":"crates/omp-types/tests/claim_strength_laws.rs","reason":"pure type laws over checked-in Rust values","surface_marker":"claim_strength"}
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    HostBound,
    TreePure,
}

impl Classification {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "HOST_BOUND" => Some(Self::HostBound),
            "TREE_PURE" => Some(Self::TreePure),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::HostBound => "HOST_BOUND",
            Self::TreePure => "TREE_PURE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetRecord {
    pub target: String,
    pub source_path: PathBuf,
    pub classification: Classification,
    pub reason: String,
    pub surface_marker: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CensusReport {
    pub ledger_targets: usize,
    pub ledger_host_bound: usize,
    pub marker_matches: usize,
    pub source_host_bound: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetVerdict {
    TreePure { target: String },
    HostBoundLocal { target: String },
    HostBoundRemote { target: String, worker: String },
}

#[derive(Debug)]
pub enum OracleError {
    Io { path: PathBuf, detail: String },
    MissingLedger(PathBuf),
    EmptyLedger(PathBuf),
    MalformedLedger { path: PathBuf, line: usize, detail: String },
    DuplicateTarget(String),
    InvalidClassification { target: String, value: String },
    MissingSource { target: String, path: PathBuf },
    MissingSurfaceMarker { target: String, marker: String },
    ZeroHostBound { source_host_bound: usize },
    MissingTarget(String),
    HostBoundRefused { target: String, worker: String, surface: &'static str },
}

impl fmt::Display for OracleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, detail } => write!(f, "WORKER_ORACLE_IO path={} detail={detail}", path.display()),
            Self::MissingLedger(path) => write!(f, "WORKER_ORACLE_REFUSED reason=ledger_missing path={}", path.display()),
            Self::EmptyLedger(path) => write!(f, "WORKER_ORACLE_REFUSED reason=ledger_empty path={}", path.display()),
            Self::MalformedLedger { path, line, detail } => write!(f, "WORKER_ORACLE_REFUSED reason=ledger_malformed path={} line={line} detail={detail}", path.display()),
            Self::DuplicateTarget(target) => write!(f, "WORKER_ORACLE_REFUSED reason=duplicate_target target={target}"),
            Self::InvalidClassification { target, value } => write!(f, "WORKER_ORACLE_REFUSED target={target} reason=invalid_classification value={value}"),
            Self::MissingSource { target, path } => write!(f, "WORKER_ORACLE_REFUSED target={target} reason=path_not_synced path={}", path.display()),
            Self::MissingSurfaceMarker { target, marker } => write!(f, "WORKER_ORACLE_REFUSED target={target} reason=ledger_marker_not_found marker={marker}"),
            Self::ZeroHostBound { source_host_bound } => write!(f, "WORKER_ORACLE_REFUSED reason=host_bound_ledger_empty grep_host_bound={source_host_bound}"),
            Self::MissingTarget(target) => write!(f, "WORKER_ORACLE_REFUSED reason=target_not_in_ledger target={target}"),
            Self::HostBoundRefused { target, worker, surface } => write!(f, "HOST_BOUND_REFUSED target={target} worker={worker} surface={surface}"),
        }
    }
}

impl std::error::Error for OracleError {}

fn io_error(path: &Path, error: impl fmt::Display) -> OracleError {
    OracleError::Io { path: path.to_owned(), detail: error.to_string() }
}

fn required_string(row: &Value, key: &str, path: &Path, line: usize) -> Result<String, OracleError> {
    row.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| OracleError::MalformedLedger { path: path.to_owned(), line, detail: format!("missing_{key}") })
}

pub fn load_ledger(repo_root: &Path) -> Result<Vec<TargetRecord>, OracleError> {
    let path = repo_root.join(LEDGER_RELATIVE_PATH);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => TEST_ORACLE_LEDGER.to_owned(),
        Err(error) => return Err(io_error(&path, error)),
    };
    let mut rows = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        if raw.trim().is_empty() { continue; }
        let row: Value = serde_json::from_str(raw).map_err(|error| OracleError::MalformedLedger { path: path.clone(), line, detail: error.to_string() })?;
        let target = required_string(&row, "target", &path, line)?;
        let classification_value = required_string(&row, "classification", &path, line)?;
        let classification = Classification::parse(&classification_value).ok_or_else(|| OracleError::InvalidClassification { target: target.clone(), value: classification_value })?;
        let source = required_string(&row, "source", &path, line)?;
        let reason = required_string(&row, "reason", &path, line)?;
        let surface_marker = required_string(&row, "surface_marker", &path, line)?;
        if rows.iter().any(|known: &TargetRecord| known.target == target) {
            return Err(OracleError::DuplicateTarget(target));
        }
        rows.push(TargetRecord { target, source_path: PathBuf::from(source), classification, reason, surface_marker });
    }
    if rows.is_empty() { return Err(OracleError::EmptyLedger(path)); }
    Ok(rows)
}

fn test_source(repo_root: &Path, record: &TargetRecord) -> Result<String, OracleError> {
    let path = repo_root.join(&record.source_path);
    fs::read_to_string(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            OracleError::MissingSource { target: record.target.clone(), path }
        } else {
            io_error(&path, error)
        }
    })
}

fn generic_host_surface(source: &str) -> bool {
    ["git log", "git remote", "git cat-file", "git show", ".git/hooks", "/Volumes/", ".local/state/", "census_gates", "CROSS-SECTION-AUTHORITY", "NUMBERS.toml"]
        .iter()
        .any(|marker| source.contains(marker))
}

pub fn census(repo_root: &Path) -> Result<CensusReport, OracleError> {
    let rows = load_ledger(repo_root)?;
    let mut marker_matches = 0;
    let mut ledger_host_bound = 0;
    for row in &rows {
        let source = test_source(repo_root, row)?;
        if source.contains(&row.surface_marker) {
            marker_matches += usize::from(row.classification == Classification::HostBound);
        } else if row.classification == Classification::HostBound {
            return Err(OracleError::MissingSurfaceMarker { target: row.target.clone(), marker: row.surface_marker.clone() });
        }
        ledger_host_bound += usize::from(row.classification == Classification::HostBound);
    }
    let source_host_bound = rows
        .iter()
        .filter(|row| row.classification == Classification::HostBound)
        .filter_map(|row| test_source(repo_root, row).ok())
        .filter(|source| generic_host_surface(source))
        .count();
    if ledger_host_bound == 0 && source_host_bound > 0 {
        return Err(OracleError::ZeroHostBound { source_host_bound });
    }
    if ledger_host_bound != marker_matches || ledger_host_bound != source_host_bound {
        return Err(OracleError::ZeroHostBound { source_host_bound });
    }
    Ok(CensusReport { ledger_targets: rows.len(), ledger_host_bound, marker_matches, source_host_bound })
}

pub fn classify_target(repo_root: &Path, target: &str, worker: &str) -> Result<TargetVerdict, OracleError> {
    let row = load_ledger(repo_root)?.into_iter().find(|row| row.target == target).ok_or_else(|| OracleError::MissingTarget(target.to_owned()))?;
    if row.classification == Classification::TreePure { return Ok(TargetVerdict::TreePure { target: target.to_owned() }); }
    if worker.trim().is_empty() || worker == "local" { return Ok(TargetVerdict::HostBoundLocal { target: target.to_owned() }); }
    let git_path = repo_root.join(".git");
    if !git_path.exists() { return Err(OracleError::HostBoundRefused { target: target.to_owned(), worker: worker.to_owned(), surface: "no git history on worker" }); }
    let git_config = git_path.join("config");
    let config = fs::read_to_string(&git_config).unwrap_or_default();
    if !config.contains(r#"[remote "origin"]"#) && !config.contains("remote.origin.url") {
        return Err(OracleError::HostBoundRefused { target: target.to_owned(), worker: worker.to_owned(), surface: "no origin" });
    }
    if !git_path.join("hooks/pre-commit").exists() {
        return Err(OracleError::HostBoundRefused { target: target.to_owned(), worker: worker.to_owned(), surface: "path not synced: .git/hooks/pre-commit" });
    }
    Ok(TargetVerdict::HostBoundRemote { target: target.to_owned(), worker: worker.to_owned() })
}

pub fn admission_check(repo_root: &Path) -> Result<CensusReport, OracleError> {
    census(repo_root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture() -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("crates/demo/tests")).unwrap();
        fs::create_dir_all(root.join(".git/hooks")).unwrap();
        fs::write(root.join(".git/config"), r#"[remote "origin"]
url = https://example.invalid/repo
"#).unwrap();
        fs::write(root.join(".git/hooks/pre-commit"), "gate").unwrap();
        fs::write(root.join("crates/demo/tests/test.rs"), r#"fn test() { let _ = "git remote"; }
"#).unwrap();
        fs::create_dir_all(root.join("crates/worker-oracle-gate/src")).unwrap();
        fs::write(root.join(LEDGER_RELATIVE_PATH), r#"{"target":"demo::test","classification":"HOST_BOUND","source":"crates/demo/tests/test.rs","reason":"reads git remote","surface_marker":"git remote"}
"#).unwrap();
        temp
    }

    #[test]
    fn census_reads_one_source_of_target_truth() {
        let temp = fixture();
        let report = census(temp.path()).unwrap();
        assert_eq!(report.ledger_targets, 1);
        assert_eq!(report.ledger_host_bound, report.marker_matches);
        assert_eq!(report.ledger_host_bound, report.source_host_bound);
    }

    #[test]
    fn remote_host_bound_target_refuses_missing_surface_with_text() {
        let temp = fixture();
        fs::remove_dir_all(temp.path().join(".git")).unwrap();
        let error = classify_target(temp.path(), "demo::test", "contabo-4").unwrap_err().to_string();
        assert!(error.contains("HOST_BOUND_REFUSED"));
        assert!(error.contains("no git history on worker"));
    }

    #[test]
    fn tree_pure_target_does_not_need_host_surfaces() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("crates/demo/tests")).unwrap();
        fs::create_dir_all(temp.path().join("crates/worker-oracle-gate/src")).unwrap();
        fs::write(temp.path().join("crates/demo/tests/test.rs"), r#"fn test() { assert!(true); }
"#).unwrap();
        fs::write(temp.path().join(LEDGER_RELATIVE_PATH), r#"{"target":"demo::test","classification":"TREE_PURE","source":"crates/demo/tests/test.rs","reason":"pure","surface_marker":"assert"}
"#).unwrap();
        assert!(matches!(classify_target(temp.path(), "demo::test", "contabo-4"), Ok(TargetVerdict::TreePure { .. })));
    }
}

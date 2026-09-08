#![forbid(unsafe_code)]

//! The mandatory `ompo` state-handling triad.
//!
//! `validate` reads one state artifact without writing or executing anything. `audit` reads the
//! existing lifecycle journal as the ledger. `why` traces one known object and reuses the shared
//! build-provenance and registry-freshness kernel instead of copying either implementation.

use crate::provenance::{probe_registry, BuildProvenance, RegistryProbe};
use serde::Serialize;
use serde_json::Value;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

const INCEPTION_PATH: &str = ".omp-orchestrator/inception.json";
const DOCTOR_PATH: &str = ".omp-orchestrator/doctor/report.json";
const LIFECYCLE_PATH: &str = ".omp-orchestrator/work/s1/lifecycle.jsonl";
const TRACEABLE_IDS: &str = "build, registry, lifecycle journal";

#[derive(Debug)]
pub enum StateError {
    MissingValue { command: &'static str },
    UnknownThing { thing: String },
    MissingArtifact { command: &'static str, path: PathBuf },
    ReadArtifact { command: &'static str, path: PathBuf, detail: String },
    InvalidArtifact { command: &'static str, path: PathBuf, detail: String },
    EmptyArtifact { command: &'static str, path: PathBuf },
    UnknownId { id: String },
}

impl StateError {
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingValue { command: "validate" } => "STATE_VALIDATE_MISSING_THING",
            Self::MissingValue { command: "audit" } => "STATE_AUDIT_MISSING_JOURNAL",
            Self::MissingValue { command: "why" } => "STATE_WHY_MISSING_ID",
            Self::MissingValue { command: _ } => "STATE_ARGUMENT_MISSING",
            Self::UnknownThing { .. } => "STATE_VALIDATE_UNKNOWN_THING",
            Self::MissingArtifact { command: "validate", .. } => "STATE_VALIDATE_MISSING",
            Self::MissingArtifact { command: "audit", .. } => "STATE_AUDIT_MISSING",
            Self::MissingArtifact { command: _, .. } => "STATE_ARTIFACT_MISSING",
            Self::ReadArtifact { command: "validate", .. } => "STATE_VALIDATE_READ_FAILED",
            Self::ReadArtifact { command: "audit", .. } => "STATE_AUDIT_READ_FAILED",
            Self::ReadArtifact { command: _, .. } => "STATE_ARTIFACT_READ_FAILED",
            Self::InvalidArtifact { command: "validate", .. } => "STATE_VALIDATE_INVALID",
            Self::InvalidArtifact { command: "audit", .. } => "STATE_AUDIT_INVALID",
            Self::InvalidArtifact { command: _, .. } => "STATE_ARTIFACT_INVALID",
            Self::EmptyArtifact { command: "validate", .. } => "STATE_VALIDATE_EMPTY",
            Self::EmptyArtifact { command: "audit", .. } => "STATE_AUDIT_EMPTY",
            Self::EmptyArtifact { command: _, .. } => "STATE_ARTIFACT_EMPTY",
            Self::UnknownId { .. } => "STATE_WHY_UNKNOWN_ID",
        }
    }
}

impl fmt::Display for StateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingValue { command } => write!(
                formatter,
                "{} command requires a value; traceable={TRACEABLE_IDS}",
                self.code_for(command)
            ),
            Self::UnknownThing { thing } => write!(
                formatter,
                "{} thing={thing:?} expected=inception|doctor|lifecycle|PATH",
                self.code()
            ),
            Self::MissingArtifact { path, .. } => {
                write!(formatter, "{} path={}", self.code(), path.display())
            }
            Self::ReadArtifact { path, detail, .. } => write!(
                formatter,
                "{} path={} detail={detail}",
                self.code(),
                path.display()
            ),
            Self::InvalidArtifact { path, detail, .. } => write!(
                formatter,
                "{} path={} detail={detail}",
                self.code(),
                path.display()
            ),
            Self::EmptyArtifact { path, .. } => {
                write!(formatter, "{} path={} reason=empty", self.code(), path.display())
            }
            Self::UnknownId { id } => write!(
                formatter,
                "{} id={id:?} reason=not_traceable traceable={TRACEABLE_IDS}",
                self.code()
            ),
        }
    }
}

impl StateError {
    fn code_for(&self, command: &str) -> &'static str {
        match command {
            "validate" => "STATE_VALIDATE_MISSING_THING",
            "audit" => "STATE_AUDIT_MISSING_JOURNAL",
            "why" => "STATE_WHY_MISSING_ID",
            _ => "STATE_ARGUMENT_MISSING",
        }
    }
}

impl std::error::Error for StateError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationReport {
    pub command: &'static str,
    pub thing: String,
    pub path: String,
    pub status: &'static str,
    pub records: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuditEntry {
    pub object_id: Option<String>,
    pub actor: String,
    pub when: String,
    pub why: String,
    pub sha: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuditReport {
    pub command: &'static str,
    pub source: String,
    pub total_entries: usize,
    pub entries: Vec<AuditEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WhyReport {
    pub command: &'static str,
    pub id: String,
    pub source: String,
    pub provenance: BuildProvenance,
    pub registry: Option<RegistryProbe>,
    pub journal_matches: Vec<AuditEntry>,
}

/// Validate one known state artifact. This function only reads and parses bytes.
#[must_use = "validation results carry the evidence or refusal"]
pub fn validate(repo: &Path, thing: &str) -> Result<ValidationReport, StateError> {
    let (thing, path, is_journal) = resolve_thing(repo, thing)?;
    let text = read_artifact("validate", &path)?;
    let records = if is_journal {
        parse_journal(&text, &path)?.len()
    } else {
        parse_json_object(&text, &path)?;
        1
    };
    if records == 0 {
        return Err(StateError::EmptyArtifact {
            command: "validate",
            path,
        });
    }
    Ok(ValidationReport {
        command: "validate",
        thing,
        path: path.display().to_string(),
        status: "VALID",
        records,
    })
}

/// Read the existing lifecycle journal as the audit ledger. No second ledger is created.
#[must_use = "audit results carry the journal evidence or refusal"]
pub fn audit(repo: &Path, limit: usize) -> Result<AuditReport, StateError> {
    let path = repo.join(LIFECYCLE_PATH);
    let text = read_artifact("audit", &path)?;
    let all_entries = parse_journal(&text, &path)?;
    let start = all_entries.len().saturating_sub(limit);
    Ok(AuditReport {
        command: "audit",
        source: path.display().to_string(),
        total_entries: all_entries.len(),
        entries: all_entries.into_iter().skip(start).collect(),
    })
}

/// Trace one known object through the shared provenance kernel and, for registry, its live probe.
#[must_use = "why results carry the provenance evidence or refusal"]
pub fn why(
    repo: &Path,
    id: &str,
    registered_adapters: &[&str],
) -> Result<WhyReport, StateError> {
    let id = id.trim();
    if id.is_empty() {
        return Err(StateError::MissingValue { command: "why" });
    }
    let provenance = BuildProvenance::current();
    match id {
        "build" => Ok(WhyReport {
            command: "why",
            id: id.to_owned(),
            source: "build-provenance".to_owned(),
            provenance,
            registry: None,
            journal_matches: Vec::new(),
        }),
        "registry" => Ok(WhyReport {
            command: "why",
            id: id.to_owned(),
            source: "registry-staleness".to_owned(),
            provenance,
            registry: Some(probe_registry(repo, registered_adapters)),
            journal_matches: Vec::new(),
        }),
        "lifecycle" | "journal" => {
            let report = audit(repo, 20)?;
            Ok(WhyReport {
                command: "why",
                id: id.to_owned(),
                source: report.source,
                provenance,
                registry: None,
                journal_matches: report.entries,
            })
        }
        _ => {
            let path = repo.join(LIFECYCLE_PATH);
            let entries = parse_journal(&read_artifact("why", &path)?, &path)?;
            let matches = entries
                .into_iter()
                .filter(|entry| entry.object_id.as_deref() == Some(id))
                .collect::<Vec<_>>();
            if matches.is_empty() {
                return Err(StateError::UnknownId { id: id.to_owned() });
            }
            Ok(WhyReport {
                command: "why",
                id: id.to_owned(),
                source: path.display().to_string(),
                provenance,
                registry: None,
                journal_matches: matches,
            })
        }
    }
}

fn resolve_thing(repo: &Path, thing: &str) -> Result<(String, PathBuf, bool), StateError> {
    match thing {
        "inception" => Ok((thing.to_owned(), repo.join(INCEPTION_PATH), false)),
        "doctor" => Ok((thing.to_owned(), repo.join(DOCTOR_PATH), false)),
        "lifecycle" | "journal" => Ok((thing.to_owned(), repo.join(LIFECYCLE_PATH), true)),
        value if value.contains('/') || value.ends_with(".json") || value.ends_with(".jsonl") => {
            let path = Path::new(value);
            let path = if path.is_absolute() {
                path.to_path_buf()
            } else {
                repo.join(path)
            };
            Ok((value.to_owned(), path.clone(), value.ends_with(".jsonl")))
        }
        _ => Err(StateError::UnknownThing {
            thing: thing.to_owned(),
        }),
    }
}

fn read_artifact(command: &'static str, path: &Path) -> Result<String, StateError> {
    if !path.is_file() {
        return Err(StateError::MissingArtifact {
            command,
            path: path.to_path_buf(),
        });
    }
    fs::read_to_string(path).map_err(|error| StateError::ReadArtifact {
        command,
        path: path.to_path_buf(),
        detail: error.to_string(),
    })
}

fn parse_json_object(text: &str, path: &Path) -> Result<(), StateError> {
    let value: Value = serde_json::from_str(text).map_err(|error| StateError::InvalidArtifact {
        command: "validate",
        path: path.to_path_buf(),
        detail: error.to_string(),
    })?;
    if !value.is_object() {
        return Err(StateError::InvalidArtifact {
            command: "validate",
            path: path.to_path_buf(),
            detail: "expected a JSON object".to_owned(),
        });
    }
    Ok(())
}

fn parse_journal(text: &str, path: &Path) -> Result<Vec<AuditEntry>, StateError> {
    let mut entries = Vec::new();
    for (line_number, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line).map_err(|error| StateError::InvalidArtifact {
            command: "audit",
            path: path.to_path_buf(),
            detail: format!("line {}: {error}", line_number + 1),
        })?;
        let object = value.as_object().ok_or_else(|| StateError::InvalidArtifact {
            command: "audit",
            path: path.to_path_buf(),
            detail: format!("line {}: expected a JSON object", line_number + 1),
        })?;
        entries.push(AuditEntry {
            object_id: string_field(object, &["id", "step", "name"]),
            actor: string_field(object, &["actor", "author", "who"])
                .unwrap_or_else(|| "unknown".to_owned()),
            when: timestamp_field(object),
            why: string_field(object, &["reason_code", "why", "reason"])
                .unwrap_or_else(|| "unknown".to_owned()),
            sha: string_field(object, &["sha", "commit", "source_revision"]),
        });
    }
    if entries.is_empty() {
        return Err(StateError::EmptyArtifact {
            command: "audit",
            path: path.to_path_buf(),
        });
    }
    Ok(entries)
}

fn string_field(object: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str).map(str::to_owned))
}

fn timestamp_field(object: &serde_json::Map<String, Value>) -> String {
    if let Some(value) = object.get("ts_unix").and_then(Value::as_u64) {
        return value.to_string();
    }
    string_field(object, &["timestamp", "ts", "when"]).unwrap_or_else(|| "unknown".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut files = BTreeMap::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(path) = pending.pop() {
            let metadata = fs::metadata(&path).expect("fixture metadata");
            if metadata.is_dir() {
                for entry in fs::read_dir(&path).expect("fixture directory") {
                    pending.push(entry.expect("fixture entry").path());
                }
            } else {
                let bytes = fs::read(&path).expect("fixture bytes");
                files.insert(path, bytes);
            }
        }
        files
    }

    #[test]
    fn validate_is_pure_read_across_two_runs() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join(INCEPTION_PATH);
        fs::create_dir_all(path.parent().expect("parent")).expect("state directory");
        fs::write(&path, br#"{"schema_version":"inception.v1","actions":0}"#)
            .expect("inception fixture");
        let before = snapshot(temp.path());
        let first = validate(temp.path(), "inception").expect("valid inception");
        let second = validate(temp.path(), "inception").expect("valid inception");
        assert_eq!(first, second);
        assert_eq!(snapshot(temp.path()), before, "validate must not mutate state");
    }

    #[test]
    fn audit_reads_the_existing_journal_without_creating_a_ledger() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join(LIFECYCLE_PATH);
        fs::create_dir_all(path.parent().expect("parent")).expect("state directory");
        fs::write(
            &path,
            "{\"actor\":\"tester\",\"reason_code\":\"ONE\",\"ts_unix\":1}\n{\"actor\":\"tester\",\"reason_code\":\"TWO\",\"ts_unix\":2}\n",
        )
        .expect("journal fixture");
        let before = snapshot(temp.path());
        let report = audit(temp.path(), 20).expect("journal audit");
        assert_eq!(report.source, path.display().to_string());
        assert_eq!(report.total_entries, 2);
        assert_eq!(report.entries.len(), 2);
        assert_eq!(snapshot(temp.path()), before, "audit must not create a second ledger");
    }

    #[test]
    fn why_build_uses_the_shared_provenance_kernel() {
        let temp = tempdir().expect("tempdir");
        let report = why(temp.path(), "build", &[]).expect("build trace");
        assert_eq!(report.source, "build-provenance");
        assert_eq!(report.provenance, BuildProvenance::current());
        assert!(report.registry.is_none());
    }

    #[test]
    fn why_registry_uses_the_shared_provenance_probe() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("crate has a workspace root");
        let report = why(repo, "registry", &["planted-phantom"]).expect("registry trace");
        let registry = report.registry.expect("registry probe");
        assert_eq!(registry.provenance, report.provenance);
        assert_eq!(registry.baked_adapter_count, 1);
        assert!(registry.live_bin_target_count.is_some_and(|count| count > 0));
        assert_eq!(registry.status, "STALE");
        assert_eq!(registry.reason_code, "OMPO_REGISTRY_STALE");
    }
    #[test]
    fn bare_why_is_a_typed_refusal() {
        let temp = tempdir().expect("tempdir");
        let error = why(temp.path(), "", &[]).expect_err("bare why must refuse");
        assert_eq!(error.code(), "STATE_WHY_MISSING_ID");
        assert!(error.to_string().contains("traceable=build, registry, lifecycle journal"));
    }

    #[test]
    fn triad_reports_are_json_serializable() {
        let temp = tempdir().expect("tempdir");
        let inception = temp.path().join(INCEPTION_PATH);
        let journal = temp.path().join(LIFECYCLE_PATH);
        fs::create_dir_all(inception.parent().expect("parent")).expect("state directory");
        fs::create_dir_all(journal.parent().expect("parent" )).expect("journal directory");
        fs::write(&inception, br#"{"ok":true}"#).expect("inception fixture");
        fs::write(&journal, "{\"actor\":\"tester\",\"reason_code\":\"ONE\",\"ts_unix\":1}\n")
            .expect("journal fixture");
        let validate_value = serde_json::to_value(validate(temp.path(), "inception").expect("validate"))
            .expect("validate JSON");
        let audit_value = serde_json::to_value(audit(temp.path(), 1).expect("audit"))
            .expect("audit JSON");
        let why_value = serde_json::to_value(why(temp.path(), "build", &[]).expect("why"))
            .expect("why JSON");
        assert_eq!(validate_value["status"], "VALID");
        assert_eq!(audit_value["command"], "audit");
        assert_eq!(why_value["command"], "why");
    }
}

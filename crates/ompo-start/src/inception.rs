#![forbid(unsafe_code)]

//! S1 inception manifest writer.
//!
//! The writer establishes the durable repository boundary consumed by later
//! stages. It records identity, control-file presence, host facts, required
//! tool names, and an explicit trust status, then reads the JSON back before
//! returning success.

use sha2::{Digest, Sha256};
use lifecycle_event::{
    default_repo_journal, DurableJournal, EmitOutcome, Layer, LifecycleEvent, ReasonCode,
};
use serde_json::Value;
use lifecycle_monitor::verify_artifact;
use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SCHEMA_VERSION: &str = "inception.v1";

const REQUIRED_CONTROL_FILES: &[&str] = &[
    "AGENTS.md",
    "CLAUDE.md",
    "README.md",
    "Cargo.toml",
    "SCHEMAS.toml",
    "docs/decisions.jsonl",
];

/// The control files an initialised repository must carry, exposed read-only.
///
/// `health` needs the same list `initialize` refuses on, and a second copy would drift the
/// moment either side changed. Reused rather than re-derived.
#[must_use]
pub fn required_control_files() -> &'static [&'static str] {
    REQUIRED_CONTROL_FILES
}

/// Presence of each required control file under `repo`, WITHOUT writing anything.
///
/// This is the single computation behind both `initialize`'s
/// `INCEPTION_CONTROL_FILES_MISSING` refusal and `ompo health`'s read-only signal, so the
/// two can never disagree about what "complete" means.
#[must_use]
pub fn control_file_presence(repo: &Path) -> BTreeMap<String, bool> {
    REQUIRED_CONTROL_FILES
        .iter()
        .map(|relative| ((*relative).to_owned(), repo.join(relative).is_file()))
        .collect()
}

const REQUIRED_TOOLS: &[&str] = &["git", "cargo", "br", "bv", "ntm", "am", "jq"];
const REQUIRED_KEYS: &[&str] = &[
    "schema_version",
    "project_id",
    "repo_identity",
    "control_files",
    "host_capabilities",
    "required_tools",
    "trust_status",
];

#[derive(Debug)]
pub enum InceptionError {
    RepositoryUnreadable { path: PathBuf, detail: String },
    MissingControlFiles(Vec<String>),
    Write { path: PathBuf, detail: String },
    Readback { path: PathBuf, detail: String },
}

impl fmt::Display for InceptionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RepositoryUnreadable { path, detail } => write!(
                formatter,
                "INCEPTION_REPOSITORY_UNREADABLE path={} detail={detail}",
                path.display()
            ),
            Self::MissingControlFiles(paths) => write!(
                formatter,
                "INCEPTION_CONTROL_FILES_MISSING paths={}",
                paths.join(",")
            ),
            Self::Write { path, detail } => write!(
                formatter,
                "INCEPTION_WRITE_FAILED path={} detail={detail}",
                path.display()
            ),
            Self::Readback { path, detail } => write!(
                formatter,
                "INCEPTION_READBACK_FAILED path={} detail={detail}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for InceptionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InceptionManifest {
    pub schema_version: String,
    pub project_id: String,
    pub repo_identity: RepoIdentity,
    pub control_files: BTreeMap<String, bool>,
    pub host_capabilities: HostCapabilities,
    pub required_tools: Vec<String>,
    pub trust_status: TrustStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InceptionReadback {
    pub repo_identity: RepoIdentity,
    pub control_files_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitReport {
    pub manifest: InceptionManifest,
    pub actions: usize,
    pub backup: Option<PathBuf>,
    pub journal_rows: usize,
    pub monitor_rows: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoIdentity {
    pub canonical_path: String,
    pub git_marker: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCapabilities {
    pub os: String,
    pub arch: String,
    pub filesystem: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustStatus {
    pub status: String,
    pub reason_code: String,
    pub control_files_complete: bool,
}

fn project_id(path: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(path.as_bytes());
    let digest = digest.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(hex, "{byte:02x}").expect("writing to String cannot fail");
    }
    format!("omp-{hex}")[..20].to_owned()
}

fn git_marker(repo_root: &Path) -> Result<String, InceptionError> {
    let marker = repo_root.join(".git");
    if marker.is_dir() {
        return Ok("directory".to_owned());
    }
    if marker.is_file() {
        let contents =
            fs::read_to_string(&marker).map_err(|error| InceptionError::RepositoryUnreadable {
                path: marker.clone(),
                detail: error.to_string(),
            })?;
        return Ok(contents.lines().next().unwrap_or("file").to_owned());
    }
    Ok("missing".to_owned())
}

fn build_manifest(repo_root: &Path) -> Result<InceptionManifest, InceptionError> {
    let canonical =
        repo_root
            .canonicalize()
            .map_err(|error| InceptionError::RepositoryUnreadable {
                path: repo_root.to_owned(),
                detail: error.to_string(),
            })?;
    if !canonical.is_dir() {
        return Err(InceptionError::RepositoryUnreadable {
            path: canonical,
            detail: "repository path is not a directory".to_owned(),
        });
    }

    let control_files: BTreeMap<String, bool> = control_file_presence(&canonical);
    let missing: Vec<String> = control_files
        .iter()
        .filter_map(|(path, present)| (!present).then_some(path.clone()))
        .collect();
    if !missing.is_empty() {
        return Err(InceptionError::MissingControlFiles(missing));
    }

    let canonical_path = canonical.display().to_string();
    Ok(InceptionManifest {
        schema_version: SCHEMA_VERSION.to_owned(),
        project_id: project_id(&canonical_path),
        repo_identity: RepoIdentity {
            canonical_path,
            git_marker: git_marker(&canonical)?,
        },
        control_files,
        host_capabilities: HostCapabilities {
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
            filesystem: "local".to_owned(),
        },
        required_tools: REQUIRED_TOOLS
            .iter()
            .map(|tool| (*tool).to_owned())
            .collect(),
        trust_status: TrustStatus {
            status: "unverified".to_owned(),
            reason_code: "TRUST_DECISION_REQUIRED".to_owned(),
            control_files_complete: true,
        },
    })
}

fn json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                write!(escaped, "\\u{:04x}", character as u32)
                    .expect("writing to String cannot fail");
            }
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}

fn render_manifest(manifest: &InceptionManifest) -> String {
    let mut output = String::from("{\n");
    writeln!(
        output,
        "  \"schema_version\": {},",
        json_string(&manifest.schema_version)
    )
    .expect("writing to String cannot fail");
    writeln!(
        output,
        "  \"project_id\": {},",
        json_string(&manifest.project_id)
    )
    .expect("writing to String cannot fail");
    writeln!(output, "  \"repo_identity\": {{").expect("writing to String cannot fail");
    writeln!(
        output,
        "    \"canonical_path\": {},",
        json_string(&manifest.repo_identity.canonical_path)
    )
    .expect("writing to String cannot fail");
    writeln!(
        output,
        "    \"git_marker\": {}",
        json_string(&manifest.repo_identity.git_marker)
    )
    .expect("writing to String cannot fail");
    writeln!(output, "  }},").expect("writing to String cannot fail");

    writeln!(output, "  \"control_files\": {{").expect("writing to String cannot fail");
    for (index, (path, present)) in manifest.control_files.iter().enumerate() {
        let comma = if index + 1 == manifest.control_files.len() {
            ""
        } else {
            ","
        };
        writeln!(output, "    {}: {present}{comma}", json_string(path))
            .expect("writing to String cannot fail");
    }
    writeln!(output, "  }},").expect("writing to String cannot fail");

    writeln!(output, "  \"host_capabilities\": {{").expect("writing to String cannot fail");
    writeln!(
        output,
        "    \"os\": {},",
        json_string(&manifest.host_capabilities.os)
    )
    .expect("writing to String cannot fail");
    writeln!(
        output,
        "    \"arch\": {},",
        json_string(&manifest.host_capabilities.arch)
    )
    .expect("writing to String cannot fail");
    writeln!(
        output,
        "    \"filesystem\": {}",
        json_string(&manifest.host_capabilities.filesystem)
    )
    .expect("writing to String cannot fail");
    writeln!(output, "  }},").expect("writing to String cannot fail");

    writeln!(output, "  \"required_tools\": [").expect("writing to String cannot fail");
    for (index, tool) in manifest.required_tools.iter().enumerate() {
        let comma = if index + 1 == manifest.required_tools.len() {
            ""
        } else {
            ","
        };
        writeln!(output, "    {}{comma}", json_string(tool))
            .expect("writing to String cannot fail");
    }
    writeln!(output, "  ],").expect("writing to String cannot fail");

    writeln!(output, "  \"trust_status\": {{").expect("writing to String cannot fail");
    writeln!(
        output,
        "    \"status\": {},",
        json_string(&manifest.trust_status.status)
    )
    .expect("writing to String cannot fail");
    writeln!(
        output,
        "    \"reason_code\": {},",
        json_string(&manifest.trust_status.reason_code)
    )
    .expect("writing to String cannot fail");
    writeln!(
        output,
        "    \"control_files_complete\": {}",
        manifest.trust_status.control_files_complete
    )
    .expect("writing to String cannot fail");
    writeln!(output, "  }}").expect("writing to String cannot fail");
    output.push_str("}\n");
    output
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(hex, "{byte:02x}").expect("writing to String cannot fail");
    }
    hex
}

fn snapshot_existing(path: &Path) -> Result<Option<PathBuf>, InceptionError> {
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|error| InceptionError::Write {
        path: path.to_owned(),
        detail: format!("backup read failed: {error}"),
    })?;
    let parent = path.parent().ok_or_else(|| InceptionError::Write {
        path: path.to_owned(),
        detail: "output has no parent directory".to_owned(),
    })?;
    let backup_dir = parent.join("backups");
    fs::create_dir_all(&backup_dir).map_err(|error| InceptionError::Write {
        path: backup_dir.clone(),
        detail: format!("backup directory failed: {error}"),
    })?;
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("inception.json");
    let backup = backup_dir.join(format!("{filename}.{}.bak", sha256_hex(&bytes)));
    if backup.exists() {
        let existing = fs::read(&backup).map_err(|error| InceptionError::Write {
            path: backup.clone(),
            detail: format!("backup verification failed: {error}"),
        })?;
        if existing != bytes {
            return Err(InceptionError::Write {
                path: backup,
                detail: "existing backup content differs from target".to_owned(),
            });
        }
        return Ok(Some(backup));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&backup)
        .map_err(|error| InceptionError::Write {
            path: backup.clone(),
            detail: format!("backup create failed: {error}"),
        })?;
    file.write_all(&bytes).map_err(|error| InceptionError::Write {
        path: backup.clone(),
        detail: format!("backup write failed: {error}"),
    })?;
    file.sync_all().map_err(|error| InceptionError::Write {
        path: backup.clone(),
        detail: format!("backup fsync failed: {error}"),
    })?;
    drop(file);
    File::open(&backup_dir)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| InceptionError::Write {
            path: backup_dir,
            detail: format!("backup parent fsync failed: {error}"),
        })?;
    Ok(Some(backup))
}

fn temporary_path(path: &Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("inception.json");
    path.with_file_name(format!(".{filename}.{}.{}.tmp", std::process::id(), stamp))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), InceptionError> {
    let parent = path.parent().ok_or_else(|| InceptionError::Write {
        path: path.to_owned(),
        detail: "output has no parent directory".to_owned(),
    })?;
    fs::create_dir_all(parent).map_err(|error| InceptionError::Write {
        path: parent.to_owned(),
        detail: error.to_string(),
    })?;

    let temporary = temporary_path(path);
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| InceptionError::Write {
                path: temporary.clone(),
                detail: error.to_string(),
            })?;
        file.write_all(bytes)
            .map_err(|error| InceptionError::Write {
                path: temporary.clone(),
                detail: error.to_string(),
            })?;
        file.sync_all().map_err(|error| InceptionError::Write {
            path: temporary.clone(),
            detail: error.to_string(),
        })?;
        drop(file);
        fs::rename(&temporary, path).map_err(|error| InceptionError::Write {
            path: path.to_owned(),
            detail: error.to_string(),
        })?;
        #[cfg(unix)]
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| InceptionError::Write {
                path: parent.to_owned(),
                detail: format!("parent fsync failed: {error}"),
            })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn validate_readback(contents: &str) -> Result<InceptionReadback, String> {
    let value: Value = serde_json::from_str(contents)
        .map_err(|error| format!("invalid JSON: {error}"))?;
    let missing: Vec<&str> = REQUIRED_KEYS
        .iter()
        .copied()
        .filter(|key| value.get(*key).is_none())
        .collect();
    if !missing.is_empty() {
        return Err(format!("missing required keys: {}", missing.join(",")));
    }
    if value.get("schema_version").and_then(Value::as_str) != Some(SCHEMA_VERSION) {
        return Err("schema_version does not match inception.v1".to_owned());
    }
    let repo_identity = value
        .get("repo_identity")
        .and_then(Value::as_object)
        .ok_or_else(|| "repo_identity is not an object".to_owned())?;
    let canonical_path = repo_identity
        .get("canonical_path")
        .and_then(Value::as_str)
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| "repo_identity.canonical_path is missing".to_owned())?;
    let git_marker = repo_identity
        .get("git_marker")
        .and_then(Value::as_str)
        .filter(|marker| !marker.trim().is_empty())
        .ok_or_else(|| "repo_identity.git_marker is missing".to_owned())?;
    let control_files = value
        .get("control_files")
        .and_then(Value::as_object)
        .ok_or_else(|| "control_files is not an object".to_owned())?;
    let control_files_complete = REQUIRED_CONTROL_FILES.iter().all(|path| {
        control_files
            .get(*path)
            .and_then(Value::as_bool)
            == Some(true)
    });
    if !control_files_complete {
        return Err("control_files is incomplete".to_owned());
    }
    let tools = value
        .get("required_tools")
        .and_then(Value::as_array)
        .ok_or_else(|| "required_tools is not an array".to_owned())?;
    if REQUIRED_TOOLS.iter().any(|tool| {
        !tools.iter().any(|value| value.as_str() == Some(tool))
    }) {
        return Err("required_tools is incomplete".to_owned());
    }
    Ok(InceptionReadback {
        repo_identity: RepoIdentity {
            canonical_path: canonical_path.to_owned(),
            git_marker: git_marker.to_owned(),
        },
        control_files_complete,
    })
}

pub fn read_inception(output: &Path) -> Result<InceptionReadback, InceptionError> {
    let contents = fs::read_to_string(output).map_err(|error| InceptionError::Readback {
        path: output.to_owned(),
        detail: error.to_string(),
    })?;
    validate_readback(&contents).map_err(|detail| InceptionError::Readback {
        path: output.to_owned(),
        detail,
    })
}


fn emit_init_event(repo_root: &Path) -> Result<usize, InceptionError> {
    let journal_path = default_repo_journal(repo_root);
    let journal = DurableJournal::open(&journal_path).map_err(|error| InceptionError::Readback {
        path: journal_path.clone(),
        detail: format!("lifecycle journal open failed: {error}"),
    })?;
    let reason = ReasonCode::new("INIT_REPROBE_OK").map_err(|error| InceptionError::Readback {
        path: journal_path.clone(),
        detail: error.to_string(),
    })?;
    let event = LifecycleEvent::new(
        Layer::L2,
        "S1.L1",
        "S1.L2",
        "ompo-init",
        EmitOutcome::Emitted,
        reason,
    );
    let readback = lifecycle_event::emit_one_host(&journal, event).map_err(|error| {
        InceptionError::Readback {
            path: journal_path,
            detail: format!("lifecycle event emit failed: {error}"),
        }
    })?;
    Ok(readback.lines)
}

pub fn initialize(repo_root: &Path, output: &Path) -> Result<InitReport, InceptionError> {
    let manifest = build_manifest(repo_root)?;
    let bytes = render_manifest(&manifest).into_bytes();
    let (actions, backup) = match fs::read(output) {
        Ok(existing) if existing == bytes => (0, None),
        Ok(_) => (1, snapshot_existing(output)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (1, None),
        Err(error) => {
            return Err(InceptionError::Readback {
                path: output.to_owned(),
                detail: format!("pre-state read failed: {error}"),
            });
        }
    };
    if actions == 1 {
        write_atomic(output, &bytes)?;
    }
    let readback = read_inception(output)?;
    if readback.repo_identity != manifest.repo_identity {
        return Err(InceptionError::Readback {
            path: output.to_owned(),
            detail: format!(
                "repo_identity changed during initialization: expected={} found={}",
                manifest.repo_identity.canonical_path, readback.repo_identity.canonical_path
            ),
        });
    }
    let journal_path = default_repo_journal(repo_root);
    let journal_rows = emit_init_event(repo_root)?;
    let monitor_rows = verify_artifact(&journal_path).map_err(|error| InceptionError::Readback {
        path: journal_path,
        detail: format!("monitor reread failed: {error}"),
    })?;
    Ok(InitReport {
        manifest,
        actions,
        backup,
        journal_rows,
        monitor_rows,
    })
}

pub fn write_inception(
    repo_root: &Path,
    output: &Path,
) -> Result<InceptionManifest, InceptionError> {
    let manifest = build_manifest(repo_root)?;
    let bytes = render_manifest(&manifest).into_bytes();
    let _backup = snapshot_existing(output)?;
    write_atomic(output, &bytes)?;
    let readback = read_inception(output)?;
    if readback.repo_identity != manifest.repo_identity {
        return Err(InceptionError::Readback {
            path: output.to_owned(),
            detail: format!(
                "repo_identity changed during readback: expected={} found={}",
                manifest.repo_identity.canonical_path, readback.repo_identity.canonical_path
            ),
        });
    }
    emit_init_event(repo_root)?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fixture() -> (TempDir, PathBuf) {
        let directory = tempfile::tempdir().expect("fixture directory");
        for relative in REQUIRED_CONTROL_FILES {
            let path = directory.path().join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("fixture parent");
            }
            fs::write(path, "fixture\n").expect("fixture file");
        }
        let output = directory.path().join(".omp-orchestrator/inception.json");
        (directory, output)
    }

    #[test]
    fn writes_and_reads_all_required_fields() {
        let (directory, output) = fixture();
        let manifest = write_inception(directory.path(), &output).expect("write inception");
        let contents = fs::read_to_string(&output).expect("read inception");
        validate_readback(&contents).expect("required fields");
        for key in REQUIRED_KEYS {
            assert!(
                contents.contains(&format!("\"{key}\":")),
                "required key {key}"
            );
        }
        assert_eq!(manifest.schema_version, SCHEMA_VERSION);
        let journal = fs::read_to_string(default_repo_journal(directory.path()))
            .expect("lifecycle journal");
        assert!(journal.contains("\"layer\":\"L2\""));
        assert!(journal.contains("\"reason_code\":\"INIT_REPROBE_OK\""));
        assert!(journal.contains("\"stage_to\":\"S1.L2\""));
        assert_eq!(manifest.required_tools.len(), REQUIRED_TOOLS.len());
    }

    #[test]
    fn readback_returns_identity_and_preserves_prior_artifact() {
        let (directory, output) = fixture();
        let first = write_inception(directory.path(), &output).expect("first write");
        let before = fs::read(&output).expect("first artifact");
        let second = write_inception(directory.path(), &output).expect("second write");
        let readback = read_inception(&output).expect("readback");
        assert_eq!(readback.repo_identity, second.repo_identity);
        assert!(readback.control_files_complete);
        assert_eq!(first.repo_identity, second.repo_identity);
        let backup_dir = output.parent().unwrap().join("backups");
        let backups: Vec<_> = fs::read_dir(&backup_dir)
            .expect("backup directory")
            .map(|entry| entry.expect("backup entry").path())
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(fs::read(&backups[0]).expect("backup artifact"), before);
    }

    #[test]
    fn initialize_reprobes_and_second_run_has_zero_artifact_actions() {
        let (directory, output) = fixture();
        let first = initialize(directory.path(), &output).expect("first init");
        let second = initialize(directory.path(), &output).expect("second init");
        assert_eq!(first.actions, 1);
        assert_eq!(second.actions, 0);
        assert_eq!(second.backup, None);
        assert_eq!(second.monitor_rows, first.monitor_rows + 1);
        assert_eq!(second.manifest.repo_identity, first.manifest.repo_identity);
        assert_eq!(second.monitor_rows, second.journal_rows);
    }

    #[test]
    fn missing_control_file_refuses_before_write() {
        let (directory, output) = fixture();
        fs::remove_file(directory.path().join("AGENTS.md")).expect("remove control file");
        let error = write_inception(directory.path(), &output).expect_err("must refuse");
        assert!(error
            .to_string()
            .contains("INCEPTION_CONTROL_FILES_MISSING"));
        assert!(!output.exists(), "refusal must not create the artifact");
    }
}

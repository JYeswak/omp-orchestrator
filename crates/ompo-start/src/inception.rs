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
use serde_json::{Map, Value};
use lifecycle_monitor::verify_artifact;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};
use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
const IDENTITY_COMMAND_DEADLINE: Duration = Duration::from_secs(10);
const REQUIRED_KEYS: &[&str] = &[
    "schema_version",
    "project_id",
    "repo_identity",
    "control_files",
    "host_capabilities",
    "required_tools",
    "trust_status",
];
const OPTIONAL_KEYS: &[&str] = &["evidence", "status", "degradations"];

#[derive(Debug)]
pub enum InceptionError {
    RepositoryUnreadable { path: PathBuf, detail: String },
    MissingControlFiles(Vec<String>),
    Write { path: PathBuf, detail: String },
    Readback { path: PathBuf, detail: String },
    IdentityUnavailable { field: &'static str, detail: String },
    /// Init refused over an AGENTS.md that carries no repo ownership stamp.
    /// Explicit opt-in (`trusted_init`) is the only override.
    UntrustedAgentsMd { path: PathBuf },
    /// AGENTS.md exists but is empty: a broken fixture, not a foreign repo.
    EmptyAgentsMd { path: PathBuf },
    /// The artifact is absent: anti-vacuity is a typed refusal, never a pass.
    ReadbackMissing { path: PathBuf },
    /// The artifact exists but is not parseable JSON.
    ReadbackMalformed { path: PathBuf, detail: String },
    /// A required or nested required field is absent.
    ReadbackMissingKey { path: PathBuf, key: String },
    /// The artifact contains evidence outside the current schema contract.
    ReadbackExtraKey { path: PathBuf, key: String },
    /// A field has a JSON type other than the contract declares.
    ReadbackWrongType {
        path: PathBuf,
        key: String,
        expected: &'static str,
        found: &'static str,
    },
    /// A required string or array item is present but empty.
    ReadbackEmpty { path: PathBuf, key: String },
    /// A typed field is present but violates its value contract.
    ReadbackInvalid {
        path: PathBuf,
        key: String,
        detail: String,
    },
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
            Self::IdentityUnavailable { field, detail } => write!(
                formatter,
                "INCEPTION_IDENTITY_UNAVAILABLE field={field} detail={detail}"
            ),
            Self::UntrustedAgentsMd { path } => write!(
                formatter,
                "HUMAN_HALT refusing init over unstamped foreign AGENTS.md path={} (pass trusted_init=true to opt in)",
                path.display()
            ),
            Self::EmptyAgentsMd { path } => write!(
                formatter,
                "INCEPTION_EMPTY_AGENTS_MD path={} — an empty AGENTS.md is a broken fixture, never a foreign repo",
                path.display()
            ),
            Self::ReadbackMissing { path } => {
                write!(formatter, "INCEPTION_READBACK_MISSING path={}", path.display())
            }
            Self::ReadbackMalformed { path, detail } => write!(
                formatter,
                "INCEPTION_READBACK_MALFORMED path={} detail={detail}",
                path.display()
            ),
            Self::ReadbackMissingKey { path, key } => write!(
                formatter,
                "INCEPTION_READBACK_MISSING_KEY path={} key={key}",
                path.display()
            ),
            Self::ReadbackExtraKey { path, key } => write!(
                formatter,
                "INCEPTION_READBACK_EXTRA_KEY path={} key={key}",
                path.display()
            ),
            Self::ReadbackWrongType { path, key, expected, found } => write!(
                formatter,
                "INCEPTION_READBACK_WRONG_TYPE path={} key={key} expected={expected} found={found}",
                path.display()
            ),
            Self::ReadbackEmpty { path, key } => write!(
                formatter,
                "INCEPTION_READBACK_EMPTY path={} key={key}",
                path.display()
            ),
            Self::ReadbackInvalid { path, key, detail } => write!(
                formatter,
                "INCEPTION_READBACK_INVALID path={} key={key} detail={detail}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for InceptionError {}

impl InceptionError {
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::ReadbackMissing { .. } | Self::ReadbackMalformed { .. } => 2,
            Self::ReadbackMissingKey { .. }
            | Self::ReadbackExtraKey { .. }
            | Self::ReadbackWrongType { .. }
            | Self::ReadbackEmpty { .. }
            | Self::ReadbackInvalid { .. } => 3,
            _ => 2,
        }
    }
}

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
    pub project_id: String,
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
    /// Files whose CONTENT this run actually replaced or created. Equal to
    /// `actions` today, published separately because the 1:1 backup law is
    /// stated over mutations, not over "something happened".
    pub files_mutated: usize,
    /// Snapshots this run wrote. The law is NOT a flat 1:1 against
    /// `files_mutated`: a virgin repo mutates one file and has nothing to
    /// snapshot, so it is legitimately 0:1. The 1:1 obligation binds only when
    /// PRE-EXISTING CONTENT was superseded.
    pub backups_written: usize,
    /// Whether the artifact already existed before this run. This is what makes
    /// 0:1 and 1:1 distinguishable instead of one ratio with two meanings.
    pub preexisting: bool,
    /// The 1:1 verdict, always emitted: never inferred from the ratio by a
    /// reader who cannot see `preexisting`.
    pub backup_ratio_verdict: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoIdentity {
    pub canonical_path: String,
    pub git_marker: String,
    pub source_revision: String,
    pub host_identity: String,
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
fn identity_field(field: &'static str, value: &str) -> Result<String, InceptionError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(InceptionError::IdentityUnavailable {
            field,
            detail: "empty value".to_owned(),
        });
    }
    Ok(value.to_owned())
}

fn run_identity_command(
    command: &mut Command,
    field: &'static str,
) -> Result<String, InceptionError> {
    match subprocess_contract::bounded_output(command, IDENTITY_COMMAND_DEADLINE) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => {
            identity_field(field, &String::from_utf8_lossy(&output.stdout))
        }
        subprocess_contract::BoundedOutcome::Completed(output) => {
            Err(InceptionError::IdentityUnavailable {
                field,
                detail: format!(
                    "command exited {:?}: {}",
                    output.status.code(),
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            })
        }
        subprocess_contract::BoundedOutcome::TimedOut => Err(InceptionError::IdentityUnavailable {
            field,
            detail: format!("command exceeded {}s", IDENTITY_COMMAND_DEADLINE.as_secs()),
        }),
        subprocess_contract::BoundedOutcome::Unspawned(error) => {
            Err(InceptionError::IdentityUnavailable {
                field,
                detail: format!("command could not start: {error}"),
            })
        }
    }
}

fn source_revision(repo_root: &Path) -> Result<String, InceptionError> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo_root).args(["rev-parse", "HEAD"]);
    run_identity_command(&mut command, "source_revision")
}

fn host_identity() -> Result<String, InceptionError> {
    let mut command = Command::new("hostname");
    run_identity_command(&mut command, "host_identity")
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
    let source_revision = source_revision(&canonical)?;
    let host_identity = host_identity()?;
    let project_id = identity_field("project_id", &project_id(&canonical_path))?;
    Ok(InceptionManifest {
        schema_version: SCHEMA_VERSION.to_owned(),
        project_id,
        repo_identity: RepoIdentity {
            canonical_path,
            git_marker: git_marker(&canonical)?,
            source_revision,
            host_identity,
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

fn render_repo_identity(output: &mut String, identity: &RepoIdentity) {
    writeln!(output, "  \"repo_identity\": {{").expect("writing to String cannot fail");
    writeln!(output, "    \"canonical_path\": {},", json_string(&identity.canonical_path))
        .expect("writing to String cannot fail");
    writeln!(output, "    \"git_marker\": {},", json_string(&identity.git_marker))
        .expect("writing to String cannot fail");
    writeln!(output, "    \"source_revision\": {},", json_string(&identity.source_revision))
        .expect("writing to String cannot fail");
    writeln!(output, "    \"host_identity\": {}", json_string(&identity.host_identity))
        .expect("writing to String cannot fail");
    writeln!(output, "  }},").expect("writing to String cannot fail");
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
    render_repo_identity(&mut output, &manifest.repo_identity);

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

/// One content-keyed backup of the inception artifact.
///
/// The name carries the SHA-256 of the CONTENT, not a timestamp
/// (`snapshot_existing` builds `{filename}.{sha256}.bak`). That is deliberate — it makes a
/// duplicate backup a no-op instead of an accumulating pile — and it has a consequence the
/// restore path must respect: **content-keyed backups have no order.** There is no
/// "latest" to resolve, so a restore that picks one when several exist would be guessing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupEntry {
    pub path: PathBuf,
    /// SHA-256 of the backup's bytes, as recorded in its filename.
    pub content_sha: String,
}

/// What a restore did, or would have done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreReport {
    /// `0` when the artifact already matched the backup byte for byte.
    pub actions: usize,
    pub restored_from: Option<PathBuf>,
    pub content_sha: String,
}

/// Every backup of `output`, sorted by content hash for determinism.
///
/// # Errors
///
/// Fails only if the backup directory exists and cannot be read. A MISSING directory is
/// an empty list, not an error: never initialised and nothing-to-restore are the same
/// observable state here, and the caller is the one positioned to type that refusal.
pub fn list_backups(output: &Path) -> Result<Vec<BackupEntry>, InceptionError> {
    let Some(parent) = output.parent() else {
        return Ok(Vec::new());
    };
    let backup_dir = parent.join("backups");
    if !backup_dir.is_dir() {
        return Ok(Vec::new());
    }
    let filename = output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("inception.json");
    let prefix = format!("{filename}.");
    let entries = fs::read_dir(&backup_dir).map_err(|error| InceptionError::Write {
        path: backup_dir.clone(),
        detail: format!("backup listing failed: {error}"),
    })?;
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| InceptionError::Write {
            path: backup_dir.clone(),
            detail: format!("backup entry unreadable: {error}"),
        })?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(rest) = name.strip_prefix(&prefix) else {
            continue;
        };
        let Some(sha) = rest.strip_suffix(".bak") else {
            continue;
        };
        // Own the sha BEFORE moving `path`: `sha` borrows through `name`, which borrows
        // `path`, so constructing the struct with `path` first is E0505. Same shape as the
        // liveness.rs:86 error diagnosed for %7 tonight -- bind the derived value, then move.
        let content_sha = sha.to_owned();
        found.push(BackupEntry { path, content_sha });
    }
    found.sort_by(|left, right| left.content_sha.cmp(&right.content_sha));
    Ok(found)
}

/// Restore `output` from `entry`, through the SAME atomic write the writer uses.
///
/// Idempotent: when the artifact already matches the backup, `actions` is `0` and nothing
/// is written. That mirrors the writer's own property rather than re-implementing it.
///
/// # Errors
///
/// Refuses a backup whose bytes do not hash to the SHA in its own filename. A corrupted
/// backup restored silently would be worse than no restore at all — the operator would
/// believe the artifact had been recovered.
pub fn restore_backup(
    output: &Path,
    entry: &BackupEntry,
) -> Result<RestoreReport, InceptionError> {
    let bytes = fs::read(&entry.path).map_err(|error| InceptionError::Write {
        path: entry.path.clone(),
        detail: format!("backup read failed: {error}"),
    })?;
    let actual = sha256_hex(&bytes);
    if actual != entry.content_sha {
        return Err(InceptionError::Write {
            path: entry.path.clone(),
            detail: format!(
                "backup integrity failed: filename claims {} but content hashes {actual}",
                entry.content_sha
            ),
        });
    }
    if fs::read(output).is_ok_and(|current| current == bytes) {
        return Ok(RestoreReport {
            actions: 0,
            restored_from: None,
            content_sha: actual,
        });
    }
    write_atomic(output, &bytes)?;
    Ok(RestoreReport {
        actions: 1,
        restored_from: Some(entry.path.clone()),
        content_sha: actual,
    })
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

/// One durability effect of an atomic replace, in the order it completed.
///
/// THE ORDER IS THE LAW. The staging file's bytes must be on stable storage
/// BEFORE the rename publishes them (q7jz: renaming an unsynced file is a torn
/// write), and the parent directory entry must be fsynced AFTER it (u7qq: the
/// entry is not durable until the parent is synced).
///
/// Neither order was observable. No failure can be injected BETWEEN the write
/// and the rename through the public API -- that is a real property of the
/// seam, not a gap in the tests -- and the remote lane runs as ROOT, so every
/// permission-based injection silently succeeds there. So the seam is MADE
/// here instead of worked around: each value is CONSTRUCTED BY THE SYSCALL
/// THAT PERFORMED IT and returned only on its success. A recorded effect
/// therefore cannot exist without its syscall having happened, and deleting
/// the fsync deletes its record with it. The vector is not a log written
/// beside the work; it is the work's return value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtomicWriteEffect {
    /// The staging file was created exclusively, beside the destination.
    StageCreated(PathBuf),
    /// This many bytes reached the staging fd.
    BytesWritten(usize),
    /// `fsync(2)` on the STAGING FILE's fd returned success.
    FileFsynced(PathBuf),
    /// `rename(2)` published the staging file over the destination.
    Renamed { from: PathBuf, to: PathBuf },
    /// `fsync(2)` on the PARENT DIRECTORY's fd returned success.
    ParentFsynced(PathBuf),
}

fn stage_create(temporary: &Path) -> Result<(File, AtomicWriteEffect), InceptionError> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)
        .map_err(|error| InceptionError::Write {
            path: temporary.to_owned(),
            detail: error.to_string(),
        })?;
    Ok((file, AtomicWriteEffect::StageCreated(temporary.to_owned())))
}

fn stage_write(
    file: &mut File,
    temporary: &Path,
    bytes: &[u8],
) -> Result<AtomicWriteEffect, InceptionError> {
    file.write_all(bytes)
        .map_err(|error| InceptionError::Write {
            path: temporary.to_owned(),
            detail: error.to_string(),
        })?;
    Ok(AtomicWriteEffect::BytesWritten(bytes.len()))
}

/// q7jz. The effect is the fsync's own success value, so it cannot be reported
/// without the fsync having returned Ok.
fn stage_fsync(file: &File, temporary: &Path) -> Result<AtomicWriteEffect, InceptionError> {
    file.sync_all().map_err(|error| InceptionError::Write {
        path: temporary.to_owned(),
        detail: error.to_string(),
    })?;
    Ok(AtomicWriteEffect::FileFsynced(temporary.to_owned()))
}

fn publish_rename(temporary: &Path, path: &Path) -> Result<AtomicWriteEffect, InceptionError> {
    fs::rename(temporary, path).map_err(|error| InceptionError::Write {
        path: path.to_owned(),
        detail: error.to_string(),
    })?;
    Ok(AtomicWriteEffect::Renamed {
        from: temporary.to_owned(),
        to: path.to_owned(),
    })
}

/// u7qq. Cites beads_rust sync/mod.rs:507-509: the directory entry is not
/// durable until the parent is fsynced, so this runs AFTER the rename, never
/// before it.
#[cfg(unix)]
fn parent_fsync(parent: &Path) -> Result<AtomicWriteEffect, InceptionError> {
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| InceptionError::Write {
            path: parent.to_owned(),
            detail: format!("parent fsync failed: {error}"),
        })?;
    Ok(AtomicWriteEffect::ParentFsynced(parent.to_owned()))
}

/// Atomic replace, returning the ordered durability effects it performed.
///
/// This is the ONLY write path -- [`write_atomic`] is a thin discard of the
/// return value -- so an observer of the vector is observing production, not a
/// parallel test implementation.
///
/// # Errors
/// [`InceptionError::Write`] naming the path whose syscall failed. A failure
/// at any step removes the staging file rather than abandoning it.
pub fn write_atomic_observed(
    path: &Path,
    bytes: &[u8],
) -> Result<Vec<AtomicWriteEffect>, InceptionError> {
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
        let mut effects = Vec::with_capacity(5);
        let (mut file, created) = stage_create(&temporary)?;
        effects.push(created);
        effects.push(stage_write(&mut file, &temporary, bytes)?);
        effects.push(stage_fsync(&file, &temporary)?);
        drop(file);
        effects.push(publish_rename(&temporary, path)?);
        #[cfg(unix)]
        effects.push(parent_fsync(parent)?);
        Ok(effects)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), InceptionError> {
    write_atomic_observed(path, bytes).map(|_| ())
}

#[derive(Debug)]
enum ReadbackValidationError {
    Malformed(String),
    MissingKey(String),
    ExtraKey(String),
    WrongType {
        key: String,
        expected: &'static str,
        found: &'static str,
    },
    Empty(String),
    Invalid { key: String, detail: String },
}

fn value_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn reject_extra_keys(
    object: &Map<String, Value>,
    allowed: &[&str],
    prefix: &str,
) -> Result<(), ReadbackValidationError> {
    let mut extras: Vec<String> = object
        .keys()
        .filter(|key| !allowed.iter().any(|allowed| *allowed == key.as_str()))
        .map(|key| format!("{prefix}{key}"))
        .collect();
    extras.sort();
    extras
        .into_iter()
        .next()
        .map_or(Ok(()), |key| Err(ReadbackValidationError::ExtraKey(key)))
}

fn required_value<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    key: &str,
) -> Result<&'a Value, ReadbackValidationError> {
    object
        .get(field)
        .ok_or_else(|| ReadbackValidationError::MissingKey(key.to_owned()))
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    key: &str,
) -> Result<&'a str, ReadbackValidationError> {
    let value = required_value(object, field, key)?;
    match value {
        Value::String(text) if !text.trim().is_empty() => Ok(text),
        Value::String(_) => Err(ReadbackValidationError::Empty(key.to_owned())),
        other => Err(ReadbackValidationError::WrongType {
            key: key.to_owned(),
            expected: "string",
            found: value_type(other),
        }),
    }
}

fn required_object<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    key: &str,
) -> Result<&'a Map<String, Value>, ReadbackValidationError> {
    let value = required_value(object, field, key)?;
    value
        .as_object()
        .ok_or_else(|| ReadbackValidationError::WrongType {
            key: key.to_owned(),
            expected: "object",
            found: value_type(value),
        })
}

fn required_bool(
    object: &Map<String, Value>,
    field: &str,
    key: &str,
) -> Result<bool, ReadbackValidationError> {
    let value = required_value(object, field, key)?;
    value
        .as_bool()
        .ok_or_else(|| ReadbackValidationError::WrongType {
            key: key.to_owned(),
            expected: "boolean",
            found: value_type(value),
        })
}

fn validate_readback(contents: &str) -> Result<InceptionReadback, ReadbackValidationError> {
    let value: Value = serde_json::from_str(contents)
        .map_err(|error| ReadbackValidationError::Malformed(error.to_string()))?;
    let object = value
        .as_object()
        .ok_or_else(|| ReadbackValidationError::WrongType {
            key: "$".to_owned(),
            expected: "object",
            found: value_type(&value),
        })?;
    let allowed: Vec<&str> = REQUIRED_KEYS
        .iter()
        .chain(OPTIONAL_KEYS.iter())
        .copied()
        .collect();
    reject_extra_keys(object, &allowed, "")?;

    let schema_version = required_string(object, "schema_version", "schema_version")?;
    if schema_version != SCHEMA_VERSION {
        return Err(ReadbackValidationError::Invalid {
            key: "schema_version".to_owned(),
            detail: format!("expected={SCHEMA_VERSION} found={schema_version}"),
        });
    }
    let project_id = required_string(object, "project_id", "project_id")?.to_owned();

    let identity = required_object(object, "repo_identity", "repo_identity")?;
    reject_extra_keys(
        identity,
        &["canonical_path", "git_marker", "source_revision", "host_identity"],
        "repo_identity.",
    )?;
    let repo_identity = RepoIdentity {
        canonical_path: required_string(
            identity,
            "canonical_path",
            "repo_identity.canonical_path",
        )?
        .to_owned(),
        git_marker: required_string(identity, "git_marker", "repo_identity.git_marker")?
            .to_owned(),
        source_revision: required_string(
            identity,
            "source_revision",
            "repo_identity.source_revision",
        )?
        .to_owned(),
        host_identity: required_string(identity, "host_identity", "repo_identity.host_identity")?
            .to_owned(),
    };

    let control_files = required_object(object, "control_files", "control_files")?;
    reject_extra_keys(control_files, REQUIRED_CONTROL_FILES, "control_files.")?;
    for relative in REQUIRED_CONTROL_FILES {
        let key = format!("control_files.{relative}");
        if !required_bool(control_files, relative, &key)? {
            return Err(ReadbackValidationError::Invalid {
                key,
                detail: "expected=true".to_owned(),
            });
        }
    }

    let host = required_object(object, "host_capabilities", "host_capabilities")?;
    reject_extra_keys(host, &["os", "arch", "filesystem"], "host_capabilities.")?;
    for field in ["os", "arch", "filesystem"] {
        let key = format!("host_capabilities.{field}");
        required_string(host, field, &key)?;
    }

    let tools_value = required_value(object, "required_tools", "required_tools")?;
    let tools = tools_value
        .as_array()
        .ok_or_else(|| ReadbackValidationError::WrongType {
            key: "required_tools".to_owned(),
            expected: "array",
            found: value_type(tools_value),
        })?;
    let mut seen = BTreeSet::new();
    for (index, value) in tools.iter().enumerate() {
        let key = format!("required_tools[{index}]");
        let tool = match value {
            Value::String(tool) if !tool.trim().is_empty() => tool,
            Value::String(_) => return Err(ReadbackValidationError::Empty(key)),
            other => {
                return Err(ReadbackValidationError::WrongType {
                    key,
                    expected: "string",
                    found: value_type(other),
                });
            }
        };
        if !REQUIRED_TOOLS.contains(&tool.as_str()) || !seen.insert(tool.to_owned()) {
            return Err(ReadbackValidationError::ExtraKey(key));
        }
    }
    for tool in REQUIRED_TOOLS {
        if !seen.contains(*tool) {
            return Err(ReadbackValidationError::MissingKey(format!("required_tools.{tool}")));
        }
    }

    let trust = required_object(object, "trust_status", "trust_status")?;
    reject_extra_keys(
        trust,
        &["status", "reason_code", "control_files_complete"],
        "trust_status.",
    )?;
    for field in ["status", "reason_code"] {
        let key = format!("trust_status.{field}");
        required_string(trust, field, &key)?;
    }
    if !required_bool(trust, "control_files_complete", "trust_status.control_files_complete")? {
        return Err(ReadbackValidationError::Invalid {
            key: "trust_status.control_files_complete".to_owned(),
            detail: "expected=true".to_owned(),
        });
    }

    Ok(InceptionReadback {
        project_id,
        repo_identity,
        control_files_complete: true,
    })
}

/// Read back the declared `required_tools` set from an on-disk inception
/// artifact. Separate from [`read_inception`] because that readback answers
/// "is this repo's identity intact"; this one answers "which tool set did the
/// artifact actually commit to", which L1's doctor report must carry rather
/// than restate from its own constant.
///
/// Refuses a partial set: a tool missing from the file is reported, never
/// silently backfilled from [`REQUIRED_TOOLS`].
pub fn read_required_tools(output: &Path) -> Result<Vec<String>, InceptionError> {
    let contents = match fs::read_to_string(output) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(InceptionError::ReadbackMissing {
                path: output.to_owned(),
            });
        }
        Err(error) => {
            return Err(InceptionError::Readback {
                path: output.to_owned(),
                detail: error.to_string(),
            });
        }
    };
    let value: Value =
        serde_json::from_str(&contents).map_err(|error| InceptionError::ReadbackMalformed {
            path: output.to_owned(),
            detail: error.to_string(),
        })?;
    let array = value
        .get("required_tools")
        .and_then(Value::as_array)
        .ok_or_else(|| InceptionError::ReadbackMissingKey {
            path: output.to_owned(),
            key: "required_tools".to_owned(),
        })?;
    let mut tools = Vec::with_capacity(array.len());
    for (index, entry) in array.iter().enumerate() {
        match entry.as_str() {
            Some(tool) if !tool.trim().is_empty() => tools.push(tool.to_owned()),
            _ => {
                return Err(InceptionError::ReadbackWrongType {
                    path: output.to_owned(),
                    key: format!("required_tools[{index}]"),
                    expected: "string",
                    found: "non-string or empty",
                });
            }
        }
    }
    for tool in REQUIRED_TOOLS {
        if !tools.iter().any(|seen| seen == tool) {
            return Err(InceptionError::ReadbackMissingKey {
                path: output.to_owned(),
                key: format!("required_tools.{tool}"),
            });
        }
    }
    Ok(tools)
}

pub fn read_inception(output: &Path) -> Result<InceptionReadback, InceptionError> {
    let contents = match fs::read_to_string(output) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(InceptionError::ReadbackMissing {
                path: output.to_owned(),
            });
        }
        Err(error) => {
            return Err(InceptionError::Readback {
                path: output.to_owned(),
                detail: error.to_string(),
            });
        }
    };
    validate_readback(&contents).map_err(|error| match error {
        ReadbackValidationError::Malformed(detail) => InceptionError::ReadbackMalformed {
            path: output.to_owned(),
            detail,
        },
        ReadbackValidationError::MissingKey(key) => InceptionError::ReadbackMissingKey {
            path: output.to_owned(),
            key,
        },
        ReadbackValidationError::ExtraKey(key) => InceptionError::ReadbackExtraKey {
            path: output.to_owned(),
            key,
        },
        ReadbackValidationError::WrongType { key, expected, found } => {
            InceptionError::ReadbackWrongType {
                path: output.to_owned(),
                key,
                expected,
                found,
            }
        }
        ReadbackValidationError::Empty(key) => InceptionError::ReadbackEmpty {
            path: output.to_owned(),
            key,
        },
        ReadbackValidationError::Invalid { key, detail } => InceptionError::ReadbackInvalid {
            path: output.to_owned(),
            key,
            detail,
        },
    })
}


fn emit_stage_event(
    repo_root: &Path,
    layer: Layer,
    stage_from: &str,
    stage_to: &str,
    actor: &str,
    reason: &str,
) -> Result<usize, InceptionError> {
    let journal_path = default_repo_journal(repo_root);
    let journal = DurableJournal::open(&journal_path).map_err(|error| InceptionError::Readback {
        path: journal_path.clone(),
        detail: format!("lifecycle journal open failed: {error}"),
    })?;
    let code = ReasonCode::new(reason).map_err(|error| InceptionError::Readback {
        path: journal_path.clone(),
        detail: error.to_string(),
    })?;
    let event = LifecycleEvent::new(layer, stage_from, stage_to, actor, EmitOutcome::Emitted, code);
    let readback = lifecycle_event::emit_one_host(&journal, event).map_err(|error| {
        InceptionError::Readback {
            path: journal_path,
            detail: format!("lifecycle event emit failed: {error}"),
        }
    })?;
    Ok(readback.lines)
}

fn emit_init_event(repo_root: &Path) -> Result<usize, InceptionError> {
    emit_stage_event(repo_root, Layer::L2, "S1.L1", "S1.L2", "ompo-init", "INIT_REPROBE_OK")
}

/// Ownership anchor: an AGENTS.md that does not name this repository is foreign.
/// Heuristic, stated plainly: content cannot prove ownership, so a foreign read
/// refuses loudly and explicit opt-in (`trusted_init`) is the only override.
pub const PROJECT_AGENTS_OWNERSHIP_STAMP: &str = "omp-orchestrator";

/// Trust gate (cbl7): refuse init over an unstamped foreign AGENTS.md unless the
/// caller explicitly opts in. Missing files never reach here (`build_manifest`
/// refuses them first); unreadable files surface as `RepositoryUnreadable`.
fn verify_agents_ownership(repo_root: &Path, trusted_init: bool) -> Result<(), InceptionError> {
    if trusted_init {
        return Ok(());
    }
    let path = repo_root.join("AGENTS.md");
    let text = fs::read_to_string(&path).map_err(|error| InceptionError::RepositoryUnreadable {
        path: path.clone(),
        detail: format!("AGENTS.md unreadable: {error}"),
    })?;
    if text.trim().is_empty() {
        return Err(InceptionError::EmptyAgentsMd { path });
    }
    if !text.contains(PROJECT_AGENTS_OWNERSHIP_STAMP) {
        return Err(InceptionError::UntrustedAgentsMd { path });
    }
    Ok(())
}

pub fn initialize(repo_root: &Path, output: &Path) -> Result<InitReport, InceptionError> {
    initialize_inner(repo_root, output, false)
}

/// Explicit opt-in init over an unstamped foreign AGENTS.md. Identical flow to
/// [`initialize`], minus the ownership refusal.
pub fn initialize_trusted(repo_root: &Path, output: &Path) -> Result<InitReport, InceptionError> {
    initialize_inner(repo_root, output, true)
}

fn initialize_inner(
    repo_root: &Path,
    output: &Path,
    trusted_init: bool,
) -> Result<InitReport, InceptionError> {
    let manifest = build_manifest(repo_root)?;
    let bytes = render_manifest(&manifest).into_bytes();
    verify_agents_ownership(repo_root, trusted_init)?;
    // ONE pre-state read answers both questions: did the artifact exist, and
    // does its content differ. Reading twice would let the two answers come
    // from two different moments.
    let (preexisting, actions, backup) = match fs::read(output) {
        Ok(existing) if existing == bytes => (true, 0, None),
        Ok(_) => (true, 1, snapshot_existing(output)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (false, 1, None),
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
    if readback.project_id != manifest.project_id
        || readback.repo_identity != manifest.repo_identity
    {
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
    // THE 1:1 LAW, stated where it can be enforced rather than left to a
    // reader of two counts. Superseding pre-existing content without a
    // snapshot is unrecoverable, so it is a typed refusal, not a low ratio.
    let files_mutated = actions;
    let backups_written = usize::from(backup.is_some());
    let backup_ratio_verdict = if preexisting && files_mutated > 0 {
        if backups_written == files_mutated {
            "BACKUP_RATIO_OK_1_TO_1".to_owned()
        } else {
            return Err(InceptionError::Readback {
                path: output.to_owned(),
                detail: format!(
                    "BACKUP_RATIO_VIOLATION backups_written={backups_written} files_mutated={files_mutated} preexisting=true"
                ),
            });
        }
    } else if files_mutated > 0 {
        // Virgin write: nothing existed, so there is nothing to snapshot.
        "BACKUP_RATIO_OK_VIRGIN_0_TO_1".to_owned()
    } else {
        "BACKUP_RATIO_OK_NO_MUTATION".to_owned()
    };
    Ok(InitReport {
        manifest,
        actions,
        backup,
        journal_rows,
        monitor_rows,
        files_mutated,
        backups_written,
        preexisting,
        backup_ratio_verdict,
    })
}

pub fn write_inception(repo_root: &Path, output: &Path) -> Result<InceptionManifest, InceptionError> {
    write_inception_inner(repo_root, output, false)
}

/// Explicit opt-in write over an unstamped foreign AGENTS.md. Identical flow to
/// [`write_inception`], minus the ownership refusal.
pub fn write_inception_trusted(
    repo_root: &Path,
    output: &Path,
) -> Result<InceptionManifest, InceptionError> {
    write_inception_inner(repo_root, output, true)
}

fn write_inception_inner(
    repo_root: &Path,
    output: &Path,
    trusted_init: bool,
) -> Result<InceptionManifest, InceptionError> {
    let manifest = build_manifest(repo_root)?;
    verify_agents_ownership(repo_root, trusted_init)?;
    let bytes = render_manifest(&manifest).into_bytes();
    let should_write = match fs::read(output) {
        Ok(existing) if existing == bytes => false,
        Ok(_) => {
            snapshot_existing(output)?;
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(error) => {
            return Err(InceptionError::Readback {
                path: output.to_owned(),
                detail: format!("pre-state read failed: {error}"),
            });
        }
    };
    if should_write {
        write_atomic(output, &bytes)?;
    }
    let readback = read_inception(output)?;
    if readback.project_id != manifest.project_id
        || readback.repo_identity != manifest.repo_identity
    {
        return Err(InceptionError::Readback {
            path: output.to_owned(),
            detail: format!(
                "repo_identity changed during readback: expected={} found={}",
                manifest.repo_identity.canonical_path, readback.repo_identity.canonical_path
            ),
        });
    }
    if should_write {
        emit_init_event(repo_root)?;
        // L5 writer (5iwj): the write+fsync+readback success chokepoint records one
        // S1.L4 -> S1.L5 row. The source stage matches the supervisor-tick L5 row so
        // journal readers see one consistent S1.L4 -> S1.L5 transition.
        emit_stage_event(repo_root, Layer::L5, "S1.L4", "S1.L5", "ompo-init", "INIT_WRITE_OK")?;
    }
    Ok(manifest)
}

#[cfg(test)]
use tempfile::TempDir;

#[cfg(test)]
fn run_test_git(repo: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    command.current_dir(repo).args(args);
    match subprocess_contract::bounded_output(&mut command, Duration::from_secs(10)) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => {}
        other => panic!("git fixture command failed: {other:?}"),
    }
}

#[cfg(test)]
fn fixture() -> (TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("fixture directory");
    for relative in REQUIRED_CONTROL_FILES {
        let path = directory.path().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture parent");
        }
        fs::write(path, "fixture\n").expect("fixture file");
    }
    // The fixture mimics a stamped repo: its AGENTS.md carries the ownership
    // anchor, so untrusted-path legs exercise the gate while every other leg
    // runs against stamped content. Foreign-content legs overwrite this file.
    fs::write(
        directory.path().join("AGENTS.md"),
        format!("fixture {PROJECT_AGENTS_OWNERSHIP_STAMP}\n"),
    )
    .expect("fixture stamp");
    run_test_git(directory.path(), &["init", "-q"]);
    run_test_git(directory.path(), &["add", "."]);
    run_test_git(
        directory.path(),
        &[
            "-c",
            "user.name=ompo-start-test",
            "-c",
            "user.email=ompo-start-test@example.invalid",
            "commit",
            "-qm",
            "fixture",
        ],
    );
    let output = directory.path().join(".omp-orchestrator/inception.json");
    (directory, output)
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(!manifest.project_id.is_empty());
        assert!(!manifest.repo_identity.canonical_path.is_empty());
        assert!(!manifest.repo_identity.source_revision.is_empty());
        assert!(!manifest.repo_identity.host_identity.is_empty());
        let journal = fs::read_to_string(default_repo_journal(directory.path()))
            .expect("lifecycle journal");
        assert!(journal.contains("\"layer\":\"L2\""));
        assert!(journal.contains("\"reason_code\":\"INIT_REPROBE_OK\""));
        assert!(journal.contains("\"stage_to\":\"S1.L2\""));
        assert_eq!(manifest.required_tools.len(), REQUIRED_TOOLS.len());
    }

    /// Typed scan refusal: zero S1.L5 rows is an ERROR with its own reason,
    /// distinct from a content mismatch. Absence of evidence is not a pass.
    fn find_s1_l5_row(journal: &Path) -> Result<serde_json::Value, String> {
        let text = std::fs::read_to_string(journal).map_err(|error| {
            format!(
                "L5_SCAN_UNREADABLE_JOURNAL path={} detail={error}",
                journal.display()
            )
        })?;
        for line in text.lines() {
            let value: serde_json::Value = serde_json::from_str(line)
                .map_err(|error| format!("L5_SCAN_UNPARSEABLE_ROW detail={error}"))?;
            if value.get("stage_to").and_then(|stage| stage.as_str()) == Some("S1.L5") {
                return Ok(value);
            }
        }
        Err(format!(
            "L5_SCAN_ZERO_S1_L5_ROWS journal={} lines={}",
            journal.display(),
            text.lines().count()
        ))
    }

    /// L5 writer leg (5iwj): one `write_inception` call emits one S1.L5 row
    /// beside the S1.L2 init row, through the shared stage-event core.
    #[test]
    fn write_inception_emits_one_s1_l5_row() {
        let (directory, output) = fixture();
        write_inception(directory.path(), &output).expect("write succeeds");
        let row =
            find_s1_l5_row(&default_repo_journal(directory.path())).expect("S1.L5 row exists");
        assert_eq!(row["stage_to"], "S1.L5");
        assert_eq!(row["stage_from"], "S1.L4");
        assert_eq!(row["layer"], "L5");
        assert_eq!(row["reason_code"], "INIT_WRITE_OK");
        assert_eq!(row["actor"], "ompo-init");
        assert_eq!(row["outcome"], "emitted");
    }

    #[test]
    fn readback_returns_identity_and_backups_replaced_artifact() {
        let (directory, output) = fixture();
        let first = write_inception(directory.path(), &output).expect("first write");
        fs::write(&output, "tampered\n").expect("tamper artifact");
        let second = write_inception(directory.path(), &output).expect("replacement write");
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
        assert_eq!(fs::read(&backups[0]).expect("backup artifact"), b"tampered\n");
    }

    #[test]
    fn writer_second_run_is_idempotent_without_events_or_backup() {
        let (directory, output) = fixture();
        write_inception(directory.path(), &output).expect("first write");
        let before = fs::read(&output).expect("first artifact");
        let journal_before =
            fs::read_to_string(default_repo_journal(directory.path())).expect("journal before");
        write_inception(directory.path(), &output).expect("second write");
        assert_eq!(fs::read(&output).expect("second artifact"), before);
        assert_eq!(
            fs::read_to_string(default_repo_journal(directory.path())).expect("journal after"),
            journal_before,
            "unchanged writer emits no lifecycle event"
        );
        assert!(
            !output.parent().unwrap().join("backups").exists(),
            "unchanged writer creates no backup"
        );
    }

    #[test]
    fn readback_refusal_classes_are_typed_and_distinct() {
        for kind in ["missing", "extra", "wrong_type", "empty"] {
            let (directory, output) = fixture();
            write_inception(directory.path(), &output).expect("write fixture");
            let mut value: Value =
                serde_json::from_str(&fs::read_to_string(&output).expect("read fixture"))
                    .expect("fixture JSON");
            let object = value.as_object_mut().expect("manifest object");
            match kind {
                "missing" => {
                    object.remove("schema_version");
                }
                "extra" => {
                    object.insert("unexpected".to_owned(), Value::Bool(true));
                }
                "wrong_type" => {
                    object.insert("project_id".to_owned(), Value::Bool(true));
                }
                "empty" => {
                    object.insert("project_id".to_owned(), Value::String(String::new()));
                }
                _ => unreachable!("case is enumerated"),
            }
            fs::write(&output, serde_json::to_vec_pretty(&value).expect("encode mutation"))
                .expect("write mutation");
            let error = read_inception(&output).expect_err("mutation must refuse");
            match kind {
                "missing" => {
                    assert_eq!(error.exit_code(), 3);
                    assert!(matches!(
                        error,
                        InceptionError::ReadbackMissingKey { key, .. } if key == "schema_version"
                    ));
                }
                "extra" => {
                    assert_eq!(error.exit_code(), 3);
                    assert!(matches!(
                        error,
                        InceptionError::ReadbackExtraKey { key, .. } if key == "unexpected"
                    ));
                }
                "wrong_type" => {
                    assert_eq!(error.exit_code(), 3);
                    assert!(matches!(
                        error,
                        InceptionError::ReadbackWrongType { key, expected, found, .. }
                            if key == "project_id" && expected == "string" && found == "boolean"
                    ));
                }
                "empty" => {
                    assert_eq!(error.exit_code(), 3);
                    assert!(matches!(
                        error,
                        InceptionError::ReadbackEmpty { key, .. } if key == "project_id"
                    ));
                }
                _ => unreachable!("case is enumerated"),
            }
        }

        let (directory, output) = fixture();
        write_inception(directory.path(), &output).expect("write optional fixture");
        let mut value: Value =
            serde_json::from_str(&fs::read_to_string(&output).expect("read optional fixture"))
                .expect("optional fixture JSON");
        let object = value.as_object_mut().expect("manifest object");
        for key in OPTIONAL_KEYS {
            object.insert((*key).to_owned(), Value::String("optional".to_owned()));
        }
        fs::write(&output, serde_json::to_vec_pretty(&value).expect("encode optional fixture"))
            .expect("write optional fixture");
        read_inception(&output).expect("SCHEMAS optional keys remain allowed");
    }

    #[test]
    fn missing_and_malformed_artifacts_are_distinct_anti_vacuity_errors() {
        let (_directory, output) = fixture();
        let missing = read_inception(&output).expect_err("missing artifact must refuse");
        assert_eq!(missing.exit_code(), 2);
        assert!(matches!(missing, InceptionError::ReadbackMissing { .. }));

        // `fixture()` deliberately leaves the artifact DIRECTORY absent so the
        // leg above is a genuine ABSENCE rather than a parse failure. Planting a
        // corrupt artifact therefore has to create that directory first: without
        // it this `fs::write` itself fails NotFound, the malformed leg is never
        // reached, and the distinction the test name asserts goes unmeasured.
        fs::create_dir_all(output.parent().expect("artifact path has a parent"))
            .expect("create artifact directory");
        fs::write(&output, "{").expect("write malformed artifact");
        let malformed = read_inception(&output).expect_err("malformed artifact must refuse");
        assert_eq!(malformed.exit_code(), 2);
        assert!(matches!(malformed, InceptionError::ReadbackMalformed { .. }));
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

    /// Known-bad (cbl7): init over an unstamped foreign AGENTS.md halts with a
    /// typed refusal and writes nothing -- no artifact, no journal rows.
    #[test]
    fn foreign_agents_md_halts_without_writes() {
        let (directory, output) = fixture();
        fs::write(directory.path().join("AGENTS.md"), "foreign template\n")
            .expect("foreign agents file");
        let error = write_inception(directory.path(), &output).expect_err("must halt");
        assert!(
            matches!(error, InceptionError::UntrustedAgentsMd { .. }),
            "wrong refusal: {error:?}"
        );
        assert!(
            error.to_string().starts_with("HUMAN_HALT"),
            "halt must name itself: {error}"
        );
        assert!(!output.exists(), "halt must not write the artifact");
        assert!(
            !default_repo_journal(directory.path()).exists(),
            "halt must not write journal rows"
        );
    }

    /// Opt-in: the trusted variant writes the identical foreign content.
    #[test]
    fn trusted_opt_in_writes_foreign_agents_md() {
        let (directory, output) = fixture();
        fs::write(directory.path().join("AGENTS.md"), "foreign template\n")
            .expect("foreign agents file");
        write_inception_trusted(directory.path(), &output).expect("opt-in writes");
        assert!(output.exists(), "opt-in must produce the artifact");
    }

    /// Opt-in covers the `initialize` entry point too: same foreign content,
    /// trusted flag on, artifact produced.
    #[test]
    fn trusted_initialize_writes_foreign_agents_md() {
        let (directory, output) = fixture();
        fs::write(directory.path().join("AGENTS.md"), "foreign template\n")
            .expect("foreign agents file");
        initialize_trusted(directory.path(), &output).expect("opt-in initializes");
        assert!(output.exists(), "opt-in must produce the artifact");
    }

    /// Anti-vacuity: an empty AGENTS.md is its own typed error, never a halt
    /// for a foreign repo and never a pass.
    #[test]
    fn empty_agents_md_is_typed_error() {
        let (directory, output) = fixture();
        fs::write(directory.path().join("AGENTS.md"), "   \n").expect("empty agents file");
        let error = write_inception(directory.path(), &output).expect_err("must refuse");
        assert!(
            matches!(error, InceptionError::EmptyAgentsMd { .. }),
            "wrong refusal: {error:?}"
        );
        assert!(!output.exists(), "refusal must not write the artifact");
    }

    /// The `initialize` entry point shares the gate: foreign content halts there too.
    #[test]
    fn initialize_over_foreign_agents_md_halts() {
        let (directory, output) = fixture();
        fs::write(directory.path().join("AGENTS.md"), "foreign template\n")
            .expect("foreign agents file");
        let error = initialize(directory.path(), &output).expect_err("must halt");
        assert!(
            matches!(error, InceptionError::UntrustedAgentsMd { .. }),
            "wrong refusal: {error:?}"
        );
        assert!(!output.exists(), "halt must not write the artifact");
    }
}

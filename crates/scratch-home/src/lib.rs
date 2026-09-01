#![forbid(unsafe_code)]

//! One discoverable scratch location per NTM session and pane.
//!
//! Long-lived throwaway work belongs below [`DEFAULT_BASE`], not `/private/tmp` or
//! `$TMPDIR`. A job directory is auto-reapable only when its owner sidecar is
//! present, valid, and agrees with the directory identity. Unknown ownership is
//! reported and never removed.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::{self, DirEntry, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const SCHEMA_VERSION: u32 = 1;
pub const ENV_VAR: &str = "ZS_SCRATCH";
pub const DEFAULT_BASE: &str = ".local/state/zeststream/scratch";
const OWNER_FILE: &str = ".owner.json";

#[derive(Debug)]
pub enum ScratchError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidComponent {
        field: &'static str,
        value: String,
    },
    InvalidAge,
    InvalidOwnerMetadata {
        path: PathBuf,
        reason: String,
    },
}

impl fmt::Display for ScratchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "I/O at {}: {source}", path.display()),
            Self::InvalidComponent { field, value } => {
                write!(f, "invalid {field} component {value:?}")
            }
            Self::InvalidAge => f.write_str("age must be a non-negative integer number of seconds"),
            Self::InvalidOwnerMetadata { path, reason } => {
                write!(f, "invalid owner metadata at {}: {reason}", path.display())
            }
        }
    }
}

impl std::error::Error for ScratchError {}

fn io(path: impl Into<PathBuf>, source: std::io::Error) -> ScratchError {
    ScratchError::Io {
        path: path.into(),
        source,
    }
}

fn component(field: &'static str, value: &str) -> Result<(), ScratchError> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || value
            .bytes()
            .any(|byte| byte == 0 || byte.is_ascii_control())
    {
        return Err(ScratchError::InvalidComponent {
            field,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn reject_symlink(path: &Path) -> Result<(), ScratchError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(io(
            path,
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "symlink is not a scratch root",
            ),
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io(path, error)),
    }
}

fn ensure_dir(path: &Path) -> Result<(), ScratchError> {
    reject_symlink(path)?;
    fs::create_dir_all(path).map_err(|error| io(path, error))?;
    reject_symlink(path)
}

fn absolute(path: PathBuf) -> Result<PathBuf, ScratchError> {
    if path.is_absolute() {
        return Ok(path);
    }
    let cwd = std::env::current_dir().map_err(|error| io(".", error))?;
    Ok(cwd.join(path))
}

/// A resolved, system-wide scratch namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScratchRoot {
    base: PathBuf,
}

impl ScratchRoot {
    pub fn new(base: impl Into<PathBuf>) -> Result<Self, ScratchError> {
        Ok(Self {
            base: absolute(base.into())?,
        })
    }

    /// Resolve `$HOME/.local/state/zeststream/scratch` without guessing another repo.
    pub fn default() -> Result<Self, ScratchError> {
        let home = std::env::var_os("HOME")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ScratchError::Io {
                path: PathBuf::from("$HOME"),
                source: std::io::Error::new(std::io::ErrorKind::NotFound, "HOME is unset"),
            })?;
        Self::new(PathBuf::from(home).join(DEFAULT_BASE))
    }

    pub fn base(&self) -> &Path {
        &self.base
    }

    /// The session directory. This call does not create it.
    pub fn session_path(&self, session: &str) -> Result<PathBuf, ScratchError> {
        component("session", session)?;
        Ok(self.base.join(session))
    }

    pub fn ensure_session(&self, session: &str) -> Result<PathBuf, ScratchError> {
        reject_symlink(&self.base)?;
        let path = self.session_path(session)?;
        ensure_dir(&path)?;
        Ok(path)
    }

    /// The NTM `--pane-env` value used while creating a session.
    ///
    /// NTM expands `{pane}` for each new pane; the session component is fixed
    /// here so labelled sessions cannot accidentally share another project's root.
    pub fn pane_env(&self, session: &str) -> Result<String, ScratchError> {
        let session_path = self.ensure_session(session)?;
        Ok(format!(
            "{ENV_VAR}={}/{pane}",
            session_path.display(),
            pane = "{pane}"
        ))
    }

    /// Arguments to prepend to `ntm spawn <session>` for session-time export.
    pub fn ntm_spawn_args(&self, session: &str) -> Result<Vec<String>, ScratchError> {
        Ok(vec![
            "spawn".to_owned(),
            session.to_owned(),
            "--pane-env".to_owned(),
            self.pane_env(session)?,
        ])
    }

    /// Create a job directory and atomically record the owner that may reap it.
    pub fn create_job(
        &self,
        session: &str,
        pane_or_agent: &str,
        job: &str,
        owner: &str,
    ) -> Result<PathBuf, ScratchError> {
        component("pane_or_agent", pane_or_agent)?;
        component("job", job)?;
        component("owner", owner)?;
        let pane_path = self.ensure_session(session)?.join(pane_or_agent);
        ensure_dir(&pane_path)?;
        let job_path = pane_path.join(job);
        if job_path.exists() {
            reject_symlink(&job_path)?;
            if !job_path.is_dir() {
                return Err(io(
                    &job_path,
                    std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        "job path is not a directory",
                    ),
                ));
            }
        } else {
            fs::create_dir(&job_path).map_err(|error| io(&job_path, error))?;
        }
        write_owner(
            &job_path,
            &OwnerMetadata {
                schema: SCHEMA_VERSION,
                session: session.to_owned(),
                pane_or_agent: pane_or_agent.to_owned(),
                job: job.to_owned(),
                owner: owner.to_owned(),
            },
        )?;
        Ok(job_path)
    }

    /// Inspect old jobs for one session. Unknown ownership is never a candidate.
    pub fn reap(&self, session: &str, min_age: Duration) -> Result<ReapReport, ScratchError> {
        reject_symlink(&self.base)?;
        let session_path = self.session_path(session)?;
        let mut report = ReapReport::default();
        let Ok(session_metadata) = fs::symlink_metadata(&session_path) else {
            return Ok(report);
        };
        if session_metadata.file_type().is_symlink() {
            report.unknown.push(UnknownEntry {
                path: session_path,
                reason: "session root is a symlink".to_owned(),
            });
            return Ok(report);
        }
        if !session_metadata.is_dir() {
            report.unknown.push(UnknownEntry {
                path: session_path,
                reason: "session root is not a directory".to_owned(),
            });
            return Ok(report);
        }
        for pane in read_dirs(&session_path)? {
            let pane_path = pane.path();
            if pane
                .file_type()
                .map_err(|error| io(&pane_path, error))?
                .is_symlink()
            {
                report.unknown.push(UnknownEntry {
                    path: pane_path,
                    reason: "pane root is a symlink".to_owned(),
                });
                continue;
            }
            if !pane
                .file_type()
                .map_err(|error| io(&pane_path, error))?
                .is_dir()
            {
                report.unknown.push(UnknownEntry {
                    path: pane_path,
                    reason: "entry is not a pane directory".to_owned(),
                });
                continue;
            }
            for job_entry in read_dirs(&pane_path)? {
                let job_path = job_entry.path();
                let file_type = job_entry
                    .file_type()
                    .map_err(|error| io(&job_path, error))?;
                if file_type.is_symlink() {
                    report.unknown.push(UnknownEntry {
                        path: job_path,
                        reason: "job is a symlink".to_owned(),
                    });
                    continue;
                }
                if !file_type.is_dir() {
                    report.unknown.push(UnknownEntry {
                        path: job_path,
                        reason: "entry is not a job directory".to_owned(),
                    });
                    continue;
                }
                match inspect_job(
                    &job_path,
                    session,
                    pane_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(""),
                    min_age,
                ) {
                    Ok(Some(candidate)) => report.candidates.push(candidate),
                    Ok(None) => {}
                    Err(reason) => report.unknown.push(UnknownEntry {
                        path: job_path,
                        reason,
                    }),
                }
            }
        }
        Ok(report)
    }

    /// Remove only candidates from a prior [`ReapReport`].
    pub fn apply(&self, report: &ReapReport) -> Result<usize, ScratchError> {
        let mut removed = 0;
        for candidate in &report.candidates {
            // Revalidate the sidecar immediately before mutation. A report is not
            // authority if another process broke attribution after inspection.
            let Some(_) = inspect_job(
                &candidate.path,
                &candidate.session,
                &candidate.pane_or_agent,
                Duration::ZERO,
            )
            .map_err(|reason| ScratchError::InvalidOwnerMetadata {
                path: candidate.path.clone(),
                reason,
            })?
            else {
                continue;
            };
            fs::remove_dir_all(&candidate.path).map_err(|error| io(&candidate.path, error))?;
            removed += 1;
        }
        Ok(removed)
    }
}

fn read_dirs(path: &Path) -> Result<Vec<DirEntry>, ScratchError> {
    let mut entries = Vec::new();
    let iterator = fs::read_dir(path).map_err(|error| io(path, error))?;
    for entry in iterator {
        entries.push(entry.map_err(|error| io(path, error))?);
    }
    entries.sort_by_key(|entry| entry.file_name());
    Ok(entries)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerMetadata {
    pub schema: u32,
    pub session: String,
    pub pane_or_agent: String,
    pub job: String,
    pub owner: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReapCandidate {
    pub path: PathBuf,
    pub session: String,
    pub pane_or_agent: String,
    pub job: String,
    pub owner: String,
    pub age_secs: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnknownEntry {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReapReport {
    pub candidates: Vec<ReapCandidate>,
    pub unknown: Vec<UnknownEntry>,
}

impl ReapReport {
    pub fn auto_reapable(&self) -> bool {
        self.unknown.is_empty()
    }
}

fn inspect_job(
    job_path: &Path,
    expected_session: &str,
    expected_pane: &str,
    min_age: Duration,
) -> Result<Option<ReapCandidate>, String> {
    let metadata_path = job_path.join(OWNER_FILE);
    let metadata = fs::symlink_metadata(&metadata_path)
        .map_err(|error| format!("owner metadata unreadable: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("owner metadata is not a regular file".to_owned());
    }
    let bytes =
        fs::read(&metadata_path).map_err(|error| format!("owner metadata unreadable: {error}"))?;
    let owner: OwnerMetadata = serde_json::from_slice(&bytes)
        .map_err(|error| format!("owner metadata malformed: {error}"))?;
    let job = job_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "job name is not UTF-8".to_owned())?;
    if owner.schema != SCHEMA_VERSION {
        return Err(format!("unsupported owner schema {}", owner.schema));
    }
    if owner.session != expected_session
        || owner.pane_or_agent != expected_pane
        || owner.job != job
        || owner.owner.is_empty()
    {
        return Err("owner metadata does not match path identity".to_owned());
    }
    let job_metadata = fs::symlink_metadata(job_path)
        .map_err(|error| format!("job metadata unreadable: {error}"))?;
    let modified = job_metadata
        .modified()
        .map_err(|error| format!("job age unavailable: {error}"))?;
    let age = SystemTime::now()
        .duration_since(modified)
        .unwrap_or(Duration::ZERO);
    if age < min_age {
        return Ok(None);
    }
    let bytes = directory_bytes(job_path)?;
    Ok(Some(ReapCandidate {
        path: job_path.to_path_buf(),
        session: owner.session,
        pane_or_agent: owner.pane_or_agent,
        job: owner.job,
        owner: owner.owner,
        age_secs: age.as_secs(),
        bytes,
    }))
}

fn directory_bytes(path: &Path) -> Result<u64, String> {
    let mut total = 0u64;
    let entries = fs::read_dir(path).map_err(|error| format!("job scan failed: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("job scan failed: {error}"))?;
        let entry_path = entry.path();
        let metadata = fs::symlink_metadata(&entry_path)
            .map_err(|error| format!("job scan failed: {error}"))?;
        if metadata.file_type().is_symlink() {
            return Err("job contains a symlink".to_owned());
        }
        if metadata.is_dir() {
            total = total.saturating_add(directory_bytes(&entry_path)?);
        } else if metadata.is_file() {
            total = total.saturating_add(metadata.len());
        }
    }
    Ok(total)
}

fn write_owner(job_path: &Path, owner: &OwnerMetadata) -> Result<(), ScratchError> {
    let path = job_path.join(OWNER_FILE);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_nanos();
    let temp = job_path.join(format!("{OWNER_FILE}.{}-{nonce}.tmp", std::process::id()));
    let encoded =
        serde_json::to_vec_pretty(owner).map_err(|error| ScratchError::InvalidOwnerMetadata {
            path: path.clone(),
            reason: error.to_string(),
        })?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)
        .map_err(|error| io(&temp, error))?;
    file.write_all(&encoded).map_err(|error| io(&temp, error))?;
    file.sync_all().map_err(|error| io(&temp, error))?;
    drop(file);
    fs::rename(&temp, &path).map_err(|error| io(&path, error))
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn root() -> (ScratchRoot, PathBuf) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("scratch-home-test-{}-{nonce}", std::process::id()));
        (ScratchRoot::new(&path).unwrap(), path)
    }

    #[test]
    fn creates_one_session_and_emits_ntm_pane_template() {
        let (root, path) = root();
        let session = root.ensure_session("demo").unwrap();
        assert_eq!(session, path.join("demo"));
        assert_eq!(
            root.pane_env("demo").unwrap(),
            format!("ZS_SCRATCH={}/{{pane}}", session.display())
        );
        assert_eq!(
            root.ntm_spawn_args("demo").unwrap()[0..2],
            ["spawn", "demo"]
        );
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn unknown_owner_is_reported_and_never_reaped() {
        let (root, path) = root();
        let job = root
            .ensure_session("demo")
            .unwrap()
            .join("cc_1")
            .join("job");
        fs::create_dir_all(&job).unwrap();
        fs::write(job.join("payload"), b"keep").unwrap();
        let report = root.reap("demo", Duration::ZERO).unwrap();
        assert!(report.candidates.is_empty());
        assert_eq!(report.unknown.len(), 1);
        assert!(job.join("payload").exists());
        assert_eq!(root.apply(&report).unwrap(), 0);
        assert!(job.exists());
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn broken_owner_attribution_refuses_apply() {
        let (root, path) = root();
        let job = root.create_job("demo", "cc_1", "run", "agent-a").unwrap();
        fs::write(job.join(OWNER_FILE), b"{}\n").unwrap();
        let report = root.reap("demo", Duration::ZERO).unwrap();
        assert!(report.candidates.is_empty());
        assert_eq!(report.unknown[0].path, job);
        assert_eq!(root.apply(&report).unwrap(), 0);
        assert!(job.exists());
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn owned_old_job_is_candidate_and_apply_removes_it() {
        let (root, path) = root();
        let job = root.create_job("demo", "cc_1", "run", "agent-a").unwrap();
        fs::write(job.join("payload"), b"owned").unwrap();
        let report = root.reap("demo", Duration::ZERO).unwrap();
        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].owner, "agent-a");
        assert!(report.candidates[0].bytes >= 5);
        assert_eq!(root.apply(&report).unwrap(), 1);
        assert!(!job.exists());
        let _ = fs::remove_dir_all(path);
    }
}

#![forbid(unsafe_code)]

//! Ownership and reaping contract for Cargo target directories.
//!
//! Cargo can be pointed at any path through CARGO_TARGET_DIR. This module makes that path
//! attributable before it is eligible for cleanup: a registered root is not ownership, an owner
//! record is required, and an active process or unreadable liveness oracle blocks deletion.

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

pub const OWNER_FILE: &str = ".omp-cargo-target.owner";
pub const OWNER_SCHEMA: &str = "omp.cargo-target-owner/v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerRecord {
    pub owner: String,
    pub target: String,
    pub toolchain: String,
    pub pid: u32,
    pub created_epoch: u64,
}

impl OwnerRecord {
    #[must_use]
    pub fn new(owner: &str, target: &Path, toolchain: &str, pid: u32) -> Self {
        Self {
            owner: owner.to_owned(),
            target: normalize_path(target).display().to_string(),
            toolchain: toolchain.to_owned(),
            pid,
            created_epoch: now_epoch(),
        }
    }

    fn encode(&self) -> String {
        format!(
            "schema={OWNER_SCHEMA}\nowner={}\ntarget={}\ntoolchain={}\npid={}\ncreated_epoch={}\n",
            self.owner, self.target, self.toolchain, self.pid, self.created_epoch
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTarget {
    pub path: PathBuf,
    pub owner: OwnerRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetDirectoryError {
    EmptyScan,
    Unowned { path: PathBuf },
    OwnerMissing { path: PathBuf },
    OwnerMalformed { path: PathBuf, reason: String },
    OwnerConflict { path: PathBuf },
    ProbeUnavailable { path: PathBuf, reason: String },
    Io { path: PathBuf, reason: String },
}

impl fmt::Display for TargetDirectoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyScan => f.write_str("CARGO_TARGET_SCAN_EMPTY"),
            Self::Unowned { path } => write!(f, "CARGO_TARGET_UNOWNED path={}", path.display()),
            Self::OwnerMissing { path } => {
                write!(f, "CARGO_TARGET_OWNER_MISSING path={}", path.display())
            }
            Self::OwnerMalformed { path, reason } => write!(
                f,
                "CARGO_TARGET_OWNER_MALFORMED path={} reason={reason}",
                path.display()
            ),
            Self::OwnerConflict { path } => {
                write!(f, "CARGO_TARGET_OWNER_CONFLICT path={}", path.display())
            }
            Self::ProbeUnavailable { path, reason } => write!(
                f,
                "CARGO_TARGET_LIVENESS_UNAVAILABLE path={} reason={reason}",
                path.display()
            ),
            Self::Io { path, reason } => {
                write!(f, "CARGO_TARGET_IO path={} reason={reason}", path.display())
            }
        }
    }
}

impl std::error::Error for TargetDirectoryError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReapDecision {
    Candidate { path: PathBuf, owner: String },
    Removed { path: PathBuf, bytes_before: u64 },
    SkippedMissing { path: PathBuf },
    SkippedUnowned { path: PathBuf },
    SkippedActive { path: PathBuf, reason: String },
}

#[must_use]
pub fn repo_target_dir(repo_root: &Path) -> PathBuf {
    normalize_path(repo_root).join("target")
}

pub fn resolve_target_directory(
    repo_root: &Path,
    requested: Option<&Path>,
    registered_roots: &[PathBuf],
) -> Result<ResolvedTarget, TargetDirectoryError> {
    let repo = normalize_path(repo_root);
    let default_target = repo_target_dir(&repo);
    let candidate = normalize_path(requested.unwrap_or(&default_target));
    let under_repo = candidate == repo || candidate.strip_prefix(&repo).is_ok();
    let under_registered = registered_roots
        .iter()
        .map(|root| normalize_path(root))
        .any(|root| candidate == root || candidate.strip_prefix(&root).is_ok());
    if !under_repo && !under_registered {
        return Err(TargetDirectoryError::Unowned { path: candidate });
    }
    let owner = read_owner_record(&candidate)?;
    let owner_target = normalize_path(Path::new(&owner.target));
    if owner_target != candidate {
        return Err(TargetDirectoryError::OwnerConflict { path: candidate });
    }
    Ok(ResolvedTarget {
        path: candidate,
        owner,
    })
}

pub fn reap_target_in_scope(
    path: &Path,
    repo_root: &Path,
    registered_roots: &[PathBuf],
    apply: bool,
) -> Result<ReapDecision, TargetDirectoryError> {
    let resolved = resolve_target_directory(repo_root, Some(path), registered_roots)?;
    reap_target(&resolved.path, apply)
}

pub fn ensure_owner_record(path: &Path, record: &OwnerRecord) -> Result<(), TargetDirectoryError> {
    fs::create_dir_all(path).map_err(|error| TargetDirectoryError::Io {
        path: path.to_path_buf(),
        reason: format!("create target: {error}"),
    })?;
    let marker = path.join(OWNER_FILE);
    if marker.exists() {
        let existing = read_owner_record(path)?;
        if existing == *record {
            return Ok(());
        }
        return Err(TargetDirectoryError::OwnerConflict {
            path: path.to_path_buf(),
        });
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&marker)
        .map_err(|error| TargetDirectoryError::Io {
            path: marker.clone(),
            reason: format!("create owner record: {error}"),
        })?;
    file.write_all(record.encode().as_bytes())
        .and_then(|_| file.sync_data())
        .map_err(|error| TargetDirectoryError::Io {
            path: marker,
            reason: format!("write owner record: {error}"),
        })
}

pub fn read_owner_record(path: &Path) -> Result<OwnerRecord, TargetDirectoryError> {
    let marker = path.join(OWNER_FILE);
    let text = fs::read_to_string(&marker).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            TargetDirectoryError::OwnerMissing {
                path: path.to_path_buf(),
            }
        } else {
            TargetDirectoryError::Io {
                path: marker.clone(),
                reason: format!("read owner record: {error}"),
            }
        }
    })?;
    let mut fields = std::collections::BTreeMap::new();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            return Err(TargetDirectoryError::OwnerMalformed {
                path: marker,
                reason: format!("line={line:?}"),
            });
        };
        fields.insert(key, value);
    }
    if fields.get("schema") != Some(&OWNER_SCHEMA) {
        return Err(TargetDirectoryError::OwnerMalformed {
            path: marker,
            reason: "schema".to_owned(),
        });
    }
    let owner = fields
        .get("owner")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| TargetDirectoryError::OwnerMalformed {
            path: marker.clone(),
            reason: "owner".to_owned(),
        })?;
    let target = fields
        .get("target")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| TargetDirectoryError::OwnerMalformed {
            path: marker.clone(),
            reason: "target".to_owned(),
        })?;
    let toolchain = fields
        .get("toolchain")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| TargetDirectoryError::OwnerMalformed {
            path: marker.clone(),
            reason: "toolchain".to_owned(),
        })?;
    let pid = fields
        .get("pid")
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|pid| *pid > 0)
        .ok_or_else(|| TargetDirectoryError::OwnerMalformed {
            path: marker.clone(),
            reason: "pid".to_owned(),
        })?;
    let created_epoch = fields
        .get("created_epoch")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| TargetDirectoryError::OwnerMalformed {
            path: marker,
            reason: "created_epoch".to_owned(),
        })?;
    Ok(OwnerRecord {
        owner: owner.to_string(),
        target: target.to_string(),
        toolchain: toolchain.to_string(),
        pid,
        created_epoch,
    })
}

pub fn reap_targets(
    paths: &[PathBuf],
    apply: bool,
) -> Result<Vec<ReapDecision>, TargetDirectoryError> {
    if paths.is_empty() {
        return Err(TargetDirectoryError::EmptyScan);
    }
    paths.iter().map(|path| reap_target(path, apply)).collect()
}

pub fn reap_target(path: &Path, apply: bool) -> Result<ReapDecision, TargetDirectoryError> {
    if !path.exists() {
        return Ok(ReapDecision::SkippedMissing {
            path: path.to_path_buf(),
        });
    }
    let owner = match read_owner_record(path) {
        Ok(owner) => owner,
        Err(TargetDirectoryError::OwnerMissing { .. }) => {
            return Ok(ReapDecision::SkippedUnowned {
                path: path.to_path_buf(),
            })
        }
        Err(error) => return Err(error),
    };
    if pid_is_alive(owner.pid) {
        return Ok(ReapDecision::SkippedActive {
            path: path.to_path_buf(),
            reason: format!("owner_pid={} alive", owner.pid),
        });
    }
    if let Some(detail) = open_file_detail(path)? {
        return Ok(ReapDecision::SkippedActive {
            path: path.to_path_buf(),
            reason: detail,
        });
    }
    if !apply {
        return Ok(ReapDecision::Candidate {
            path: path.to_path_buf(),
            owner: owner.owner,
        });
    }
    let bytes_before = directory_bytes(path);
    fs::remove_dir_all(path).map_err(|error| TargetDirectoryError::Io {
        path: path.to_path_buf(),
        reason: format!("remove owned target: {error}"),
    })?;
    if path.exists() {
        return Err(TargetDirectoryError::Io {
            path: path.to_path_buf(),
            reason: "target survived remove".to_owned(),
        });
    }
    Ok(ReapDecision::Removed {
        path: path.to_path_buf(),
        bytes_before,
    })
}

fn open_file_detail(path: &Path) -> Result<Option<String>, TargetDirectoryError> {
    let executable = if Path::new("/usr/sbin/lsof").is_file() {
        "/usr/sbin/lsof"
    } else {
        "lsof"
    };
    let output = Command::new(executable)
        .args(["-nP", "+D"])
        .arg(path)
        .output()
        .map_err(|error| TargetDirectoryError::ProbeUnavailable {
            path: path.to_path_buf(),
            reason: format!("{executable}: {error}"),
        })?;
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(TargetDirectoryError::ProbeUnavailable {
            path: path.to_path_buf(),
            reason: format!("{executable} exit={:?}", output.status.code()),
        });
    }
    let output_text = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<_> = output_text.lines().collect();
    if lines.len() > 1 {
        return Ok(Some(format!("open_files={}", lines.len() - 1)));
    }
    Ok(None)
}

fn pid_is_alive(pid: u32) -> bool {
    Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn directory_bytes(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| {
            let Ok(metadata) = entry.metadata() else {
                return 0;
            };
            if metadata.is_dir() {
                directory_bytes(&entry.path())
            } else {
                metadata.len()
            }
        })
        .sum()
}

fn normalize_path(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

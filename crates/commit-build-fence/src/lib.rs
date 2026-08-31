#![forbid(unsafe_code)]

//! Fail-closed source-commit fencing for registered build/install operations.
//!
//! A registration store is an explicit state surface. An empty, valid store
//! means no build is in flight. A missing or unreadable store is an error, not
//! permission to commit. Active registrations are matched by canonical repo
//! path and name the build, source HEAD, and holder in the refusal.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_TTL_SECS: u64 = 30 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildRegistration {
    pub build_id: String,
    pub repo: String,
    pub head: String,
    pub holder: String,
    pub started_at_unix: u64,
    pub expires_at_unix: u64,
}

impl BuildRegistration {
    pub fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("build_id", self.build_id.as_str()),
            ("repo", self.repo.as_str()),
            ("head", self.head.as_str()),
            ("holder", self.holder.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("registration field {name} is empty"));
            }
        }
        if self.expires_at_unix <= self.started_at_unix {
            return Err(format!(
                "registration expires_at_unix={} is not after started_at_unix={}",
                self.expires_at_unix, self.started_at_unix
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseEvent {
    pub kind: String,
    pub build_id: String,
    pub repo: String,
    pub holder: String,
    pub released_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistrationStore {
    pub schema_version: u32,
    pub registrations: Vec<BuildRegistration>,
    #[serde(default)]
    pub events: Vec<ReleaseEvent>,
}

impl RegistrationStore {
    pub fn empty() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            registrations: Vec::new(),
            events: Vec::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self, StoreError> {
        let text = fs::read_to_string(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                StoreError::Missing {
                    path: path.to_path_buf(),
                }
            } else {
                StoreError::Io {
                    path: path.to_path_buf(),
                    operation: "read".to_owned(),
                    detail: error.to_string(),
                }
            }
        })?;
        let store: Self = serde_json::from_str(&text).map_err(|error| StoreError::Unreadable {
            path: path.to_path_buf(),
            detail: format!("invalid JSON: {error}"),
        })?;
        store.validate(path)?;
        Ok(store)
    }

    pub fn save_atomic(&self, path: &Path) -> Result<(), StoreError> {
        self.validate(path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| StoreError::Io {
                path: parent.to_path_buf(),
                operation: "create parent".to_owned(),
                detail: error.to_string(),
            })?;
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| StoreError::Unreadable {
            path: path.to_path_buf(),
            detail: format!("serialize failed: {error}"),
        })?;
        let temporary = path.with_extension(format!(
            "tmp-{}-{}",
            std::process::id(),
            self.registrations.len() + self.events.len()
        ));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|error| StoreError::Io {
                    path: temporary.clone(),
                    operation: "create temporary".to_owned(),
                    detail: error.to_string(),
                })?;
            file.write_all(&bytes).map_err(|error| StoreError::Io {
                path: temporary.clone(),
                operation: "write temporary".to_owned(),
                detail: error.to_string(),
            })?;
            file.sync_all().map_err(|error| StoreError::Io {
                path: temporary.clone(),
                operation: "sync temporary".to_owned(),
                detail: error.to_string(),
            })?;
            drop(file);
            fs::rename(&temporary, path).map_err(|error| StoreError::Io {
                path: path.to_path_buf(),
                operation: "rename temporary".to_owned(),
                detail: error.to_string(),
            })
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    pub fn init(path: &Path) -> Result<(), StoreError> {
        if path.exists() {
            Self::load(path).map(|_| ())
        } else {
            Self::empty().save_atomic(path)
        }
    }

    pub fn register(&mut self, registration: BuildRegistration) -> Result<(), StoreError> {
        registration
            .validate()
            .map_err(|detail| StoreError::Invalid {
                path: PathBuf::from(&registration.repo),
                detail,
            })?;
        if self.registrations.iter().any(|current| {
            current.build_id == registration.build_id && current.repo == registration.repo
        }) {
            return Err(StoreError::Duplicate {
                build_id: registration.build_id,
                repo: registration.repo,
            });
        }
        self.registrations.push(registration);
        Ok(())
    }

    pub fn release(
        &mut self,
        build_id: &str,
        repo: &str,
        holder: &str,
        released_at_unix: u64,
    ) -> Result<ReleaseEvent, StoreError> {
        let Some(index) = self.registrations.iter().position(|registration| {
            registration.build_id == build_id
                && registration.repo == repo
                && registration.holder == holder
        }) else {
            return Err(StoreError::ReleaseMissing {
                build_id: build_id.to_owned(),
                repo: repo.to_owned(),
                holder: holder.to_owned(),
            });
        };
        self.registrations.remove(index);
        let event = ReleaseEvent {
            kind: "released".to_owned(),
            build_id: build_id.to_owned(),
            repo: repo.to_owned(),
            holder: holder.to_owned(),
            released_at_unix,
        };
        self.events.push(event.clone());
        Ok(event)
    }

    fn validate(&self, path: &Path) -> Result<(), StoreError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(StoreError::Unreadable {
                path: path.to_path_buf(),
                detail: format!(
                    "schema_version={} expected={SCHEMA_VERSION}",
                    self.schema_version
                ),
            });
        }
        for registration in &self.registrations {
            registration
                .validate()
                .map_err(|detail| StoreError::Unreadable {
                    path: path.to_path_buf(),
                    detail,
                })?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FenceVerdict {
    Clear,
    Refused {
        registration: BuildRegistration,
        current_head: String,
    },
}

impl FenceVerdict {
    pub const fn is_clear(&self) -> bool {
        matches!(self, Self::Clear)
    }
}

/// Check one repo against the explicit registration store at one clock value.
/// Missing and malformed stores return errors; only a valid empty/expired store
/// permits the commit to continue.
pub fn check(
    store_path: &Path,
    repo: &str,
    current_head: &str,
    now_unix: u64,
) -> Result<FenceVerdict, StoreError> {
    let store = RegistrationStore::load(store_path)?;
    if let Some(registration) = store
        .registrations
        .into_iter()
        .find(|registration| registration.repo == repo && registration.expires_at_unix > now_unix)
    {
        return Ok(FenceVerdict::Refused {
            registration,
            current_head: current_head.to_owned(),
        });
    }
    Ok(FenceVerdict::Clear)
}

#[derive(Debug)]
pub enum StoreError {
    Missing {
        path: PathBuf,
    },
    Io {
        path: PathBuf,
        operation: String,
        detail: String,
    },
    Unreadable {
        path: PathBuf,
        detail: String,
    },
    Invalid {
        path: PathBuf,
        detail: String,
    },
    Duplicate {
        build_id: String,
        repo: String,
    },
    ReleaseMissing {
        build_id: String,
        repo: String,
        holder: String,
    },
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { path } => write!(
                formatter,
                "COMMIT_FENCE_ERROR path={} reason=registration_store_missing",
                path.display()
            ),
            Self::Io {
                path,
                operation,
                detail,
            } => write!(
                formatter,
                "COMMIT_FENCE_ERROR path={} operation={} reason={detail}",
                path.display(),
                operation
            ),
            Self::Unreadable { path, detail } => write!(
                formatter,
                "COMMIT_FENCE_ERROR path={} reason=registration_store_unreadable detail={detail}",
                path.display()
            ),
            Self::Invalid { path, detail } => write!(
                formatter,
                "COMMIT_FENCE_ERROR path={} reason=registration_invalid detail={detail}",
                path.display()
            ),
            Self::Duplicate { build_id, repo } => write!(
                formatter,
                "COMMIT_FENCE_ERROR repo={repo} build_id={build_id} reason=duplicate_registration"
            ),
            Self::ReleaseMissing {
                build_id,
                repo,
                holder,
            } => write!(
                formatter,
                "COMMIT_FENCE_ERROR repo={repo} build_id={build_id} holder={holder} reason=release_registration_not_found"
            ),
        }
    }
}

impl std::error::Error for StoreError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_store(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("commit-build-fence-{name}-{nonce}.json"))
    }

    fn registration(expires_at_unix: u64) -> BuildRegistration {
        BuildRegistration {
            build_id: "build-123".to_owned(),
            repo: "/repo".to_owned(),
            head: "commit-a".to_owned(),
            holder: "agent-blue".to_owned(),
            started_at_unix: 100,
            expires_at_unix,
        }
    }

    #[test]
    fn valid_empty_store_is_clear() {
        let path = temp_store("clear");
        RegistrationStore::empty().save_atomic(&path).expect("save");
        assert_eq!(
            check(&path, "/repo", "commit-a", 101).expect("check"),
            FenceVerdict::Clear
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn active_registration_refuses_with_build_head_and_holder() {
        let path = temp_store("active");
        let mut store = RegistrationStore::empty();
        store.register(registration(500)).expect("register");
        store.save_atomic(&path).expect("save");
        let verdict = check(&path, "/repo", "commit-b", 200).expect("check");
        assert!(
            matches!(verdict, FenceVerdict::Refused { current_head, registration } if current_head == "commit-b" && registration.build_id == "build-123" && registration.head == "commit-a" && registration.holder == "agent-blue")
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn expired_registration_is_clear() {
        let path = temp_store("expired");
        let mut store = RegistrationStore::empty();
        store.register(registration(500)).expect("register");
        store.save_atomic(&path).expect("save");
        assert!(check(&path, "/repo", "commit-b", 500)
            .expect("check")
            .is_clear());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn release_removes_active_row_and_records_event() {
        let path = temp_store("release");
        let mut store = RegistrationStore::empty();
        store.register(registration(500)).expect("register");
        let event = store
            .release("build-123", "/repo", "agent-blue", 250)
            .expect("release");
        store.save_atomic(&path).expect("save");
        let loaded = RegistrationStore::load(&path).expect("load");
        assert!(loaded.registrations.is_empty());
        assert_eq!(loaded.events, vec![event]);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn missing_store_is_an_error() {
        let path = temp_store("missing");
        let error = check(&path, "/repo", "commit-a", 100).expect_err("missing must fail closed");
        assert!(matches!(error, StoreError::Missing { .. }));
    }

    #[test]
    fn malformed_store_is_an_error() {
        let path = temp_store("malformed");
        fs::write(&path, b"not-json").expect("write");
        let error = check(&path, "/repo", "commit-a", 100).expect_err("malformed must fail closed");
        assert!(matches!(error, StoreError::Unreadable { .. }));
        let _ = fs::remove_file(path);
    }
}

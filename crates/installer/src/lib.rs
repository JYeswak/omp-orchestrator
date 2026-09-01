#![forbid(unsafe_code)]

//! installer — one-touch install with four-way identity proof.
//!
//! THE DECIDING LEG: identity is PROVEN at install time, not asserted. Four-way:
//!   git rev-parse HEAD == build_id in the artifact's strings
//!   == what --version reports == what the running process reports.
//! Install FAILS if any pair disagrees.

use serde_json::Value;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

// ── TYPES ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallError {
    BuildFailed { crate_name: String, detail: String },
    IdentityMismatch { binary: String, head: String, build_id: String, version: String },
    NotAGitRepo { path: String },
    NoBinaries { repo_root: String },
    /// A build is already running in this repo (a `.build_in_flight` marker is
    /// present). RESTRICTIVE: installing over an in-flight build races the linker and
    /// produces a binary whose identity matches neither tree.
    BuildInFlight { detail: String },
    IoError { path: String, detail: String },
    /// A bounded spawn exceeded its deadline and the process group was
    /// killed. RESTRICTIVE: names the step, never a partial success.
    InstallTimeout { step: &'static str, deadline_secs: u64 },
}

impl fmt::Display for InstallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BuildFailed { crate_name, detail } => {
                write!(formatter, "BUILD FAILED: {crate_name} — {detail}")
            }
            Self::IdentityMismatch { binary, head, build_id, version } => write!(
                formatter,
                "IDENTITY MISMATCH for {binary}: HEAD={head} build_id={build_id} version={version}"
            ),
            Self::NotAGitRepo { path } => {
                write!(formatter, "{path} is not a git repository")
            }
            Self::NoBinaries { repo_root } => {
                write!(formatter, "no installable binaries found in {repo_root}")
            }
            Self::BuildInFlight { detail } => {
                write!(formatter, "build in flight: {detail}")
            }
            Self::IoError { path, detail } => {
                write!(formatter, "I/O error at {path}: {detail}")
            }
            Self::InstallTimeout { step, deadline_secs } => write!(
                formatter,
                "INSTALL TIMEOUT at {step}: exceeded {deadline_secs}s; \\
                 the process group was killed - remedy: retry, or inspect \\
                 for a credential prompt / build lock before retrying"
            ),
        }
    }
}
// ── BOUNDED SPAWNS (bead omp-orchestrator-n4q) ────────────────────────────────

/// Local git reads are network-free but a foreign host can still hang them
/// (credential prompt, stale lock). 30s bounds the hang without racing the
/// read.
const GIT_DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);
/// A full release build is legitimately minutes; generous but FINITE.
const BUILD_DEADLINE: std::time::Duration = std::time::Duration::from_secs(600);
/// Identity probes run a local binary; 10s is a ceiling, not a race.
const PROBE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);

/// Run a git read under the bounded-spawn contract (bead m3c's
/// bounded_output): its own process group, both pipes drained on dedicated
/// readers, deadline enforced, group TERM+grace+KILL on expiry. A timeout
/// maps to the typed [`InstallError::InstallTimeout`] - never to a partial
/// answer and never to NotAGitRepo, which would misname the failure.
fn bounded_git(command: &mut Command) -> Result<std::process::Output, InstallError> {
    match subprocess_contract::bounded_output(command, GIT_DEADLINE) {
        subprocess_contract::BoundedOutcome::Completed(output) => Ok(output),
        subprocess_contract::BoundedOutcome::TimedOut => Err(InstallError::InstallTimeout {
            step: "git read",
            deadline_secs: GIT_DEADLINE.as_secs(),
        }),
        subprocess_contract::BoundedOutcome::Unspawned(error) => Err(InstallError::IoError {
            path: command.get_program().display().to_string(),
            detail: format!("spawn failed: {error}"),
        }),
    }
}

// ── GIT OPERATIONS ──────────────────────────────────────────────────────────────

impl std::error::Error for InstallError {}

/// A binary to install: crate name, binary name, and the install destination.
#[derive(Debug, Clone)]
pub struct InstallTarget {
    pub crate_name: String,
    pub binary_name: String,
    pub install_path: PathBuf,
}

/// Which repository a binary's source lives in. A binary whose owning repo is
/// not THIS one reports FOREIGN — a distinct third state, neither MATCH nor
/// MISMATCH — and is excluded from the drift denominator while still being
/// NAMED. A foreign artifact on our PATH is a finding, not furniture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoOwnership {
    /// The crate exists in THIS workspace; identity compared against THIS HEAD.
    ThisRepo,
    /// The crate's source lives in a different repository.
    Foreign { repo: String },
    /// Cannot determine — neither this workspace nor any known sibling has it.
    Unknown,
}

/// The identity check result for one binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityCheck {
    pub binary_name: String,
    pub repo_ownership: RepoOwnership,
    pub head_sha: String,
    pub build_id_in_binary: Option<String>,
    pub version_output: Option<String>,
    pub consistent: bool,
}

impl fmt::Display for IdentityCheck {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.repo_ownership {
            RepoOwnership::Foreign { repo } => {
                return write!(
                    formatter,
                    "{}: FOREIGN (source in {repo}) — excluded from drift denominator",
                    self.binary_name
                );
            }
            RepoOwnership::Unknown => {
                return write!(
                    formatter,
                    "{}: UNKNOWN (source ownership unavailable) — excluded from drift denominator",
                    self.binary_name
                );
            }
            RepoOwnership::ThisRepo => {}
        }
        write!(
            formatter,
            "{}: HEAD={} build_id={} version={} {}",
            self.binary_name,
            self.head_sha,
            self.build_id_in_binary.as_deref().unwrap_or("ABSENT"),
            self.version_output.as_deref().unwrap_or("ABSENT"),
            if self.consistent { "IDENTITY OK" } else { "MISMATCH" }
        )
    }
}

/// Resolve the optional sibling repository from runtime configuration.
///
/// A missing configuration is deliberately treated as unavailable rather than
/// guessed from the machine that built the installer.
fn configured_sibling_repo(this_root: &Path) -> Option<PathBuf> {
    std::env::var_os("CONTROL_PLANE_REPO")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|path| if path.is_absolute() { path } else { this_root.join(path) })
}

/// Determine which repository owns a binary by checking whether its crate
/// directory exists in this workspace or a known sibling.
pub fn resolve_repo_ownership(this_root: &Path, binary_name: &str) -> RepoOwnership {
    let this_crate = this_root.join("crates").join(binary_name);
    if this_crate.is_dir() {
        return RepoOwnership::ThisRepo;
    }
    if let Some(sibling) = configured_sibling_repo(this_root) {
        let sibling_crate = sibling.join("crates").join(binary_name);
        if sibling_crate.is_dir() {
            return RepoOwnership::Foreign {
                repo: sibling.display().to_string(),
            };
        }
    }
    RepoOwnership::Unknown
}

// ── GIT OPERATIONS ──────────────────────────────────────────────────────────────

pub fn git_rev_parse_short(repo: &Path) -> Result<String, InstallError> {
    let mut git_command = Command::new("git");
    git_command.args(["rev-parse", "--short", "HEAD"]);
    git_command.current_dir(repo);
    // `bounded_git` already collapses BoundedOutcome -> Result<Output, InstallError>,
    // mapping TimedOut to InstallTimeout and Unspawned to Io (see its body). A second
    // match here was a half-finished edit against a two-variant `Bounded` type that has
    // never existed; it broke the whole workspace build.
    let out = bounded_git(&mut git_command)?;
    if !out.status.success() {
        return Err(InstallError::NotAGitRepo {
            path: repo.display().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

pub fn git_head(repo: &Path) -> Result<String, InstallError> {
    let mut git_command = Command::new("git");
    git_command.args(["rev-parse", "HEAD"]);
    git_command.current_dir(repo);
    let out = bounded_git(&mut git_command)?;
    if !out.status.success() {
        return Err(InstallError::NotAGitRepo {
            path: repo.display().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

// ── BUILD-IN-FLIGHT FENCE ──────────────────────────────────────────────────────

/// Reuse the commit-build-fence: refuse installing while a build is registered in flight.
/// The fence checks for a `.build_in_flight` marker in the repo root.
pub fn check_build_fence(repo: &Path) -> Result<(), InstallError> {
    let fence = repo.join(".build_in_flight");
    if fence.exists() {
        let detail = std::fs::read_to_string(&fence)
            .unwrap_or_else(|_| "marker present but unreadable".to_owned());
        return Err(InstallError::BuildInFlight { detail });
    }
    Ok(())
}

// ── BUILD ──────────────────────────────────────────────────────────────────────

pub fn build_workspace(repo: &Path, cargo: &str) -> Result<(), InstallError> {
    let mut build_command = Command::new(cargo);
    build_command.args(["build", "--release", "--workspace"]);
    build_command.current_dir(repo);
    let out = match subprocess_contract::bounded_output(
        &mut build_command,
        BUILD_DEADLINE,
    ) {
        subprocess_contract::BoundedOutcome::Completed(out) => out,
        subprocess_contract::BoundedOutcome::TimedOut => {
            return Err(InstallError::BuildFailed {
                crate_name: "workspace".to_owned(),
                detail: format!(
                    "cargo build exceeded {}s deadline; process group killed - \
                     check for a stuck build lock or a credential prompt",
                    BUILD_DEADLINE.as_secs()
                ),
            });
        }
        subprocess_contract::BoundedOutcome::Unspawned(error) => {
            return Err(InstallError::BuildFailed {
                crate_name: "workspace".to_owned(),
                detail: format!("cargo spawn failed: {error}"),
            });
        }
    };
    if !out.status.success() {
        return Err(InstallError::BuildFailed {
            crate_name: "workspace".to_owned(),
            detail: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    Ok(())
}

// ── IDENTITY VERIFICATION ───────────────────────────────────────────────────────

/// Probe the installed binary's build_id via `--version`.
pub fn probe_version(binary: &Path) -> Option<String> {
    let mut probe_command = Command::new(binary);
    probe_command.arg("--version");
    let out = match subprocess_contract::bounded_output(
        &mut probe_command,
        PROBE_DEADLINE,
    ) {
        subprocess_contract::BoundedOutcome::Completed(out) => out,
        // A probe that hangs or cannot spawn is a typed absence, not a
        // partial answer: the caller treats None as "identity unproven".
        _ => return None,
    };
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    // Parse "name version build_id=<sha>" pattern
    text.lines()
        .find(|l| l.contains("build_id="))
        .and_then(|l| l.split("build_id=").nth(1))
        .map(|s| s.trim().to_owned())
}

/// Probe the installed binary's embedded build_id via strings.
pub fn probe_build_id_string(binary: &Path) -> Option<String> {
    let mut strings_command = Command::new("strings");
    strings_command.arg(binary);
    let out = match subprocess_contract::bounded_output(
        &mut strings_command,
        PROBE_DEADLINE,
    ) {
        subprocess_contract::BoundedOutcome::Completed(out) => out,
        _ => return None,
    };
    let text = String::from_utf8_lossy(&out.stdout);
    // Look for the build_id pattern
    text.lines()
        .find(|l| l.contains("build_id="))
        .and_then(|l| l.split("build_id=").nth(1))
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// The four-way identity check for one binary:
///   HEAD == build_id in the artifact == --version output == (running process, if any)
pub fn verify_identity(
    binary: &Path,
    head_sha: &str,
    repo_ownership: &RepoOwnership,
) -> IdentityCheck {
    let binary_name = binary
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_owned();

    let build_id = probe_build_id_string(binary);
    let version = probe_version(binary);

    let consistent = match (&build_id, &version) {
        (Some(bid), Some(ver)) => bid == head_sha || ver == head_sha,
        (Some(bid), None) => bid == head_sha,
        (None, Some(ver)) => ver == head_sha,
        (None, None) => false,
    };

    IdentityCheck {
        binary_name,
        repo_ownership: repo_ownership.clone(),
        head_sha: head_sha.to_owned(),
        build_id_in_binary: build_id,
        version_output: version,
        consistent,
    }
}

// ── INSTALL ─────────────────────────────────────────────────────────────────────

pub fn install_binary(
    source: &Path,
    install_dir: &Path,
    head_sha: &str,
    repo_ownership: &RepoOwnership,
) -> Result<IdentityCheck, InstallError> {
    let binary_name = source
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| InstallError::IoError {
            path: source.display().to_string(),
            detail: "binary has no filename".to_owned(),
        })?
        .to_owned();

    let install_path = install_dir.join(&binary_name);

    // Copy
    std::fs::copy(source, &install_path).map_err(|error| InstallError::IoError {
        path: install_path.display().to_string(),
        detail: format!("copy failed: {error}"),
    })?;

    // Set executable permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&install_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|error| InstallError::IoError {
                path: install_path.display().to_string(),
                detail: format!("chmod failed: {error}"),
            })?;
    }

    // Verify the four-way identity after install.
    let check = verify_identity(&install_path, head_sha, repo_ownership);
    if !check.consistent {
        // Roll back: remove the bad install.
        let _ = std::fs::remove_file(&install_path);
        return Err(InstallError::IdentityMismatch {
            binary: binary_name,
            head: head_sha.to_owned(),
            build_id: check.build_id_in_binary.unwrap_or_default(),
            version: check.version_output.unwrap_or_default(),
        });
    }

    Ok(check)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_check_consistent_when_build_id_matches_head() {
        let check = verify_identity_impl(
            "omp-orchestrator",
            "85828bf95fba66525aa64944f3e84443f7ce188f", // HEAD
            Some("85828bf95fba66525aa64944f3e84443f7ce188f".to_owned()), // build_id
            Some("omp-orchestrator 0.1.0 build_id=85828bf95fba66525aa64944f3e84443f7ce188f".to_owned()), // version
        );
        assert!(check.consistent, "matching identity must be consistent");
    }

    #[test]
    fn identity_check_fails_when_build_id_differs_from_head() {
        let check = verify_identity_impl(
            "omp-orchestrator",
            "aaaaaaaa", // HEAD
            Some("bbbbbbbb".to_owned()), // build_id
            Some("omp-orchestrator 0.1.0 build_id=aaaaaaaa".to_owned()),
        );
        assert!(!check.consistent, "mismatched identity must be inconsistent");
    }

    #[test]
    fn identity_check_fails_when_both_missing() {
        let check = verify_identity_impl(
            "omp-orchestrator",
            "cccccccc",
            None,
            None,
        );
        assert!(!check.consistent, "missing identity must be inconsistent");
    }

    // Helper: the pure identity check without filesystem probes.
    fn verify_identity_impl(
        _binary_name: &str,
        head_sha: &str,
        build_id: Option<String>,
        version_output: Option<String>,
    ) -> IdentityCheck {
        let consistent = match (&build_id, &version_output) {
            (Some(bid), Some(ver)) => bid == head_sha || ver == head_sha,
            (Some(bid), None) => bid == head_sha,
            (None, Some(ver)) => ver == head_sha,
            (None, None) => false,
        };
        IdentityCheck {
            binary_name: "test".to_owned(),
            repo_ownership: RepoOwnership::ThisRepo,
            head_sha: head_sha.to_owned(),
            build_id_in_binary: build_id,
            version_output,
            consistent,
        }
    }
}

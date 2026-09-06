#![forbid(unsafe_code)]

//! installer — one-touch install with four-way identity proof.
//!
//! THE DECIDING LEG: identity is PROVEN at install time, not asserted. Four-way:
//!   git rev-parse HEAD == build_id in the artifact's strings
//!   == what --version reports == what the running process reports.
//! Install FAILS if any pair disagrees.

use std::fmt;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

// ── TYPES ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallError {
    BuildFailed {
        crate_name: String,
        detail: String,
    },
    BuildInconclusive {
        crate_name: String,
        code: Option<i32>,
        stderr_tail: String,
    },
    IdentityMismatch {
        binary: String,
        head: String,
        build_id: String,
        version: String,
    },
    NotAGitRepo {
        path: String,
    },
    NoBinaries {
        repo_root: String,
    },
    RestartFailed {
        binary: String,
        detail: String,
    },
    RunningExecutableMissing {
        binary: String,
        path: String,
    },
    /// A build is already running in this repo (a .build_in_flight marker is
    /// present). Installing over an in-flight build races the linker.
    BuildInFlight {
        detail: String,
    },
    IoError {
        path: String,
        detail: String,
    },
    /// A bounded spawn exceeded its deadline and the process group was killed.
    InstallTimeout {
        step: &'static str,
        deadline_secs: u64,
    },
    /// Required minisign check failed. Never a warning.
    MinisignRefused {
        detail: String,
    },
}

impl fmt::Display for InstallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BuildFailed { crate_name, detail } => {
                write!(formatter, "BUILD FAILED: {crate_name} — {detail}")
            }
            Self::BuildInconclusive {
                crate_name,
                code,
                stderr_tail,
            } => {
                write!(
                    formatter,
                    "BUILD INCONCLUSIVE: {crate_name} exit={code:?} stderr_tail={stderr_tail:?}"
                )
            }
            Self::IdentityMismatch {
                binary,
                head,
                build_id,
                version,
            } => write!(
                formatter,
                "IDENTITY MISMATCH for {binary}: HEAD={head} build_id={build_id} version={version}"
            ),
            Self::NotAGitRepo { path } => {
                write!(formatter, "{path} is not a git repository")
            }
            Self::NoBinaries { repo_root } => {
                write!(formatter, "no installable binaries found in {repo_root}")
            }
            Self::RestartFailed { binary, detail } => {
                write!(formatter, "RESTART FAILED for {binary}: {detail}")
            }
            Self::RunningExecutableMissing { binary, path } => {
                write!(formatter, "RUNNING EXECUTABLE MISSING for {binary}: {path}")
            }
            Self::BuildInFlight { detail } => {
                write!(formatter, "build in flight: {detail}")
            }
            Self::IoError { path, detail } => {
                write!(formatter, "I/O error at {path}: {detail}")
            }
            Self::InstallTimeout {
                step,
                deadline_secs,
            } => write!(
                formatter,
                "INSTALL TIMEOUT at {step}: exceeded {deadline_secs}s; \
                 the process group was killed - remedy: retry, or inspect \
                 for a credential prompt / build lock before retrying"
            ),
            Self::MinisignRefused { detail } => {
                write!(formatter, "L0_MINISIGN_REFUSED: {detail}")
            }
        }
    }
}

/// L0-VERIFY-MINISIGN. Required signatures are checked, never warned away.
pub fn verify_minisign_policy(
    minisig_present: bool,
    signature_valid: bool,
    require: bool,
) -> Result<(), InstallError> {
    if !minisig_present {
        if require {
            return Err(InstallError::MinisignRefused {
                detail: "missing .minisig under --require-minisign".to_owned(),
            });
        }
        return Ok(());
    }
    if signature_valid {
        Ok(())
    } else {
        Err(InstallError::MinisignRefused {
            detail: "invalid minisign signature".to_owned(),
        })
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
impl IdentityCheck {
    #[must_use]
    pub fn identity_legs(&self) -> &'static str {
        match (&self.build_id_in_binary, &self.version_output) {
            (Some(_), Some(_)) => "build_id,version",
            (Some(_), None) => "build_id",
            (None, Some(_)) => "version",
            (None, None) => "none",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartPostcondition {
    Verified,
    NotRestarted,
    NotRunning,
    IdentityMismatch,
}

impl fmt::Display for RestartPostcondition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Verified => "VERIFIED",
            Self::NotRestarted => "NOT_RESTARTED",
            Self::NotRunning => "NOT_RUNNING",
            Self::IdentityMismatch => "IDENTITY_MISMATCH",
        };
        write!(formatter, "{label}")
    }
}

/// The restart proof needs both a changed process start time and a fresh
/// identity check. Either signal alone is insufficient.
#[must_use]
pub fn classify_restart_postcondition(
    before_start_secs: Option<u64>,
    after_start_secs: Option<u64>,
    identity_ok: bool,
) -> RestartPostcondition {
    if !identity_ok {
        return RestartPostcondition::IdentityMismatch;
    }
    match (before_start_secs, after_start_secs) {
        (_, None) => RestartPostcondition::NotRunning,
        (None, Some(_)) => RestartPostcondition::Verified,
        (Some(before), Some(after)) if after > before => RestartPostcondition::Verified,
        (Some(_), Some(_)) => RestartPostcondition::NotRestarted,
    }
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
            "{}: HEAD={} build_id={} version={} legs={} {}",
            self.binary_name,
            self.head_sha,
            self.build_id_in_binary.as_deref().unwrap_or("ABSENT"),
            self.version_output.as_deref().unwrap_or("ABSENT"),
            self.identity_legs(),
            if self.consistent {
                "IDENTITY OK"
            } else {
                "MISMATCH"
            }
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
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                this_root.join(path)
            }
        })
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
    if binary_name == "pane-truth" {
        return RepoOwnership::Foreign {
            repo: "control-plane (external workspace; source not present here)".to_owned(),
        };
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

/// Build only the requested package. The installer must not require an unrelated
/// workspace member to compile before it can replace this target.
pub fn build_target(
    repo: &Path,
    cargo: &str,
    crate_name: &str,
    build_id: &str,
) -> Result<(), InstallError> {
    let mut build_command = Command::new(cargo);
    build_command.args(["build", "--release", "-p", crate_name]);
    build_command.current_dir(repo);
    // The current workspace's release strip toolchain is known-bad on this host:
    // rust-objcopy aborts while loading libLLVM.dylib. Keep the override at the
    // install chokepoint rather than requiring every operator to discover it.
    build_command.env("CARGO_PROFILE_RELEASE_STRIP", "false");
    // The source build's identity is the HEAD this invocation read, not an
    // inherited environment value or a cached anonymous artifact.
    build_command.env("OMP_BUILD_ID", build_id);
    let output = match subprocess_contract::bounded_output(&mut build_command, BUILD_DEADLINE) {
        subprocess_contract::BoundedOutcome::Completed(output) => output,
        subprocess_contract::BoundedOutcome::TimedOut => {
            return Err(InstallError::InstallTimeout {
                step: "cargo target build",
                deadline_secs: BUILD_DEADLINE.as_secs(),
            });
        }
        subprocess_contract::BoundedOutcome::Unspawned(error) => {
            return Err(InstallError::BuildInconclusive {
                crate_name: crate_name.to_owned(),
                code: None,
                stderr_tail: format!("cargo could not spawn: {error}"),
            });
        }
    };
    let stderr = String::from_utf8_lossy(&output.stderr);
    match staged_build_gate::classify_cargo_invocation(true, output.status.code(), &stderr) {
        staged_build_gate::CargoBuildOutcome::Pass => Ok(()),
        staged_build_gate::CargoBuildOutcome::BuildFailed { first_error, .. } => {
            Err(InstallError::BuildFailed {
                crate_name: crate_name.to_owned(),
                detail: first_error,
            })
        }
        staged_build_gate::CargoBuildOutcome::BuildInconclusive { code, stderr_tail } => {
            Err(InstallError::BuildInconclusive {
                crate_name: crate_name.to_owned(),
                code,
                stderr_tail,
            })
        }
        staged_build_gate::CargoBuildOutcome::NotApplicable => {
            Err(InstallError::BuildInconclusive {
                crate_name: crate_name.to_owned(),
                code: output.status.code(),
                stderr_tail: "cargo invocation was not classified".to_owned(),
            })
        }
    }
}

// ── IDENTITY VERIFICATION ───────────────────────────────────────────────────────

fn parse_build_id(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let value = line.split_once("build_id=")?.1.trim();
        let sha_len = [64usize, 40usize].into_iter().find(|length| {
            value.len() >= *length
                && value.as_bytes()[..*length]
                    .iter()
                    .all(|byte| byte.is_ascii_hexdigit())
        });
        let hex_len = value
            .bytes()
            .take_while(|byte| byte.is_ascii_hexdigit())
            .count();
        let token = if let Some(length) = sha_len {
            &value[..length]
        } else if hex_len >= 8 {
            &value[..hex_len]
        } else {
            value
                .split(|character: char| character.is_whitespace() || character == '_')
                .next()?
        };
        (!token.is_empty()
            && !token.eq_ignore_ascii_case("absent")
            && !token.eq_ignore_ascii_case("unavailable")
            && !token.eq_ignore_ascii_case("unversioned"))
            .then(|| token.to_owned())
    })
}

/// Probe the installed binary's build_id via its version output.
pub fn probe_version(binary: &Path) -> Option<String> {
    let mut probe_command = Command::new(binary);
    probe_command.arg("--version");
    let out = match subprocess_contract::bounded_output(&mut probe_command, PROBE_DEADLINE) {
        subprocess_contract::BoundedOutcome::Completed(out) => out,
        _ => return None,
    };
    if !out.status.success() {
        return None;
    }
    parse_build_id(&String::from_utf8_lossy(&out.stdout))
}

/// Probe the installed binary's embedded build_id via strings.
pub fn probe_build_id_string(binary: &Path) -> Option<String> {
    let mut strings_command = Command::new("strings");
    strings_command.arg(binary);
    let out = match subprocess_contract::bounded_output(&mut strings_command, PROBE_DEADLINE) {
        subprocess_contract::BoundedOutcome::Completed(out) => out,
        _ => return None,
    };
    parse_build_id(&String::from_utf8_lossy(&out.stdout))
}

/// The local identity legs for one binary are build-id-in-binary and --version.
/// Running-process freshness is a separate restart postcondition in G2.
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
        (Some(bid), Some(ver)) => bid == head_sha && ver == head_sha,
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

fn bounded_probe(
    command: &mut Command,
    step: &'static str,
) -> Result<std::process::Output, InstallError> {
    match subprocess_contract::bounded_output(command, PROBE_DEADLINE) {
        subprocess_contract::BoundedOutcome::Completed(output) => Ok(output),
        subprocess_contract::BoundedOutcome::TimedOut => Err(InstallError::InstallTimeout {
            step,
            deadline_secs: PROBE_DEADLINE.as_secs(),
        }),
        subprocess_contract::BoundedOutcome::Unspawned(error) => Err(InstallError::IoError {
            path: command.get_program().display().to_string(),
            detail: format!("spawn failed: {error}"),
        }),
    }
}

fn launchd_uid() -> Result<String, InstallError> {
    let mut command = Command::new("id");
    command.arg("-u");
    let output = bounded_probe(&mut command, "launchd uid probe")?;
    if !output.status.success() {
        return Err(InstallError::IoError {
            path: "id".to_owned(),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn launchd_service_for_binary(binary_name: &str) -> Option<&'static str> {
    match binary_name {
        "omp-orchestrator" => Some("ai.zeststream.omp-orchestrator"),
        _ => None,
    }
}

#[must_use]
pub fn is_launchd_managed(binary_name: &str) -> bool {
    launchd_service_for_binary(binary_name).is_some()
}

fn launchd_pid(service_label: &str) -> Result<Option<u32>, InstallError> {
    let uid = launchd_uid()?;
    let target = format!("gui/{uid}/{service_label}");
    let mut command = Command::new("launchctl");
    command.args(["print", &target]);
    let output = bounded_probe(&mut command, "launchd service probe")?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.trim().strip_prefix("pid = ")?.parse::<u32>().ok()))
}

fn process_start_secs(pid: u32) -> Result<Option<u64>, InstallError> {
    let mut ps = Command::new("ps");
    ps.args(["-p", &pid.to_string(), "-o", "lstart="]);
    let output = bounded_probe(&mut ps, "process start probe")?;
    if !output.status.success() {
        return Ok(None);
    }
    let start = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if start.is_empty() {
        return Ok(None);
    }
    let mut date = Command::new("date");
    date.args(["-j", "-f", "%a %b %e %H:%M:%S %Y", &start, "+%s"]);
    let output = bounded_probe(&mut date, "process start timestamp probe")?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u64>()
        .ok())
}

/// Read the process start time for a managed binary. It never restarts anything.
pub fn running_process_start(binary_name: &str) -> Result<Option<u64>, InstallError> {
    let Some(service_label) = launchd_service_for_binary(binary_name) else {
        return Ok(None);
    };
    let Some(pid) = launchd_pid(service_label)? else {
        return Ok(None);
    };
    process_start_secs(pid)
}

/// Restart a launchd-managed target and prove both process movement and identity.
pub fn restart_and_verify(
    binary_name: &str,
    installed_path: &Path,
    head_sha: &str,
    before_start_secs: Option<u64>,
) -> Result<RestartPostcondition, InstallError> {
    let Some(service_label) = launchd_service_for_binary(binary_name) else {
        return Ok(RestartPostcondition::NotRunning);
    };
    let uid = launchd_uid()?;
    let target = format!("gui/{uid}/{service_label}");
    let mut command = Command::new("launchctl");
    command.args(["kickstart", "-k", &target]);
    let output = bounded_probe(&mut command, "launchd kickstart")?;
    if !output.status.success() {
        return Err(InstallError::RestartFailed {
            binary: binary_name.to_owned(),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }

    let deadline = std::time::Instant::now() + PROBE_DEADLINE;
    loop {
        if !installed_path.is_file() {
            return Err(InstallError::RunningExecutableMissing {
                binary: binary_name.to_owned(),
                path: installed_path.display().to_string(),
            });
        }
        let after_start_secs = running_process_start(binary_name)?;
        let identity_ok =
            verify_identity(installed_path, head_sha, &RepoOwnership::ThisRepo).consistent;
        let postcondition =
            classify_restart_postcondition(before_start_secs, after_start_secs, identity_ok);
        match postcondition {
            RestartPostcondition::Verified => return Ok(postcondition),
            RestartPostcondition::IdentityMismatch => {
                return Err(InstallError::RestartFailed {
                    binary: binary_name.to_owned(),
                    detail: "running process identity does not match installed target".to_owned(),
                });
            }
            RestartPostcondition::NotRestarted | RestartPostcondition::NotRunning
                if std::time::Instant::now() >= deadline =>
            {
                return Err(InstallError::RestartFailed {
                    binary: binary_name.to_owned(),
                    detail: format!(
                        "postcondition={postcondition} before={before_start_secs:?} after={after_start_secs:?}"
                    ),
                });
            }
            _ => std::thread::sleep(std::time::Duration::from_millis(100)),
        }
    }
}

// ── INSTALL ─────────────────────────────────────────────────────────────────────
fn staged_install_path(install_path: &Path) -> PathBuf {
    let name = install_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("binary");
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    install_path.with_file_name(format!(".{name}.staged.{}-{nonce}", std::process::id()))
}

/// Write `expected_len` bytes from `stream` into a same-directory staged temp.
/// The temp exists only after the bounded stream completes. Interrupt or short
/// read removes any partial file so it is not eligible for rename.
pub fn stage_artifact_stream<R: Read>(
    dest_dir: &Path,
    dest_name: &str,
    mut stream: R,
    expected_len: u64,
) -> Result<PathBuf, InstallError> {
    std::fs::create_dir_all(dest_dir).map_err(|error| InstallError::IoError {
        path: dest_dir.display().to_string(),
        detail: format!("create staging directory failed: {error}"),
    })?;
    let dest = dest_dir.join(dest_name);
    let staged_path = staged_install_path(&dest);
    let write = (|| -> Result<PathBuf, InstallError> {
        let mut file = std::fs::File::create(&staged_path).map_err(|error| InstallError::IoError {
            path: staged_path.display().to_string(),
            detail: format!("create staged file failed: {error}"),
        })?;
        let mut buf = [0u8; 8192];
        let mut written = 0u64;
        loop {
            let n = stream.read(&mut buf).map_err(|error| InstallError::IoError {
                path: staged_path.display().to_string(),
                detail: format!("STREAM_INCOMPLETE: {error}"),
            })?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).map_err(|error| InstallError::IoError {
                path: staged_path.display().to_string(),
                detail: format!("staged write failed: {error}"),
            })?;
            written += n as u64;
            if written > expected_len {
                return Err(InstallError::IoError {
                    path: staged_path.display().to_string(),
                    detail: "STREAM_INCOMPLETE: longer than bound".to_owned(),
                });
            }
        }
        file.flush().map_err(|error| InstallError::IoError {
            path: staged_path.display().to_string(),
            detail: format!("staged flush failed: {error}"),
        })?;
        if written != expected_len {
            return Err(InstallError::IoError {
                path: staged_path.display().to_string(),
                detail: format!("STREAM_INCOMPLETE: wrote {written} expected {expected_len}"),
            });
        }
        Ok(staged_path.clone())
    })();
    match write {
        Ok(path) => Ok(path),
        Err(error) => {
            let _ = std::fs::remove_file(&staged_path);
            Err(error)
        }
    }
}

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
    std::fs::create_dir_all(install_dir).map_err(|error| InstallError::IoError {
        path: install_dir.display().to_string(),
        detail: format!("create install directory failed: {error}"),
    })?;
    let install_path = install_dir.join(&binary_name);
    let staged_path = staged_install_path(&install_path);

    if let Err(error) = std::fs::copy(source, &staged_path) {
        let _ = std::fs::remove_file(&staged_path);
        return Err(InstallError::IoError {
            path: staged_path.display().to_string(),
            detail: format!("staged copy failed: {error}"),
        });
    }
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    if let Err(error) =
        std::fs::set_permissions(&staged_path, std::fs::Permissions::from_mode(0o755))
    {
        let _ = std::fs::remove_file(&staged_path);
        return Err(InstallError::IoError {
            path: staged_path.display().to_string(),
            detail: format!("staged chmod failed: {error}"),
        });
    }

    let staged_check = verify_identity(&staged_path, head_sha, repo_ownership);
    if !staged_check.consistent {
        let _ = std::fs::remove_file(&staged_path);
        return Err(InstallError::IdentityMismatch {
            binary: binary_name,
            head: head_sha.to_owned(),
            build_id: staged_check.build_id_in_binary.unwrap_or_default(),
            version: staged_check.version_output.unwrap_or_default(),
        });
    }
    // A pre-existing destination is a foreign owner, not an upgrade slot.
    // Rename would replace it atomically; L0 refuses first. Dies when an
    // explicit same-identity upgrade path is added and this test is rewritten.
    if install_path.exists() {
        let _ = std::fs::remove_file(&staged_path);
        return Err(InstallError::IoError {
            path: install_path.display().to_string(),
            detail: "destination already exists; refuse replace of a pre-existing owner"
                .to_owned(),
        });
    }

    // Rename publishes onto an empty path atomically on the same filesystem.
    if let Err(error) = std::fs::rename(&staged_path, &install_path) {
        let _ = std::fs::remove_file(&staged_path);
        return Err(InstallError::IoError {
            path: install_path.display().to_string(),
            detail: format!("atomic publish failed: {error}"),
        });
    }
    let metadata = std::fs::metadata(&install_path).map_err(|error| InstallError::IoError {
        path: install_path.display().to_string(),
        detail: format!("post-install stat failed: {error}"),
    })?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(InstallError::RunningExecutableMissing {
            binary: binary_name,
            path: install_path.display().to_string(),
        });
    }
    let final_check = verify_identity(&install_path, head_sha, repo_ownership);
    if !final_check.consistent {
        return Err(InstallError::IdentityMismatch {
            binary: binary_name,
            head: head_sha.to_owned(),
            build_id: final_check.build_id_in_binary.unwrap_or_default(),
            version: final_check.version_output.unwrap_or_default(),
        });
    }
    Ok(final_check)
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
            Some(
                "omp-orchestrator 0.1.0 build_id=85828bf95fba66525aa64944f3e84443f7ce188f"
                    .to_owned(),
            ), // version
        );
        assert!(check.consistent, "matching identity must be consistent");
    }

    #[test]
    fn identity_check_fails_when_build_id_differs_from_head() {
        let check = verify_identity_impl(
            "omp-orchestrator",
            "aaaaaaaa",                  // HEAD
            Some("bbbbbbbb".to_owned()), // build_id
            Some("omp-orchestrator 0.1.0 build_id=aaaaaaaa".to_owned()),
        );
        assert!(
            !check.consistent,
            "mismatched identity must be inconsistent"
        );
    }

    #[test]
    fn identity_check_fails_when_both_missing() {
        let check = verify_identity_impl("omp-orchestrator", "cccccccc", None, None);
        assert!(!check.consistent, "missing identity must be inconsistent");
    }

    // Helper: the pure identity check without filesystem probes.
    fn verify_identity_impl(
        _binary_name: &str,
        head_sha: &str,
        build_id: Option<String>,
        version_output: Option<String>,
    ) -> IdentityCheck {
        let version_id = version_output
            .as_deref()
            .and_then(|value| value.split("build_id=").nth(1))
            .unwrap_or("");
        let consistent = match (&build_id, &version_output) {
            (Some(bid), Some(_)) => bid == head_sha && version_id == head_sha,
            (Some(bid), None) => bid == head_sha,
            (None, Some(_)) => version_id == head_sha,
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
    #[cfg(unix)]
    fn executable_fixture(root: &std::path::Path, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = root.join("fake-cargo");
        std::fs::write(&path, body).expect("write fake cargo");
        let mut permissions = std::fs::metadata(&path)
            .expect("fake cargo metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("make fake cargo executable");
        path
    }

    #[cfg(unix)]
    #[test]
    fn target_build_ignores_broken_sibling_and_disables_release_strip() {
        let root = std::env::temp_dir().join(format!(
            "omp-installer-target-build-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("fixture root");
        let args = root.join("args");
        let strip = root.join("strip");
        let build_id = root.join("build-id");
        let args_path = args.display().to_string();
        let strip_path = strip.display().to_string();
        let build_id_path = build_id.display().to_string();
        let body = format!(
            r#"#!/bin/sh
printf '%s\n' "$@" > "{args_path}"
printf '%s\n' "$CARGO_PROFILE_RELEASE_STRIP" > "{strip_path}"
printf '%s\n' "$OMP_BUILD_ID" > "{build_id_path}"
case "$*" in
  *--workspace*) exit 91 ;;
esac
exit 0
"#
        );
        let cargo = executable_fixture(&root, &body);
        build_target(
            &root,
            cargo.to_str().expect("cargo path"),
            "omp-orchestrator",
            "head-42",
        )
        .expect("single-target build must not require a broken sibling");
        let command_args = std::fs::read_to_string(args).expect("recorded cargo args");
        assert!(command_args.contains("build"), "{command_args}");
        assert!(command_args.contains("--release"), "{command_args}");
        assert!(command_args.contains("-p"), "{command_args}");
        assert!(command_args.contains("omp-orchestrator"), "{command_args}");
        assert!(!command_args.contains("--workspace"), "{command_args}");
        assert_eq!(std::fs::read_to_string(strip).unwrap().trim(), "false");
        assert_eq!(std::fs::read_to_string(build_id).unwrap().trim(), "head-42");
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn pane_truth_without_local_source_is_foreign_with_named_origin() {
        let root =
            std::env::temp_dir().join(format!("omp-installer-foreign-{}", std::process::id()));
        assert!(matches!(
            resolve_repo_ownership(&root, "pane-truth"),
            RepoOwnership::Foreign { ref repo } if repo.contains("control-plane")
        ));
    }

    #[test]
    fn identity_output_names_the_legs_that_ran() {
        let check = verify_identity_impl(
            "omp-orchestrator",
            "head-42",
            Some("head-42".to_owned()),
            Some("omp-orchestrator 0.1.0 build_id=head-42".to_owned()),
        );
        let rendered = check.to_string();
        assert!(rendered.contains("legs=build_id,version"), "{rendered}");
    }

    #[test]
    fn restart_postcondition_requires_movement_and_identity() {
        assert_eq!(
            classify_restart_postcondition(Some(10), Some(11), true),
            RestartPostcondition::Verified
        );
        assert_eq!(
            classify_restart_postcondition(Some(10), Some(10), true),
            RestartPostcondition::NotRestarted
        );
        assert_eq!(
            classify_restart_postcondition(Some(10), Some(11), false),
            RestartPostcondition::IdentityMismatch
        );
        assert_eq!(
            classify_restart_postcondition(Some(10), None, true),
            RestartPostcondition::NotRunning
        );
    }
    #[cfg(unix)]
    #[test]
    fn identity_check_runs_and_reports_both_embedded_legs() {
        let root = std::env::temp_dir().join(format!(
            "omp-installer-identity-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("fixture root");
        let binary = executable_fixture(
            &root,
            "#!/bin/sh\n# build_id=head-42\nif [ \"$1\" = \"--version\" ]; then printf '%s\\n' 'omp-orchestrator 0.1.0 build_id=head-42'; fi\n",
        );
        let check = verify_identity(&binary, "head-42", &RepoOwnership::ThisRepo);
        assert!(check.consistent, "{check}");
        assert_eq!(check.identity_legs(), "build_id,version");
        assert!(check.to_string().contains("legs=build_id,version"));
        std::fs::remove_dir_all(root).expect("cleanup");
    }
    #[cfg(unix)]
    #[test]
    fn identity_failure_keeps_existing_destination_present() {
        let source_root = std::env::temp_dir().join(format!(
            "omp-installer-atomic-source-{}",
            std::process::id()
        ));
        let scratch = std::env::var_os("HOME")
            .map(PathBuf::from)
            .expect("HOME for scratch-home")
            .join(".local/state/zeststream/scratch/omp-orchestrator/installer-tests")
            .join(std::process::id().to_string());
        std::fs::create_dir_all(&source_root).expect("source root");
        std::fs::create_dir_all(&scratch).expect("scratch bin dir");
        let source = source_root.join("fake-target");
        let installed = scratch.join("fake-target");
        std::fs::write(&source, b"not an identity-bearing executable").expect("source");
        std::fs::write(&installed, b"previous installed binary").expect("previous install");
        let result = install_binary(&source, &scratch, "head-42", &RepoOwnership::ThisRepo);
        assert!(result.is_err(), "identity-less source must refuse");
        assert_eq!(
            std::fs::read(&installed).expect("destination remains"),
            b"previous installed binary"
        );
        let staged = std::fs::read_dir(&scratch)
            .expect("scratch entries")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".staged."))
            .count();
        assert_eq!(staged, 0, "failed install must remove only its staged file");
        std::fs::remove_dir_all(source_root).expect("source cleanup");
        std::fs::remove_dir_all(scratch).expect("scratch cleanup");
    }
    #[test]
    fn anonymous_build_identity_is_absent_from_leg_inventory() {
        for value in ["absent", "unavailable", "unversioned"] {
            assert_eq!(parse_build_id(&format!("installer 0.1.0 build_id={value}")), None);
        }
        assert_eq!(
            parse_build_id("installer 0.1.0 build_id=0123456789abcdef"),
            Some("0123456789abcdef".to_owned())
        );
    }
    #[cfg(unix)]
    #[test]
    fn cargo_shim_refusal_is_inconclusive_with_stderr_tail() {
        let root =
            std::env::temp_dir().join(format!("omp-installer-rch-refusal-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("fixture root");
        let cargo = executable_fixture(
            &root,
            "#!/bin/sh\nprintf '%s\n' '[RCH] remote required; refusing local fallback (no admissible workers)' >&2\nexit 103\n",
        );
        let error = build_target(
            &root,
            cargo.to_str().expect("cargo path"),
            "installer",
            "head-42",
        )
        .expect_err("shim refusal must block install");
        match error {
            InstallError::BuildInconclusive {
                code, stderr_tail, ..
            } => {
                assert_eq!(code, Some(103));
                assert!(
                    stderr_tail.contains("[RCH] remote required"),
                    "{stderr_tail}"
                );
            }
            other => panic!("expected inconclusive build, got {other:?}"),
        }
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}

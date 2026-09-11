#![forbid(unsafe_code)]

//! installer — one-touch install with four-way identity proof.
//!
//! THE DECIDING LEG: identity is PROVEN at install time, not asserted. Four-way:
//!   git rev-parse HEAD == build_id in the artifact's strings
//!   == what --version reports == what the running process reports.
//! Install FAILS if any pair disagrees.

use std::fmt;
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sha256FailureClass {
    MissingExpected,
    EmptyExpected,
    MalformedExpected,
    SourceMissing,
    ReadFailed,
    Mismatch,
}

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
    /// Typed SHA-256 verification failure. Never a warning or skip.
    /// L0-VERIFY-SIGSTORE. Cosign version floor, keyless certificate identity,
    /// or local-key policy refused the artifact.
    SigstoreRefused {
        detail: String,
    },
    Sha256Refused {
        class: Sha256FailureClass,
        detail: String,
    },
    /// Existing PATH owners of the install name. Non-interactive refuse.
    PathCollision {
        hits: Vec<String>,
    },
    /// Hook merge rolled back to timestamped backups.
    HookMergeFailed {
        backups: Vec<String>,
    },
    /// Zero detected agent families. An empty scan is ERROR, never clean.
    EmptyAgentScan,
    /// L0-REPORT missing a detected agent, digest, or identity.
    IncompleteInstallReport {
        missing: Vec<String>,
    },
    /// Unsupported host tuple for the L0 artifact resolver.
    PlatformTripleUnsupported {
        os: String,
        arch: String,
        libc: Option<String>,
    },
    /// The install path is already occupied by a runnable artifact this
    /// installer did not publish. Refused BEFORE anything is staged or renamed.
    DestinationNotOurs {
        path: String,
        detail: String,
    },
    /// L0-DURABILITY. A platform-applicable synchronization step did not
    /// complete, so the publication is not durable. The stage is carried
    /// explicitly: "it failed somewhere in publish" is not attributable.
    DurabilityRefused {
        stage: DurabilityStage,
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
            Self::SigstoreRefused { detail } => {
                write!(formatter, "L0_SIGSTORE_REFUSED: {detail}")
            }
            Self::Sha256Refused { detail, .. } => {
                write!(formatter, "L0_SHA256_REFUSED: {detail}")
            }
            Self::PathCollision { hits } => {
                write!(formatter, "L0_PATH_COLLISION: {}", hits.join(" "))
            }
            Self::HookMergeFailed { backups } => write!(
                formatter,
                "L0_HOOK_MERGE: restored from backups {}",
                backups.join(" ")
            ),
            Self::EmptyAgentScan => write!(
                formatter,
                "L0_EMPTY_SCAN: zero agents is ERROR, never a success report"
            ),
            Self::IncompleteInstallReport { missing } => write!(
                formatter,
                "L0-REPORT: incomplete; missing {}",
                missing.join(",")
            ),
            Self::PlatformTripleUnsupported { os, arch, libc } => write!(
                formatter,
                "L0-PLATFORM-TRIPLE unsupported os={os} arch={arch} libc={libc:?}"
            ),
            Self::DestinationNotOurs { path, detail } => write!(
                formatter,
                "L0_DESTINATION_NOT_OURS: refusing to replace {path}: {detail}"
            ),
            Self::DurabilityRefused { stage, detail } => write!(
                formatter,
                "L0_DURABILITY_REFUSED stage={stage}: {detail}"
            ),
        }
    }
}

/// L0-DURABILITY. Which synchronization step of a publication is being spoken
/// about. A refusal names its stage because "publish failed" is not evidence:
/// the four stages have different remedies and different blast radii.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurabilityStage {
    /// The staged artifact's own bytes reach stable storage.
    FileSync,
    /// Darwin `F_FULLFSYNC`: the drive is told to flush its write cache.
    FullFsync,
    /// The same-directory rename that publishes the destination name.
    Rename,
    /// The parent directory entry itself reaches stable storage. A renamed
    /// file whose parent was never synced can vanish on power loss WITH the
    /// old name still resolving, which is the exact hazard `install(1)` leaves
    /// open (it syncs the destination fd, never its parent).
    ParentSync,
}

impl fmt::Display for DurabilityStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::FileSync => "file_sync",
            Self::FullFsync => "fullfsync",
            Self::Rename => "rename",
            Self::ParentSync => "parent_sync",
        };
        formatter.write_str(name)
    }
}

/// L0-DURABILITY-FULLFSYNC. What this host can honestly say about
/// `F_FULLFSYNC`, which exists only on Darwin.
///
/// A non-Darwin host has no `F_FULLFSYNC` to execute, so it reports
/// `Unmeasured` with its platform and the reason. It NEVER reports a pass it
/// did not perform and never reports a failure it did not observe — a Linux
/// lane claiming either about Darwin durability is fabricating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FullFsyncObservation {
    /// Darwin: the full-flush path ran and returned success.
    Applied,
    /// Not Darwin: the step is inapplicable here and stays UNMEASURED.
    Unmeasured {
        platform: &'static str,
        reason: &'static str,
    },
}

impl fmt::Display for FullFsyncObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Applied => formatter.write_str("APPLIED"),
            Self::Unmeasured { platform, reason } => write!(
                formatter,
                "UNMEASURED platform={platform} reason={reason}"
            ),
        }
    }
}

/// L0-METRIC verdict. `Unmeasured` is a third state on purpose: a metric with
/// no observations is an ERROR to report as passing, and a zero denominator
/// must not render as either `0.0` RED or a vacuous `1.0` GREEN.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricVerdict {
    Green,
    Red,
    Unmeasured,
}

impl fmt::Display for MetricVerdict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Green => "GREEN",
            Self::Red => "RED",
            Self::Unmeasured => "UNMEASURED",
        };
        formatter.write_str(name)
    }
}

/// L0-METRIC. `L0_DURABILITY_COVERAGE = parent_fsync_successes /
/// atomic_rename_attempts`. A successful install requires `1.0`.
///
/// Both counters are incremented ONLY on their real paths: the attempt counter
/// immediately before the rename syscall (so a refused rename still counts as
/// an attempt, which is what makes a missing parent sync visible), and the
/// success counter only after the parent directory sync actually returns Ok.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DurabilityMetric {
    pub atomic_rename_attempts: u64,
    pub parent_fsync_successes: u64,
}

impl DurabilityMetric {
    /// `None` when nothing has been attempted: absent observations are
    /// UNMEASURED, never a passing zero.
    #[must_use]
    pub fn coverage(&self) -> Option<f64> {
        if self.atomic_rename_attempts == 0 {
            return None;
        }
        #[allow(clippy::cast_precision_loss)]
        Some(self.parent_fsync_successes as f64 / self.atomic_rename_attempts as f64)
    }

    #[must_use]
    pub fn verdict(&self) -> MetricVerdict {
        match self.coverage() {
            None => MetricVerdict::Unmeasured,
            Some(coverage) if coverage >= 1.0 => MetricVerdict::Green,
            Some(_) => MetricVerdict::Red,
        }
    }

    /// A parent sync cannot succeed for a rename that was never attempted.
    /// A violated invariant means the counters were incremented off their real
    /// paths and the coverage figure is not attributable.
    #[must_use]
    pub fn invariant_holds(&self) -> bool {
        self.parent_fsync_successes <= self.atomic_rename_attempts
    }
}

/// What a durable publication actually did, so a caller can assert the steps
/// rather than infer them from the absence of an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DurabilityRecord {
    pub file_synced: bool,
    pub fullfsync: FullFsyncObservation,
    pub renamed: bool,
    pub parent_synced: bool,
}


/// Canonical artifact triple selected from the running platform tuple.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformResolution {
    pub artifact_triple: &'static str,
    pub fallback: Option<&'static str>,
}

/// L0-PLATFORM-TRIPLE. Resolve the four supported OS/arch tuples and the one
/// explicit Linux-musl -> Linux-gnu artifact fallback.
pub fn resolve_platform_triple(
    os: &str,
    arch: &str,
    libc: Option<&str>,
) -> Result<PlatformResolution, InstallError> {
    let result = match (os, arch, libc) {
        ("macos", "aarch64", None) => PlatformResolution { artifact_triple: "aarch64-apple-darwin", fallback: None },
        ("macos", "x86_64", None) => PlatformResolution { artifact_triple: "x86_64-apple-darwin", fallback: None },
        ("linux", "aarch64", Some("gnu")) => PlatformResolution { artifact_triple: "aarch64-unknown-linux-gnu", fallback: None },
        ("linux", "x86_64", Some("gnu")) => PlatformResolution { artifact_triple: "x86_64-unknown-linux-gnu", fallback: None },
        ("linux", "x86_64", Some("musl")) => PlatformResolution { artifact_triple: "x86_64-unknown-linux-gnu", fallback: Some("musl-to-gnu") },
        _ => return Err(InstallError::PlatformTripleUnsupported { os: os.to_owned(), arch: arch.to_owned(), libc: libc.map(ToOwned::to_owned) }),
    };
    Ok(result)
}

/// Resolve the platform that will own the installed artifact.
pub fn current_platform_triple() -> Result<PlatformResolution, InstallError> {
    let libc = match std::env::consts::OS {
        "linux" => Some(if cfg!(target_env = "musl") { "musl" } else { "gnu" }),
        _ => None,
    };
    resolve_platform_triple(std::env::consts::OS, std::env::consts::ARCH, libc)
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

/// L0-VERIFY-SIGSTORE. The cosign release this installer refuses to trust below.
///
/// ⛔ PROVENANCE: this constant is the CONTRACT'S DECLARED FLOOR, carried from
/// `docs/contracts/s1_l0_install.md` row B05. This crate does not and cannot
/// verify the advisory itself — nothing here reaches an upstream CVE database,
/// and no measurement in this workspace establishes which cosign release fixed
/// [`COSIGN_CVE_ID`]. The floor is a POLICY INPUT that an operator or a later
/// advisory feed may raise; it is not a fact this code proves.
pub const COSIGN_CVE_FLOOR: &str = "2.4.1";

/// The advisory the floor exists for, so a refusal can be traced to its reason
/// rather than to an unexplained version number.
pub const COSIGN_CVE_ID: &str = "CVE-2026-22703";

/// A three-component cosign release, ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CosignVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl fmt::Display for CosignVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Parse `2.4.1`, `v2.4.1`, `2.4.1-rc.1`, or `2.4.1+build.7` into an ordered
/// triple.
///
/// A pre-release or build suffix is DISCARDED rather than ordered, so
/// `2.4.1-rc.1` compares EQUAL to `2.4.1`. That is the permissive direction for
/// a pre-release of the fixing release, which is why the refusal text carries
/// the caller's RAW string alongside the parsed triple: an operator reading
/// `cosign 2.4.1-rc.1` can see what was actually presented. Callers needing
/// pre-release ordering must supply a released version.
///
/// Anything else is `None`, and an unparseable version REFUSES — a version this
/// code cannot order is not a version it may approve.
#[must_use]
pub fn parse_cosign_version(raw: &str) -> Option<CosignVersion> {
    let trimmed = raw.trim();
    let trimmed = trimmed.strip_prefix('v').unwrap_or(trimmed);
    // `split` always yields at least one element, so the core is never empty
    // of a candidate; a leading `-` simply parses as an empty major and fails.
    let core = trimmed.split(['-', '+']).next().unwrap_or(trimmed);
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(CosignVersion {
        major,
        minor,
        patch,
    })
}

/// How an artifact's sigstore signature claims to be trusted.
///
/// Keyless and local-key are DISTINCT modes, not two spellings of one. An
/// artifact signed with a local key cannot satisfy a keyless identity policy by
/// presenting a matching string, and the mismatch is named as a mode mismatch
/// rather than silently compared field by field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SigstoreTrust {
    /// Keyless: an OIDC certificate identity and the issuer that minted it.
    /// The ISSUER is part of the identity: `release@example.com` from an
    /// attacker-controlled issuer is a different principal from the same string
    /// minted by the expected one.
    CertificateIdentity { identity: String, issuer: String },
    /// A configured local public key, named by its key id.
    LocalKey { key_id: String },
}

impl SigstoreTrust {
    fn mode(&self) -> &'static str {
        match self {
            Self::CertificateIdentity { .. } => "keyless",
            Self::LocalKey { .. } => "local-key",
        }
    }
}

/// What a passing sigstore verification actually established, so a caller can
/// assert the facts rather than infer them from the absence of an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigstoreVerdict {
    pub cosign_version: CosignVersion,
    pub floor: CosignVersion,
    pub trust: SigstoreTrust,
}

/// L0-VERIFY-SIGSTORE. Cosign version floor, OIDC certificate identity, and
/// local-key policy, all fail-closed.
///
/// Checks run in escalating order so the refusal names the FIRST thing wrong
/// rather than the last: a below-floor cosign is refused before its signature is
/// consulted, because a vulnerable verifier's verdict is not evidence.
///
/// NO-CLAIM: this is the POLICY, not the cryptography. `bundle_valid` is a
/// caller-supplied input describing what a real `cosign verify` returned; this
/// function does not execute cosign and does not check a signature. A caller
/// that fabricates `true` gets a pass, exactly as [`verify_minisign_policy`]'s
/// `signature_valid` does.
pub fn verify_sigstore_policy(
    cosign_version: Option<&str>,
    presented: &SigstoreTrust,
    expected: &SigstoreTrust,
    bundle_valid: bool,
) -> Result<SigstoreVerdict, InstallError> {
    let floor = parse_cosign_version(COSIGN_CVE_FLOOR)
        .expect("COSIGN_CVE_FLOOR is a compile-time constant in x.y.z form");
    let Some(raw) = cosign_version else {
        return Err(InstallError::SigstoreRefused {
            detail: format!(
                "cosign unavailable; sigstore policy requires at least {floor} for {COSIGN_CVE_ID}"
            ),
        });
    };
    let Some(version) = parse_cosign_version(raw) else {
        return Err(InstallError::SigstoreRefused {
            detail: format!(
                "unparseable cosign version {raw:?}; a version that cannot be ordered \
                 against the {COSIGN_CVE_ID} floor {floor} is never approved"
            ),
        });
    };
    if version < floor {
        return Err(InstallError::SigstoreRefused {
            detail: format!(
                "cosign {version} is below the {COSIGN_CVE_ID} floor {floor} \
                 (presented {raw:?})"
            ),
        });
    }
    if presented.mode() != expected.mode() {
        return Err(InstallError::SigstoreRefused {
            detail: format!(
                "trust mode mismatch: artifact presented {} but policy requires {}",
                presented.mode(),
                expected.mode()
            ),
        });
    }
    match (presented, expected) {
        (
            SigstoreTrust::CertificateIdentity { identity, issuer },
            SigstoreTrust::CertificateIdentity {
                identity: want_identity,
                issuer: want_issuer,
            },
        ) => {
            if identity != want_identity || issuer != want_issuer {
                return Err(InstallError::SigstoreRefused {
                    detail: format!(
                        "certificate identity mismatch: presented {identity} via {issuer}, \
                         policy requires {want_identity} via {want_issuer}"
                    ),
                });
            }
        }
        (
            SigstoreTrust::LocalKey { key_id },
            SigstoreTrust::LocalKey {
                key_id: want_key_id,
            },
        ) => {
            if key_id != want_key_id {
                return Err(InstallError::SigstoreRefused {
                    detail: format!(
                        "local key mismatch: presented {key_id}, policy requires {want_key_id}"
                    ),
                });
            }
        }
        // Unreachable while `mode()` gates the pair above, and still restrictive
        // if that guard is ever weakened: an unrecognised combination refuses.
        (presented, expected) => {
            return Err(InstallError::SigstoreRefused {
                detail: format!(
                    "unrecognised trust pairing: {} against {}",
                    presented.mode(),
                    expected.mode()
                ),
            })
        }
    }
    if !bundle_valid {
        return Err(InstallError::SigstoreRefused {
            detail: format!(
                "sigstore bundle failed verification under cosign {version} \
                 for {}",
                expected.mode()
            ),
        });
    }
    Ok(SigstoreVerdict {
        cosign_version: version,
        floor,
        trust: expected.clone(),
    })
}

/// Every PATH directory that already contains `binary_name`.
pub fn path_collision_hits(binary_name: &str, path_env: &str) -> Vec<PathBuf> {
    let mut hits = Vec::new();
    for dir in path_env.split(':') {
        if dir.is_empty() {
            continue;
        }
        let candidate = Path::new(dir).join(binary_name);
        if candidate.is_file() {
            hits.push(candidate);
        }
    }
    hits
}

/// L0-PATH-COLLISION. Lists every conflicting hit. Does not overwrite.
/// `owned_dest`, if present, is not a collision (it is the install target).
pub fn refuse_path_collisions(
    binary_name: &str,
    path_env: &str,
    owned_dest: Option<&Path>,
) -> Result<(), InstallError> {
    let hits: Vec<String> = path_collision_hits(binary_name, path_env)
        .into_iter()
        .filter(|hit| owned_dest != Some(hit.as_path()))
        .map(|hit| hit.display().to_string())
        .collect();
    if hits.is_empty() {
        Ok(())
    } else {
        Err(InstallError::PathCollision { hits })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookWrite {
    pub path: PathBuf,
    pub merged: Vec<u8>,
}

fn timestamped_backup_path(path: &Path) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("hook");
    path.with_file_name(format!("{name}.bak.{nanos}"))
}

/// L0-HOOK-MERGE. Backup every file first, then write. Injected failure after
/// `fail_after` writes restores pre-merge bytes from those backups.
pub fn merge_hooks(
    writes: &[HookWrite],
    fail_after: Option<usize>,
) -> Result<Vec<PathBuf>, InstallError> {
    let mut backups = Vec::new();
    let mut originals = Vec::new();
    for write in writes {
        let original = std::fs::read(&write.path).unwrap_or_default();
        let backup = timestamped_backup_path(&write.path);
        std::fs::write(&backup, &original).map_err(|error| InstallError::IoError {
            path: backup.display().to_string(),
            detail: format!("hook backup failed: {error}"),
        })?;
        backups.push(backup);
        originals.push(original);
    }
    for (index, write) in writes.iter().enumerate() {
        if Some(index) == fail_after {
            for (path, bytes) in writes.iter().map(|w| &w.path).zip(originals.iter()) {
                let _ = std::fs::write(path, bytes);
            }
            return Err(InstallError::HookMergeFailed {
                backups: backups
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect(),
            });
        }
        if let Err(error) = std::fs::write(&write.path, &write.merged) {
            for (path, bytes) in writes.iter().map(|w| &w.path).zip(originals.iter()) {
                let _ = std::fs::write(path, bytes);
            }
            return Err(InstallError::IoError {
                path: write.path.display().to_string(),
                detail: format!("hook write failed: {error}"),
            });
        }
    }
    Ok(backups)
}

/// Detected agent families. Empty is unrepresentable as success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentScan {
    pub families: Vec<String>,
}

/// L0-GATE / LAW-L0-OBSERVABLE-REFUSAL. Zero agents is an empty-scan ERROR,
/// never `clean`, PASS, or a zero-agent success report.
pub fn classify_agent_scan(detected: &[&str]) -> Result<AgentScan, InstallError> {
    if detected.is_empty() {
        return Err(InstallError::EmptyAgentScan);
    }
    Ok(AgentScan {
        families: detected.iter().map(|family| (*family).to_owned()).collect(),
    })
}
// ── BOUNDED SPAWNS (bead omp-orchestrator-n4q) ────────────────────────────────

/// Per-agent durable outcome. Empty family is unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentOutcome {
    pub family: String,
    pub outcome: String,
}

/// L0-REPORT. One document with agent outcomes, backups, PATH hits, digest,
/// and identity. Success is unreachable until this seals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallReport {
    pub agent_outcomes: Vec<AgentOutcome>,
    pub backups: Vec<PathBuf>,
    pub path_hits: Vec<PathBuf>,
    pub digest: String,
    pub identity: IdentityCheck,
}

impl fmt::Display for InstallReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let agents = self
            .agent_outcomes
            .iter()
            .map(|row| format!("{}={}", row.family, row.outcome))
            .collect::<Vec<_>>()
            .join(" ");
        let backups = self
            .backups
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let hits = self
            .path_hits
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(" ");
        write!(
            formatter,
            "L0-REPORT agents={agents} backups={backups} path_hits={hits} digest={} identity={} HEAD={} consistent={} legs={}",
            self.digest,
            self.identity.binary_name,
            self.identity.head_sha,
            self.identity.consistent,
            self.identity.identity_legs()
        )
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Seal the L0-REPORT. Missing a detected agent, digest, or identity is ERROR,
/// never a success. Empty detected scan is EmptyAgentScan, not a blank report.
pub fn seal_install_report(
    detected: &AgentScan,
    outcomes: Vec<AgentOutcome>,
    backups: Vec<PathBuf>,
    path_hits: Vec<PathBuf>,
    identity: IdentityCheck,
) -> Result<InstallReport, InstallError> {
    if detected.families.is_empty() {
        return Err(InstallError::EmptyAgentScan);
    }
    let mut missing = Vec::new();
    for family in &detected.families {
        if !outcomes.iter().any(|row| row.family == *family) {
            missing.push(family.clone());
        }
    }
    if identity.binary_name.trim().is_empty() || identity.head_sha.trim().is_empty() {
        missing.push("identity".to_owned());
    }
    if !missing.is_empty() {
        return Err(InstallError::IncompleteInstallReport { missing });
    }
    let canonical = format!(
        "agents={}|backups={}|hits={}|identity={}:{}",
        outcomes
            .iter()
            .map(|row| format!("{}={}", row.family, row.outcome))
            .collect::<Vec<_>>()
            .join(","),
        backups
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(","),
        path_hits
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(","),
        identity.binary_name,
        identity.head_sha
    );
    let digest = format!("{:016x}", fnv1a64(canonical.as_bytes()));
    let report = InstallReport {
        agent_outcomes: outcomes,
        backups,
        path_hits,
        digest,
        identity,
    };
    if report.digest.is_empty() {
        return Err(InstallError::IncompleteInstallReport {
            missing: vec!["digest".to_owned()],
        });
    }
    Ok(report)
}

/// Local git reads are network-free but a foreign host can still hang them
/// (credential prompt, stale lock). 30s bounds the hang without racing the
/// read.
const GIT_DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);
/// A full release build is legitimately minutes; generous but FINITE.
const BUILD_DEADLINE: std::time::Duration = std::time::Duration::from_secs(600);
/// Identity probes run a local binary; 10s is a ceiling, not a race.
const PROBE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(10);
const SHA256_BUFFER_SIZE: usize = 8192;

/// Hash a readable artifact through one fixed-size buffer.
///
/// The source path is retained for exact typed read-failure diagnostics.
fn hash_sha256_reader<R: Read>(
    source: &Path,
    mut reader: R,
) -> Result<String, InstallError> {
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; SHA256_BUFFER_SIZE];
    loop {
        let bytes_read = reader
            .read(&mut buffer)
            .map_err(|error| InstallError::Sha256Refused {
                class: Sha256FailureClass::ReadFailed,
                detail: format!("read failure path={}: {}", source.display(), error),
            })?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    let digest = hasher.finalize();
    let mut actual = String::with_capacity(64);
    for byte in digest {
        write!(&mut actual, "{byte:02x}").expect("writing a digest to String cannot fail");
    }
    Ok(actual)
}

/// Verify an artifact against an explicit SHA-256 digest before installation.
///
/// The artifact is read through one fixed-size buffer. The returned digest is
/// always the canonical lowercase 64-character hexadecimal form.
#[must_use]
pub fn verify_sha256(source: &Path, expected: Option<&str>) -> Result<String, InstallError> {
    let expected = match expected {
        None => {
            return Err(InstallError::Sha256Refused {
                class: Sha256FailureClass::MissingExpected,
                detail: "missing expected digest".to_owned(),
            })
        }
        Some(value) if value.trim().is_empty() => {
            return Err(InstallError::Sha256Refused {
                class: Sha256FailureClass::EmptyExpected,
                detail: "empty expected digest".to_owned(),
            })
        }
        Some(value) => value.trim(),
    };
    if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(InstallError::Sha256Refused {
            class: Sha256FailureClass::MalformedExpected,
            detail: format!(
                "malformed expected digest length={}; expected 64 hexadecimal characters",
                expected.len()
            ),
        });
    }
    let expected = expected.to_ascii_lowercase();
    let file = match std::fs::File::open(source) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(InstallError::Sha256Refused {
                class: Sha256FailureClass::SourceMissing,
                detail: format!("source missing path={}", source.display()),
            })
        }
        Err(error) => {
            return Err(InstallError::Sha256Refused {
                class: Sha256FailureClass::ReadFailed,
                detail: format!("read failure path={}: {}", source.display(), error),
            })
        }
    };
    let actual = hash_sha256_reader(source, file)?;
    if actual != expected {
        return Err(InstallError::Sha256Refused {
            class: Sha256FailureClass::Mismatch,
            detail: format!(
                "digest mismatch path={} expected={} actual={}",
                source.display(), expected, actual
            ),
        });
    }
    Ok(actual)
}

/// Verify an artifact before invoking the action that publishes it.
///
/// The action is never called when digest verification refuses, which gives
/// callers a testable seam for the no-write-before-verification invariant.
#[must_use]
pub fn verify_sha256_before_install<T, F>(
    source: &Path,
    expected: Option<&str>,
    install: F,
) -> Result<T, InstallError>
where
    F: FnOnce() -> Result<T, InstallError>,
{
    verify_sha256(source, expected)?;
    install()
}

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

/// What a failed identity comparison is ENTITLED to claim.
///
/// THE DEFECT THIS CLOSES. `consistent: bool` has two false values that call for
/// opposite remedies, and this check rendered both as `MISMATCH` / `IDENTITY DRIFT`:
///
///   * a leg recovered a COMMIT and it is not HEAD — the artifact is stale, so
///     rebuild and reinstall;
///   * no leg recovered a commit at all — the artifact cannot say what it was built
///     from, so there is nothing to compare and reinstalling from a source tree that
///     is equally unable to derive a commit reproduces the same state forever.
///
/// MEASURED 2026-09-11 on the installed flagship: `strings ~/.local/bin/ompo` yields
/// `build_id=nogit-1789100480…` — the UNDERIVED fallback `build.rs` stamps when the
/// object database cannot resolve HEAD, whose own NO-CLAIM says it "does not name a
/// commit" (`crates/installer/build.rs:36`) — and `ompo --version` is
/// `UAD_UNKNOWN_VERB`, so the second leg does not exist. Both legs therefore name no
/// commit, and `installer --check` reported IDENTITY DRIFT: "the subject is wrong,
/// reinstall it".
///
/// `ompo` itself already refuses this exact input correctly and says so in one
/// vocabulary: `PROVENANCE_UNSTAMPED`, *"this build cannot state its own origin;
/// staleness is UNMEASURED"*, mapped to instrument-error `3` rather than degraded `1`
/// because "degraded asserts the subject is wrong … and sending a reader to reinstall
/// something we never measured is the wrong remedy"
/// (`crates/ompo-doctor/src/provenance.rs:84-91,230-232`). Two oracles reading the
/// same unstamped binary disagreed; this is the installer adopting that vocabulary.
///
/// NO-CLAIM: `Unstamped` is still a FINDING. It keeps [`IdentityReport::drifted`]
/// true and the exit non-zero. It renames the finding; it does not forgive it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityVerdict {
    /// Every recovered leg names HEAD.
    Consistent,
    /// A leg recovered a commit-shaped identity that disagrees with HEAD.
    Mismatch,
    /// No leg recovered a commit at all, so no comparison happened.
    Unstamped,
}

impl fmt::Display for IdentityVerdict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Consistent => "IDENTITY OK",
            Self::Mismatch => "MISMATCH",
            Self::Unstamped => {
                "UNSTAMPED — this build cannot state its own origin, so identity is \
                 UNMEASURED rather than drifted and reinstalling from an equally \
                 underived source reproduces it"
            }
        };
        formatter.write_str(label)
    }
}

/// Does `token` name a git commit, as opposed to the underived `nogit-<epoch>`
/// fallback? Commit-shaped means hexadecimal and at least as wide as the shortest
/// abbreviation [`parse_build_id`] accepts, which is the same 8 characters.
#[must_use]
pub fn token_names_a_commit(token: &str) -> bool {
    let token = leg_token(token);
    token.len() >= 8 && token.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// The identity token one leg recovered.
///
/// [`verify_identity`] stores the already-parsed token, while a raw `--version` line
/// carries it after `build_id=`. Both shapes normalise here so the classifier reads
/// the same token whichever producer filled the field.
fn leg_token(raw: &str) -> &str {
    raw.rsplit_once("build_id=")
        .map_or(raw, |(_, token)| token)
        .trim()
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

    /// Classify the comparison. A disagreement is only a MISMATCH when some leg
    /// actually named a commit to disagree WITH.
    #[must_use]
    pub fn verdict(&self) -> IdentityVerdict {
        if self.consistent {
            return IdentityVerdict::Consistent;
        }
        let named_a_commit = [&self.build_id_in_binary, &self.version_output]
            .into_iter()
            .flatten()
            .any(|token| token_names_a_commit(token));
        if named_a_commit {
            IdentityVerdict::Mismatch
        } else {
            IdentityVerdict::Unstamped
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
            self.verdict()
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

// ── THE OWNED ROSTER, AND WHY IT LIVES HERE ────────────────────────────────────

/// The `(crate, installed binary)` pairs this repository owns and installs.
///
/// It lives in the LIBRARY, not in `main.rs`, because a roster no test can reach is
/// a roster that drifts unobserved.
///
/// MEASURED 2026-09-10 at HEAD 08e9bc3: the INSTALLED `installer` (build_id 2a862a2,
/// Sep 4) still carried the pair `("omp-orchestrator", "omp-orchestrator")`. The
/// supervisor cutover (07dad3f) renamed that artifact to `ompo`, and
/// `crates/omp-orchestrator` now declares exactly one bin — `omp-target-dir`. So the
/// flagship `ompo` — 7,869,104 bytes, installed, running — was NEVER PROBED, and the
/// hole in the identity proof rendered as the benign line
/// `omp-orchestrator: NOT INSTALLED (skipped)`.
///
/// A compile-time roster reports the world as of its own build, so the defence is not
/// a better list: it is [`crate_declares_bin`], which compares every entry against the
/// bin targets this workspace actually declares. A renamed artifact is then a FINDING,
/// not a skipped row.
pub const OWNED_BINARIES: &[(&str, &str)] = &[
    ("ompo-doctor", "ompo"),
    ("tick-monitor", "tick-monitor"),
    ("pane-truth", "pane-truth"),
    ("installer", "installer"),
    ("bead-availability", "bead-availability"),
    // 8nuh item 3 (owner S1L3Obs, added here because this file already had a
    // writer): an uncovered binary whose interface can drift is how the
    // supervisor's reap-abort became latent. `crate_declares_bin` resolves this
    // entry against the `[[bin]] name = "reap-finished-panes"` the crate
    // declares, so a rename becomes a FINDING rather than a skipped row.
    ("reap-finished-panes", "reap-finished-panes"),
];

/// Every bin target name `crate_name` produces in this workspace: the explicit
/// `[[bin]]` entries plus the two cargo auto-discovers — `src/main.rs` (named after
/// the package) and each `src/bin/NAME.rs`.
///
/// A crate with no readable manifest yields an EMPTY list rather than a guess, so an
/// unreadable manifest cannot be mistaken for a satisfied roster entry.
#[must_use]
pub fn declared_bin_targets(repo_root: &Path, crate_name: &str) -> Vec<String> {
    let crate_dir = repo_root.join("crates").join(crate_name);
    let Ok(manifest) = std::fs::read_to_string(crate_dir.join("Cargo.toml")) else {
        return Vec::new();
    };
    let mut names: Vec<String> = Vec::new();
    let mut package_name: Option<String> = None;
    let mut table = "";
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            table = if trimmed.starts_with("[[bin]]") {
                "bin"
            } else if trimmed.starts_with("[package]") {
                "package"
            } else {
                "other"
            };
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        if key.trim() != "name" {
            continue;
        }
        let value = value.trim().trim_matches('"').to_owned();
        match table {
            "bin" => names.push(value),
            "package" if package_name.is_none() => package_name = Some(value),
            _ => {}
        }
    }
    if crate_dir.join("src").join("main.rs").is_file() {
        if let Some(name) = package_name {
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir(crate_dir.join("src").join("bin")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|extension| extension == "rs") {
                if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                    if !names.iter().any(|name| name == stem) {
                        names.push(stem.to_owned());
                    }
                }
            }
        }
    }
    names
}

/// Does `crate_name` actually build a binary called `bin_name` in this workspace?
#[must_use]
pub fn crate_declares_bin(repo_root: &Path, crate_name: &str, bin_name: &str) -> bool {
    declared_bin_targets(repo_root, crate_name)
        .iter()
        .any(|name| name == bin_name)
}

/// One roster entry's outcome. Every entry gets a row: a roster member that produces
/// no row is invisible, which is how the flagship stayed out of the proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RosterRow {
    /// The artifact was found and its identity legs were compared.
    Probed(IdentityCheck),
    /// This workspace builds the target, and the install directory does not have it.
    /// NAMED, never probed — absence is not consistency.
    NotInstalled {
        crate_name: String,
        binary_name: String,
    },
    /// The roster names a bin target this workspace does not build. This entry can
    /// NEVER be probed, so it is a finding, not a skip.
    RosterStale {
        crate_name: String,
        binary_name: String,
    },
}

impl fmt::Display for RosterRow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Probed(check) => write!(formatter, "{check}"),
            Self::NotInstalled {
                crate_name,
                binary_name,
            } => write!(
                formatter,
                "{binary_name}: NOT INSTALLED — crate {crate_name} builds this target; \
                 nothing was compared, so this row is NOT coverage"
            ),
            Self::RosterStale {
                crate_name,
                binary_name,
            } => write!(
                formatter,
                "{binary_name}: ROSTER STALE — crate {crate_name} builds no bin named \
                 {binary_name}, so this entry can never be probed"
            ),
        }
    }
}

/// The whole-roster verdict for one install directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityReport {
    pub head_sha: String,
    pub rows: Vec<RosterRow>,
    /// Owned artifacts whose identity was actually compared. Counted, never derived
    /// from `rows.len()`: a denominator that includes rows it never examined is
    /// unverifiable.
    pub probed: usize,
    /// Artifacts whose recovered legs DISAGREE with HEAD. Rebuild-and-reinstall is
    /// the remedy.
    pub mismatches: usize,
    /// Artifacts that named no commit on any leg, so nothing was compared. Counted
    /// apart from `mismatches` because the remedy is to stamp the build, not to
    /// reinstall the same underived bytes.
    pub unstamped: usize,
    pub foreign: usize,
    pub unavailable: usize,
    pub not_installed: usize,
    pub roster_stale: usize,
}

impl IdentityReport {
    /// Drift is a disagreeing artifact, an artifact that cannot name its origin, OR a
    /// roster entry naming an artifact this workspace does not build. The third is the
    /// defect that hid `ompo`; the second is the state the flagship is actually in.
    #[must_use]
    pub fn drifted(&self) -> bool {
        self.mismatches > 0 || self.unstamped > 0 || self.roster_stale > 0
    }

    /// The process exit code this report maps to. `main` returns exactly this.
    ///
    /// The ladder is `ompo`'s own — `0` success, `1` degraded, `2` usage or safety
    /// refusal, `3` instrument error — so an UNSTAMPED-only report exits `3` rather
    /// than `1`: degraded asserts the subject is wrong, and what could not answer
    /// here is the artifact about its own origin. See
    /// `crates/ompo-doctor/src/provenance.rs:76-91`. Every non-clean report is still
    /// NON-ZERO; only the code the reader branches on changes.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        if self.mismatches > 0 || self.roster_stale > 0 {
            return 1;
        }
        if self.unstamped > 0 {
            return 3;
        }
        0
    }

    #[must_use]
    pub fn row_for(&self, binary_name: &str) -> Option<&RosterRow> {
        self.rows.iter().find(|row| match row {
            RosterRow::Probed(check) => check.binary_name == binary_name,
            RosterRow::NotInstalled { binary_name: name, .. }
            | RosterRow::RosterStale { binary_name: name, .. } => name == binary_name,
        })
    }
}

/// Sweep an install directory against the roster.
///
/// Roster integrity is checked BEFORE installation state, because an entry naming a
/// bin target this workspace does not build is stale whether or not a file of that
/// name happens to exist.
#[must_use]
pub fn sweep_installed_identity(
    repo_root: &Path,
    bin_dir: &Path,
    head_sha: &str,
    roster: &[(&str, &str)],
) -> IdentityReport {
    let mut report = IdentityReport {
        head_sha: head_sha.to_owned(),
        rows: Vec::with_capacity(roster.len()),
        probed: 0,
        mismatches: 0,
        unstamped: 0,
        foreign: 0,
        unavailable: 0,
        not_installed: 0,
        roster_stale: 0,
    };
    for &(crate_name, binary_name) in roster {
        let ownership = resolve_repo_ownership(repo_root, crate_name);
        if matches!(ownership, RepoOwnership::ThisRepo)
            && !crate_declares_bin(repo_root, crate_name, binary_name)
        {
            report.roster_stale += 1;
            report.rows.push(RosterRow::RosterStale {
                crate_name: crate_name.to_owned(),
                binary_name: binary_name.to_owned(),
            });
            continue;
        }
        let binary = bin_dir.join(binary_name);
        if !binary.exists() {
            report.not_installed += 1;
            report.rows.push(RosterRow::NotInstalled {
                crate_name: crate_name.to_owned(),
                binary_name: binary_name.to_owned(),
            });
            continue;
        }
        let check = verify_identity(&binary, head_sha, &ownership);
        match (&ownership, check.verdict()) {
            (RepoOwnership::Foreign { .. }, _) => report.foreign += 1,
            (RepoOwnership::Unknown, _) => report.unavailable += 1,
            (RepoOwnership::ThisRepo, IdentityVerdict::Consistent) => report.probed += 1,
            (RepoOwnership::ThisRepo, IdentityVerdict::Mismatch) => {
                report.probed += 1;
                report.mismatches += 1;
            }
            // PROBED, because legs were read; not a MISMATCH, because none of them
            // named a commit for HEAD to disagree with.
            (RepoOwnership::ThisRepo, IdentityVerdict::Unstamped) => {
                report.probed += 1;
                report.unstamped += 1;
            }
        }
        report.rows.push(RosterRow::Probed(check));
    }
    report
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

fn is_anonymous_sentinel(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let prefix_len = if bytes.len() >= 6
        && bytes[..6] == [b'a', b'b', b's', b'e', b'n', b't']
    {
        6
    } else if bytes.len() >= 11
        && bytes[..11] == [
            b'u', b'n', b'a', b'v', b'a', b'i', b'l', b'a', b'b', b'l', b'e',
        ]
    {
        11
    } else if bytes.len() >= 11
        && bytes[..11] == [
            b'u', b'n', b'v', b'e', b'r', b's', b'i', b'o', b'n', b'e', b'd',
        ]
    {
        11
    } else {
        return false;
    };
    bytes.len() == prefix_len
        || bytes
            .get(prefix_len)
            .is_some_and(|character| !character.is_ascii_alphanumeric())
}

/// Characters a generated non-hex build id may contain. `~`, `/` and `:` are
/// deliberately absent: they are what the NEIGHBOURING rodata starts with when
/// `strings` packs the marker against it, and admitting them is what let a whole
/// packed run masquerade as one identity token.
fn is_build_id_character(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
}

fn is_fallback_build_id(token: &str) -> bool {
    let mut characters = token.bytes();
    let Some(first) = characters.next() else {
        return false;
    };
    if !first.is_ascii_alphanumeric() {
        return false;
    }
    let mut separator = false;
    for character in characters {
        if character.is_ascii_alphanumeric() {
            continue;
        }
        if is_build_id_character(character) {
            separator = true;
            continue;
        }
        return false;
    }
    separator
}

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
            // A non-hex fallback id has no fixed width, so unlike the 40/64-hex
            // branches above nothing clipped it — and `strings` emits the marker
            // packed against the rodata that follows it
            // (`build_id=nogit-1789100969~//Users/.../crate`), which contains no
            // whitespace. Splitting on whitespace therefore returned the neighbour
            // as part of the identity, and `is_fallback_build_id` accepted it
            // because it admitted `~` and `/` as separators. Terminate the token
            // at the first character a generated id can never contain instead.
            let width = value.bytes().take_while(|byte| is_build_id_character(*byte)).count();
            &value[..width]
        };
        if token.is_empty() || is_anonymous_sentinel(token) {
            return None;
        }
        let positive = sha_len.is_some() || hex_len >= 8 || is_fallback_build_id(token);
        positive.then(|| token.to_owned())
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
        "ompo" => Some("ai.zeststream.omp-orchestrator"),
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

/// L0-ATOMIC-RENAME. Only a complete same-directory staged temp is renamed.
/// A staged file mutated after verification (length ≠ expected_len) is refused.
///
/// This is the no-injection, metric-discarding form kept for callers that only
/// care about the rename guards. It delegates to [`publish_atomic_durable`] so
/// there is exactly ONE publication path: a caller cannot reach a rename that
/// skipped the synchronization steps by picking the shorter function.
pub fn publish_atomic(
    staged: &Path,
    dest: &Path,
    expected_len: u64,
) -> Result<(), InstallError> {
    let mut metric = DurabilityMetric::default();
    publish_atomic_durable(staged, dest, expected_len, &mut metric, None).map(|_| ())
}

/// L0-ATOMIC-RENAME + L0-DURABILITY-PARENT + L0-DURABILITY-FULLFSYNC.
///
/// The publication ORDER is the property: complete staging, then file sync
/// (which on Darwin is `F_FULLFSYNC`), then rename, then parent-directory
/// sync. No `Ok` is returned before every platform-applicable step finishes,
/// so a caller cannot observe success for a publication that is not durable.
///
/// `injected_failure` is the deterministic fault seam: it makes each of the
/// four stages fail on any platform without needing a full filesystem or a
/// privileged mount. Production passes `None` and there is no production input
/// that can set it.
pub fn publish_atomic_durable(
    staged: &Path,
    dest: &Path,
    expected_len: u64,
    metric: &mut DurabilityMetric,
    injected_failure: Option<DurabilityStage>,
) -> Result<DurabilityRecord, InstallError> {
    if staged.parent() != dest.parent() {
        return Err(InstallError::IoError {
            path: dest.display().to_string(),
            detail: "ATOMIC_REFUSED: staged file is not in the destination directory".to_owned(),
        });
    }
    let staged_name = staged
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if !staged_name.contains(".staged.") {
        return Err(InstallError::IoError {
            path: staged.display().to_string(),
            detail: "ATOMIC_REFUSED: not a staged temporary".to_owned(),
        });
    }
    let meta = std::fs::metadata(staged).map_err(|error| InstallError::IoError {
        path: staged.display().to_string(),
        detail: format!("stat staged failed: {error}"),
    })?;
    if meta.len() != expected_len {
        return Err(InstallError::IoError {
            path: staged.display().to_string(),
            detail: format!(
                "ATOMIC_REFUSED: staged length {} != verified length {expected_len}",
                meta.len()
            ),
        });
    }
    if dest.exists() {
        return Err(InstallError::IoError {
            path: dest.display().to_string(),
            detail: "ATOMIC_REFUSED: destination already exists".to_owned(),
        });
    }
    // Every field below is a RETURN VALUE, never a literal: a record that
    // asserts `parent_synced: true` beside a call that may have been deleted
    // is decoration, and a test reading it would pass over the missing step.
    let (file_synced, fullfsync) = sync_staged_artifact(staged, injected_failure)?;
    let renamed = rename_for_publication(staged, dest, metric, injected_failure)?;
    let parent_synced = sync_parent_directory(dest, metric, injected_failure)?;
    Ok(DurabilityRecord {
        file_synced,
        fullfsync,
        renamed,
        parent_synced,
    })
}

/// L0-DURABILITY-FULLFSYNC. Force the staged artifact's own bytes to stable
/// storage BEFORE the rename that publishes its name.
///
/// `File::sync_all` is the safe platform adapter this step needs and the
/// reason no `unsafe` and no `libc` dependency appears in this crate: the
/// standard library implements it as `fcntl(fd, F_FULLFSYNC)` for
/// `target_vendor = "apple"` and `fsync(fd)` elsewhere — see
/// `library/std/src/sys/fs/unix.rs` `FileDesc::fsync`, lines 1266-1278 of the
/// toolchain source read on 2026-09-11. So on Darwin this call IS the
/// `F_FULLFSYNC` the contract names, and on Linux it is a plain `fsync`, which
/// is why the Darwin claim is reported UNMEASURED from a Linux lane rather
/// than asserted.
fn sync_staged_artifact(
    staged: &Path,
    injected_failure: Option<DurabilityStage>,
) -> Result<(bool, FullFsyncObservation), InstallError> {
    if injected_failure == Some(DurabilityStage::FileSync) {
        return Err(InstallError::DurabilityRefused {
            stage: DurabilityStage::FileSync,
            detail: format!("injected file sync failure path={}", staged.display()),
        });
    }
    let file = std::fs::File::open(staged).map_err(|error| InstallError::DurabilityRefused {
        stage: DurabilityStage::FileSync,
        detail: format!(
            "open staged artifact for sync failed path={}: {error}",
            staged.display()
        ),
    })?;
    file.sync_all()
        .map_err(|error| InstallError::DurabilityRefused {
            stage: DurabilityStage::FileSync,
            detail: format!("file sync failed path={}: {error}", staged.display()),
        })?;
    if injected_failure == Some(DurabilityStage::FullFsync) {
        return Err(InstallError::DurabilityRefused {
            stage: DurabilityStage::FullFsync,
            detail: format!(
                "injected F_FULLFSYNC failure path={} platform={}",
                staged.display(),
                std::env::consts::OS
            ),
        });
    }
    Ok((true, fullfsync_observation()))
}

#[cfg(target_vendor = "apple")]
fn fullfsync_observation() -> FullFsyncObservation {
    FullFsyncObservation::Applied
}

#[cfg(not(target_vendor = "apple"))]
fn fullfsync_observation() -> FullFsyncObservation {
    FullFsyncObservation::Unmeasured {
        platform: std::env::consts::OS,
        reason: "F_FULLFSYNC is a Darwin fcntl; this platform has none to execute",
    }
}

/// The rename itself. The attempt counter is incremented immediately BEFORE
/// the syscall, which is what makes a publication that renamed but never
/// synced its parent visible in the metric instead of invisible.
fn rename_for_publication(
    staged: &Path,
    dest: &Path,
    metric: &mut DurabilityMetric,
    injected_failure: Option<DurabilityStage>,
) -> Result<bool, InstallError> {
    metric.atomic_rename_attempts += 1;
    if injected_failure == Some(DurabilityStage::Rename) {
        return Err(InstallError::DurabilityRefused {
            stage: DurabilityStage::Rename,
            detail: format!("injected rename failure path={}", dest.display()),
        });
    }
    std::fs::rename(staged, dest).map_err(|error| InstallError::IoError {
        path: dest.display().to_string(),
        detail: format!("atomic publish failed: {error}"),
    })?;
    Ok(true)
}

/// L0-DURABILITY-PARENT. Sync the directory ENTRY, not the file.
///
/// A rename is only durable once the directory holding the new name is itself
/// synchronized; `install(1)` syncs the destination descriptor and never its
/// parent, which is the escape this step closes. The success counter moves
/// only after the sync actually returns Ok.
fn sync_parent_directory(
    dest: &Path,
    metric: &mut DurabilityMetric,
    injected_failure: Option<DurabilityStage>,
) -> Result<bool, InstallError> {
    let parent = dest
        .parent()
        .ok_or_else(|| InstallError::DurabilityRefused {
            stage: DurabilityStage::ParentSync,
            detail: format!(
                "destination has no parent directory to sync path={}",
                dest.display()
            ),
        })?;
    if injected_failure == Some(DurabilityStage::ParentSync) {
        return Err(InstallError::DurabilityRefused {
            stage: DurabilityStage::ParentSync,
            detail: format!(
                "injected parent directory sync failure path={}",
                parent.display()
            ),
        });
    }
    let directory =
        std::fs::File::open(parent).map_err(|error| InstallError::DurabilityRefused {
            stage: DurabilityStage::ParentSync,
            detail: format!(
                "open parent directory for sync failed path={}: {error}",
                parent.display()
            ),
        })?;
    directory
        .sync_all()
        .map_err(|error| InstallError::DurabilityRefused {
            stage: DurabilityStage::ParentSync,
            detail: format!(
                "parent directory sync failed path={}: {error}",
                parent.display()
            ),
        })?;
    metric.parent_fsync_successes += 1;
    Ok(true)
}
fn replacement_backup_path(dest: &Path) -> PathBuf {
    let name = dest
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("binary");
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    dest.with_file_name(format!(".{name}.previous.{}-{nonce}", std::process::id()))
}

/// Publish a verified artifact over an existing destination without exposing a
/// partially-written file. The old bytes are copied to a same-directory rollback
/// file before the atomic rename and retained until the final identity check.
///
/// This is the PRODUCTION publication path — `installer main` -> `run_install`
/// -> `install_binary` -> here — and it runs the same file sync, `F_FULLFSYNC`,
/// rename, parent-directory sync sequence as [`publish_atomic_durable`], with
/// the rollback snapshot taken before the first synchronization so a failure at
/// any stage leaves the prior owner recoverable.
fn replace_atomic(
    staged: &Path,
    dest: &Path,
    expected_len: u64,
    metric: &mut DurabilityMetric,
    injected_failure: Option<DurabilityStage>,
) -> Result<Option<PathBuf>, InstallError> {
    if staged.parent() != dest.parent() {
        return Err(InstallError::IoError {
            path: dest.display().to_string(),
            detail: "ATOMIC_REFUSED: staged file is not in the destination directory".to_owned(),
        });
    }
    let staged_name = staged
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if !staged_name.contains(".staged.") {
        return Err(InstallError::IoError {
            path: staged.display().to_string(),
            detail: "ATOMIC_REFUSED: not a staged temporary".to_owned(),
        });
    }
    let metadata = std::fs::metadata(staged).map_err(|error| InstallError::IoError {
        path: staged.display().to_string(),
        detail: format!("stat staged failed: {error}"),
    })?;
    if metadata.len() != expected_len {
        return Err(InstallError::IoError {
            path: staged.display().to_string(),
            detail: format!(
                "ATOMIC_REFUSED: staged length {} != verified length {expected_len}",
                metadata.len()
            ),
        });
    }

    let rollback = if dest.exists() {
        let rollback = replacement_backup_path(dest);
        std::fs::copy(dest, &rollback).map_err(|error| InstallError::IoError {
            path: rollback.display().to_string(),
            detail: format!("rollback snapshot failed: {error}"),
        })?;
        Some(rollback)
    } else {
        None
    };
    let discard_rollback = |rollback: &Option<PathBuf>| {
        if let Some(rollback) = rollback {
            let _ = std::fs::remove_file(rollback);
        }
    };
    if let Err(error) = sync_staged_artifact(staged, injected_failure) {
        discard_rollback(&rollback);
        return Err(error);
    }
    if let Err(error) = rename_for_publication(staged, dest, metric, injected_failure) {
        discard_rollback(&rollback);
        return Err(error);
    }
    if let Err(error) = sync_parent_directory(dest, metric, injected_failure) {
        // The rename already happened, so the destination now holds the new
        // bytes but is not durable. Put the prior owner back rather than
        // leaving a non-durable publication behind under a success-shaped path.
        if let Some(rollback) = &rollback {
            restore_atomic(rollback, dest)?;
        }
        return Err(error);
    }
    Ok(rollback)
}

fn restore_atomic(rollback: &Path, dest: &Path) -> Result<(), InstallError> {
    std::fs::rename(rollback, dest).map_err(|error| InstallError::IoError {
        path: dest.display().to_string(),
        detail: format!("rollback restore failed: {error}"),
    })
}

/// L0-REFUSE-BEFORE-REPLACE. Refuse an install path already owned by a runnable
/// artifact this installer did not publish.
///
/// The ORDERING is the property, not the predicate: this runs before anything is
/// staged, which is strictly earlier than the `replace_atomic` rename it guards,
/// so a foreign owner is never even momentarily at risk and no staged temp is
/// written beside it. Before this existed `install_binary` staged, verified the
/// SOURCE, and then renamed over whatever sat at the destination — so a
/// same-identity source silently clobbered a stranger's binary and the rollback
/// copy was deleted on success, which is how `/usr/sbin/installer` would have
/// been overwritten by a plain `installer` install.
///
/// OWNERSHIP DISCRIMINATOR: every binary this workspace publishes carries
/// `#[used] BUILD_ID_MARKER` (see `src/main.rs`), so a recoverable `build_id=`
/// token in the occupant's own bytes means it is a prior artifact of ours and
/// replacing it is an upgrade rather than a clobber. The probe reads bytes and
/// never EXECUTES the occupant: an unowned binary sitting at our install path is
/// exactly the thing not to run.
///
/// A non-runnable occupant is not a PATH owner — no shell can invoke it — so it
/// is debris from an interrupted install and replacing it clobbers nobody.
///
/// NO-CLAIM: an artifact of ours built without a derivable identity stamps the
/// anonymous sentinel, which `parse_build_id` refuses, so it reads as foreign and
/// its own upgrade is refused. That is fail-closed on purpose: a binary that
/// cannot say what it was built from cannot prove it is ours.
fn refuse_foreign_destination(dest: &Path) -> Result<(), InstallError> {
    let metadata = match std::fs::symlink_metadata(dest) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(InstallError::IoError {
                path: dest.display().to_string(),
                detail: format!("stat destination failed: {error}"),
            })
        }
    };
    if !destination_is_runnable(&metadata) {
        return Ok(());
    }
    if probe_build_id_string(dest).is_some() {
        return Ok(());
    }
    Err(InstallError::DestinationNotOurs {
        path: dest.display().to_string(),
        detail: "a runnable artifact carrying no build_id marker already owns this path"
            .to_owned(),
    })
}

#[cfg(unix)]
fn destination_is_runnable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.file_type().is_symlink()
        || (metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn destination_is_runnable(metadata: &std::fs::Metadata) -> bool {
    !metadata.is_dir()
}

/// Install one verified artifact. Durability observations are discarded; use
/// [`install_binary_with_durability`] when the caller must READ what the
/// publication actually synchronized.
pub fn install_binary(
    source: &Path,
    install_dir: &Path,
    head_sha: &str,
    repo_ownership: &RepoOwnership,
) -> Result<IdentityCheck, InstallError> {
    let mut metric = DurabilityMetric::default();
    install_binary_with_durability(
        source,
        install_dir,
        head_sha,
        repo_ownership,
        &mut metric,
        None,
    )
}

/// The production install path with its durability seam exposed.
///
/// `metric` accumulates L0-METRIC across calls, and `injected_failure` is the
/// deterministic fault seam for the four durability stages. Production supplies
/// `None`; nothing reachable from the CLI can set it.
pub fn install_binary_with_durability(
    source: &Path,
    install_dir: &Path,
    head_sha: &str,
    repo_ownership: &RepoOwnership,
    metric: &mut DurabilityMetric,
    injected_failure: Option<DurabilityStage>,
) -> Result<IdentityCheck, InstallError> {
    let binary_name = source
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| InstallError::IoError {
            path: source.display().to_string(),
            detail: "binary has no filename".to_owned(),
        })?
        .to_owned();
    let source_file = std::fs::File::open(source).map_err(|error| InstallError::IoError {
        path: source.display().to_string(),
        detail: format!("open source artifact failed: {error}"),
    })?;
    let expected_len = source_file
        .metadata()
        .map_err(|error| InstallError::IoError {
            path: source.display().to_string(),
            detail: format!("stat source artifact failed: {error}"),
        })?
        .len();
    let install_path = install_dir.join(&binary_name);
    refuse_foreign_destination(&install_path)?;
    let staged_path = stage_artifact_stream(install_dir, &binary_name, source_file, expected_len)?;
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

    let rollback = replace_atomic(
        &staged_path,
        &install_path,
        expected_len,
        metric,
        injected_failure,
    )?;
    let final_check = verify_identity(&install_path, head_sha, repo_ownership);
    if !final_check.consistent {
        if let Some(rollback) = rollback {
            restore_atomic(&rollback, &install_path)?;
        }
        return Err(InstallError::IdentityMismatch {
            binary: binary_name,
            head: head_sha.to_owned(),
            build_id: final_check.build_id_in_binary.unwrap_or_default(),
            version: final_check.version_output.unwrap_or_default(),
        });
    }
    if let Some(rollback) = rollback {
        std::fs::remove_file(&rollback).map_err(|error| InstallError::IoError {
            path: rollback.display().to_string(),
            detail: format!("rollback cleanup failed: {error}"),
        })?;
    }
    Ok(final_check)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_check_consistent_when_build_id_matches_head() {
        let check = verify_identity_impl(
            "ompo",
            "85828bf95fba66525aa64944f3e84443f7ce188f", // HEAD
            Some("85828bf95fba66525aa64944f3e84443f7ce188f".to_owned()), // build_id
            Some(
                "ompo supervise 0.1.0 build_id=85828bf95fba66525aa64944f3e84443f7ce188f"
                    .to_owned(),
            ), // version
        );
        assert!(check.consistent, "matching identity must be consistent");
    }

    #[test]
    fn identity_check_fails_when_build_id_differs_from_head() {
        let check = verify_identity_impl(
            "ompo",
            "aaaaaaaa",                  // HEAD
            Some("bbbbbbbb".to_owned()), // build_id
            Some("ompo supervise 0.1.0 build_id=aaaaaaaa".to_owned()),
        );
        assert!(
            !check.consistent,
            "mismatched identity must be inconsistent"
        );
    }

    #[test]
    fn identity_check_fails_when_both_missing() {
        let check = verify_identity_impl("ompo", "cccccccc", None, None);
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
            "ompo-doctor",
            "head-42",
        )
        .expect("single-target build must not require a broken sibling");
        let command_args = std::fs::read_to_string(args).expect("recorded cargo args");
        assert!(command_args.contains("build"), "{command_args}");
        assert!(command_args.contains("--release"), "{command_args}");
        assert!(command_args.contains("-p"), "{command_args}");
        assert!(command_args.contains("ompo-doctor"), "{command_args}");
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
            "ompo",
            "head-42",
            Some("head-42".to_owned()),
            Some("ompo supervise 0.1.0 build_id=head-42".to_owned()),
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
            "#!/bin/sh\n# build_id=head-42\nif [ \"$1\" = \"--version\" ]; then printf '%s\\n' 'ompo supervise 0.1.0 build_id=head-42'; fi\n",
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
    #[cfg(unix)]
    #[test]
    fn existing_destination_is_replaced_atomically_after_identity_check() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "omp-installer-replace-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("fixture root");
        let source = root.join("ompo");
        let install_dir = root.join("bin");
        std::fs::create_dir_all(&install_dir).expect("install directory");
        std::fs::write(
            &source,
            b"#!/bin/sh\n# build_id=head-42\nif [ \"$1\" = \"--version\" ]; then printf '%s\\n' 'ompo supervise 0.1.0 build_id=head-42'; fi\n",
        )
        .expect("source artifact");
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755))
            .expect("source permissions");
        let destination = install_dir.join("ompo");
        std::fs::write(&destination, b"previous artifact").expect("previous destination");

        let check = install_binary(&source, &install_dir, "head-42", &RepoOwnership::ThisRepo)
            .expect("same-identity replacement must succeed");
        assert!(check.consistent, "{check}");
        assert_ne!(
            std::fs::read(&destination).expect("installed artifact"),
            b"previous artifact"
        );
        assert!(
            std::fs::read_to_string(&destination)
                .expect("installed artifact text")
                .contains("build_id=head-42"),
            "new artifact must be published"
        );
        let rollback_count = std::fs::read_dir(&install_dir)
            .expect("install directory entries")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".previous."))
            .count();
        assert_eq!(rollback_count, 0, "successful replacement cleans its rollback file");
        std::fs::remove_dir_all(root).expect("cleanup");
    }
    /// The refuse-before-replace guard must not become an upgrade blocker: a
    /// destination that carries our own `build_id=` marker is a prior artifact of
    /// ours, and installing over it is the normal upgrade path.
    #[cfg(unix)]
    #[test]
    fn owned_executable_destination_is_still_upgraded() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "omp-installer-upgrade-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let install_dir = root.join("bin");
        std::fs::create_dir_all(&install_dir).expect("install directory");
        let source = root.join("ompo");
        std::fs::write(
            &source,
            b"#!/bin/sh\n# build_id=head-42\nif [ \"$1\" = \"--version\" ]; then printf '%s\\n' 'ompo supervise 0.1.0 build_id=head-42'; fi\n",
        )
        .expect("source artifact");
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755))
            .expect("source permissions");

        // A prior artifact OF OURS: executable, and carrying the marker.
        let destination = install_dir.join("ompo");
        std::fs::write(
            &destination,
            b"#!/bin/sh\n# build_id=head-41\nif [ \"$1\" = \"--version\" ]; then printf '%s\\n' 'ompo supervise 0.1.0 build_id=head-41'; fi\n",
        )
        .expect("previous owned artifact");
        std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(0o755))
            .expect("previous artifact permissions");

        let check = install_binary(&source, &install_dir, "head-42", &RepoOwnership::ThisRepo)
            .expect("an owned marker-bearing destination must still upgrade");
        assert!(check.consistent, "{check}");
        assert!(
            std::fs::read_to_string(&destination)
                .expect("installed artifact text")
                .contains("build_id=head-42"),
            "the upgrade must publish the new artifact"
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    /// The same directory, the same install name, an occupant that is NOT ours:
    /// refused, and refused before anything is staged.
    #[cfg(unix)]
    #[test]
    fn foreign_executable_destination_is_refused_before_staging() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "omp-installer-foreign-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let install_dir = root.join("bin");
        std::fs::create_dir_all(&install_dir).expect("install directory");
        let source = root.join("ompo");
        std::fs::write(
            &source,
            b"#!/bin/sh\n# build_id=head-42\nif [ \"$1\" = \"--version\" ]; then printf '%s\\n' 'ompo supervise 0.1.0 build_id=head-42'; fi\n",
        )
        .expect("source artifact");
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755))
            .expect("source permissions");

        let destination = install_dir.join("ompo");
        std::fs::write(&destination, b"#!/bin/sh\nexit 0\n").expect("foreign owner");
        std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(0o755))
            .expect("foreign owner permissions");

        let error = install_binary(&source, &install_dir, "head-42", &RepoOwnership::ThisRepo)
            .expect_err("a foreign runnable owner must refuse");
        match &error {
            InstallError::DestinationNotOurs { path, .. } => {
                assert_eq!(path, &destination.display().to_string());
            }
            other => panic!("expected DestinationNotOurs, got {other:?}"),
        }
        assert!(error.to_string().starts_with("L0_DESTINATION_NOT_OURS"), "{error}");
        assert_eq!(
            std::fs::read(&destination).expect("foreign owner remains"),
            b"#!/bin/sh\nexit 0\n"
        );
        let entries: Vec<_> = std::fs::read_dir(&install_dir)
            .expect("install directory entries")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec!["ompo".to_owned()], "nothing may be staged: {entries:?}");
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn anonymous_build_identity_is_absent_from_leg_inventory() {
        for value in ["absent", "unavailable", "unversioned"] {
            assert_eq!(parse_build_id(&format!("installer 0.1.0 build_id={value}")), None);
            assert_eq!(parse_build_id(&format!("installer 0.1.0 build_id={value}~/")), None);
        }
        assert_eq!(
            parse_build_id("build_id=absentunavailableunversionedmarker"),
            None,
            "packed sentinel literals must not become an identity token"
        );
        assert_eq!(
            parse_build_id("installer 0.1.0 build_id=head-42"),
            Some("head-42".to_owned())
        );
        assert_eq!(
            parse_build_id("installer 0.1.0 build_id=0123456789abcdef"),
            Some("0123456789abcdef".to_owned())
        );
        let hex40 = "0123456789abcdef0123456789abcdef01234567";
        let hex64 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert_eq!(parse_build_id(&format!("build_id={hex40}~/")), Some(hex40.to_owned()));
        assert_eq!(parse_build_id(&format!("build_id={hex64}~/")), Some(hex64.to_owned()));
    }

    /// A 40/64-hex id is clipped by width, so the packed-rodata neighbour never
    /// reached it. A generated fallback id has no width to clip by, and the
    /// neighbour rode in with it.
    #[test]
    fn packed_fallback_marker_clips_at_the_neighbouring_rodata() {
        // Shape observed from `strings` on the real artifact: the marker, then the
        // rodata neighbour, with no separator either can see.
        assert_eq!(
            parse_build_id("build_id=nogit-1789100969~//build/root/crates/installercrate"),
            Some("nogit-1789100969".to_owned()),
            "a packed fallback marker must yield the id, not the id plus its neighbour"
        );
        assert_eq!(
            parse_build_id("build_id=v1.2.3-rc1/usr/lib"),
            Some("v1.2.3-rc1".to_owned()),
            "a deliberate release stamp is a fallback id too"
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


#[cfg(test)]
#[path = "sha256_tests.rs"]
mod sha256_tests;

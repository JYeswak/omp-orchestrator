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
use std::process::{Command, ExitCode};
use sha2::{Digest, Sha256};
use lifecycle_event::{default_repo_journal, Layer};
use lifecycle_monitor::{
    gate_freshness_verdict, measure_metric, measure_ratio_metric, observe_layer, verify_artifact,
    MetricDelta, MetricDeltaVerdict, MetricDirection, MetricExpectation, MetricMeasureError,
    MetricThreshold,
};
/// Typed reasons the canonical install report cannot be accepted by the
/// production observability flow. Each state has a distinct variant and
/// stable reason code; callers never infer the cause from a generic I/O error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallReportCause {
    ManifestPartial {
        bound_kind: String,
        bound_value: usize,
        source: String,
    },
    ManifestRefused {
        reason: String,
    },
    WriterSuppressed {
        path: PathBuf,
    },
    Missing {
        path: PathBuf,
    },
    DeletedAfterWrite {
        path: PathBuf,
    },
    ZeroBytes {
        path: PathBuf,
    },
    Truncated {
        path: PathBuf,
        detail: String,
    },
    ReadbackFailed {
        path: PathBuf,
        detail: String,
    },
    ReadbackMismatch {
        path: PathBuf,
        detail: String,
    },
    MissingField {
        path: PathBuf,
        field: String,
    },
    WrongType {
        path: PathBuf,
        field: String,
        expected: &'static str,
        found: &'static str,
    },
    EmptyField {
        path: PathBuf,
        field: String,
    },
    IdentityMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    InceptionMissing {
        path: PathBuf,
    },
    HostCapabilitiesInvalid {
        path: PathBuf,
        detail: String,
    },
    MetricUnmeasurable {
        path: PathBuf,
        detail: String,
    },
}

impl fmt::Display for InstallReportCause {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManifestPartial {
                bound_kind,
                bound_value,
                source,
            } => write!(
                formatter,
                "L0_REPORT_MANIFEST_PARTIAL bound={bound_kind} value={bound_value} source={source}"
            ),
            Self::ManifestRefused { reason } => {
                write!(formatter, "L0_REPORT_MANIFEST_REFUSED reason={reason}")
            }
            Self::WriterSuppressed { path } => write!(
                formatter,
                "L0_REPORT_WRITE_SUPPRESSED path={}",
                path.display()
            ),
            Self::Missing { path } => {
                write!(formatter, "L0_REPORT_MISSING path={}", path.display())
            }
            Self::DeletedAfterWrite { path } => write!(
                formatter,
                "L0_REPORT_DELETED_AFTER_WRITE path={}",
                path.display()
            ),
            Self::ZeroBytes { path } => {
                write!(formatter, "L0_REPORT_ZERO_BYTES path={}", path.display())
            }
            Self::Truncated { path, detail } => write!(
                formatter,
                "L0_REPORT_TRUNCATED path={} detail={detail}",
                path.display()
            ),
            Self::ReadbackFailed { path, detail } => write!(
                formatter,
                "L0_REPORT_READBACK_FAILED path={} detail={detail}",
                path.display()
            ),
            Self::ReadbackMismatch { path, detail } => write!(
                formatter,
                "L0_REPORT_READBACK_MISMATCH path={} detail={detail}",
                path.display()
            ),
            Self::MissingField { path, field } => write!(
                formatter,
                "L0_REPORT_FIELD_MISSING path={} field={field}",
                path.display()
            ),
            Self::WrongType {
                path,
                field,
                expected,
                found,
            } => write!(
                formatter,
                "L0_REPORT_FIELD_TYPE path={} field={field} expected={expected} found={found}",
                path.display()
            ),
            Self::EmptyField { path, field } => write!(
                formatter,
                "L0_REPORT_FIELD_EMPTY path={} field={field}",
                path.display()
            ),
            Self::IdentityMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "L0_REPORT_IDENTITY_MISMATCH field={field} expected={expected:?} actual={actual:?}"
            ),
            Self::InceptionMissing { path } => write!(
                formatter,
                "L0_REPORT_INCEPTION_MISSING path={}",
                path.display()
            ),
            Self::HostCapabilitiesInvalid { path, detail } => write!(
                formatter,
                "L0_REPORT_HOST_CAPABILITIES_INVALID path={} detail={detail}",
                path.display()
            ),
            Self::MetricUnmeasurable { path, detail } => write!(
                formatter,
                "L0_REPORT_METRIC_UNMEASURABLE path={} detail={detail}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for InstallReportCause {}

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
    /// Empty hook write set. Merging nothing is ERROR, never a vacuous
    /// backup set.
    EmptyHookWrites,
    /// Zero detected agent families. An empty scan is ERROR, never clean.
    EmptyAgentScan,
    /// L0-REPORT missing a detected agent, digest, or identity.
    IncompleteInstallReport {
        missing: Vec<String>,
    },
    /// Canonical report persistence or readback refused with a typed cause.
    InstallReportRefused(InstallReportCause),
    /// L0-B11 skill installation reached no all-agent success. Per-family
    /// outcomes travel with the refusal so partial progress survives it;
    /// seal-level incompleteness stays IncompleteInstallReport.
    SkillInstallFailed {
        reason: String,
        outcomes: Vec<AgentOutcome>,
    },
    /// L0-B12: a PARTIAL input manifest cannot seal -- the bound it names
    /// is not the full input set.
    PartialInputManifest {
        bound_kind: String,
        bound_value: usize,
        source: String,
    },
    /// L0-B12: a REFUSED input manifest cannot seal -- the named reason
    /// withheld an input.
    RefusedInputManifest { reason: String },
    /// Unsupported host tuple for the L0 artifact resolver.
    PlatformTripleUnsupported {
        os: String,
        arch: String,
        libc: Option<String>,
    },
    /// L0-PLATFORM-TRIPLE composition. The resolved artifact is absent from
    /// the catalog, built for another arch/triple, unreadable, or empty.
    ArtifactUnavailable {
        triple: String,
        detail: String,
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
    /// qod0: the metric record or delta cannot be cited as numeric.
    InstallMetricUnmeasurable(InstallMetricUnmeasurable),
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
            Self::EmptyHookWrites => write!(
                formatter,
                "L0_HOOK_MERGE_EMPTY: zero hook writes is ERROR, never a vacuous backup set"
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
            Self::InstallReportRefused(cause) => write!(formatter, "{cause}"),
            Self::SkillInstallFailed { reason, outcomes } => write!(
                formatter,
                "L0_SKILLS_FAILED {reason} outcomes={}",
                outcomes
                    .iter()
                    .map(|row| format!("{}={}", row.family, row.outcome))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Self::PlatformTripleUnsupported { os, arch, libc } => write!(
                formatter,
                "L0-PLATFORM-TRIPLE unsupported os={os} arch={arch} libc={libc:?}"
            ),
            Self::ArtifactUnavailable { triple, detail } => write!(
                formatter,
                "L0-PLATFORM-TRIPLE artifact unavailable for {triple}: {detail}"
            ),
            Self::PartialInputManifest {
                bound_kind,
                bound_value,
                source,
            } => write!(
                formatter,
                "L0_INPUT_MANIFEST_PARTIAL bound={bound_kind} value={bound_value} source={source}: partial input cannot seal"
            ),
            Self::RefusedInputManifest { reason } => write!(
                formatter,
                "L0_INPUT_MANIFEST_REFUSED reason={reason}: refused input cannot seal"
            ),
            Self::DestinationNotOurs { path, detail } => write!(
                formatter,
                "L0_DESTINATION_NOT_OURS: refusing to replace {path}: {detail}"
            ),
            Self::DurabilityRefused { stage, detail } => write!(
                formatter,
                "L0_DURABILITY_REFUSED stage={stage}: {detail}"
            ),
            Self::InstallMetricUnmeasurable(error) => write!(formatter, "{error}"),
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

pub const DURABILITY_COVERAGE_SCALE: u64 = 1_000_000;
pub const DURABILITY_COVERAGE_EXPECTATION: MetricExpectation = MetricExpectation {
    metric: "L0_DURABILITY_COVERAGE",
    unit: "ppm",
    expected: DURABILITY_COVERAGE_SCALE,
};

/// Materialize the durability ratio for the persisted report and lifecycle event.
/// Empty scope and impossible counters are typed errors, never passing ratios.
pub fn materialize_durability_metric(
    metric: DurabilityMetric,
) -> Result<MetricDelta, InstallMetricUnmeasurable> {
    if !metric.invariant_holds() {
        return Err(InstallMetricUnmeasurable::DurabilityInvariant {
            parent_fsync_successes: metric.parent_fsync_successes,
            atomic_rename_attempts: metric.atomic_rename_attempts,
        });
    }
    measure_ratio_metric(
        DURABILITY_COVERAGE_EXPECTATION,
        Some(MetricThreshold {
            tolerance: 0,
            direction: MetricDirection::AtLeast,
        }),
        metric.parent_fsync_successes,
        metric.atomic_rename_attempts,
        DURABILITY_COVERAGE_SCALE,
    )
    .map_err(Into::into)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DurabilityRecord {
    pub file_synced: bool,
    pub fullfsync: FullFsyncObservation,
    pub renamed: bool,
    pub parent_synced: bool,
}


pub const INSTALL_DURATION_EXPECTATION_MS: u64 = 30_000;
pub const INSTALL_DURATION_THRESHOLD_MS: u64 = 5_000;
pub const INSTALL_PATH_HITS_EXPECTED: u64 = 1;
pub const INSTALL_BACKUP_RATIO_SCALE: u64 = 1_000_000;

pub const INSTALL_DURATION_EXPECTATION: MetricExpectation = MetricExpectation {
    metric: "install_to_verified_path_ms",
    unit: "ms",
    expected: INSTALL_DURATION_EXPECTATION_MS,
};
pub const INSTALL_PATH_HITS_EXPECTATION: MetricExpectation = MetricExpectation {
    metric: "path_hits",
    unit: "count",
    expected: INSTALL_PATH_HITS_EXPECTED,
};
pub const INSTALL_BACKUP_RATIO_EXPECTATION: MetricExpectation = MetricExpectation {
    metric: "backups_written_per_file_mutated",
    unit: "ppm",
    expected: INSTALL_BACKUP_RATIO_SCALE,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallMetricThresholds {
    pub duration: Option<MetricThreshold>,
    pub path_hits: Option<MetricThreshold>,
    pub backup_ratio: Option<MetricThreshold>,
}

impl InstallMetricThresholds {
    #[must_use]
    pub const fn production() -> Self {
        Self {
            duration: Some(MetricThreshold {
                tolerance: INSTALL_DURATION_THRESHOLD_MS,
                direction: MetricDirection::AtMost,
            }),
            path_hits: Some(MetricThreshold {
                tolerance: 0,
                direction: MetricDirection::AtMost,
            }),
            backup_ratio: Some(MetricThreshold {
                tolerance: 0,
                direction: MetricDirection::AtLeast,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallMetricInputs {
    pub started_at_ms: Option<u64>,
    pub verified_path_at_ms: Option<u64>,
    pub path_hits: usize,
    pub backups_written: usize,
    pub files_mutated: usize,
    pub durability_metric: DurabilityMetric,
    pub thresholds: InstallMetricThresholds,
}

impl InstallMetricInputs {
    #[must_use]
    pub fn production(
        started_at_ms: Option<u64>,
        verified_path_at_ms: Option<u64>,
        path_hits: usize,
        backups_written: usize,
        outcomes: &[AgentOutcome],
        durability_metric: DurabilityMetric,
    ) -> Self {
        Self {
            started_at_ms,
            verified_path_at_ms,
            path_hits,
            backups_written,
            files_mutated: outcomes
                .iter()
                .filter(|row| row.outcome == "merged")
                .count(),
            durability_metric,
            thresholds: InstallMetricThresholds::production(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallMetrics {
    pub writer: &'static str,
    pub started_at_ms: u64,
    pub verified_path_at_ms: u64,
    pub observed_at_ms: u64,
    pub freshness_threshold_ms: u64,
    pub attempt_identity: AttemptIdentity,
    pub manifest_digest: String,
    pub deltas: [MetricDelta; 3],
    pub record_digest: String,
}

impl InstallMetrics {
    #[must_use]
    pub fn overall_verdict(&self) -> MetricDeltaVerdict {
        if self
            .deltas
            .iter()
            .any(|row| row.verdict == MetricDeltaVerdict::Red)
        {
            MetricDeltaVerdict::Red
        } else {
            MetricDeltaVerdict::Pass
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallMetricUnmeasurable {
    MissingHome,
    DurabilityMetricMissing,
    DurabilityInvariant {
        parent_fsync_successes: u64,
        atomic_rename_attempts: u64,
    },
    WriterAbsent { path: PathBuf },
    InputNotFull { state: String },
    TimestampMissing { field: &'static str },
    TimestampReversed { earlier: u64, later: u64 },
    AttemptIdentityMissing,
    StaleReport { age_ms: u64, threshold_ms: u64 },
    ReadbackFailed { path: PathBuf, detail: String },
    Evaluation(MetricMeasureError),
}

impl fmt::Display for InstallMetricUnmeasurable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHome => f.write_str(
                "INSTALL_METRIC_UNMEASURABLE reason=MISSING_METRIC_HOME",
            ),
            Self::DurabilityMetricMissing => f.write_str(
                "INSTALL_METRIC_UNMEASURABLE reason=MISSING_DURABILITY_METRIC_HOME",
            ),
            Self::DurabilityInvariant {
                parent_fsync_successes,
                atomic_rename_attempts,
            } => write!(
                f,
                "INSTALL_METRIC_UNMEASURABLE reason=DURABILITY_INVARIANT parent_fsync_successes={parent_fsync_successes} atomic_rename_attempts={atomic_rename_attempts}"
            ),
            Self::WriterAbsent { path } => write!(
                f,
                "INSTALL_METRIC_UNMEASURABLE reason=WRITER_ABSENT path={}",
                path.display()
            ),
            Self::InputNotFull { state } => write!(
                f,
                "INSTALL_METRIC_UNMEASURABLE reason=INPUT_NOT_FULL state={state}"
            ),
            Self::TimestampMissing { field } => write!(
                f,
                "INSTALL_METRIC_UNMEASURABLE reason=TIMESTAMP_MISSING field={field}"
            ),
            Self::TimestampReversed { earlier, later } => write!(
                f,
                "INSTALL_METRIC_UNMEASURABLE reason=TIMESTAMP_REVERSED earlier={earlier} later={later}"
            ),
            Self::AttemptIdentityMissing => f.write_str(
                "INSTALL_METRIC_UNMEASURABLE reason=ATTEMPT_IDENTITY_MISSING",
            ),
            Self::StaleReport {
                age_ms,
                threshold_ms,
            } => write!(
                f,
                "INSTALL_METRIC_UNMEASURABLE reason=STALE_REPORT age_ms={age_ms} threshold_ms={threshold_ms}"
            ),
            Self::ReadbackFailed { path, detail } => write!(
                f,
                "INSTALL_METRIC_UNMEASURABLE reason=READBACK_FAILED path={} detail={detail}",
                path.display()
            ),
            Self::Evaluation(error) => {
                write!(f, "INSTALL_METRIC_UNMEASURABLE reason=EVALUATION {error}")
            }
        }
    }
}

impl std::error::Error for InstallMetricUnmeasurable {}

impl From<MetricMeasureError> for InstallMetricUnmeasurable {
    fn from(error: MetricMeasureError) -> Self {
        Self::Evaluation(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallMetricDeltaReport {
    pub metrics: InstallMetrics,
    pub durability_metric: MetricDelta,
    pub readback_bytes: usize,
    pub age_ms: u64,
    pub fresh: bool,
}

impl InstallMetricDeltaReport {
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        if self.metrics.overall_verdict() == MetricDeltaVerdict::Red
            || self.durability_metric.verdict == MetricDeltaVerdict::Red
        {
            1
        } else {
            0
        }
    }
}

#[must_use]
pub fn current_time_ms() -> Option<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
}
fn count_to_u64(
    metric: &'static str,
    value: usize,
) -> Result<u64, InstallMetricUnmeasurable> {
    u64::try_from(value).map_err(|_| {
        MetricMeasureError::Overflow {
            metric,
            operation: "usize_to_u64",
        }
        .into()
    })
}

pub fn materialize_install_metrics(
    inputs: InstallMetricInputs,
    identity: &AttemptIdentity,
    manifest: &InputManifest,
) -> Result<InstallMetrics, InstallMetricUnmeasurable> {
    let manifest_digest = match manifest {
        InputManifest::Full { digest } if !digest.trim().is_empty() => digest.clone(),
        other => {
            return Err(InstallMetricUnmeasurable::InputNotFull {
                state: other.to_string(),
            });
        }
    };
    if identity.attempt.trim().is_empty() {
        return Err(InstallMetricUnmeasurable::AttemptIdentityMissing);
    }
    let started_at_ms = inputs.started_at_ms.ok_or(
        InstallMetricUnmeasurable::TimestampMissing {
            field: "started_at_ms",
        },
    )?;
    let verified_path_at_ms = inputs.verified_path_at_ms.ok_or(
        InstallMetricUnmeasurable::TimestampMissing {
            field: "verified_path_at_ms",
        },
    )?;
    let duration_ms = verified_path_at_ms.checked_sub(started_at_ms).ok_or(
        InstallMetricUnmeasurable::TimestampReversed {
            earlier: started_at_ms,
            later: verified_path_at_ms,
        },
    )?;
    let path_hits = count_to_u64(INSTALL_PATH_HITS_EXPECTATION.metric, inputs.path_hits)?;
    let backups_written = count_to_u64(
        INSTALL_BACKUP_RATIO_EXPECTATION.metric,
        inputs.backups_written,
    )?;
    let files_mutated = count_to_u64(
        INSTALL_BACKUP_RATIO_EXPECTATION.metric,
        inputs.files_mutated,
    )?;
    let deltas = [
        measure_metric(
            INSTALL_DURATION_EXPECTATION,
            inputs.thresholds.duration,
            duration_ms,
        )?,
        measure_metric(
            INSTALL_PATH_HITS_EXPECTATION,
            inputs.thresholds.path_hits,
            path_hits,
        )?,
        measure_ratio_metric(
            INSTALL_BACKUP_RATIO_EXPECTATION,
            inputs.thresholds.backup_ratio,
            backups_written,
            files_mutated,
            INSTALL_BACKUP_RATIO_SCALE,
        )?,
    ];
    let canonical = format!(
        "writer=installer::assemble_install_report|start={started_at_ms}|verified={verified_path_at_ms}|attempt={}:{}:{}|manifest={manifest_digest}|duration={}:{}|path_hits={}:{}|backup_ratio={}:{}:{backups_written}:{files_mutated}",
        identity.pane,
        identity.incarnation,
        identity.attempt,
        deltas[0].observed,
        deltas[0].delta,
        deltas[1].observed,
        deltas[1].delta,
        deltas[2].observed,
        deltas[2].delta,
    );
    Ok(InstallMetrics {
        writer: "installer::assemble_install_report",
        started_at_ms,
        verified_path_at_ms,
        observed_at_ms: verified_path_at_ms,
        freshness_threshold_ms: L0_OBSERVE_STALL_MS,
        attempt_identity: identity.clone(),
        manifest_digest,
        deltas,
        record_digest: format!("{:016x}", fnv1a64(canonical.as_bytes())),
    })
}

pub fn validate_install_metric_freshness(
    metrics: &InstallMetrics,
    now_ms: u64,
) -> Result<u64, InstallMetricUnmeasurable> {
    if metrics.attempt_identity.attempt.trim().is_empty() {
        return Err(InstallMetricUnmeasurable::AttemptIdentityMissing);
    }
    if metrics.verified_path_at_ms < metrics.started_at_ms
        || metrics.observed_at_ms < metrics.verified_path_at_ms
        || now_ms < metrics.observed_at_ms
    {
        return Err(InstallMetricUnmeasurable::TimestampReversed {
            earlier: metrics.observed_at_ms,
            later: now_ms,
        });
    }
    let age_ms = now_ms - metrics.observed_at_ms;
    if age_ms > metrics.freshness_threshold_ms {
        return Err(InstallMetricUnmeasurable::StaleReport {
            age_ms,
            threshold_ms: metrics.freshness_threshold_ms,
        });
    }
    Ok(age_ms)
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
/// One cataloged build artifact: which triple it was built for, which binary
/// it is, and where it lives. A catalog is caller-provided — a directory
/// scan tagged with the resolved triple, or an explicit list. It is never
/// inferred from target/release/<binary>: that join assumes the artifact
/// exists instead of proving it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactEntry {
    pub triple: String,
    pub binary: String,
    pub path: PathBuf,
}

/// Read an artifact directory as an explicit catalog. Every regular file
/// becomes a row tagged with the triple the resolver selected; the triple
/// comes from the resolver, never from filenames. An unreadable directory
/// is a typed error; an empty directory is a valid empty catalog whose
/// selection refuses downstream.
pub fn catalog_artifact_dir(dir: &Path, triple: &str) -> Result<Vec<ArtifactEntry>, InstallError> {
    let entries = std::fs::read_dir(dir).map_err(|error| InstallError::ArtifactUnavailable {
        triple: triple.to_owned(),
        detail: format!("artifact catalog {} unreadable: {error}", dir.display()),
    })?;
    let mut catalog = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| InstallError::ArtifactUnavailable {
            triple: triple.to_owned(),
            detail: format!("artifact catalog {} entry unreadable: {error}", dir.display()),
        })?;
        if !entry.file_type().map(|kind| kind.is_file()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        catalog.push(ArtifactEntry {
            triple: triple.to_owned(),
            binary: name,
            path: entry.path(),
        });
    }
    Ok(catalog)
}

/// L0-PLATFORM-TRIPLE composition. resolve_platform_triple stays pure
/// selection; this proves the selected artifact EXISTS before staging. A
/// musl→gnu fallback (and every other resolution) requires a catalog row
/// naming a NONEMPTY EXISTING file whose triple and binary match. Absent
/// row, wrong arch/triple, unreadable file, and empty file are typed
/// L0-PLATFORM-TRIPLE errors. resolve_platform_triple's own vocabulary is
/// untouched.
pub fn select_fallback_artifact(
    resolution: &PlatformResolution,
    catalog: &[ArtifactEntry],
    binary_name: &str,
) -> Result<PathBuf, InstallError> {
    let triple = resolution.artifact_triple;
    let row = catalog
        .iter()
        .find(|entry| entry.triple == triple && entry.binary == binary_name);
    let row = match row {
        Some(row) => row,
        None => {
            let mut have: Vec<String> = catalog
                .iter()
                .map(|entry| format!("{}:{}", entry.triple, entry.binary))
                .collect();
            have.sort();
            have.dedup();
            return Err(InstallError::ArtifactUnavailable {
                triple: triple.to_owned(),
                detail: if have.is_empty() {
                    format!("empty catalog names no {binary_name}")
                } else {
                    format!("catalog names no {triple} {binary_name}; have {}", have.join(" "))
                },
            });
        }
    };
    let bytes = std::fs::metadata(&row.path).map(|meta| meta.len());
    match bytes {
        Err(error) => Err(InstallError::ArtifactUnavailable {
            triple: triple.to_owned(),
            detail: format!("artifact {} unreadable: {error}", row.path.display()),
        }),
        Ok(0) => Err(InstallError::ArtifactUnavailable {
            triple: triple.to_owned(),
            detail: format!("artifact {} is empty", row.path.display()),
        }),
        Ok(_) => Ok(row.path.clone()),
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

/// Backup record for one mutated path. Backup coverage is one-to-one with
/// mutation coverage: `path` names the mutated file, `backup` the
/// timestamped file holding its prior bytes, and `existed` whether the path
/// existed before the merge. A path that did not exist has no prior bytes,
/// so no backup file is created for it — only the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookBackup {
    pub path: PathBuf,
    pub backup: PathBuf,
    pub existed: bool,
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

/// Roll back the first `upto` writes: existing paths regain their prior
/// bytes from their backup files, and paths that did not exist before the
/// merge are deleted. A new path is residue, never restored as empty bytes —
/// an empty-byte write would fake a restore while leaving litter. Best
/// effort throughout: the failure is already decided; leftovers are named
/// by the returned backup list.
fn rollback_hook_writes(writes: &[HookWrite], backups: &[HookBackup], upto: usize) {
    for (write, backup) in writes.iter().zip(backups.iter()).take(upto) {
        if backup.existed {
            let _ = std::fs::copy(&backup.backup, &write.path);
        } else {
            let _ = std::fs::remove_file(&write.path);
        }
    }
}

/// L0-HOOK-MERGE. Backup every file first, then write. An empty write set is
/// a typed error, never a vacuous backup set. Injected failure after
/// `fail_after` writes rolls back the writes that already landed: existing
/// paths regain pre-merge bytes, new paths are deleted.
pub fn merge_hooks(
    writes: &[HookWrite],
    fail_after: Option<usize>,
) -> Result<Vec<HookBackup>, InstallError> {
    if writes.is_empty() {
        return Err(InstallError::EmptyHookWrites);
    }
    let mut backups = Vec::new();
    for write in writes {
        // Prior presence is the path's own dir entry, not a read: a missing
        // path reads as empty bytes, and that conflation is exactly what
        // produced empty-byte fake restores.
        let existed = std::fs::symlink_metadata(&write.path).is_ok();
        let backup = timestamped_backup_path(&write.path);
        if existed {
            let original = std::fs::read(&write.path).map_err(|error| {
                InstallError::IoError {
                    path: write.path.display().to_string(),
                    detail: format!("hook read failed: {error}"),
                }
            })?;
            std::fs::write(&backup, &original).map_err(|error| {
                InstallError::IoError {
                    path: backup.display().to_string(),
                    detail: format!("hook backup failed: {error}"),
                }
            })?;
        }
        backups.push(HookBackup {
            path: write.path.clone(),
            backup,
            existed,
        });
    }
    for (index, write) in writes.iter().enumerate() {
        if Some(index) == fail_after {
            rollback_hook_writes(writes, &backups, index);
            return Err(InstallError::HookMergeFailed {
                backups: backups
                    .iter()
                    .map(|record| record.backup.display().to_string())
                    .collect(),
            });
        }
        if let Err(error) = std::fs::write(&write.path, &write.merged) {
            rollback_hook_writes(writes, &backups, index);
            return Err(InstallError::IoError {
                path: write.path.display().to_string(),
                detail: format!("hook write failed: {error}"),
            });
        }
    }
    Ok(backups)
}

// b09-x282: ratified roster + detection live in agent_families.rs (own file,
// own tests) to stay clear of the active b07 lane in this file.
pub mod agent_families;

// b11-uegf: per-family skill installation lives in skill_install.rs (own
// file, own tests), beside the roster it consumes.
pub mod skill_install;

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
    pub attempt_identity: AttemptIdentity,
    pub install_metrics: Option<InstallMetrics>,
    pub durability_metric: Option<MetricDelta>,
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
        let metric_digest = self
            .install_metrics
            .as_ref()
            .map(|metrics| metrics.record_digest.as_str())
            .unwrap_or("UNMEASURABLE");
        write!(
            formatter,
            "L0-REPORT agents={agents} backups={backups} path_hits={hits} digest={} identity={} HEAD={} consistent={} legs={} attempt={} metrics={metric_digest}",
            self.digest,
            self.identity.binary_name,
            self.identity.head_sha,
            self.identity.consistent,
            self.identity.identity_legs(),
            self.attempt_identity.attempt,
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

/// Seal the L0 report. Missing a detected agent, digest, report identity,
/// or attempt identity is ERROR; an empty path-hit set remains valid data.
pub fn seal_install_report(
    detected: &AgentScan,
    outcomes: Vec<AgentOutcome>,
    backups: Vec<PathBuf>,
    path_hits: Vec<PathBuf>,
    identity: IdentityCheck,
    attempt_identity: AttemptIdentity,
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
    if attempt_identity.attempt.trim().is_empty() {
        missing.push("attempt_identity.attempt".to_owned());
    }
    if !missing.is_empty() {
        return Err(InstallError::IncompleteInstallReport { missing });
    }
    let canonical = format!(
        "agents={}|backups={}|hits={}|identity={}:{}|attempt={}:{}:{}",
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
        identity.head_sha,
        attempt_identity.pane,
        attempt_identity.incarnation,
        attempt_identity.attempt,
    );
    let digest = format!("{:016x}", fnv1a64(canonical.as_bytes()));
    let report = InstallReport {
        agent_outcomes: outcomes,
        backups,
        path_hits,
        digest,
        identity,
        attempt_identity,
        install_metrics: None,
        durability_metric: None,
    };
    if report.digest.is_empty() {
        return Err(InstallError::IncompleteInstallReport {
            missing: vec!["digest".to_owned()],
        });
    }
    Ok(report)
}
/// Bind the materialized metric record into the report's content identity.
fn attach_install_metrics(
    mut report: InstallReport,
    metrics: InstallMetrics,
    durability_metric: MetricDelta,
) -> InstallReport {
    let durability_json = durability_metric.to_json_value().to_string();
    let canonical = format!(
        "{}|metrics={}|durability={durability_json}",
        report.digest, metrics.record_digest
    );
    report.digest = format!("{:016x}", fnv1a64(canonical.as_bytes()));
    report.install_metrics = Some(metrics);
    report.durability_metric = Some(durability_metric);
    report
}

/// L0-B12 input manifest: what input set a sealed report closed over.
/// FULL carries the verified artifact digest. PARTIAL and REFUSED are
/// first-class restrictive states and never reach report persistence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputManifest {
    Full {
        digest: String,
    },
    Partial {
        bound_kind: String,
        bound_value: usize,
        source: String,
    },
    Refused {
        reason: String,
    },
}

impl std::fmt::Display for InputManifest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full { digest } => write!(formatter, "FULL digest={digest}"),
            Self::Partial {
                bound_kind,
                bound_value,
                source,
            } => write!(
                formatter,
                "PARTIAL bound={bound_kind} value={bound_value} source={source}"
            ),
            Self::Refused { reason } => write!(formatter, "REFUSED reason={reason}"),
        }
    }
}

/// Canonical durable report path from the S1 L0 contract. This replaces the
/// historical text artifact; there is one writer and one report.
pub const INSTALL_REPORT_ARTIFACT: &str = ".omp-orchestrator/work/s1/l0/install-report.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedInstallReport {
    pub report: InstallReport,
    pub artifact: PathBuf,
    pub write_bytes: usize,
    pub file_fsynced: bool,
    pub parent_fsynced: bool,
    pub readback_bytes: usize,
    pub artifact_inode: Option<u64>,
}
#[cfg(unix)]
fn metadata_inode(metadata: &std::fs::Metadata) -> Option<u64> {
    Some(std::os::unix::fs::MetadataExt::ino(metadata))
}

#[cfg(not(unix))]
fn metadata_inode(_metadata: &std::fs::Metadata) -> Option<u64> {
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorrelatedInstallReport {
    pub readback_bytes: usize,
    pub path_hits: usize,
    pub host_capabilities: usize,
    pub metric_count: usize,
}

fn repo_ownership_json(ownership: &RepoOwnership) -> serde_json::Value {
    match ownership {
        RepoOwnership::ThisRepo => serde_json::json!({"state": "this_repo"}),
        RepoOwnership::Foreign { repo } => {
            serde_json::json!({"state": "foreign", "repo": repo})
        }
        RepoOwnership::Unknown => serde_json::json!({"state": "unknown"}),
    }
}

fn input_manifest_json(manifest: &InputManifest) -> serde_json::Value {
    match manifest {
        InputManifest::Full { digest } => {
            serde_json::json!({"state": "FULL", "digest": digest})
        }
        InputManifest::Partial {
            bound_kind,
            bound_value,
            source,
        } => serde_json::json!({
            "state": "PARTIAL",
            "bound_kind": bound_kind,
            "bound_value": bound_value,
            "source": source,
        }),
        InputManifest::Refused { reason } => {
            serde_json::json!({"state": "REFUSED", "reason": reason})
        }
    }
}

fn install_metrics_json(metrics: &InstallMetrics) -> serde_json::Value {
    serde_json::json!({
        "writer": metrics.writer,
        "started_at_ms": metrics.started_at_ms,
        "verified_path_at_ms": metrics.verified_path_at_ms,
        "observed_at_ms": metrics.observed_at_ms,
        "freshness_threshold_ms": metrics.freshness_threshold_ms,
        "attempt_identity": {
            "pane": metrics.attempt_identity.pane,
            "incarnation": metrics.attempt_identity.incarnation,
            "attempt": metrics.attempt_identity.attempt,
        },
        "input_manifest": {"state": "FULL", "digest": metrics.manifest_digest},
        "deltas": metrics.deltas.iter().copied().map(MetricDelta::to_json_value).collect::<Vec<_>>(),
        "record_digest": metrics.record_digest,
    })
}

fn durability_metric_json(metric: MetricDelta) -> serde_json::Value {
    let mut value = metric.to_json_value();
    let object = value
        .as_object_mut()
        .expect("MetricDelta::to_json_value always returns an object");
    object.insert(
        "parent_fsync_successes".to_owned(),
        serde_json::json!(metric.numerator),
    );
    object.insert(
        "atomic_rename_attempts".to_owned(),
        serde_json::json!(metric.denominator),
    );
    value
}

fn install_report_document(report: &InstallReport, manifest: &InputManifest) -> String {
    let outcomes: Vec<serde_json::Value> = report
        .agent_outcomes
        .iter()
        .map(|row| serde_json::json!({"family": row.family, "outcome": row.outcome}))
        .collect();
    let backups: Vec<String> = report
        .backups
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    let path_hits: Vec<String> = report
        .path_hits
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    let install_metrics = report
        .install_metrics
        .as_ref()
        .map(install_metrics_json)
        .unwrap_or(serde_json::Value::Null);
    let durability_metric = report
        .durability_metric
        .map(durability_metric_json)
        .unwrap_or(serde_json::Value::Null);
    let value = serde_json::json!({
        "schema_version": "install-report.v1",
        "attempt_identity": {
            "pane": report.attempt_identity.pane,
            "incarnation": report.attempt_identity.incarnation,
            "attempt": report.attempt_identity.attempt,
        },
        "agent_outcomes": outcomes,
        "backups": backups,
        "path_hits": path_hits,
        "digest": report.digest,
        "identity": {
            "binary_name": report.identity.binary_name,
            "repo_ownership": repo_ownership_json(&report.identity.repo_ownership),
            "head_sha": report.identity.head_sha,
            "build_id_in_binary": report.identity.build_id_in_binary,
            "version_output": report.identity.version_output,
            "consistent": report.identity.consistent,
            "identity_legs": report.identity.identity_legs(),
        },
        "input_manifest": input_manifest_json(manifest),
        "install_metrics": install_metrics,
        "durability_metric": durability_metric,
    });
    let mut document =
        serde_json::to_string_pretty(&value).expect("serializing a JSON Value cannot fail");
    document.push('\n');
    document
}

fn metric_readback_error(
    path: &Path,
    detail: impl Into<String>,
) -> InstallMetricUnmeasurable {
    let detail = detail.into();
    let path = path.to_path_buf();
    InstallMetricUnmeasurable::ReadbackFailed { path, detail }
}

fn metric_object<'a>(
    value: &'a serde_json::Value,
    path: &Path,
    field: &str,
) -> Result<&'a serde_json::Map<String, serde_json::Value>, InstallMetricUnmeasurable> {
    value
        .as_object()
        .ok_or_else(|| metric_readback_error(path, format!("field={field} expected=object")))
}

fn metric_u64(
    object: &serde_json::Map<String, serde_json::Value>,
    path: &Path,
    field: &str,
) -> Result<u64, InstallMetricUnmeasurable> {
    object
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| metric_readback_error(path, format!("field={field} expected=u64")))
}

fn metric_timestamp(
    object: &serde_json::Map<String, serde_json::Value>,
    field: &'static str,
) -> Result<u64, InstallMetricUnmeasurable> {
    object
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .ok_or(InstallMetricUnmeasurable::TimestampMissing { field })
}

fn metric_string<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    path: &Path,
    field: &str,
) -> Result<&'a str, InstallMetricUnmeasurable> {
    object
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| metric_readback_error(path, format!("field={field} expected=nonempty-string")))
}

fn metric_attempt_identity(
    value: &serde_json::Value,
    path: &Path,
) -> Result<AttemptIdentity, InstallMetricUnmeasurable> {
    let object = metric_object(value, path, "attempt_identity")?;
    let attempt = object
        .get("attempt")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or(InstallMetricUnmeasurable::AttemptIdentityMissing)?
        .to_owned();
    Ok(AttemptIdentity {
        pane: object
            .get("pane")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        incarnation: object
            .get("incarnation")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        attempt,
    })
}

fn metric_delta_row<'a>(
    rows: &'a [serde_json::Value],
    path: &Path,
    metric: &'static str,
) -> Result<&'a serde_json::Map<String, serde_json::Value>, InstallMetricUnmeasurable> {
    rows.iter()
        .filter_map(serde_json::Value::as_object)
        .find(|row| row.get("metric").and_then(serde_json::Value::as_str) == Some(metric))
        .ok_or_else(|| metric_readback_error(path, format!("metric={metric} absent")))
}

fn metric_threshold_from_row(
    row: &serde_json::Map<String, serde_json::Value>,
    direction: MetricDirection,
    expectation: MetricExpectation,
) -> Result<MetricThreshold, InstallMetricUnmeasurable> {
    let tolerance = row
        .get("threshold")
        .and_then(serde_json::Value::as_u64)
        .ok_or(MetricMeasureError::MissingThreshold {
            metric: expectation.metric,
        })?;
    Ok(MetricThreshold {
        tolerance,
        direction,
    })
}

fn parse_durability_metric(
    root: &serde_json::Map<String, serde_json::Value>,
    path: &Path,
) -> Result<MetricDelta, InstallMetricUnmeasurable> {
    let value = root
        .get("durability_metric")
        .filter(|value| !value.is_null())
        .ok_or(InstallMetricUnmeasurable::DurabilityMetricMissing)?;
    let row = metric_object(value, path, "durability_metric")?;
    let metric = DurabilityMetric {
        parent_fsync_successes: metric_u64(row, path, "parent_fsync_successes")?,
        atomic_rename_attempts: metric_u64(row, path, "atomic_rename_attempts")?,
    };
    let recomputed = materialize_durability_metric(metric)?;
    if durability_metric_json(recomputed) != *value {
        return Err(metric_readback_error(
            path,
            "durability metric record/readback mismatch",
        ));
    }
    Ok(recomputed)
}

fn parse_install_metrics(
    root: &serde_json::Map<String, serde_json::Value>,
    path: &Path,
) -> Result<InstallMetrics, InstallMetricUnmeasurable> {
    let metric_value = root
        .get("install_metrics")
        .filter(|value| !value.is_null())
        .ok_or(InstallMetricUnmeasurable::MissingHome)?;
    let metric = metric_object(metric_value, path, "install_metrics")?;
    let writer = metric
        .get("writer")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty());
    if writer != Some("installer::assemble_install_report") {
        return Err(InstallMetricUnmeasurable::WriterAbsent {
            path: path.to_owned(),
        });
    }
    let top_manifest = metric_object(
        root.get("input_manifest")
            .ok_or_else(|| metric_readback_error(path, "field=input_manifest absent"))?,
        path,
        "input_manifest",
    )?;
    let state = metric_string(top_manifest, path, "state")?;
    if state != "FULL" {
        return Err(InstallMetricUnmeasurable::InputNotFull {
            state: state.to_owned(),
        });
    }
    let manifest_digest = metric_string(top_manifest, path, "digest")?.to_owned();
    let identity = metric_attempt_identity(
        metric
            .get("attempt_identity")
            .ok_or(InstallMetricUnmeasurable::AttemptIdentityMissing)?,
        path,
    )?;
    let top_identity = metric_attempt_identity(
        root.get("attempt_identity")
            .ok_or(InstallMetricUnmeasurable::AttemptIdentityMissing)?,
        path,
    )?;
    if identity != top_identity {
        return Err(InstallMetricUnmeasurable::AttemptIdentityMissing);
    }
    let deltas = metric
        .get("deltas")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| metric_readback_error(path, "field=deltas expected=array"))?;
    if deltas.len() != 3 {
        return Err(metric_readback_error(
            path,
            format!("field=deltas expected=3 actual={}", deltas.len()),
        ));
    }
    let duration = metric_delta_row(deltas, path, INSTALL_DURATION_EXPECTATION.metric)?;
    let path_hits = metric_delta_row(deltas, path, INSTALL_PATH_HITS_EXPECTATION.metric)?;
    let backup_ratio = metric_delta_row(deltas, path, INSTALL_BACKUP_RATIO_EXPECTATION.metric)?;
    let observed_at_ms = metric_timestamp(metric, "observed_at_ms")?;
    let freshness_threshold_ms = metric_timestamp(metric, "freshness_threshold_ms")?;
    let inputs = InstallMetricInputs {
        started_at_ms: Some(metric_timestamp(metric, "started_at_ms")?),
        verified_path_at_ms: Some(metric_timestamp(metric, "verified_path_at_ms")?),
        path_hits: usize::try_from(metric_u64(path_hits, path, "observed")?)
            .map_err(|_| metric_readback_error(path, "path_hits exceeds usize"))?,
        backups_written: usize::try_from(metric_u64(backup_ratio, path, "numerator")?)
            .map_err(|_| metric_readback_error(path, "backups_written exceeds usize"))?,
        files_mutated: usize::try_from(metric_u64(backup_ratio, path, "denominator")?)
            .map_err(|_| metric_readback_error(path, "files_mutated exceeds usize"))?,
        // Parsed separately from the three qod0 install metrics.
        durability_metric: DurabilityMetric::default(),
        thresholds: InstallMetricThresholds {
            duration: Some(metric_threshold_from_row(
                duration,
                MetricDirection::AtMost,
                INSTALL_DURATION_EXPECTATION,
            )?),
            path_hits: Some(metric_threshold_from_row(
                path_hits,
                MetricDirection::AtMost,
                INSTALL_PATH_HITS_EXPECTATION,
            )?),
            backup_ratio: Some(metric_threshold_from_row(
                backup_ratio,
                MetricDirection::AtLeast,
                INSTALL_BACKUP_RATIO_EXPECTATION,
            )?),
        },
    };
    let manifest = InputManifest::Full {
        digest: manifest_digest,
    };
    let recomputed = materialize_install_metrics(inputs, &identity, &manifest)?;
    if observed_at_ms < recomputed.verified_path_at_ms {
        return Err(InstallMetricUnmeasurable::TimestampReversed {
            earlier: recomputed.verified_path_at_ms,
            later: observed_at_ms,
        });
    }
    if observed_at_ms != recomputed.observed_at_ms
        || freshness_threshold_ms != recomputed.freshness_threshold_ms
    {
        return Err(metric_readback_error(path, "timestamp/freshness readback mismatch"));
    }
    if install_metrics_json(&recomputed) != *metric_value {
        return Err(metric_readback_error(path, "metric record digest/readback mismatch"));
    }
    let top_path_hits = root
        .get("path_hits")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| metric_readback_error(path, "field=path_hits expected=array"))?;
    if top_path_hits.len() != inputs.path_hits {
        return Err(metric_readback_error(path, "metric path_hits differs from report"));
    }
    let top_backups = root
        .get("backups")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| metric_readback_error(path, "field=backups expected=array"))?;
    if top_backups.len() != inputs.backups_written {
        return Err(metric_readback_error(path, "metric backups differs from report"));
    }
    let files_mutated = root
        .get("agent_outcomes")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| metric_readback_error(path, "field=agent_outcomes expected=array"))?
        .iter()
        .filter(|row| row.get("outcome").and_then(serde_json::Value::as_str) == Some("merged"))
        .count();
    if files_mutated != inputs.files_mutated {
        return Err(metric_readback_error(path, "metric files_mutated differs from report"));
    }
    Ok(recomputed)
}

pub fn read_install_metric_deltas(
    repo_root: &Path,
    now_ms: u64,
) -> Result<InstallMetricDeltaReport, InstallMetricUnmeasurable> {
    let path = repo_root.join(INSTALL_REPORT_ARTIFACT);
    let bytes = std::fs::read(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            InstallMetricUnmeasurable::WriterAbsent { path: path.clone() }
        } else {
            metric_readback_error(&path, error.to_string())
        }
    })?;
    if bytes.is_empty() {
        return Err(metric_readback_error(&path, "zero-byte report"));
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| metric_readback_error(&path, error.to_string()))?;
    let root = metric_object(&value, &path, "$")?;
    if metric_string(root, &path, "schema_version")? != "install-report.v1" {
        return Err(metric_readback_error(&path, "unsupported schema_version"));
    }
    let metrics = parse_install_metrics(root, &path)?;
    let durability_metric = parse_durability_metric(root, &path)?;
    let age_ms = validate_install_metric_freshness(&metrics, now_ms)?;
    Ok(InstallMetricDeltaReport {
        metrics,
        durability_metric,
        readback_bytes: bytes.len(),
        age_ms,
        fresh: true,
    })
}

impl InstallMetricDeltaReport {
    #[must_use]
    pub fn to_json_line(&self) -> String {
        serde_json::json!({
            "schema": "installer.metric_delta.v1",
            "verdict": self.metrics.overall_verdict().as_str(),
            "fresh": self.fresh,
            "age_ms": self.age_ms,
            "readback_bytes": self.readback_bytes,
            "metrics": install_metrics_json(&self.metrics),
            "durability_metric": durability_metric_json(self.durability_metric),
        })
        .to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InstallReportPersistence {
    write_bytes: usize,
    file_fsynced: bool,
    parent_fsynced: bool,
}

fn persist_install_report(
    artifact: &Path,
    document: &str,
) -> Result<InstallReportPersistence, InstallError> {
    let parent = artifact.parent().ok_or_else(|| InstallError::IoError {
        path: artifact.display().to_string(),
        detail: "report path has no parent".to_owned(),
    })?;
    std::fs::create_dir_all(parent).map_err(|error| InstallError::IoError {
        path: parent.display().to_string(),
        detail: format!("report dir failed: {error}"),
    })?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(artifact)
        .map_err(|error| InstallError::IoError {
            path: artifact.display().to_string(),
            detail: format!("report open failed: {error}"),
        })?;
    file.write_all(document.as_bytes())
        .map_err(|error| InstallError::IoError {
            path: artifact.display().to_string(),
            detail: format!("report write failed: {error}"),
        })?;
    file.sync_all().map_err(|error| InstallError::IoError {
        path: artifact.display().to_string(),
        detail: format!("report fsync failed: {error}"),
    })?;
    let directory = std::fs::File::open(parent).map_err(|error| InstallError::IoError {
        path: parent.display().to_string(),
        detail: format!("report parent open failed: {error}"),
    })?;
    directory
        .sync_all()
        .map_err(|error| InstallError::IoError {
            path: parent.display().to_string(),
            detail: format!("report parent sync failed: {error}"),
        })?;
    Ok(InstallReportPersistence {
        write_bytes: document.len(),
        file_fsynced: true,
        parent_fsynced: true,
    })
}

fn persist_and_read_install_report(
    artifact: &Path,
    document: &str,
) -> Result<(InstallReportPersistence, usize), InstallError> {
    let persistence = persist_install_report(artifact, document)?;
    if persistence.write_bytes == 0 {
        return Err(InstallError::InstallReportRefused(
            InstallReportCause::WriterSuppressed {
                path: artifact.to_owned(),
            },
        ));
    }
    let readback = std::fs::read_to_string(artifact).map_err(|error| {
        InstallError::InstallReportRefused(InstallReportCause::ReadbackFailed {
            path: artifact.to_owned(),
            detail: error.to_string(),
        })
    })?;
    if readback.is_empty() {
        return Err(InstallError::InstallReportRefused(
            InstallReportCause::ZeroBytes {
                path: artifact.to_owned(),
            },
        ));
    }
    if readback != document {
        return Err(InstallError::InstallReportRefused(
            InstallReportCause::ReadbackMismatch {
                path: artifact.to_owned(),
                detail: "artifact bytes differ from sealed bytes".to_owned(),
            },
        ));
    }
    Ok((persistence, readback.len()))
}

fn seal_persisted_report(
    report: InstallReport,
    artifact: PathBuf,
    persistence: InstallReportPersistence,
    readback_bytes: usize,
) -> Result<SealedInstallReport, InstallError> {
    let artifact_inode = std::fs::metadata(&artifact)
        .map(|metadata| metadata_inode(&metadata))
        .map_err(|error| InstallError::IoError {
            path: artifact.display().to_string(),
            detail: format!("report metadata failed: {error}"),
        })?;
    Ok(SealedInstallReport {
        report,
        artifact,
        write_bytes: persistence.write_bytes,
        file_fsynced: persistence.file_fsynced,
        parent_fsynced: persistence.parent_fsynced,
        readback_bytes,
        artifact_inode,
    })
}

pub fn assemble_install_report(
    repo_root: &Path,
    scan: &AgentScan,
    outcomes: &[AgentOutcome],
    backups: &[PathBuf],
    path_hits: &[PathBuf],
    identity: &IdentityCheck,
    attempt_identity: &AttemptIdentity,
    metric_inputs: InstallMetricInputs,
    manifest: &InputManifest,
) -> Result<SealedInstallReport, InstallError> {
    match manifest {
        InputManifest::Full { digest } if !digest.is_empty() => {}
        InputManifest::Full { .. } => {
            return Err(InstallError::IncompleteInstallReport {
                missing: vec!["digest".to_owned()],
            });
        }
        InputManifest::Partial {
            bound_kind,
            bound_value,
            source,
        } => {
            return Err(InstallError::PartialInputManifest {
                bound_kind: bound_kind.clone(),
                bound_value: *bound_value,
                source: source.clone(),
            });
        }
        InputManifest::Refused { reason } => {
            return Err(InstallError::RefusedInputManifest {
                reason: reason.clone(),
            });
        }
    }
    let report = seal_install_report(
        scan,
        outcomes.to_vec(),
        backups.to_vec(),
        path_hits.to_vec(),
        identity.clone(),
        attempt_identity.clone(),
    )?;
    let durability_metric = materialize_durability_metric(metric_inputs.durability_metric)
        .map_err(InstallError::InstallMetricUnmeasurable)?;
    let metrics = materialize_install_metrics(metric_inputs, attempt_identity, manifest)
        .map_err(InstallError::InstallMetricUnmeasurable)?;
    let report = attach_install_metrics(report, metrics, durability_metric);
    let artifact = repo_root.join(INSTALL_REPORT_ARTIFACT);
    let document = install_report_document(&report, manifest);
    let (persistence, readback_bytes) =
        persist_and_read_install_report(&artifact, &document)?;
    seal_persisted_report(report, artifact, persistence, readback_bytes)
}

/// One B12 writer followed immediately by r19i correlation over the same
/// identity, attempt, manifest, and canonical artifact.
pub fn assemble_and_correlate_install_report(
    repo_root: &Path,
    scan: &AgentScan,
    outcomes: &[AgentOutcome],
    backups: &[PathBuf],
    path_hits: &[PathBuf],
    identity: &IdentityCheck,
    attempt_identity: &AttemptIdentity,
    metric_inputs: InstallMetricInputs,
    manifest: &InputManifest,
) -> Result<(SealedInstallReport, CorrelatedInstallReport), InstallError> {
    let sealed = assemble_install_report(
        repo_root,
        scan,
        outcomes,
        backups,
        path_hits,
        identity,
        attempt_identity,
        metric_inputs,
        manifest,
    )?;
    let correlated = correlate_install_report(
        repo_root,
        Some(&sealed),
        identity,
        attempt_identity,
        manifest,
    )
    .map_err(InstallError::InstallReportRefused)?;
    Ok((sealed, correlated))
}

fn report_value_type(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn required_report_value<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    field: &str,
    path: &Path,
    key: &str,
) -> Result<&'a serde_json::Value, InstallReportCause> {
    object
        .get(field)
        .ok_or_else(|| InstallReportCause::MissingField {
            path: path.to_owned(),
            field: key.to_owned(),
        })
}

fn required_report_object<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    field: &str,
    path: &Path,
    key: &str,
) -> Result<&'a serde_json::Map<String, serde_json::Value>, InstallReportCause> {
    let value = required_report_value(object, field, path, key)?;
    value
        .as_object()
        .ok_or_else(|| InstallReportCause::WrongType {
            path: path.to_owned(),
            field: key.to_owned(),
            expected: "object",
            found: report_value_type(value),
        })
}

fn required_report_string<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    field: &str,
    path: &Path,
    key: &str,
    allow_empty: bool,
) -> Result<&'a str, InstallReportCause> {
    let value = required_report_value(object, field, path, key)?;
    match value {
        serde_json::Value::String(text) if allow_empty || !text.trim().is_empty() => Ok(text),
        serde_json::Value::String(_) => Err(InstallReportCause::EmptyField {
            path: path.to_owned(),
            field: key.to_owned(),
        }),
        other => Err(InstallReportCause::WrongType {
            path: path.to_owned(),
            field: key.to_owned(),
            expected: "string",
            found: report_value_type(other),
        }),
    }
}

fn required_report_array<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    field: &str,
    path: &Path,
    key: &str,
) -> Result<&'a Vec<serde_json::Value>, InstallReportCause> {
    let value = required_report_value(object, field, path, key)?;
    value
        .as_array()
        .ok_or_else(|| InstallReportCause::WrongType {
            path: path.to_owned(),
            field: key.to_owned(),
            expected: "array",
            found: report_value_type(value),
        })
}

fn identity_mismatch(
    field: &str,
    expected: impl ToString,
    actual: impl ToString,
) -> InstallReportCause {
    InstallReportCause::IdentityMismatch {
        field: field.to_owned(),
        expected: expected.to_string(),
        actual: actual.to_string(),
    }
}

fn full_report_manifest_digest<'a>(
    manifest: &'a InputManifest,
    canonical: &Path,
) -> Result<&'a str, InstallReportCause> {
    match manifest {
        InputManifest::Full { digest } if !digest.trim().is_empty() => Ok(digest),
        InputManifest::Full { .. } => Err(InstallReportCause::EmptyField {
            path: canonical.to_owned(),
            field: "input_manifest.digest".to_owned(),
        }),
        InputManifest::Partial {
            bound_kind,
            bound_value,
            source,
        } => Err(InstallReportCause::ManifestPartial {
            bound_kind: bound_kind.clone(),
            bound_value: *bound_value,
            source: source.clone(),
        }),
        InputManifest::Refused { reason } => Err(InstallReportCause::ManifestRefused {
            reason: reason.clone(),
        }),
    }
}

fn validate_sealed_report_receipts(
    canonical: &Path,
    sealed: Option<&SealedInstallReport>,
) -> Result<(), InstallReportCause> {
    let Some(sealed) = sealed else {
        return Ok(());
    };
    if sealed.artifact != canonical {
        return Err(identity_mismatch(
            "artifact_path",
            canonical.display(),
            sealed.artifact.display(),
        ));
    }
    if sealed.write_bytes == 0 {
        return Err(InstallReportCause::WriterSuppressed {
            path: canonical.to_owned(),
        });
    }
    if !sealed.file_fsynced || !sealed.parent_fsynced {
        return Err(InstallReportCause::ReadbackFailed {
            path: canonical.to_owned(),
            detail: format!(
                "B12 fsync receipt incomplete file={} parent={}",
                sealed.file_fsynced, sealed.parent_fsynced
            ),
        });
    }
    if sealed.readback_bytes == 0 {
        return Err(InstallReportCause::ReadbackFailed {
            path: canonical.to_owned(),
            detail: "B12 readback receipt is zero".to_owned(),
        });
    }
    Ok(())
}

fn read_canonical_report(
    canonical: &Path,
    was_sealed: bool,
) -> Result<String, InstallReportCause> {
    let contents = match std::fs::read_to_string(canonical) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(if was_sealed {
                InstallReportCause::DeletedAfterWrite {
                    path: canonical.to_owned(),
                }
            } else {
                InstallReportCause::Missing {
                    path: canonical.to_owned(),
                }
            });
        }
        Err(error) => {
            return Err(InstallReportCause::ReadbackFailed {
                path: canonical.to_owned(),
                detail: error.to_string(),
            });
        }
    };
    if contents.is_empty() {
        return Err(InstallReportCause::ZeroBytes {
            path: canonical.to_owned(),
        });
    }
    Ok(contents)
}

fn validate_report_path_hits(
    object: &serde_json::Map<String, serde_json::Value>,
    canonical: &Path,
    sealed: Option<&SealedInstallReport>,
) -> Result<Vec<String>, InstallReportCause> {
    let path_hits = required_report_array(object, "path_hits", canonical, "path_hits")?;
    let mut actual = Vec::with_capacity(path_hits.len());
    for (index, value) in path_hits.iter().enumerate() {
        let Some(path) = value.as_str() else {
            return Err(InstallReportCause::WrongType {
                path: canonical.to_owned(),
                field: format!("path_hits[{index}]"),
                expected: "string",
                found: report_value_type(value),
            });
        };
        actual.push(path.to_owned());
    }
    if let Some(sealed) = sealed {
        let expected: Vec<String> = sealed
            .report
            .path_hits
            .iter()
            .map(|path| path.display().to_string())
            .collect();
        if actual != expected {
            return Err(InstallReportCause::ReadbackMismatch {
                path: canonical.to_owned(),
                detail: format!("path_hits expected={expected:?} actual={actual:?}"),
            });
        }
    }
    Ok(actual)
}

fn validate_report_attempt(
    object: &serde_json::Map<String, serde_json::Value>,
    canonical: &Path,
    expected: &AttemptIdentity,
) -> Result<(), InstallReportCause> {
    let attempt =
        required_report_object(object, "attempt_identity", canonical, "attempt_identity")?;
    for (field, wanted, allow_empty) in [
        ("pane", expected.pane.as_str(), true),
        ("incarnation", expected.incarnation.as_str(), true),
        ("attempt", expected.attempt.as_str(), false),
    ] {
        let key = format!("attempt_identity.{field}");
        let actual = required_report_string(attempt, field, canonical, &key, allow_empty)?;
        if actual != wanted {
            return Err(identity_mismatch(&key, wanted, actual));
        }
    }
    Ok(())
}

fn validate_report_identity(
    object: &serde_json::Map<String, serde_json::Value>,
    canonical: &Path,
    expected: &IdentityCheck,
) -> Result<(), InstallReportCause> {
    let identity = required_report_object(object, "identity", canonical, "identity")?;
    for (field, wanted) in [
        ("binary_name", expected.binary_name.as_str()),
        ("head_sha", expected.head_sha.as_str()),
        ("identity_legs", expected.identity_legs()),
    ] {
        let key = format!("identity.{field}");
        let actual = required_report_string(identity, field, canonical, &key, false)?;
        if actual != wanted {
            return Err(identity_mismatch(&key, wanted, actual));
        }
    }
    let consistent = required_report_value(identity, "consistent", canonical, "identity.consistent")?;
    let Some(consistent) = consistent.as_bool() else {
        return Err(InstallReportCause::WrongType {
            path: canonical.to_owned(),
            field: "identity.consistent".to_owned(),
            expected: "boolean",
            found: report_value_type(consistent),
        });
    };
    if consistent != expected.consistent {
        return Err(identity_mismatch(
            "identity.consistent",
            expected.consistent,
            consistent,
        ));
    }
    for (field, wanted) in [
        ("repo_ownership", repo_ownership_json(&expected.repo_ownership)),
        (
            "build_id_in_binary",
            serde_json::json!(&expected.build_id_in_binary),
        ),
        ("version_output", serde_json::json!(&expected.version_output)),
    ] {
        let key = format!("identity.{field}");
        let actual = required_report_value(identity, field, canonical, &key)?;
        if actual != &wanted {
            return Err(identity_mismatch(&key, wanted, actual));
        }
    }
    Ok(())
}

fn validate_report_manifest(
    object: &serde_json::Map<String, serde_json::Value>,
    canonical: &Path,
    expected_digest: &str,
) -> Result<(), InstallReportCause> {
    let manifest =
        required_report_object(object, "input_manifest", canonical, "input_manifest")?;
    let state = required_report_string(
        manifest,
        "state",
        canonical,
        "input_manifest.state",
        false,
    )?;
    if state != "FULL" {
        return Err(identity_mismatch("input_manifest.state", "FULL", state));
    }
    let digest = required_report_string(
        manifest,
        "digest",
        canonical,
        "input_manifest.digest",
        false,
    )?;
    if digest != expected_digest {
        return Err(identity_mismatch(
            "input_manifest.digest",
            expected_digest,
            digest,
        ));
    }
    Ok(())
}

fn validate_inception_host_capabilities(repo_root: &Path) -> Result<(), InstallReportCause> {
    let path = repo_root.join(".omp-orchestrator/inception.json");
    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(InstallReportCause::InceptionMissing { path });
        }
        Err(error) => {
            return Err(InstallReportCause::HostCapabilitiesInvalid {
                path,
                detail: error.to_string(),
            });
        }
    };
    let value: serde_json::Value = serde_json::from_str(&contents).map_err(|error| {
        InstallReportCause::HostCapabilitiesInvalid {
            path: path.clone(),
            detail: error.to_string(),
        }
    })?;
    let host = value
        .as_object()
        .and_then(|object| object.get("host_capabilities"))
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| InstallReportCause::HostCapabilitiesInvalid {
            path: path.clone(),
            detail: "host_capabilities missing or not an object".to_owned(),
        })?;
    for field in ["os", "arch", "filesystem"] {
        if !host
            .get(field)
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
        {
            return Err(InstallReportCause::HostCapabilitiesInvalid {
                path: path.clone(),
                detail: format!("host_capabilities.{field} missing, empty, or not a string"),
            });
        }
    }
    Ok(())
}

fn validate_sealed_report_document(
    contents: &str,
    canonical: &Path,
    sealed: Option<&SealedInstallReport>,
    manifest: &InputManifest,
) -> Result<(), InstallReportCause> {
    let Some(sealed) = sealed else {
        return Ok(());
    };
    if contents.len() != sealed.readback_bytes {
        return Err(InstallReportCause::ReadbackMismatch {
            path: canonical.to_owned(),
            detail: format!(
                "readback bytes expected={} actual={}",
                sealed.readback_bytes,
                contents.len()
            ),
        });
    }
    if contents != install_report_document(&sealed.report, manifest) {
        return Err(InstallReportCause::ReadbackMismatch {
            path: canonical.to_owned(),
            detail: "canonical document differs from B12 sealed report".to_owned(),
        });
    }
    Ok(())
}

pub fn correlate_install_report(
    repo_root: &Path,
    sealed: Option<&SealedInstallReport>,
    expected_identity: &IdentityCheck,
    expected_attempt: &AttemptIdentity,
    manifest: &InputManifest,
) -> Result<CorrelatedInstallReport, InstallReportCause> {
    let canonical = repo_root.join(INSTALL_REPORT_ARTIFACT);
    let manifest_digest = full_report_manifest_digest(manifest, &canonical)?;
    validate_sealed_report_receipts(&canonical, sealed)?;
    let contents = read_canonical_report(&canonical, sealed.is_some())?;
    let value: serde_json::Value = serde_json::from_str(&contents).map_err(|error| {
        InstallReportCause::Truncated {
            path: canonical.clone(),
            detail: error.to_string(),
        }
    })?;
    let object = value
        .as_object()
        .ok_or_else(|| InstallReportCause::WrongType {
            path: canonical.clone(),
            field: "$".to_owned(),
            expected: "object",
            found: report_value_type(&value),
        })?;
    let schema = required_report_string(
        object,
        "schema_version",
        &canonical,
        "schema_version",
        false,
    )?;
    if schema != "install-report.v1" {
        return Err(identity_mismatch(
            "schema_version",
            "install-report.v1",
            schema,
        ));
    }
    let digest = required_report_string(object, "digest", &canonical, "digest", false)?;
    if let Some(sealed) = sealed {
        if digest != sealed.report.digest {
            return Err(identity_mismatch("digest", &sealed.report.digest, digest));
        }
    }
    let path_hits = validate_report_path_hits(object, &canonical, sealed)?;
    validate_report_attempt(object, &canonical, expected_attempt)?;
    validate_report_identity(object, &canonical, expected_identity)?;
    validate_report_manifest(object, &canonical, manifest_digest)?;
    validate_inception_host_capabilities(repo_root)?;
    let metrics = parse_install_metrics(object, &canonical).map_err(|error| {
        InstallReportCause::MetricUnmeasurable {
            path: canonical.clone(),
            detail: error.to_string(),
        }
    })?;
    parse_durability_metric(object, &canonical).map_err(|error| {
        InstallReportCause::MetricUnmeasurable {
            path: canonical.clone(),
            detail: error.to_string(),
        }
    })?;
    validate_sealed_report_document(&contents, &canonical, sealed, manifest)?;
    Ok(CorrelatedInstallReport {
        readback_bytes: contents.len(),
        path_hits: path_hits.len(),
        host_capabilities: 3,
        metric_count: metrics.deltas.len() + 1,
    })
}
/// L0-B12-obs-writer (bead xic2): attempt identity carried explicitly on
/// every emit. `pane`/`incarnation` name the operating agent when known
/// (empty means unknown, never fabricated); `attempt` is minted once per
/// operator invocation and is REQUIRED non-empty -- an unattributed emit
/// is refused rather than journaled anonymously.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptIdentity {
    pub pane: String,
    pub incarnation: String,
    pub attempt: String,
}

/// Mint one attempt identity token for an operator invocation. Process id
/// plus nanos: unique per attempt without ambient coordination. Callers
/// that know their pane pass it explicitly; nothing here reads it from
/// the environment.
#[must_use]
pub fn mint_attempt_id() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    )
}

/// Centralized allowed S1.L0 reason set. Every stage_to=S1.L0 reason the
/// production installer emits is listed here; the emitter refuses anything
/// else with a typed reason so a misspelled or invented reason can never
/// journal as a legitimate outcome. Other stages pass through: their
/// authority lives with their own lanes, not here.
pub const ALLOWED_L0_REASONS: &[&str] = &[
    "INSTALL_FENCE_BLOCKED",
    "INSTALL_UNKNOWN_TARGET",
    "INSTALL_FOREIGN_TARGET",
    "INSTALL_GIT_HEAD_REFUSED",
    "INSTALL_PLATFORM_REFUSED",
    "INSTALL_RESTART_READ_REFUSED",
    "INSTALL_BUILD_REFUSED",
    "INSTALL_CATALOG_REFUSED",
    "INSTALL_SELECT_REFUSED",
    "INSTALL_VERIFY_REFUSED",
    "INSTALL_RESTART_REFUSED",
    "INSTALL_SKILLS_REFUSED",
    "INSTALL_REPORT_REFUSED",
    "INSTALL_OBSERVE_REFUSED",
    "INSTALL_VERIFIED",
];

/// Typed lifecycle-event emit with durable append, fsync, and readback proof.
/// Moved from the binary root (bead xic2) so integration legs drive the
/// same boundary production calls -- one writer, reachable from both.
///
/// Success carries the journal [`Readback`]. Any failure -- missing reason,
/// disallowed L0 reason, unattributed attempt, non-FULL manifest, unopenable
/// journal, unwritable file, failed fsync, or failed readback -- is `Err`
/// and MUST refuse the caller's success: the success line prints only
/// behind the `Ok` arm (see [`guard_success`]), so log-and-continue cannot
/// report success for an event that is not durable.
///
/// Manifest split, stated once: the emit path requires FULL *state* (the
/// input set was complete); the digest *value* is checked at report
/// assembly, because no digest exists yet when early refusals emit. A
/// PARTIAL or REFUSED manifest refuses here with its own typed reason.
/// The event schema carries pane/incarnation/attempt when present, and metric
/// events copy the same attempt identity already sealed in the report.
pub fn emit_s1(
    repo_root: &Path,
    layer: Layer,
    stage_to: &str,
    outcome: lifecycle_event::EmitOutcome,
    reason: &str,
    identity: &AttemptIdentity,
    manifest: &InputManifest,
    metric_deltas: &[MetricDelta],
) -> Result<lifecycle_event::Readback, lifecycle_event::EmitError> {
    emit_event_to_journal_with_metrics(
        &lifecycle_event::default_repo_journal(repo_root),
        layer,
        stage_to,
        outcome,
        reason,
        identity,
        manifest,
        metric_deltas,
    )
}

/// Sole journal writer; ordinary and metric-bearing [`emit_s1`] calls terminate here.
fn emit_event_to_journal_with_metrics(
    journal_path: &Path,
    layer: Layer,
    stage_to: &str,
    outcome: lifecycle_event::EmitOutcome,
    reason: &str,
    identity: &AttemptIdentity,
    manifest: &InputManifest,
    metric_deltas: &[MetricDelta],
) -> Result<lifecycle_event::Readback, lifecycle_event::EmitError> {
    use lifecycle_event::{DurableJournal, LifecycleEvent, ReasonCode};
    let refuse = |op: &'static str, detail: String| lifecycle_event::EmitError::Io {
        op,
        detail,
    };
    if identity.attempt.trim().is_empty() {
        return Err(refuse(
            "attempt_identity",
            "unattributed emit refused: attempt must be non-empty".to_owned(),
        ));
    }
    if stage_to == "S1.L0" && !ALLOWED_L0_REASONS.contains(&reason) {
        return Err(refuse(
            "l0_reason_allowlist",
            format!("reason {reason:?} is not allowlisted for S1.L0"),
        ));
    }
    match manifest {
        InputManifest::Full { .. } => {}
        InputManifest::Partial { .. } | InputManifest::Refused { .. } => {
            return Err(refuse(
                "input_manifest",
                format!("non-FULL manifest cannot emit: {manifest}"),
            ));
        }
    }
    let code = ReasonCode::new(reason).map_err(|error| {
        eprintln!(
            "LIFECYCLE_EVENT_EMIT_FAILED layer={} detail={error}",
            layer.as_str()
        );
        error
    })?;
    let event = LifecycleEvent::new(layer, "HUMAN", stage_to, "installer", outcome, code)
        .with_pane(identity.pane.clone())
        .with_incarnation(identity.incarnation.clone())
        .with_attempt(identity.attempt.clone())
        .with_metric_deltas(metric_deltas.to_vec());
    DurableJournal::open(journal_path.to_path_buf())
        .and_then(|journal| lifecycle_event::emit_one_host(&journal, event))
        .map_err(|error| {
            eprintln!(
                "LIFECYCLE_EVENT_EMIT_FAILED layer={} detail={error}",
                layer.as_str()
            );
            error
        })
}

/// Best-effort refusal event for restrictive paths. The `Result` exists for
/// the wiring proof (`main -> run_check/run_install -> event result ->
/// success guard`); callers discard it (`let _ =`) so a refused emit can
/// neither convert the original failure into success nor mask it with a
/// second failure.
pub fn emit_refusal(
    repo_root: &Path,
    layer: Layer,
    stage_to: &str,
    reason: &str,
    identity: &AttemptIdentity,
    manifest: &InputManifest,
) -> Result<lifecycle_event::Readback, lifecycle_event::EmitError> {
    emit_s1(repo_root, layer, stage_to, lifecycle_event::EmitOutcome::Refused, reason, identity, manifest, &[])
}

/// The success guard: the only place a success verdict prints. A refused
/// emit returns exit 1 and never the success line -- restoring log-and-continue
/// (ignore the `Result`, print success, return `SUCCESS`) reddens the
/// `success_guard_refuses_on_emit_failure` leg by construction.
pub fn guard_success(
    emit: Result<lifecycle_event::Readback, lifecycle_event::EmitError>,
    ok_line: &str,
) -> std::process::ExitCode {
    match emit {
        Ok(_) => {
            println!("{ok_line}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("INSTALLER LIFECYCLE REFUSED: success withheld, event not durable: {error}");
            ExitCode::from(1)
        }
    }
}

/// L0-B15-obs-gate (bead fx3d): exit for a refused install report. The
/// process exit vocabulary is 0 success, 1 operational refusal, 2 CLI
/// usage, 3 environment/identity -- and 4 vacuous input. An empty agent
/// scan measured nothing, so it must never share the measured-refusal
/// code: exit 4 keeps UNMEASURED distinguishable from REFUSED. Every
/// other assembly failure (manifest, digest, write, readback) is a
/// measured refusal and stays 1. `run_install` owns the call site; legs
/// pin both arms here.
#[must_use]
pub fn report_assembly_exit(error: &InstallError) -> std::process::ExitCode {
    if matches!(error, InstallError::EmptyAgentScan) {
        std::process::ExitCode::from(4)
    } else {
        std::process::ExitCode::from(1)
    }
}

/// L0 observe stall bound: follows the 60s operator default used by the
/// tick fleet and the resident supervisor. An emit that just landed reads
/// back in milliseconds, so a row older than this at gate time is stale
/// by observation, not by clock skew.
pub const L0_OBSERVE_STALL_MS: u64 = 60_000;

/// Which observability stage refused. The `Display` on
/// [`ObserveGateError`] is the exact refusal reason the legs pin.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObserveStage {
    Event,
    Monitor,
    Artifact,
    Gate,
}
impl ObserveStage {
    /// Stable stage token for refusal messages.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        const NAMES: [&str; 4] = ["EVENT", "MONITOR", "ARTIFACT", "GATE"];
        NAMES[self as usize]
    }
}
/// L0 rows observed with a fresh progressing verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObserveGate {
    pub rows: usize,
    pub last_ts: u64,
    pub age_ms: u64,
    pub freshness_threshold_ms: u64,
    pub fresh: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportArtifactObservation {
    pub path: PathBuf,
    pub inode: Option<u64>,
    pub sha256: String,
    pub write_bytes: usize,
    pub readback_bytes: usize,
    pub file_fsynced: bool,
    pub parent_fsynced: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledArtifactObservation {
    pub path: PathBuf,
    pub inode: Option<u64>,
    pub sha256: String,
    pub identity: IdentityCheck,
}

/// Installed-artifact authority consumed by the correlated L0 monitor.
#[derive(Debug, Clone, Copy)]
pub struct InstallObserveRequest<'a> {
    pub sealed: &'a SealedInstallReport,
    pub binary_name: &'a str,
    pub expected_path: &'a Path,
    pub path_env: &'a str,
    pub expected_identity: &'a IdentityCheck,
}

impl<'a> InstallObserveRequest<'a> {
    #[must_use]
    pub const fn new(
        sealed: &'a SealedInstallReport,
        binary_name: &'a str,
        expected_path: &'a Path,
        path_env: &'a str,
        expected_identity: &'a IdentityCheck,
    ) -> Self {
        Self {
            sealed,
            binary_name,
            expected_path,
            path_env,
            expected_identity,
        }
    }
}

impl SealedInstallReport {
    pub fn emit_verified_event(
        &self,
        repo_root: &Path,
        identity: &AttemptIdentity,
        manifest: &InputManifest,
    ) -> Result<lifecycle_event::Readback, lifecycle_event::EmitError> {
        let metrics = self.report.install_metrics.as_ref().ok_or_else(|| {
            lifecycle_event::EmitError::Io {
                op: "install_metrics",
                detail: InstallMetricUnmeasurable::MissingHome.to_string(),
            }
        })?;
        let durability_metric = self.report.durability_metric.ok_or_else(|| {
            lifecycle_event::EmitError::Io {
                op: "durability_metric",
                detail: InstallMetricUnmeasurable::DurabilityMetricMissing.to_string(),
            }
        })?;
        let mut deltas = Vec::with_capacity(metrics.deltas.len() + 1);
        deltas.extend(metrics.deltas);
        deltas.push(durability_metric);
        emit_s1(
            repo_root,
            Layer::L0,
            "S1.L0",
            lifecycle_event::EmitOutcome::Emitted,
            "INSTALL_VERIFIED",
            identity,
            manifest,
            &deltas,
        )
    }

    /// Gate a completed install against the artifact reached by this process's PATH.
    pub fn gate_install(
        &self,
        repo_root: &Path,
        manifest: &InputManifest,
        report: CorrelatedInstallReport,
        binary_name: &str,
        bin_dir: &Path,
        expected_identity: &IdentityCheck,
    ) -> Result<InstallObserveGate, ObserveGateError> {
        let expected_path = bin_dir.join(binary_name);
        let path_env = std::env::var("PATH").unwrap_or_default();
        gate_correlated_observability(
            repo_root,
            manifest,
            report,
            InstallObserveRequest::new(
                self,
                binary_name,
                &expected_path,
                &path_env,
                expected_identity,
            ),
        )
    }
}

/// B15 gate result after consuming r19i correlation, the durable report,
/// lifecycle freshness, required manifest, and installed artifact identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallObserveGate {
    pub report: ReportArtifactObservation,
    pub installed: Option<InstalledArtifactObservation>,
    pub manifest_digest: String,
    pub path_hits: usize,
    pub host_capabilities: usize,
    pub rows: usize,
    pub last_ts: u64,
    pub age_ms: u64,
    pub freshness_threshold_ms: u64,
    pub fresh: bool,
    pub metrics: InstallMetrics,
    pub durability_metric: MetricDelta,
    pub metric_age_ms: u64,
    pub metric_fresh: bool,
}
/// A refused observability gate: which stage refused and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObserveGateError {
    pub stage: ObserveStage,
    pub reason: String,
}

impl std::fmt::Display for ObserveGateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "INSTALLER_OBSERVE_REFUSED stage={} reason={}",
            self.stage.as_str(),
            self.reason
        )
    }
}

impl std::error::Error for ObserveGateError {}

fn artifact_observe_error(code: &str, detail: impl Into<String>) -> ObserveGateError {
    ObserveGateError {
        stage: ObserveStage::Artifact,
        reason: format!("{code} detail={}", detail.into()),
    }
}

#[cfg(unix)]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
    std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &std::fs::Metadata) -> bool {
    true
}

fn full_observe_manifest_digest(manifest: &InputManifest) -> Result<&str, ObserveGateError> {
    match manifest {
        InputManifest::Full { digest } if !digest.trim().is_empty() => Ok(digest),
        InputManifest::Full { .. } => Err(artifact_observe_error(
            "L0_MONITOR_MANIFEST_EMPTY",
            "FULL manifest digest is empty",
        )),
        InputManifest::Partial { .. } | InputManifest::Refused { .. } => Err(ObserveGateError {
            stage: ObserveStage::Gate,
            reason: format!("non-FULL manifest cannot gate: manifest={manifest}"),
        }),
    }
}

fn observe_report_artifact(
    sealed: &SealedInstallReport,
) -> Result<ReportArtifactObservation, ObserveGateError> {
    if sealed.write_bytes == 0 {
        return Err(artifact_observe_error(
            "L0_REPORT_WRITE_SUPPRESSED",
            sealed.artifact.display().to_string(),
        ));
    }
    if !sealed.file_fsynced || !sealed.parent_fsynced {
        return Err(artifact_observe_error(
            "L0_REPORT_FSYNC_INCOMPLETE",
            format!(
                "file={} parent={}",
                sealed.file_fsynced, sealed.parent_fsynced
            ),
        ));
    }
    let bytes = std::fs::read(&sealed.artifact).map_err(|error| {
        artifact_observe_error(
            "L0_REPORT_READBACK_FAILED",
            format!("path={} error={error}", sealed.artifact.display()),
        )
    })?;
    if bytes.len() != sealed.readback_bytes {
        return Err(artifact_observe_error(
            "L0_REPORT_READBACK_MISMATCH",
            format!("expected={} actual={}", sealed.readback_bytes, bytes.len()),
        ));
    }
    let metadata = std::fs::metadata(&sealed.artifact).map_err(|error| {
        artifact_observe_error(
            "L0_REPORT_METADATA_FAILED",
            format!("path={} error={error}", sealed.artifact.display()),
        )
    })?;
    let inode = metadata_inode(&metadata);
    if inode != sealed.artifact_inode {
        return Err(artifact_observe_error(
            "L0_REPORT_INODE_MISMATCH",
            format!("expected={:?} actual={inode:?}", sealed.artifact_inode),
        ));
    }
    let file = std::fs::File::open(&sealed.artifact).map_err(|error| {
        artifact_observe_error(
            "L0_REPORT_READBACK_FAILED",
            format!("path={} error={error}", sealed.artifact.display()),
        )
    })?;
    let sha256 = hash_sha256_reader(&sealed.artifact, file)
        .map_err(|error| artifact_observe_error("L0_REPORT_DIGEST_FAILED", error.to_string()))?;
    Ok(ReportArtifactObservation {
        path: sealed.artifact.clone(),
        inode,
        sha256,
        write_bytes: sealed.write_bytes,
        readback_bytes: sealed.readback_bytes,
        file_fsynced: sealed.file_fsynced,
        parent_fsynced: sealed.parent_fsynced,
    })
}

/// Resolve the same first executable PATH hit that `command -v ompo` names,
/// then prove it is the installed inode/content/identity already accepted by
/// B12. Mtime is intentionally absent: ordering is not provenance.
pub fn observe_installed_ompo(
    path_env: &str,
    expected_path: &Path,
    expected_identity: &IdentityCheck,
    manifest: &InputManifest,
) -> Result<InstalledArtifactObservation, ObserveGateError> {
    let digest = full_observe_manifest_digest(manifest)?;
    let observed_path = path_collision_hits("ompo", path_env)
        .into_iter()
        .find(|path| std::fs::metadata(path).is_ok_and(|metadata| is_executable(&metadata)))
        .ok_or_else(|| {
            artifact_observe_error(
                "L0_INSTALLED_PATH_MISSING",
                "command-v ompo found no executable PATH entry",
            )
        })?;
    let expected_metadata = std::fs::metadata(expected_path).map_err(|error| {
        artifact_observe_error(
            "L0_INSTALLED_EXPECTED_MISSING",
            format!("path={} error={error}", expected_path.display()),
        )
    })?;
    let observed_metadata = std::fs::metadata(&observed_path).map_err(|error| {
        artifact_observe_error(
            "L0_INSTALLED_METADATA_FAILED",
            format!("path={} error={error}", observed_path.display()),
        )
    })?;
    let expected_inode = metadata_inode(&expected_metadata);
    let observed_inode = metadata_inode(&observed_metadata);
    let same_path = expected_path.canonicalize().ok() == observed_path.canonicalize().ok();
    if expected_inode != observed_inode || !same_path {
        return Err(artifact_observe_error(
            "L0_INSTALLED_INODE_MISMATCH",
            format!(
                "expected_path={} observed_path={} expected_inode={expected_inode:?} observed_inode={observed_inode:?}",
                expected_path.display(),
                observed_path.display()
            ),
        ));
    }
    let sha256 = verify_sha256(&observed_path, Some(digest)).map_err(|error| {
        artifact_observe_error("L0_INSTALLED_CONTENT_MISMATCH", error.to_string())
    })?;
    let identity = verify_identity(
        &observed_path,
        &expected_identity.head_sha,
        &expected_identity.repo_ownership,
    );
    if !identity.consistent || identity != *expected_identity {
        return Err(artifact_observe_error(
            "L0_INSTALLED_IDENTITY_MISMATCH",
            format!("expected={expected_identity:?} actual={identity:?}"),
        ));
    }
    Ok(InstalledArtifactObservation {
        path: observed_path,
        inode: observed_inode,
        sha256,
        identity,
    })
}

/// L0-B15-obs-gate (bead fx3d): the exact success verdict line. Production
/// prints this bare word and nothing else on the composed-gate success
/// path; legs pin the literal so a reworded verdict reddens instead of
/// drifting. A refusing gate never yields this line: every refusal
/// returns before the success guard that prints it.
pub const GATE_OK_VERDICT: &str = "GATE_OK";
/// L0-B15 observability gate: event -> monitor -> gate verdict.
///
/// After the report seals, this re-reads the L0 journal through the
/// existing monitor (`observe_layer`), independently re-verifies the
/// artifact (`verify_artifact`), and consumes the freshness gate
/// (`gate_freshness_verdict`) before any success verdict. Every stage
/// names itself: journal/read failures blame EVENT; a non-progressing
/// layer blames MONITOR; a failed freshness gate blames GATE. Nothing
/// here duplicates an existing channel: the emit path stays in the
/// installer's emitter, the metric path stays in `lifecycle-monitor`,
/// and this is the only composition of the three. The manifest travels
/// by reference so every channel observes the same declared input set.
/// Called by `run_install` after the report seals; integration legs
/// drive it directly with hermetic journals.
pub fn gate_observability(
    repo_root: &Path,
    manifest: &InputManifest,
) -> Result<ObserveGate, ObserveGateError> {
    let journal = default_repo_journal(repo_root);
    let manifest_text = manifest.to_string();
    // INPUT: the gate verdicts only over a complete input set. A PARTIAL
    // or REFUSED manifest refuses here with its own typed reason before
    // any channel is consulted -- verdicting over undeclared input would
    // launder it into a clean gate.
    let _ = full_observe_manifest_digest(manifest)?;
    let event_rows = verify_artifact(&journal).map_err(|error| ObserveGateError {
        stage: ObserveStage::Event,
        reason: format!("{error} manifest={manifest_text}"),
    })?;
    let verdict = observe_layer(&journal, Layer::L0, L0_OBSERVE_STALL_MS).map_err(|error| {
        ObserveGateError {
            stage: ObserveStage::Monitor,
            reason: format!("{error} manifest={manifest_text}"),
        }
    })?;
    let last_ts = verdict.last_ts.ok_or_else(|| ObserveGateError {
        stage: ObserveStage::Monitor,
        reason: "L0_MONITOR_TIMESTAMP_UNAVAILABLE".to_owned(),
    })?;
    let freshness_threshold_ms =
        verdict
            .freshness_threshold_ms
            .ok_or_else(|| ObserveGateError {
                stage: ObserveStage::Monitor,
                reason: "L0_MONITOR_THRESHOLD_UNAVAILABLE".to_owned(),
            })?;
    // GATE: freshness consumes the verdict before any success verdict.
    gate_freshness_verdict(std::slice::from_ref(&verdict)).map_err(|error| {
        ObserveGateError {
            stage: ObserveStage::Gate,
            reason: format!("{error} manifest={manifest_text}"),
        }
    })?;
    Ok(ObserveGate {
        rows: event_rows,
        last_ts,
        age_ms: verdict.age_ms,
        freshness_threshold_ms,
        fresh: verdict.fresh,
    })
}


fn verify_metric_event(
    repo_root: &Path,
    metrics: &InstallMetrics,
    durability_metric: MetricDelta,
) -> Result<(), ObserveGateError> {
    let journal = default_repo_journal(repo_root);
    let text = std::fs::read_to_string(&journal).map_err(|error| ObserveGateError {
        stage: ObserveStage::Gate,
        reason: format!(
            "INSTALL_METRIC_UNMEASURABLE reason=WRITER_ABSENT path={} detail={error}",
            journal.display()
        ),
    })?;
    let event = text
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|value| {
            value.get("layer").and_then(serde_json::Value::as_str) == Some("L0")
                && value
                    .get("reason_code")
                    .and_then(serde_json::Value::as_str)
                    == Some("INSTALL_VERIFIED")
        })
        .ok_or_else(|| ObserveGateError {
            stage: ObserveStage::Gate,
            reason: "INSTALL_METRIC_UNMEASURABLE reason=WRITER_ABSENT event=INSTALL_VERIFIED"
                .to_owned(),
        })?;
    if event.get("attempt").and_then(serde_json::Value::as_str)
        != Some(metrics.attempt_identity.attempt.as_str())
    {
        return Err(ObserveGateError {
            stage: ObserveStage::Gate,
            reason: InstallMetricUnmeasurable::AttemptIdentityMissing.to_string(),
        });
    }
    let actual = event
        .get("metrics")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| ObserveGateError {
            stage: ObserveStage::Gate,
            reason: "INSTALL_METRIC_UNMEASURABLE reason=WRITER_ABSENT field=event.metrics"
                .to_owned(),
        })?;
    let mut expected = Vec::with_capacity(metrics.deltas.len() + 1);
    expected.extend(
        metrics
            .deltas
            .iter()
            .copied()
            .map(MetricDelta::to_json_value),
    );
    expected.push(durability_metric.to_json_value());
    if actual != &expected {
        return Err(ObserveGateError {
            stage: ObserveStage::Gate,
            reason: "INSTALL_METRIC_UNMEASURABLE reason=READBACK_FAILED field=event.metrics"
                .to_owned(),
        });
    }
    Ok(())
}

/// The production B15 consumer: a previously correlated canonical report
/// becomes part of the same event/monitor/freshness verdict, not a second gate.
pub fn gate_correlated_observability(
    repo_root: &Path,
    manifest: &InputManifest,
    report: CorrelatedInstallReport,
    request: InstallObserveRequest<'_>,
) -> Result<InstallObserveGate, ObserveGateError> {
    let gate = gate_observability(repo_root, manifest)?;
    let report_artifact = observe_report_artifact(request.sealed)?;
    let manifest_digest = full_observe_manifest_digest(manifest)?.to_owned();
    let now_ms = current_time_ms().ok_or_else(|| ObserveGateError {
        stage: ObserveStage::Gate,
        reason: InstallMetricUnmeasurable::TimestampMissing {
            field: "observer_now_ms",
        }
        .to_string(),
    })?;
    let metric_report = read_install_metric_deltas(repo_root, now_ms).map_err(|error| {
        ObserveGateError {
            stage: ObserveStage::Gate,
            reason: error.to_string(),
        }
    })?;
    let durability_metric = metric_report.durability_metric;
    verify_metric_event(repo_root, &metric_report.metrics, durability_metric)?;
    if report.metric_count != metric_report.metrics.deltas.len() + 1 {
        return Err(ObserveGateError {
            stage: ObserveStage::Gate,
            reason: "INSTALL_METRIC_UNMEASURABLE reason=CORRELATED_COUNT_MISMATCH".to_owned(),
        });
    }
    let installed = if request.binary_name == "ompo" {
        Some(observe_installed_ompo(
            request.path_env,
            request.expected_path,
            request.expected_identity,
            manifest,
        )?)
    } else {
        None
    };
    Ok(InstallObserveGate {
        report: report_artifact,
        installed,
        manifest_digest,
        path_hits: report.path_hits,
        host_capabilities: report.host_capabilities,
        rows: gate.rows,
        last_ts: gate.last_ts,
        age_ms: gate.age_ms,
        metrics: metric_report.metrics,
        durability_metric,
        metric_age_ms: metric_report.age_ms,
        metric_fresh: metric_report.fresh,
        freshness_threshold_ms: gate.freshness_threshold_ms,
        fresh: gate.fresh,
    })
}
fn render_install_observation(gate: &InstallObserveGate) -> String {
    let (installed_path, installed_inode, installed_sha256, installed_identity) = gate
        .installed
        .as_ref()
        .map(|installed| {
            (
                installed.path.display().to_string(),
                format!("{:?}", installed.inode),
                installed.sha256.as_str(),
                installed.identity.consistent,
            )
        })
        .unwrap_or_else(|| ("not-required".to_owned(), "None".to_owned(), "none", true));
    let [duration, path_hits, backup_ratio] = gate.metrics.deltas;
    let mut rendered = format!(
        "  OBSERVE rows={} last_ts={} age_ms={} freshness_threshold_ms={} fresh={} manifest_state=FULL manifest_digest={} report_path={} report_inode={:?} report_sha256={} report_bytes={} report_file_fsynced={} report_parent_fsynced={} path_hits={} host_capabilities={} installed_path={} installed_inode={} installed_sha256={} installed_identity={} metric_verdict={} metric_age_ms={} metric_fresh={} install_started_at_ms={} verified_path_at_ms={} duration_observed={} duration_expected={} duration_threshold={} duration_delta={} path_hits_observed={} path_hits_delta={} backups_written={:?} files_mutated={:?} backup_ratio_ppm={} backup_ratio_delta={}",
        gate.rows,
        gate.last_ts,
        gate.age_ms,
        gate.freshness_threshold_ms,
        gate.fresh,
        gate.manifest_digest,
        gate.report.path.display(),
        gate.report.inode,
        gate.report.sha256,
        gate.report.readback_bytes,
        gate.report.file_fsynced,
        gate.report.parent_fsynced,
        gate.path_hits,
        gate.host_capabilities,
        installed_path,
        installed_inode,
        installed_sha256,
        installed_identity,
        gate.metrics.overall_verdict().as_str(),
        gate.metric_age_ms,
        gate.metric_fresh,
        gate.metrics.started_at_ms,
        gate.metrics.verified_path_at_ms,
        duration.observed,
        duration.expectation.expected,
        duration.threshold.tolerance,
        duration.delta,
        path_hits.observed,
        path_hits.delta,
        backup_ratio.numerator,
        backup_ratio.denominator,
        backup_ratio.observed,
        backup_ratio.delta,
    );
    write!(
        rendered,
        " durability_verdict={} durability_coverage_ppm={} parent_fsync_successes={:?} atomic_rename_attempts={:?}",
        gate.durability_metric.verdict.as_str(),
        gate.durability_metric.observed,
        gate.durability_metric.numerator,
        gate.durability_metric.denominator,
    )
    .expect("writing to String cannot fail");
    rendered
}

fn metric_red_exit(
    repo_root: &Path,
    identity: &AttemptIdentity,
    manifest: &InputManifest,
    gate: &InstallObserveGate,
) -> Option<ExitCode> {
    if gate.metrics.overall_verdict() == MetricDeltaVerdict::Pass
        && gate.durability_metric.verdict == MetricDeltaVerdict::Pass
    {
        return None;
    }
    eprintln!(
        "INSTALLER METRIC RED: {} durability={} parent_fsync_successes={:?} atomic_rename_attempts={:?}",
        gate.metrics.record_digest,
        gate.durability_metric.verdict.as_str(),
        gate.durability_metric.numerator,
        gate.durability_metric.denominator,
    );
    let _ = emit_refusal(
        repo_root,
        Layer::L0,
        "S1.L0",
        "INSTALL_OBSERVE_REFUSED",
        identity,
        manifest,
    );
    Some(ExitCode::from(1))
}

/// Final L0 observability decision used by the production installer and its
/// integration tests. A failed gate emits the refusal event and returns 1
/// before the sole success-printing guard can run.
pub fn guard_observability_success(
    repo_root: &Path,
    identity: &AttemptIdentity,
    manifest: &InputManifest,
    readback: lifecycle_event::Readback,
    gate: Result<InstallObserveGate, ObserveGateError>,
) -> ExitCode {
    match gate {
        Ok(gate) => {
            println!("{}", render_install_observation(&gate));
            if let Some(exit) = metric_red_exit(repo_root, identity, manifest, &gate) {
                return exit;
            }
        }
        Err(error) => {
            eprintln!("INSTALLER OBSERVE REFUSED: {error}");
            let _ = emit_refusal(
                repo_root,
                Layer::L0,
                "S1.L0",
                "INSTALL_OBSERVE_REFUSED",
                identity,
                manifest,
            );
            return ExitCode::from(1);
        }
    }
    guard_success(Ok(readback), GATE_OK_VERDICT)
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

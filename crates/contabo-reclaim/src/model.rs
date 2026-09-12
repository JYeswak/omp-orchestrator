#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Component, Path, PathBuf};

pub const WORKERS: &[WorkerSpec] = &[
    WorkerSpec {
        id: "contabo-1",
        host: "89.117.22.43",
    },
    WorkerSpec {
        id: "contabo-2",
        host: "94.72.121.46",
    },
    WorkerSpec {
        id: "contabo-3",
        host: "94.72.121.48",
    },
    WorkerSpec {
        id: "contabo-4",
        host: "94.72.122.112",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerSpec {
    pub id: &'static str,
    pub host: &'static str,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerSelection {
    One(WorkerSpec),
    All,
}

pub fn worker_by_id(id: &str) -> Result<WorkerSpec, ReclaimError> {
    WORKERS
        .iter()
        .copied()
        .find(|worker| worker.id == id)
        .ok_or_else(|| ReclaimError::InvalidWorker {
            worker: id.to_owned(),
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReclaimMode {
    DryRun,
    Apply,
}

impl ReclaimMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DryRun => "dry-run",
            Self::Apply => "apply",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Directory,
    Symlink,
    Other,
}

impl EntryKind {
    pub fn parse(value: &str) -> Result<Self, ReclaimError> {
        match value {
            "d" => Ok(Self::Directory),
            "l" => Ok(Self::Symlink),
            other => Err(ReclaimError::MalformedListing {
                detail: format!("unsupported entry type {other:?}"),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WhitelistRule {
    RchTarget,
    RchTargetPool,
    RchTmp,
    Mutation,
    Grade,
    DotGrade,
}

impl WhitelistRule {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RchTarget => ".rch-target",
            Self::RchTargetPool => ".rch-target-*",
            Self::RchTmp => ".rch-tmp",
            Self::Mutation => "*-mut",
            Self::Grade => "grade-*",
            Self::DotGrade => ".grade-*",
        }
    }

    pub fn matches_basename(self, basename: &str) -> bool {
        match self {
            Self::RchTarget => basename == ".rch-target",
            Self::RchTargetPool => basename
                .strip_prefix(".rch-target-")
                .is_some_and(|suffix| !suffix.is_empty()),
            Self::RchTmp => basename == ".rch-tmp",
            Self::Mutation => basename
                .strip_suffix("-mut")
                .is_some_and(|prefix| !prefix.is_empty()),
            Self::Grade => basename
                .strip_prefix("grade-")
                .is_some_and(|suffix| !suffix.is_empty()),
            Self::DotGrade => basename
                .strip_prefix(".grade-")
                .is_some_and(|suffix| !suffix.is_empty()),
        }
    }
}

pub const WHITELIST: &[WhitelistRule] = &[
    WhitelistRule::RchTarget,
    WhitelistRule::RchTargetPool,
    WhitelistRule::RchTmp,
    WhitelistRule::Mutation,
    WhitelistRule::Grade,
    WhitelistRule::DotGrade,
];

pub fn basename_is_whitelisted(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|basename| WHITELIST.iter().any(|rule| rule.matches_basename(basename)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub path: PathBuf,
    pub kind: EntryKind,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedCandidate {
    pub candidate: Candidate,
    pub rule: WhitelistRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefusalReason {
    BaseNotAbsolute,
    BaseItself,
    ParentComponent,
    OutsideBase,
    NotWhitelisted,
    SymlinkEscape,
    SymlinkTargetUnreadable,
    UnsupportedEntry,
}

impl RefusalReason {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::BaseNotAbsolute => "BASE_NOT_ABSOLUTE",
            Self::BaseItself => "BASE_ITSELF",
            Self::ParentComponent => "PARENT_COMPONENT",
            Self::OutsideBase => "OUTSIDE_BASE",
            Self::NotWhitelisted => "NOT_WHITELISTED",
            Self::SymlinkEscape => "SYMLINK_ESCAPE",
            Self::SymlinkTargetUnreadable => "SYMLINK_TARGET_UNREADABLE",
            Self::UnsupportedEntry => "UNSUPPORTED_ENTRY",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReclaimRefusal {
    pub path: PathBuf,
    pub reason: RefusalReason,
    pub detail: String,
}

impl fmt::Display for ReclaimRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "RECLAIM_REFUSED path={} reason={} detail={}",
            self.path.display(),
            self.reason.as_str(),
            self.detail
        )
    }
}
impl ReclaimRefusal {
    pub const fn exit_code(&self) -> u8 {
        1
    }
}

pub fn validate_candidate(
    base: &Path,
    candidate: Candidate,
    resolved_target: Option<&Path>,
) -> Result<ValidatedCandidate, ReclaimRefusal> {
    if !base.is_absolute() {
        return Err(ReclaimRefusal {
            path: candidate.path,
            reason: RefusalReason::BaseNotAbsolute,
            detail: format!("base={} must be absolute", base.display()),
        });
    }
    if candidate.path == base {
        return Err(ReclaimRefusal {
            path: candidate.path,
            reason: RefusalReason::BaseItself,
            detail: "the configured base itself is never a candidate".to_owned(),
        });
    }
    if candidate
        .path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(ReclaimRefusal {
            path: candidate.path,
            reason: RefusalReason::ParentComponent,
            detail: "parent traversal is refused before normalization".to_owned(),
        });
    }
    if !candidate.path.starts_with(base) {
        return Err(ReclaimRefusal {
            path: candidate.path,
            reason: RefusalReason::OutsideBase,
            detail: format!("base={}", base.display()),
        });
    }
    let Some(basename) = candidate
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
    else {
        return Err(ReclaimRefusal {
            path: candidate.path,
            reason: RefusalReason::NotWhitelisted,
            detail: "candidate has no UTF-8 basename".to_owned(),
        });
    };
    let Some(rule) = WHITELIST
        .iter()
        .copied()
        .find(|rule| rule.matches_basename(&basename))
    else {
        return Err(ReclaimRefusal {
            path: candidate.path,
            reason: RefusalReason::NotWhitelisted,
            detail: format!("basename={basename} allowed={:?}", WHITELIST),
        });
    };
    match candidate.kind {
        EntryKind::Directory => {}
        EntryKind::Symlink => {
            let Some(resolved_target) = resolved_target else {
                return Err(ReclaimRefusal {
                    path: candidate.path,
                    reason: RefusalReason::SymlinkTargetUnreadable,
                    detail: "realpath did not produce a target".to_owned(),
                });
            };
            if !resolved_target.starts_with(base) {
                return Err(ReclaimRefusal {
                    path: candidate.path,
                    reason: RefusalReason::SymlinkEscape,
                    detail: format!("resolved_target={}", resolved_target.display()),
                });
            }
        }
        EntryKind::Other => {
            return Err(ReclaimRefusal {
                path: candidate.path,
                reason: RefusalReason::UnsupportedEntry,
                detail: "only directories and symlinks are candidates".to_owned(),
            })
        }
    }
    Ok(ValidatedCandidate { candidate, rule })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveBuild {
    pub id: String,
    pub worker_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlSnapshot {
    pub worker_id: String,
    pub worker_host: String,
    pub worker_status: String,
    pub used_slots: u64,
    pub active_builds: Vec<ActiveBuild>,
}

impl ControlSnapshot {
    pub fn matches_worker(&self, worker: WorkerSpec) -> bool {
        self.worker_id == worker.id && self.worker_host == worker.host
    }

    pub fn is_clear(&self) -> bool {
        self.worker_status == "healthy" && self.used_slots == 0 && self.active_builds.is_empty()
    }

    pub fn is_busy(&self) -> bool {
        self.used_slots > 0 || !self.active_builds.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteProcessObservation {
    Empty,
    Present { lines: Vec<String> },
    Unavailable { detail: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardDecision {
    Authorized,
    SkippedLiveBuild {
        detail: String,
        active_build_ids: Vec<String>,
    },
    Unknown {
        detail: String,
    },
    Unreachable {
        detail: String,
    },
}

pub fn decide_guards(
    control: &ControlSnapshot,
    remote: &RemoteProcessObservation,
) -> GuardDecision {
    match (control.is_clear(), control.is_busy(), remote) {
        (true, false, RemoteProcessObservation::Empty) => GuardDecision::Authorized,
        (false, true, RemoteProcessObservation::Present { lines })
            if control.worker_status == "healthy" =>
        {
            GuardDecision::SkippedLiveBuild {
                detail: format!(
                    "worker={} used_slots={} active_builds={} active_build_ids={:?} remote_processes={}",
                    control.worker_id,
                    control.used_slots,
                    control.active_builds.len(),
                    control.active_builds.iter().map(|build| &build.id).collect::<Vec<_>>(),
                    lines.len()
                ),
                active_build_ids: control.active_builds.iter().map(|build| build.id.clone()).collect(),
            }
        }
        (_, _, RemoteProcessObservation::Unavailable { detail }) => GuardDecision::Unreachable {
            detail: detail.clone(),
        },
        (false, false, _) if control.worker_status != "healthy" => GuardDecision::Unknown {
            detail: format!(
                "control-plane worker status={} is not healthy",
                control.worker_status
            ),
        },
        (true, false, RemoteProcessObservation::Present { lines }) => GuardDecision::Unknown {
            detail: format!(
                "control-plane clear but remote process probe found {} cargo/rustc process(es)",
                lines.len()
            ),
        },
        (false, true, RemoteProcessObservation::Empty) => GuardDecision::Unknown {
            detail: "control-plane busy but remote cargo/rustc process probe was empty".to_owned(),
        },
        (_, _, _) => GuardDecision::Unknown {
            detail: "dual live-build authorities did not agree on a safe clear state".to_owned(),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateSet {
    Empty,
    NonEmpty(Vec<Candidate>),
}

pub fn parse_listing(text: &str) -> Result<CandidateSet, ReclaimError> {
    let mut candidates = Vec::new();
    for (line_number, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let mut fields = line.split('\t');
        let path = fields.next().filter(|value| !value.is_empty());
        let kind = fields.next().filter(|value| !value.is_empty());
        let bytes = fields.next().filter(|value| !value.is_empty());
        if fields.next().is_some() || path.is_none() || kind.is_none() || bytes.is_none() {
            return Err(ReclaimError::MalformedListing {
                detail: format!("line={} expected path\\tkind\\tbytes", line_number + 1),
            });
        }
        let kind = EntryKind::parse(kind.expect("checked above"))?;
        let bytes = bytes
            .expect("checked above")
            .parse::<u64>()
            .map_err(|error| ReclaimError::MalformedListing {
                detail: format!("line={} invalid bytes: {error}", line_number + 1),
            })?;
        candidates.push(Candidate {
            path: PathBuf::from(path.expect("checked above")),
            kind,
            bytes,
        });
    }
    if candidates.is_empty() {
        Ok(CandidateSet::Empty)
    } else {
        Ok(CandidateSet::NonEmpty(candidates))
    }
}

#[derive(Debug)]
pub enum ReclaimError {
    InvalidWorker {
        worker: String,
    },
    MultipleWorkers,
    MissingSelection,
    EmptyFleetReport,
    InvalidBase {
        detail: String,
    },
    MalformedStatus {
        detail: String,
    },
    MalformedListing {
        detail: String,
    },
    Probe {
        worker: String,
        detail: String,
    },
    Timeout {
        worker: String,
        operation: &'static str,
    },
    Output {
        worker: String,
        operation: &'static str,
        detail: String,
    },
    Runtime {
        detail: String,
    },
}
impl fmt::Display for ReclaimError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidWorker { worker } => {
                write!(formatter, "CONTABO_RECLAIM_INVALID_WORKER worker={worker}")
            }
            Self::MultipleWorkers => formatter.write_str(
                "CONTABO_RECLAIM_USAGE reason=ONE_SELECTION_REQUIRED; choose exactly one --worker contabo-N or --all-workers",
            ),
            Self::MissingSelection => formatter.write_str(
                "CONTABO_RECLAIM_USAGE reason=SELECTION_REQUIRED; choose exactly one --worker contabo-N or --all-workers",
            ),
            Self::EmptyFleetReport => formatter.write_str(
                "CONTABO_RECLAIM_FLEET_ERROR reason=EMPTY_REPORT_SET",
            ),
            Self::InvalidBase { detail } => {
                write!(formatter, "CONTABO_RECLAIM_INVALID_BASE {detail}")
            }
            Self::MalformedStatus { detail } => {
                write!(formatter, "CONTABO_RECLAIM_CONTROL_MALFORMED {detail}")
            }
            Self::MalformedListing { detail } => {
                write!(formatter, "CONTABO_RECLAIM_LISTING_MALFORMED {detail}")
            }
            Self::Probe { worker, detail } => {
                write!(formatter, "CONTABO_RECLAIM_PROBE_FAILED worker={worker} {detail}")
            }
            Self::Timeout { worker, operation } => write!(
                formatter,
                "CONTABO_RECLAIM_TIMEOUT worker={worker} operation={operation}"
            ),
            Self::Output {
                worker,
                operation,
                detail,
            } => write!(
                formatter,
                "CONTABO_RECLAIM_COMMAND_FAILED worker={worker} operation={operation} detail={detail}"
            ),
            Self::Runtime { detail } => write!(formatter, "CONTABO_RECLAIM_RUNTIME {detail}"),
        }
    }
}

impl std::error::Error for ReclaimError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RunOutcome {
    Planned,
    Reclaimed,
    SkippedLiveBuild,
    Unknown,
    Unreachable,
    Refused,
    /// The liveness oracle answered empty (leg 1B/1C): an empty oracle says
    /// "nothing is live", which in apply mode deletes every export cache.
    OracleRefused,
    /// The reclaimed counter lied about the filesystem (leg 2).
    IntegrityRefused,
    /// The sweep evaluated nothing successfully (leg 6, sweep half).
    Vacuous,
    /// Named deletes failed (leg 3).
    DeleteFailed,
}

impl RunOutcome {
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::Planned | Self::Reclaimed | Self::SkippedLiveBuild => 0,
            Self::Refused => 1,
            Self::Unknown | Self::Unreachable | Self::OracleRefused => 2,
            Self::IntegrityRefused => 3,
            Self::Vacuous => 4,
            Self::DeleteFailed => 5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReclaimReport {
    pub schema: &'static str,
    pub worker: String,
    pub host: String,
    pub mode: ReclaimModeWire,
    pub outcome: RunOutcome,
    pub active_build_ids: Vec<String>,
    pub guards: Vec<String>,
    pub candidates: Vec<String>,
    pub refused: Vec<String>,
    pub bytes: u64,
    pub directories: usize,
    /// Kept by the twin gate (leg 4): export caches whose Mac twin is live.
    pub kept: usize,
    /// Named delete failures, one line per failed delete (leg 3).
    pub failures: Vec<String>,
    /// Deletes skipped because the path was already absent (leg 5): never
    /// booked as successes, counted here instead.
    pub absent_before: usize,
    /// One row per twin-gated decision (leg 4), carrying the instant and the
    /// twin state observed then. The verifier input; an ok report with zero
    /// rows is vacuous, never clean (leg 6, verifier half).
    pub decision_rows: Vec<LeaseDecision>,
    pub detail: String,
}

/// The outcome for an empty candidate listing (legs 1B + 6, sweep half).
/// A one-line pin so the decision "empty means vacuous exit 4, not clean
/// exit 0" is a legged value rather than a comment at the call site: the
/// mutation that softens it reddens the leg below.
#[must_use]
pub const fn vacuous_listing_outcome() -> RunOutcome {
    RunOutcome::Vacuous
}

/// The reconciliation identity (leg 5): every evaluated candidate lands in
/// exactly one bucket. The short form `evaluated == dirs + kept` holds only
/// when refused, failures, and absent are all zero, and coinciding on real
/// boxes is not an invariant.

pub fn evaluated_total(report: &ReclaimReport) -> usize {
    report.directories
        + report.kept
        + report.refused.len()
        + report.failures.len()
        + report.absent_before
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReclaimModeWire {
    DryRun,
    Apply,
}

impl From<ReclaimMode> for ReclaimModeWire {
    fn from(mode: ReclaimMode) -> Self {
        match mode {
            ReclaimMode::DryRun => Self::DryRun,
            ReclaimMode::Apply => Self::Apply,
        }
    }
}

impl ReclaimReport {
    pub fn new(worker: WorkerSpec, mode: ReclaimMode) -> Self {
        Self {
            schema: "contabo-reclaim/report-v1",
            worker: worker.id.to_owned(),
            host: worker.host.to_owned(),
            mode: mode.into(),
            outcome: RunOutcome::Unknown,
            active_build_ids: Vec::new(),
            guards: Vec::new(),
            candidates: Vec::new(),
            refused: Vec::new(),
            bytes: 0,
            directories: 0,
            kept: 0,
            failures: Vec::new(),
            absent_before: 0,
            decision_rows: Vec::new(),
            detail: String::new(),
        }
    }

    pub fn error(worker: WorkerSpec, mode: ReclaimMode, detail: String) -> Self {
        let mut report = Self::new(worker, mode);
        report.detail = format!("WORKER_ERROR {detail}");
        report
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FleetOutcome {
    Complete,
    DeferredActiveBuild,
    Incomplete,
    /// A member's reclaimed counter disagreed with the filesystem (leg 2).
    IntegrityRefused,
    /// The fleet evaluated nothing successfully (leg 6, verifier half).
    Vacuous,
    /// Named deletes failed on at least one member (leg 3).
    DeleteFailed,
}

impl FleetOutcome {
    /// Whole-sweep process exits, shell-conformant (bead 4bem8 legs 2/3/6).
    /// `DeferredActiveBuild` moved 3 -> 2: deferral is "did not finish",
    /// and exit 3 now means exactly one thing -- the counter lied.
    /// Severity order, not arrival order: a lied counter outranks named
    /// failures, which outrank unknowns, which outrank vacuity.
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::Complete => 0,
            Self::DeferredActiveBuild => 2,
            Self::Incomplete => 2,
            Self::IntegrityRefused => 3,
            Self::Vacuous => 4,
            Self::DeleteFailed => 5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetReport {
    pub schema: &'static str,
    pub mode: ReclaimModeWire,
    pub outcome: FleetOutcome,
    pub workers: Vec<ReclaimReport>,
    pub deferred_active_workers: usize,
    pub incomplete_workers: usize,
    pub integrity_refused_workers: usize,
    pub vacuous_workers: usize,
    pub delete_failed_workers: usize,
    pub detail: String,
}

impl FleetReport {
    pub fn from_reports(
        mode: ReclaimMode,
        workers: Vec<ReclaimReport>,
    ) -> Result<Self, ReclaimError> {
        if workers.is_empty() {
            return Err(ReclaimError::EmptyFleetReport);
        }
        let mut deferred_active_workers = 0;
        let mut incomplete_workers = 0;
        let mut integrity_refused_workers = 0;
        let mut vacuous_workers = 0;
        let mut delete_failed_workers = 0;
        let mut evaluated_sum = 0usize;
        for report in &workers {
            evaluated_sum += evaluated_total(report);
            match report.outcome {
                RunOutcome::Planned | RunOutcome::Reclaimed => {}
                RunOutcome::SkippedLiveBuild => deferred_active_workers += 1,
                RunOutcome::Unknown
                | RunOutcome::Unreachable
                | RunOutcome::Refused
                | RunOutcome::OracleRefused => incomplete_workers += 1,
                RunOutcome::IntegrityRefused => integrity_refused_workers += 1,
                RunOutcome::Vacuous => vacuous_workers += 1,
                RunOutcome::DeleteFailed => delete_failed_workers += 1,
            }
        }
        let outcome = if integrity_refused_workers > 0 {
            FleetOutcome::IntegrityRefused
        } else if delete_failed_workers > 0 {
            FleetOutcome::DeleteFailed
        } else if incomplete_workers > 0 {
            FleetOutcome::Incomplete
        } else if vacuous_workers > 0 || evaluated_sum == 0 {
            // Every member is quiet and nothing was evaluated anywhere: an
            // all-quiet fleet that reports success evaluated nothing, which
            // is the vacuity leg 6 exists to catch.
            FleetOutcome::Vacuous
        } else if deferred_active_workers > 0 {
            FleetOutcome::DeferredActiveBuild
        } else {
            FleetOutcome::Complete
        };
        let detail = format!(
            "workers={} evaluated={} deferred_active_workers={} incomplete_workers={} \
             integrity_refused_workers={} vacuous_workers={} delete_failed_workers={}",
            workers.len(),
            evaluated_sum,
            deferred_active_workers,
            incomplete_workers,
            integrity_refused_workers,
            vacuous_workers,
            delete_failed_workers,
        );
        Ok(Self {
            schema: "contabo-reclaim/fleet-report-v1",
            mode: mode.into(),
            outcome,
            workers,
            deferred_active_workers,
            incomplete_workers,
            integrity_refused_workers,
            vacuous_workers,
            delete_failed_workers,
            detail,
        })
    }

    pub const fn exit_code(&self) -> u8 {
        self.outcome.exit_code()
    }
}

// ===== Conformance port, bead omp-orchestrator-4bem8 =====
//
// Every item below was EXECUTED against the shell reclaimer on a constructed
// fixture, so each leg has an observed oracle rather than a description.
// Pure decision functions over injected observations, mirroring the
// decide_guards split: readers (ssh df, local twin stat, remote test -e)
// live in probe.rs; what the observations MEAN lives here, unit-legged.

/// df-vs-counter tolerance in MiB (leg 2). Both operands are truncated
/// integer divisions by 1024, so rounding alone permits 2 MiB; the worst
/// measured divergence across four real boxes was 1,1,0,1 MiB. 4 MiB is 2x
/// the structural bound and 4x the worst observation -- and it cannot hide
/// a real failure, because every reclaimable cache on this fleet is 1-18
/// GB. A PERCENTAGE TOLERANCE IS FORBIDDEN: 1% of a 30 GB sweep is 300 MB
/// and would hide a failed delete of a small cache.
pub const INTEGRITY_TOLERANCE_MB: u64 = 4;

/// The df-vs-counter verdict (leg 2). DIRECTIONAL: a counter that claims
/// more than the filesystem freed is a lie about our own work and refuses;
/// a filesystem that moved more than the counter is concurrent activity or
/// a partially-credited delete and warns, never refuses (refusing there
/// trains operators to ignore the gate -- measured on a real 5 MB move
/// against a zero counter).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrityVerdict {
    Agree { counter_mb: u64, df_mb: u64 },
    /// counter_mb > df_mb + tol: REFUSE, exit 3.
    CounterLie { counter_mb: u64, df_mb: u64 },
    /// df_mb > counter_mb + tol: WARN ONLY, naming both candidate causes.
    DfExcess {
        counter_mb: u64,
        df_mb: u64,
        failures: u64,
    },
}

/// Compare a reclaimed-bytes counter against df-observed movement. Inputs
/// are KiB (du -sk / df -Pk units); the MiB truncation is applied here so
/// the rounding bound is structural rather than caller discipline.
#[must_use]
pub fn check_integrity(counter_kb: u64, df_delta_kb: u64, failures: u64) -> IntegrityVerdict {
    let counter_mb = counter_kb / 1024;
    let df_mb = df_delta_kb / 1024;
    if counter_mb > df_mb + INTEGRITY_TOLERANCE_MB {
        IntegrityVerdict::CounterLie { counter_mb, df_mb }
    } else if df_mb > counter_mb + INTEGRITY_TOLERANCE_MB {
        IntegrityVerdict::DfExcess {
            counter_mb,
            df_mb,
            failures,
        }
    } else {
        IntegrityVerdict::Agree { counter_mb, df_mb }
    }
}
/// Render the DfExcess warning. Names BOTH candidate causes -- (a) our own
/// partially-successful delete freed uncredited bytes, (b) concurrent
/// activity on the box -- and marks (a) UNSUPPORTED when failures == 0.
/// Asserting one cause was a real defect in the shell version, false in
/// the very run that produced it.
#[must_use]
pub fn df_excess_warn_text(counter_mb: u64, df_mb: u64, failures: u64) -> String {
    format!(
        "INTEGRITY-WARN counter={counter_mb}MB df_delta={df_mb}MB failures={failures} \
         cause=(a)partially-successful-delete-freed-uncredited-bytes[{}] \
         (b)concurrent-activity-on-box",
        if failures == 0 { "UNSUPPORTED" } else { "SUPPORTED" }
    )
}

/// One delete's outcome (legs 3 + 5). DELETED-BY-ME and ALREADY-ABSENT are
/// different states in the type system: `rm -rf` on an already-absent path
/// exits 0, so a post-hoc existence test books it as a success, and du of
/// an absent path is 0 KB -- and the df gate is structurally blind to it
/// (both arms read 0, agree, and are both correct). Existence is probed
/// BEFORE the delete, never inferred after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteOutcome {
    DeletedByMe { bytes: u64 },
    AlreadyAbsent,
    Failed { stderr: String },
}

/// The counter deltas one delete earns. Returned WITH the outcome from a
/// single call so the two cannot drift apart (leg 3: the pre-fix shape
/// incremented both counters on a suppressed rm).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteApplication {
    pub outcome: DeleteOutcome,
    pub bytes_delta: u64,
    pub dir_delta: usize,
    pub failure_line: Option<String>,
    pub absent_delta: usize,
}

/// Apply one delete observation to the counters. The delete's outcome and
/// the counter update are ONE operation: callers add the returned deltas,
/// they do not maintain counters beside the delete.
#[must_use]
pub fn apply_delete_observation(
    path: &str,
    existed_before: bool,
    rm_success: bool,
    stderr: &str,
    bytes: u64,
) -> DeleteApplication {
    if !existed_before {
        return DeleteApplication {
            outcome: DeleteOutcome::AlreadyAbsent,
            bytes_delta: 0,
            dir_delta: 0,
            failure_line: None,
            absent_delta: 1,
        };
    }
    if rm_success {
        return DeleteApplication {
            outcome: DeleteOutcome::DeletedByMe { bytes },
            bytes_delta: bytes,
            dir_delta: 1,
            failure_line: None,
            absent_delta: 0,
        };
    }
    DeleteApplication {
        outcome: DeleteOutcome::Failed {
            stderr: stderr.trim().to_owned(),
        },
        bytes_delta: 0,
        dir_delta: 0,
        failure_line: Some(format!(
            "DELETE-FAILED path={path} rc=1 stderr={}",
            stderr.trim()
        )),
        absent_delta: 0,
    }
}

/// Checkouts whose export caches are canonical (leg 4): a twin-present
/// export cache under one of these is live work, never swept.
pub const CANONICAL_CHECKOUTS: &[&str] = &["omp-orchestrator", "franken-harvest", "uds"];

/// Mac-side liveness of an export cache (leg 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TwinState {
    Present,
    Absent,
}

/// Read the Mac-side twin: present iff the path resolves to something live.
/// A missing path reads Absent; a DANGLING symlink reads Absent too -- the
/// link itself exists but its target does not, so no live grade stands
/// behind it. (`symlink_metadata` alone cannot see this: it stats the link,
/// which exists even when dangling. The leg below pins the distinction.)
#[must_use]
pub fn read_twin_state(path: &Path) -> TwinState {
    if std::fs::symlink_metadata(path).is_err() {
        return TwinState::Absent;
    }
    if std::fs::metadata(path).is_ok() {
        TwinState::Present
    } else {
        TwinState::Absent
    }
}

/// True iff the twin gate governs this rule (leg 4 scope rule). ONLY the
/// export-cache path (`.rch-tmp`) is twin-gated; regenerable build
/// artifacts (`.rch-target*`, `*-mut`, `grade-*`, `.grade-*`) are swept BY
/// KIND. Applying the twin predicate to build artifacts manufactures a
/// false violation on every correct sweep -- measured three false
/// LIVE-DESTROYED accusations on first contact with real data. Closed by
/// construction: a new rule defaults to false here until its twin need is
/// argued, not by matching a wider pattern.
#[must_use]
pub const fn twin_gate_applies(rule: WhitelistRule) -> bool {
    matches!(rule, WhitelistRule::RchTmp)
}

/// Keep-or-sweep for one export cache (leg 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SweepVerdict {
    Sweep { reason: String },
    Keep { reason: String },
}

/// Gate one export cache: canonical parents and live twins are kept, only
/// an absent twin under a non-canonical parent sweeps. A cache whose
/// Mac-side source no longer exists cannot be serving a live grade.
#[must_use]
pub fn gate_export_cache(parent_is_canonical: bool, twin: TwinState) -> SweepVerdict {
    if parent_is_canonical {
        SweepVerdict::Keep {
            reason: "canonical-checkout".to_owned(),
        }
    } else if twin == TwinState::Present {
        SweepVerdict::Keep {
            reason: "twin-present".to_owned(),
        }
    } else {
        SweepVerdict::Sweep {
            reason: "twin-absent".to_owned(),
        }
    }
}

/// One twin-gated decision, carrying the instant and the twin state
/// observed AT THAT INSTANT (leg 4). A keep/sweep decision is only correct
/// as of the instant it is taken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseDecision {
    pub path: PathBuf,
    pub sweep: bool,
    pub reason: String,
    pub epoch_secs: u64,
    pub twin_at_decision: TwinState,
}

/// What verification says about one past decision (leg 4). Classified on
/// REASON before twin: a `.rch-target` inside a live checkout is exactly
/// what is supposed to die, so twin rows never govern build artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyVerdict {
    /// Swept on an absent twin: the sweep was legitimate.
    JustifiedSweep,
    /// Kept on a live twin that has since vanished: the Mac side raced us,
    /// NOT a violation. Occurred naturally twice on 2026-09-12.
    ReapedAfterDecision,
    /// Kept on an absent twin and still standing: the sweep the decision
    /// demanded never happened. MUST still fire (mandatory known-good):
    /// a discriminator that reclassifies every violation as benign makes
    /// the accusation it exists to make unmakeable.
    AbsentViolation,
    /// Swept despite a keep, or swept on a live twin: destroyed live work.
    LiveDestroyed,
    /// Decided sweep, never swept: pending, not a verdict.
    NotSwept,
    /// Kept and still correctly standing.
    ConfirmedKeep,
}

/// Verify one past twin-gated decision against the twin state now.
#[must_use]
pub fn verify_sweep(
    decision: &LeaseDecision,
    twin_now: TwinState,
    was_swept: bool,
) -> VerifyVerdict {
    if was_swept {
        if !decision.sweep {
            return VerifyVerdict::LiveDestroyed;
        }
        return match decision.twin_at_decision {
            TwinState::Absent => VerifyVerdict::JustifiedSweep,
            TwinState::Present => VerifyVerdict::LiveDestroyed,
        };
    }
    if decision.sweep {
        return VerifyVerdict::NotSwept;
    }
    match (decision.twin_at_decision, twin_now) {
        (TwinState::Present, TwinState::Absent) => VerifyVerdict::ReapedAfterDecision,
        (TwinState::Absent, TwinState::Absent) => VerifyVerdict::AbsentViolation,
        (TwinState::Present, TwinState::Present)
        | (TwinState::Absent, TwinState::Present) => VerifyVerdict::ConfirmedKeep,
    }
}

/// Refuse an empty decision-row set (leg 6, verifier half): a verifier
/// finding zero rows reports a typed refusal, never "0 violations". The
/// shell verifier's proof-line match is a start-of-line anchor; the
/// anti-vacuity gate converts a stale pattern from a silent clean into
/// a refusal. Structured rows make the fragile match unconstructible,
/// and this check makes an empty input unpassable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VacuousDecisions {
    pub detail: String,
}

/// Count the rows or refuse the empty set. Called on every sweep's own
/// rows as a self-check: an ok report with zero rows is vacuous.
pub fn check_decisions_nonvacuous(rows: &[LeaseDecision]) -> Result<usize, VacuousDecisions> {
    if rows.is_empty() {
        Err(VacuousDecisions {
            detail: "zero decision rows: vacuous, never clean".to_owned(),
        })
    } else {
        Ok(rows.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitelist_is_typed_and_does_not_accept_near_misses() {
        assert_eq!(
            WHITELIST
                .iter()
                .copied()
                .find(|rule| rule.matches_basename(".rch-target")),
            Some(WhitelistRule::RchTarget)
        );
        assert!(WHITELIST
            .iter()
            .all(|rule| !rule.matches_basename(".rch-target-")));
        assert!(WHITELIST
            .iter()
            .all(|rule| !rule.matches_basename("target")));
    }

    #[test]
    fn guard_authority_matrix_fails_closed_on_disagreement() {
        let clear = ControlSnapshot {
            worker_id: "contabo-2".to_owned(),
            worker_host: "94.72.121.46".to_owned(),
            worker_status: "healthy".to_owned(),
            used_slots: 0,
            active_builds: Vec::new(),
        };
        assert_eq!(
            decide_guards(&clear, &RemoteProcessObservation::Empty),
            GuardDecision::Authorized
        );
        assert!(matches!(
            decide_guards(
                &ControlSnapshot {
                    used_slots: 2,
                    active_builds: vec![ActiveBuild {
                        id: "build-1".to_owned(),
                        worker_id: "contabo-2".to_owned(),
                    }],
                    ..clear.clone()
                },
                &RemoteProcessObservation::Present {
                    lines: vec!["123 cargo".to_owned()]
                }
            ),
            GuardDecision::SkippedLiveBuild { .. }
        ));
        assert!(matches!(
            decide_guards(
                &clear,
                &RemoteProcessObservation::Present {
                    lines: vec!["123 rustc".to_owned()]
                }
            ),
            GuardDecision::Unknown { .. }
        ));
    }
}

#[cfg(test)]
mod conformance_tests {
    use super::*;
    use std::collections::BTreeSet;

    fn worker() -> WorkerSpec {
        WorkerSpec {
            id: "contabo-2",
            host: "94.72.121.46",
        }
    }

    fn report_with(outcome: RunOutcome) -> ReclaimReport {
        let mut report = ReclaimReport::new(worker(), ReclaimMode::Apply);
        report.outcome = outcome;
        report
    }

    #[test]
    fn shell_conformant_exits() {
        assert_eq!(RunOutcome::OracleRefused.exit_code(), 2);
        assert_eq!(RunOutcome::IntegrityRefused.exit_code(), 3);
        assert_eq!(RunOutcome::Vacuous.exit_code(), 4);
        assert_eq!(RunOutcome::DeleteFailed.exit_code(), 5);
        assert_eq!(RunOutcome::Refused.exit_code(), 1);
        assert_eq!(RunOutcome::Reclaimed.exit_code(), 0);
        // Deferred moved 3 -> 2: deferral is "did not finish", and exit 3
        // now means exactly one thing -- the counter lied.
        assert_eq!(RunOutcome::SkippedLiveBuild.exit_code(), 0);
        assert_eq!(FleetOutcome::DeferredActiveBuild.exit_code(), 2);
        assert_eq!(FleetOutcome::IntegrityRefused.exit_code(), 3);
        assert_eq!(FleetOutcome::Vacuous.exit_code(), 4);
        assert_eq!(FleetOutcome::DeleteFailed.exit_code(), 5);
    }

    #[test]
    fn integrity_agrees_on_equal_measures() {
        assert!(matches!(
            check_integrity(120 * 1024, 120 * 1024, 0),
            IntegrityVerdict::Agree { .. }
        ));
    }

    #[test]
    fn integrity_refuses_the_counter_lie() {
        // Positive control shape: counter 120 MB, df 0 MB.
        match check_integrity(120 * 1024, 0, 1) {
            IntegrityVerdict::CounterLie { counter_mb, df_mb } => {
                assert_eq!((counter_mb, df_mb), (120, 0));
            }
            other => panic!("counter lie must refuse, got {other:?}"),
        }
    }

    #[test]
    fn integrity_tolerance_is_4mb_not_a_percent() {
        // Boundary: 4 MB over reads Agree, 5 MB over refuses.
        assert!(matches!(
            check_integrity(4 * 1024, 0, 0),
            IntegrityVerdict::Agree { .. }
        ));
        assert!(matches!(
            check_integrity(5 * 1024, 0, 0),
            IntegrityVerdict::CounterLie { .. }
        ));
    }

    #[test]
    fn integrity_warns_on_df_excess_and_marks_support() {
        match check_integrity(0, 5 * 1024, 0) {
            IntegrityVerdict::DfExcess { failures, .. } => assert_eq!(failures, 0),
            other => panic!("df excess must warn, got {other:?}"),
        }
        let unsupported = df_excess_warn_text(0, 5, 0);
        assert!(unsupported.contains("UNSUPPORTED"), "got {unsupported}");
        assert!(unsupported.contains("(b)concurrent-activity-on-box"), "got {unsupported}");
        let supported = df_excess_warn_text(0, 5, 1);
        assert!(supported.contains("SUPPORTED"), "got {supported}");
        assert!(!supported.contains("UNSUPPORTED"), "got {supported}");
    }

    #[test]
    fn delete_application_moves_counters_once() {
        let applied = apply_delete_observation("/b/a", true, true, "", 1024);
        assert!(matches!(
            applied.outcome,
            DeleteOutcome::DeletedByMe { bytes: 1024 }
        ));
        assert_eq!((applied.bytes_delta, applied.dir_delta), (1024, 1));
        assert_eq!((applied.absent_delta, applied.failure_line.is_some()), (0, false));
    }

    #[test]
    fn delete_application_never_books_absence_as_success() {
        let applied = apply_delete_observation("/b/a", false, true, "", 0);
        assert_eq!(applied.outcome, DeleteOutcome::AlreadyAbsent);
        assert_eq!((applied.bytes_delta, applied.dir_delta, applied.absent_delta), (0, 0, 1));
        assert_eq!(applied.failure_line, None);
    }

    #[test]
    fn delete_application_freezes_counters_on_failure_and_names_it() {
        let applied =
            apply_delete_observation("/b/a", true, false, "rm: Permission denied", 999);
        assert!(matches!(applied.outcome, DeleteOutcome::Failed { .. }));
        assert_eq!((applied.bytes_delta, applied.dir_delta), (0, 0));
        let line = applied.failure_line.expect("failure must be named");
        assert!(line.contains("/b/a"), "got {line}");
        assert!(line.contains("Permission denied"), "got {line}");
    }

    #[test]
    fn twin_gate_keeps_canonical_and_live_twins() {
        assert!(matches!(
            gate_export_cache(true, TwinState::Present),
            SweepVerdict::Keep { .. }
        ));
        assert!(matches!(
            gate_export_cache(true, TwinState::Absent),
            SweepVerdict::Keep { .. }
        ));
        assert!(matches!(
            gate_export_cache(false, TwinState::Present),
            SweepVerdict::Keep { .. }
        ));
        assert!(matches!(
            gate_export_cache(false, TwinState::Absent),
            SweepVerdict::Sweep { .. }
        ));
    }

    #[test]
    fn twin_gate_governs_only_the_export_cache_path() {
        let mut governed = BTreeSet::new();
        for rule in [
            WhitelistRule::RchTarget,
            WhitelistRule::RchTargetPool,
            WhitelistRule::RchTmp,
            WhitelistRule::Mutation,
            WhitelistRule::Grade,
            WhitelistRule::DotGrade,
        ] {
            if twin_gate_applies(rule) {
                governed.insert(rule.as_str());
            }
        }
        assert_eq!(governed, BTreeSet::from([".rch-tmp"]));
    }

    fn decision(sweep: bool, at: TwinState) -> LeaseDecision {
        LeaseDecision {
            path: PathBuf::from("/b/x"),
            sweep,
            reason: "twin-absent".to_owned(),
            epoch_secs: 1,
            twin_at_decision: at,
        }
    }

    #[test]
    fn verify_justifies_absent_sweeps_and_condemns_live_ones() {
        use TwinState::{Absent, Present};
        use VerifyVerdict::{
            AbsentViolation, ConfirmedKeep, JustifiedSweep, LiveDestroyed, NotSwept,
            ReapedAfterDecision,
        };
        assert_eq!(
            verify_sweep(&decision(true, Absent), Absent, true),
            JustifiedSweep
        );
        assert_eq!(
            verify_sweep(&decision(true, Present), Present, true),
            LiveDestroyed
        );
        assert_eq!(verify_sweep(&decision(true, Absent), Absent, false), NotSwept);
        assert_eq!(
            verify_sweep(&decision(false, Absent), Absent, true),
            LiveDestroyed
        );
        assert_eq!(
            verify_sweep(&decision(false, Present), Absent, false),
            ReapedAfterDecision
        );
        // MANDATORY known-good: absent at decision and still standing fires.
        assert_eq!(
            verify_sweep(&decision(false, Absent), Absent, false),
            AbsentViolation
        );
        assert_eq!(
            verify_sweep(&decision(false, Present), Present, false),
            ConfirmedKeep
        );
        assert_eq!(
            verify_sweep(&decision(false, Absent), Present, false),
            ConfirmedKeep
        );
    }

    #[test]
    fn empty_decision_rows_are_vacuous_never_clean() {
        assert!(check_decisions_nonvacuous(&[]).is_err());
        let rows = [decision(true, TwinState::Absent)];
        assert_eq!(check_decisions_nonvacuous(&rows), Ok(1));
    }

    #[test]
    fn evaluated_identity_counts_every_bucket_once() {
        let mut report = ReclaimReport::new(worker(), ReclaimMode::Apply);
        report.directories = 2;
        report.kept = 1;
        report.refused = vec!["r".to_owned()];
        report.failures = vec!["f".to_owned()];
        report.absent_before = 3;
        assert_eq!(evaluated_total(&report), 8);
    }

    #[test]
    fn fleet_fold_prefers_integrity_then_failures_then_unknowns() {
        let fleet = FleetReport::from_reports(
            ReclaimMode::Apply,
            vec![
                report_with(RunOutcome::Reclaimed),
                report_with(RunOutcome::DeleteFailed),
                report_with(RunOutcome::IntegrityRefused),
            ],
        )
        .expect("fold");
        assert_eq!(fleet.outcome, FleetOutcome::IntegrityRefused);
        assert_eq!(fleet.exit_code(), 3);
        let fleet = FleetReport::from_reports(
            ReclaimMode::Apply,
            vec![
                report_with(RunOutcome::Reclaimed),
                report_with(RunOutcome::DeleteFailed),
            ],
        )
        .expect("fold");
        assert_eq!(fleet.outcome, FleetOutcome::DeleteFailed);
        assert_eq!(fleet.exit_code(), 5);
    }

    #[test]
    fn fleet_fold_is_vacuous_only_when_nothing_was_evaluated() {
        let mut quiet = report_with(RunOutcome::Planned);
        quiet.directories = 0;
        let fleet = FleetReport::from_reports(ReclaimMode::DryRun, vec![quiet])
            .expect("fold");
        assert_eq!(fleet.outcome, FleetOutcome::Vacuous);
        assert_eq!(fleet.exit_code(), 4);
        assert!(FleetReport::from_reports(ReclaimMode::Apply, vec![]).is_err());
    }

    #[test]
    fn twin_state_reads_filesystem() {
        let dir = std::env::temp_dir().join(format!(
            "contabo-reclaim-twin-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("fixture dir");
        let live = dir.join("live-export");
        std::fs::write(&live, "x").expect("fixture file");
        assert_eq!(read_twin_state(&live), TwinState::Present);
        assert_eq!(read_twin_state(&dir.join("absent-export")), TwinState::Absent);
        #[cfg(unix)]
        {
            let dangling = dir.join("dangling");
            std::os::unix::fs::symlink(dir.join("no-target"), &dangling).expect("symlink");
            assert_eq!(read_twin_state(&dangling), TwinState::Absent);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod vacuous_pin_tests {
    use super::*;

    #[test]
    fn empty_listing_is_vacuous_exit_4_never_clean() {
        assert_eq!(vacuous_listing_outcome(), RunOutcome::Vacuous);
        assert_eq!(vacuous_listing_outcome().exit_code(), 4);
    }
}

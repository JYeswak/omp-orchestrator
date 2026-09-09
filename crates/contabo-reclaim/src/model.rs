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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RunOutcome {
    Planned,
    Reclaimed,
    AlreadyClean,
    SkippedLiveBuild,
    Unknown,
    Unreachable,
    Refused,
}

impl RunOutcome {
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::Planned | Self::Reclaimed | Self::AlreadyClean | Self::SkippedLiveBuild => 0,
            Self::Refused => 1,
            Self::Unknown | Self::Unreachable => 2,
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
    pub detail: String,
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
}

impl FleetOutcome {
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::Complete => 0,
            Self::DeferredActiveBuild => 3,
            Self::Incomplete => 2,
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
        for report in &workers {
            match report.outcome {
                RunOutcome::Planned | RunOutcome::Reclaimed | RunOutcome::AlreadyClean => {}
                RunOutcome::SkippedLiveBuild => deferred_active_workers += 1,
                RunOutcome::Unknown | RunOutcome::Unreachable | RunOutcome::Refused => {
                    incomplete_workers += 1
                }
            }
        }
        let outcome = if incomplete_workers > 0 {
            FleetOutcome::Incomplete
        } else if deferred_active_workers > 0 {
            FleetOutcome::DeferredActiveBuild
        } else {
            FleetOutcome::Complete
        };
        let detail = format!(
            "workers={} deferred_active_workers={} incomplete_workers={}",
            workers.len(),
            deferred_active_workers,
            incomplete_workers
        );
        Ok(Self {
            schema: "contabo-reclaim/fleet-report-v1",
            mode: mode.into(),
            outcome,
            workers,
            deferred_active_workers,
            incomplete_workers,
            detail,
        })
    }

    pub const fn exit_code(&self) -> u8 {
        self.outcome.exit_code()
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

#![forbid(unsafe_code)]

use crate::model::{
    decide_guards, parse_listing, validate_candidate, ActiveBuild, CandidateSet, ControlSnapshot,
    EntryKind, FleetReport, GuardDecision, ReclaimError, ReclaimMode, ReclaimRefusal,
    ReclaimReport, RemoteProcessObservation, RunOutcome, ValidatedCandidate, WorkerSpec, WORKERS,
};
use asupersync::process::Command;
use asupersync::time::timeout;
use asupersync::Cx;
use serde_json::Value;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::Duration;
use subprocess_contract::{run_output, RunError};

const PROBE_DEADLINE: Duration = Duration::from_secs(30);
const RCH_STATUS_ARGS: &[&str] = &["status", "--json"];
const PGPREP_PATTERNS: &[&str] = &["[c]argo", "[r]ustc"];

#[derive(Debug, Clone)]
pub struct Config {
    pub worker: WorkerSpec,
    pub base: PathBuf,
    pub mode: ReclaimMode,
}

pub async fn run(cx: &Cx, config: &Config) -> Result<ReclaimReport, ReclaimError> {
    validate_base(&config.base)?;
    cx.checkpoint().map_err(|_| ReclaimError::Runtime {
        detail: "cancelled before control probe".to_owned(),
    })?;

    let mut report = ReclaimReport::new(config.worker, config.mode);
    let control = match control_snapshot(cx, config.worker).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            report.outcome = RunOutcome::Unknown;
            report.detail = error.to_string();
            return Ok(report);
        }
    };
    if !control.matches_worker(config.worker) {
        report.outcome = RunOutcome::Unknown;
        report.detail = format!(
            "CONTROL_HOST_MISMATCH worker={} selected_host={} control_host={}",
            config.worker.id, config.worker.host, control.worker_host
        );
        return Ok(report);
    }
    report.guards.push(format!(
        "control-plane worker={} host={} status={} active_builds={} used_slots={}",
        control.worker_id,
        control.worker_host,
        control.worker_status,
        control.active_builds.len(),
        control.used_slots
    ));
    let remote = remote_processes(cx, config.worker).await;
    append_process_guard(&mut report, &remote);
    match decide_guards(&control, &remote) {
        GuardDecision::Authorized => {}
        GuardDecision::SkippedLiveBuild {
            detail,
            active_build_ids,
        } => {
            report.active_build_ids = active_build_ids;
            report.outcome = RunOutcome::SkippedLiveBuild;
            report.detail = detail;
            return Ok(report);
        }
        GuardDecision::Unknown { detail } => {
            report.outcome = RunOutcome::Unknown;
            report.detail = detail;
            return Ok(report);
        }
        GuardDecision::Unreachable { detail } => {
            report.outcome = RunOutcome::Unreachable;
            report.detail = detail;
            return Ok(report);
        }
    }

    let listing = match list_candidates(cx, config.worker, &config.base).await {
        Ok(listing) => listing,
        Err(error) => {
            report.outcome = RunOutcome::Unreachable;
            report.detail = error.to_string();
            return Ok(report);
        }
    };
    let candidates = match parse_listing(&listing) {
        // Leg 6, sweep half: a sweep evaluating zero directories is an
        // ERROR, never clean. An empty find output on a healthy box is
        // indistinguishable from a broken find, so fail closed.
        Ok(CandidateSet::Empty) => {
            report.outcome = crate::model::vacuous_listing_outcome();
            report.detail =
                "VACUOUS_CANDIDATE_SET: find listed zero entries under base".to_owned();
            return Ok(report);
        }
        Ok(CandidateSet::NonEmpty(candidates)) => candidates,
        Err(error) => {
            report.outcome = RunOutcome::Unknown;
            report.detail = error.to_string();
            return Ok(report);
        }
    };

    let mut valid = Vec::with_capacity(candidates.len());
    let mut refusals = Vec::new();
    for candidate in candidates {
        cx.checkpoint().map_err(|_| ReclaimError::Runtime {
            detail: "cancelled during candidate validation".to_owned(),
        })?;
        let resolved = if candidate.kind == EntryKind::Symlink {
            match remote_realpath(cx, config.worker, &candidate.path).await {
                Ok(path) => Some(path),
                Err(error) => {
                    refusals.push(ReclaimRefusal {
                        path: candidate.path,
                        reason: crate::model::RefusalReason::SymlinkTargetUnreadable,
                        detail: error.to_string(),
                    });
                    continue;
                }
            }
        } else {
            None
        };
        match validate_candidate(&config.base, candidate, resolved.as_deref()) {
            Ok(candidate) => valid.push(candidate),
            Err(refusal) => refusals.push(refusal),
        }
    }
    report.candidates = valid
        .iter()
        .map(|candidate| candidate.candidate.path.display().to_string())
        .collect();
    report.refused = refusals.iter().map(ToString::to_string).collect();
    if !refusals.is_empty() {
        report.outcome = RunOutcome::Refused;
        report.detail = "one or more candidates failed whitelist or containment".to_owned();
        return Ok(report);
    }

    // Twin gate (leg 4): ONLY export caches consult the Mac-side twin.
    // Build artifacts sweep by kind; the twin predicate never sees them.
    // Every twin-gated candidate records a decision row carrying the
    // instant and the twin observed then.
    let epoch_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let mut sweepable: Vec<ValidatedCandidate> = Vec::with_capacity(valid.len());
    for candidate in valid {
        if !crate::model::twin_gate_applies(candidate.rule) {
            sweepable.push(candidate);
            continue;
        }
        let parent_is_canonical = candidate
            .candidate
            .path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            .is_some_and(|name| crate::model::CANONICAL_CHECKOUTS.contains(&name));
        // The worker replicates the Mac layout, so the twin is the same
        // absolute path on this host.
        let twin = crate::model::read_twin_state(&candidate.candidate.path);
        match crate::model::gate_export_cache(parent_is_canonical, twin) {
            crate::model::SweepVerdict::Sweep { reason } => {
                report.decision_rows.push(crate::model::LeaseDecision {
                    path: candidate.candidate.path.clone(),
                    sweep: true,
                    reason,
                    epoch_secs,
                    twin_at_decision: twin,
                });
                sweepable.push(candidate);
            }
            crate::model::SweepVerdict::Keep { reason } => {
                report.decision_rows.push(crate::model::LeaseDecision {
                    path: candidate.candidate.path.clone(),
                    sweep: false,
                    reason,
                    epoch_secs,
                    twin_at_decision: twin,
                });
                report.kept += 1;
            }
        }
    }
    if config.mode == ReclaimMode::DryRun {
        report.outcome = RunOutcome::Planned;
        report.directories = sweepable.len();
        report.bytes = sweepable.iter().map(|candidate| candidate.candidate.bytes).sum();
        report.detail =
            "DRY_RUN: no deletion requested; twin gate decided, integrity not consulted on a plan"
                .to_owned();
        return Ok(report);
    }
    let df_before = match df_avail_kb(cx, config.worker, &config.base).await {
        Ok(kb) => kb,
        Err(error) => {
            report.outcome = RunOutcome::Unknown;
            report.detail = format!("df_before_unreadable detail={error}");
            return Ok(report);
        }
    };
    let mut reclaimed_bytes: u64 = 0;
    for candidate in &sweepable {
        cx.checkpoint().map_err(|_| ReclaimError::Runtime {
            detail: "cancelled before deletion".to_owned(),
        })?;
        let control = match control_snapshot(cx, config.worker).await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                report.outcome = RunOutcome::Unknown;
                report.detail = format!("before_delete={error}");
                return Ok(report);
            }
        };
        if !control.matches_worker(config.worker) {
            report.outcome = RunOutcome::Unknown;
            report.detail = format!(
                "before_delete=CONTROL_HOST_MISMATCH worker={} selected_host={} control_host={}",
                config.worker.id, config.worker.host, control.worker_host
            );
            return Ok(report);
        }
        let remote = remote_processes(cx, config.worker).await;
        match decide_guards(&control, &remote) {
            GuardDecision::Authorized => {}
            GuardDecision::SkippedLiveBuild {
                detail,
                active_build_ids,
            } => {
                report.active_build_ids = active_build_ids;
                report.outcome = RunOutcome::SkippedLiveBuild;
                report.detail = format!("before_delete={detail}");
                return Ok(report);
            }
            GuardDecision::Unknown { detail } => {
                report.outcome = RunOutcome::Unknown;
                report.detail = format!("before_delete={detail}");
                return Ok(report);
            }
            GuardDecision::Unreachable { detail } => {
                report.outcome = RunOutcome::Unreachable;
                report.detail = format!("before_delete={detail}");
                return Ok(report);
            }
        }
        // Leg 5: existence probed BEFORE the delete, never inferred after.
        // Leg 3: the outcome and the counter update are one operation.
        let path = candidate.candidate.path.display().to_string();
        let existed = match remote_exists(cx, config.worker, &candidate.candidate.path).await {
            Ok(existed) => existed,
            Err(error) => {
                report.outcome = RunOutcome::Unknown;
                report.detail = format!("existence_probe_failed path={path} detail={error}");
                return Ok(report);
            }
        };
        let mut stderr = String::new();
        let rm_success = if existed {
            match delete_candidate(cx, config.worker, &candidate.candidate.path).await {
                Ok(()) => true,
                Err(error) => {
                    stderr = error.to_string();
                    false
                }
            }
        } else {
            false
        };
        let applied = crate::model::apply_delete_observation(
            &path,
            existed,
            rm_success,
            &stderr,
            candidate.candidate.bytes,
        );
        reclaimed_bytes += applied.bytes_delta;
        report.directories += applied.dir_delta;
        report.absent_before += applied.absent_delta;
        if let Some(line) = applied.failure_line {
            report.failures.push(line);
        }
    }
    report.bytes = reclaimed_bytes;
    // Leg 2 runs BEFORE the failure accounting, per the observed oracle: a
    // failing delete beside a lying counter refused with exit 3, not 5.
    // DRY never reaches here, so the counter below is reclaimed bytes and
    // the df delta must show them.
    let df_after = match df_avail_kb(cx, config.worker, &config.base).await {
        Ok(kb) => kb,
        Err(error) => {
            report.outcome = RunOutcome::Unknown;
            report.detail = format!("df_after_unreadable detail={error}");
            return Ok(report);
        }
    };
    match crate::model::check_integrity(
        reclaimed_bytes / 1024,
        df_after.saturating_sub(df_before),
        report.failures.len() as u64,
    ) {
        crate::model::IntegrityVerdict::Agree { .. } => {}
        crate::model::IntegrityVerdict::CounterLie { counter_mb, df_mb } => {
            report.outcome = RunOutcome::IntegrityRefused;
            report.detail = format!(
                "INTEGRITY counter={counter_mb}MB df_delta={df_mb}MB tolerance=4MB -- \
                 the counter lied about the filesystem"
            );
            return Ok(report);
        }
        crate::model::IntegrityVerdict::DfExcess {
            counter_mb,
            df_mb,
            failures,
        } => {
            report.detail = crate::model::df_excess_warn_text(counter_mb, df_mb, failures);
        }
    }
    if !report.failures.is_empty() {
        report.outcome = RunOutcome::DeleteFailed;
        report.detail = format!(
            "{}; DELETE-FAILURES -- {} delete(s) failed and are named above; box is not 'done'",
            report.detail,
            report.failures.len()
        );
        return Ok(report);
    }
    // Leg 6, verifier half as a self-check: an ok report with zero decision
    // rows is vacuous. Unreachable through the flow above (every validated
    // candidate leaves a row or a refusal), legged synthetically.
    if crate::model::check_decisions_nonvacuous(&report.decision_rows).is_err()
        && evaluated_is_empty(&report)
    {
        report.outcome = RunOutcome::Vacuous;
        report.detail = "VACUOUS_SWEEP: ok verdict with no evaluated bucket filled".to_owned();
        return Ok(report);
    }
    report.outcome = RunOutcome::Reclaimed;
    report.detail = "APPLY: all candidates re-authorized immediately before deletion".to_owned();
    Ok(report)
}

/// True when every evaluated bucket is empty (leg 5 identity, all zero).
fn evaluated_is_empty(report: &ReclaimReport) -> bool {
    crate::model::evaluated_total(report) == 0
}

/// Available KiB on the filesystem holding BASE (portability leg: `df -Pk`
/// reports KiB on both macOS and Linux; and the mount measured is BASE's,
/// never `/`).
async fn df_avail_kb(
    cx: &Cx,
    worker: WorkerSpec,
    base: &Path,
) -> Result<u64, ReclaimError> {
    let base = base.to_string_lossy().into_owned();
    let output = run_command(
        cx,
        ssh(worker, &["df", "-Pk", &base]),
        worker.id,
        "df-avail",
    )
    .await?;
    if !output.success {
        return Err(ReclaimError::Output {
            worker: worker.id.to_owned(),
            operation: "df-avail",
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let row = text.lines().filter(|line| !line.trim().is_empty()).last();
    let avail = row
        .and_then(|line| line.split_whitespace().nth(3))
        .ok_or_else(|| ReclaimError::Probe {
            worker: worker.id.to_owned(),
            detail: "df -Pk output has no available-blocks column".to_owned(),
        })?;
    avail.parse::<u64>().map_err(|error| ReclaimError::Probe {
        worker: worker.id.to_owned(),
        detail: format!("df available blocks not numeric: {error}"),
    })
}

/// Pre-delete existence probe (leg 5): `test -e` exit 0/1. Any other exit
/// is a probe failure, never evidence either way.
async fn remote_exists(cx: &Cx, worker: WorkerSpec, path: &Path) -> Result<bool, ReclaimError> {
    let path = path.to_string_lossy().into_owned();
    let output = run_command(
        cx,
        ssh(worker, &["test", "-e", "--", &path]),
        worker.id,
        "existence-probe",
    )
    .await?;
    match output.code {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        other => Err(ReclaimError::Output {
            worker: worker.id.to_owned(),
            operation: "existence-probe",
            detail: format!("test -e exited {other:?}"),
        }),
    }
}
pub async fn run_all_workers(
    cx: &Cx,
    base: &Path,
    mode: ReclaimMode,
) -> Result<FleetReport, ReclaimError> {
    if WORKERS.is_empty() {
        return Err(ReclaimError::EmptyFleetReport);
    }
    let reports = run_workers_sequentially(cx, base, mode, |cx, config| {
        Box::pin(async move { run(cx, &config).await })
    })
    .await?;
    FleetReport::from_reports(mode, reports)
}

/// Fleet entry honoring a worker selection: one worker's report still folds
/// through `from_reports` so exit codes and the vacuity aggregate mean the
/// same thing for one worker and for all of them.
pub async fn run_selected(
    cx: &Cx,
    base: &Path,
    mode: ReclaimMode,
    selection: &crate::model::WorkerSelection,
) -> Result<FleetReport, ReclaimError> {
    match selection {
        crate::model::WorkerSelection::One(worker) => {
            let config = Config {
                worker: *worker,
                base: base.to_path_buf(),
                mode,
            };
            let report = run(cx, &config).await?;
            FleetReport::from_reports(mode, vec![report])
        }
        crate::model::WorkerSelection::All => run_all_workers(cx, base, mode).await,
    }
}

/// Report-keyed single-export disposal (bead 4ftow): drop the worker-side
/// pools keyed to one retired grade export. The grader invokes this at
/// report time with its own export basename and the worker its build ran
/// on; an export nobody names is never enumerated, so an unreported grade's
/// pool survives by construction (item 5, first half).
///
/// Authorization is the explicit invocation, not a twin or an age: no
/// registry is consulted and none is maintained, so there is no register
/// to drift. Pool basenames come from the remote listing, never from argv;
/// argv names only the export scope, and every enumerated pool passes
/// [`validate_candidate`] plus [`is_retire_pool_basename`] before deletion.
/// A busy worker DEFERS (exit 2, retry later) -- SkippedLiveBuild's 0 would
/// report disposal that never happened. Zero pools is [`Vacuous`] exit 4,
/// never clean. Every dropped pool is proven absent by a post-delete
/// `test -e` (item 5, second half); df-integrity is deliberately not
/// consulted (single known scope, verified absence instead).
pub async fn retire_export(
    cx: &Cx,
    base: &Path,
    worker: WorkerSpec,
    export: &str,
    mode: ReclaimMode,
) -> Result<ReclaimReport, ReclaimError> {
    use crate::model::{is_retire_pool_basename, parse_retire_export};
    validate_base(base)?;
    let export = parse_retire_export(export)?;
    let mut report = ReclaimReport::new(worker, mode);
    let control = match control_snapshot(cx, worker).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            report.outcome = RunOutcome::Unknown;
            report.detail = error.to_string();
            return Ok(report);
        }
    };
    if !control.matches_worker(worker) {
        report.outcome = RunOutcome::Unknown;
        report.detail = format!(
            "CONTROL_HOST_MISMATCH worker={} selected_host={} control_host={}",
            worker.id, worker.host, control.worker_host
        );
        return Ok(report);
    }
    report.guards.push(format!(
        "control-plane worker={} host={} status={} active_builds={} used_slots={}",
        control.worker_id,
        control.worker_host,
        control.worker_status,
        control.active_builds.len(),
        control.used_slots
    ));
    let remote = remote_processes(cx, worker).await;
    append_process_guard(&mut report, &remote);
    match decide_guards(&control, &remote) {
        GuardDecision::Authorized => {}
        GuardDecision::SkippedLiveBuild { detail, .. } => {
            // Retire-only mapping: a sweep skips and retries the next box
            // (exit 0); a retire that exits 0 without deleting reports a
            // disposal that never happened. Deferral is "did not finish".
            report.outcome = RunOutcome::Deferred;
            report.detail = format!("retire_deferred_live_build {detail}");
            return Ok(report);
        }
        GuardDecision::Unknown { detail } => {
            report.outcome = RunOutcome::Unknown;
            report.detail = detail;
            return Ok(report);
        }
        GuardDecision::Unreachable { detail } => {
            report.outcome = RunOutcome::Unreachable;
            report.detail = detail;
            return Ok(report);
        }
    }
    let export_dir = base.join(&export);
    // Missing export is vacuous with its own reason, not "no pools" and not
    // a find failure: the grader named a scope the box does not have, and a
    // find error would read as a broken box instead of a wrong name.
    match remote_exists(cx, worker, &export_dir).await {
        Ok(true) => {}
        Ok(false) => {
            report.outcome = crate::model::vacuous_listing_outcome();
            report.detail =
                format!("RETIRE_NO_SUCH_EXPORT export={export}: the box has no such export dir");
            return Ok(report);
        }
        Err(error) => {
            report.outcome = RunOutcome::Unknown;
            report.detail = format!("retire_export_probe_failed export={export} detail={error}");
            return Ok(report);
        }
    }
    let listing = match list_retire_pools(cx, worker, &export_dir).await {
        Ok(listing) => listing,
        Err(error) => {
            report.outcome = RunOutcome::Unreachable;
            report.detail = error.to_string();
            return Ok(report);
        }
    };
    let pools = match parse_listing(&listing) {
        Ok(CandidateSet::Empty) => {
            report.outcome = crate::model::vacuous_listing_outcome();
            report.detail = format!(
                "RETIRE_VACUOUS export={export}: no pools under the retired export"
            );
            return Ok(report);
        }
        Ok(CandidateSet::NonEmpty(pools)) => pools,
        Err(error) => {
            report.outcome = RunOutcome::Unknown;
            report.detail = error.to_string();
            return Ok(report);
        }
    };
    let mut valid = Vec::with_capacity(pools.len());
    let mut refusals = Vec::new();
    for pool in pools {
        cx.checkpoint().map_err(|_| ReclaimError::Runtime {
            detail: "cancelled during retire validation".to_owned(),
        })?;
        let Some(basename) = pool
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
        else {
            refusals.push(ReclaimRefusal {
                path: pool.path,
                reason: crate::model::RefusalReason::NotWhitelisted,
                detail: "retire pool has no UTF-8 basename".to_owned(),
            });
            continue;
        };
        if !is_retire_pool_basename(&basename) {
            refusals.push(ReclaimRefusal {
                path: pool.path,
                reason: crate::model::RefusalReason::NotWhitelisted,
                detail: format!(
                    "retire drops only .rch-target* pools; basename={basename}"
                ),
            });
            continue;
        }
        let resolved = if pool.kind == EntryKind::Symlink {
            match remote_realpath(cx, worker, &pool.path).await {
                Ok(path) => Some(path),
                Err(error) => {
                    refusals.push(ReclaimRefusal {
                        path: pool.path,
                        reason: crate::model::RefusalReason::SymlinkTargetUnreadable,
                        detail: error.to_string(),
                    });
                    continue;
                }
            }
        } else {
            None
        };
        match validate_candidate(base, pool, resolved.as_deref()) {
            Ok(candidate) => valid.push(candidate),
            Err(refusal) => refusals.push(refusal),
        }
    }
    report.candidates = valid
        .iter()
        .map(|candidate| candidate.candidate.path.display().to_string())
        .collect();
    report.refused = refusals.iter().map(ToString::to_string).collect();
    if !refusals.is_empty() {
        report.outcome = RunOutcome::Refused;
        report.detail = "one or more pools failed whitelist or containment".to_owned();
        return Ok(report);
    }
    if mode == ReclaimMode::DryRun {
        report.outcome = RunOutcome::Planned;
        report.directories = valid.len();
        report.detail =
            "DRY_RUN: retire planned, nothing deleted; re-run with --apply to drop".to_owned();
        return Ok(report);
    }
    let mut reclaimed_bytes: u64 = 0;
    for candidate in &valid {
        cx.checkpoint().map_err(|_| ReclaimError::Runtime {
            detail: "cancelled before retire delete".to_owned(),
        })?;
        let control = match control_snapshot(cx, worker).await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                report.outcome = RunOutcome::Unknown;
                report.detail = format!("retire_before_delete={error}");
                return Ok(report);
            }
        };
        if !control.matches_worker(worker) {
            report.outcome = RunOutcome::Unknown;
            report.detail = format!(
                "retire_before_delete=CONTROL_HOST_MISMATCH worker={} selected_host={} control_host={}",
                worker.id, worker.host, control.worker_host
            );
            return Ok(report);
        }
        let remote = remote_processes(cx, worker).await;
        match decide_guards(&control, &remote) {
            GuardDecision::Authorized => {}
            GuardDecision::SkippedLiveBuild { detail, .. } => {
                report.outcome = RunOutcome::Deferred;
                report.detail = format!("retire_before_delete=deferred {detail}");
                return Ok(report);
            }
            GuardDecision::Unknown { detail } => {
                report.outcome = RunOutcome::Unknown;
                report.detail = format!("retire_before_delete={detail}");
                return Ok(report);
            }
            GuardDecision::Unreachable { detail } => {
                report.outcome = RunOutcome::Unreachable;
                report.detail = format!("retire_before_delete={detail}");
                return Ok(report);
            }
        }
        let path = candidate.candidate.path.display().to_string();
        // Honest bytes BEFORE the delete: du of an absent path is 0 KB, and
        // booking that would under-report by the whole pool. A du failure
        // fails closed (Unknown) rather than booking zero.
        let kb = match du_kb(cx, worker, &candidate.candidate.path).await {
            Ok(kb) => kb,
            Err(error) => {
                report.outcome = RunOutcome::Unknown;
                report.detail = format!("retire_du_failed path={path} detail={error}");
                return Ok(report);
            }
        };
        let existed = match remote_exists(cx, worker, &candidate.candidate.path).await {
            Ok(existed) => existed,
            Err(error) => {
                report.outcome = RunOutcome::Unknown;
                report.detail = format!("retire_existence_probe_failed path={path} detail={error}");
                return Ok(report);
            }
        };
        let mut stderr = String::new();
        let rm_success = if existed {
            match delete_candidate(cx, worker, &candidate.candidate.path).await {
                Ok(()) => true,
                Err(error) => {
                    stderr = error.to_string();
                    false
                }
            }
        } else {
            false
        };
        // Absence proof (item 5, second half): rm exit 0 is not the verdict;
        // the path must read absent afterwards. A success that persists is
        // folded into Failed with its own stderr, so the counters stay
        // single-sourced through apply_delete_observation.
        let absent_after = match remote_exists(cx, worker, &candidate.candidate.path).await {
            Ok(absent) => !absent,
            Err(error) => {
                report.outcome = RunOutcome::Unknown;
                report.detail =
                    format!("retire_absence_probe_failed path={path} detail={error}");
                return Ok(report);
            }
        };
        if rm_success && !absent_after {
            stderr = format!("rm succeeded but the path persists {stderr}").trim().to_owned();
        }
        let applied = crate::model::apply_delete_observation(
            &path,
            existed,
            rm_success && absent_after,
            &stderr,
            kb.saturating_mul(1024),
        );
        reclaimed_bytes += applied.bytes_delta;
        report.directories += applied.dir_delta;
        report.absent_before += applied.absent_delta;
        if let Some(line) = applied.failure_line {
            report.failures.push(line);
        }
    }
    report.bytes = reclaimed_bytes;
    if report.failures.is_empty() {
        report.outcome = RunOutcome::Reclaimed;
        report.detail = format!("RETIRE_COMPLETE export={export} pools={}", valid.len());
    } else {
        report.outcome = RunOutcome::DeleteFailed;
        report.detail = "one or more pool deletes failed or unverified".to_owned();
    }
    Ok(report)
}

/// Scoped pool listing: `.rch-target*` entries directly under one export
/// dir, in the `parse_listing` shape. The export scope comes from validated
/// argv; pool names come from the box. Anything the box returns that is not
/// pool-shaped is refused downstream, never deleted.
async fn list_retire_pools(
    cx: &Cx,
    worker: WorkerSpec,
    export_dir: &Path,
) -> Result<String, ReclaimError> {
    let export_dir = export_dir.to_string_lossy().into_owned();
    let output = run_command(
        cx,
        ssh(
            worker,
            &[
                "find",
                &export_dir,
                "-mindepth",
                "1",
                "-maxdepth",
                "1",
                "-name",
                ".rch-target*",
                "-printf",
                "'%p\\t%y\\t%s\\n'",
            ],
        ),
        worker.id,
        "retire-pool-list",
    )
    .await?;
    if !output.success {
        return Err(ReclaimError::Output {
            worker: worker.id.to_owned(),
            operation: "retire-pool-list",
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Honest bytes for one pool: `du -sk` (KiB) scaled to bytes. The listing's
/// `%s` is directory-metadata size, not recursive content; booking that
/// would under-report by orders of magnitude and poison the counter the
/// integrity check trusts.
async fn du_kb(cx: &Cx, worker: WorkerSpec, path: &Path) -> Result<u64, ReclaimError> {
    let path = path.to_string_lossy().into_owned();
    let output = run_command(
        cx,
        ssh(worker, &["du", "-sk", "--", &path]),
        worker.id,
        "retire-du",
    )
    .await?;
    if !output.success {
        return Err(ReclaimError::Output {
            worker: worker.id.to_owned(),
            operation: "retire-du",
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .ok_or_else(|| ReclaimError::Probe {
            worker: worker.id.to_owned(),
            detail: "du -sk emitted no size field".to_owned(),
        })?
        .parse::<u64>()
        .map_err(|error| ReclaimError::Probe {
            worker: worker.id.to_owned(),
            detail: format!("du -sk size not numeric: {error}"),
        })
}

pub async fn run_workers_sequentially<F>(
    cx: &Cx,
    base: &Path,
    mode: ReclaimMode,
    mut runner: F,
) -> Result<Vec<ReclaimReport>, ReclaimError>
where
    F: for<'a> FnMut(
        &'a Cx,
        Config,
    )
        -> Pin<Box<dyn Future<Output = Result<ReclaimReport, ReclaimError>> + 'a>>,
{
    let mut reports = Vec::with_capacity(WORKERS.len());
    for worker in WORKERS.iter().copied() {
        cx.checkpoint().map_err(|_| ReclaimError::Runtime {
            detail: format!("cancelled before worker {}", worker.id),
        })?;
        let config = Config {
            worker,
            base: base.to_path_buf(),
            mode,
        };
        match runner(cx, config).await {
            Ok(report) => reports.push(report),
            Err(error) => reports.push(ReclaimReport::error(worker, mode, error.to_string())),
        }
    }
    Ok(reports)
}

fn validate_base(base: &Path) -> Result<(), ReclaimError> {
    if !base.is_absolute() {
        return Err(ReclaimError::InvalidBase {
            detail: format!("base={} must be absolute", base.display()),
        });
    }
    if base
        .components()
        .any(|component| component == std::path::Component::ParentDir)
    {
        return Err(ReclaimError::InvalidBase {
            detail: "base contains parent traversal".to_owned(),
        });
    }
    if base.to_string_lossy().chars().any(|character| {
        character.is_whitespace() || matches!(character, ';' | '&' | '|' | '`' | '$' | '>' | '<')
    }) {
        return Err(ReclaimError::InvalidBase {
            detail: "base contains shell-significant whitespace or metacharacters".to_owned(),
        });
    }
    Ok(())
}

#[derive(Debug)]
struct CapturedOutput {
    success: bool,
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

async fn run_command(
    cx: &Cx,
    command: Command,
    worker: &str,
    operation: &'static str,
) -> Result<CapturedOutput, ReclaimError> {
    cx.checkpoint().map_err(|_| ReclaimError::Runtime {
        detail: format!("cancelled before {operation}"),
    })?;
    match timeout(
        cx.now_for_observability(),
        PROBE_DEADLINE,
        run_output(cx, command),
    )
    .await
    {
        Ok(Ok(output)) => {
            cx.checkpoint().map_err(|_| ReclaimError::Runtime {
                detail: format!("cancelled after {operation}"),
            })?;
            Ok(CapturedOutput {
                success: output.status.success(),
                code: output.status.code(),
                stdout: output.stdout,
                stderr: output.stderr,
            })
        }
        Ok(Err(RunError::Timeout)) | Err(_) => Err(ReclaimError::Timeout {
            worker: worker.to_owned(),
            operation,
        }),
        Ok(Err(error)) => Err(ReclaimError::Probe {
            worker: worker.to_owned(),
            detail: format!("operation={operation} {error}"),
        }),
    }
}

fn target(worker: WorkerSpec) -> String {
    format!("root@{}", worker.host)
}

fn ssh(worker: WorkerSpec, args: &[&str]) -> Command {
    let mut command = Command::new("ssh");
    command.args([
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=5",
        &target(worker),
    ]);
    command.args(args);
    command
}

async fn control_snapshot(cx: &Cx, worker: WorkerSpec) -> Result<ControlSnapshot, ReclaimError> {
    let mut command = Command::new("rch");
    command.args(RCH_STATUS_ARGS);
    let output = run_command(cx, command, worker.id, "rch-status").await?;
    if !output.success {
        return Err(ReclaimError::Output {
            worker: worker.id.to_owned(),
            operation: "rch-status",
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    parse_control_snapshot(&output.stdout, worker)
}

pub fn parse_control_snapshot(
    bytes: &[u8],
    worker: WorkerSpec,
) -> Result<ControlSnapshot, ReclaimError> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|error| ReclaimError::MalformedStatus {
            detail: format!("invalid JSON: {error}"),
        })?;
    if value.get("success").and_then(Value::as_bool) != Some(true) {
        return Err(ReclaimError::MalformedStatus {
            detail: "control-plane status did not report success=true".to_owned(),
        });
    }
    let daemon = value
        .get("data")
        .and_then(|data| data.get("daemon"))
        .ok_or_else(|| ReclaimError::MalformedStatus {
            detail: "missing data.daemon".to_owned(),
        })?;
    let workers = daemon
        .get("workers")
        .and_then(Value::as_array)
        .ok_or_else(|| ReclaimError::MalformedStatus {
            detail: "missing data.daemon.workers array".to_owned(),
        })?;
    let worker_value = workers
        .iter()
        .find(|value| value.get("id").and_then(Value::as_str) == Some(worker.id))
        .ok_or_else(|| ReclaimError::MalformedStatus {
            detail: format!("worker {} absent from control-plane status", worker.id),
        })?;
    let worker_status = worker_value
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| ReclaimError::MalformedStatus {
            detail: format!("worker {} missing status", worker.id),
        })?
        .to_owned();
    let worker_host = worker_value
        .get("host")
        .and_then(Value::as_str)
        .ok_or_else(|| ReclaimError::MalformedStatus {
            detail: format!("worker {} missing host", worker.id),
        })?
        .to_owned();
    let used_slots = worker_value
        .get("used_slots")
        .and_then(Value::as_u64)
        .ok_or_else(|| ReclaimError::MalformedStatus {
            detail: format!("worker {} missing used_slots", worker.id),
        })?;
    let active_values = daemon
        .get("active_builds")
        .and_then(Value::as_array)
        .ok_or_else(|| ReclaimError::MalformedStatus {
            detail: "missing data.daemon.active_builds array".to_owned(),
        })?;
    let mut active_builds = Vec::new();
    for value in active_values {
        let worker_id = value
            .get("worker_id")
            .and_then(Value::as_str)
            .ok_or_else(|| ReclaimError::MalformedStatus {
                detail: "active build missing worker_id".to_owned(),
            })?;
        if worker_id != worker.id {
            continue;
        }
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| "<missing-id>".to_owned());
        active_builds.push(ActiveBuild {
            id,
            worker_id: worker_id.to_owned(),
        });
    }
    Ok(ControlSnapshot {
        worker_id: worker.id.to_owned(),
        worker_host,
        worker_status,
        used_slots,
        active_builds,
    })
}

pub fn parse_process_probe(
    code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
    pattern: &str,
) -> RemoteProcessObservation {
    let lines: Vec<String> = String::from_utf8_lossy(stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_owned)
        .collect();
    match code {
        Some(0) if !lines.is_empty() => RemoteProcessObservation::Present { lines },
        Some(0) => RemoteProcessObservation::Unavailable {
            detail: format!("pgrep pattern={pattern} exited 0 without process rows"),
        },
        Some(1) if lines.is_empty() => RemoteProcessObservation::Empty,
        Some(1) => RemoteProcessObservation::Present { lines },
        Some(code) => RemoteProcessObservation::Unavailable {
            detail: format!(
                "pgrep pattern={pattern} exit={code} stderr={}",
                String::from_utf8_lossy(stderr).trim()
            ),
        },
        None => RemoteProcessObservation::Unavailable {
            detail: format!("pgrep pattern={pattern} terminated by signal"),
        },
    }
}

async fn remote_processes(cx: &Cx, worker: WorkerSpec) -> RemoteProcessObservation {
    let mut lines = Vec::new();
    for pattern in PGPREP_PATTERNS {
        let output = match run_command(
            cx,
            ssh(worker, &["pgrep", "-a", "-f", pattern]),
            worker.id,
            "remote-process-probe",
        )
        .await
        {
            Ok(output) => output,
            Err(error) => {
                return RemoteProcessObservation::Unavailable {
                    detail: error.to_string(),
                }
            }
        };
        match parse_process_probe(output.code, &output.stdout, &output.stderr, pattern) {
            RemoteProcessObservation::Empty => {}
            RemoteProcessObservation::Present { lines: found } => lines.extend(found),
            RemoteProcessObservation::Unavailable { detail } => {
                return RemoteProcessObservation::Unavailable { detail }
            }
        }
    }
    lines.sort();
    lines.dedup();
    if lines.is_empty() {
        RemoteProcessObservation::Empty
    } else {
        RemoteProcessObservation::Present { lines }
    }
}

fn append_process_guard(report: &mut ReclaimReport, remote: &RemoteProcessObservation) {
    match remote {
        RemoteProcessObservation::Empty => report
            .guards
            .push("remote cargo/rustc processes=0".to_owned()),
        RemoteProcessObservation::Present { lines } => report
            .guards
            .push(format!("remote cargo/rustc processes={}", lines.len())),
        RemoteProcessObservation::Unavailable { detail } => report
            .guards
            .push(format!("remote process probe=UNKNOWN {detail}")),
    }
}

async fn list_candidates(cx: &Cx, worker: WorkerSpec, base: &Path) -> Result<String, ReclaimError> {
    let base = base.to_string_lossy().into_owned();
    let output = run_command(
        cx,
        ssh(
            worker,
            &[
                "find",
                &base,
                "-mindepth",
                "1",
                "-maxdepth",
                "1",
                "\\(",
                "-name",
                ".rch-target",
                "-o",
                "-name",
                ".rch-target-\\*",
                "-o",
                "-name",
                ".rch-tmp",
                "-o",
                "-name",
                "\\*-mut",
                "-o",
                "-name",
                "grade-\\*",
                "-o",
                "-name",
                ".grade-\\*",
                "\\)",
                "-printf",
                "'%p\\t%y\\t%s\\n'",
            ],
        ),
        worker.id,
        "candidate-list",
    )
    .await?;
    if !output.success {
        return Err(ReclaimError::Output {
            worker: worker.id.to_owned(),
            operation: "candidate-list",
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

async fn remote_realpath(
    cx: &Cx,
    worker: WorkerSpec,
    path: &Path,
) -> Result<PathBuf, ReclaimError> {
    let path = path.to_string_lossy().into_owned();
    let output = run_command(
        cx,
        ssh(worker, &["realpath", "-e", "--", &path]),
        worker.id,
        "symlink-realpath",
    )
    .await?;
    if !output.success {
        return Err(ReclaimError::Output {
            worker: worker.id.to_owned(),
            operation: "symlink-realpath",
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    let resolved = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if resolved.is_empty() {
        return Err(ReclaimError::Probe {
            worker: worker.id.to_owned(),
            detail: "realpath returned an empty target".to_owned(),
        });
    }
    Ok(PathBuf::from(resolved))
}

async fn delete_candidate(cx: &Cx, worker: WorkerSpec, path: &Path) -> Result<(), ReclaimError> {
    let path = path.to_string_lossy().into_owned();
    let output = run_command(
        cx,
        ssh(worker, &["rm", "-rf", "--", &path]),
        worker.id,
        "delete-candidate",
    )
    .await?;
    if output.success {
        Ok(())
    } else {
        Err(ReclaimError::Output {
            worker: worker.id.to_owned(),
            operation: "delete-candidate",
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worker() -> WorkerSpec {
        WorkerSpec {
            id: "contabo-2",
            host: "94.72.121.46",
        }
    }

    #[test]
    fn control_status_requires_both_authority_fields() {
        let json = br#"{
            "success": true,
            "data": {
                "daemon": {
                    "workers": [{"id":"contabo-2","host":"94.72.121.46","status":"healthy","used_slots":0}],
                    "active_builds": []
                }
            }
        }"#;
        let snapshot = parse_control_snapshot(json, worker()).expect("valid status");
        assert!(snapshot.is_clear());
        let missing = br#"{"success":true,"data":{"daemon":{"workers":[]}}}"#;
        let error = parse_control_snapshot(missing, worker()).expect_err("missing fields");
        assert!(error.to_string().contains("worker contabo-2 absent"));
    }
    #[test]
    fn process_probe_distinguishes_empty_present_and_unknown() {
        assert_eq!(
            parse_process_probe(Some(1), b"", b"", "[c]argo"),
            RemoteProcessObservation::Empty
        );
        assert_eq!(
            parse_process_probe(Some(0), b"123 cargo --build\n", b"", "[c]argo"),
            RemoteProcessObservation::Present {
                lines: vec!["123 cargo --build".to_owned()]
            }
        );
        assert!(matches!(
            parse_process_probe(Some(2), b"", b"permission denied", "[c]argo"),
            RemoteProcessObservation::Unavailable { .. }
        ));
    }
}

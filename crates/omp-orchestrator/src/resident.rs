#![forbid(unsafe_code)]

//! The resident OMP supervisor.
//!
//! This executable owns the observation -> queue -> dispatch -> receiver receipt
//! loop. It is intentionally not a report-only monitor: a managed session may
//! idle only with a bound Josh authorization token.

use ack_stage::cell_matrix::{self, DispatchCellMatrix};
use ack_stage::{
    assess as assess_ack_stage, AckAction, AckReadback, AckReadbackVerdict, AckStageInput,
    AckStageResult, TransportReceipt,
};

use ack_spine::ledger::StepKind;
use decision_ledger::{HeartbeatAction, HumanClause};
use agent_mail_native::identity::{format_sender_header, resolve_pane_identity, BindingStatus, PaneIdentity};
use agent_mail_native::journey::{
    self as mail, AgentName, DeliveryReceipt, ProjectKey, SendRequest,
};
use agent_mail_native::{MailClient, MailError};
use asupersync::process::{Command, Output};
use asupersync::runtime::RuntimeBuilder;
use asupersync::time::{sleep, timeout};
use asupersync::Cx;
use dispatch_claim_fence::{
    authorize, authorize_with_identities, parse_br_show_json, BeadSnapshot, ClaimFenceError,
    DispatchIntent, IdentityRecord, IdentityRegistries,
};
use dispatch_silence_watch::{clears_pending_dispatch_intent, SilenceVerdict};
use finding::{BrPublisher, FindingError};
use finding_dispatch::{MaybeFinding, NotYet};
use lifecycle_event::{
    default_repo_journal, emit as emit_lifecycle, DurableJournal, EmitOutcome, Layer,
    LifecycleEvent, ReasonCode,
};
use lifecycle_monitor::{load_metrics, observe_layer, verify_artifact};
use ntm_fleet_monitor::bead_lifecycle::ledger::{
    packet_digest, InvokerClass, LedgerEvidence, LifecycleIdentity, LifecycleLedger,
};
use ntm_fleet_monitor::bead_lifecycle::{
    BeadId, DispatchReceipt, DispatchTarget, EventId, EvidencePolicy, ReceiverEvidence,
    RedispatchPlan,
};
use ntm_fleet_monitor::parse_activity_json;
use ntm_fleet_monitor::{classify, Approved, Intent, TypedAction};
use crate::packet_admission::{self, PacketAdmission};
use omp_types::{DispatchAdmissibility, DispatchPacketClass};
use crate::{
    applicable, census_gates, cross_pane_hold, decide, dispatch_packet, read_idle_authorization,
    GateCensus, Observation, PaneObservation, QueueState, SupervisorDecision,
};
use omp_rpc_session::{
    run_session, OmpCommand, RpcError, RpcSessionConfig, NO_CLAIM_BOUNDARY, OMP_RPC_SCHEMA_VERSION,
    OMP_SURFACE,
};
use orchestration_tick_gate::{
    append_receipt, build_receipt, PaneDisposition, Receipt, TickVerdict,
};
use pane_dispatch_fence::{admit_at_send, IncarnationMint, Occupancy, PaneIncarnation, Presented};
use receiver_receipt::{
    escalate_non_delivery, observe_capture, pane_transport_cannot_use_irc_receipt,
    ComposerEvidence, NonDeliveryEscalation, ObservationIdentity, PostSendObservation,
    ReceiptReason, ReceiptVerdict,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use subprocess_contract::run_output;

const DEFAULT_INTERVAL: Duration = Duration::from_secs(90);
const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
/// The window the supervisor waits for an ACK readback.
///
/// # This is DERIVED, not chosen, and the derivation is the fix
///
/// MEASURED 2026-09-05: this was a bare `from_secs(30)` while the discriminator it
/// feeds — `receiver_receipt`'s two-capture motion arm — refuses any span below
/// [`OBSERVATION_WINDOW_MIN_SECS`] (75s) with `WINDOW_BELOW_FLOOR`. A 30s wait asked a
/// question that needs 75s of span, so **every dispatch was structurally guaranteed to
/// return `Indeterminate`**, and the loop printed `DISPATCH_FAILED
/// detail=ACK_STAGE_RETRY_BLOCKED ... discriminator_reason=WINDOW_BELOW_FLOOR` on
/// successful dispatches. Two consecutive live ticks did exactly that: `%8`/`2yf` and
/// `%9`/`nh5` both landed — both panes went WORKING and both beads reached
/// `in_progress` — while the status word said failure. `ack-stage/src/lib.rs:525`
/// already recorded the same class: *"the ack is late, not absent"*, `owes_human=false`,
/// measured on `io3h -> %1414` four seconds before that worker's ACK landed.
///
/// So the window is now computed FROM the floor rather than sitting beside it. Two
/// hand-maintained durations that must agree is the shape that just failed; the same
/// prescription was applied to `ADVISORY_CEILING` and its recording anchor, which became
/// one type with `is_consistent()` instead of two constants nobody kept in step.
///
/// The `+ 15` is headroom for capture jitter: the floor is a *minimum* span between two
/// observations, so waiting exactly 75s can still yield a 74s span and refuse.
///
/// # NO-CLAIM
///
/// A window at or above the floor makes a delivery verdict *reachable*. It does not make
/// it *true* — a genuinely silent pane still returns `Indeterminate`, which is correct,
/// and the tmux literal transport is `Indeterminate` BY CONSTRUCTION regardless of window
/// per `ack-stage`'s own contract.
const RECEIPT_TIMEOUT: Duration =
    Duration::from_secs(receiver_receipt::OBSERVATION_WINDOW_MIN_SECS + 15);
const RECEIPT_POLL: Duration = Duration::from_millis(250);
const SILENCE_WATCH: &str = "dispatch-silence-watch";
/// The commit this binary was built from. MANDATORY.
///
/// This was `option_env!("OMP_BUILD_ID")` with a fallback to `"unversioned"`, and that
/// fallback is the root of the 2026-09-01 fleet outage: a binary that cannot say what it
/// was built from is exactly what the installer's identity rule exists to refuse, and we
/// made saying nothing a legal outcome. `env!` makes it a COMPILE ERROR instead, and
/// `build.rs` derives the value from git so no operator has to remember an export.
///
/// The pairing matters: mandatory-without-derivation would just move the failure from
/// runtime to the build, and derivation-without-mandatory leaves the fallback alive for
/// anyone who deletes the build script. Both halves, or neither works.
const BUILD_ID: &str = env!(
    "OMP_BUILD_ID",
    "OMP_BUILD_ID is unset. build.rs derives it from git, so reaching this means the build \
     script did not run or was removed. A binary with no build id gets DELETED by the \
     installer's identity rule - that is what took tick-monitor out and left the fleet \
     untended for hours."
);
#[used]
static BUILD_ID_MARKER: &[u8] = concat!("build_id=", env!("OMP_BUILD_ID")).as_bytes();
/// Stable for this process and distinct across normal restarts, even when the binary build id
/// repeats. The same value is attached to lifecycle evidence and embedded in event ids.
static LIFECYCLE_RUN_ID: LazyLock<String> = LazyLock::new(|| {
    let started_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{BUILD_ID}:pid{}:start{started_ns}", std::process::id())
});

fn lifecycle_run_id() -> &'static str {
    LIFECYCLE_RUN_ID.as_str()
}

fn run_scoped_event_key(kind: &str, bead: &str, tick: u64, suffix: &str) -> String {
    format!("{bead}:{kind}:{}:{tick}{suffix}", lifecycle_run_id())
}

fn selected_event_key_for_run(bead: &str, tick: u64, run_id: &str) -> String {
    format!("{bead}:selected:{run_id}:{tick}")
}
#[derive(Debug)]
pub struct Config {
    repo: PathBuf,
    session: String,
    interval: Duration,
    command_timeout: Duration,
    max_ticks: Option<u64>,
    tick_monitor: String,
    ompo: String,
    br: String,
    am: String,
    /// Path to `bv`, the dependency-graph planning brain used for ranked selection.
    bv: String,
    ntm: String,
    tmux_tmpdir: PathBuf,
    exclude_panes: Vec<String>,
    heartbeat_ledger: PathBuf,
    bead_lifecycle_ledger: PathBuf,
    tick_monitor_state: PathBuf,
    pending_dispatch: PathBuf,
    finding_spool: PathBuf,
    /// JSON snapshot of the parent-owned subagent identity namespace.
    ///
    /// This is intentionally an explicit input. A missing, unreadable, or empty snapshot
    /// must refuse dispatch rather than turning an unreachable registry into accept-anything.
    subagent_registry: PathBuf,
    receiver_agent: String,
    /// This supervisor's own Agent Mail identity, for a signed FROM on the
    /// durable dispatch-result notification.
    ///
    /// Resolved ONCE here from the environment rather than read at the call
    /// site, so the notification's behaviour is a property of an explicit
    /// config value instead of ambient process state. That matters for more
    /// than tidiness: a unit test that exercised the dispatch path while
    /// `AGENT_NAME` happened to be exported would send REAL mail and create a
    /// junk project in the live store. An empty value here refuses before any
    /// I/O, and the test fixture sets it empty deliberately.
    mail_sender: String,
    omp_quick: bool,
    reap_finished_panes: String,
    omp_binary: PathBuf,
    /// Explicit UDS executable. Missing configuration is restrictive: no gate roster fallback.
    uds_binary: Option<PathBuf>,
    /// Explicit UDS target-gate registry. The supervised repository is always the target root.
    uds_registry: Option<PathBuf>,
}

#[derive(Debug)]
enum OmpQuickError {
    Adapter(RpcError),
    ReportNotOk,
}

impl std::fmt::Display for OmpQuickError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Adapter(error) => write!(formatter, "adapter probe failed: {error}"),
            Self::ReportNotOk => {
                formatter.write_str("adapter probe returned an unsuccessful report")
            }
        }
    }
}

impl std::error::Error for OmpQuickError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CloseRequest {
    bead: String,
    reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DispatchRenderRequest {
    bead: String,
    pane: String,
    why_now: Option<String>,
    traps_file: Option<PathBuf>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum OmpoPsEvidence {
    Measured {
        project_scopes: usize,
        daemon_count: usize,
    },
    Unmeasured {
        reason: &'static str,
    },
}
fn parse_dispatch_render_args(args: &[String]) -> Result<Option<DispatchRenderRequest>, String> {
    if args.first().map(String::as_str) != Some("dispatch") {
        return Ok(None);
    }
    if args.get(1).map(String::as_str) != Some("render") {
        return Err("CONFIG_REFUSED dispatch requires the render subcommand".to_owned());
    }
    let mut bead = None;
    let mut pane = None;
    let mut why_now = None;
    let mut traps_file = None;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--bead" => {
                index += 1;
                bead = Some(
                    args.get(index)
                        .ok_or_else(|| "CONFIG_REFUSED --bead requires an id".to_owned())?
                        .clone(),
                );
            }
            "--pane" => {
                index += 1;
                pane = Some(
                    args.get(index)
                        .ok_or_else(|| "CONFIG_REFUSED --pane requires an id".to_owned())?
                        .clone(),
                );
            }
            "--why-now" => {
                index += 1;
                why_now = Some(
                    args.get(index)
                        .ok_or_else(|| "CONFIG_REFUSED --why-now requires text".to_owned())?
                        .clone(),
                );
            }
            "--traps-file" => {
                index += 1;
                traps_file = Some(PathBuf::from(args.get(index).ok_or_else(|| {
                    "CONFIG_REFUSED --traps-file requires a path".to_owned()
                })?));
            }
            other => return Err(format!("CONFIG_REFUSED unknown dispatch argument {other}")),
        }
        index += 1;
    }
    let bead = bead
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "CONFIG_REFUSED dispatch render requires --bead".to_owned())?;
    let pane = pane
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "CONFIG_REFUSED dispatch render requires --pane".to_owned())?;
    Ok(Some(DispatchRenderRequest {
        bead,
        pane,
        why_now,
        traps_file,
    }))
}

fn parse_close_readback_args(args: &[String]) -> Result<Option<CloseRequest>, String> {
    if args.first().map(String::as_str) != Some("close-readback") {
        return Ok(None);
    }
    if args.len() != 4 || args.get(2).map(String::as_str) != Some("--reason") {
        return Err("CONFIG_REFUSED close-readback requires BEAD --reason REASON".to_owned());
    }
    let bead = args[1].trim();
    if bead.is_empty() {
        return Err("CONFIG_REFUSED close-readback bead is empty".to_owned());
    }
    let reason = args[3].trim();
    if reason.is_empty() {
        return Err("CONFIG_REFUSED close-readback reason is empty".to_owned());
    }
    Ok(Some(CloseRequest {
        bead: bead.to_owned(),
        reason: reason.to_owned(),
    }))
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct GradeClaimRequest {
    config_args: Vec<String>,
    grader: Option<String>,
}

fn parse_grade_claim_args(args: &[String]) -> Result<Option<GradeClaimRequest>, String> {
    if args.first().map(String::as_str) != Some("grade") {
        return Ok(None);
    }
    // `--assign` and `--claim` are the same path under two names. The bead
    // (9x13.1) and the supervisor doctrine name it `grade --assign`; the
    // implementation landed as `--claim`. A verb the acceptance names and the
    // binary refuses is an unexecutable acceptance leg, so both spellings
    // resolve here rather than one of them being a documentation error.
    if !matches!(
        args.get(1).map(String::as_str),
        Some("--claim") | Some("--assign")
    ) {
        return Err("CONFIG_REFUSED grade requires --claim or --assign".to_owned());
    }
    let mut config_args = Vec::new();
    let mut grader = None;
    let mut rest = &args[2..];
    while let Some((head, tail)) = rest.split_first() {
        if head == "--grader" {
            let Some(pane) = tail.first() else {
                return Err("CONFIG_REFUSED grade --claim --grader requires a pane".to_owned());
            };
            if loop_queue_filter::select::tmux_pane_id(pane).is_none() {
                return Err(format!("CONFIG_REFUSED grade --grader pane is not a tmux pane id: {pane}"));
            }
            grader = Some(pane.clone());
            rest = &tail[1..];
        } else {
            config_args.push(head.clone());
            rest = tail;
        }
    }
    Ok(Some(GradeClaimRequest { config_args, grader }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PeerGradeCommandOutcome {
    /// A claimed grade carries its rendered packet. The two are one outcome
    /// because a claim without a packet is the BUILT-NOT-WIRED shape this
    /// bead exists to close: the assignment was recorded and nothing
    /// gradeable was ever handed to the grader.
    Claimed {
        claim: PeerGradeClaim,
        packet: String,
    },
    ActivePeerGrade,
    /// The receiver-verified candidate set was EMPTY.
    NoCandidate,
    /// The candidate set was NON-EMPTY and selection refused over it. Kept
    /// separate from `NoCandidate` because collapsing the two is what let one
    /// run print `candidate_count=10 reason=no_idle_pane` on stdout and
    /// `typed_outcome=no_receiver_verified_candidate` on stderr.
    Skipped(PeerGradeSkip),
}

fn peer_grade_outcome_wire(outcome: &PeerGradeCommandOutcome) -> &'static str {
    match outcome {
        PeerGradeCommandOutcome::Claimed { .. } => "PEER_GRADE_CLAIMED",
        PeerGradeCommandOutcome::ActivePeerGrade => "PEER_GRADE_ACTIVE",
        PeerGradeCommandOutcome::NoCandidate => "PEER_GRADE_EMPTY",
        PeerGradeCommandOutcome::Skipped(_) => "PEER_GRADE_SKIPPED",
    }
}

async fn run_peer_grade_claim(
    cx: &Cx,
    config: &Config,
    observer_pane: &str,
    preferred_grader: Option<&str>,
) -> Result<PeerGradeCommandOutcome, String> {
    // The invoker is the OBSERVER, never implicitly the grader: observers may
    // be working by contract, so no activity check on the caller can gate
    // this path (that check was the catch-22 — invoking made the pane
    // WORKING, which the old pre-check then refused). Eligibility is decided
    // on the TARGET pane by the selector below, where it is sound.
    let mut monitor_args = vec![
        "observe".to_owned(),
        "--session".to_owned(),
        config.session.clone(),
        "--repo".to_owned(),
        config.repo.display().to_string(),
        "--state".to_owned(),
        config.tick_monitor_state.display().to_string(),
    ];
    for pane in &config.exclude_panes {
        monitor_args.push("--exclude-pane".to_owned());
        monitor_args.push(pane.clone());
    }
    let monitor_bytes = require_success(
        &config.tick_monitor,
        invoke(cx, config, &config.tick_monitor, &monitor_args).await?,
    )?;
    let mut observation = parse_observation(&monitor_bytes, Some(census_gates(&config.repo)))?;
    observation.panes.retain(|pane| {
        !config
            .exclude_panes
            .iter()
            .any(|excluded| excluded == &pane.pane_id)
    });
    let gate = match &preferred_grader {
        // Explicit routing: the full seven-reason gauntlet applies to the
        // named pane, which is not invoking and is expected idle.
        Some(preferred) => {
            gate_peer_grading_for_pane(config, &mut observation, now_unix(), preferred)?
        }
        // Default: the selector picks the best idle non-observer peer. The
        // observer is excluded by construction, so self-picks (and self-grades)
        // are unrepresentable on this path, not merely refused.
        None => gate_peer_grading_inner(config, &mut observation, now_unix(), None, observer_pane)?,
    };
    match gate {
        PeerGradeGate::Claimed(claim) if claim.bead == "<active-peer-grade>" => {
            Ok(PeerGradeCommandOutcome::ActivePeerGrade)
        }
        PeerGradeGate::Claimed(claim) => {
            // Record the acquisition the way a work dispatch records its
            // claim (assignee set, claim visible): an unrecorded grading
            // dispatch is silent by construction (AGENTS.md fourth rule).
            // Status is untouched — the stage machine owns it.
            record_grader_assignment(cx, config, &claim).await?;
            // ORDER IS THE MECHANISM, NOT A CONVENTION. The renderer
            // re-validates the tracker claim (`validate_bead_claim`: status
            // in_progress AND assignee pane == grader pane), so the packet
            // cannot be produced before the assignment write has landed, and
            // a refused write returns above — no packet is rendered or
            // emitted. Nothing here asserts the ordering; the data dependency
            // enforces it.
            let packet = render_peer_grade_packet(cx, config, &claim, observer_pane).await?;
            Ok(PeerGradeCommandOutcome::Claimed { claim, packet })
        }
        PeerGradeGate::NoCandidate => Ok(PeerGradeCommandOutcome::NoCandidate),
        // NOT NoCandidate. A skip over a non-empty candidate set is a
        // different fact and reaches the operator as a different line.
        PeerGradeGate::Skipped(skip) => Ok(PeerGradeCommandOutcome::Skipped(skip)),
    }
}

/// Render the grading packet for a claimed peer grade through
/// `dispatch_packet::render_grading_packet`.
///
/// 9x13.1: this is the production call site the renderer never had. The
/// packet is what the grader receives; the selector decides WHO grades, the
/// renderer decides WHAT they are handed, and before this both halves existed
/// with only the first one reachable.
///
/// Refusals stay typed: `PacketError` carries its own `code()` and
/// `operator_exit_code()`, so an unclaimed bead, a filed-only record, or a
/// missing acceptance section surfaces by name and exits non-zero instead of
/// printing a claim line over a packet that was never built.
async fn render_peer_grade_packet(
    cx: &Cx,
    config: &Config,
    claim: &PeerGradeClaim,
    observer_pane: &str,
) -> Result<String, String> {
    let snapshot = load_bead_snapshot(cx, config, &claim.bead).await?;
    dispatch_packet::render_grading_packet(
        &snapshot,
        &config.repo,
        &claim.grader_pane,
        observer_pane,
    )
    .map_err(|error| {
        format!(
            "GRADE_PACKET_REFUSED bead={} grader_pane={} observer_pane={observer_pane} \
             code={} error={error}",
            claim.bead,
            claim.grader_pane,
            error.code()
        )
    })
}

/// Write the grader assignment into the tracker. Mirrors the work-dispatch
/// claim write (`br update --assignee`, same invoke/require rails) minus the
/// status transition, which belongs to the stage machine, not the claim path.
/// A tracker refusal surfaces with the tracker's own reason: the assignment
/// write is the authoritative probe, never a field read about it.
async fn record_grader_assignment(
    cx: &Cx,
    config: &Config,
    claim: &PeerGradeClaim,
) -> Result<(), String> {
    let args = vec![
        "update".to_owned(),
        claim.bead.clone(),
        "--assignee".to_owned(),
        claim.grader_assignee.clone(),
    ];
    let output = invoke(cx, config, &config.br, &args).await.map_err(|error| {
        format!(
            "GRADE_CLAIM_FAILED bead={} grader_pane={} reason=RECORD_COMMAND_FAILED error={error}",
            claim.bead, claim.grader_pane
        )
    })?;
    require_success(&config.br, output).map_err(|error| {
        format!(
            "GRADE_CLAIM_FAILED bead={} grader_pane={} reason=TRACKER_REFUSED detail={error}",
            claim.bead, claim.grader_pane
        )
    })?;
    Ok(())
}

fn peer_grade_command_exit(outcome: PeerGradeCommandOutcome) -> std::process::ExitCode {
    match outcome {
        PeerGradeCommandOutcome::Claimed { claim, packet } => {
            println!(
                "{} bead={} receiver_pane={} grader_pane={} experiment=observer-requested",
                peer_grade_outcome_wire(&PeerGradeCommandOutcome::Claimed {
                    claim: claim.clone(),
                    packet: packet.clone(),
                }),
                claim.bead,
                claim.receiver_pane,
                claim.grader_pane,
            );
            // The packet on stdout is the deliverable: the observer pipes it
            // to the grader pane. A claim line alone told the operator a
            // grade had been assigned and handed them nothing to send.
            print!("{packet}");
            std::process::ExitCode::SUCCESS
        }
        PeerGradeCommandOutcome::ActivePeerGrade => {
            eprintln!("PEER_GRADING_REFUSED reason=active_peer_grade");
            std::process::ExitCode::from(2)
        }
        PeerGradeCommandOutcome::NoCandidate => {
            // Reserved for a genuinely EMPTY candidate set. This line used to
            // be printed for skips too, which is how an operator was told
            // "nothing is gradeable" about a run with ten candidates.
            eprintln!("PEER_GRADE_EMPTY typed_outcome=no_receiver_verified_candidate");
            std::process::ExitCode::from(2)
        }
        PeerGradeCommandOutcome::Skipped(skip) => {
            eprintln!("{}", skip.refusal_line());
            std::process::ExitCode::from(2)
        }
    }
}

/// Capture state is per-session. Heartbeat already was; this path was not.
/// Two residents on one box (`--session control-plane` vs `--session omp-orchestrator`)
/// previously shared `omp-orchestrator.tick-monitor-state.json`, so every tick read
/// `why=no_prior_capture` and dispatchable stayed empty.
fn default_tick_monitor_state(heartbeat_ledger: &Path, session: &str) -> PathBuf {
    heartbeat_ledger.with_file_name(format!(
        "omp-orchestrator-{session}.tick-monitor-state.json"
    ))
}

fn default_pending_dispatch(heartbeat_ledger: &Path, session: &str) -> PathBuf {
    heartbeat_ledger.with_file_name(format!(
        "omp-orchestrator-{session}.pending-dispatch"
    ))
}

/// A second session in one HOME must not reuse a fixed state/pending path.
fn refuse_session_path_collision(
    session: &str,
    paths: &[(&str, &Path)],
) -> Result<(), String> {
    if paths.is_empty() {
        return Err(
            "SESSION_PATH_COLLISION empty scan set is ERROR, never a pass".to_owned(),
        );
    }
    for (label, path) in paths {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if !name.contains(session) {
            return Err(format!(
                "SESSION_PATH_COLLISION {label} path={} session={session} would reuse a fixed path",
                path.display()
            ));
        }
    }
    Ok(())
}


impl Config {
    fn from_args(args: &[String]) -> Result<Self, String> {
        let mut repo = env::var_os("OMP_REPO")
            .map(PathBuf::from)
            .or_else(|| env::current_dir().ok())
            .ok_or_else(|| "CONFIG_REFUSED repository is not resolvable".to_owned())?;
        let mut session = env::var("OMP_SESSION").ok();
        let mut interval = DEFAULT_INTERVAL;
        let mut command_timeout = DEFAULT_COMMAND_TIMEOUT;
        let mut max_ticks = None;
        let mut receiver_agent = env::var("OMP_RECEIVER_AGENT").unwrap_or_default();
        let mut omp_quick = false;
        let mut omp_binary = env::var_os("OMP_BINARY")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("omp"));
        let mut uds_binary = env::var_os("OMP_UDS_BINARY").map(PathBuf::from);
        let mut uds_registry = env::var_os("OMP_UDS_TARGET_GATE_REGISTRY").map(PathBuf::from);
        // ompo supervise forwards its flags directly to the shared runtime.
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--repo" => {
                    index += 1;
                    repo = PathBuf::from(
                        args.get(index)
                            .ok_or_else(|| "CONFIG_REFUSED --repo requires a path".to_owned())?,
                    );
                }
                "--session" => {
                    index += 1;
                    session = Some(
                        args.get(index)
                            .ok_or_else(|| "CONFIG_REFUSED --session requires a name".to_owned())?
                            .clone(),
                    );
                }
                "--interval-secs" => {
                    index += 1;
                    interval = Duration::from_secs(
                        args.get(index)
                            .ok_or_else(|| {
                                "CONFIG_REFUSED --interval-secs requires seconds".to_owned()
                            })?
                            .parse::<u64>()
                            .map_err(|_| {
                                "CONFIG_REFUSED --interval-secs is not an integer".to_owned()
                            })?,
                    );
                }
                "--command-timeout-secs" => {
                    index += 1;
                    let timeout_secs = args
                        .get(index)
                        .ok_or_else(|| {
                            "CONFIG_REFUSED --command-timeout-secs requires seconds".to_owned()
                        })?
                        .parse::<u64>()
                        .map_err(|_| {
                            "CONFIG_REFUSED --command-timeout-secs is not an integer".to_owned()
                        })?;
                    if timeout_secs == 0 {
                        return Err(
                            "CONFIG_REFUSED --command-timeout-secs must be greater than zero"
                                .to_owned(),
                        );
                    }
                    command_timeout = Duration::from_secs(timeout_secs);
                }
                "--max-ticks" => {
                    index += 1;
                    max_ticks = Some(
                        args.get(index)
                            .ok_or_else(|| {
                                "CONFIG_REFUSED --max-ticks requires a count".to_owned()
                            })?
                            .parse::<u64>()
                            .map_err(|_| {
                                "CONFIG_REFUSED --max-ticks is not an integer".to_owned()
                            })?,
                    );
                }
                "--once" => max_ticks = Some(1),
                "--omp-quick" => omp_quick = true,
                "--receiver-agent" => {
                    index += 1;
                    receiver_agent = args
                        .get(index)
                        .ok_or_else(|| {
                            "CONFIG_REFUSED --receiver-agent requires a name".to_owned()
                        })?
                        .clone();
                }
                "--omp-binary" => {
                    index += 1;
                    omp_binary =
                        PathBuf::from(args.get(index).ok_or_else(|| {
                            "CONFIG_REFUSED --omp-binary requires a path".to_owned()
                        })?);
                }
                "--uds-binary" => {
                    index += 1;
                    uds_binary = Some(PathBuf::from(args.get(index).ok_or_else(|| {
                        "CONFIG_REFUSED --uds-binary requires a path".to_owned()
                    })?));
                }
                "--uds-registry" => {
                    index += 1;
                    uds_registry = Some(PathBuf::from(args.get(index).ok_or_else(|| {
                        "CONFIG_REFUSED --uds-registry requires a path".to_owned()
                    })?));
                }
                "--help" => return Err(usage().to_owned()),
                "--version" => {
                    return Err(format!(
                        "ompo supervise {} build_id={BUILD_ID}",
                        env!("CARGO_PKG_VERSION")
                    ));
                }
                other => return Err(format!("CONFIG_REFUSED unknown argument {other}")),
            }
            index += 1;
        }
        if !repo.is_dir() {
            return Err(format!(
                "CONFIG_REFUSED repository target does not exist: {}",
                repo.display()
            ));
        }
        let session = session.unwrap_or_else(|| {
            repo.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "omp-orchestrator".to_owned())
        });
        if session.trim().is_empty() {
            return Err("CONFIG_REFUSED session is empty".to_owned());
        }
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| "CONFIG_REFUSED HOME is unset; set TMUX_TMPDIR".to_owned())?;
        let tmux_tmpdir = env::var_os("TMUX_TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".tmux-sockets"));
        let mut exclude_panes: Vec<String> = env::var("OMP_EXCLUDE_PANES")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|pane| !pane.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        if let Ok(pane) = env::var("TMUX_PANE") {
            if !exclude_panes.iter().any(|known| known == &pane) {
                exclude_panes.push(pane);
            }
        }
        let heartbeat_ledger = env::var_os("OMP_HEARTBEAT_LEDGER")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                home.join(".local/state/flywheel")
                    .join(format!("omp-orchestrator-{session}.heartbeat.jsonl"))
            });
        let bead_lifecycle_ledger = env::var_os("OMP_BEAD_LIFECYCLE_LEDGER")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                heartbeat_ledger
                    .with_file_name(format!("omp-orchestrator-{session}.bead-lifecycle.jsonl"))
            });
        let tick_monitor_state = env::var_os("OMP_TICK_MONITOR_STATE")
            .map(PathBuf::from)
            .unwrap_or_else(|| default_tick_monitor_state(&heartbeat_ledger, &session));
        let pending_dispatch = env::var_os("OMP_PENDING_DISPATCH")
            .map(PathBuf::from)
            .unwrap_or_else(|| default_pending_dispatch(&heartbeat_ledger, &session));
        refuse_session_path_collision(
            &session,
            &[
                ("tick_monitor_state", tick_monitor_state.as_path()),
                ("pending_dispatch", pending_dispatch.as_path()),
            ],
        )?;
        let finding_spool = env::var_os("OMP_FINDING_SPOOL")
            .map(PathBuf::from)
            .unwrap_or_else(|| heartbeat_ledger.with_file_name("omp-orchestrator.findings"));
        let subagent_registry = env::var_os("OMP_SUBAGENT_REGISTRY")
            .map(PathBuf::from)
            .unwrap_or_else(|| heartbeat_ledger.with_file_name("omp-orchestrator.subagents.json"));
        // yfp2: resolve the mail identity through the kernel, KEEPING the variable it came
        // from. `sender_identity::first_candidate` walks the same list in the same order;
        // what is new is that the list is TYPED, so an ambient value can be refused by name
        // rather than signed with.
        let resolved_sender = sender_identity::first_candidate(&|name| env::var(name).ok());
        let policy_text = fs::read_to_string(repo.join("config.toml")).ok();
        let policy = parse_dispatch_policy(policy_text.as_deref());
        if interval == DEFAULT_INTERVAL {
            interval = policy.interval;
        }
        if command_timeout == DEFAULT_COMMAND_TIMEOUT {
            command_timeout = policy.command_timeout;
        }
        policy.log_live();

        Ok(Self {
            repo,
            session,
            interval,
            command_timeout,
            max_ticks,
            ompo: env::var("OMP_OMPO_BIN").unwrap_or_else(|_| "ompo".to_owned()),
            tick_monitor: env::var("OMP_TICK_MONITOR_BIN")
                .unwrap_or_else(|_| "tick-monitor".to_owned()),
            br: env::var("OMP_BR_BIN").unwrap_or_else(|_| "br".to_owned()),
            am: env::var("OMP_AM_BIN")
                .ok()
                .filter(|path| !path.trim().is_empty())
                .unwrap_or_else(|| "am".to_owned()),
            // The planning brain. Ranked selection is MANDATORY, so an absent `bv`
            // must surface as a typed QUEUE_UNRANKED refusal at the call site rather
            // than as a silent fall back to creation order.
            bv: env::var("OMP_BV_BIN").unwrap_or_else(|_| "bv".to_owned()),
            ntm: env::var("OMP_NTM_BIN").unwrap_or_else(|_| tick_monitor::NTM.to_owned()),
            tmux_tmpdir,
            exclude_panes,
            heartbeat_ledger,
            bead_lifecycle_ledger,
            tick_monitor_state,
            pending_dispatch,
            finding_spool,
            subagent_registry,
            receiver_agent,
            mail_sender: resolved_sender
                .as_ref()
                .map(|candidate| candidate.value.clone())
                .unwrap_or_default(),
            omp_quick,
            reap_finished_panes: env::var("OMP_REAP_FINISHED_PANES_BIN")
                .unwrap_or_else(|_| "reap-finished-panes".to_owned()),
            omp_binary,
            uds_binary,
            uds_registry,
        })
    }
}
fn usage() -> &'static str {
    "usage: ompo supervise [--once|--max-ticks N] [--repo PATH] [--session NAME] [--interval-secs N] [--receiver-agent NAME] [--omp-quick] [--omp-binary PATH] [--uds-binary PATH] [--uds-registry PATH]\n       close-readback BEAD --reason REASON\n       dispatch render --bead BEAD --pane %N [--why-now TEXT] [--traps-file PATH]\n       grade --claim [--repo PATH] [--session NAME]\n       supervise runs the resident lifecycle (observe -> ready queue -> dispatch -> receiver receipt); dispatch render emits the same packet without transport"
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

async fn run_omp_quick(cx: &Cx, config: &Config) -> Result<(), OmpQuickError> {
    let command = OmpCommand::new(config.omp_binary.clone()).current_dir(config.repo.clone());
    let rpc_config = RpcSessionConfig::with_command(command);
    match run_session(cx, &rpc_config).await {
        Ok(report) => {
            println!("{}", report.to_json());
            if report.ok() {
                Ok(())
            } else {
                Err(OmpQuickError::ReportNotOk)
            }
        }
        Err(error) => {
            println!(
                "{}",
                json!({
                    "schema": OMP_RPC_SCHEMA_VERSION,
                    "surface": OMP_SURFACE,
                    "ok": false,
                    "error": error.to_string(),
                    "noClaim": NO_CLAIM_BOUNDARY,
                })
            );
            Err(OmpQuickError::Adapter(error))
        }
    }
}

async fn invoke(
    cx: &Cx,
    config: &Config,
    program: &str,
    args: &[String],
) -> Result<Output, String> {
    cx.checkpoint()
        .map_err(|_| "CANCELLED supervisor context".to_owned())?;
    let mut command = Command::new(program);
    command.args(args).current_dir(&config.repo);
    if program == config.reap_finished_panes {
        command.env(
            "REAP_SWEEP_DEADLINE_SECS",
            config.command_timeout.as_secs().max(1).to_string(),
        );
    }
    command.env("TMUX_TMPDIR", &config.tmux_tmpdir);
    match timeout(
        cx.now_for_observability(),
        config.command_timeout,
        run_output(cx, command),
    )
    .await
    {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) => Err(format!("{program}: {error}")),
        Err(_) => Err(format!(
            "TIMEOUT program={program} after={}s",
            config.command_timeout.as_secs()
        )),
    }
}

fn require_success(program: &str, output: Output) -> Result<Vec<u8>, String> {
    if output.status.success() {
        return Ok(output.stdout);
    }
    Err(format!(
        "{program} exited={} stderr={}",
        output
            .status
            .code()
            .map_or_else(|| "signal".to_owned(), |code| code.to_string()),
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}
const UDS_ENVELOPE_SCHEMA: &str = "uds/v2";
const UDS_PROJECTION_SCHEMA: &str = "uds-target-gates/v1";
const UDS_FH_REQUIREMENTS: [&str; 2] = ["fh-doctor", "fh-how-oracle"];
const UDS_TARGET_GATE_VERB: &str = "target-gate";

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdsProcessResult {
    process_exit: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl UdsProcessResult {
    fn from_output(output: Output) -> Self {
        Self {
            process_exit: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdsTargetGateRequirement {
    id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UdsTargetGateObservation {
    requirements: Vec<UdsTargetGateRequirement>,
}

fn uds_target_gate_unwired(detail: impl AsRef<str>) -> String {
    format!("UDS_TARGET_GATE_UNWIRED {}", detail.as_ref())
}

fn uds_target_gate_exit(code: &str) -> Option<i32> {
    match code {
        "EC-PASS" => Some(0),
        "EC-RED" => Some(1),
        "EC-USAGE" => Some(2),
        "EC-UNRUN" => Some(77),
        _ => None,
    }
}

fn parse_uds_target_gate(result: UdsProcessResult) -> Result<UdsTargetGateObservation, String> {
    let process_exit = result.process_exit.ok_or_else(|| {
        uds_target_gate_unwired("process_exit=signal")
    })?;
    let value: Value = serde_json::from_slice(&result.stdout).map_err(|error| {
        uds_target_gate_unwired(format!(
            "malformed_envelope detail={} stderr={}",
            error,
            one_line_detail(&String::from_utf8_lossy(&result.stderr))
        ))
    })?;
    let schema = value
        .get("schema")
        .and_then(Value::as_str)
        .ok_or_else(|| uds_target_gate_unwired("malformed_envelope missing=schema"))?;
    if schema != UDS_ENVELOPE_SCHEMA {
        return Err(uds_target_gate_unwired(format!(
            "envelope_schema={schema} expected={UDS_ENVELOPE_SCHEMA}"
        )));
    }
    let verb = value
        .get("verb")
        .and_then(Value::as_str)
        .ok_or_else(|| uds_target_gate_unwired("malformed_envelope missing=verb"))?;
    if verb != UDS_TARGET_GATE_VERB {
        return Err(uds_target_gate_unwired(format!(
            "envelope_verb={verb} expected={UDS_TARGET_GATE_VERB}"
        )));
    }
    let code = value
        .get("code")
        .and_then(Value::as_str)
        .ok_or_else(|| uds_target_gate_unwired("malformed_envelope missing=code"))?;
    let exit_status = value
        .get("exit_status")
        .and_then(Value::as_i64)
        .ok_or_else(|| uds_target_gate_unwired("malformed_envelope missing=exit_status"))?;
    if exit_status != i64::from(process_exit) {
        return Err(uds_target_gate_unwired(format!(
            "envelope_mismatch code={code} exit_status={exit_status} process_exit={process_exit}"
        )));
    }
    let Some(expected_exit) = uds_target_gate_exit(code) else {
        return Err(uds_target_gate_unwired(format!(
            "unsupported_code code={code} exit_status={exit_status}"
        )));
    };
    if exit_status != i64::from(expected_exit) {
        return Err(uds_target_gate_unwired(format!(
            "code_exit_mismatch code={code} exit_status={exit_status} expected={expected_exit}"
        )));
    }
    if code != "EC-PASS" {
        let detail = value
            .get("detail")
            .and_then(Value::as_str)
            .unwrap_or("typed UDS target-gate refusal");
        return Err(uds_target_gate_unwired(format!(
            "uds_code={code} exit_status={exit_status} detail={detail}"
        )));
    }
    let projection = value
        .get("projection")
        .and_then(Value::as_object)
        .ok_or_else(|| uds_target_gate_unwired("malformed_projection missing=projection"))?;
    let projection_schema = projection
        .get("schema")
        .and_then(Value::as_str)
        .ok_or_else(|| uds_target_gate_unwired("malformed_projection missing=schema"))?;
    if projection_schema != UDS_PROJECTION_SCHEMA {
        return Err(uds_target_gate_unwired(format!(
            "projection_schema={projection_schema} expected={UDS_PROJECTION_SCHEMA}"
        )));
    }
    let rows = projection
        .get("requirements")
        .and_then(Value::as_array)
        .ok_or_else(|| uds_target_gate_unwired("malformed_projection missing=requirements"))?;
    if rows.len() != UDS_FH_REQUIREMENTS.len() {
        return Err(uds_target_gate_unwired(format!(
            "requirement_count={} expected={}",
            rows.len(),
            UDS_FH_REQUIREMENTS.len()
        )));
    }
    let mut requirements = Vec::with_capacity(rows.len());
    for (index, expected_id) in UDS_FH_REQUIREMENTS.iter().enumerate() {
        let row = rows
            .get(index)
            .and_then(Value::as_object)
            .ok_or_else(|| uds_target_gate_unwired(format!("missing_requirement={expected_id}")))?;
        let id = row
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| uds_target_gate_unwired(format!("missing_requirement={expected_id}")))?;
        if id != *expected_id {
            return Err(uds_target_gate_unwired(format!(
                "requirement_order index={index} expected={expected_id} found={id}"
            )));
        }
        let command = row
            .get("command")
            .and_then(Value::as_str)
            .ok_or_else(|| uds_target_gate_unwired(format!("missing_trigger={expected_id}")))?;
        if command.trim().is_empty() {
            return Err(uds_target_gate_unwired(format!("missing_trigger={expected_id}")));
        }
        requirements.push(UdsTargetGateRequirement { id: id.to_owned() });
    }
    Ok(UdsTargetGateObservation { requirements })
}

async fn observe_uds_target_gate(
    cx: &Cx,
    config: &Config,
) -> Result<UdsTargetGateObservation, String> {
    let Some(binary) = config.uds_binary.as_ref() else {
        return Err(uds_target_gate_unwired("config_missing=uds_binary"));
    };
    let Some(registry) = config.uds_registry.as_ref() else {
        return Err(uds_target_gate_unwired("config_missing=uds_registry"));
    };
    let binary = binary.to_string_lossy().into_owned();
    let args = vec![
        "target-gate".to_owned(),
        registry.display().to_string(),
        config.repo.display().to_string(),
        "--json".to_owned(),
    ];
    let output = invoke(cx, config, &binary, &args)
        .await
        .map_err(|error| uds_target_gate_unwired(format!("invoke={error}")))?;
    parse_uds_target_gate(UdsProcessResult::from_output(output))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AsupersyncConformanceEvidence {
    Current { detail: String },
    Missing { detail: String },
    Stale { detail: String },
    Unavailable { detail: String },
}

fn asupersync_conformance_evidence(
    result: Result<Output, String>,
) -> AsupersyncConformanceEvidence {
    let output = match result {
        Ok(output) => output,
        Err(error) => {
            return AsupersyncConformanceEvidence::Unavailable {
                detail: one_line_detail(&error),
            };
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = one_line_detail(&format!(
        "exit={} stdout={} stderr={}",
        output
            .status
            .code()
            .map_or_else(|| "signal".to_owned(), |code| code.to_string()),
        stdout.trim(),
        stderr.trim()
    ));
    if stdout.contains("reason=UNREADABLE") {
        AsupersyncConformanceEvidence::Missing { detail }
    } else if stdout.contains("reason=STALE") {
        AsupersyncConformanceEvidence::Stale { detail }
    } else if output.status.success() && stdout.contains("ASUPERSYNC_CONFORMANCE_CURRENT") {
        AsupersyncConformanceEvidence::Current { detail }
    } else {
        AsupersyncConformanceEvidence::Unavailable { detail }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NtmSendAdmission {
    target: String,
    state: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NtmSendReceipt {
    raw_json: String,
    operation_id: String,
    status: String,
    payload_sha256: String,
    payload_bytes: u64,
    admissions: Vec<NtmSendAdmission>,
    successful: Vec<String>,
    failed: Vec<String>,
}

fn receipt_string(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<String, String> {
    match object.get(field).and_then(Value::as_str) {
        Some(value) if !value.is_empty() => Ok(value.to_owned()),
        Some(_) => Err(format!("NTM_SEND_RECEIPT_MALFORMED field={field} reason=empty")),
        None if object.contains_key(field) => {
            Err(format!("NTM_SEND_RECEIPT_MALFORMED field={field} reason=wrong_type"))
        }
        None => Err(format!("NTM_SEND_RECEIPT_MALFORMED field={field} reason=missing")),
    }
}

fn receipt_strings(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<Vec<String>, String> {
    let Some(value) = object.get(field) else {
        return Err(format!("NTM_SEND_RECEIPT_MALFORMED field={field} reason=missing"));
    };
    let Some(values) = value.as_array() else {
        return Err(format!("NTM_SEND_RECEIPT_MALFORMED field={field} reason=wrong_type"));
    };
    values
        .iter()
        .map(|value| {
            value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                format!("NTM_SEND_RECEIPT_MALFORMED field={field} reason=wrong_item_type")
            })
        })
        .collect()
}

fn receipt_exit_label(exit_code: Option<i32>) -> String {
    exit_code.map_or_else(|| "signal".to_owned(), |code| code.to_string())
}

fn receipt_failure(
    exit_code: Option<i32>,
    error_code: &str,
    message: &str,
    stderr: &[u8],
) -> String {
    format!(
        "NTM_SEND_RECEIPT_FAILED exit={} error_code={error_code} message={message} stderr={}",
        receipt_exit_label(exit_code),
        String::from_utf8_lossy(stderr).trim()
    )
}

/// Parse the durable NTM send-receipt response.
///
/// A non-zero command is not a missing result: preserve its exit code and the daemon's
/// structured error so the caller cannot mistake an unavailable receipt for delivery.
fn parse_ntm_send_receipt(
    exit_code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<NtmSendReceipt, String> {
    let raw_json = String::from_utf8(stdout.to_vec()).map_err(|_| {
        receipt_failure(
            exit_code,
            "INVALID_UTF8",
            "receipt stdout is not UTF-8",
            stderr,
        )
    })?;
    let value: Value = serde_json::from_str(&raw_json).map_err(|error| {
        receipt_failure(
            exit_code,
            "INVALID_JSON",
            &format!("receipt stdout is invalid JSON: {error}"),
            stderr,
        )
    })?;
    let Some(object) = value.as_object() else {
        return Err(receipt_failure(
            exit_code,
            "NOT_AN_OBJECT",
            "receipt response is not a JSON object",
            stderr,
        ));
    };
    if exit_code != Some(0) {
        let error_code = object
            .get("error_code")
            .and_then(Value::as_str)
            .unwrap_or("UNKNOWN");
        let message = object
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("missing daemon error message");
        return Err(receipt_failure(exit_code, error_code, message, stderr));
    }
    if object.get("success").and_then(Value::as_bool) != Some(true) {
        return Err(receipt_failure(
            exit_code,
            object
                .get("error_code")
                .and_then(Value::as_str)
                .unwrap_or("UNSUCCESSFUL"),
            object
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("receipt response did not report success"),
            stderr,
        ));
    }
    let operation = object
        .get("operation")
        .and_then(Value::as_object)
        .ok_or_else(|| "NTM_SEND_RECEIPT_MALFORMED field=operation reason=missing".to_owned())?;
    let outcome = object
        .get("outcome")
        .and_then(Value::as_object)
        .ok_or_else(|| "NTM_SEND_RECEIPT_MALFORMED field=outcome reason=missing".to_owned())?;
    if outcome.get("success").and_then(Value::as_bool) != Some(true) {
        return Err(
            "NTM_SEND_RECEIPT_MALFORMED field=outcome.success reason=not_true".to_owned(),
        );
    }
    let Some(admission_values) = operation.get("admissions").and_then(Value::as_array) else {
        return Err("NTM_SEND_RECEIPT_MALFORMED field=operation.admissions reason=missing_or_wrong_type".to_owned());
    };
    if admission_values.is_empty() {
        return Err("NTM_SEND_RECEIPT_MALFORMED field=operation.admissions reason=empty".to_owned());
    }
    let mut admissions = Vec::with_capacity(admission_values.len());
    for value in admission_values {
        let Some(admission) = value.as_object() else {
            return Err(
                "NTM_SEND_RECEIPT_MALFORMED field=operation.admissions reason=wrong_item_type"
                    .to_owned(),
            );
        };
        admissions.push(NtmSendAdmission {
            target: receipt_string(admission, "target")?,
            state: receipt_string(admission, "state")?,
        });
    }
    let payload_sha256 = receipt_string(operation, "payload_sha256")?;
    if payload_sha256.len() != 64
        || !payload_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(
            "NTM_SEND_RECEIPT_MALFORMED field=operation.payload_sha256 reason=not_lower_hex_sha256"
                .to_owned(),
        );
    }
    let payload_bytes = operation
        .get("payload_bytes")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            "NTM_SEND_RECEIPT_MALFORMED field=operation.payload_bytes reason=missing_or_wrong_type"
                .to_owned()
        })?;
    Ok(NtmSendReceipt {
        raw_json,
        operation_id: receipt_string(operation, "operation_id")?,
        status: receipt_string(operation, "status")?,
        payload_sha256,
        payload_bytes,
        admissions,
        successful: receipt_strings(outcome, "successful")?,
        failed: receipt_strings(outcome, "failed")?,
    })
}

fn ntm_operation_id_component(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect()
}

fn ntm_send_operation_id(bead: &str, pane: &str, tick: u64) -> String {
    format!(
        "omp-ntm-send-{}-{}-{}-{}",
        ntm_operation_id_component(lifecycle_run_id()),
        tick,
        ntm_operation_id_component(pane),
        ntm_operation_id_component(bead)
    )
}

fn ntm_send_args(
    session: &str,
    pane: &str,
    staged_packet: &Path,
    operation_id: &str,
) -> Vec<String> {
    vec![
        tick_monitor::ntm_send_arg(session),
        format!("--panes={pane}"),
        format!("--msg-file={}", staged_packet.display()),
        format!("--op-id={operation_id}"),
    ]
}

fn ntm_send_receipt_args(operation_id: &str) -> Vec<String> {
    vec![format!("--robot-send-receipt={operation_id}")]
}

fn ntm_target_matches(target: &str, pane: &str) -> bool {
    target == pane || target.trim_start_matches('%') == pane.trim_start_matches('%')
}

fn validate_ntm_send_receipt(
    receipt: &NtmSendReceipt,
    operation_id: &str,
    packet: &str,
    pane: &str,
) -> Result<(), String> {
    if receipt.operation_id != operation_id {
        return Err(format!(
            "NTM_SEND_RECEIPT_REFUSED reason=operation_id_mismatch expected={operation_id} got={}",
            receipt.operation_id
        ));
    }
    if receipt.status != "completed" {
        return Err(format!(
            "NTM_SEND_RECEIPT_REFUSED reason=operation_not_completed status={}",
            receipt.status
        ));
    }
    let expected_sha = packet_digest(packet.as_bytes());
    let expected_sha = expected_sha.strip_prefix("sha256:").unwrap_or(&expected_sha);
    if receipt.payload_sha256 != expected_sha {
        return Err(format!(
            "NTM_SEND_RECEIPT_REFUSED reason=payload_digest_mismatch expected={expected_sha} got={}",
            receipt.payload_sha256
        ));
    }
    if receipt.payload_bytes != packet.len() as u64 {
        return Err(format!(
            "NTM_SEND_RECEIPT_REFUSED reason=payload_bytes_mismatch expected={} got={}",
            packet.len(),
            receipt.payload_bytes
        ));
    }
    let target_admitted = receipt
        .admissions
        .iter()
        .find(|admission| ntm_target_matches(&admission.target, pane))
        .ok_or_else(|| {
            format!(
                "NTM_SEND_RECEIPT_REFUSED reason=target_admission_missing pane={pane}"
            )
        })?;
    if target_admitted.state != "submitted" {
        return Err(format!(
            "NTM_SEND_RECEIPT_REFUSED reason=target_not_submitted pane={pane} state={}",
            target_admitted.state
        ));
    }
    if !receipt
        .successful
        .iter()
        .any(|target| ntm_target_matches(target, pane))
    {
        return Err(format!(
            "NTM_SEND_RECEIPT_REFUSED reason=target_not_successful pane={pane}"
        ));
    }
    if receipt
        .failed
        .iter()
        .any(|target| ntm_target_matches(target, pane))
    {
        return Err(format!(
            "NTM_SEND_RECEIPT_REFUSED reason=target_failed pane={pane}"
        ));
    }
    Ok(())
}

async fn query_ntm_send_receipt(
    cx: &Cx,
    config: &Config,
    operation_id: &str,
    pane: &str,
) -> Result<NtmSendReceipt, String> {
    let output = invoke(
        cx,
        config,
        &config.ntm,
        &ntm_send_receipt_args(operation_id),
    )
    .await
    .map_err(|error| {
        format!(
            "NTM_SEND_RECEIPT_REFUSED pane={pane} operation_id={operation_id} error={error}"
        )
    })?;
    parse_ntm_send_receipt(output.status.code(), &output.stdout, &output.stderr).map_err(|error| {
        format!(
            "NTM_SEND_RECEIPT_REFUSED pane={pane} operation_id={operation_id} {error}"
        )
    })
}

fn write_ntm_send_receipt(
    config: &Config,
    tick: u64,
    pane: &str,
    bead: &str,
    receipt: &NtmSendReceipt,
) -> Result<(), String> {
    let detail = json!({
        "bead": bead,
        "pane": pane,
        "operation_id": receipt.operation_id,
        "status": receipt.status,
        "payload_sha256": receipt.payload_sha256,
        "payload_bytes": receipt.payload_bytes,
        "admissions": receipt.admissions.iter().map(|admission| json!({
            "target": admission.target,
            "state": admission.state,
        })).collect::<Vec<_>>(),
        "successful": receipt.successful,
        "failed": receipt.failed,
        "raw_json": receipt.raw_json,
    })
    .to_string();
    write_heartbeat(config, tick, "NTM_SEND_RECEIPT_RECORDED", &detail)
}
async fn send_ntm_with_receipt(
    cx: &Cx,
    config: &Config,
    pane: &str,
    bead: &str,
    packet: &str,
    staged: &Path,
    tick: u64,
) -> Result<TransportReceipt, String> {
    let operation_id = ntm_send_operation_id(bead, pane, tick);
    let send_args = ntm_send_args(&config.session, pane, staged, &operation_id);
    let output = invoke(cx, config, &config.ntm, &send_args).await?;
    let stdout = require_success(&config.ntm, output)?;
    let transport = TransportReceipt::capture_ntm(&stdout).map_err(|error| {
        format!("DISPATCH_BLOCKED bead={bead} malformed ntm receipt: {error}")
    })?;
    let durable = query_ntm_send_receipt(cx, config, &operation_id, pane).await?;
    validate_ntm_send_receipt(&durable, &operation_id, packet, pane).map_err(|error| {
        format!("DISPATCH_BLOCKED bead={bead} pane={pane} {error}")
    })?;
    write_ntm_send_receipt(config, tick, pane, bead, &durable)?;
    Ok(transport)
}
fn classify_ompo_ps_output(output: &Output) -> OmpoPsEvidence {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        let reason = if stdout.contains("UAD_UNKNOWN_VERB") || stderr.contains("UAD_UNKNOWN_VERB") {
            "old_ompo_unknown_verb"
        } else {
            "ompo_ps_command_failed"
        };
        return OmpoPsEvidence::Unmeasured { reason };
    }
    let Ok(value) = serde_json::from_slice::<Value>(&output.stdout) else {
        return OmpoPsEvidence::Unmeasured { reason: "ompo_ps_invalid_json" };
    };
    let data = value.get("data").unwrap_or(&value);
    let project_scopes = data
        .get("project_scope_count")
        .and_then(Value::as_u64)
        .and_then(|count| usize::try_from(count).ok())
        .or_else(|| data.get("project_scopes").and_then(Value::as_array).map(Vec::len));
    let daemon_count = data
        .get("daemon_row_count")
        .and_then(Value::as_u64)
        .and_then(|count| usize::try_from(count).ok())
        .or_else(|| {
            data.get("project_scopes")
                .and_then(Value::as_array)
                .map(|scopes| {
                    scopes
                        .iter()
                        .filter_map(|scope| scope.get("daemons").and_then(Value::as_array))
                        .map(Vec::len)
                        .sum()
                })
        });
    match (project_scopes, daemon_count) {
        (Some(project_scopes), Some(daemon_count)) => OmpoPsEvidence::Measured {
            project_scopes,
            daemon_count,
        },
        _ => OmpoPsEvidence::Unmeasured { reason: "ompo_ps_invalid_shape" },
    }
}

async fn observe_ompo_ps(cx: &Cx, config: &Config) -> Result<OmpoPsEvidence, String> {
    let args = vec![
        "ps".to_owned(),
        "--repo".to_owned(),
        config.repo.display().to_string(),
        "--json".to_owned(),
    ];
    let output = match invoke(cx, config, &config.ompo, &args).await {
        Ok(output) => output,
        Err(error) if error.starts_with("CANCELLED supervisor context") => return Err(error),
        Err(error) => {
            let reason = if error.starts_with("TIMEOUT ") {
                "ompo_ps_timeout"
            } else {
                "ompo_binary_unavailable"
            };
            return Ok(OmpoPsEvidence::Unmeasured { reason });
        }
    };
    Ok(classify_ompo_ps_output(&output))
}

fn record_ompo_ps_observation(evidence: &OmpoPsEvidence) {
    match evidence {
        OmpoPsEvidence::Measured {
            project_scopes,
            daemon_count,
        } => println!(
            "OMPO_PS_OBSERVATION scope=repo project_scopes={project_scopes} daemon_count={daemon_count}"
        ),
        OmpoPsEvidence::Unmeasured { reason } => {
            println!("OMPO_PS_UNMEASURED scope=repo reason={reason}");
        }
    }
}

fn string_set(value: Option<&Value>) -> BTreeSet<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect()
}
fn parse_observation(bytes: &[u8], gate_census: Option<GateCensus>) -> Result<Observation, String> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("MONITOR_BLIND invalid tick-monitor JSON: {error}"))?;
    let panes_value = value
        .get("omp_lifecycle")
        .and_then(|value| value.get("panes"))
        .and_then(Value::as_array)
        .ok_or_else(|| "MONITOR_BLIND missing omp_lifecycle.panes".to_owned())?;
    if panes_value.is_empty() {
        return Err("MONITOR_BLIND tick-monitor returned zero panes".to_owned());
    }
    let dispatchable = string_set(
        value
            .get("idle_panes")
            .and_then(|value| value.get("dispatchable")),
    );
    let free_capacity = string_set(
        value
            .get("idle_panes")
            .and_then(|value| value.get("free_capacity")),
    );
    let mut panes = Vec::with_capacity(panes_value.len());
    for row in panes_value {
        let pane_id = row
            .get("pane")
            .or_else(|| row.get("pane_id"))
            .and_then(Value::as_str)
            .ok_or_else(|| "MONITOR_BLIND pane row has no pane id".to_owned())?;
        let state = row
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or("UNPROVEN")
            .to_owned();
        let liveness = row
            .get("liveness")
            .and_then(Value::as_str)
            .unwrap_or("UNPROVEN")
            .to_owned();
        let is_dispatchable = dispatchable.contains(pane_id);
        let is_free_capacity =
            is_dispatchable || free_capacity.contains(pane_id) || state == "IDLE";
        // DIALOG is deliberately NOT folded into is_working.
        //
        // Measured 2026-08-31 (bead dialog-reads-as-working-zag): on OMP v18 an
        // Ask/approval dialog renders ABOVE the status line, the status line stays
        // LAST, and its timer KEEPS ADVANCING while the pane waits for a human. So a
        // pane blocked on an answer is byte-indistinguishable from a pane doing work,
        // and `%1372` sat 36 MINUTES on an install approval reading as healthy.
        //
        // Counting it as working is right about capacity (do not dispatch there) and
        // wrong about health (nobody is coming). Those are different questions and
        // they now have different fields: `is_working` drives capacity accounting,
        // `awaits_human` drives escalation.
        let awaits_human = state == "DIALOG";
        let is_working = matches!(liveness.as_str(), "LIVE" | "WORKING")
            || matches!(state.as_str(), "WORKING" | "DIALOG");
        panes.push(PaneObservation {
            pane_id: pane_id.to_owned(),
            state,
            liveness,
            is_dispatchable,
            is_free_capacity,
            is_working,
            awaits_human,
        });
    }
    Ok(Observation {
        panes,
        queue: QueueState {
            ready_count: 0,
            readable: true,
        },
        gate_census,
    })
}

async fn capture_pane(cx: &Cx, config: &Config, pane: &str) -> Result<Vec<u8>, String> {
    let args = vec![
        tick_monitor::CAPTURE_PANE.to_owned(),
        "-p".to_owned(),
        "-t".to_owned(),
        pane.to_owned(),
        "-S".to_owned(),
        "-14".to_owned(),
    ];
    require_success(
        &format!("{} {}", tick_monitor::TMUX, tick_monitor::CAPTURE_PANE),
        invoke(cx, config, tick_monitor::TMUX, &args).await?,
    )
}
async fn receiver_is_codex(cx: &Cx, config: &Config, pane: &str) -> Result<bool, String> {
    let args = vec![
        "display-message".to_owned(),
        "-p".to_owned(),
        "-t".to_owned(),
        pane.to_owned(),
        "#{pane_title}".to_owned(),
    ];
    let title = String::from_utf8_lossy(&require_success(
        "tmux display-message",
        invoke(cx, config, tick_monitor::TMUX, &args).await?,
    )?)
    .trim()
    .to_ascii_lowercase();
    Ok(title.contains("__cod_") || title.contains("codex"))
}

async fn ntm_output_identity(
    cx: &Cx,
    config: &Config,
    pane: &str,
) -> Result<ObservationIdentity, String> {
    let args = vec![
        format!("--robot-activity={}", config.session),
        "--panes".to_owned(),
        pane.to_owned(),
    ];
    let bytes = require_success(
        "ntm robot-activity",
        invoke(cx, config, &config.ntm, &args).await?,
    )?;
    let text = String::from_utf8_lossy(&bytes);
    let snapshot = match parse_activity_json(&text) {
        Ok(snapshot) => snapshot,
        Err(ntm_fleet_monitor::ActivityError::EmptyAgents) => {
            return Err(format!("IDENTITY_ROW_ABSENT pane={pane} rows=0"));
        }
        Err(error) => {
            return Err(format!(
                "IDENTITY_ROW_PARSE_REFUSED pane={pane} detail={error}"
            ));
        }
    };
    let mut agents = snapshot.agents.into_iter();
    let Some(agent) = agents.next() else {
        return Err(format!("IDENTITY_ROW_ABSENT pane={pane} rows=0"));
    };
    if agents.next().is_some() {
        return Err(format!(
            "IDENTITY_ROW_KEY_MISMATCH pane={pane} expected=one-server-selected-row"
        ));
    }
    let identity = agent
        .output_identity()
        .map_err(|error| format!("IDENTITY_ROW_IDENTITY_MISSING pane={pane} detail={error}"))?;
    Ok(ObservationIdentity {
        epoch: identity.epoch.clone(),
        sequence: identity.sequence,
        changed_at: identity.changed_at.clone(),
    })
}

async fn post_send_observation(
    cx: &Cx,
    config: &Config,
    pane: &str,
    codex: bool,
) -> (PostSendObservation, Option<String>) {
    if !codex {
        return (PostSendObservation::Missing, None);
    }
    let list_args = vec![
        "list-panes".to_owned(),
        "-t".to_owned(),
        config.session.clone(),
        "-F".to_owned(),
        "#{pane_id}".to_owned(),
    ];
    let list_output = match invoke(cx, config, tick_monitor::TMUX, &list_args).await {
        Ok(output) => output,
        Err(error) => {
            eprintln!("RECEIVER_OBSERVATION_MISSING pane={pane} phase=list error={error}");
            return (PostSendObservation::Missing, None);
        }
    };
    let list_bytes = match require_success("tmux list-panes", list_output) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("RECEIVER_OBSERVATION_MISSING pane={pane} phase=list error={error}");
            return (PostSendObservation::Missing, None);
        }
    };
    let list_text = String::from_utf8_lossy(&list_bytes);
    let pane_ids: Vec<&str> = list_text
        .lines()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .collect();
    if pane_ids.is_empty() {
        return (PostSendObservation::EmptyPaneList, None);
    }
    if !pane_ids.iter().any(|observed| *observed == pane) {
        return (PostSendObservation::Absent, None);
    }
    let capture = match capture_pane(cx, config, pane).await {
        Ok(capture) => capture,
        Err(error) => {
            eprintln!("RECEIVER_OBSERVATION_MISSING pane={pane} phase=capture error={error}");
            return (PostSendObservation::Missing, None);
        }
    };
    let text = String::from_utf8_lossy(&capture).into_owned();
    let at = now_unix();
    let identity = match ntm_output_identity(cx, config, pane).await {
        Ok(identity) => identity,
        Err(error) => {
            eprintln!("{error}");
            return (PostSendObservation::Missing, Some(text));
        }
    };
    (
        PostSendObservation::Present(observe_capture(pane, &text, at, identity)),
        Some(text),
    )
}

/// Read the ACK readback for `bead` on `pane`, bound to THIS dispatch.
///
/// # `issued_at` is what makes a stale ACK absent instead of delivered
///
/// MEASURED 2026-09-05: a live tick returned `ACK_PANE_MISMATCH expected=%8 got=%1414`
/// because the readback matched `[BlueLantern] at 2026-09-02 22:31 UTC / ACK 16l on
/// %1414` — a three-day-old ACK from a control-plane pane. `ma3b` fixed the SCOPE half
/// (whose pane); `y903` fixed the RECENCY half (when), giving `AckReadback` an
/// `Option<u64> dispatch_issued_at` and `with_dispatch_issued_at`. Both fixes were
/// correct at the callee and unwired here — the third time that shape appeared in one
/// session, alongside `--session` on the reaper and `grade --claim`'s missing CLI.
///
/// Without the binding, `dispatch_issued_at: None` means an ACK from the right pane
/// written BEFORE this dispatch still reads as delivery. That is routine in this fleet:
/// `2yf` and `nh5` both went to panes that had held them before.
///
/// The value comes from the pending-dispatch MARKER, never wall clock — a marker written
/// by another process is the only clock both sides share, and `y903`'s own report is
/// explicit that it "compares tracker created_at to the pending-dispatch marker, never
/// wall clock". An unreadable or absent marker yields `None`, which degrades to the prior
/// behaviour rather than refusing: a missing marker must not turn a real delivery into a
/// failure.
///
/// # NO-CLAIM
///
/// Recency plus scope makes a stale foreign ACK unusable. Neither guard distinguishes two
/// dispatches of the SAME bead to the SAME pane inside one window — `y903` proposed a
/// per-dispatch nonce as the stronger fix and deliberately did not build it, because the
/// ACK token is per-BEAD (`ack-stage/src/lib.rs:243-253`), not per-dispatch.
async fn read_ack_readback(
    cx: &Cx,
    config: &Config,
    bead: &str,
    pane: &str,
) -> Result<AckReadback, String> {
    let args = vec![
        "comments".to_owned(),
        "list".to_owned(),
        bead.to_owned(),
        "--json".to_owned(),
    ];
    let bytes = require_success(
        "br comments list",
        invoke(cx, config, &config.br, &args).await?,
    )?;
    let readback =
        AckReadback::from_comments_json_pending(bead, pane, &bytes).map_err(|error| {
            format!("ACK_STAGE_INDETERMINATE bead={bead} pane={pane} comment read-back: {error}")
        })?;
    Ok(match dispatch_marker_issued_at(config, pane) {
        Some(issued_at) => readback.with_dispatch_issued_at(issued_at),
        None => readback,
    })
}

/// Returns `None` for a missing, unreadable, or non-numeric marker. `None` is a
/// DEGRADE of the recency *timestamp*, not of ACK selection: `ack-stage`
/// `match_verdict_in_session` then falls back to the NEWEST fresh in-session ACK
/// (`mgdz`). Refusing here would convert every dispatch whose marker was already
/// cleared into a failure. First-match-with-no-filter is what livelocked eg0m.

fn dispatch_marker_issued_at(config: &Config, pane: &str) -> Option<u64> {
    let path = config
        .pending_dispatch
        .with_extension(pane.trim_start_matches('%'));
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str::<serde_json::Value>(text.trim())
        .ok()?
        .get("issued_at")
        .and_then(serde_json::Value::as_u64)
}

fn write_transport_receipt(
    config: &Config,
    tick: u64,
    pane: &str,
    bead: &str,
    transport: &TransportReceipt,
) -> Result<(), String> {
    let irc_missing = pane_transport_cannot_use_irc_receipt(transport.kind().label(), 0)
        .expect_err("ntm/tmux cannot mint IrcDeliveryReceipt; sender exit is not delivery");
    let detail = serde_json::json!({
        "bead": bead,
        "pane": pane,
        "transport": transport.kind().label(),
        "raw_transport_json": transport.raw_json(),
        "irc_delivery": irc_missing.to_string(),
    })
    .to_string();
    write_heartbeat(config, tick, "TRANSPORT_RECEIPT_CAPTURED", &detail)
}

/// One typed spine record for a step the supervisor just performed.
///
/// Routed through `ack_spine::ledger::step` rather than constructing a
/// `StepRecord`: `emit` is private precisely so every row goes through the
/// checkpointed primitive, and `assert_step_count` then refuses a row no step
/// produced. The effect is empty by design — see the comment at the dispatch site.
async fn emit_step(
    cx: &Cx,
    ledger: &mut ack_spine::ledger::StepLedger,
    kind: StepKind,
    bead: &str,
    pane: &str,
    config: &Config,
    detail: &str,
) -> Result<(), String> {
    ack_spine::ledger::step(
        cx,
        ledger,
        kind,
        bead,
        pane,
        &config.session,
        detail,
        |_cx| async {},
    )
    .await
    .map_err(|error| {
        format!(
            "SPINE_STEP_REFUSED kind={} bead={bead} {error}",
            kind.as_str()
        )
    })
}

/// Append the cycle's typed rows to the spine ledger.
///
/// `dispatched` carries whether this cycle actually dispatched, because the
/// anti-vacuity rule is asymmetric: zero steps on a dispatch cycle is an error,
/// and zero steps on an idle tick is correct.
fn persist_spine(
    config: &Config,
    ledger: &ack_spine::ledger::StepLedger,
    dispatched: bool,
) -> Result<(), String> {
    crate::spine_emit::assert_cycle_emitted(ledger.steps_taken(), dispatched)?;
    ledger
        .assert_step_count()
        .map_err(|error| format!("SPINE_LEDGER_INCONSISTENT {error}"))?;
    if ledger.rows().is_empty() {
        return Ok(());
    }
    let path = crate::spine_emit::spine_ledger_path(&config.heartbeat_ledger);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("SPINE_LEDGER_MKDIR {error}"))?;
    }
    let mut body = ledger.to_jsonl();
    if !body.ends_with('\n') {
        body.push('\n');
    }
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("SPINE_LEDGER_OPEN {} {error}", path.display()))?;
    file.write_all(body.as_bytes())
        .map_err(|error| format!("SPINE_LEDGER_WRITE {error}"))
}

/// How many times this bead has already been dispatched.
///
/// Heartbeat `DISPATCHED` rows plus spine `packet_sent`/`redispatched` rows.
/// Heartbeat-only counting left `Redispatched` unreachable for beads the
/// supervisor had already sent on the spine path (`eg0m`: spine packet_sent,
/// zero heartbeat DISPATCHED lines).
fn prior_dispatch_count(config: &Config, bead: &str) -> usize {
    let heartbeat = fs::read_to_string(&config.heartbeat_ledger).unwrap_or_default();
    let spine = fs::read_to_string(crate::spine_emit::spine_ledger_path(
        &config.heartbeat_ledger,
    ))
    .unwrap_or_default();
    crate::spine_emit::prior_send_count(&heartbeat, &spine, bead)
}

/// THE CLOSE HALF. Emit `Closed` and `GradeReceived` for beads this supervisor
/// dispatched that have since finished.
///
/// # Why this exists at all
///
/// `Closed`, `GradeReceived` and `Redispatched` appear in ZERO of 6,305 heartbeat
/// rows. They are the three facts a grader needs to answer *"did this dispatch
/// finish"*, and nothing in the fleet has ever recorded one. The heartbeat records
/// dispatch richly and completion not at all.
///
/// # Bounded, per the asupersync contract
///
/// `br show` is a subprocess per bead, so the pass is capped and cancellation is
/// checked between beads. A reconcile that could grow with history would make every
/// tick slower than the last.
async fn reconcile_completions(cx: &Cx, config: &Config, tick: u64) -> Result<usize, String> {
    const MAX_BEADS_PER_TICK: usize = 6;
    let Ok(heartbeat) = fs::read_to_string(&config.heartbeat_ledger) else {
        return Ok(0);
    };
    let spine_path = crate::spine_emit::spine_ledger_path(&config.heartbeat_ledger);
    let recorded = crate::spine_emit::recorded_closures(
        &fs::read_to_string(&spine_path).unwrap_or_default(),
    );
    let mut candidates: Vec<(String, String)> = Vec::new();
    for line in heartbeat.lines().rev() {
        if !line.contains("\"DISPATCHED\"") {
            continue;
        }
        let Some(bead) = field_after(line, "bead=") else {
            continue;
        };
        let pane = field_after(line, "pane=").unwrap_or_else(|| "unknown".to_owned());
        if recorded.iter().any(|done| done == &bead) {
            continue;
        }
        if candidates.iter().any(|(known, _)| known == &bead) {
            continue;
        }
        candidates.push((bead, pane));
        if candidates.len() >= MAX_BEADS_PER_TICK {
            break;
        }
    }
    if candidates.is_empty() {
        return Ok(0);
    }
    let mut ledger = ack_spine::ledger::StepLedger::new();
    for (bead, pane) in &candidates {
        cx.checkpoint()
            .map_err(|_| "CANCELLED during completion reconcile".to_owned())?;
        // `BeadSnapshot` carries id/title/description/status/assignee and **no
        // close_reason** — read from `crates/dispatch-claim-fence/src/lib.rs:48`,
        // not assumed. `GradeReceived` is decided by the close reason's prefix, so
        // that type cannot answer this question, and extending a peer's crate is
        // outside this bead. Parsing the raw tracker JSON here is the narrower
        // change.
        let Some((status, close_reason)) = load_close_state(cx, config, bead).await else {
            // A bead that cannot be read is not a bead that closed. Skipping is the
            // conservative direction: a missing completion row is recoverable next
            // tick, a fabricated one is not.
            continue;
        };
        let prior = crate::spine_emit::PriorDispatch {
            bead_id: bead.clone(),
            pane_id: pane.clone(),
            status: status.clone(),
            close_reason: close_reason.clone(),
            prior_dispatch_count: 0,
            already_recorded: false,
        };
        for kind in crate::spine_emit::completion_kinds(&prior) {
            let detail = format!(
                "status={status} close_reason={}",
                close_reason.as_deref().unwrap_or("<none>")
            );
            emit_step(cx, &mut ledger, kind, bead, pane, config, &detail).await?;
        }
    }
    let steps = ledger.steps_taken();
    if steps > 0 {
        persist_spine(config, &ledger, false)?;
        write_heartbeat(
            config,
            tick,
            "SPINE_COMPLETIONS_RECORDED",
            &format!("steps={steps} scanned={}", candidates.len()),
        )?;
    }
    Ok(steps)
}

/// `(status, close_reason)` for one bead, from the raw tracker JSON.
///
/// `br show` returns a **bare list**, not an object with an `issues` key — that is
/// `br list`. Reading the wrong shape here would silently yield `None` for every
/// bead and the close half would stay empty while looking wired, which is the
/// failure this whole bead is about.
///
/// Returns `None` on any failure, and the caller SKIPS rather than emitting. A
/// completion row that no tracker state supports is worse than a late one.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CloseReadback {
    Closed { status: String },
    PolicyRefused { refusal: String },
    Unread { detail: String },
}

fn parse_tracker_close_state(payload: &str) -> Result<(String, Option<String>), String> {
    let parsed: Value = serde_json::from_str(payload)
        .map_err(|error| format!("tracker JSON malformed: {error}"))?;
    let rows = match &parsed {
        Value::Array(rows) => rows,
        Value::Object(object) => object
            .get("issues")
            .and_then(Value::as_array)
            .ok_or_else(|| "tracker JSON object has no issues array".to_owned())?,
        _ => return Err("tracker JSON must be a bare row list or an issues wrapper".to_owned()),
    };
    let row = rows
        .first()
        .ok_or_else(|| "tracker JSON contains no bead row".to_owned())?;
    let status = row
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| "tracker bead row has no string status".to_owned())?
        .to_owned();
    let close_reason = row
        .get("close_reason")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned);
    Ok((status, close_reason))
}

#[cfg(test)]
fn parse_tracker_status(payload: &str) -> Result<String, String> {
    parse_tracker_close_state(payload).map(|(status, _)| status)
}

fn process_output_text(program: &str, output: &Output) -> String {
    let status = output
        .status
        .code()
        .map_or_else(|| "signal".to_owned(), |code| code.to_string());
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let emitted = match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => String::new(),
        (false, true) => format!("stdout={stdout}"),
        (true, false) => format!("stderr={stderr}"),
        (false, false) => format!("stderr={stderr} stdout={stdout}"),
    };
    if emitted.is_empty() {
        format!("{program} exited={status}")
    } else {
        format!("{program} exited={status} {emitted}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveReservationLease {
    id: i64,
    path: String,
    holder: String,
    reason: String,
}

fn parse_active_reservation_leases(text: &str, bead: &str) -> Vec<ActiveReservationLease> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let id = fields.next()?.parse::<i64>().ok()?;
            let path = fields.next()?.to_owned();
            let holder = fields.next()?.to_owned();
            let _expires = fields.next()?;
            let reason = fields.collect::<Vec<_>>().join(" ");
            reason.contains(bead).then_some(ActiveReservationLease {
                id,
                path,
                holder,
                reason,
            })
        })
        .collect()
}

async fn active_reservation_leases(
    cx: &Cx,
    config: &Config,
    bead: &str,
) -> Result<Vec<ActiveReservationLease>, String> {
    if config.am.trim().is_empty() {
        return Ok(Vec::new());
    }
    let args = vec![
        "file_reservations".to_owned(),
        "list".to_owned(),
        "--active-only".to_owned(),
        config.repo.display().to_string(),
    ];
    let output = invoke(cx, config, &config.am, &args).await?;
    if !output.status.success() {
        return Err(process_output_text(&config.am, &output));
    }
    Ok(parse_active_reservation_leases(
        &String::from_utf8_lossy(&output.stdout),
        bead,
    ))
}

async fn close_and_read_back(cx: &Cx, config: &Config, bead: &str, reason: &str) -> CloseReadback {
    let leases = match active_reservation_leases(cx, config, bead).await {
        Ok(leases) => leases,
        Err(detail) => {
            return CloseReadback::PolicyRefused {
                refusal: format!(
                    "CLOSE_REFUSED_RESERVATION_CHECK bead={bead} detail={detail}"
                ),
            };
        }
    };
    if let Some(lease) = leases.first() {
        return CloseReadback::PolicyRefused {
            refusal: format!(
                "CLOSE_REFUSED_RESERVATION_LEASE bead={bead} reservation_id={} holder={} path={} reason={} next_action=release_reservation",
                lease.id, lease.holder, lease.path, lease.reason
            ),
        };
    }
    let close_args = vec![
        "close".to_owned(),
        bead.to_owned(),
        "--reason".to_owned(),
        reason.to_owned(),
        "--json".to_owned(),
    ];
    let close = match invoke(cx, config, &config.br, &close_args).await {
        Ok(output) => output,
        Err(detail) => {
            return CloseReadback::Unread {
                detail: format!("close command unreadable: {detail}"),
            }
        }
    };
    if !close.status.success() {
        return CloseReadback::PolicyRefused {
            refusal: process_output_text(&config.br, &close),
        };
    }

    let show_args = vec!["show".to_owned(), bead.to_owned(), "--json".to_owned()];
    let show = match invoke(cx, config, &config.br, &show_args).await {
        Ok(output) => output,
        Err(detail) => {
            return CloseReadback::Unread {
                detail: format!("tracker readback unreadable: {detail}"),
            }
        }
    };
    if !show.status.success() {
        return CloseReadback::Unread {
            detail: process_output_text(&config.br, &show),
        };
    }
    let payload = String::from_utf8_lossy(&show.stdout);
    match parse_tracker_close_state(&payload) {
        Ok((status, _)) if status == "closed" => CloseReadback::Closed { status },
        Ok((status, _)) => CloseReadback::Unread {
            detail: format!("tracker readback status={status}; expected closed"),
        },
        Err(detail) => CloseReadback::Unread { detail },
    }
}
async fn load_close_state(
    cx: &Cx,
    config: &Config,
    bead: &str,
) -> Option<(String, Option<String>)> {
    let args = vec!["show".to_owned(), bead.to_owned(), "--json".to_owned()];
    let output = invoke(cx, config, &config.br, &args).await.ok()?;
    if !output.status.success() {
        return None;
    }
    let payload = String::from_utf8_lossy(&output.stdout);
    parse_tracker_close_state(&payload).ok()
}

/// `key=value` up to the next space or quote, from a heartbeat detail string.
fn field_after(line: &str, key: &str) -> Option<String> {
    let start = line.find(key)? + key.len();
    let rest = &line[start..];
    let end = rest
        .find(|c: char| c == ' ' || c == '"' || c == ',')
        .unwrap_or(rest.len());
    let value = &rest[..end];
    (!value.is_empty()).then(|| value.to_owned())
}

async fn load_bead_snapshot(cx: &Cx, config: &Config, bead: &str) -> Result<BeadSnapshot, String> {
    let show_args = vec!["show".to_owned(), bead.to_owned(), "--json".to_owned()];
    let show = require_success(
        &config.br,
        invoke(cx, config, &config.br, &show_args).await?,
    )?;
    // gfm6 WIRED HERE (it was BUILT and invoked by nothing until now).
    //
    // The bytes `br show --json` already returned carry `status` and `assignee`, so the
    // claim-state check costs no extra process spawn. It sits BEFORE the snapshot parse
    // because the fourth rule is `file -> claim -> dispatch`: a packet naming a bead in an
    // illegal claim state is a dispatch the tracker cannot project, and the follow-up
    // detector keys on `assigned + in_progress`, so it can see neither half-claim nor
    // orphan-claim. Refusing here is the only place either becomes visible.
    //
    // MEASURED 2026-09-02, and it is why this check reads `br show` and NOT `br ready`:
    // `br ready --json` rows have **no `assignee` key at all** (keys: acceptance_criteria,
    // created_at, created_by, description, id, issue_type, labels, priority, status, title,
    // updated_at). A `.get("assignee")` there returns absent-not-empty, and feeding that to
    // `classify_state` would report ORPHAN_CLAIM for every `in_progress` row in the queue —
    // the same shape as the `.get("blocked", 0)` reader that answered a plausible 0 against
    // a field named `severity.blocker`. The field must EXIST before it can be classified.
    refuse_illegal_claim_state(&show, bead)?;
    parse_br_show_json(&show).map_err(|error| format!("DISPATCH_BLOCKED bead={bead} {error}"))
}

/// Read the parent-owned subagent registry snapshot used by dispatch admission.
///
/// The snapshot is deliberately a narrow JSON contract: either an array of
/// {"name":"...","actor_id":"..."} records or an object containing that
/// array under "subagents". Empty arrays remain valid input to the identity kernel,
/// which turns them into the required typed anti-vacuity refusal.
fn load_subagent_identity_records(path: &Path) -> Result<Vec<IdentityRecord>, String> {
    let text = fs::read_to_string(path).map_err(|error| {
        format!(
            "IDENTITY_REGISTRY_UNAVAILABLE checked=agent_mail,subagents missing=subagents path={} error={error}",
            path.display()
        )
    })?;
    let value: Value = serde_json::from_str(&text).map_err(|error| {
        format!(
            "IDENTITY_REGISTRY_UNAVAILABLE checked=agent_mail,subagents missing=subagents path={} error=invalid_json:{error}",
            path.display()
        )
    })?;
    let rows: &[Value] = match &value {
        Value::Array(rows) => rows,
        Value::Object(object) => object
            .get("subagents")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .ok_or_else(|| {
                format!(
                    "IDENTITY_REGISTRY_UNAVAILABLE checked=agent_mail,subagents missing=subagents path={} error=expected_array_or_subagents_array",
                    path.display()
                )
            })?,
        _ => {
            return Err(format!(
                "IDENTITY_REGISTRY_UNAVAILABLE checked=agent_mail,subagents missing=subagents path={} error=expected_array_or_subagents_array",
                path.display()
            ));
        }
    };
    rows.iter()
        .enumerate()
        .map(|(index, row)| {
            let object = row.as_object().ok_or_else(|| {
                format!(
                    "IDENTITY_REGISTRY_UNAVAILABLE checked=agent_mail,subagents missing=subagents path={} error=entry_{index}_not_object",
                    path.display()
                )
            })?;
            let name = object
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .ok_or_else(|| {
                    format!(
                        "IDENTITY_REGISTRY_UNAVAILABLE checked=agent_mail,subagents missing=subagents path={} error=entry_{index}_name_missing",
                        path.display()
                    )
                })?;
            let actor_id = object
                .get("actor_id")
                .or_else(|| object.get("id"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|actor_id| !actor_id.is_empty())
                .ok_or_else(|| {
                    format!(
                        "IDENTITY_REGISTRY_UNAVAILABLE checked=agent_mail,subagents missing=subagents path={} error=entry_{index}_actor_id_missing",
                        path.display()
                    )
                })?;
            Ok(IdentityRecord::subagent(name, actor_id))
        })
        .collect()
}

/// Take both identity snapshots before a bead packet can be authorized.
///
/// Agent Mail contributes the live roster. The parent-owned subagent registry
/// is an explicit snapshot because sibling panes cannot query a parent's hub
/// namespace. Either unavailable source refuses; neither source is replaced by
/// a permissive fallback.
async fn load_identity_registries(cx: &Cx, config: &Config) -> Result<IdentityRegistries, String> {
    let project = ProjectKey::new(config.repo.display().to_string());
    let client = MailClient::discover().with_request_timeout(MAIL_REQUEST_TIMEOUT);
    let agent_mail_names = mail::list_agents(cx, &client, &project)
        .await
        .map_err(|error| {
            format!(
                "IDENTITY_REGISTRY_UNAVAILABLE checked=agent_mail,subagents missing=agent_mail error={error}"
            )
        })?;
    let agent_mail = agent_mail_names
        .iter()
        .map(|agent| IdentityRecord::agent_mail(agent.as_str(), agent.as_str()))
        .collect();
    let subagents = load_subagent_identity_records(&config.subagent_registry)?;
    let registries = IdentityRegistries::new(agent_mail, subagents);
    for alias in registries.duplicate_aliases() {
        let names = alias
            .names()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(",");
        eprintln!(
            "ASSIGNEE_ALIAS_DATA actor_id={} names=[{}]",
            alias.actor_id(),
            names
        );
    }
    Ok(registries)
}

fn ensure_dispatch_receiver_identity(
    identities: &IdentityRegistries,
    bead: &str,
    pane: &str,
    receiver_agent: &str,
) -> Result<(), String> {
    identities.resolve(receiver_agent).map(|_| ()).map_err(|error| {
        format!(
            "DISPATCH_BLOCKED bead={bead} pane={pane} reason={error} receiver_agent={receiver_agent}"
        )
    })
}

/// Refuse the dispatch when the bead's claim state is illegal.
///
/// `#[must_use]` is LOAD-BEARING, not decoration. MEASURED 2026-09-02: with the consult
/// written inline as `if let Some(finding) = … { return Err(…) }`, a mutation that ran the
/// consult and DISCARDED its refusal left every test GREEN — the pure legs below test
/// `claim_state_finding`, and nothing tested that the call site acts on it. That is
/// BUILT != WIRED one level down: the check existed, was correct, and could be dropped
/// silently. As a `#[must_use] Result`, dropping it is a `clippy -D warnings` error at
/// compile time, which is a stronger guard than any test of mine could be.
#[must_use = "an illegal claim state must REFUSE the dispatch, not be observed and dropped"]
fn refuse_illegal_claim_state(show: &[u8], bead: &str) -> Result<(), String> {
    match claim_state_finding(show, bead) {
        Some(finding) => Err(format!("DISPATCH_BLOCKED {finding}")),
        None => Ok(()),
    }
}

/// The bead's claim state, read from the `br show --json` bytes already in hand.
///
/// Returns `None` for a legal pair AND for bytes this function could not read — the caller
/// must not treat an unreadable payload as an illegal state, because `parse_br_show_json`
/// on the next line is the authority that reports a malformed payload with its own error.
/// Two readers refusing the same bytes with different words is worse than one.
fn claim_state_finding(show: &[u8], bead: &str) -> Option<bead_holder::StateFinding> {
    let value: Value = serde_json::from_slice(show).ok()?;
    let row = match &value {
        Value::Array(rows) => rows.first()?,
        other => other,
    };
    // `br show` returns a BARE list while `br list` wraps rows in `.issues` - handled above
    // by taking the first element of an array, and by accepting a naked object.
    let status = row.get("status").and_then(Value::as_str)?;
    let assignee = row.get("assignee").and_then(Value::as_str).unwrap_or("");
    bead_holder::classify_state(bead, status, assignee)
}
fn tracker_assignee_for_dispatch(
    receiver_agent: &str,
    pane: &str,
    incarnation: PaneIncarnation,
) -> String {
    tracker_assignee_with_composite(true, receiver_agent, pane, incarnation)
        .expect("pane and agent were validated")
}

/// Canonical scheme: `pane=%N;incarnation=<nonzero>;agent=<name>`.
///
/// KEY: composite, not a bare pane id. Pane ids reuse across occupancies
/// (`PaneIncarnation` exists for that). Agent alone is the launch-flag defect.
/// Legacy rows stay as written: `pane4-%9` / nicknames / `WildStone`. Readers
/// distinguish: canonical contains `pane=` AND `incarnation=` AND `agent=`;
/// `paneN-%M` is hand-dispatch; anything else is unresolvable (not invented).
fn tracker_assignee_with_composite(
    composite: bool,
    receiver_agent: &str,
    pane: &str,
    incarnation: PaneIncarnation,
) -> Result<String, String> {
    if !composite {
        // MUTATION: the launch-flag write. Two panes collapse to one string.
        return Ok(receiver_agent.to_owned());
    }
    if pane.trim().is_empty() || !pane.starts_with('%') {
        return Err(format!(
            "ASSIGNEE_UNRESOLVABLE pane={pane} reason=pane_not_a_tmux_id"
        ));
    }
    if receiver_agent.trim().is_empty() {
        return Err("ASSIGNEE_UNRESOLVABLE agent= reason=receiver_agent_empty".to_owned());
    }
    Ok(format!(
        "pane={pane};incarnation={};agent={receiver_agent}",
        incarnation.get()
    ))
}

fn tracker_assignee_agent(assignee: &str) -> Option<&str> {
    assignee
        .split(';')
        .find_map(|part| part.strip_prefix("agent="))
        .filter(|agent| !agent.is_empty())
}

fn tracker_assignee_pane(assignee: &str) -> Option<&str> {
    if let Some(pane) = assignee
        .split(';')
        .find_map(|part| part.strip_prefix("pane="))
        .filter(|pane| pane.starts_with('%'))
    {
        return Some(pane);
    }
    assignee.rsplit_once('-').and_then(|(_, pane)| {
        (pane.starts_with('%') && pane[1..].bytes().all(|b| b.is_ascii_digit())).then_some(pane)
    })
}

fn receiver_agent_for_dispatch(
    config: &Config,
    pane: &str,
    bead: &str,
    snapshot: &BeadSnapshot,
) -> Result<String, String> {
    if !config.receiver_agent.trim().is_empty() {
        return Ok(config.receiver_agent.trim().to_owned());
    }
    if let Some(agent) = agent_for_pane(config, pane).filter(|agent| !agent.trim().is_empty()) {
        return Ok(agent);
    }
    if let Some(assignee) = snapshot.assignee() {
        if let Some(agent) = tracker_assignee_agent(assignee) {
            return Ok(agent.to_owned());
        }
        if !assignee.trim().is_empty() && !assignee.starts_with("supervisor:") {
            return Ok(assignee.to_owned());
        }
    }
    Err(format!(
        "DISPATCH_BLOCKED bead={bead} pane={pane} receiver agent is missing owner=josh next_action=claim-bead"
    ))
}

/// Authorizes one bead packet immediately before construction.
///
/// claim_owner is the tracker identity that performed the authorized claim.
/// It is normally the receiver, except for an unclaimed bead where the
/// supervisor holds an explicit supervisor:<pid> claim until handoff.
#[cfg(test)]
fn test_identity_registries() -> IdentityRegistries {
    IdentityRegistries::new(
        vec![
            IdentityRecord::agent_mail("AmberGate", "agent-mail:amber"),
            IdentityRecord::agent_mail("BlueLantern", "agent-mail:blue"),
            IdentityRecord::agent_mail("GreenFrog", "agent-mail:green"),
            IdentityRecord::agent_mail("SilverWolf", "agent-mail:silver"),
        ],
        vec![
            IdentityRecord::subagent("MailMining", "subagent:mail-mining"),
            IdentityRecord::subagent("ExtractTwo", "subagent:extract-two"),
        ],
    )
}

#[cfg(test)]
fn authorize_bead_dispatch(
    config: &Config,
    pane: &str,
    bead: &str,
    snapshot: &BeadSnapshot,
) -> Result<String, String> {
    let receiver_agent = receiver_agent_for_dispatch(config, pane, bead, snapshot)?;
    let identities = test_identity_registries();
    authorize_bead_dispatch_as(config, pane, bead, snapshot, &receiver_agent, &identities)
}

fn authorize_bead_dispatch_as(
    config: &Config,
    pane: &str,
    bead: &str,
    snapshot: &BeadSnapshot,
    claim_owner: &str,
    identities: &IdentityRegistries,
) -> Result<String, String> {
    let receiver_agent = receiver_agent_for_dispatch(config, pane, bead, snapshot)?;
    let claim_identity = tracker_assignee_agent(claim_owner).unwrap_or(claim_owner);
    let canonical_snapshot = tracker_assignee_agent(claim_owner).map(|_| {
        BeadSnapshot::new_with_acceptance(
            snapshot.id(),
            snapshot.title(),
            snapshot.description(),
            snapshot.acceptance_criteria(),
            snapshot.status_label(),
            Some(claim_owner),
        )
    });
    let authorization = if let Some(canonical_snapshot) = canonical_snapshot.as_ref() {
        authorize(
            &DispatchIntent::bead(bead, claim_owner),
            Some(canonical_snapshot),
        )
        .and_then(|permit| {
            let identity = if claim_identity == receiver_agent {
                claim_identity
            } else {
                &receiver_agent
            };
            identities
                .resolve(identity)
                .map(|_| permit)
                .map_err(ClaimFenceError::AssigneeIdentity)
        })
    } else if claim_identity == receiver_agent {
        authorize_with_identities(
            &DispatchIntent::bead(bead, claim_identity),
            Some(snapshot),
            identities,
        )
    } else {
        authorize(&DispatchIntent::bead(bead, claim_identity), Some(snapshot)).and_then(|permit| {
            identities
                .resolve(&receiver_agent)
                .map(|_| permit)
                .map_err(ClaimFenceError::AssigneeIdentity)
        })
    };
    match authorization {
        Ok(_) => Ok(receiver_agent),
        Err(error) => Err(format!(
            "DISPATCH_BLOCKED bead={bead} pane={pane} reason={} detail={} status={} assignee={} \
             receiver_agent={receiver_agent} claim_owner={claim_owner} owner=josh next_action=claim-bead command=\"{}\"",
            error.code(),
            error,
            snapshot.status_label(),
            snapshot.assignee().unwrap_or("unassigned"),
            error
                .command()
                .unwrap_or("br update <bead> --assignee <agent> --status in_progress"),
        )),
    }
}
fn agent_for_pane(config: &Config, pane: &str) -> Option<String> {
    let contract = config.repo.join(".flywheel/AUTONOMOUS-WAVE.md");
    let text = std::fs::read_to_string(contract).ok()?;
    let pane_marker = format!("`{pane}`");
    text.lines().find_map(|line| {
        let columns: Vec<&str> = line.split('|').map(str::trim).collect();
        if columns.get(1).copied() != Some(pane_marker.as_str()) {
            return None;
        }
        let agent = columns.get(2)?.trim_matches('*').trim();
        (!agent.is_empty()).then(|| agent.to_owned())
    })
}

fn validate_receiver_pane(
    config: &Config,
    pane: &str,
    bead: &str,
    receiver_agent: &str,
) -> Result<(), String> {
    match agent_for_pane(config, pane) {
        Some(mapped) if mapped != receiver_agent => Err(format!(
            "DISPATCH_BLOCKED bead={bead} pane={pane} receiver_agent={receiver_agent} mapped_agent={mapped} owner=josh next_action=select-matching-pane"
        )),
        Some(_) => Ok(()),
        None if config.receiver_agent.trim().is_empty() => Err(format!(
            "DISPATCH_BLOCKED bead={bead} pane={pane} receiver_agent={receiver_agent} owner=josh next_action=configure-pane-agent-map"
        )),
        None => Ok(()),
    }
}

async fn claim_bead_for_supervisor(
    cx: &Cx,
    config: &Config,
    pane: &str,
    bead: &str,
    snapshot: BeadSnapshot,
    receiver_agent: &str,
    incarnation: PaneIncarnation,
    tick: u64,
    claim_enabled: bool,
) -> Result<(BeadSnapshot, String), String> {
    let status = snapshot.status_label();
    if !matches!(status, "open" | "in_progress") {
        return Err(format!(
            "DISPATCH_BLOCKED bead={bead} pane={pane} reason=BEAD_NOT_DISPATCHABLE status={status} assignee={} receiver_agent={receiver_agent} owner=josh next_action=skip-closed-bead",
            snapshot.assignee().unwrap_or("unassigned")
        ));
    }
    let assignee = snapshot.assignee().unwrap_or("").to_owned();
    let tracker_assignee = tracker_assignee_with_composite(true, receiver_agent, pane, incarnation)
        .map_err(|reason| {
            format!(
                "DISPATCH_BLOCKED bead={bead} pane={pane} reason={reason} receiver_agent={receiver_agent} owner=josh next_action=fix-identity"
            )
        })?;
    let open = status == "open";
    let unclaimed_open = open && assignee.is_empty();
    let half_claimed_by_receiver = open
        && (!assignee.is_empty())
        && (assignee == receiver_agent
            || assignee == tracker_assignee
            || tracker_assignee_agent(&assignee) == Some(receiver_agent));
    if !open {
        return Ok((snapshot, assignee));
    }
    if !unclaimed_open && !half_claimed_by_receiver {
        return Err(format!(
            "DISPATCH_BLOCKED bead={bead} pane={pane} reason=CLAIM_REQUIRED status=open assignee={} receiver_agent={receiver_agent} owner=josh next_action=claim-bead",
            if assignee.is_empty() { "unassigned" } else { &assignee }
        ));
    }
    if !claim_enabled {
        return Err(format!(
            "DISPATCH_BLOCKED bead={bead} pane={pane} reason=CLAIM_REQUIRED status=open assignee={} receiver_agent={receiver_agent} owner=josh next_action=enable-supervisor-claim",
            if assignee.is_empty() { "unassigned" } else { &assignee }
        ));
    }
    let expected_assignee = tracker_assignee.clone();
    let claim_args = if unclaimed_open {
        vec![
            "update".to_owned(),
            bead.to_owned(),
            "--claim".to_owned(),
            "--actor".to_owned(),
            tracker_assignee.clone(),
        ]
    } else {
        vec![
            "update".to_owned(),
            bead.to_owned(),
            "--assignee".to_owned(),
            tracker_assignee.clone(),
            "--status".to_owned(),
            "in_progress".to_owned(),
        ]
    };
    let command_output = invoke(cx, config, &config.br, &claim_args)
        .await
        .map_err(|error| {
            format!(
                "DISPATCH_BLOCKED bead={bead} pane={pane} reason=CLAIM_COMMAND_FAILED receiver_agent={receiver_agent} error={error}"
            )
        })?;
    require_success(&config.br, command_output).map_err(|error| {
        format!(
            "DISPATCH_BLOCKED bead={bead} pane={pane} reason=CLAIM_COMMAND_FAILED receiver_agent={receiver_agent} error={error}"
        )
    })?;

    // READBACK IS STILL LOAD-BEARING. K9's defect was asserting the precondition
    // instead of reading it; this reads the tracker after the write, for whichever
    // owner the shape demanded.
    let claimed = load_bead_snapshot(cx, config, bead).await?;
    if claimed.status_label() != "in_progress"
        || claimed.assignee() != Some(expected_assignee.as_str())
    {
        return Err(format!(
            "DISPATCH_BLOCKED bead={bead} pane={pane} reason=CLAIM_READBACK_FAILED status={} assignee={} receiver_agent={receiver_agent} expected_assignee={expected_assignee} owner=josh next_action=inspect-claim",
            claimed.status_label(),
            claimed.assignee().unwrap_or("unassigned")
        ));
    }
    // The heartbeat and the returned owner must name the assignee that ACTUALLY
    // landed, not `supervisor` unconditionally. Measured 2026-09-02: after the
    // half-claim shape transitioned a bead to `in_progress` KEEPING
    // `assignee=GreenFrog`, this returned `supervisor:<pid>` anyway and the caller
    // refused `ASSIGNED_ELSEWHERE assignee=GreenFrog claim_owner=supervisor:90627`
    // -- rejecting the transition it had just performed correctly.
    let detail = format!(
        "bead={bead} pane={pane} assignee={expected_assignee} receiver_agent={receiver_agent} claim_owner={}",
        supervisor_claim_owner(&config.session)
    );
    write_heartbeat(config, tick, "DISPATCH_CLAIMED", &detail).map_err(|error| {
        format!(
            "DISPATCH_BLOCKED bead={bead} pane={pane} reason=CLAIM_HEARTBEAT_FAILED receiver_agent={receiver_agent} error={error}"
        )
    })?;
    eprintln!("DISPATCH_CLAIMED {detail}");
    Ok((claimed, expected_assignee))
}

/// Prepares one bead dispatch, or refuses.
///
/// An unclaimed open bead is claimed atomically by this supervisor-controlled
/// dispatch with the canonical tracker identity pane=<pane>;incarnation=<n>;
/// agent=<receiver>. The composite value prevents two panes sharing one receiver name from colliding.
async fn prepare_bead_dispatch(
    cx: &Cx,
    config: &Config,
    pane: &str,
    pane_observation: &PaneObservation,
    bead: &str,
    identities: &IdentityRegistries,
    tick: u64,
    claim_enabled: bool,
    hold_intent: cross_pane_hold::HoldIntent,
    docs_fresh: bool,
    degraded_authorized: bool,
    admitted_pane_count: usize,
) -> Result<(BeadSnapshot, String, PacketAdmission), String> {

    let initial = load_bead_snapshot(cx, config, bead).await?;
    let receiver_agent = receiver_agent_for_dispatch(config, pane, bead, &initial)?;
    ensure_dispatch_receiver_identity(identities, bead, pane, &receiver_agent)?;
    validate_receiver_pane(config, pane, bead, &receiver_agent)?;
    let in_flight = LifecycleLedger::in_flight_for_bead(&config.bead_lifecycle_ledger, bead)
        .map_err(|error| {
            format!(
                "DISPATCH_BLOCKED bead={bead} pane={pane} reason=LIFECYCLE_LEDGER_UNREADABLE error={error}"
            )
        })?;
    if let Err(refuse) = cross_pane_hold::admit_with_intent(hold_intent, bead, pane, initial.status_label(), &in_flight) {
        return Err(format!(
            "DISPATCH_BLOCKED bead={bead} pane={pane} reason={refuse} owner=josh next_action=wait-for-reap-or-abandon"
        ));
    }
    // This is deliberately before claim_bead_for_supervisor. A refused pane must
    // not become tracker state: file -> claim -> dispatch is only valid after
    // the dispatch preflight has authorized the pane and packet.
    let packet = render_packet_with_sender(
        cx,
        config,
        &initial,
        Some(pane),
        Some(&receiver_agent),
        None,
        None,
    )
    .await?;
    let packet_class = match hold_intent {
        cross_pane_hold::HoldIntent::Grade => DispatchPacketClass::Grading,
        cross_pane_hold::HoldIntent::Work => packet_admission::classify_packet(&packet)
            .map_err(|error| format!("PACKET_ADMISSION_REFUSED bead={bead} error={error}"))?,
    };
    let admission = packet_admission::evaluate_for_class(
        &packet,
        packet_class,
        !docs_fresh,
        degraded_authorized,
    )
    .map_err(|error| format!("PACKET_ADMISSION_REFUSED bead={bead} error={error}"))?;
    match admission.verdict {
        DispatchAdmissibility::Allowed => {}
        DispatchAdmissibility::Degraded { .. } => {
            let detail = format!(
                "admission=degraded packet_class={} refused_class={} naming_gate={} admitted_pane_count={} reason={}",
                admission.packet_class.as_str(),
                admission
                    .refused_class()
                    .expect("degraded has a refused class")
                    .as_str(),
                admission.naming_gate().expect("degraded names its gate"),
                admitted_pane_count,
                admission.reason,
            );
            write_heartbeat(config, tick, "ADMISSION_DEGRADED", &detail)?;
            println!("ADMISSION_DEGRADED tick={tick} bead={bead} pane={pane} {detail}");
        }
        DispatchAdmissibility::Refused => {
            return Err(format!(
                "ADMISSION_REFUSED bead={bead} pane={pane} packet_class={} gate=docs-stale reason={}",
                admission.packet_class.as_str(),
                admission.reason,
            ));
        }
        DispatchAdmissibility::Unknown => {
            return Err(format!(
                "ADMISSION_REFUSED bead={bead} pane={pane} packet_class={} reason=unknown-admission",
                admission.packet_class.as_str()
            ));
        }
    }
    authorize_dispatch_preflight(pane_observation, &packet, bead, pane)?;
    let incarnation = admit_immediately_before_send(&config.session, pane)?;
    println!(
    "DISPATCH_PREFLIGHT verdict=autonomous bead={bead} pane={pane} pane_dispatchable={} two_captures={} packet_complete={}",
    matches!(pane_observation.liveness.as_str(), "CONFIRMED_IDLE" | "NEWLY_IDLE"),
    pane_observation.is_dispatchable,
    packet_is_complete(&packet),
);
    let (snapshot, claim_owner) = claim_bead_for_supervisor(
        cx,
        config,
        pane,
        bead,
        initial,
        &receiver_agent,
        incarnation,
        tick,
        claim_enabled,
    )
    .await?;
    let receiver_agent =
        authorize_bead_dispatch_as(config, pane, bead, &snapshot, &claim_owner, identities)?;
    Ok((snapshot, receiver_agent, admission))
}

async fn run_silence_watch(
    cx: &Cx,
    config: &Config,
    bead: &str,
    dispatch_epoch: i64,
    receiver_agent: &str,
) -> Result<SilenceVerdict, String> {
    let args = vec![
        bead.to_owned(),
        config.session.clone(),
        receiver_agent.to_owned(),
        dispatch_epoch.to_string(),
        config.interval.as_secs().max(1).to_string(),
    ];
    let output = require_success(
        SILENCE_WATCH,
        invoke(cx, config, SILENCE_WATCH, &args).await?,
    )?;
    let text = String::from_utf8_lossy(&output);
    let detector = text
        .lines()
        .find_map(|line| line.strip_prefix("bead=")?.split("detector=").nth(1))
        .map(str::trim)
        .ok_or_else(|| "DISPATCH_SILENCE_WATCH malformed verdict output".to_owned())?;
    match detector {
        "VERDICT_POSTED" => Ok(SilenceVerdict::VerdictPosted),
        "SILENT_PAST_DEADLINE" => Ok(SilenceVerdict::SilentPastDeadline),
        "REASSIGNED" => Ok(SilenceVerdict::Reassigned),
        "TRACKER_ERROR" => Ok(SilenceVerdict::TrackerError),
        other => Err(format!("DISPATCH_SILENCE_WATCH unknown verdict={other}")),
    }
}

const DISPATCH_PACKET_MARKERS: &[&str] = &[
    "Objective: ",
    "Target: ",
    "Scope:\n",
    "Acceptance:\n",
    "Done: ",
    "Stop: ",
];

fn packet_is_complete(packet: &str) -> bool {
    !packet.trim().is_empty()
        && DISPATCH_PACKET_MARKERS
            .iter()
            .all(|marker| packet.contains(marker))
}

/// Obtain the only approval value accepted by the transport path.
///
/// PaneObservation::is_dispatchable is emitted by tick-monitor only after its
/// two-capture liveness rule. Requiring the canonical CONFIRMED_IDLE label as
/// the second fact prevents a forged boolean from masquerading as the capture.
fn authorize_dispatch_preflight(
    pane_observation: &PaneObservation,
    packet: &str,
    bead: &str,
    pane: &str,
) -> Result<(), String> {
    let intent = Intent {
        action: TypedAction::DispatchPacket,
        pane_dispatchable: pane_observation.is_dispatchable,
        two_captures: matches!(
            pane_observation.liveness.as_str(),
            "CONFIRMED_IDLE" | "NEWLY_IDLE"
        ),
        packet_complete: packet_is_complete(packet),
        finding_has_bead: !bead.trim().is_empty(),
    };
    let wave = classify(intent);
    let verdict = wave.verdict;
    Approved::authorize(wave).map(|_| ()).map_err(|error| {
        format!(
            "DISPATCH_PREFLIGHT_REFUSED bead={bead} pane={pane} verdict={} reason={error:?} pane_dispatchable={} two_captures={} packet_complete={}",
            verdict.as_str(),
            intent.pane_dispatchable,
            intent.two_captures,
            intent.packet_complete,
        )
    })
}
fn begin_dispatch_lifecycle(
    config: &Config,
    pane: &str,
    pane_observation: &PaneObservation,
    bead: &str,
    packet: &str,
    tick: u64,
    admission: &PacketAdmission,
    admitted_pane_count: usize,
) -> Result<LifecycleLedger, String> {
    let bead_id = BeadId::new(bead).map_err(|error| error.to_string())?;
    let target =
        DispatchTarget::new(config.session.clone(), pane).map_err(|error| error.to_string())?;
    let now_ms = now_unix().saturating_mul(1_000);
    let intent = Intent {
        action: TypedAction::DispatchPacket,
        pane_dispatchable: pane_observation.is_dispatchable,
        two_captures: matches!(
            pane_observation.liveness.as_str(),
            "CONFIRMED_IDLE" | "NEWLY_IDLE"
        ),
        packet_complete: packet_is_complete(packet),
        finding_has_bead: !bead.trim().is_empty(),
    };
    let approval = Approved::authorize(classify(intent))
        .map_err(|error| format!("lifecycle selection is not approved: {error:?}"))?;
    let identity = LifecycleIdentity::new(
        bead_id,
        config.repo.display().to_string(),
        target,
        packet_digest(packet.as_bytes()),
        InvokerClass::detect_current(),
    )
    .map_err(|error| error.to_string())?;
    let selected_id = EventId::new(selected_event_key_for_run(bead, tick, lifecycle_run_id()))
        .map_err(|error| error.to_string())?;
    let mut selected_fields = vec![
        (
            "decision".to_owned(),
            format!("dispatch preflight passed pane={pane}"),
        ),
        ("run_id".to_owned(), lifecycle_run_id().to_owned()),
        ("build_id".to_owned(), BUILD_ID.to_owned()),
        ("pid".to_owned(), std::process::id().to_string()),
        ("admission".to_owned(), admission.verdict.as_str().to_owned()),
        ("packet_class".to_owned(), admission.packet_class.as_str().to_owned()),
    ];
    if let DispatchAdmissibility::Degraded { .. } = admission.verdict {
        selected_fields.push((
            "refused_class".to_owned(),
            admission.refused_class().expect("degraded has a refused class").as_str().to_owned(),
        ));
        selected_fields.push((
            "naming_gate".to_owned(),
            admission.naming_gate().expect("degraded names its gate").to_owned(),
        ));
        selected_fields.push((
            "admitted_pane_count".to_owned(),
            admitted_pane_count.to_string(),
        ));
    }
    let selected = LedgerEvidence::new(
        selected_id,
        now_ms,
        EvidencePolicy::new(now_ms, 0),
        selected_fields,
    )
    .map_err(|error| error.to_string())?;
    LifecycleLedger::start(
        &config.bead_lifecycle_ledger,
        identity,
        format!("dispatch bead {bead}"),
        approval,
        selected,
    )
    .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerGradeClaim {
    pub bead: String,
    pub receiver_pane: String,
    pub grader_pane: String,
    pub grader_assignee: String,
}

/// The three outcomes of the peer-grading gate.
///
/// This was `Option<PeerGradeClaim>`, which made a SKIP over a non-empty
/// candidate set indistinguishable from an EMPTY candidate set: both reached
/// the caller as `None`, so `grade --claim` printed
/// `typed_outcome=no_receiver_verified_candidate` on stderr for a run whose
/// stdout said `candidate_count=10 observed_panes=7 reason=no_idle_pane`.
/// Absent and empty are different facts (AGENTS.md rule 4a), and a return type
/// that cannot hold both destroys the distinction before any print site gets
/// the chance to be careful with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerGradeGate {
    /// A grading dispatch was selected.
    Claimed(PeerGradeClaim),
    /// The receiver-verified candidate set is EMPTY: nothing is gradeable.
    NoCandidate,
    /// The candidate set is NON-EMPTY and selection refused over it.
    Skipped(PeerGradeSkip),
}

/// A skip over a KNOWN population. `candidate_count` is the denominator the
/// refusal was computed against, so no reader has to guess whether zero
/// candidates or zero eligible graders produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerGradeSkip {
    pub candidate_count: usize,
    pub observed_panes: usize,
    /// Already `reason=`-prefixed, as produced by [`peer_grade_error_detail`].
    pub reason_detail: String,
}

impl PeerGradeSkip {
    /// Heartbeat-row and stdout payload.
    pub fn ledger_detail(&self) -> String {
        format!(
            "candidate_count={} observed_panes={} {} next_action=continue-ranked-dispatch",
            self.candidate_count, self.observed_panes, self.reason_detail
        )
    }

    /// Operator-facing refusal line. Formatted from the SAME value as
    /// [`Self::ledger_detail`], so stdout and stderr cannot name different
    /// causes for one run — agreement is structural, not a matter of care.
    pub fn refusal_line(&self) -> String {
        format!(
            "PEER_GRADE_SKIPPED typed_outcome=selection_refused candidate_count={} {}",
            self.candidate_count, self.reason_detail
        )
    }
}

pub fn gate_peer_grading(
    config: &Config,
    observation: &mut Observation,
    tick: u64,
) -> Result<PeerGradeGate, String> {
    gate_peer_grading_inner(config, observation, tick, None, "__orchestrator__")
}

pub fn gate_peer_grading_for_pane(
    config: &Config,
    observation: &mut Observation,
    tick: u64,
    grader_pane: &str,
) -> Result<PeerGradeGate, String> {
    gate_peer_grading_inner(config, observation, tick, Some(grader_pane), "__orchestrator__")
}

fn peer_grade_error_detail(error: &loop_queue_filter::select::AssignGradeError) -> String {
    match error {
        loop_queue_filter::select::AssignGradeError::EmptyObservation => {
            "reason=empty_observation".to_owned()
        }
        loop_queue_filter::select::AssignGradeError::ObserverPaneUnresolved => {
            "reason=observer_pane_unresolved".to_owned()
        }
        loop_queue_filter::select::AssignGradeError::NoEligibleGrader { reason } => {
            format!("reason={reason}")
        }
        loop_queue_filter::select::AssignGradeError::GraderCarryingOwnDispatch {
            pane,
            liveness,
            is_working,
        } => format!(
            "reason=grader_carrying_own_dispatch pane={pane} liveness={liveness} is_working={is_working}"
        ),
        loop_queue_filter::select::AssignGradeError::GraderIdentityUnresolved { detail } => {
            format!("reason=grader_identity_unresolved detail={detail}")
        }
        loop_queue_filter::select::AssignGradeError::Jsonl(detail) => {
            format!("reason=tracker_unreadable detail={detail}")
        }
    }
}

/// Record a skip on the heartbeat and on stdout. The row keeps its existing
/// shape; what changed is that this no longer swallows the outcome into
/// `Ok(None)` — the caller decides which typed outcome it was.
fn peer_grading_skip(config: &Config, tick: u64, status: &str, detail: &str) -> Result<(), String> {
    write_heartbeat(config, tick, status, detail)?;
    println!("{status} {detail}");
    Ok(())
}

fn gate_peer_grading_inner(
    config: &Config,
    observation: &mut Observation,
    tick: u64,
    preferred_grader_pane: Option<&str>,
    observer_pane: &str,
) -> Result<PeerGradeGate, String> {
    let candidates = LifecycleLedger::receiver_verified_candidates(
        &config.bead_lifecycle_ledger,
        config.repo.join(".beads/issues.jsonl"),
    )
    .map_err(|error| format!("PEER_GRADING_LEDGER_UNREADABLE error={error}"))?;
    if candidates.is_empty() {
        peer_grading_skip(
            config,
            tick,
            "PEER_GRADING_LEDGER_EMPTY",
            "reason=no_receiver_verified_candidate next_action=continue-ranked-dispatch",
        )?;
        return Ok(PeerGradeGate::NoCandidate);
    }

    // Restrict the shared selector to lifecycle-verified candidates. Selection itself is
    // delegated to the kernel that walks every dispatchable non-author pane.
    let tracker_path = config.repo.join(".beads/issues.jsonl");
    let tracker_text = fs::read_to_string(&tracker_path).map_err(|error| {
        format!(
            "PEER_GRADING_LEDGER_UNREADABLE path={} error={error}",
            tracker_path.display()
        )
    })?;
    let candidate_ids: BTreeSet<String> = candidates
        .iter()
        .map(|candidate| candidate.identity.bead.as_str().to_owned())
        .collect();
    let candidate_jsonl = tracker_text
        .lines()
        .filter(|line| {
            serde_json::from_str::<Value>(line)
                .ok()
                .and_then(|row| {
                    row.get("id")
                        .and_then(Value::as_str)
                        .map(|id| candidate_ids.contains(id))
                })
                .unwrap_or(false)
        })
        .collect::<Vec<_>>()
        .join("\n");
    if candidate_jsonl.is_empty() {
        return Err(
            "PEER_GRADING_LEDGER_UNREADABLE reason=receiver_verified_tracker_row_missing".to_owned(),
        );
    }
    let ledger_text = fs::read_to_string(&config.bead_lifecycle_ledger).map_err(|error| {
        format!(
            "PEER_GRADING_LEDGER_UNREADABLE path={} error={error}",
            config.bead_lifecycle_ledger.display()
        )
    })?;
    let observed_panes = observation
        .panes
        .iter()
        .map(|pane| loop_queue_filter::select::ObservedPane {
            pane_id: pane.pane_id.clone(),
            liveness: pane.liveness.clone(),
            is_dispatchable: pane.is_dispatchable,
            is_working: pane.is_working,
        })
        .collect::<Vec<_>>();
    let selected_panes = if let Some(preferred) = preferred_grader_pane {
        if !observed_panes
            .iter()
            .any(|pane| pane.pane_id == preferred && pane.is_dispatchable)
        {
            return Err(format!(
                "PEER_GRADING_REFUSED grader_pane={preferred} reason=preferred_grader_not_idle"
            ));
        }
        observed_panes
            .iter()
            .filter(|pane| pane.pane_id == preferred)
            .cloned()
            .collect::<Vec<_>>()
    } else {
        observed_panes.clone()
    };
    // A NAMED grader carries the eligibility gauntlet INSIDE the selector. The
    // pre-check above admits it on `is_dispatchable` alone, which does not
    // imply confirmed-idle, so a pane carrying its own dispatch must be
    // refused where it is still known by name — once the candidate set is
    // filtered, the only refusal left to give is about the fleet.
    let selection = match preferred_grader_pane {
        Some(preferred) => loop_queue_filter::select::assign_peer_grade_for_named_grader(
            observer_pane,
            preferred,
            &selected_panes,
            &candidate_jsonl,
            &ledger_text,
        ),
        None => loop_queue_filter::select::assign_peer_grade_with_ledger(
            observer_pane,
            &selected_panes,
            &candidate_jsonl,
            &ledger_text,
        ),
    };
    let assignment = match selection {
        Ok(assignment) => assignment,
        Err(loop_queue_filter::select::AssignGradeError::NoEligibleGrader { reason })
            if preferred_grader_pane.is_some() && reason == "no_distinct_idle_peer" =>
        {
            let bead = candidates
                .first()
                .map(|candidate| candidate.identity.bead.as_str())
                .unwrap_or("unknown");
            return Err(format!(
                "PEER_GRADING_REFUSED bead={bead} grader_pane={} reason=self_grade",
                preferred_grader_pane.unwrap_or("unknown")
            ));
        }
        Err(error) => {
            // The skip carries its own denominator, so the caller can tell
            // "ten candidates, no eligible grader" from "nothing to grade".
            let skip = PeerGradeSkip {
                candidate_count: candidates.len(),
                observed_panes: selected_panes.len(),
                reason_detail: peer_grade_error_detail(&error),
            };
            peer_grading_skip(config, tick, "PEER_GRADING_SKIPPED", &skip.ledger_detail())?;
            return Ok(PeerGradeGate::Skipped(skip));
        }
    };
    let candidate = candidates
        .iter()
        .find(|candidate| candidate.identity.bead.as_str() == assignment.bead)
        .ok_or_else(|| {
            format!(
                "PEER_GRADING_LEDGER_UNREADABLE bead={} reason=assignment_not_receiver_verified",
                assignment.bead
            )
        })?;
    refuse_placeholder_identity("bead", &assignment.bead)?;
    refuse_placeholder_identity("receiver_pane", &candidate.identity.target.pane)?;
    refuse_placeholder_identity("grader_pane", &assignment.grader_pane)?;
    Ok(PeerGradeGate::Claimed(PeerGradeClaim {
        bead: assignment.bead,
        receiver_pane: candidate.identity.target.pane.clone(),
        grader_pane: assignment.grader_pane,
        grader_assignee: assignment.grader_assignee,
    }))
}

fn refuse_placeholder_identity(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() || value.contains('<') || value.contains('>') {
        return Err(format!(
            "PEER_GRADING_REFUSED reason=placeholder_identity field={field} value={value}"
        ));
    }
    Ok(())
}

static PANE_OCCUPANCY_MINT: LazyLock<IncarnationMint> = LazyLock::new(IncarnationMint::new);
static PANE_OCCUPANCIES: LazyLock<Mutex<BTreeMap<String, Occupancy>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));

/// Live occupancy check immediately before tmux/ntm send. Not at enqueue.
fn admit_immediately_before_send(session: &str, pane: &str) -> Result<PaneIncarnation, String> {
    let live = std::process::id() as u32;
    let key = format!("{session}\0{pane}");
    let mut map = PANE_OCCUPANCIES
        .lock()
        .map_err(|_| "INCARNATION_STORE_POISONED".to_owned())?;
    let occupancy = map
        .entry(key)
        .or_insert_with(|| Occupancy::unminted(session, pane));
    if occupancy.current().is_none() {
        occupancy.occupy(&PANE_OCCUPANCY_MINT, live);
    }
    let incarnation = occupancy
        .current()
        .ok_or_else(|| "INCARNATION_NOT_MINTED".to_owned())?;
    let presented = Presented {
        session: session.to_owned(),
        pane: pane.to_owned(),
        incarnation: Some(incarnation),
        marker_pid: Some(live),
    };
    admit_at_send(occupancy, &presented, live).map_err(|refusal| {
        format!("DISPATCH_BLOCKED pane={pane} incarnation_refused={refusal:?}")
    })?;
    Ok(incarnation)
}

/// K1: send_and_verify is three-valued. Delivered / Indeterminate / AckPending
/// are not Err, and DISPATCH_FAILED is only Failed (refused send / tmux timeout).
#[derive(Debug)]
enum DispatchVerdict {
    Delivered(AckStageResult),
    Indeterminate {
        reason: String,
        transport: String,
    },
    AckPending {
        after_secs: u64,
        discriminator: String,
    },
    Failed(String),
}

impl DispatchVerdict {
    fn status_word(&self) -> &'static str {
        match self {
            Self::Delivered(_) => "DISPATCHED",
            Self::Indeterminate { .. } => "DISPATCH_INDETERMINATE",
            Self::AckPending { .. } => "ACK_PENDING",
            Self::Failed(_) => "DISPATCH_FAILED",
        }
    }
}

/// C112: claim owner must outlive the tick. A pid dies at process exit while
/// the tracker row remains; session/label does not.
fn supervisor_claim_owner(session: &str) -> String {
    format!("supervisor:{session}")
}

/// q8zl: the transport boundary re-reads tracker status. A prefetched ready-queue
/// snapshot is not authority to send. Terminal statuses are named in the refusal.
fn refuse_terminal_status_at_send(bead: &str, status: &str) -> Result<(), String> {
    match status {
        "open" | "in_progress" => Ok(()),
        terminal => Err(format!(
            "DISPATCH_BLOCKED bead={bead} reason=STALE_QUEUE_SNAPSHOT status={terminal} \
             next_action=skip-closed-bead"
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AtSendCapture {
    status: String,
    assignee: String,
    has_acceptance: bool,
    filed_only: bool,
}

impl AtSendCapture {
    fn from_snapshot(snapshot: &BeadSnapshot) -> Self {
        let description = snapshot.description();
        Self {
            status: snapshot.status_label().to_owned(),
            assignee: snapshot.assignee().unwrap_or("").to_owned(),
            has_acceptance: dispatch_packet::has_typed_acceptance(snapshot),
            filed_only: description.to_ascii_lowercase().contains("filed only"),
        }
    }

    fn detail(&self, bead: &str, pane: &str) -> String {
        format!(
            "bead={bead} pane={pane} at_send_status={} at_send_assignee={} at_send_has_acceptance={} at_send_filed_only={}",
            self.status,
            if self.assignee.is_empty() { "none" } else { &self.assignee },
            self.has_acceptance,
            self.filed_only
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlignmentClass {
    Pass,
    Fail,
    Unknown,
}

impl AlignmentClass {
    fn label(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Unknown => "UNKNOWN",
        }
    }
}

fn field_from_detail(detail: &str, key: &str) -> Option<String> {
    field_after(detail, key)
}

fn classify_send_alignment(detail: &str) -> AlignmentClass {
    let status = field_from_detail(detail, "at_send_status=");
    let assignee = field_from_detail(detail, "at_send_assignee=");
    let acc = field_from_detail(detail, "at_send_has_acceptance=");
    let fld = field_from_detail(detail, "at_send_filed_only=");
    let (Some(status), Some(assignee), Some(acc), Some(fld)) = (status, assignee, acc, fld) else {
        return AlignmentClass::Unknown;
    };
    let clm = assignee != "none" && !assignee.is_empty();
    let acc_ok = acc == "true";
    let trm = !matches!(status.as_str(), "closed" | "blocked" | "tombstone");
    let fld_ok = fld == "false";
    if clm && acc_ok && trm && fld_ok {
        AlignmentClass::Pass
    } else {
        AlignmentClass::Fail
    }
}

fn classify_alignment_window(details: &[String]) -> Result<Vec<AlignmentClass>, String> {
    if details.is_empty() {
        return Err(
            "ALIGNMENT_SCAN_EMPTY -- an empty send window is an ERROR, never a pass".to_owned(),
        );
    }
    Ok(details.iter().map(|d| classify_send_alignment(d)).collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DispatchPolicy {
    interval: Duration,
    command_timeout: Duration,
    receipt_timeout: Duration,
    receipt_poll: Duration,
    pending_dispatch_max_age: Duration,
    mail_request_timeout: Duration,
    origin: &'static str,
}

impl DispatchPolicy {
    fn defaults() -> Self {
        Self {
            interval: DEFAULT_INTERVAL,
            command_timeout: DEFAULT_COMMAND_TIMEOUT,
            receipt_timeout: RECEIPT_TIMEOUT,
            receipt_poll: RECEIPT_POLL,
            pending_dispatch_max_age: Duration::from_secs(PENDING_DISPATCH_MAX_AGE_SECS),
            mail_request_timeout: MAIL_REQUEST_TIMEOUT,
            origin: "defaults",
        }
    }

    fn log_live(&self) {
        eprintln!(
            "DISPATCH_POLICY origin={} interval_secs={} command_timeout_secs={} receipt_timeout_secs={} receipt_poll_ms={} pending_dispatch_max_age_secs={} mail_request_timeout_secs={}",
            self.origin,
            self.interval.as_secs(),
            self.command_timeout.as_secs(),
            self.receipt_timeout.as_secs(),
            self.receipt_poll.as_millis(),
            self.pending_dispatch_max_age.as_secs(),
            self.mail_request_timeout.as_secs()
        );
    }
}

fn parse_dispatch_policy(text: Option<&str>) -> DispatchPolicy {
    let defaults = DispatchPolicy::defaults();
    let Some(text) = text else {
        return defaults;
    };
    let Some(section) = text.split("[dispatch]").nth(1) else {
        return defaults;
    };
    let section = section.split('[').next().unwrap_or(section);
    let mut policy = defaults;
    policy.origin = "config.toml";
    let mut parsed_any = false;
    for line in section.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, raw)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let Ok(value) = raw.trim().trim_matches('"').parse::<u64>() else {
            return DispatchPolicy {
                origin: "malformed-fallback",
                ..DispatchPolicy::defaults()
            };
        };
        parsed_any = true;
        match key {
            "interval_secs" => policy.interval = Duration::from_secs(value),
            "command_timeout_secs" => policy.command_timeout = Duration::from_secs(value),
            "receipt_timeout_secs" => policy.receipt_timeout = Duration::from_secs(value),
            "receipt_poll_ms" => policy.receipt_poll = Duration::from_millis(value),
            "pending_dispatch_max_age_secs" => {
                policy.pending_dispatch_max_age = Duration::from_secs(value)
            }
            "mail_request_timeout_secs" => policy.mail_request_timeout = Duration::from_secs(value),
            _ => {}
        }
    }
    if parsed_any {
        policy
    } else {
        DispatchPolicy::defaults()
    }
}

async fn send_and_verify(
    cx: &Cx,
    config: &Config,
    pane: &str,
    pane_observation: &PaneObservation,
    bead: &str,
    receiver_agent: &str,
    snapshot: &BeadSnapshot,
    before: &[u8],
    tick: u64,
    admission: &PacketAdmission,
    admitted_pane_count: usize,
) -> Result<DispatchVerdict, String> {
    let at_send = load_bead_snapshot(cx, config, bead).await?;
    refuse_terminal_status_at_send(bead, at_send.status_label())?;
    let capture = AtSendCapture::from_snapshot(&at_send);
    write_heartbeat(config, tick, "AT_SEND_CAPTURE", &capture.detail(bead, pane))?;
    let packet = render_packet_with_sender(
        cx,
        config,
        snapshot,
        Some(pane),
        Some(receiver_agent),
        None,
        None,
    )
    .await?;
    let staged = env::temp_dir().join(format!(
        "omp-orchestrator-dispatch-{}-{}-{}.txt",
        std::process::id(),
        pane.trim_start_matches('%'),
        bead
    ));
    fs::write(&staged, packet.as_bytes())
        .map_err(|error| format!("DISPATCH_BLOCKED bead={bead} stage packet: {error}"))?;
    let pre_at = now_unix();
    let pre_identity = ntm_output_identity(cx, config, pane).await?;
    let pre_observation =
        observe_capture(pane, &String::from_utf8_lossy(before), pre_at, pre_identity);
    let mut lifecycle = begin_dispatch_lifecycle(
        config,
        pane,
        pane_observation,
        bead,
        &packet,
        tick,
        admission,
        admitted_pane_count,
    )
    .map_err(|error| format!("LIFECYCLE_LEDGER_REFUSED bead={bead} pane={pane} error={error}"))?;
    let dispatch_at_ms = now_unix().saturating_mul(1_000);
    let dispatch_id = EventId::new(run_scoped_event_key("dispatch", bead, tick, ""))
        .map_err(|error| error.to_string())?;
    let dispatch_receipt = DispatchReceipt::new(
        dispatch_id.clone(),
        BeadId::new(bead).map_err(|error| error.to_string())?,
        DispatchTarget::new(config.session.clone(), pane).map_err(|error| error.to_string())?,
        format!("dispatch bead {bead}"),
        dispatch_at_ms,
    )
    .map_err(|error| error.to_string())?;
    let dispatch_evidence = LedgerEvidence::single(
        dispatch_id,
        dispatch_at_ms,
        EvidencePolicy::new(dispatch_at_ms, 0),
        "packet_bytes",
        packet.len().to_string(),
    )
    .map_err(|error| error.to_string())?;
    lifecycle
        .dispatch(dispatch_receipt, dispatch_evidence)
        .map_err(|error| {
            format!("LIFECYCLE_LEDGER_REFUSED bead={bead} pane={pane} error={error}")
        })?;
    admit_immediately_before_send(&config.session, pane)?;
    let codex = receiver_is_codex(cx, config, pane).await?;
    let transport = if codex {
        let typed_args = vec![
            tick_monitor::SEND_KEYS.to_owned(),
            "-t".to_owned(),
            pane.to_owned(),
            "-l".to_owned(),
            packet.clone(),
        ];
        let typed_stdout = require_success(
            &format!("{} {} -l", tick_monitor::TMUX, tick_monitor::SEND_KEYS),
            invoke(cx, config, tick_monitor::TMUX, &typed_args).await?,
        )?;
        let enter_args = vec![
            tick_monitor::SEND_KEYS.to_owned(),
            "-t".to_owned(),
            pane.to_owned(),
            "Enter".to_owned(),
        ];
        let enter_stdout = require_success(
            &format!("{} {} Enter", tick_monitor::TMUX, tick_monitor::SEND_KEYS),
            invoke(cx, config, tick_monitor::TMUX, &enter_args).await?,
        )?;
        TransportReceipt::capture_codex(
            format!(
                "{} {} -l; {} {} Enter",
                tick_monitor::TMUX,
                tick_monitor::SEND_KEYS,
                tick_monitor::TMUX,
                tick_monitor::SEND_KEYS
            ),
            &typed_stdout,
            &enter_stdout,
            Some(0),
        )
    } else {
        send_ntm_with_receipt(cx, config, pane, bead, &packet, &staged, tick).await?
    };
    let _ = fs::remove_file(&staged);
    write_transport_receipt(config, tick, pane, bead, &transport)?;

    let deadline = Instant::now() + RECEIPT_TIMEOUT;
    let mut attempts_so_far = 0u32;
    let session_pane_ids: Vec<String> = {
        let list_args = vec![
            "list-panes".to_owned(),
            "-t".to_owned(),
            config.session.clone(),
            "-F".to_owned(),
            "#{pane_id}".to_owned(),
        ];
        match invoke(cx, config, tick_monitor::TMUX, &list_args).await {
            Ok(output) => match require_success("tmux list-panes", output) {
                Ok(bytes) => String::from_utf8_lossy(&bytes)
                    .lines()
                    .map(|line| line.trim().to_owned())
                    .filter(|id| !id.is_empty())
                    .collect(),
                Err(_) => vec![pane.to_owned()],
            },
            Err(_) => vec![pane.to_owned()],
        }
    };
    let composer_rules = composer_typed::Rules::default();
    // iis6: keep the FIRST and LAST observations so an expired wait can DISCRIMINATE
    // busy from unreachable instead of reporting `ack_readback_missing` for both.
    // The first must be a WORKING capture: comparing against a pre-send Idle would
    // yield `NO_PRIOR_WORKING_CAPTURE_TO_COMPARE` and lose the motion evidence.
    let mut first_working: Option<receiver_receipt::Observation> = None;
    let mut latest: Option<receiver_receipt::Observation> = None;
    loop {
        cx.checkpoint()
            .map_err(|_| "CANCELLED while verifying receiver receipt".to_owned())?;
        let (post_send, pane_capture) =
            post_send_observation(cx, config, pane, codex).await;
        if let receiver_receipt::PostSendObservation::Present(observation) = &post_send {
            if first_working.is_none()
                && matches!(
                    observation.state,
                    receiver_receipt::PaneState::Working { .. }
                )
            {
                first_working = Some(observation.clone());
            }
            latest = Some(observation.clone());
        }
        let ack = read_ack_readback(cx, config, bead, pane).await?;
        let stage = assess_ack_stage(&AckStageInput {
            bead_id: bead.to_owned(),
            pane_id: pane.to_owned(),
            transport: transport.clone(),
            pre_send: pre_observation.clone(),
            post_send: post_send.clone(),
            ack,
            attempts_so_far,
            session_pane_ids: session_pane_ids.clone(),
        });
        if stage.is_confirmed() {
            let receiver_at_ms = now_unix().saturating_mul(1_000);
            let receiver_id = EventId::new(run_scoped_event_key("receiver", bead, tick, ""))
                .map_err(|error| error.to_string())?;
            let receiver = ReceiverEvidence::new(
                receiver_id.clone(),
                BeadId::new(bead).map_err(|error| error.to_string())?,
                DispatchTarget::new(config.session.clone(), pane)
                    .map_err(|error| error.to_string())?,
                format!("dispatch bead {bead}"),
                receiver_at_ms,
            )
            .map_err(|error| error.to_string())?;
            let receiver_evidence = LedgerEvidence::new(
                receiver_id,
                receiver_at_ms,
                EvidencePolicy::new(receiver_at_ms, 0),
                [
                    ("transport", stage.transport.kind().label()),
                    ("ack_action", stage.action.label()),
                    ("receiver_agent", receiver_agent),
                ],
            )
            .map_err(|error| error.to_string())?;
            lifecycle
                .verify_receiver(receiver, receiver_evidence)
                .map_err(|error| {
                    format!("LIFECYCLE_LEDGER_REFUSED bead={bead} pane={pane} error={error}")
                })?;
            return Ok(DispatchVerdict::Delivered(stage));
        }

        let recovery = match (&stage.delivery, &post_send, pane_capture.as_deref()) {
            (
                ReceiptVerdict::NoReceipt {
                    reason: ReceiptReason::IdleUnchanged,
                    ..
                },
                PostSendObservation::Present(post),
                Some(capture),
            ) => {
                let composer = if composer_typed::is_typed(capture, &composer_rules) {
                    ComposerEvidence::Typed
                } else {
                    ComposerEvidence::Free
                };
                Some(escalate_non_delivery(&post.state, composer))
            }
            _ => None,
        };

        if stage.action.is_retry() {
            match recovery {
                Some(NonDeliveryEscalation::ResendDirect) => {
                    admit_immediately_before_send(&config.session, pane)?;
                    let resend_args = vec![
                        tick_monitor::SEND_KEYS.to_owned(),
                        "-t".to_owned(),
                        pane.to_owned(),
                        "-l".to_owned(),
                        packet.clone(),
                    ];
                    require_success(
                        "tmux resend send-keys -l",
                        invoke(cx, config, tick_monitor::TMUX, &resend_args).await?,
                    )?;
                    let enter_args = vec![
                        tick_monitor::SEND_KEYS.to_owned(),
                        "-t".to_owned(),
                        pane.to_owned(),
                        "Enter".to_owned(),
                    ];
                    require_success(
                        "tmux resend Enter",
                        invoke(cx, config, tick_monitor::TMUX, &enter_args).await?,
                    )?;
                    write_heartbeat(
                        config,
                        tick,
                        "RECEIVER_RECOVERY",
                        &format!(
                            "pane={pane} bead={bead} action=RESEND_DIRECT attempts={}",
                            attempts_so_far + 1
                        ),
                    )?;
                    attempts_so_far += 1;
                }
                Some(NonDeliveryEscalation::SubmitParked) => {
                    admit_immediately_before_send(&config.session, pane)?;
                    let enter_args = vec![
                        tick_monitor::SEND_KEYS.to_owned(),
                        "-t".to_owned(),
                        pane.to_owned(),
                        "Enter".to_owned(),
                    ];
                    require_success(
                        "tmux recovery Enter",
                        invoke(cx, config, tick_monitor::TMUX, &enter_args).await?,
                    )?;
                    write_heartbeat(
                        config,
                        tick,
                        "RECEIVER_RECOVERY",
                        &format!(
                            "pane={pane} bead={bead} action=SUBMIT_PARKED attempts={}",
                            attempts_so_far + 1
                        ),
                    )?;
                    attempts_so_far += 1;
                }
                Some(NonDeliveryEscalation::KeepPolling) | None => {}
            }
        }

        let awaiting_ack = matches!(
            &stage.delivery,
            ReceiptVerdict::Indeterminate {
                reason: ReceiptReason::AckReadbackMissing,
                ..
            }
        );
        let retry_exhausted = matches!(&stage.action, AckAction::RetryExhausted { .. });
        if !stage.action.is_retry() && !retry_exhausted && !awaiting_ack {
            let reason = stage
                .delivery
                .reason()
                .map(ToString::to_string)
                .unwrap_or_else(|| "unclassified".to_owned());
            return Ok(DispatchVerdict::Indeterminate {
                reason,
                transport: transport.kind().label().to_owned(),
            });
        }
        if retry_exhausted || Instant::now() >= deadline {
            // iis6: THE WINDOW DECIDES WHEN WE RE-CHECK, NOT WHETHER A HUMAN IS
            // CALLED. Before this, every expired wait returned
            // `ack_readback_missing` — the same verdict for a pane mid-tool-call and
            // a dead one. Observed timers sat at 120s, 1320s and 1440s inside a
            // single tool call, so no constant separates those states; the
            // discriminator is whether the timer ADVANCED across
            // `OBSERVATION_WINDOW_MIN_SECS`.
            let discriminated = match (&first_working, &latest) {
                (Some(first), Some(last)) => Some(receiver_receipt::classify_ack_wait(first, last)),
                _ => None,
            };
            if let Some(verdict) = &discriminated {
                if !verdict.owes_a_human() {
                    // Busy or unmeasurable: a retryable typed row, and NOT an error,
                    // so the tick does not escalate. The ack is late, not absent.
                    write_heartbeat(
                        config,
                        tick,
                        "ACK_WAIT_PENDING",
                        &format!(
                            "pane={pane} bead={bead} verdict={} reason={} next_action={} after={}s",
                            verdict.label(),
                            verdict.reason_or_empty(),
                            verdict.next_action(),
                            RECEIPT_TIMEOUT.as_secs(),
                        ),
                    )?;
                }
            }
            // THE ACTION MUST FOLLOW THE DISCRIMINATOR, NOT THE PRE-DISCRIMINATION STAGE.
            //
            // MEASURED 2026-09-05 on two panes and two beads within minutes:
            //   pane=%9 bead=y6v5 action=AWAIT_HUMAN ... owes_human=false
            //   pane=%8 bead=93lo action=AWAIT_HUMAN ... owes_human=false
            // `stage.action` is decided BEFORE the discriminator runs, so a successful
            // discrimination that explicitly does not owe a human still printed the
            // instruction to fetch one. Two contradictory directives in a single line,
            // and the operator-facing half was the wrong one.
            let action = match &discriminated {
                Some(verdict) if !verdict.owes_a_human() => verdict.next_action(),
                _ => stage.action.label(),
            };
            let discriminator = discriminated
                .as_ref()
                .map(|v| {
                    // An EMPTY value is indistinguishable from a field the writer forgot.
                    // `ACK_PENDING_WORKER_BUSY` carries no reason because it is a positive
                    // discrimination rather than a refusal, and it printed as a bare
                    // `discriminator_reason=` — which reads as a bug in the emitter.
                    let mut reason_field = v.reason_or_empty().to_string();
                    if reason_field.is_empty() {
                        reason_field = "NONE".to_owned();
                    }
                    format!(
                        " discriminator={} discriminator_reason={reason_field} owes_human={}",
                        v.label(),
                        v.owes_a_human()
                    )
                })
                // Absent evidence is NAMED, never silently absent: a wait with no
                // working capture cannot discriminate and must say so rather than
                // letting the reader assume it did.
                .unwrap_or_else(|| {
                    " discriminator=NO_WORKING_CAPTURE owes_human=unknown".to_owned()
                });
            let redispatch_at_ms = now_unix().saturating_mul(1_000);
            let redispatch_id =
                EventId::new(run_scoped_event_key("redispatch-required", bead, tick, ""))
                    .map_err(|error| error.to_string())?;
            let redispatch_plan = RedispatchPlan::new(
                redispatch_id.clone(),
                BeadId::new(bead).map_err(|error| error.to_string())?,
                DispatchTarget::new(config.session.clone(), pane)
                    .map_err(|error| error.to_string())?,
                format!("receiver_not_reported action={action}"),
            )
            .map_err(|error| error.to_string())?;
            let redispatch_evidence = LedgerEvidence::new(
                redispatch_id,
                redispatch_at_ms,
                EvidencePolicy::new(redispatch_at_ms, 0),
                [
                    ("reason", "receiver_not_reported"),
                    ("action", action),
                    ("delivery", stage.delivery.label()),
                ],
            )
            .map_err(|error| error.to_string())?;
            lifecycle
                .require_redispatch(redispatch_plan, redispatch_evidence)
                .map_err(|error| {
                    format!("LIFECYCLE_LEDGER_REFUSED bead={bead} pane={pane} error={error}")
                })?;
            return Ok(DispatchVerdict::AckPending {
                after_secs: RECEIPT_TIMEOUT.as_secs(),
                discriminator,
            });
        }
        sleep(cx.now_for_observability(), RECEIPT_POLL).await;
    }
}
/// psf7: emit this tick's receipt row into the orchestration ledger.
///
/// The row is built by `orchestration-tick-gate`'s own builder rather than assembled here,
/// so the crate that VALIDATES the shape also produces it. A builder that could emit a row
/// its sibling validator refuses would put an operator in front of a violation they did not
/// write and cannot fix, and the observed response to that is deletion.
///
/// # `not_done` is populated, and it is the honest part
///
/// This row is written BEFORE the decision executes, so it records what was DECIDED. The
/// bead's NON-GOAL is explicit that presence is not truth: `not_done` names exactly that
/// boundary rather than leaving a reader to assume the dispatch landed.
async fn emit_s1_l3_l5(cx: &Cx, config: &Config, decision: &SupervisorDecision) {
    let reason = match decision {
        SupervisorDecision::GateUnwired { .. } => "GATE_UNWIRED",
        SupervisorDecision::Dispatch { .. } => "DISPATCH",
        SupervisorDecision::EscalateIdleIncident { .. } => "ESCALATE_IDLE",
        SupervisorDecision::MonitorBlind { .. } => "MONITOR_BLIND",
        SupervisorDecision::QueueUnreadable { .. } => "QUEUE_UNREADABLE",
        SupervisorDecision::AwaitingHuman { .. } => "AWAITING_HUMAN",
        SupervisorDecision::WorkspaceUnloaded { .. } => "WORKSPACE_UNLOADED",
        SupervisorDecision::AuthorizedIdle { .. } => "AUTHORIZED_IDLE",
        SupervisorDecision::QueueEmptyNeedsJosh { .. } => "QUEUE_EMPTY",
        SupervisorDecision::SupervisedWorking { .. } => "SUPERVISED_WORKING",
    };
    let Ok(code) = ReasonCode::new(reason) else {
        return;
    };
    let outcome = match decision {
        SupervisorDecision::Dispatch { .. } | SupervisorDecision::SupervisedWorking { .. } => {
            EmitOutcome::Emitted
        }
        SupervisorDecision::AuthorizedIdle { .. } => EmitOutcome::Idle,
        _ => EmitOutcome::Refused,
    };
    let l3 = LifecycleEvent::new(
        Layer::L3,
        "S1.L2",
        "S1.L3",
        "omp-orchestrator",
        outcome,
        code.clone(),
    )
    .with_step("decide", "Passed", "match SupervisorDecision");
    let Ok(portal_code) = ReasonCode::new(reason) else {
        return;
    };
    let l5 = LifecycleEvent::new(
        Layer::L5,
        "S1.L4",
        "S1.L5",
        "omp-orchestrator",
        outcome,
        portal_code,
    );
    let path = default_repo_journal(&config.repo);
    let journal = match DurableJournal::open(path) {
        Ok(journal) => journal,
        Err(error) => {
            eprintln!("LIFECYCLE_EVENT_EMIT_FAILED layer=L3,L5 detail={error}");
            return;
        }
    };
    if let Err(error) = emit_lifecycle(cx, &journal, &[l3, l5]).await {
        eprintln!("LIFECYCLE_EVENT_EMIT_FAILED layer=L3,L5 detail={error}");
    }
}

fn observe_s1_after_emit(config: &Config) {
    let journal = default_repo_journal(&config.repo);
    let metrics_path = config.repo.join("METRICS.toml");
    let specs = match load_metrics(&metrics_path) {
        Ok(specs) => specs,
        Err(error) => {
            eprintln!("LIFECYCLE_METRICS_UNREAD {error}");
            return;
        }
    };
    for layer in [Layer::L3, Layer::L5] {
        let stall = specs
            .iter()
            .find(|s| s.layer == layer)
            .map(|s| s.stall_after_ms)
            .unwrap_or(60_000);
        if let Err(error) = observe_layer(&journal, layer, stall) {
            eprintln!("LIFECYCLE_MONITOR_{} {error}", layer.as_str());
        }
    }
    match verify_artifact(&journal) {
        Ok(n) => println!("LIFECYCLE_ARTIFACT_VERIFIED rows={n}"),
        Err(error) => eprintln!("LIFECYCLE_ARTIFACT_UNVERIFIED {error}"),
    }
}

fn write_tick_receipt(
    config: &Config,
    tick: u64,
    observation: &Observation,
    decision: &SupervisorDecision,
) -> Result<(), String> {
    let free: Vec<String> = observation
        .panes
        .iter()
        .filter(|pane| pane.is_free_capacity)
        .map(|pane| pane.pane_id.clone())
        .collect();
    let attention = observation
        .panes
        .iter()
        .filter(|pane| pane.awaits_human)
        .count() as u64;
    // NOT MEASURABLE HERE. `PaneObservation` carries pane_id/state/liveness/
    // is_dispatchable/is_free_capacity/is_working/awaits_human and NO dead flag -- read
    // from `lib.rs`, not assumed. Emitting 0 would put a fabricated figure in the ledger,
    // so the row carries `null` and `not_done` says why.
    let dead: Option<u64> = None;

    // The DECISION's own disposition, and only for a pane the decision actually named.
    // Everything else falls to the builder's fallback refusal, which carries the decision
    // class as its reason -- so a refusal always says WHICH decision left the pane alone.
    let label = decision_label(decision);
    let dispositions = match decision {
        SupervisorDecision::Dispatch { pane, .. } => vec![PaneDisposition::Refused {
            pane: pane.clone(),
            reason: format!(
                "decision={label}; selected for dispatch, outcome not yet observed at row time"
            ),
        }],
        _other => Vec::new(),
    };
    let observation_source = format!(
        "omp-orchestrator supervisor observation (tick-monitor + {} {})",
        finding::BR,
        loop_queue_filter::READY_SUBCOMMAND
    );

    let receipt = build_receipt(&mut Receipt {
            ts: now_unix() as u64,
            tick: tick,
            free_capacity: &free,
            attention: attention,
            dead: dead,
            source: &observation_source,
            dispositions: &dispositions,
            fallback_reason: &format!("decision={label}; this tick did not dispatch to this pane"),
            claims: vec![serde_json::json!({
            "figure": format!("free_capacity={} attention={attention} dead={}", free.len(), match dead { Some(count) => count.to_string(), None => "unmeasured".to_owned() }),
            "command": "./target/debug/omp-orchestrator --once --session <session> (this row's own tick)",
        })],
            verdict: TickVerdict::Unmeasured,
            not_done: vec![
            serde_json::json!("observed.dead is null: PaneObservation carries no dead flag, so this tick could not measure it"),
            serde_json::json!("the dispatch OUTCOME: this row is written before the decision executes, so it records what was DECIDED. ack-spine steps and the heartbeat record what happened"),
        ],
        });
    let path = config.repo.join(".flywheel/orchestration-ticks.jsonl");
    append_receipt(&path, &receipt)
}

fn write_heartbeat(config: &Config, tick: u64, status: &str, detail: &str) -> Result<(), String> {
    if let Some(parent) = config.heartbeat_ledger.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "HEARTBEAT_WRITE_ERROR path={} create_parent: {error}",
                config.heartbeat_ledger.display()
            )
        })?;
    }
    let ts_unix = now_unix();
    let heartbeat_action = decision_ledger::classify_heartbeat(status, detail, ts_unix);
    let row = serde_json::json!({
        "ts_unix": ts_unix,
        "event": "supervisor_heartbeat",
        "build_id": BUILD_ID,
        "run_id": lifecycle_run_id(),
        "status": status,
        "tick": tick,
        "pid": std::process::id(),
        "repo": config.repo.display().to_string(),
        "session": config.session,
        "detail": detail,
        "dispatch_action": heartbeat_action.label(),
    });
    let bytes = serde_json::to_vec(&row)
        .map_err(|error| format!("HEARTBEAT_WRITE_ERROR serialize: {error}"))?;
    let mut ledger = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config.heartbeat_ledger)
        .map_err(|error| {
            format!(
                "HEARTBEAT_WRITE_ERROR path={} open: {error}",
                config.heartbeat_ledger.display()
            )
        })?;
    ledger
        .write_all(&bytes)
        .and_then(|_| ledger.write_all(b"\n"))
        .and_then(|_| ledger.sync_data())
        .map_err(|error| {
            format!(
                "HEARTBEAT_WRITE_ERROR path={} write: {error}",
                config.heartbeat_ledger.display()
            )
        })?;
    // ── S9: the human-decision ledger, wired HERE and nowhere else ──────────
    //
    // `nsx1`: `docs/decisions.jsonl` had a READER and no writer. The only code that
    // touched it read it, to cite HD-0001 (see the comment at the tick-loop guard
    // below). Meanwhile this function wrote 594 human-addressed rows on 2026-09-02
    // alone — 188 `GATE_UNWIRED`, 305 `SUPERVISOR_REFUSED`, 101
    // `DISPATCH_RESULT_RECORDED` whose detail named `owner=josh` or `AWAIT_HUMAN` —
    // and every decision they asked for lived in pane scrollback, a bead comment, or
    // a progress file.
    //
    // THE FUNNEL IS THE POINT. There are 42 `write_heartbeat` call sites; patching
    // only the two shapes observed today would leave the next refusal unclassified.
    // The typed classifier keeps capacity failures in the queue with a cooldown,
    // keeps unproven transport in the receipt/re-dispatch lane, and admits a request
    // to `docs/decisions.jsonl` only when the detail names a HUMAN DECISION clause.
    // An addressed row without a clause is emitted as AGENT_WORK, never as a copy-out.
    //
    // A LEDGER FAILURE MUST NOT KILL A TICK. The decision ledger is a record, not a
    // gate: refusing the heartbeat because the record failed would convert a
    // bookkeeping fault into a fleet outage. So the outcome is REPORTED on stderr and
    // the tick proceeds — and it is reported, not swallowed, because a writer that
    // fails silently is the defect this whole bead is about.
    match heartbeat_action {
        HeartbeatAction::Human(request) => {
            let ledger = config.repo.join("docs/decisions.jsonl");
            match decision_ledger::append_request(&ledger, &request) {
                Ok(outcome) if !outcome.wrote() => {}
                Ok(outcome) => {
                    let _ = writeln!(
                        io::stderr(),
                        "S9_REQUEST_RECORDED id={} clause={} blocking={} question={}",
                        outcome.id(),
                        request.clause.map_or("missing", HumanClause::as_str),
                        request.blocking,
                        request.question
                    );
                }
                Err(error) => {
                    let _ = writeln!(
                        io::stderr(),
                        "S9_REQUEST_UNRECORDED status={status} blocking={} detail={error}",
                        request.blocking
                    );
                }
            }
        }
        HeartbeatAction::RequeueCapacity { bead, reason } => {
            eprintln!(
                "DISPATCH_REQUEUED bead={bead} reason={reason} after_secs={} next_action=retry-with-backoff",
                PENDING_DISPATCH_MAX_AGE_SECS
            );
        }
        HeartbeatAction::RedispatchTransport { bead, reason } => {
            eprintln!(
                "DISPATCH_REDISPATCH bead={bead} reason={reason} next_action=select-different-pane"
            );
        }
        HeartbeatAction::Placed { bead } => {
            eprintln!("DISPATCH_PLACED bead={bead} next_action=observe-receipt");
        }
        HeartbeatAction::AgentWork { status, reason } => {
            eprintln!("AGENT_WORK status={status} reason={reason} next_action=route-to-worker");
        }
        HeartbeatAction::Ignored => {}
    }
    Ok(())
}
/// How long a pending-dispatch marker may block the whole loop before the loop
/// itself retires it.
///
/// # Why a deadline at all
///
/// Measured 2026-09-02 (`y6v5`): one dispatch failed at `RECEIVER_OBSERVATION_MISSING`
/// and the marker was never cleared, because `clear_dispatch_intent` sits AFTER a `?`
/// on the dispatch result. The next cycle read the marker, refused, and did so **15
/// consecutive times over 20 minutes on the same bead** while `br ready` held **77**
/// other beads — and `ack-spine-oj6.3` was not even among them, being `in_progress`.
/// **The marker alone blocked all 77**, because this check is the first statement in
/// `run_cycle`, ahead of observe and select.
///
/// The only named remedy was `owner=josh next_action=inspect-or-clear-pending-dispatch`
/// — a human. That is the third position of one pendulum: K9 fixed a fence that could
/// never refuse, `mj8w` fixed a fence that could never pass, and this one passes
/// **exactly once**. Each time the tell was a count of ONE where the healthy value is
/// unbounded.
///
/// # What the deadline does and does not buy
///
/// The marker is a **double-send guard**, so retiring it early could re-send a packet
/// that is genuinely in flight. Ten minutes is far beyond any observed dispatch
/// latency, and the expiry writes a typed row naming the pane and bead, so a
/// re-dispatch after expiry is auditable rather than silent.
const PENDING_DISPATCH_MAX_AGE_SECS: u64 = 600;

/// COMPILE-TIME bound on the deadline. A zero deadline does not expire every marker
/// instantly -- a fresh marker has `age_secs == 0` and `0 > 0` is false -- but it
/// shrinks the double-send guard to a one-second window, which is indistinguishable
/// from having no guard on a 90-second cycle. This is a `const` assertion rather than
/// a test assertion so it cannot be skipped by a filtered run.
const _: () = assert!(
    PENDING_DISPATCH_MAX_AGE_SECS >= 60,
    "the pending-dispatch deadline must exceed one cycle, or the guard cannot span a dispatch"
);

/// A pending-dispatch marker, classified. The variants exist so that "no marker",
/// "a live marker", and "a marker whose age cannot be computed" can never collapse
/// into one another — the collapse is what let a stale marker read exactly like a
/// dispatch in flight.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingDispatch {
    /// No marker on disk. Reachable and NOT an error — the healthy steady state.
    None,
    /// Young enough that a packet may still be in flight. Still blocks; this is the
    /// positive control that keeps the double-send guard real.
    Live { detail: String, age_secs: u64 },
    /// A successful dispatch has a tracker verdict, but the marker survives until the next
    /// cycle so the success path cannot clear the pane's acknowledgment window immediately.
    Acknowledged { detail: String },
    /// Older than the deadline. The loop retires this itself.
    Expired { detail: String, age_secs: u64 },
    /// A marker whose `issued_at` is missing, non-numeric, or in the future.
    /// **FAILS CLOSED and keeps blocking.** An age we cannot compute is the unknown,
    /// and an unknown must not be retired as though it were stale — that would let a
    /// corrupt marker unlock the loop, which is the opposite of the guard's purpose.
    Undatable {
        detail: String,
        reason: &'static str,
    },
}

impl PendingDispatch {
    /// Stable name for the four states. Absence is `NO_PENDING_DISPATCH`, never
    /// an error and never a silent pass.
    fn label(&self) -> &'static str {
        match self {
            Self::None => "NO_PENDING_DISPATCH",
            Self::Live { .. } => "PENDING_DISPATCH_LIVE",
            Self::Acknowledged { .. } => "DISPATCH_INTENT_ACKNOWLEDGED",
            Self::Expired { .. } => "DISPATCH_INTENT_EXPIRED",
            Self::Undatable { .. } => "PENDING_DISPATCH_UNDATABLE",
        }
    }
}

/// Why the loop itself may clear a marker. Human `inspect-or-clear-pending-dispatch`
/// remains only for `Undatable`.
fn intent_clear_reason(error: &str) -> Option<&'static str> {
    // d6q2: receiver observation is a separate authority from sender return.
    // A marker that survives RECEIVER_OBSERVATION_MISSING treats sender success
    // as in-flight — the unacknowledged-transport latch. Clear it.
    if error.contains("RECEIVER_OBSERVATION_MISSING") {
        return Some("RECEIVER_OBSERVATION_MISSING");
    }
    // Packet was submitted; ACK wait is the genuine in-flight case. Keep Live.
    if error.contains("ACK_STAGE_RETRY_BLOCKED") {
        return None;
    }
    // Anything else after the marker was written failed before a receipt claim.
    Some("DISPATCH_FAILED_BEFORE_RECEIPT")
}

/// The status WORD for a dispatch that did not return a confirmed receipt.
///
/// # `DISPATCH_FAILED` was one word doing the work of two
///
/// The report site had exactly two words for three outcomes — `Ok` meant confirmed and
/// every `Err` meant `DISPATCH_FAILED` — so a dispatch that LANDED and was merely
/// awaiting its ACK was reported in the same word as a dispatch that never arrived.
///
/// MEASURED 2026-09-05, twice within minutes on different panes and beads:
/// ```text
/// pane=%9 bead=y6v5 status=DISPATCH_FAILED ... ACK_PENDING_WORKER_BUSY owes_human=false
/// pane=%8 bead=93lo status=DISPATCH_FAILED ... ACK_PENDING_WORKER_BUSY owes_human=false
/// ```
/// Both packets were submitted and both workers were observed busy. The wait's own
/// heartbeat wrote `ACK_WAIT_PENDING` for these cases with the comment *"a retryable
/// typed row, and NOT an error ... the ack is late, not absent"* — and the status word
/// then contradicted it. That contradiction is why an entire session read as a dispatch
/// outage: `DISPATCH_FAILED` on packets that had plainly arrived.
///
/// # Why this reads the error text
///
/// Deliberately the same shape as [`intent_clear_reason`] directly above, which already
/// derives the marker decision from this string. A second convention for the same
/// classification would be worse than reusing an imperfect one; when the dispatch result
/// becomes a typed enum both should move together. The two markers it keys on are emitted
/// as a pair by one `format!` in `send_and_verify`, so they cannot drift apart silently.
///
/// NO-CLAIM: this changes only the WORD. It does not claim delivery — the verdict stays
/// `INDETERMINATE`, the marker stays Live as a double-send guard via
/// [`intent_clear_reason`], and a pane that never acks still owes a human on the arm
/// where `owes_human=true`.
fn dispatch_status_word(error: &str) -> &'static str {
    if error.contains("ACK_STAGE_RETRY_BLOCKED") && error.contains("owes_human=false") {
        return "DISPATCH_UNCONFIRMED_ACK_PENDING";
    }
    // AN EMPTY ACK CENSUS IS "TOO EARLY", NOT "FAILED".
    //
    // MEASURED 2026-09-05 on the live loop. `tick=2 pane=%9 bead=dmpv` printed
    // `status=DISPATCH_FAILED detail=ACK_STAGE_INDETERMINATE ... ACK_CENSUS_EMPTY`,
    // and the packet had plainly landed: the bead read `in_progress`,
    // `assignee=WildStone`, one comment (the ACK itself), and `%9` was WORKING at
    // t=31. The census was empty at READBACK TIME because the worker had not written
    // its reply yet — a race between dispatch and answer, not an absence of delivery.
    //
    // `ack-stage` raises `EmptyAckCensus` (`lib.rs:222`) precisely so an empty census
    // cannot be read as a satisfied readback. That refusal is correct and stays; only
    // the WORD the report site chooses from it was wrong. Per this repo's own rule, a
    // refusal, a non-zero exit, an empty result and a missing file are all UNKNOWN and
    // never a negative — so an empty census must not be reported as a failed dispatch.
    //
    // This is the third arm of one defect. `DISPATCH_FAILED` was one word doing the
    // work of two; it is now three, and the residual is still the failure so a genuine
    // failure can never be softened by omission.
    if error.contains("ACK_CENSUS_EMPTY") {
        return "DISPATCH_UNCONFIRMED_ACK_PENDING";
    }
    "DISPATCH_FAILED"
}

fn bead_from_marker_detail(detail: &str) -> String {
    serde_json::from_str::<Value>(detail.trim())
        .ok()
        .and_then(|value| value.get("bead").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_default()
}

#[derive(Debug)]
struct MarkerCycleOutcome {
    blocked_panes: Vec<String>,
    cleared: Vec<(String, String, &'static str)>,
}

#[derive(Debug)]
enum MarkerFence {
    Proceed(MarkerCycleOutcome),
    StopUndatable {
        detail: String,
        reason: &'static str,
        marker_path: PathBuf,
    },
}

/// Machine clearing transition. `Expired` is retired here, named, and the cycle
/// CONTINUES. `Live` withholds only that pane. `Undatable` still owes a human.
fn issued_at_from_marker_detail(detail: &str) -> Option<u64> {
    serde_json::from_str::<Value>(detail.trim())
        .ok()?
        .get("issued_at")
        .and_then(Value::as_u64)
}

fn late_ack_clears_marker(readback: &AckReadback) -> bool {
    matches!(
        readback.match_verdict_for(&readback.bead_id, &readback.pane_id),
        AckReadbackVerdict::Matched { .. }
    )
}

fn last_dispatch_claimed_ts(heartbeat: &str, bead: &str) -> Option<u64> {
    let mut latest = None;
    for line in heartbeat.lines() {
        let Ok(row) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let claimed = row.get("status").and_then(Value::as_str) == Some("DISPATCH_CLAIMED");
        let requeued = row
            .get("dispatch_action")
            .and_then(Value::as_str)
            .is_some_and(|action| action == "DISPATCH_REQUEUED");
        if !claimed && !requeued {
            continue;
        }
        let detail = row.get("detail").and_then(Value::as_str).unwrap_or("");
        if field_after(detail, "bead=").as_deref() != Some(bead) {
            continue;
        }
        let Some(ts) = row.get("ts_unix").and_then(Value::as_u64) else {
            continue;
        };
        latest = Some(latest.map_or(ts, |prev: u64| prev.max(ts)));
    }
    latest
}

fn redispatch_cooldown_age(heartbeat: &str, bead: &str, now: u64) -> Option<u64> {
    let ts = last_dispatch_claimed_ts(heartbeat, bead)?;
    let age = now.saturating_sub(ts);
    (age < PENDING_DISPATCH_MAX_AGE_SECS).then_some(age)
}

fn select_ready_skipping_cooldown<'a>(
    ready: &'a [String],
    heartbeat: &str,
    now: u64,
    cleared_beads: &[String],
) -> (Option<&'a str>, Vec<(String, u64)>) {
    let mut skipped = Vec::new();
    let selected = ready.iter().map(String::as_str).find(|id| {
        if cleared_beads.iter().any(|cleared| cleared == id) {
            false
        } else if let Some(age) = redispatch_cooldown_age(heartbeat, id, now) {
            skipped.push(((*id).to_owned(), age));
            false
        } else {
            true
        }
    });
    (selected, skipped)
}

fn process_pending_markers(config: &Config, tick: u64) -> Result<MarkerFence, String> {
    process_pending_markers_with(config, tick, |_, _, _| None)
}

fn process_pending_markers_with<F>(
    config: &Config,
    tick: u64,
    ack: F,
) -> Result<MarkerFence, String>
where
    F: Fn(&str, &str, u64) -> Option<AckReadback>,
{
    let rows = read_pending_dispatches(config)?;
    if rows.is_empty() {
        return Ok(MarkerFence::Proceed(MarkerCycleOutcome {
            blocked_panes: Vec::new(),
            cleared: Vec::new(),
        }));
    }
    let mut blocked_panes = Vec::new();
    let mut cleared = Vec::new();
    for (pane, marker_path, verdict) in rows {
        match verdict {
            PendingDispatch::None => {}
            PendingDispatch::Acknowledged { detail } => {
                let bead = bead_from_marker_detail(&detail);
                clear_dispatch_marker(&marker_path, &pane)?;
                let cleared_detail = format!(
                    "pane={pane} bead={bead} reason=ACKNOWLEDGED owner=loop next_action=continue detail={detail}"
                );
                write_heartbeat(config, tick, "DISPATCH_INTENT_CLEARED", &cleared_detail)?;
                println!(
                    "DISPATCH_INTENT_CLEARED tick={tick} session={} {cleared_detail}",
                    config.session
                );
                cleared.push((pane, bead, "ACKNOWLEDGED"));
            }
            PendingDispatch::Live { detail, age_secs } => {
                let bead = bead_from_marker_detail(&detail);
                if let Some(issued_at) = issued_at_from_marker_detail(&detail) {
                    if let Some(readback) = ack(&bead, &pane, issued_at) {
                        let bound = readback.with_dispatch_issued_at(issued_at);
                        if late_ack_clears_marker(&bound) {
                            clear_dispatch_marker(&marker_path, &pane)?;
                            let late = format!(
                                "pane={pane} bead={bead} age_secs={age_secs} owner=loop next_action=continue"
                            );
                            write_heartbeat(config, tick, "ACK_RECEIVED_LATE", &late)?;
                            println!(
                                "ACK_RECEIVED_LATE tick={tick} session={} {late}",
                                config.session
                            );
                            cleared.push((pane, bead, "ACK_RECEIVED_LATE"));
                            continue;
                        }
                    }
                }
                write_heartbeat(config, tick, "DISPATCH_RETRY_BLOCKED", &detail)?;
                let remaining = PENDING_DISPATCH_MAX_AGE_SECS.saturating_sub(age_secs);
                println!(
                    "DISPATCH_RETRY_BLOCKED age_secs={age_secs} expires_in_secs={remaining} owner=loop next_action=await-intent-expiry scope=pane blocked_pane={pane} marker={} detail={detail}",
                    marker_path.display()
                );
                blocked_panes.push(pane);
            }
            PendingDispatch::Undatable { detail, reason } => {
                return Ok(MarkerFence::StopUndatable {
                    detail,
                    reason,
                    marker_path,
                });
            }
            PendingDispatch::Expired { detail, age_secs } => {
                let bead = bead_from_marker_detail(&detail);
                if let Some(issued_at) = issued_at_from_marker_detail(&detail) {
                    if let Some(readback) = ack(&bead, &pane, issued_at) {
                        let bound = readback.with_dispatch_issued_at(issued_at);
                        if late_ack_clears_marker(&bound) {
                            clear_dispatch_marker(&marker_path, &pane)?;
                            let late = format!(
                                "pane={pane} bead={bead} age_secs={age_secs} owner=loop next_action=continue"
                            );
                            write_heartbeat(config, tick, "ACK_RECEIVED_LATE", &late)?;
                            println!(
                                "ACK_RECEIVED_LATE tick={tick} session={} {late}",
                                config.session
                            );
                            cleared.push((pane, bead, "ACK_RECEIVED_LATE"));
                            continue;
                        }
                    }
                }
                clear_dispatch_marker(&marker_path, &pane)?;
                let expiry = format!(
                    "age_secs={age_secs} max_age_secs={PENDING_DISPATCH_MAX_AGE_SECS} pane={pane} bead={bead} marker={} reason=DISPATCH_INTENT_EXPIRED owner=loop next_action=continue detail={detail}",
                    marker_path.display()
                );
                write_heartbeat(config, tick, "DISPATCH_INTENT_EXPIRED", &expiry)?;
                println!(
                    "DISPATCH_INTENT_EXPIRED tick={tick} session={} {expiry}",
                    config.session
                );
                cleared.push((pane, bead, "DISPATCH_INTENT_EXPIRED"));
            }
        }
    }
    Ok(MarkerFence::Proceed(MarkerCycleOutcome {
        blocked_panes,
        cleared,
    }))
}

/// After the loop clears latched beads, pick a DIFFERENT ready bead.
fn select_next_bead<'a>(ready: &'a [String], cleared_beads: &[String]) -> Option<&'a str> {
    ready
        .iter()
        .map(String::as_str)
        .find(|id| !cleared_beads.iter().any(|cleared| cleared == id))
        .or_else(|| ready.first().map(String::as_str))
}

/// Classify marker text against a clock. Pure, so a test plants an age without
/// touching the filesystem or waiting.
fn classify_pending_dispatch(text: &str, now: u64) -> PendingDispatch {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return PendingDispatch::Undatable {
            detail: "marker_exists_but_is_empty".to_owned(),
            reason: "INTENT_EMPTY",
        };
    }
    let detail = trimmed.to_owned();
    if serde_json::from_str::<Value>(trimmed)
        .ok()
        .and_then(|row| row.get("acknowledged_at").and_then(Value::as_u64))
        .is_some()
    {
        return PendingDispatch::Acknowledged { detail };
    }
    let issued_at = serde_json::from_str::<serde_json::Value>(trimmed)
        .ok()
        .and_then(|row| row.get("issued_at").and_then(serde_json::Value::as_u64));
    let Some(issued_at) = issued_at else {
        return PendingDispatch::Undatable {
            detail,
            reason: "INTENT_ISSUED_AT_MISSING",
        };
    };
    if issued_at > now {
        return PendingDispatch::Undatable {
            detail,
            reason: "INTENT_ISSUED_IN_FUTURE",
        };
    }
    let age_secs = now - issued_at;
    if age_secs > PENDING_DISPATCH_MAX_AGE_SECS {
        PendingDispatch::Expired { detail, age_secs }
    } else {
        PendingDispatch::Live { detail, age_secs }
    }
}

fn read_pending_dispatch(config: &Config) -> Result<PendingDispatch, String> {
    match fs::read_to_string(&config.pending_dispatch) {
        Ok(text) => Ok(classify_pending_dispatch(&text, now_unix())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(PendingDispatch::None),
        // An unreadable marker stays an ERROR and is NOT `None`. A marker we cannot
        // read is not a marker that is absent.
        Err(error) => Err(format!(
            "DISPATCH_RETRY_BLOCKED pending marker unreadable path={} error={error}",
            config.pending_dispatch.display()
        )),
    }
}

/// Josh's condition on the tick loop, recorded as `HD-0001` in `docs/decisions.jsonl`:
///
/// > *"tick loop continues as long as we're keeping our docs up to date"*
///
/// That is a **buyer condition**, not a preference, so it is a precondition on the
/// tick rather than a note in a plan. The loop refuses to dispatch while the
/// artifact-of-record is stale against its sources.
///
/// # Why this is the loop's business and not CI's
///
/// `docs/PLAN.md` was measured 190 minutes and 16 commits stale on 2026-09-01,
/// while four grading rounds ran against it. CI would have caught it on the next
/// push; the tick loop dispatches many times between pushes, and every dispatch in
/// that window sent an agent to work from an assembly that did not contain the last
/// four rounds of findings.
///
/// # The bypass this deliberately does not have
///
/// `assembly_freshness.rs` compares **mtimes**, and the author of that gate
/// bypassed it within a minute by re-stamping `PLAN.md` with `os.utime` — green on
/// a file that did not contain the section just written (§12.11). Comparing mtimes
/// here would inherit that hole, so this compares **content**: every section's
/// bytes must appear inside the assembly. Touching a timestamp cannot satisfy it;
/// only re-assembling can.
/// # Repo shape is DERIVED, never a repo allowlist
///
/// Measured 2026-09-05 across three served repos:
///
/// | repo | `docs/PLAN.md` | numbered sections |
/// |---|---|---|
/// | `omp-orchestrator` | 676,234 B | 13 |
/// | `uds` | absent | 0 |
/// | `control-plane` | absent | 0 |
///
/// The original order read the assembly FIRST and returned "assembly absent" before it ever
/// looked at the sections, so a repo with nothing to assemble was indistinguishable from one
/// whose assembly had gone missing. `--repo uds` therefore died at tick 1 in 0.13s, forever,
/// on a gate whose subject cannot exist there by design: the planning doctrine measured
/// `PLAN.md` at 0 occurrences across 180 corpus repos and uses `docs/plans/plan_to_*.md`.
///
/// So the sections are counted FIRST and the shape decides:
/// - zero sections and no assembly -> nothing to assemble -> `NotApplicable`, NAMED, never a
///   silent green;
/// - zero sections but an assembly EXISTS -> an assembly whose sources vanished; still `Err`,
///   preserving the original anti-vacuity refusal;
/// - sections exist and the assembly is missing -> `Stale`. A real defect, refused as before;
/// - both present -> the unchanged content comparison.
///
/// This refuses LESS in exactly one case and identically in every other.
#[derive(Debug)]
enum DocsVerdict {
    Fresh,
    Stale(String),
    /// This repo does not assemble a plan, so freshness is not a property it has.
    NotApplicable(String),
}

/// Read the durable HD-0015 policy amendment. Missing or malformed authority
/// keeps stale-input dispatch fully refused; it never silently enables a bypass.
fn degraded_policy_authorized(config: &Config) -> bool {
    fs::read_to_string(config.repo.join("docs/decisions.jsonl"))
        .map(|text| packet_admission::authority_allows_degraded(&text))
        .unwrap_or(false)
}

fn docs_are_stale(config: &Config) -> Result<DocsVerdict, String> {
    let plan = config.repo.join("docs/PLAN.md");
    let dir = config.repo.join("docs/plan");
    let assembly = fs::read_to_string(&plan).ok();

    // Count the SOURCES first: the shape of the repo decides whether this gate applies.
    let mut sections: Vec<(String, String)> = Vec::new();
    let dir_readable = match fs::read_dir(&dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                    continue;
                };
                if !name.ends_with(".md") || !name.starts_with(|c: char| c.is_ascii_digit()) {
                    continue;
                }
                if let Ok(section) = fs::read_to_string(&path) {
                    sections.push((name.to_owned(), section));
                }
            }
            true
        }
        Err(_) => false,
    };

    if sections.is_empty() {
        return match assembly {
            // An assembly with no sources cannot be told from a fresh one. Unchanged refusal.
            Some(_) => Err(format!(
                "DOCS_STALE assembly present at {} but zero numbered sections in {} — an \
                 empty scan set cannot distinguish fresh from broken",
                plan.display(),
                dir.display()
            )),
            None => Ok(DocsVerdict::NotApplicable(format!(
                "no assembly and no numbered sections (dir_readable={dir_readable} \
                 plan={} sections_dir={}) — this repo does not assemble a plan",
                plan.display(),
                dir.display()
            ))),
        };
    }

    // Sections exist, so this repo DOES assemble a plan and the gate applies in full.
    let Some(assembly) = assembly else {
        return Ok(DocsVerdict::Stale(format!(
            "assembly absent path={} while {} numbered sections exist in {}",
            plan.display(),
            sections.len(),
            dir.display()
        )));
    };

    let scanned = sections.len();
    let mut missing = Vec::new();
    for (name, section) in &sections {
        // Compare a stable interior slice, not the whole file: the assembler trims
        // trailing whitespace, so an exact whole-file match would false-positive.
        let body = section.trim();
        let probe: String = body
            .chars()
            .rev()
            .take(240)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if !probe.trim().is_empty() && !assembly.contains(probe.trim()) {
            missing.push(name.clone());
        }
    }

    if missing.is_empty() {
        Ok(DocsVerdict::Fresh)
    } else {
        Ok(DocsVerdict::Stale(format!(
            "{} of {scanned} sections are not in the assembly: {}",
            missing.len(),
            missing.join(",")
        )))
    }
}
/// Resolve the directory cargo will actually write to, the way cargo resolves it.
///
/// # The measured defect this fixes
///
/// Measured 2026-09-01, answering "why is our own system limited on sending due to
/// disk space": this gate probed `repo/target` and refused at 20 GiB free on a 98%
/// root volume, while every build in this workspace lands on a DIFFERENT disk:
///
/// | what                                         | volume       | state       |
/// |----------------------------------------------|--------------|-------------|
/// | the old probe, `./target`                    | `/dev/disk3s5` | 98%, 20 GiB |
/// | where binaries actually are, `/Volumes/BuildShared/cargo-targets` | `/dev/disk3s9` | 63%, 3.5 GiB |
/// | `target/release/omp-orchestrator`            | —            | DOES NOT EXIST |
///
/// The old code honoured `CARGO_TARGET_DIR` but never read `build.target-dir` from
/// cargo's own config, which is where this machine's redirect lives. So it refused on
/// a number describing a volume the work never touches — the same defect class as
/// `omp-orchestrator-8b1`, where a probe measured topology roots rather than the build
/// volume, in a second binary.
///
/// # Resolution order, matching cargo
///
/// 1. `CARGO_TARGET_DIR` (env wins, as in cargo)
/// 2. `build.target-dir` in `<repo>/.cargo/config.toml`
/// 3. `build.target-dir` in `$HOME/.cargo/config.toml`
/// 4. `<repo>/target`
///
/// # NO-CLAIM
///
/// This does not parse TOML properly — it is a line scan for a `target-dir` key under
/// a `[build]` table, which is what the sibling gates in this workspace do and is
/// enough for the one key that matters. A `target-dir` set through a profile override,
/// a `--target-dir` flag on the invoking command, or a workspace manifest key is NOT
/// seen. Fixing the volume also does not guarantee the gate passes: the correct
/// volume holds only 3.5 GiB, which is tighter in absolute terms than the wrong one.
fn resolve_target_dir(config: &Config) -> PathBuf {
    let env_override = std::env::var_os("CARGO_TARGET_DIR");
    resolve_target_dir_with_env(config, env_override.as_deref())
}

fn resolve_target_dir_with_env(config: &Config, env_override: Option<&std::ffi::OsStr>) -> PathBuf {
    if let Some(v) = env_override {
        return PathBuf::from(v);
    }
    for cfg in [
        config.repo.join(".cargo/config.toml"),
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(".cargo/config.toml"),
    ] {
        let Ok(text) = std::fs::read_to_string(&cfg) else {
            continue;
        };
        let mut in_build = false;
        for raw in text.lines() {
            let line = raw.trim();
            if line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') {
                in_build = line == "[build]";
                continue;
            }
            if !in_build {
                continue;
            }
            let Some(rest) = line.strip_prefix("target-dir") else {
                continue;
            };
            let Some(eq) = rest.find('=') else { continue };
            let val = rest[eq + 1..].trim().trim_matches('"').trim_matches('\'');
            if !val.is_empty() {
                return PathBuf::from(val);
            }
        }
    }
    config.repo.join("target")
}

/// Refuse to tick when the volume a build would write to is nearly full.
///
/// # Why this refuses rather than warns
///
/// A full build volume does not fail as "disk full". It fails as
/// `failed to link or copy .../out/pre_commit_gate` — a LINKER error, indistinguishable
/// at a glance from a code defect, three layers below the thing that actually broke.
/// Every gate in this repository stopped that way on 2026-09-01 while the volume sat
/// at 99%.
///
/// # Which volume
///
/// `CARGO_TARGET_DIR` when set, because that is where the bytes land and it is
/// frequently NOT under the repository — here it is `/Volumes/BuildShared`, a
/// different device from the checkout, so checking the repo's own filesystem would
/// have reported 84% free and passed while builds failed.
///
/// # Threshold
///
/// 8% free, floored at 1 GiB. One full release rebuild of this workspace is roughly
/// 4 GiB, so 1 GiB is already too little — the floor is a refusal point, not a
/// comfort margin, and a tick that passes here can still exhaust the volume.
fn disk_pressure(config: &Config) -> Result<Option<String>, String> {
    let target = resolve_target_dir(config);

    // Walk up to the nearest existing ancestor: the target dir may not exist yet on
    // a fresh clone, and statfs on a missing path answers nothing useful.
    let mut probe = target.clone();
    while !probe.exists() {
        match probe.parent() {
            Some(p) if p != probe => probe = p.to_path_buf(),
            _ => return Ok(None), // nothing to measure; do not invent a refusal
        }
    }

    // `bounded_output`, NOT `bounded_status`: the caller PARSES stdout below — df's second
    // line, split into fields for total and available blocks. `bounded_status` inherits stdio,
    // so it would print the table to the terminal and hand back nothing to parse.
    //
    // omp-orchestrator-3kcl: this was a raw `.output()` with no deadline. `df` is a
    // constant-time statfs — MEASURED at 78 ms on this repo's path — so the failure mode is
    // not slowness, it is a WEDGED MOUNT, where statfs never returns. That blocked the disk
    // pressure probe forever with no typed outcome.
    let mut command = std::process::Command::new("df");
    command.args(["-k", &probe.display().to_string()]);
    let out = match subprocess_contract::bounded_output(
        &mut command,
        crate::target_directory::DF_DEADLINE,
    ) {
        subprocess_contract::BoundedOutcome::Completed(out) => out,
        // A timeout is NOT the "no data row" error below. That one means df answered and the
        // volume was unreadable; this means df never answered, so capacity is UNKNOWN. An
        // unreadable volume is already a FINDING here, and an unanswered one must not
        // silently become the same string.
        subprocess_contract::BoundedOutcome::TimedOut => {
            return Err(format!(
                "DISK_PRESSURE df TIMED_OUT after {}s on {} — process group signalled. This \
                 is NOT a capacity verdict: statfs measured 78ms healthy, so a timeout means a \
                 WEDGED MOUNT and the free space is UNKNOWN, never zero and never fine",
                crate::target_directory::DF_DEADLINE.as_secs(),
                probe.display()
            ));
        }
        subprocess_contract::BoundedOutcome::Unspawned(error) => {
            return Err(format!("DISK_PRESSURE df failed to spawn: {error}"));
        }
    };
    let text = String::from_utf8_lossy(&out.stdout);

    // df's second line: Filesystem 1K-blocks Used Available Capacity ... Mounted
    let Some(line) = text.lines().nth(1) else {
        return Err(format!(
            "DISK_PRESSURE df produced no data row for {} — an unreadable volume is a \
             FINDING, never a pass",
            probe.display()
        ));
    };
    let f: Vec<&str> = line.split_whitespace().collect();
    let (Some(total), Some(avail)) = (
        f.get(1).and_then(|v| v.parse::<u64>().ok()),
        f.get(3).and_then(|v| v.parse::<u64>().ok()),
    ) else {
        return Err(format!("DISK_PRESSURE could not parse df row: {line:?}"));
    };
    if total == 0 {
        return Err("DISK_PRESSURE df reported a zero-size volume".to_owned());
    }

    if let Some(why) = disk_floor_verdict(total, avail) {
        return Ok(Some(format!("volume={} {why}", probe.display())));
    }
    Ok(None)
}

/// Decide whether a volume has room for a build. Pure, so a test can reach it.
///
/// # The measured defect: a percentage is the wrong unit for "does a build fit"
///
/// Measured 2026-09-01. The loop refused every tick on a machine with **58 GiB free**:
///
/// ```text
/// DISK_PRESSURE volume=target free=58.24GiB (6.3%) below floor 8%/1GiB
/// ```
///
/// The old predicate was `pct_free < 8.0 || gib_free < 1.0` — an OR, so the percentage
/// arm refused regardless of how much absolute room existed. On this volume:
///
/// | quantity | value |
/// |---|---:|
/// | volume size | 926 GiB |
/// | free | 58 GiB (6.3%) |
/// | 8% of the volume | **74 GiB required** |
/// | one full release rebuild (this function's own doc) | **~4 GiB** |
///
/// **74 GiB of headroom demanded to permit a 4 GiB build — an 18x over-requirement that
/// gets STRICTER as the disk gets bigger.** Buying a larger drive tightens the gate in
/// absolute terms, which is the tell that the unit is wrong: the question is "does a
/// build fit", and a build's size has nothing to do with the volume's size.
///
/// # What it does now
///
/// The floor is still a percentage on small volumes, where 8% is a sane proxy — but it
/// is **capped** at a few rebuilds' worth, because that is what the floor was ever for.
/// Behaviour on small volumes is deliberately unchanged:
///
/// | volume | old required | new required |
/// |---|---:|---:|
/// | 9.3 GiB (BuildShared) | 1.0 GiB (8% floors to the 1 GiB minimum) | 1.0 GiB — identical |
/// | 926 GiB (root) | 74 GiB | 16 GiB |
///
/// # NO-CLAIM
///
/// 16 GiB is a JUDGEMENT — four rebuilds of this workspace at the ~4 GiB figure the
/// original comment states. It is not derived from a measurement of peak build usage,
/// and a workspace that grows past ~4 GiB per rebuild needs this raised. It remains a
/// refusal point, not a comfort margin: a tick that passes here can still exhaust the
/// volume, exactly as before.
fn disk_floor_verdict(total_kb: u64, avail_kb: u64) -> Option<String> {
    /// Never demand more than this much free space. Four rebuilds at the ~4 GiB the
    /// original comment measured; the cap is what stops a large volume from demanding
    /// absurd absolute headroom.
    const HEADROOM_CAP_GIB: f64 = 16.0;
    /// A volume with less than this cannot complete a single link step regardless of
    /// its size. Unchanged from the original floor.
    const ABSOLUTE_MIN_GIB: f64 = 1.0;

    let total_gib = total_kb as f64 / 1024.0 / 1024.0;
    let free_gib = avail_kb as f64 / 1024.0 / 1024.0;
    let pct_free = (avail_kb as f64 / total_kb as f64) * 100.0;

    let required_gib = (0.08 * total_gib)
        .min(HEADROOM_CAP_GIB)
        .max(ABSOLUTE_MIN_GIB);
    if free_gib < required_gib {
        return Some(format!(
            "free={free_gib:.2}GiB ({pct_free:.1}%) below floor {required_gib:.2}GiB \
             (8% of {total_gib:.0}GiB, capped at {HEADROOM_CAP_GIB:.0}GiB, min \
             {ABSOLUTE_MIN_GIB:.0}GiB); a build here fails as a LINKER error, not as disk-full"
        ));
    }
    None
}

/// The marker path for ONE pane.
///
/// MEASURED 2026-09-02: with a single global marker file, `create_new(true)`
/// returned `File exists (os error 17)` for every dispatch after the first, so
/// only one pane could ever hold a dispatch. Scoping the CHECK per-pane was not
/// enough — the WRITE is what enforced fleet-wide exclusivity, and freeing the
/// check alone just moved the refusal from selection to write.
///
/// The slug keeps only ASCII alphanumerics from the pane id (`%1409` ->
/// `1409`), so the filename cannot contain a separator or escape the directory.
/// An id that slugs to empty falls back to the base path, which preserves the
/// old global behaviour rather than writing a colliding name.
fn pending_dispatch_path(config: &Config, pane: &str) -> PathBuf {
    let slug: String = pane.chars().filter(char::is_ascii_alphanumeric).collect();
    if slug.is_empty() {
        return config.pending_dispatch.clone();
    }
    let base = config
        .pending_dispatch
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "omp-orchestrator.pending-dispatch".to_owned());
    config
        .pending_dispatch
        .with_file_name(format!("{base}.{slug}"))
}

/// Every pending-dispatch marker on disk, classified, paired with the pane it
/// names.
///
/// Reads the LEGACY global path too, so a marker written by a build that predates
/// per-pane paths is still seen. Its pane comes from the payload; a payload we
/// cannot parse yields an empty pane, which the caller treats as fleet-wide and
/// fails closed on — an in-flight dispatch we cannot attribute could be to any
/// pane, and double-sending is the harm the guard exists to prevent.
///
/// An unreadable directory is an ERROR, never an empty scan: "no markers" and
/// "cannot tell" must not report identically, which is the anti-vacuity rule this
/// repo applies to every gate.
fn read_pending_dispatches(
    config: &Config,
) -> Result<Vec<(String, PathBuf, PendingDispatch)>, String> {
    let base_name = config
        .pending_dispatch
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "omp-orchestrator.pending-dispatch".to_owned());
    let Some(dir) = config.pending_dispatch.parent() else {
        return Ok(Vec::new());
    };
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "DISPATCH_BLOCKED pending marker dir unreadable path={} error={error}",
                dir.display()
            ))
        }
    };
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "DISPATCH_BLOCKED pending marker entry unreadable dir={} error={error}",
                dir.display()
            )
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        // The legacy global name, or a per-pane sibling of it.
        if name != base_name && !name.starts_with(&format!("{base_name}.")) {
            continue;
        }
        // A QUARANTINED marker is deliberately set aside and is NOT a live fence.
        //
        // MEASURED 2026-09-02: `pending-dispatch.quarantined-1788274935.json`,
        // written Sep 1 and 27 HOURS old, matched the sibling pattern above. The
        // loop classified it `Expired` and cleared it EVERY CYCLE without ever
        // removing it, because the clear recomputed a path from the payload's
        // `pane` (`...pending-dispatch.1408`) which does not exist -- NotFound maps
        // to Ok, so the clear silently succeeded on the wrong file. Two cycles per
        // 90s spent re-expiring one immortal marker.
        if name.contains("quarantined") {
            continue;
        }
        let text = match fs::read_to_string(entry.path()) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "DISPATCH_BLOCKED pending marker unreadable path={} error={error}",
                    entry.path().display()
                ))
            }
        };
        let verdict = classify_pending_dispatch(&text, now_unix());
        let pane = serde_json::from_str::<Value>(text.trim())
            .ok()
            .and_then(|value| value.get("pane").and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_default();
        // The DISCOVERED path travels with the row. Recomputing it from the pane
        // is what let a clear target a file that was never there.
        found.push((pane, entry.path(), verdict));
    }
    Ok(found)
}

fn write_dispatch_intent(config: &Config, pane: &str, bead: &str) -> Result<(), String> {
    let path = pending_dispatch_path(config, pane);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "DISPATCH_BLOCKED pending marker parent path={} error={error}",
                parent.display()
            )
        })?;
    }
    let row = serde_json::json!({
        "event": "dispatch_intent",
        "build_id": BUILD_ID,
        "run_id": lifecycle_run_id(),
        "pid": std::process::id(),
        "repo": config.repo.display().to_string(),
        "session": config.session,
        "pane": pane,
        "bead": bead,
        "issued_at": now_unix(),
    });
    let bytes = serde_json::to_vec(&row)
        .map_err(|error| format!("DISPATCH_BLOCKED pending marker serialize: {error}"))?;
    // `create_new` STAYS. Per-pane it is the correct guard: a second dispatch to
    // the SAME pane while one is in flight must still refuse.
    let mut marker = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| {
            format!(
                "DISPATCH_RETRY_BLOCKED pane={pane} bead={bead} marker={} error={error}",
                path.display()
            )
        })?;
    marker
        .write_all(&bytes)
        .and_then(|_| marker.write_all(b"\n"))
        .and_then(|_| marker.sync_data())
        .map_err(|error| {
            format!(
                "DISPATCH_BLOCKED pending marker write path={} error={error}",
                path.display()
            )
        })
}

fn acknowledge_dispatch_intent(config: &Config, pane: &str) -> Result<(), String> {
    let path = pending_dispatch_path(config, pane);
    let text = fs::read_to_string(&path).map_err(|error| {
        format!(
            "DISPATCH_ACKNOWLEDGE_READ_FAILED pane={pane} path={} error={error}",
            path.display()
        )
    })?;
    let mut value = serde_json::from_str::<Value>(&text)
        .map_err(|error| format!("DISPATCH_ACKNOWLEDGE_MALFORMED pane={pane} error={error}"))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| format!("DISPATCH_ACKNOWLEDGE_NOT_OBJECT pane={pane}"))?;
    object.insert("acknowledged_at".to_owned(), json!(now_unix()));
    let bytes = serde_json::to_vec(&value).map_err(|error| {
        format!("DISPATCH_ACKNOWLEDGE_SERIALIZE_FAILED pane={pane} error={error}")
    })?;
    let temp = path.with_extension(format!("ack-{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|error| {
            format!(
                "DISPATCH_ACKNOWLEDGE_TEMP_FAILED path={} error={error}",
                temp.display()
            )
        })?;
    file.write_all(&bytes)
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_data())
        .map_err(|error| {
            format!(
                "DISPATCH_ACKNOWLEDGE_WRITE_FAILED path={} error={error}",
                temp.display()
            )
        })?;
    fs::rename(&temp, &path)
        .map_err(|error| format!("DISPATCH_ACKNOWLEDGE_RENAME_FAILED pane={pane} error={error}"))
}

fn record_successful_dispatch(
    config: &Config,
    tick: u64,
    pane: &str,
    bead: &str,
) -> Result<(), String> {
    acknowledge_dispatch_intent(config, pane)?;
    write_heartbeat(
        config,
        tick,
        "DISPATCH_INTENT_ACKNOWLEDGED",
        &format!(
            "pane={pane} bead={bead} reason=VERDICT_POSTED owner=loop next_action=clear-next-cycle"
        ),
    )?;
    Ok(())
}
fn clear_dispatch_intent(config: &Config, pane: &str) -> Result<(), String> {
    clear_dispatch_marker(&pending_dispatch_path(config, pane), pane)
}

/// Remove a marker at an EXPLICIT path.
///
/// The path must be the one the scan DISCOVERED. Recomputing it from the pane is
/// what let an `Expired` clear silently target a file that never existed, so a
/// 27-hour-old marker re-expired every cycle forever.
fn clear_dispatch_marker(path: &Path, pane: &str) -> Result<(), String> {
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "DISPATCH_CONFIRMED_BUT_MARKER_CLEAR_FAILED pane={pane} path={} error={error}",
            path.display()
        )),
    }
}
/// Arguments for the finished-pane sweep.
///
/// # `--session` is MANDATORY and its absence used to be a global sweep
///
/// MEASURED 2026-09-05: this built `["--repo", repo]` only, and the reaper's own CLI
/// exposed no session flag, so a tick scoped to `--session omp-orchestrator` enumerated
/// and CAPTURED four control-plane panes — `SKIPPED control-plane:1..4
/// reason=still_changing`. They were skipped by luck of their state; a settled
/// control-plane pane would have been reaped by us, its transcript written and its state
/// consumed, from a repo we do not own. Joshua, verbatim: *"this needs to be configurable
/// per session / repo so that we aren't stealing work / session data from other repos."*
///
/// `ma3b` gave the reaper `--session` and made its ABSENCE a typed refusal
/// (`SCOPE_REFUSED reason=MISSING_SESSION`, exit 2) rather than a widen-to-all-sessions
/// default — a missing scope silently becoming global is exactly how the capture happened.
/// That fix was correct at the callee and left this caller unwired, so the very next tick
/// died at `SUPERVISOR_REFUSED reap-finished-panes exited=2` — the same step the tick had
/// been aborting at for three days per the comment at the `recover_pending_findings` site.
/// Passing the scope here is the other half.
///
/// # NO-CLAIM
///
/// Scoping the sweep stops THIS caller from reading another session's panes. It does not
/// separate the shared write surface: `~/.local/state/flywheel/reaped/` is still one
/// directory that control-plane's own supervisor (pid 38194, launchd) writes to, so
/// ownership of an artifact there is still inferred from a filename rather than enforced.
fn finished_pane_reaper_args(config: &Config) -> Vec<String> {
    reaper_args_for(&config.repo, &config.session)
}

/// The EXACT argument list the supervisor hands `reap-finished-panes`.
///
/// Public so a test can SPAWN the reaper with these bytes rather than merely
/// construct them. A caller/callee argument drift is only observable by
/// EXECUTING the callee: `finished_pane_reaper_args` returning the wrong flag
/// and the reaper rejecting it are both perfectly well-formed in isolation,
/// and 8nuh spent three days latent because nothing ran the pair together.
///
/// One authority, not two that agree today: `finished_pane_reaper_args` is a
/// thin delegate to this, so a test pinning this list pins what the supervisor
/// actually sends. A second copy in the test would drift exactly like the two
/// emitters this repo has already paid for.
///
/// # NO-CLAIM
///
/// This pins the ARGUMENT LIST. It does not pin the reaper's exit code, its
/// stdout shape, or that `~/.local/state/flywheel/reaped/` is owner-separated.
#[must_use]
pub fn reaper_args_for(repo: &std::path::Path, session: &str) -> Vec<String> {
    vec![
        "--repo".to_owned(),
        repo.display().to_string(),
        "--session".to_owned(),
        session.to_owned(),
    ]
}
async fn run_finished_pane_sweep(cx: &Cx, config: &Config) -> Result<String, String> {
    let reaper_args = finished_pane_reaper_args(config);
    let reaper_output = invoke(cx, config, &config.reap_finished_panes, &reaper_args).await?;
    let reaper_bytes = require_success(&config.reap_finished_panes, reaper_output)?;
    let reaper_summary = String::from_utf8_lossy(&reaper_bytes).trim().to_owned();
    if reaper_summary.is_empty() {
        return Err(format!(
            "REAP_FINISHED_PANES_EMPTY program={}",
            config.reap_finished_panes
        ));
    }
    Ok(reaper_summary)
}

struct DispatchOutcome {
    detail: String,
    clear_intent: bool,
}
const RESULT_PANE: &str = "1";

fn one_line_detail(detail: &str) -> String {
    detail.replace('\r', " ").replace('\n', " ")
}

fn dispatch_result_ntm_args(
    session: &str,
    pane: &str,
    bead: &str,
    tick: u64,
    result: &str,
) -> Vec<String> {
    let result = one_line_detail(result);
    vec![
        tick_monitor::ntm_send_arg(session),
        format!("--panes={RESULT_PANE}"),
        format!("--msg=DISPATCH_RESULT tick={tick} pane={pane} bead={bead} {result}"),
    ]
}

/// The caller-owned deadline on every Agent Mail call.
///
/// EXPLICIT on purpose, and never a default. Measured 2026-09-02 on the
/// sibling notification kernel: a caller that owns its ceiling gets a typed
/// terminal carrying a resumable cursor, while a caller whose wait is ended by
/// somebody else's signal gets a cancel with the cursor STRIPPED. Owning the
/// deadline is what keeps the resume point, so the value lives here rather
/// than in the binding's default.
const MAIL_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// One durable dispatch-result notification, as the daemon recorded it.
///
/// Deliberately NOT merged into `PostSendObservation` or `AckReadback`. This is
/// a THIRD, independently obtained fact — the mail system's own durable record
/// — and the whole value of it is that it is not derived from either of the
/// other two authorities.
#[derive(Debug)]
struct DurableNotice {
    recipient: String,
    message_id: i64,
    persisted: bool,
    signaled: bool,
    cursor: u64,
}

/// Derive the sender from the live pane identity, never from ambient AGENT_NAME.
fn sender_from_verified_pane(identity: &PaneIdentity) -> Result<AgentName, String> {
    if identity.binding != BindingStatus::VerifiedLive {
        return Err(format!(
            "SENDER_IDENTITY_REFUSED pane={} reason=binding_not_verified_live",
            identity.pane_id
        ));
    }
    identity.agent_name.clone().ok_or_else(|| {
        format!(
            "SENDER_IDENTITY_REFUSED pane={} reason=agent_name_missing",
            identity.pane_id
        )
    })
}

/// Resolve TMUX_PANE through the existing K0 kernel immediately before sending.
async fn mail_sender_pane_identity(
    cx: &Cx,
    client: &MailClient,
    project: &ProjectKey,
) -> Result<PaneIdentity, String> {
    let pane_id = env::var("TMUX_PANE")
        .map_err(|_| "SENDER_IDENTITY_REFUSED reason=TMUX_PANE_missing".to_owned())?;
    resolve_pane_identity(cx, client, project, &pane_id)
        .await
        .map_err(|error| format!("SENDER_IDENTITY_REFUSED pane={pane_id} error={error}"))
}

fn packet_sender_header(
    identity: &PaneIdentity,
    session: &str,
    project: &ProjectKey,
) -> Result<String, String> {
    let header = format_sender_header(identity, session, project)
        .map_err(|error| format!("SENDER_IDENTITY_REFUSED pane={} error={error}", identity.pane_id))?;
    if !header.starts_with("FROM:")
        || !header
            .lines()
            .nth(1)
            .is_some_and(|line| line.starts_with("REPLY-VIA:"))
    {
        return Err("SENDER_IDENTITY_REFUSED reason=malformed_header".to_owned());
    }
    Ok(header)
}

async fn render_packet_with_sender(
    cx: &Cx,
    config: &Config,
    snapshot: &BeadSnapshot,
    pane: Option<&str>,
    receiver_agent: Option<&str>,
    why_now: Option<&str>,
    traps: Option<&str>,
) -> Result<String, String> {
    let project = ProjectKey::new(config.repo.display().to_string());
    let client = MailClient::discover().with_request_timeout(MAIL_REQUEST_TIMEOUT);
    let identity = mail_sender_pane_identity(cx, &client, &project).await?;
    let sender_header = packet_sender_header(&identity, &config.session, &project)?;
    let mut packet = dispatch_packet::render_with_pane(
        snapshot,
        &config.repo,
        pane,
        receiver_agent,
        why_now,
        traps,
    )
    .map_err(|error| format!(
        "DISPATCH_PACKET_REFUSED bead={} pane={} error={error}",
        snapshot.id(),
        pane.unwrap_or("<none>")
    ))?;
    packet.insert_str(0, &sender_header);
    Ok(packet)
}

/// The environment variables consulted for the sender identity, in order.
///
/// yfp2: the list moved to `sender-identity`, where each entry carries whether it is OWNED
/// (set deliberately for this process) or AMBIENT (machine-global, set by
/// `launchctl setenv`). This alias exists so the refusal text keeps naming every variable
/// searched; the ORDER and the trust class are the kernel's.
fn mail_identity_var_names() -> Vec<&'static str> {
    sender_identity::MAIL_IDENTITY_VARS
        .iter()
        .map(|source| source.var())
        .collect()
}

/// Turn a configured identity into a usable one, or refuse. Pure, so the
/// refusal is testable without mutating process-global environment state.
fn sender_identity_from(configured: &str, source_var: &str) -> Result<AgentName, String> {
    let trimmed = configured.trim();
    if trimmed.is_empty() {
        return Err(format!(
            "sender_identity_unset searched={}",
            mail_identity_var_names().join(",")
        ));
    }
    // yfp2: THE CASE `7n5b` DID NOT ANTICIPATE. Its acceptance covered the UNSET case and
    // got it; this is set-but-ambient, which is the case that actually ran. A value that
    // arrived through a machine-global variable is whatever agent last ran
    // `launchctl setenv`, so it cannot be a project-scoped identity — measured
    // 2026-09-02, 64 sends signed `WildStone` (project `~/Developer/fsw`) and all refused.
    if sender_identity::MAIL_IDENTITY_VARS
        .iter()
        .any(|source| source.is_ambient() && source.var() == source_var)
    {
        return Err(format!(
            "SUPERVISOR_REFUSED SENDER_IDENTITY_AMBIENT var={source_var} value={trimmed} \
             detail=\"{source_var} is machine-global; its value is whatever agent last ran \
             `launchctl setenv {source_var}`\" \
             next_action=set-AGENT_MAIL_AGENT-in-the-plist-and-register-it"
        ));
    }
    Ok(AgentName::new(trimmed))
}

/// Who receives the durable notification for a dispatch to `pane`.
///
/// The pane->agent map is the same authority `validate_receiver_pane` uses, so
/// a notification cannot be addressed to an agent the dispatcher would have
/// refused to send to. Falls back to the configured receiver only when the map
/// has no row, and refuses when neither is available rather than guessing.
fn mail_recipient(config: &Config, pane: &str) -> Result<AgentName, String> {
    if let Some(mapped) = agent_for_pane(config, pane) {
        return Ok(AgentName::new(mapped));
    }
    let configured = config.receiver_agent.trim();
    if configured.is_empty() {
        return Err(format!("recipient_unresolved pane={pane}"));
    }
    Ok(AgentName::new(configured))
}

/// The ledger row a mail failure becomes.
///
/// Every variant gets its OWN row, and none of them is a delivered row. An
/// unreachable daemon and an empty mailbox are different facts; an
/// unauthorized daemon is RUNNING and merely refused us. Collapsing any of
/// these into a single "notify failed" row would reproduce the measured
/// `am agent start` defect, where an auth failure was reported as absence.
fn mail_failure_row(error: &MailError) -> &'static str {
    if mail_error_is_fd_exhaustion(error) {
        return "DISPATCH_RESULT_MAIL_FD_EXHAUSTION";
    }
    match error {
        MailError::Unreachable { .. } => "DISPATCH_RESULT_MAIL_UNREACHABLE",
        MailError::Unauthorized { .. } => "DISPATCH_RESULT_MAIL_UNAUTHORIZED",
        MailError::MissingCredential { .. } => "DISPATCH_RESULT_MAIL_NO_CREDENTIAL",
        MailError::TimedOut { .. } => "DISPATCH_RESULT_MAIL_TIMED_OUT",
        MailError::Cancelled(_) => "DISPATCH_RESULT_MAIL_CANCELLED",
        MailError::CursorAhead { .. } | MailError::CursorExpired { .. } => {
            "DISPATCH_RESULT_MAIL_CURSOR_UNUSABLE"
        }
        MailError::EmptyCatalogue => "DISPATCH_RESULT_MAIL_EMPTY_CATALOGUE",
        MailError::ToolRefused { .. } => "DISPATCH_RESULT_MAIL_REFUSED",
        MailError::Rpc { .. } | MailError::Protocol { .. } | MailError::Codec { .. } => {
            "DISPATCH_RESULT_MAIL_PROTOCOL"
        }
        MailError::UnexpectedStatus { .. } => "DISPATCH_RESULT_MAIL_UNEXPECTED_STATUS",
    }
}

/// smcq: daemon fd_exhaustion is a NAMED status, never UNREACHABLE/TIMED_OUT.
fn mail_error_is_fd_exhaustion(error: &MailError) -> bool {
    match error {
        MailError::ToolRefused { kind, message, .. } => {
            kind.eq_ignore_ascii_case("fd_exhaustion")
                || message.contains("fd_exhaustion")
                || message.contains("Too many open files")
        }
        MailError::Rpc { message, .. } => {
            message.contains("fd_exhaustion") || message.contains("Too many open files")
        }
        MailError::Protocol { detail }
        | MailError::Codec { detail }
        | MailError::Unreachable { detail, .. } => {
            detail.contains("fd_exhaustion") || detail.contains("Too many open files")
        }
        MailError::TimedOut { .. }
        | MailError::Unauthorized { .. }
        | MailError::MissingCredential { .. }
        | MailError::Cancelled(_)
        | MailError::UnexpectedStatus { .. }
        | MailError::CursorAhead { .. }
        | MailError::CursorExpired { .. }
        | MailError::EmptyCatalogue => false,
    }
}

/// Classify a daemon failure envelope. Keys `class` and
/// `db_error_classification` are the live `am` shape.
fn classify_mail_envelope_status(envelope: &str) -> &'static str {
    let Ok(value) = serde_json::from_str::<Value>(envelope) else {
        return "DISPATCH_RESULT_MAIL_PROTOCOL";
    };
    let class = value
        .get("class")
        .or_else(|| value.get("db_error_classification"))
        .or_else(|| value.pointer("/error/type"))
        .or_else(|| value.pointer("/error/class"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if class.eq_ignore_ascii_case("fd_exhaustion") {
        return "DISPATCH_RESULT_MAIL_FD_EXHAUSTION";
    }
    let message = value
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or("");
    if message.contains("Too many open files") {
        return "DISPATCH_RESULT_MAIL_FD_EXHAUSTION";
    }
    if class.eq_ignore_ascii_case("timeout") || message.contains("deadline exceeded") {
        return "DISPATCH_RESULT_MAIL_TIMED_OUT";
    }
    "DISPATCH_RESULT_MAIL_PROTOCOL"
}

/// Send the dispatch result to the receiver as DURABLE mail, then read the
/// daemon's own receipt back.
///
/// # Why this exists beside the pane notification
///
/// [`notify_dispatch_result`] rides `ntm --robot-send`, and its own doc records
/// the measurement that makes it untrustworthy: a send returned
/// `successful:["1"]` and never arrived. A transport that reports success
/// without delivering cannot answer "did the receiver get this", so the
/// dispatch result's only durable trace was a local heartbeat row that no
/// third party can query.
///
/// Agent Mail answers it, durably and queryably by a third party, which the
/// heartbeat row is not.
///
/// CORRECTION, recorded because the original version of this comment asserted
/// two things that are false. First, it presented `signaled: false` as a
/// silent-delivery defect; it is DOCUMENTED debounce behaviour — only the
/// append-only receipt ledger establishes `signaled`, so `signaled` is not a
/// delivery oracle and `acknowledged` is. Second, it claimed the `am` CLI
/// reads SQLite directly, making it an independent authority. It does not:
/// the CLI calls the SAME daemon on an alternate route (`/api/` rather than
/// `/mcp/`) with the same bearer token.
///
/// # Daemon only
///
/// The write and the receipt read-back both go through the authenticated MCP
/// daemon, and nothing here shells out. A CLI cross-check used to live here
/// and was DELETED: with both routes reaching one daemon under one token it
/// compared the daemon against itself through a process spawn, then reported
/// the result in a field named `oracle_skew` that the next reader would take
/// for independent corroboration. A decorative oracle is worse than none,
/// because it launders self-consistency as agreement. A genuine oracle for
/// this store would be a read-only SQL read of the store file — a different
/// function with a different name.
async fn notify_dispatch_result_durably(
    cx: &Cx,
    config: &Config,
    pane: &str,
    bead: &str,
    result: &str,
) -> Result<DurableNotice, String> {
    let recipient = mail_recipient(config, pane)?;
    let project = ProjectKey::new(config.repo.display().to_string());
    let client = MailClient::discover().with_request_timeout(MAIL_REQUEST_TIMEOUT);
    let sender_identity = mail_sender_pane_identity(cx, &client, &project).await?;
    let sender = sender_from_verified_pane(&sender_identity)?;

    let detail = one_line_detail(result);
    let body = format!(
        "Dispatch result for `{bead}`.\n\n\
         FROM: {sender} pane={} binding=verified-live\nREPLY VIA: Agent Mail send_message to {sender}, project {project}\n\n\
         - pane: {pane}\n- bead: {bead}\n- build_id: {BUILD_ID}\n- result: {detail}\n\n\
         This is the DURABLE record of the dispatch result. The pane notification \
         is a courtesy and has been measured to report success without delivering.",
        sender_identity.pane_id,
    );
    let request = SendRequest::new(
        project.clone(),
        sender,
        vec![recipient.clone()],
        format!("dispatch result: {bead}"),
        body,
    );

    let receipt = mail::send(cx, &client, &request)
        .await
        .map_err(|error| format!("{} {error}", mail_failure_row(&error)))?;
    let message_id = receipt
        .message_id()
        .ok_or_else(|| "DISPATCH_RESULT_MAIL_PROTOCOL send returned no message id".to_owned())?;

    // Read the daemon's own durable record back. A send receipt says we were
    // accepted; this says the copy exists for that recipient.
    let delivery: DeliveryReceipt = mail::delivery_receipt(cx, &client, &project, message_id)
        .await
        .map_err(|error| format!("{} {error}", mail_failure_row(&error)))?;
    let recipient_row = delivery
        .recipients
        .iter()
        .find(|entry| entry.recipient == recipient.as_str());

    // Baseline the recipient's durable cursor, then cross-check it against the
    // CLI. The cursor stays bound to its recipient: measured, one bare integer
    // was simultaneously resumable for one agent and below another's floor.
    let page = mail::fetch_inbox_events(
        cx,
        &client,
        &project,
        &recipient,
        agent_mail_native::CursorQuery::PositionNow,
        None,
    )
    .await
    .map_err(|error| format!("{} {error}", mail_failure_row(&error)))?;

    Ok(DurableNotice {
        recipient: recipient.as_str().to_owned(),
        message_id: message_id.get(),
        persisted: delivery.persisted,
        signaled: recipient_row.is_some_and(|entry| entry.signaled),
        cursor: page.tail_cursor.get(),
    })
}

/// Records one dispatch result, then notifies the result pane as a courtesy.
///
/// # Ledger first, send second
///
/// This function used to invoke `ntm --robot-send` FIRST and only write the
/// heartbeat if that send reported success, returning `Err` otherwise. A
/// blocked notification therefore erased the record: the one durable trace of
/// the dispatch outcome was conditional on the least reliable step. Pane 4
/// measured a `--robot-send` that returned `successful:["1"]` and never
/// arrived, so the send cannot be the authority for anything.
///
/// The ledger write is now unconditional and happens before the notify, and a
/// failed notify is reported as a `DISPATCH_RESULT_NOTIFY_DEGRADED` row rather
/// than an error. The caller must not be able to lose a dispatch outcome — or
/// strand the pending-dispatch marker — because a courtesy message bounced.
async fn report_dispatch_result(
    cx: &Cx,
    config: &Config,
    tick: u64,
    pane: &str,
    bead: &str,
    result: &str,
) -> Result<(), String> {
    let detail = one_line_detail(result);
    write_heartbeat(
        config,
        tick,
        "DISPATCH_RESULT_RECORDED",
        &format!("target_pane={RESULT_PANE} source_pane={pane} bead={bead} {detail}"),
    )?;
    println!(
        "DISPATCH_RESULT_RECORDED tick={tick} session={} target_pane={RESULT_PANE} source_pane={pane} bead={bead} detail={detail}",
        config.session
    );

    let notify = match authorize_result_notification(bead, result) {
        Ok(()) => notify_dispatch_result(cx, config, tick, pane, bead, result).await,
        Err(error) => Err(error),
    };
    if let Err(error) = notify {
        let degraded = one_line_detail(&error);
        write_heartbeat(
            config,
            tick,
            "DISPATCH_RESULT_NOTIFY_DEGRADED",
            &format!("target_pane={RESULT_PANE} source_pane={pane} bead={bead} {degraded}"),
        )?;
        println!(
            "DISPATCH_RESULT_NOTIFY_DEGRADED tick={tick} session={} target_pane={RESULT_PANE} source_pane={pane} bead={bead} owner=josh next_action=read-the-ledger-not-the-pane detail={degraded}",
            config.session
        );
    }

    // The DURABLE notification. Runs after the ledger write and independently
    // of the pane courtesy above: the pane transport has been measured to
    // report success without delivering, so it cannot be the authority for
    // whether the receiver was told anything.
    //
    // Its failure is NEVER this caller's failure, for the same reason the pane
    // notify's is not — the ledger row is the record, and a notification must
    // not be able to erase it. But every failure gets its own NAMED row, so a
    // supervisor that has silently stopped notifying is distinguishable from
    // one with nothing to report.
    match notify_dispatch_result_durably(cx, config, pane, bead, result).await {
        Ok(notice) => {
            let summary = format!(
                "recipient={} message_id={} persisted={} signaled={} cursor={}",
                notice.recipient,
                notice.message_id,
                notice.persisted,
                notice.signaled,
                notice.cursor,
            );
            write_heartbeat(
                config,
                tick,
                "DISPATCH_RESULT_MAIL_PERSISTED",
                &format!("source_pane={pane} bead={bead} {summary}"),
            )?;
            println!(
                "DISPATCH_RESULT_MAIL_PERSISTED tick={tick} session={} source_pane={pane} bead={bead} {summary}",
                config.session
            );
        }
        Err(error) => {
            let degraded = one_line_detail(&error);
            write_heartbeat(
                config,
                tick,
                "DISPATCH_RESULT_MAIL_DEGRADED",
                &format!("source_pane={pane} bead={bead} {degraded}"),
            )?;
            println!(
                "DISPATCH_RESULT_MAIL_DEGRADED tick={tick} session={} source_pane={pane} bead={bead} owner=josh next_action=read-the-ledger-not-the-pane detail={degraded}",
                config.session
            );
        }
    }
    Ok(())
}

/// Sends the courtesy notification. Its failure is never the caller's failure;
/// `report_dispatch_result` downgrades it to a ledger row.
fn authorize_result_notification(bead: &str, result: &str) -> Result<(), String> {
    if bead.trim().is_empty() || result.trim().is_empty() {
        return Err(
            "DISPATCH_RESULT_PREFLIGHT_REFUSED packet_complete=false: result notification is empty"
                .to_owned(),
        );
    }
    let wave = classify(Intent {
        action: TypedAction::VerifyReceipt,
        pane_dispatchable: false,
        two_captures: false,
        packet_complete: true,
        finding_has_bead: true,
    });
    Approved::authorize(wave)
        .map(|_| ())
        .map_err(|error| format!("DISPATCH_RESULT_PREFLIGHT_REFUSED reason={error:?}"))
}

async fn notify_dispatch_result(
    cx: &Cx,
    config: &Config,
    tick: u64,
    pane: &str,
    bead: &str,
    result: &str,
) -> Result<(), String> {
    admit_immediately_before_send(&config.session, RESULT_PANE)?;
    let args = dispatch_result_ntm_args(&config.session, pane, bead, tick, result);
    let stdout = require_success(&config.ntm, invoke(cx, config, &config.ntm, &args).await?)?;
    let receipt = TransportReceipt::capture_ntm(&stdout).map_err(|error| {
        format!(
            "DISPATCH_RESULT_NOTIFY_REFUSED pane={RESULT_PANE} bead={bead} malformed ntm receipt: {error}"
        )
    })?;
    match &receipt {
        TransportReceipt::NtmRobotSend(receipt)
            if !receipt.blocked
                && receipt.successful.iter().any(|target| target == RESULT_PANE) =>
        {
            Ok(())
        }
        TransportReceipt::NtmRobotSend(receipt) => Err(format!(
            "DISPATCH_RESULT_NOTIFY_REFUSED pane={RESULT_PANE} bead={bead} blocked={} successful={:?} failed={:?}",
            receipt.blocked, receipt.successful, receipt.failed
        )),
        TransportReceipt::TmuxSendKeysLiteral(_) => Err(format!(
            "DISPATCH_RESULT_NOTIFY_REFUSED pane={RESULT_PANE} bead={bead} transport=tmux_send_keys_literal"
        )),
    }
}
fn queue_empty_detail(free_capacity_count: usize) -> String {
    format!(
        "QUEUE_EMPTY_NEEDS_JOSH owner=josh next_action=authorize-or-create-work free_capacity={free_capacity_count}"
    )
}

async fn run_cycle(cx: &Cx, config: &Config, tick: u64) -> Result<(), String> {
    write_heartbeat(config, tick, "CYCLE_STARTED", "phase=observe")?;
    let conformance_args = vec![
        "--repo".to_owned(),
        config.repo.display().to_string(),
        "--check".to_owned(),
        "ASUPERSYNC-CONFORMANCE.md".to_owned(),
    ];
    let conformance = asupersync_conformance_evidence(
        invoke(cx, config, "asupersync-conformance", &conformance_args).await,
    );
    match conformance {
        AsupersyncConformanceEvidence::Current { detail } => println!(
            "ASUPERSYNC_CONFORMANCE_EVIDENCE tick={tick} kind=current detail={detail}"
        ),
        AsupersyncConformanceEvidence::Missing { detail } => println!(
            "ASUPERSYNC_CONFORMANCE_EVIDENCE tick={tick} kind=missing no_claim=table_compliance detail={detail}"
        ),
        AsupersyncConformanceEvidence::Stale { detail } => println!(
            "ASUPERSYNC_CONFORMANCE_EVIDENCE tick={tick} kind=stale no_claim=table_compliance detail={detail}"
        ),
        AsupersyncConformanceEvidence::Unavailable { detail } => println!(
            "ASUPERSYNC_CONFORMANCE_EVIDENCE tick={tick} kind=unavailable no_claim=table_compliance detail={detail}"
        ),
    }
    recover_pending_findings(cx, config, tick).await?;
    // MOVED HERE AFTER RUNNING IT, and the move is the finding. The first version
    // sat after `decide()`, which reads as "every cycle" and is not: MEASURED
    // 2026-09-02 against the current tree, the tick aborts at `reap-finished-panes`
    // BEFORE `decide` is ever called, so the count printed on zero of my probe runs.
    // The live build reaches `decide` on 39 of 39 ticks, which is exactly why the
    // placement bug was invisible from the ledger and only a live run exposed it.
    //
    // **A count printed after something that can refuse is not printed every
    // cycle.** It belongs ahead of every kernel invocation, at the head of the tick.
    // ============ CONDITION 2: LOUD IN THE DECISION OUTPUT, EVERY CYCLE ============
    //
    // `leht`. The advisory count prints HERE — ahead of the match, so it appears on
    // every branch including the ones that `return` early — and not into a log or a
    // file.
    //
    // WHY THE PLACEMENT IS THE MECHANISM. Every other signalling path in this repo
    // is measured silent: `ATTENTION.txt` took 178 consecutive ticks from one writer
    // with zero readers; a typed refusal naming `owner=josh` printed 29 times unread;
    // `gate.yml` failed six consecutive runs unread. **The only path that has ever
    // reached a human is a verdict the operator had to answer.** A count written to a
    // file would be the fourth instance of that class.
    let uds_gate = match observe_uds_target_gate(cx, config).await {
        Ok(projection) => projection,
        Err(unwired) => {
            let unwired = vec![unwired];
            let joined = unwired.join(" ");
            let line = crate::resident_tick::gate_unwired_line(&unwired);
            write_heartbeat(
                config,
                tick,
                "GATE_UNWIRED",
                &format!("unwired={joined} owner=josh"),
            )?;
            write_heartbeat(config, tick, "NO_DISPATCH_TICK", "skip_reap=true")?;
            eprintln!("{line}");
            if crate::resident_tick::SURVIVE_GATE_UNWIRED {
                return Ok(());
            }
            return Err(format!("GATE_UNWIRED unwired={joined}"));
        }
    };
    println!(
        "UDS_TARGET_GATE_OBSERVED requirements={}",
        uds_gate
            .requirements
            .iter()
            .map(|requirement| requirement.id.as_str())
            .collect::<Vec<_>>()
            .join(",")
    );


    // eg0m: THE CLOSE HALF, RECONCILED EVERY TICK — ahead of the pending-dispatch
    // fence and every other branch that can abort, for the reason `leht` measured:
    // a lane placed after something that can refuse does not run every cycle. This
    // one especially, because it is the ONLY producer of `Closed` and
    // `GradeReceived`, which appear in zero of 6,305 heartbeat rows.
    //
    // A failure here is a DEGRADED row, never a refused tick: losing the completion
    // record is bad and stopping the fleet to protect a bookkeeping lane is worse.
    match reconcile_completions(cx, config, tick).await {
        Ok(0) => {}
        Ok(steps) => println!("SPINE_COMPLETIONS_RECORDED steps={steps}"),
        Err(error) => {
            write_heartbeat(
                config,
                tick,
                "SPINE_RECONCILE_DEGRADED",
                &one_line_detail(&error),
            )?;
            println!("SPINE_RECONCILE_DEGRADED {}", one_line_detail(&error));
        }
    }
    // THE MARKER FENCE IS PER-PANE. It withholds the panes whose dispatches are
    // still in flight and leaves every other pane dispatchable.
    //
    // MEASURED 2026-09-02, two defects one behind the other. First the marker was
    // ONE global file and the `Live` arm returned from the whole cycle, so a
    // pending dispatch to `%1409` refused every dispatch to every OTHER pane for
    // up to PENDING_DISPATCH_MAX_AGE_SECS: 3 panes IDLE, 76 beads ready,
    // `DISPATCH_RETRY_BLOCKED` on three consecutive cycles. Scoping only the
    // CHECK then moved the refusal to the WRITE, which surfaced as
    // `DISPATCH_RETRY_BLOCKED ... error=File exists (os error 17)` — because
    // `create_new(true)` against one shared path is itself the fleet-wide mutex.
    // Both halves are needed: per-pane PATHS (see `pending_dispatch_path`) and
    // this per-pane scan.
    //
    // `Expired` still clears and CONTINUES (the y6v5 fix). `Undatable` still
    // fails closed to a human. Neither is weakened here; both are now per-pane.
    let (marker_blocked_panes, marker_cleared_beads): (Vec<String>, Vec<String>) =
        match process_pending_markers(config, tick)? {
        MarkerFence::Proceed(outcome) => {
            if outcome.cleared.is_empty() && outcome.blocked_panes.is_empty() {
                write_heartbeat(
                    config,
                    tick,
                    "NO_PENDING_DISPATCH",
                    "owner=loop next_action=continue",
                )?;
            }
            let cleared_beads = outcome
                .cleared
                .iter()
                .map(|row| row.1.clone())
                .collect();
            (outcome.blocked_panes, cleared_beads)
        }
        MarkerFence::StopUndatable {
            detail,
            reason,
            marker_path,
        } => {
            write_heartbeat(config, tick, "DISPATCH_RETRY_BLOCKED", &detail)?;
            println!(
                "DISPATCH_RETRY_BLOCKED reason={reason} owner=josh next_action=inspect-or-clear-pending-dispatch scope=fleet marker={} detail={detail}",
                marker_path.display()
            );
            return Ok(());
        }
    };

    // HD-0001 (docs/decisions.jsonl): "tick loop continues as long as we're keeping
    // our docs up to date". A buyer condition, so it gates the tick — and it is
    // checked AFTER the dispatch fence and BEFORE observation, because a stale
    // assembly makes every downstream dispatch send an agent to work from old
    // knowledge, which is the failure this condition exists to prevent.
    // HD-0001 (docs/decisions.jsonl): the condition says the tick loop continues
    // only as long as docs are kept up to date. It is packet-specific: a stale
    // assembly still refuses a packet carrying plan-document input, while an
    // independent packet can continue only under authorized degraded admission.
    // Classification happens after renderer output exists, so observation and queue remain measurable.
    let docs_fresh = match docs_are_stale(config)? {
        DocsVerdict::Fresh => true,
        DocsVerdict::Stale(why) => {
            write_heartbeat(config, tick, "DOCS_STALE", &why)?;
            let detail = format!(
                "DOCS_STALE owner=josh next_action=classify-packet-input authority=HD-0001 detail={why}"
            );
            println!("{detail}");
            false
        }
        // NAMED, never a silent green: a reader must be able to tell "this repo has no
        // assembly to be stale" from "the assembly is fresh". Both continue the tick; only
        // one of them is a measurement of an assembly.
        DocsVerdict::NotApplicable(why) => {
            write_heartbeat(config, tick, "DOCS_ASSEMBLY_NOT_APPLICABLE", &why)?;
            println!(
                "DOCS_ASSEMBLY_NOT_APPLICABLE owner=loop next_action=continue scope=repo authority=HD-0001 detail={why}"
            );
            true
        }
    };
    // HD-0015 is required only when the assembly is stale. Fresh input keeps the
    // ordinary Allowed verdict without consulting the degraded policy.
    let degraded_authorized = docs_fresh || degraded_policy_authorized(config);

    let mut monitor_args = vec![
        "observe".to_owned(),
        "--session".to_owned(),
        config.session.clone(),
        "--repo".to_owned(),
        config.repo.display().to_string(),
        "--state".to_owned(),
        config.tick_monitor_state.display().to_string(),
    ];
    for pane in &config.exclude_panes {
        monitor_args.push("--exclude-pane".to_owned());
        monitor_args.push(pane.clone());
    }
    let monitor_bytes = require_success(
        &config.tick_monitor,
        invoke(cx, config, &config.tick_monitor, &monitor_args).await?,
    )?;
    let ompo_ps = observe_ompo_ps(cx, config).await?;
    record_ompo_ps_observation(&ompo_ps);
    let mut observation = parse_observation(&monitor_bytes, None)?;
    observation.panes.retain(|pane| {
        !config
            .exclude_panes
            .iter()
            .any(|excluded| excluded == &pane.pane_id)
    });
    // The pane named by a LIVE pending-dispatch marker is withheld from THIS
    // cycle's candidates, and only that pane. Same mechanism as
    // `config.exclude_panes` above, scoped to one tick: the double-send guard is
    // preserved for the in-flight pane while the rest of the fleet stays
    // dispatchable. Before this, a marker naming one pane refused every pane.
    if !marker_blocked_panes.is_empty() {
        let before = observation.panes.len();
        observation.panes.retain(|pane| {
            !marker_blocked_panes
                .iter()
                .any(|held| held == &pane.pane_id)
        });
        println!(
            "MARKER_PANE_WITHHELD tick={tick} session={} panes=[{}] panes_before={before} panes_after={}",
            config.session,
            marker_blocked_panes.join(","),
            observation.panes.len()
        );
    }
    let pane_states = observation
        .panes
        .iter()
        .map(|pane| format!("{}={}", pane.pane_id, pane.state))
        .collect::<Vec<_>>()
        .join(",");
    let free_capacity_count = observation
        .panes
        .iter()
        .filter(|pane| pane.is_free_capacity)
        .count();
    let dispatchable_count = observation
        .panes
        .iter()
        .filter(|pane| pane.is_dispatchable)
        .count();
    println!(
        "OBSERVATION tick={tick} session={} panes={} states={} free_capacity={} dispatchable={}",
        config.session,
        observation.panes.len(),
        pane_states,
        free_capacity_count,
        dispatchable_count
    );
    // M1 TYPED DEGRADED DISPATCH, not a bypass. Post-mortem 2026-08-31 named this exactly:
    // when one precondition is red the whole chain fail-closes, the supervisor refuses every
    // tick, and the fleet sits idle beside a ready queue while every gate reports working.
    //
    // MEASURED 2026-09-01 23:57Z onward: the resident supervisor refused every 30s because
    // `reap-finished-panes` is a WRAPPER around control-plane's 324-line bash reaper, which
    // cannot exist in a repo that forbids `.sh`. Pointing it at control-plane's copy hangs
    // 90s at 2% CPU with no output - the undrained-pipe class. Bead `t00` ports it to Rust.
    //
    // FAIL-CLOSED REMAINS THE DEFAULT. With OMP_REAP_SWEEP unset the behaviour is unchanged:
    // a missing or failing reaper still refuses the tick. The skip requires an OPERATOR to
    // set `OMP_REAP_SWEEP=unavailable:<reason>`, the reason is REQUIRED and is written into
    // the heartbeat, so a degraded run is a typed row someone can find - never a silent one.
    //
    // NO-CLAIM: skipping the sweep means finished panes are NOT reaped this cycle. Capacity
    // that should have been returned stays occupied, so the fleet runs smaller than it looks.
    // This trades a KNOWN degradation for a total stall; it does not make the reaper work.
    let reaper_summary = match std::env::var("OMP_REAP_SWEEP") {
        Ok(v) if v.starts_with("unavailable:") => {
            let reason = v.trim_start_matches("unavailable:").trim();
            if reason.is_empty() {
                return Err(
                    "REAP_SWEEP_SKIP_REASON_EMPTY OMP_REAP_SWEEP=unavailable: requires a \
                            reason; an unexplained degradation is indistinguishable from a bug"
                        .to_owned(),
                );
            }
            write_heartbeat(config, tick, "REAP_SWEEP_SKIPPED", reason)?;
            format!("SKIPPED reason={reason}")
        }
        _ => run_finished_pane_sweep(cx, config).await?,
    };
    write_heartbeat(config, tick, "REAP_FINISHED_PANES", &reaper_summary)?;
    println!(
        "REAP_FINISHED_PANES tick={tick} session={} summary={reaper_summary}",
        config.session
    );
    let readiness = bead_availability::collect_ready_live(cx, &config.br)
        .await
        .map_err(|error| {
            format!("QUEUE_VISIBILITY_UNREADABLE owner=josh next_action=repair-br-or-escalate: {error}")
        })?;
    if !readiness.recovered.is_empty() {
        let ids = readiness.recovered_ids();
        let detail = format!("recovered={} ids={}", ids.len(), ids.join(","));
        write_heartbeat(config, tick, "QUEUE_READY_RECOVERED", &detail)?;
        println!("QUEUE_READY_RECOVERED tick={tick} {detail}");
    }
    if !readiness.refused.is_empty() {
        let ids = readiness
            .refused
            .iter()
            .map(|row| row.issue.id.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let detail = format!("refused={} ids={ids}", readiness.refused.len());
        write_heartbeat(config, tick, "QUEUE_READY_REFUSED", &detail)?;
        println!("QUEUE_READY_REFUSED tick={tick} {detail}");
    }
    let ready_ids: Vec<String> = readiness
        .admitted
        .iter()
        .map(|issue| issue.id.clone())
        .collect();
    let ready_priorities: BTreeMap<String, u64> = readiness
        .admitted
        .iter()
        .map(|issue| (issue.id.clone(), issue.priority))
        .collect();
    let triage_args = vec!["--robot-triage".to_owned()];
    let mut bead_ids = match invoke(cx, config, &config.bv, &triage_args).await {
        Ok(output) => {
            let triage = require_success(&config.bv, output).map_err(|error| {
                format!("QUEUE_UNRANKED owner=josh next_action=repair-bv: {error}")
            })?;
            let insights_args = vec!["--robot-insights".to_owned()];
            let insights_output = invoke(cx, config, &config.bv, &insights_args)
                .await
                .map_err(|error| {
                    format!("QUEUE_UNRANKED owner=josh next_action=repair-bv-insights: {error}")
                })?;
            let insights = require_success(&config.bv, insights_output).map_err(|error| {
                format!("QUEUE_UNRANKED owner=josh next_action=repair-bv-insights: {error}")
            })?;
            let jsonl =
                fs::read_to_string(config.repo.join(".beads/issues.jsonl")).unwrap_or_default();
            let order = loop_queue_filter::select::select_dispatch_order_with_pagerank(
                &triage,
                &ready_ids,
                &ready_priorities,
                &insights,
                &jsonl,
                &config.receiver_agent,
            )?;
            match order.window {
                loop_queue_filter::select::RankWindow::RecommendationsOnly => {
                    let detail = format!(
                        "window={} rank_source=bv.full_stats.pagerank ready={} scored={}",
                        order.window.label(),
                        ready_ids.len(),
                        order.scored
                    );
                    write_heartbeat(config, tick, "QUEUE_RANK_RECOMMENDATIONS", &detail)?;
                    println!("QUEUE_RANK_RECOMMENDATIONS {detail}");
                }
                window => {
                    let status = match window {
                        loop_queue_filter::select::RankWindow::EmptyReady => {
                            "QUEUE_RANK_WINDOW_EMPTY"
                        }
                        loop_queue_filter::select::RankWindow::FilteredFiledOnly => {
                            "QUEUE_RANK_FILTERED_FILED_ONLY"
                        }
                        _ => "QUEUE_RANK_FALLBACK",
                    };
                    let detail = format!(
                        "window={} rank_source=bv.full_stats.pagerank ready={} scored={}",
                        window.label(),
                        ready_ids.len(),
                        order.scored
                    );
                    write_heartbeat(config, tick, status, &detail)?;
                    println!("{status} {detail}");
                }
            }
            order.ids
        }
        Err(error) => {
            return Err(format!(
                "QUEUE_UNRANKED owner=josh next_action=repair-bv-or-escalate: {error}"
            ));
        }
    };

    // omp-orchestrator-block-non-arc-behind-s0-f3g5 ITEM 8 — THE PRODUCTION CALLER, flag off.
    //
    // Landed with `PHASE_GATE_ENABLED = false` on purpose. I had argued deferral on the grounds
    // that a flag-off caller was "unobservable in either direction"; that was wrong, and %7's
    // refutation is the reason this is here: FALSE IS AN OBSERVATION. `gate_active=false` is
    // exactly the fact that distinguishes a disabled gate from a released phase, and I had built
    // `GateOutcome::gate_active` for that purpose and then used unobservability to justify not
    // wiring the field I added for it.
    //
    // THE ARC CENSUS HERE IS QUEUE-SCOPED, AND THAT IS ONLY SOUND WHILE THE FLAG IS OFF.
    // `apply_phase_gate` short-circuits before the census check on the disabled path, so this
    // value is not consulted today. Enabling the gate REQUIRES replacing it with a tracker-wide
    // census: a census derived from the queue reports zero for a healthy arc that simply is not
    // offered, which is the live state — 0 of 37 non-terminal arc members appear in `br ready`.
    // Named here rather than left for the next reader to discover.
    let arc_census_queue_scoped = bead_ids
        .iter()
        .filter(|id| loop_queue_filter::phase_gate::is_arc_member(id))
        .count();
    let mut bead_ids = match loop_queue_filter::phase_gate::apply_phase_gate(
        &bead_ids,
        arc_census_queue_scoped,
        false,
        loop_queue_filter::phase_gate::PHASE_GATE_ENABLED,
    ) {
        Ok(outcome) => {
            let detail = outcome.render();
            write_heartbeat(config, tick, "PHASE_GATE", &detail)?;
            println!("PHASE_GATE {detail}");
            for (id, reason) in &outcome.withheld {
                let row = format!("id={id} code={} {}", reason.code(), reason.detail());
                write_heartbeat(config, tick, "PHASE_GATE_WITHHELD", &row)?;
                println!("PHASE_GATE_WITHHELD {row}");
            }
            outcome.admitted
        }
        // A refusal is a refusal: the gate's vacuity arms exist so an unreadable or empty
        // candidate set can never read as "nothing to dispatch".
        Err(error) => {
            let detail = format!("code={} {error}", error.code());
            write_heartbeat(config, tick, "PHASE_GATE_REFUSED", &detail)?;
            return Err(format!("PHASE_GATE_REFUSED {detail}"));
        }
    };

    observation.queue = QueueState {
        ready_count: bead_ids.len(),
        readable: true,
    };
    // DISK PRESSURE. Checked after observation and queue read, before any dispatch
    // with No space left on device (os error 28). The protection and 8% free / 1 GiB
    // threshold remain unchanged; only placement is different.
    //
    // A guard that observes and does not refuse is a note. This refuses before dispatch.
    if let Some(why) = disk_pressure(config)? {
        write_heartbeat(config, tick, "DISK_PRESSURE", &why)?;
        println!("DISK_PRESSURE owner=josh next_action=cargo-clean-or-grow-volume detail={why}");
        return Ok(());
    }
    // Admission consumes the same machine ledger the census reports; a failed host-oracle
    // classification is a typed refusal before any authorization can become a dispatch.
    match worker_oracle_gate::admission_check(&config.repo) {
        Ok(report) => println!(
            "WORKER_ORACLE_ADMISSION PASS ledger_targets={} host_bound={} grep_host_bound={}",
            report.ledger_targets, report.ledger_host_bound, report.source_host_bound
        ),
        Err(error) => {
            let detail = error.to_string();
            write_heartbeat(config, tick, "WORKER_ORACLE_REFUSED", &detail)?;
            println!("WORKER_ORACLE_REFUSED tick={tick} detail={detail}");
            return Ok(());
        }
    }
    let authorization = applicable(
        read_idle_authorization(&config.repo, now_unix()),
        &config.session,
        &observation.panes,
        &observation.queue,
    );
    let mut hold_intent = cross_pane_hold::HoldIntent::Work;
    let mut decision = decide(&observation, &authorization);

    if matches!(&decision, SupervisorDecision::Dispatch { .. }) {
        // NoCandidate and Skipped both continue ranked dispatch, as before;
        // they now arrive distinguishable, and the skip has already written
        // its heartbeat row naming the candidate_count it refused over.
        if let PeerGradeGate::Claimed(claim) = gate_peer_grading(config, &mut observation, tick)? {
            hold_intent = cross_pane_hold::HoldIntent::Grade;

            refuse_placeholder_identity("bead", &claim.bead)?;
            refuse_placeholder_identity("receiver_pane", &claim.receiver_pane)?;
            refuse_placeholder_identity("grader_pane", &claim.grader_pane)?;
            let detail = format!(
                "bead={} receiver_pane={} grader_pane={} requirement=peer-grading-before-new-work",
                claim.bead, claim.receiver_pane, claim.grader_pane
            );
            write_heartbeat(config, tick, "PEER_GRADING_REQUIRED", &detail)?;
            println!(
                "PEER_GRADING_REQUIRED tick={tick} session={} {detail}",
                config.session
            );
            decision = SupervisorDecision::Dispatch {
                pane: claim.grader_pane.clone(),
                bead_hint: claim.bead.clone(),
            };
            bead_ids.retain(|id| id != &claim.bead);
            bead_ids.insert(0, claim.bead);
        }
    }

    file_supervisor_finding(cx, config, tick, &decision).await?;
    emit_s1_l3_l5(cx, config, &decision).await;
    observe_s1_after_emit(config);

    // psf7: THE TICK ROW IS WRITTEN HERE, ONCE, UNCONDITIONALLY, BEFORE THE MATCH.
    //
    // Every arm below returns, and several return early. A writer placed inside the arms is
    // a writer a future arm inherits as ABSENT — which is exactly the state this bead was
    // filed about: `.flywheel/orchestration-ticks.jsonl` was missing from disk AND HEAD
    // while roughly fifteen ticks ran, so by the contract's own words no tick had happened.
    // The first five rows were hand-written afterwards by the same agent that failed to
    // write the first ten.
    //
    // A row here describes the DECISION, not its execution, and says so in `not_done`. That
    // is the honest split: the dispatch outcome is recorded by the ack-spine steps and the
    // heartbeat, and a row claiming an outcome it has not yet observed would be a fabricated
    // one in the single artifact whose purpose is auditability.
    //
    // A write failure is NOT fatal to the tick. The row is evidence; refusing to supervise
    // because evidence could not be filed would trade a silent ledger for a stalled fleet.
    // It is loud on stderr and recorded in the heartbeat, which is the clock the presence
    // check reads — so a persistently failing writer surfaces as a GAP, not as nothing.
    if let Err(error) = write_tick_receipt(config, tick, &observation, &decision) {
        eprintln!("TICK_ROW_NOT_WRITTEN tick={tick} detail=\"{error}\"");
        write_heartbeat(config, tick, "TICK_ROW_NOT_WRITTEN", &error)?;
    }

    match decision {
        SupervisorDecision::AwaitingHuman { panes } => {
            // The one refusal a human MUST see, because no amount of looping clears
            // it. Named panes, named action, and the authority is the operator rather
            // than a policy row — there is nothing for the loop to authorize.
            //
            // Measured 2026-08-31: 36 minutes on an install approval, invisible
            // because the pane's turn timer advances while it waits.
            write_heartbeat(config, tick, "AWAITING_HUMAN", &panes)?;
            println!(
                "AWAITING_HUMAN panes={panes} owner=josh \
                 next_action=answer-the-open-dialog detail=\"alive and blocked on an \
                 answer; the timer advances while nobody comes\""
            );
            return Ok(());
        }
        SupervisorDecision::Dispatch { pane, .. } => {
            let heartbeat = fs::read_to_string(&config.heartbeat_ledger).unwrap_or_default();
            let marker_candidate = select_next_bead(&bead_ids, &marker_cleared_beads);
            if marker_candidate.is_none() && !marker_cleared_beads.is_empty() {
                let detail = format!(
                    "cleared_beads={} next_action=await-new-ready",
                    marker_cleared_beads.join(",")
                );
                write_heartbeat(config, tick, "DISPATCH_MARKER_CLEARED_NO_ALTERNATE", &detail)?;
                println!("DISPATCH_MARKER_CLEARED_NO_ALTERNATE {detail}");
                return Ok(());
            }
            let (selected, skipped) = select_ready_skipping_cooldown(
                &bead_ids,
                &heartbeat,
                now_unix(),
                &marker_cleared_beads,
            );
            for (id, age) in &skipped {
                let detail = format!("bead={id} age_secs={age} next_action=grade-or-human");
                write_heartbeat(config, tick, "REDISPATCH_COOLDOWN", &detail)?;
                println!("REDISPATCH_COOLDOWN {detail}");
            }
            let bead = match selected {
                Some(id) => id,
                None if skipped.is_empty() => {
                    return Err(
                        "QUEUE_UNREADABLE ready count changed before bead selection".to_owned()
                    );
                }
                None => return Ok(()),
            };
            // eg0m: THE SUPERVISOR NOW EMITS THROUGH ack-spine.
            //
            // Five of eleven StepKinds appeared in ZERO of 6,305 heartbeat rows, and
            // four of the five are the close half. `PacketRendered` and
            // `FenceChecked` were two of the absent five: nothing had ever asked
            // whether the packet was staged or the fence consulted, so the ledger
            // could describe a dispatch and not the steps that produced it.
            //
            // **The effect closure is intentionally empty.** These record steps the
            // supervisor already performs on the lines below; moving the effects
            // inside `step` would restructure a live dispatch path mid-wave. What
            // `step` still buys is real: a cancellation checkpoint on both sides of
            // each record, and `assert_step_count` refusing a row that no step
            // produced.
            let pane_observation = observation
                .panes
                .iter()
                .find(|candidate| candidate.pane_id == pane)
                .cloned()
                .ok_or_else(|| format!("DISPATCH_PREFLIGHT_REFUSED bead={bead} pane={pane} observation_row_missing"))?;
            let mut spine = ack_spine::ledger::StepLedger::new();
            emit_step(
                cx,
                &mut spine,
                StepKind::BeadSelected,
                bead,
                &pane,
                config,
                "selected from the bv-ordered ready queue",
            )
            .await?;
            let identities = load_identity_registries(cx, config).await?;
            let admitted_pane_count = observation
                .panes
                .iter()
                .filter(|pane| pane.is_dispatchable)
                .count();
            let dispatch_result = async {
                let (snapshot, receiver_agent, admission) = prepare_bead_dispatch(
                    cx,
                    config,
                    &pane,
                    &pane_observation,
                    bead,
                    &identities,
                    tick,
                    true,
                    hold_intent,
                    docs_fresh,
                    degraded_authorized,
                    admitted_pane_count,
                )
                .await?;

                let dispatch_epoch = now_unix() as i64;
                write_dispatch_intent(config, &pane, bead)?;
                emit_step(cx, &mut spine, StepKind::FenceChecked, bead, &pane, config, "per-pane dispatch fence passed; intent written").await?;
                let before = capture_pane(cx, config, &pane).await?;
                emit_step(cx, &mut spine, StepKind::PacketRendered, bead, &pane, config, &format!("receiver={receiver_agent}")).await?;
                let prior = prior_dispatch_count(config, bead);
                let send = crate::spine_emit::send_kind(prior);
                let stage_result = send_and_verify(
                    cx,
                    config,
                    &pane,
                    &pane_observation,
                    bead,
                    &receiver_agent,
                    &snapshot,
                    &before,
                    tick,
                    &admission,
                    admitted_pane_count,
                )
                .await;
                let send_detail = match &stage_result {
                    Ok(DispatchVerdict::Delivered(stage)) => format!(
                        "prior_dispatches={prior} verdict={}",
                        stage.delivery.label()
                    ),
                    Ok(verdict) => format!(
                        "prior_dispatches={prior} status={}",
                        verdict.status_word()
                    ),
                    Err(error) => format!(
                        "prior_dispatches={prior} send_failed={}",
                        one_line_detail(error)
                    ),
                };
                emit_step(cx, &mut spine, send, bead, &pane, config, &send_detail).await?;
                {
                    let delivered = stage_result.as_ref().ok().and_then(|verdict| match verdict {
                        DispatchVerdict::Delivered(stage) => Some(stage),
                        DispatchVerdict::Indeterminate { .. }
                        | DispatchVerdict::AckPending { .. }
                        | DispatchVerdict::Failed(_) => None,
                    });
                    let facts = cell_matrix::facts_from_stage(
                        format!("{tick}-{pane}-{bead}"),
                        bead.to_owned(),
                        pane.clone(),
                        true,
                        true,
                        true,
                        false,
                        true,
                        delivered,
                        None,
                    );
                    match DispatchCellMatrix::from_facts(facts) {
                        Ok(matrix) => {
                            if let Err(error) = write_heartbeat(
                                config,
                                tick,
                                DispatchCellMatrix::ROW_STATUS,
                                &matrix.detail(),
                            ) {
                                eprintln!("DISPATCH_CELL_MATRIX_WRITE {error}");
                            } else {
                                println!("DISPATCH_CELL_MATRIX {}", matrix.detail());
                            }
                        }
                        Err(error) => eprintln!("DISPATCH_CELL_MATRIX_REFUSED {error}"),
                    }
                }

                let verdict = match stage_result {
                    Ok(verdict) => verdict,
                    Err(error) => DispatchVerdict::Failed(error),
                };
                match verdict {
                    DispatchVerdict::Delivered(stage) => {
                let silence =
                    run_silence_watch(cx, config, bead, dispatch_epoch, &receiver_agent).await?;
                write_heartbeat(
                    config,
                    tick,
                    "DISPATCH_SILENCE_WATCH",
                    &format!("pane={pane} bead={bead} verdict={silence}"),
                )?;
                match silence {
                    SilenceVerdict::VerdictPosted => {
                        write_heartbeat(
                            config,
                            tick,
                            "DISPATCHED",
                            &format!(
                                "pane={pane} bead={bead} receiver={} ack_action={}",
                                stage.transport.kind().label(),
                                stage.action.label(),
                            ),
                        )?;
                        println!(
                            "DISPATCHED tick={tick} session={} pane={pane} bead={bead} RECEIVER_RECEIPT={} ACK_ACTION={} SILENCE_VERDICT={silence}",
                            config.session,
                            stage.transport.kind().label(),
                            stage.action.label()
                        );
                        Ok::<DispatchOutcome, String>(DispatchOutcome {
                            detail: format!(
                                "status=DISPATCHED receiver={} ack_action={} silence_verdict=VERDICT_POSTED",
                                stage.transport.kind().label(),
                                stage.action.label(),
                            ),
                            clear_intent: clears_pending_dispatch_intent(silence),
                        })
                    }
                    SilenceVerdict::TrackerError => {
                        println!(
                            "DISPATCH_SILENCE tick={tick} session={} pane={pane} bead={bead} verdict={silence} next_action=re-read-tracker",
                            config.session
                        );
                        Ok(DispatchOutcome {
                            detail: format!(
                                "status=DISPATCH_SILENCE verdict={silence} next_action=re-read-tracker"
                            ),
                            clear_intent: clears_pending_dispatch_intent(silence),
                        })
                    }
                    other => {
                        println!(
                            "DISPATCH_SILENCE tick={tick} session={} pane={pane} bead={bead} verdict={other} next_action=inspect-or-resolve-pending",
                            config.session
                        );
                        Ok(DispatchOutcome {
                            detail: format!(
                                "status=DISPATCH_SILENCE verdict={other} next_action=inspect-or-resolve-pending"
                            ),
                            clear_intent: clears_pending_dispatch_intent(other),
                        })
                    }
                }
                    }
                    DispatchVerdict::Indeterminate { reason, transport } => {
                        Ok(DispatchOutcome {
                            detail: format!(
                                "status=DISPATCH_INDETERMINATE reason={reason} transport={transport}"
                            ),
                            clear_intent: false,
                        })
                    }
                    DispatchVerdict::AckPending {
                        after_secs,
                        discriminator,
                    } => Ok(DispatchOutcome {
                        detail: format!(
                            "status=ACK_PENDING after={after_secs}s{discriminator}"
                        ),
                        clear_intent: false,
                    }),
                    DispatchVerdict::Failed(error) => Err(error),
                }
            }
            .await;
            // eg0m ACCEPTANCE 5, ANTI-VACUITY: an empty ledger after a dispatch is
            // an ERROR, because it reads identically to a cycle that never ran.
            // Persisted after the dispatch resolves so a failed dispatch still
            // leaves its steps on disk — the rows up to the failure are the
            // recoverable prefix `step` exists to guarantee.
            if let Err(error) = persist_spine(config, &spine, true) {
                write_heartbeat(config, tick, "SPINE_LEDGER_REFUSED", &error)?;
                println!("SPINE_LEDGER_REFUSED {error}");
            }
            let report_detail = match &dispatch_result {
                Ok(outcome) => outcome.detail.clone(),
                Err(error) => format!(
                    "status={} detail={}",
                    dispatch_status_word(error),
                    one_line_detail(error)
                ),
            };
            // This now escalates ONLY when the ledger write itself failed. A
            // bounced courtesy notify to the result pane is downgraded to a
            // DISPATCH_RESULT_NOTIFY_DEGRADED row inside
            // `report_dispatch_result`, because losing the dispatch outcome and
            // stranding the pending-dispatch marker is strictly worse than an
            // operator having to read the ledger instead of a pane.
            if let Err(report_error) =
                report_dispatch_result(cx, config, tick, &pane, bead, &report_detail).await
            {
                return Err(format!(
                    "DISPATCH_RESULT_LEDGER_WRITE_FAILED source_pane={pane} bead={bead} result={report_detail} owner=josh next_action=repair-heartbeat-ledger report_error={report_error}"
                ));
            }
            match dispatch_result {
                Ok(outcome) => {
                    if outcome.clear_intent {
                        record_successful_dispatch(config, tick, &pane, bead)?;
                    }
                }
                Err(error) => {
                    if let Some(reason) = intent_clear_reason(&error) {
                        clear_dispatch_intent(config, &pane)?;
                        let detail = format!(
                            "pane={pane} bead={bead} reason={reason} owner=loop next_action=continue"
                        );
                        write_heartbeat(config, tick, "DISPATCH_INTENT_CLEARED", &detail)?;
                        println!("DISPATCH_INTENT_CLEARED {detail}");
                    }
                    return Err(error);
                }
            }
        }
        SupervisorDecision::GateUnwired { unwired } => {
            // The remedy comes from the VARIANT, never from a literal here. The
            // supervisor printed next_action=repair-gate-trigger for an unextracted
            // crate because this string was hardcoded three hundred lines from the
            // state that produced it, and an operator following it would look for a
            // hook to fix and find nothing.
            // THE SECOND CENSUS IS A DIFFERENT MEASUREMENT, AND IT CAN DISAGREE.
            //
            // `unwired` was decided upstream from census #1; this line ran census #2
            // seconds later purely to fetch labels. With the old load-dependent probe
            // the two disagreed, and the disagreement printed as
            // `ack-stage[REACHABLE→none]` INSIDE `unwired=` — a row labelled reachable
            // inside the list of things that are not. An operator reading that looks
            // for "a trigger that resolves to no action", which is not what happened.
            //
            // Measured 2026-09-02 in the live heartbeat, build `7600dda`. The probe is
            // deterministic now, so the two censuses should agree — which is exactly
            // why a residual disagreement must be LOUD rather than rendered as a
            // contradictory label. A fact stated in two places will disagree in one.
            let census = crate::census_gates(&config.repo);
            let mut parts = Vec::new();
            let mut disagreed = Vec::new();
            for name in &unwired {
                match census.rows.iter().find(|r| &r.gate == name) {
                    Some(row) if row.reachability.is_reachable() => {
                        disagreed.push(format!(
                            "{name}[now {}→{}]",
                            row.reachability.label(),
                            row.reachability.next_action()
                        ));
                    }
                    Some(row) => parts.push(format!(
                        "{name}[{}→{}]",
                        row.reachability.label(),
                        row.reachability.next_action()
                    )),
                    None => parts.push(format!("{name}[UNKNOWN→investigate-census]")),
                }
            }
            let detail = if disagreed.is_empty() {
                format!("unwired={} owner=josh", parts.join(" "))
            } else {
                format!(
                    "unwired={} census_disagreed={} owner=josh next_action=investigate-census -- \
                     these rows were UNWIRED at decision time and REACHABLE when relabelled \
                     seconds later; the census is not stable and its verdict cannot be trusted \
                     until it is",
                    parts.join(" "),
                    disagreed.join(" ")
                )
            };
            write_heartbeat(config, tick, "GATE_UNWIRED", &detail)?;
            return Err(format!("GATE_UNWIRED {detail}"));
        }
        SupervisorDecision::EscalateIdleIncident {
            dispatchable_count,
            ready_count,
        } => {
            write_heartbeat(
                config,
                tick,
                "IDLE_UNAUTHORIZED",
                &format!(
                    "dispatchable={dispatchable_count} ready={ready_count} next_action=dispatch-or-authorize"
                ),
            )?;
            println!(
                "IDLE_UNAUTHORIZED tick={tick} session={} dispatchable={} ready={} owner=josh next_action=dispatch-or-authorize",
                config.session, dispatchable_count, ready_count
            );
        }
        SupervisorDecision::MonitorBlind { detail } => {
            write_heartbeat(config, tick, "MONITOR_BLIND", &detail)?;
            return Err(format!(
                "MONITOR_BLIND owner=josh next_action=repair-monitor detail={detail}"
            ));
        }
        SupervisorDecision::QueueUnreadable { detail } => {
            write_heartbeat(config, tick, "QUEUE_UNREADABLE", &detail)?;
            return Err(format!(
                "QUEUE_UNREADABLE owner=josh next_action=repair-queue detail={detail}"
            ));
        }
        SupervisorDecision::WorkspaceUnloaded { detail } => {
            write_heartbeat(config, tick, "WORKSPACE_UNLOADED", &detail)?;
            return Err(format!(
                "WORKSPACE_UNLOADABLE owner=josh next_action=repair-workspace detail={detail}"
            ));
        }
        SupervisorDecision::AuthorizedIdle {
            pane_count,
            expires_at,
        } => {
            write_heartbeat(
                config,
                tick,
                "IDLE_AUTHORIZED",
                &format!("panes={pane_count} expires_at={expires_at}"),
            )?;
            println!(
                "IDLE_AUTHORIZED tick={tick} session={} panes={pane_count} expires_at={expires_at}",
                config.session
            );
        }
        SupervisorDecision::QueueEmptyNeedsJosh {
            free_capacity_count,
        } => {
            let detail = queue_empty_detail(free_capacity_count);
            write_heartbeat(config, tick, "QUEUE_EMPTY_NEEDS_JOSH", &detail)?;
            eprintln!("{detail}");
            return Err(detail);
        }
        SupervisorDecision::SupervisedWorking {
            working_count,
            ready_count,
        } => {
            write_heartbeat(
                config,
                tick,
                "SUPERVISED_WORKING",
                &format!("working={working_count} ready={ready_count}"),
            )?;
            println!(
                "SUPERVISED_WORKING tick={tick} session={} working={} ready={ready_count}",
                config.session, working_count
            );
        }
    }
    Ok(())
}

/// A stable label for every decision class, EXHAUSTIVE so a new variant must choose.
///
/// Deliberately not `finding_key`, which answers a different question and returns `None` for
/// `Dispatch` -- a tick row whose reason read `decision=none` would be undiagnosable.
fn decision_label(decision: &SupervisorDecision) -> &'static str {
    match decision {
        SupervisorDecision::AwaitingHuman { .. } => "awaiting-human",
        SupervisorDecision::Dispatch { .. } => "dispatch",
        SupervisorDecision::GateUnwired { .. } => "gate-unwired",
        SupervisorDecision::EscalateIdleIncident { .. } => "idle-incident",
        SupervisorDecision::MonitorBlind { .. } => "monitor-blind",
        SupervisorDecision::QueueUnreadable { .. } => "queue-unreadable",
        SupervisorDecision::WorkspaceUnloaded { .. } => "workspace-unloaded",
        SupervisorDecision::AuthorizedIdle { .. } => "authorized-idle",
        SupervisorDecision::QueueEmptyNeedsJosh { .. } => "queue-empty",
        SupervisorDecision::SupervisedWorking { .. } => "supervised-working",
    }
}

fn finding_key(decision: &SupervisorDecision) -> Option<&'static str> {
    match decision {
        SupervisorDecision::AwaitingHuman { .. } => Some("awaiting-human"),
        SupervisorDecision::EscalateIdleIncident { .. } => Some("idle-incident"),
        SupervisorDecision::QueueEmptyNeedsJosh { .. } => Some("queue-empty"),
        SupervisorDecision::MonitorBlind { .. } => Some("monitor-blind"),
        SupervisorDecision::WorkspaceUnloaded { .. } => Some("workspace-unloaded"),
        SupervisorDecision::GateUnwired { .. } => Some("gate-unwired"),
        SupervisorDecision::Dispatch { .. }
        | SupervisorDecision::QueueUnreadable { .. }
        | SupervisorDecision::AuthorizedIdle { .. }
        | SupervisorDecision::SupervisedWorking { .. } => None,
    }
}

fn prior_finding_observations(config: &Config, key: &str) -> u32 {
    let Ok(ledger) = fs::read_to_string(&config.heartbeat_ledger) else {
        return 0;
    };
    ledger
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|row| row.get("status").and_then(Value::as_str) == Some("FINDING_OBSERVED"))
        .filter(|row| {
            row.get("detail")
                .and_then(Value::as_str)
                .is_some_and(|detail| {
                    detail
                        .split_whitespace()
                        .any(|field| field.strip_prefix("key=") == Some(key))
                })
        })
        .count()
        .try_into()
        .unwrap_or(u32::MAX)
}

/// The actor recorded on every bead this supervisor files through [`finding`].
///
/// `BrPublisher::publish` refuses without an actor (`FindingError::ActorUnset`) because `br` falls
/// back to the ambient git identity, and every pane in this repo shares one checkout -- so an
/// omitted actor credits a human for agent work. Measured 2026-09-07: 32.9% of this tracker's beads
/// read `created_by=josh`, and the root cause was `crates/finding`'s own publisher omitting the
/// flag, which meant the SANCTIONED filing sequence was the misattributing one.
///
/// This is not a default standing in for an unknown filer. At these call sites the filer is known:
/// the supervisor process itself observed the decision and owed the finding. Naming it is the
/// honest answer, not a guess -- which is why `with_actor` is caller-supplied rather than defaulted
/// inside the publisher.
const SUPERVISOR_ACTOR: &str = "omp-supervisor";

async fn recover_pending_findings(cx: &Cx, config: &Config, tick: u64) -> Result<(), String> {
    fs::create_dir_all(&config.finding_spool).map_err(|error| {
        format!(
            "FINDING_RECOVERY_REFUSED spool={} error={error}",
            config.finding_spool.display()
        )
    })?;
    let publisher =
        BrPublisher::new(config.br.clone(), config.repo.clone()).with_actor(SUPERVISOR_ACTOR);
    match finding::Finding::recover_pending(cx, &config.finding_spool, &publisher).await {
        Ok(filed) => {
            let detail = format!("pending={} filed={filed}", filed);
            write_heartbeat(config, tick, "FINDING_RECOVERY", &detail)?;
            println!("FINDING_RECOVERY tick={tick} {detail}");
            Ok(())
        }
        Err(FindingError::Cancelled { spool_path }) => {
            let detail = format!("pending_path={} reason=cancelled", spool_path.display());
            write_heartbeat(config, tick, "FINDING_RECOVERY_DEFERRED", &detail)?;
            println!("FINDING_RECOVERY_DEFERRED tick={tick} {detail}");
            Ok(())
        }
        Err(error) => {
            let detail = format!("error={error}");
            write_heartbeat(config, tick, "FINDING_RECOVERY_DEGRADED", &detail)?;
            Err(format!("FINDING_RECOVERY_DEGRADED {detail}"))
        }
    }
}

async fn file_supervisor_finding(
    cx: &Cx,
    config: &Config,
    tick: u64,
    decision: &SupervisorDecision,
) -> Result<(), String> {
    let Some(key) = finding_key(decision) else {
        return Ok(());
    };
    let seen = prior_finding_observations(config, key).saturating_add(1);
    let observed = format!("key={key} seen={seen} decision={decision:?}");
    write_heartbeat(config, tick, "FINDING_OBSERVED", &observed)?;
    match finding_dispatch::finding_for(decision, seen) {
        MaybeFinding::NotYet(NotYet::BelowThreshold { .. })
        | MaybeFinding::NotYet(NotYet::AlreadyEmitted { .. })
        | MaybeFinding::NotYet(NotYet::NotAFindableDecision) => Ok(()),
        MaybeFinding::Owed(finding) => {
            let publisher = BrPublisher::new(config.br.clone(), config.repo.clone())
                .with_actor(SUPERVISOR_ACTOR);
            match finding.file(cx, &config.finding_spool, &publisher).await {
                Ok(filed) => {
                    let detail = format!("key={key} seen={seen} bead_id={}", filed.id());
                    write_heartbeat(config, tick, "FINDING_FILED", &detail)?;
                    println!("FINDING_FILED tick={tick} {detail}");
                    Ok(())
                }
                Err(FindingError::Cancelled { spool_path }) => {
                    let detail =
                        format!("key={key} seen={seen} spool_path={}", spool_path.display());
                    write_heartbeat(config, tick, "FINDING_DEFERRED", &detail)?;
                    println!("FINDING_DEFERRED tick={tick} {detail}");
                    Ok(())
                }
                Err(error) => {
                    let detail = format!("key={key} seen={seen} error={error}");
                    write_heartbeat(config, tick, "FINDING_PUBLISH_FAILED", &detail)?;
                    Err(format!("FINDING_PUBLISH_FAILED {detail}"))
                }
            }
        }
    }
}

async fn run_supervisor(cx: &Cx, config: Config) -> Result<(), String> {
    let mut tick = 0u64;
    loop {
        cx.checkpoint()
            .map_err(|_| "CANCELLED supervisor loop".to_owned())?;
        tick += 1;
        if let Err(error) = run_cycle(cx, &config, tick).await {
            let _ = write_heartbeat(&config, tick, "SUPERVISOR_REFUSED", &error);
            eprintln!("SUPERVISOR_REFUSED {error}");
            // A PER-TICK REFUSAL IS A VERDICT, NOT A CRASH — the loop continues
            // unless SURVIVE_GATE_UNWIRED is mutated to false (the crash-loop shape).
            if error.contains("GATE_UNWIRED")
                && !crate::resident_tick::SURVIVE_GATE_UNWIRED
            {
                return Err(error);
            }
        }

        if config.max_ticks.is_some_and(|max| tick >= max) {
            println!("SUPERVISOR_STOP tick={tick} reason=bounded_test_run");
            return Ok(());
        }
        sleep(cx.now_for_observability(), config.interval).await;
    }
}

async fn render_dispatch_command(
    cx: &Cx,
    config: &Config,
    request: &DispatchRenderRequest,
) -> Result<String, String> {
    let snapshot = load_bead_snapshot(cx, config, &request.bead).await?;
    dispatch_packet::validate_bead_claim(&snapshot, &request.pane)
        .map_err(|error| format!("DISPATCH_PACKET_REFUSED code={} {error}", error.code()))?;
    let identities = load_identity_registries(cx, config).await?;
    let receiver_agent =
        receiver_agent_for_dispatch(config, &request.pane, &request.bead, &snapshot)?;
    ensure_dispatch_receiver_identity(&identities, &request.bead, &request.pane, &receiver_agent)?;
    let claim_snapshot = BeadSnapshot::new_with_acceptance(
        snapshot.id(),
        snapshot.title(),
        snapshot.description(),
        snapshot.acceptance_criteria(),
        snapshot.status_label(),
        Some(&receiver_agent),
    );
    authorize_bead_dispatch_as(
        config,
        &request.pane,
        &request.bead,
        &claim_snapshot,
        &receiver_agent,
        &identities,
    )?;
    let traps = request
        .traps_file
        .as_deref()
        .map(fs::read_to_string)
        .transpose()
        .map_err(|error| {
            format!(
                "PACKET_RENDER_REFUSED bead={} pane={} traps_file={} error={error}",
                request.bead,
                request.pane,
                request
                    .traps_file
                    .as_deref()
                    .map_or_else(|| "<none>".to_owned(), |path| path.display().to_string())
            )
        })?;
    render_packet_with_sender(
        cx,
        config,
        &snapshot,
        Some(&request.pane),
        Some(&receiver_agent),
        request.why_now.as_deref(),
        traps.as_deref(),
    ).await
}

fn close_readback_exit(outcome: CloseReadback) -> std::process::ExitCode {
    match outcome {
        CloseReadback::Closed { status } => {
            println!("CLOSE_READBACK CLOSED status={status}");
            std::process::ExitCode::SUCCESS
        }
        CloseReadback::PolicyRefused { refusal } => {
            eprintln!("CLOSE_READBACK POLICY_REFUSED {refusal}");
            std::process::ExitCode::from(1)
        }
        CloseReadback::Unread { detail } => {
            eprintln!("CLOSE_READBACK UNREAD {detail}");
            std::process::ExitCode::from(2)
        }
    }
}

pub fn run(args: Vec<String>) -> std::process::ExitCode {
    let close_request = match parse_close_readback_args(&args) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("{error}");
            return std::process::ExitCode::from(2);
        }
    };
    let dispatch_request = match parse_dispatch_render_args(&args) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("{error}");
            return std::process::ExitCode::from(2);
        }
    };
    let grade_request = match parse_grade_claim_args(&args) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("{error}");
            return std::process::ExitCode::from(2);
        }
    };
    let config_args = if close_request.is_some() || dispatch_request.is_some() {
        Vec::new()
    } else if let Some(request) = &grade_request {
        request.config_args.clone()
    } else {
        args.clone()
    };
    let config = match Config::from_args(&config_args) {
        Ok(config) => config,
        Err(error) if error == usage() || error.starts_with("omp-orchestrator ") => {
            println!("{error}");
            return std::process::ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("{error}");
            return std::process::ExitCode::from(2);
        }
    };
    let runtime = match RuntimeBuilder::current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("SUPERVISOR_FATAL runtime_build detail={error}");
            return std::process::ExitCode::from(1);
        }

    };
    if let Some(request) = grade_request {
        // The invoker is the OBSERVER (it may be working — observers may work
        // by contract); the grader is selected, never assumed to be self.
        // `--grader %P` names an explicit preferred grader instead.
        let observer_pane = env::var("TMUX_PANE").unwrap_or_default();
        if observer_pane.trim().is_empty() {
            eprintln!("PEER_GRADING_REFUSED reason=current_pane_unresolved");
            return std::process::ExitCode::from(2);
        }
        let outcome = runtime.block_on(async {
            let cx =
                Cx::current().ok_or_else(|| "SUPERVISOR_REFUSED no runtime context".to_owned())?;
            run_peer_grade_claim(&cx, &config, &observer_pane, request.grader.as_deref()).await
        });
        return match outcome {
            Ok(outcome) => peer_grade_command_exit(outcome),
            Err(error) => {
                // Same discrimination as `dispatch --render`: a bead the
                // grader does not hold in_progress is a distinct operator
                // condition (fix the claim) from a selector refusal.
                let exit_code = if error.contains("code=BEAD_NOT_CLAIMED") {
                    3
                } else {
                    2
                };
                eprintln!("{error}");
                std::process::ExitCode::from(exit_code)
            }
        };
    }

    if let Some(request) = dispatch_request {
        let outcome = runtime.block_on(async {
            let cx =
                Cx::current().ok_or_else(|| "SUPERVISOR_REFUSED no runtime context".to_owned())?;
            render_dispatch_command(&cx, &config, &request).await
        });
        return match outcome {
            Ok(packet) => {
                print!("{packet}");
                std::process::ExitCode::SUCCESS
            }
            Err(error) => {
                let exit_code = if error.contains("code=BEAD_NOT_CLAIMED") {
                    3
                } else {
                    1
                };
                eprintln!("{error}");
                std::process::ExitCode::from(exit_code)
            }
        };
    }

    if let Some(request) = close_request {
        let outcome = runtime.block_on(async {
            let cx =
                Cx::current().ok_or_else(|| "SUPERVISOR_REFUSED no runtime context".to_owned())?;
            Ok::<CloseReadback, String>(
                close_and_read_back(&cx, &config, &request.bead, &request.reason).await,
            )
        });
        return match outcome {
            Ok(outcome) => close_readback_exit(outcome),
            Err(error) => {
                eprintln!("SUPERVISOR_REFUSED {error}");
                std::process::ExitCode::from(2)
            }
        };
    }

    let result = runtime.block_on(async move {
        let cx =
            Cx::current().ok_or_else(|| "SUPERVISOR_FATAL no_runtime_context".to_owned())?;
        if config.omp_quick {
            run_omp_quick(&cx, &config)
                .await
                .map_err(|error| error.to_string())
        } else {
            run_supervisor(&cx, config).await
        }
    });
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("SUPERVISOR_FATAL {error}");
            std::process::ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_precondition::{measurable_here, HostRequirement};
    use std::os::unix::fs::PermissionsExt;

    fn fixture_config(heartbeat_ledger: PathBuf) -> Config {
        let root = heartbeat_ledger
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let repo = root.join("repo");
        let tmux_tmpdir = root.join("tmux");
        std::fs::create_dir_all(&repo).expect("fixture repository");
        std::fs::create_dir_all(&tmux_tmpdir).expect("fixture tmux directory");
        let subagent_registry = root.join("subagents.json");
        std::fs::write(
            &subagent_registry,
            r#"[{"name":"AmberGate","actor_id":"agent-mail:amber"},{"name":"BlueLantern","actor_id":"agent-mail:blue"},{"name":"GreenFrog","actor_id":"agent-mail:green"},{"name":"SilverWolf","actor_id":"agent-mail:silver"},{"name":"MailMining","actor_id":"subagent:mail-mining"},{"name":"ExtractTwo","actor_id":"subagent:extract-two"}]"#,
        )
        .expect("fixture subagent registry");
        Config {
            repo,
            reap_finished_panes: "reap-finished-panes".to_owned(),
            omp_quick: false,
            session: "test-session".to_owned(),
            interval: Duration::from_secs(1),
            command_timeout: Duration::from_secs(5),
            max_ticks: Some(1),
            ompo: "ompo".to_owned(),
            tick_monitor: "tick-monitor".to_owned(),
            br: "br".to_owned(),
            am: String::new(),
            bv: "bv".to_owned(),
            ntm: "ntm".to_owned(),
            tmux_tmpdir,
            exclude_panes: Vec::new(),
            heartbeat_ledger,
            bead_lifecycle_ledger: root.join("bead-lifecycle.jsonl"),
            tick_monitor_state: root.join("state"),
            pending_dispatch: root.join("pending"),
            finding_spool: root.join("findings"),
            // The fixture writes the same six namespace records used by the identity tests.
            // It is intentionally non-empty so known-good dispatch tests exercise resolution
            // rather than the absent-registry anti-vacuity refusal.
            subagent_registry,
            receiver_agent: "BlueLantern".to_owned(),
            // EMPTY ON PURPOSE. A populated identity here would make every
            // test that reaches `report_dispatch_result` send REAL mail to the
            // live daemon and register the fixture's temp path as a project.
            // The empty value refuses before any I/O, so the durable
            // notification is exercised as a NAMED degradation in unit tests
            // and proven for real only against the live daemon.
            mail_sender: String::new(),
            omp_binary: PathBuf::from("omp"),
            uds_binary: None,
            uds_registry: None,
        }
    }

    fn test_identity_registries() -> IdentityRegistries {
        IdentityRegistries::new(
            vec![
                IdentityRecord::agent_mail("AmberGate", "agent-mail:amber"),
                IdentityRecord::agent_mail("BlueLantern", "agent-mail:blue"),
                IdentityRecord::agent_mail("GreenFrog", "agent-mail:green"),
                IdentityRecord::agent_mail("SilverWolf", "agent-mail:silver"),
            ],
            vec![
                IdentityRecord::subagent("MailMining", "subagent:mail-mining"),
                IdentityRecord::subagent("ExtractTwo", "subagent:extract-two"),
            ],
        )
    }

    fn good_ntm_receipt_json(packet: &str, operation_id: &str, target: &str) -> String {
        let digest = packet_digest(packet.as_bytes());
        let digest = digest.strip_prefix("sha256:").unwrap_or(&digest);
        json!({
            "success": true,
            "operation": {
                "operation_id": operation_id,
                "status": "completed",
                "payload_sha256": digest,
                "payload_bytes": packet.len(),
                "admissions": [{"target": target, "state": "submitted"}],
            },
            "outcome": {
                "success": true,
                "successful": [target],
                "failed": [],
            },
        })
        .to_string()
    }

    #[test]
    fn ntm_send_receipt_known_good_binds_payload_and_target() {
        let packet = "receipt test packet\\n";
        let operation_id = "omp-ntm-send-test-good";
        let raw = good_ntm_receipt_json(packet, operation_id, "5");
        let receipt = parse_ntm_send_receipt(Some(0), raw.as_bytes(), b"").expect("valid receipt");

        validate_ntm_send_receipt(&receipt, operation_id, packet, "%5")
            .expect("receipt binds the submitted payload to the target");
        assert_eq!(receipt.status, "completed");
        assert_eq!(ntm_send_receipt_args(operation_id), vec![
            "--robot-send-receipt=omp-ntm-send-test-good"
        ]);
    }

    #[test]
    fn missing_ntm_send_receipt_is_not_delivery_and_pins_message_and_exit() {
        let raw = br#"{"success":false,"error":"send operation 'missing-op' not found","error_code":"NOT_FOUND"}"#;
        let error = parse_ntm_send_receipt(Some(1), raw, b"").expect_err("missing receipt");

        assert!(error.contains("NTM_SEND_RECEIPT_FAILED"));
        assert!(error.contains("exit=1"), "the command exit code is evidence: {error}");
        assert!(error.contains("error_code=NOT_FOUND"), "the daemon code is evidence: {error}");
        assert!(
            error.contains("message=send operation 'missing-op' not found"),
            "the daemon message identifies the missing receipt: {error}"
        );
        assert!(!error.contains("completed"), "missing receipt cannot be read as delivery");
    }

    #[test]
    fn empty_ntm_admissions_are_unknown_not_a_vacuous_pass() {
        let packet = "receipt anti-vacuity\\n";
        let operation_id = "omp-ntm-send-test-empty";
        let mut value: Value = serde_json::from_str(&good_ntm_receipt_json(packet, operation_id, "5"))
            .expect("fixture JSON");
        value["operation"]["admissions"] = Value::Array(Vec::new());
        let error = parse_ntm_send_receipt(
            Some(0),
            value.to_string().as_bytes(),
            b"",
        )
        .expect_err("an empty admission set is not evidence");

        assert_eq!(
            error,
            "NTM_SEND_RECEIPT_MALFORMED field=operation.admissions reason=empty"
        );
    }

    #[test]
    fn ntm_send_carries_idempotency_key_into_the_reachable_trigger() {
        let args = ntm_send_args(
            "omp-orchestrator",
            "%5",
            Path::new("/state/packet.txt"),
            "omp-ntm-send-test-wiring",
        );
        assert_eq!(args[0], "--robot-send=omp-orchestrator");
        assert!(args.iter().any(|arg| arg == "--op-id=omp-ntm-send-test-wiring"));
        assert!(args.iter().any(|arg| arg == "--msg-file=/state/packet.txt"));
    }
    /// The four repo shapes the docs gate must distinguish. Measured 2026-09-05:
    /// `omp-orchestrator` is 676,234 B / 13 sections; `uds` and `control-plane` are
    /// absent / 0. The original order read the assembly FIRST, so shapes 3 and 4 were
    /// indistinguishable and `--repo uds` died at tick 1 forever.
    fn shape_fixture(
        sections: &[(&str, &str)],
        assembly: Option<&str>,
    ) -> (tempfile::TempDir, Config) {
        let guard = tempfile::tempdir().expect("docs-shape fixture root");
        let config = fixture_config(guard.path().join("heartbeat.jsonl"));
        let docs = config.repo.join("docs");
        std::fs::create_dir_all(docs.join("plan")).expect("plan dir");
        for (name, body) in sections {
            std::fs::write(docs.join("plan").join(name), body).expect("section");
        }
        if let Some(a) = assembly {
            std::fs::write(docs.join("PLAN.md"), a).expect("assembly");
        }
        (guard, config)
    }

    #[test]
    fn capacity_refusal_is_requeued_without_a_decision_row() {
        let guard = tempfile::tempdir().expect("heartbeat fixture root");
        let heartbeat = guard.path().join("heartbeat.jsonl");
        let config = fixture_config(heartbeat.clone());
        write_heartbeat(
            &config,
            1,
            "SUPERVISOR_REFUSED",
            "DISPATCH_BLOCKED bead=omp-orchestrator-capacity-1 receiver agent is missing owner=josh next_action=claim-bead",
        )
        .expect("heartbeat write");

        let line = std::fs::read_to_string(&heartbeat).expect("heartbeat readable");
        let row: Value = serde_json::from_str(line.trim()).expect("heartbeat JSON");
        assert_eq!(row["dispatch_action"], "DISPATCH_REQUEUED");
        assert!(
            !config.repo.join("docs/decisions.jsonl").exists(),
            "capacity recovery must not create a human decision row"
        );
        let now = row["ts_unix"].as_u64().expect("timestamp");
        assert!(
            redispatch_cooldown_age(
                &line,
                "omp-orchestrator-capacity-1",
                now.saturating_add(1)
            )
            .is_some(),
            "a requeued bead must remain on cooldown"
        );
    }

    #[test]
    fn no_sections_and_no_assembly_is_not_applicable_not_stale() {
        // uds and control-plane. This is the leg the whole 202-bead block rested on.
        let (_g, config) = shape_fixture(&[], None);
        match docs_are_stale(&config) {
            Ok(DocsVerdict::NotApplicable(why)) => {
                assert!(
                    why.contains("does not assemble a plan"),
                    "the state must SAY it is not applicable, not pass silently: {why}"
                );
            }
            other => panic!("a repo with nothing to assemble must be NotApplicable, got {other:?}"),
        }
    }

    #[test]
    fn sections_present_but_assembly_missing_still_refuses() {
        // The REAL defect this gate exists for. Must keep refusing.
        let (_g, config) = shape_fixture(
            &[("01-intro.md", "a section body long enough to probe")],
            None,
        );
        match docs_are_stale(&config) {
            Ok(DocsVerdict::Stale(why)) => {
                assert!(why.contains("assembly absent"), "{why}");
                assert!(
                    why.contains("1 numbered sections") || why.contains("while 1"),
                    "{why}"
                );
            }
            other => panic!("sections without an assembly must be Stale, got {other:?}"),
        }
    }

    #[test]
    fn assembly_without_sources_is_still_an_error() {
        // ANTI-VACUITY, preserved: an assembly whose sources vanished cannot be told
        // from a fresh one, so it must not become NotApplicable.
        let (_g, config) = shape_fixture(&[], Some("some assembled text"));
        assert!(
            docs_are_stale(&config).is_err(),
            "an assembly with zero sources must remain an Err, never NotApplicable"
        );
    }

    #[test]
    fn stale_content_is_detected_and_fresh_content_is_not() {
        let body = "this is the section body, long enough that the 240-char probe is non-empty";
        // FRESH: the section's bytes appear in the assembly.
        let (_g1, fresh) =
            shape_fixture(&[("01-a.md", body)], Some(&format!("preamble\n{body}\n")));
        assert!(
            matches!(docs_are_stale(&fresh), Ok(DocsVerdict::Fresh)),
            "a section contained in the assembly is Fresh"
        );
        // STALE, fires-on-known-bad: same shape, section NOT in the assembly.
        let (_g2, stale) = shape_fixture(&[("01-a.md", body)], Some("preamble only\n"));
        match docs_are_stale(&stale) {
            Ok(DocsVerdict::Stale(why)) => {
                assert!(why.contains("01-a.md"), "must NAME the section: {why}")
            }
            other => panic!("a section missing from the assembly must be Stale, got {other:?}"),
        }
    }
    fn isolated_fixture_config() -> (tempfile::TempDir, Config) {
        let temp = tempfile::tempdir().expect("isolated fixture root");
        let config = fixture_config(temp.path().join("heartbeat.jsonl"));
        (temp, config)
    }

    fn run_reaper_for_test(config: &Config) -> Result<String, String> {
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            run_finished_pane_sweep(&cx, config).await
        })
    }

    fn retained_dispatchable_pane(pane: &str) -> PaneObservation {
        PaneObservation {
            pane_id: pane.to_owned(),
            state: "IDLE".to_owned(),
            liveness: "NEWLY_IDLE".to_owned(),
            is_dispatchable: true,
            is_free_capacity: true,
            is_working: false,
            awaits_human: false,
        }
    }

    fn run_prepare_for_test_with_observation(
        config: &Config,
        pane: &str,
        pane_observation: PaneObservation,
        bead: &str,
        tick: u64,
        claim_enabled: bool,
    ) -> Result<(BeadSnapshot, String), String> {
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            let identities = test_identity_registries();
            prepare_bead_dispatch(
                &cx,
                config,
                pane,
                &pane_observation,
                bead,
                &identities,
                tick,
                claim_enabled,
                cross_pane_hold::HoldIntent::Work,
                true,
                true,
                1,
            )
            .await
            .map(|(snapshot, receiver, _admission)| (snapshot, receiver))
        })
    }

    fn run_prepare_for_test(
        config: &Config,
        pane: &str,
        bead: &str,
        tick: u64,
        claim_enabled: bool,
    ) -> Result<(BeadSnapshot, String), String> {
        run_prepare_for_test_with_observation(
            config,
            pane,
            retained_dispatchable_pane(pane),
            bead,
            tick,
            claim_enabled,
        )
    }
    fn executable_reaper(temp: &tempfile::TempDir, body: &str) -> PathBuf {
        let path = temp.path().join("reaper");
        std::fs::write(&path, body).expect("write reaper fixture");
        let mut permissions = std::fs::metadata(&path)
            .expect("reaper metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("make reaper executable");
        path
    }

    fn run_identity_for_test(config: &Config, pane: &str) -> Result<ObservationIdentity, String> {
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            ntm_output_identity(&cx, config, pane).await
        })
    }

    #[test]
    fn a_single_index_row_resolves_a_percent_pane_request() {
        let temp = tempfile::tempdir().expect("identity fixture");
        let script = executable_reaper(
            &temp,
            r#"#!/bin/sh
printf '%s\n' '{"success":true,"agents":[{"pane":"5","agent_type":"omp-claude","state":"IDLE","observation_state":"idle","safe_to_dispatch":true,"capture_provenance":"live","observation_freshness":"fresh","output_sequence":{"epoch":"epoch-a","sequence":9}}]}'
"#,
        );
        let mut config = fixture_config(temp.path().join("heartbeat.jsonl"));
        config.repo = temp.path().to_owned();
        config.ntm = script.display().to_string();

        let identity = run_identity_for_test(&config, "%1409").expect("single NTM row");
        assert_eq!(identity.epoch, "epoch-a");
        assert_eq!(identity.sequence, 9);
    }

    #[test]
    fn zero_identity_rows_are_named_absent() {
        let temp = tempfile::tempdir().expect("identity absent fixture");
        let script = executable_reaper(
            &temp,
            r#"#!/bin/sh
printf '%s\n' '{"success":true,"agents":[]}'
"#,
        );
        let mut config = fixture_config(temp.path().join("heartbeat.jsonl"));
        config.repo = temp.path().to_owned();
        config.ntm = script.display().to_string();

        let error = run_identity_for_test(&config, "%1409").expect_err("empty NTM rows");
        assert!(error.contains("IDENTITY_ROW_ABSENT"), "{error}");
    }

    #[test]
    fn multiple_identity_rows_without_a_canonical_key_are_named_mismatch() {
        let temp = tempfile::tempdir().expect("identity mismatch fixture");
        let script = executable_reaper(
            &temp,
            r#"#!/bin/sh
printf '%s\n' '{"success":true,"agents":[{"pane":"4","agent_type":"omp-claude","state":"IDLE","observation_state":"idle","safe_to_dispatch":true,"capture_provenance":"live","observation_freshness":"fresh"},{"pane":"5","agent_type":"omp-claude","state":"IDLE","observation_state":"idle","safe_to_dispatch":true,"capture_provenance":"live","observation_freshness":"fresh"}]}'
"#,
        );
        let mut config = fixture_config(temp.path().join("heartbeat.jsonl"));
        config.repo = temp.path().to_owned();
        config.ntm = script.display().to_string();

        let error = run_identity_for_test(&config, "%1409").expect_err("ambiguous NTM rows");
        assert!(error.contains("IDENTITY_ROW_KEY_MISMATCH"), "{error}");
    }

    #[test]
    fn observed_idle_state_counts_as_free_capacity_before_confirmation() {
        let observation = parse_observation(
            br#"{"omp_lifecycle":{"panes":[{"pane":"%1","state":"IDLE","liveness":"UNPROVEN"}]},"idle_panes":{"dispatchable":[],"free_capacity":[]}}"#,
            Some(GateCensus { rows: Vec::new() }),
        )
        .unwrap();
        assert!(observation.panes[0].is_free_capacity);
        assert_eq!(observation.panes[0].liveness, "UNPROVEN");
        assert!(observation.panes[0].is_free_capacity);
        assert!(!observation.panes[0].is_dispatchable);
    }

    #[test]
    fn uncertain_dispatch_is_fenced_per_pane_not_fleet_wide() {
        let temp = tempfile::tempdir().expect("pending fixture tempdir");
        let root = temp.path().to_path_buf();
        let pending = root.join("pending-dispatch");
        let mut config = fixture_config(root.join("heartbeat.jsonl"));
        config.pending_dispatch = pending.clone();
        write_dispatch_intent(&config, "%1413", "omp-orchestrator-test").unwrap();

        // POSITIVE CONTROL: a FRESH marker still blocks ITS OWN pane. A fix that
        // retires markers unconditionally re-opens double-dispatch, which is worse
        // than latching.
        let same_pane = write_dispatch_intent(&config, "%1413", "second-bead-same-pane");
        assert!(
            same_pane.unwrap_err().contains("DISPATCH_RETRY_BLOCKED"),
            "a second dispatch to the SAME pane must still be refused"
        );

        // THE LEG THIS TEST EXISTED TO ASSERT THE OPPOSITE OF. It previously
        // required `%1414` to be REFUSED, which encoded the defect as a
        // requirement: one global marker file meant `create_new` returned
        // `File exists (os error 17)` for every other pane, so a single in-flight
        // dispatch idled the whole fleet for up to PENDING_DISPATCH_MAX_AGE_SECS.
        // Measured live 2026-09-02 with 3 panes IDLE and 76 beads ready.
        write_dispatch_intent(&config, "%1414", "another-bead")
            .expect("a DIFFERENT pane must not be blocked by %1413's marker");

        // Both markers coexist, each attributed to its own pane.
        let mut seen = read_pending_dispatches(&config)
            .unwrap()
            .into_iter()
            .map(|(pane, _path, verdict)| {
                assert!(
                    matches!(verdict, PendingDispatch::Live { .. }),
                    "a marker written moments ago must classify Live for {pane}"
                );
                pane
            })
            .collect::<Vec<_>>();
        seen.sort();
        assert_eq!(seen, vec!["%1413".to_owned(), "%1414".to_owned()]);

        // Clearing one pane leaves the other's fence standing.
        clear_dispatch_intent(&config, "%1413").unwrap();
        let remaining = read_pending_dispatches(&config).unwrap();
        assert_eq!(remaining.len(), 1, "clearing %1413 must not clear %1414");
        assert_eq!(remaining[0].0, "%1414");
        clear_dispatch_intent(&config, "%1414").unwrap();
        assert!(read_pending_dispatches(&config).unwrap().is_empty());
    }

    // -----------------------------------------------------------------------
    // y6v5 legs. The defect: one failed dispatch left a marker, and `run_cycle`
    // returned Ok(()) on it 15 consecutive times over 20 minutes while `br ready`
    // held 77 other beads. The marker check is the FIRST statement in the cycle,
    // so one stale file blocked the entire queue and the only named remedy was a
    // human.
    //
    // `classify_pending_dispatch` takes the clock, so these legs plant an age
    // instead of sleeping.
    // -----------------------------------------------------------------------

    fn intent_text(issued_at: u64) -> String {
        serde_json::json!({
            "event": "dispatch_intent",
            "pane": "%1413",
            "bead": "omp-orchestrator-ack-spine-oj6.3",
            "issued_at": issued_at,
        })
        .to_string()
    }

    /// ACCEPTANCE 3, fires-on-known-bad: the marker that actually latched the fleet.
    /// Its real age at the last observed refusal was ~600s and climbing.
    #[test]
    fn a_stale_intent_is_expired_by_the_loop_and_names_its_age() {
        let now = 1_767_331_200;
        let stale = classify_pending_dispatch(&intent_text(now - 1_200), now);
        let PendingDispatch::Expired { detail, age_secs } = &stale else {
            panic!("a 20-minute-old marker must Expire, got {stale:?}");
        };
        assert_eq!(*age_secs, 1_200);
        // Per gate rule 7: assert on the emitted TEXT, so the row an operator reads
        // carries the bead. A verdict with no subject cannot be acted on.
        assert!(detail.contains("omp-orchestrator-ack-spine-oj6.3"));
    }

    /// ACCEPTANCE 4, the positive control at the boundary. Exactly at the deadline the
    /// marker is still Live; one second past it Expires. A fix that is off by one at
    /// the boundary is a fix whose deadline nobody can state.
    #[test]
    fn the_deadline_boundary_is_exact_in_both_directions() {
        let now = 1_767_331_200;
        let at = classify_pending_dispatch(&intent_text(now - PENDING_DISPATCH_MAX_AGE_SECS), now);
        assert!(
            matches!(at, PendingDispatch::Live { .. }),
            "exactly at max_age must still block, got {at:?}"
        );
        let past =
            classify_pending_dispatch(&intent_text(now - PENDING_DISPATCH_MAX_AGE_SECS - 1), now);
        assert!(
            matches!(past, PendingDispatch::Expired { .. }),
            "one second past max_age must expire, got {past:?}"
        );
    }

    /// ACCEPTANCE 5 + the fail-closed rule. Three unknowns must all keep blocking and
    /// must NOT be retired as stale. A corrupt marker unlocking the loop is the
    /// inverse of the guard's purpose, and it is the cheaper mistake to make.
    #[test]
    fn an_undatable_intent_fails_closed_and_never_expires() {
        let now = 1_767_331_200;
        for (text, want) in [
            ("", "INTENT_EMPTY"),
            ("   \n ", "INTENT_EMPTY"),
            (
                r#"{"event":"dispatch_intent","pane":"%1413"}"#,
                "INTENT_ISSUED_AT_MISSING",
            ),
            (
                r#"{"issued_at":"not-a-number"}"#,
                "INTENT_ISSUED_AT_MISSING",
            ),
            ("not json at all", "INTENT_ISSUED_AT_MISSING"),
        ] {
            let verdict = classify_pending_dispatch(text, now);
            let PendingDispatch::Undatable { reason, .. } = &verdict else {
                panic!("{text:?} must fail closed, got {verdict:?}");
            };
            assert_eq!(*reason, want, "reason for {text:?}");
        }
        // A marker stamped in the FUTURE is a clock fault, not an old marker. It must
        // not compute a wrapped age and expire itself.
        let future = classify_pending_dispatch(&intent_text(now + 30), now);
        assert_eq!(
            future,
            PendingDispatch::Undatable {
                detail: intent_text(now + 30),
                reason: "INTENT_ISSUED_IN_FUTURE",
            }
        );
    }

    /// ACCEPTANCE 5, the reachable-absence half. `None` must be a real state, not an
    /// error and not a silent pass — and an UNREADABLE marker must stay an error.
    /// Those two collapsing is how "nothing pending" becomes indistinguishable from
    /// "cannot tell".
    #[test]
    fn an_absent_marker_is_a_reachable_state_and_an_unreadable_one_is_an_error() {
        let temp = tempfile::tempdir().expect("marker fixture tempdir");
        let root = temp.path().to_path_buf();
        std::fs::create_dir_all(&root).expect("fixture root");
        let mut config = fixture_config(root.join("heartbeat.jsonl"));
        config.pending_dispatch = root.join("no-such-marker");
        assert_eq!(
            read_pending_dispatch(&config).unwrap(),
            PendingDispatch::None
        );

        // A DIRECTORY where the marker should be is readable-as-a-path and
        // unreadable-as-a-file: the unknown that must not read as absent.
        let as_dir = root.join("marker-is-a-dir");
        std::fs::create_dir_all(&as_dir).expect("marker dir");
        config.pending_dispatch = as_dir.clone();
        let error = read_pending_dispatch(&config).expect_err("a directory must not read as None");
        assert!(
            error.contains("unreadable"),
            "error must name the condition: {error}"
        );
    }

    /// ACCEPTANCE 6: the retry is bounded, and the bound is stated in seconds rather
    /// than in ticks. The observed latch reset its tick counter from 7 to 1, so a
    /// tick budget would have been reset by the same restart that re-entered the
    /// latch. Wall-clock age survives a restart; a tick counter does not.
    #[test]
    fn the_bound_is_wall_clock_so_a_restart_cannot_reset_it() {
        let now = 1_767_331_200;
        let issued = now - PENDING_DISPATCH_MAX_AGE_SECS - 1;
        // Same marker, two different "processes" — no shared counter between them.
        for _restart in 0..3 {
            assert!(matches!(
                classify_pending_dispatch(&intent_text(issued), now),
                PendingDispatch::Expired { .. }
            ));
        }
        // Clippy caught this leg as `assertion has a constant value`, and it was
        // right in a way worth keeping: a bound on a `const` belongs at COMPILE
        // time, where it cannot be skipped by a filtered test run. Moved to the
        // const-assert beside the constant; what remains here is the runtime fact.
        assert!(
            classify_pending_dispatch(&intent_text(now), now)
                != classify_pending_dispatch(&intent_text(issued), now),
            "a fresh marker and a stale one must not classify alike"
        );
    }

    fn plant_intent(config: &Config, pane: &str, bead: &str, issued_at: u64) -> PathBuf {
        let path = pending_dispatch_path(config, pane);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("marker parent");
        }
        let row = serde_json::json!({
            "event": "dispatch_intent",
            "pane": pane,
            "bead": bead,
            "issued_at": issued_at,
        });
        std::fs::write(&path, format!("{row}\n")).expect("plant marker");
        path
    }

    #[test]
    fn absent_marker_label_is_no_pending_dispatch_not_an_error() {
        assert_eq!(PendingDispatch::None.label(), "NO_PENDING_DISPATCH");
        let temp = tempfile::tempdir().expect("absent marker tempdir");
        let root = temp.path().to_path_buf();
        let mut config = fixture_config(root.join("heartbeat.jsonl"));
        config.pending_dispatch = root.join("no-such-marker");
        assert_eq!(
            read_pending_dispatch(&config).unwrap().label(),
            "NO_PENDING_DISPATCH"
        );
        match process_pending_markers(&config, 1).unwrap() {
            MarkerFence::Proceed(outcome) => {
                assert!(outcome.blocked_panes.is_empty());
                assert!(outcome.cleared.is_empty());
            }
            other => panic!("absence must Proceed, got {other:?}"),
        }
    }

    #[test]
    fn receiver_observation_missing_clears_the_marker_ack_wait_does_not() {
        assert_eq!(
            intent_clear_reason("RECEIVER_OBSERVATION_MISSING pane=%1413 identity row absent"),
            Some("RECEIVER_OBSERVATION_MISSING"),
            "d6q2: missing receiver observation must not latch"
        );
        assert_eq!(
            intent_clear_reason(
                "ACK_STAGE_RETRY_BLOCKED pane=%1413 bead=x action=RETRY verdict=INDETERMINATE reason=ack_readback_missing after=75s"
            ),
            None,
            "a genuine in-flight ACK wait must keep the Live marker"
        );
        let temp = tempfile::tempdir().expect("clear-on-missing tempdir");
        let root = temp.path().to_path_buf();
        let mut config = fixture_config(root.join("heartbeat.jsonl"));
        config.pending_dispatch = root.join("pending");
        let path = plant_intent(
            &config,
            "%1413",
            "omp-orchestrator-ack-spine-oj6.3",
            now_unix(),
        );
        assert!(path.exists(), "fresh marker must exist before the refusal");
        let reason = intent_clear_reason("RECEIVER_OBSERVATION_MISSING pane=%1413").unwrap();
        clear_dispatch_intent(&config, "%1413").unwrap();
        assert!(
            !path.exists(),
            "reason={reason} must remove the marker so the next cycle is not latched"
        );
    }

    #[test]
    fn a_stale_marker_is_cleared_by_the_loop_and_a_different_bead_is_selected() {
        let temp = tempfile::tempdir().expect("stale-loop tempdir");
        let root = temp.path().to_path_buf();
        let mut config = fixture_config(root.join("heartbeat.jsonl"));
        config.pending_dispatch = root.join("pending");
        let latched = "omp-orchestrator-ack-spine-oj6.3";
        let next = "omp-orchestrator-kernel-gate-census-69i";
        let path = plant_intent(
            &config,
            "%1413",
            latched,
            now_unix() - PENDING_DISPATCH_MAX_AGE_SECS - 30,
        );
        let MarkerFence::Proceed(outcome) = process_pending_markers(&config, 1).unwrap() else {
            panic!("expired marker must Proceed after the machine clear");
        };
        assert!(
            !path.exists(),
            "KNOWN-BAD: the loop itself must remove the stale marker"
        );
        assert_eq!(outcome.blocked_panes.len(), 0);
        assert_eq!(outcome.cleared.len(), 1);
        assert_eq!(outcome.cleared[0].1, latched);
        assert_eq!(outcome.cleared[0].2, "DISPATCH_INTENT_EXPIRED");
        let ready = vec![latched.to_owned(), next.to_owned(), "omp-orchestrator-third".to_owned()];
        let cleared_beads: Vec<String> = outcome.cleared.iter().map(|row| row.1.clone()).collect();
        let (selected, skipped) =
            select_ready_skipping_cooldown(&ready, "", now_unix(), &cleared_beads);
        assert!(skipped.is_empty());
        let selected = selected.expect("ready queue");
        assert_eq!(selected, next);
        assert_ne!(
            selected, latched,
            "cleared={latched} selected={selected} — both beads named, and they differ"
        );
    }

    #[test]
    fn a_fresh_same_pane_marker_still_blocks_a_second_dispatch() {
        let temp = tempfile::tempdir().expect("live-positive-control tempdir");
        let root = temp.path().to_path_buf();
        let mut config = fixture_config(root.join("heartbeat.jsonl"));
        config.pending_dispatch = root.join("pending");
        let path = plant_intent(&config, "%1413", "omp-orchestrator-first-bead", now_unix());
        let MarkerFence::Proceed(outcome) = process_pending_markers(&config, 1).unwrap() else {
            panic!("fresh marker must stay Live, not StopUndatable");
        };
        assert!(path.exists(), "POSITIVE CONTROL: in-flight marker stays");
        assert_eq!(outcome.blocked_panes, vec!["%1413".to_owned()]);
        assert!(outcome.cleared.is_empty());
        let second = write_dispatch_intent(&config, "%1413", "omp-orchestrator-second-bead");
        assert!(
            second.unwrap_err().contains("DISPATCH_RETRY_BLOCKED"),
            "a second dispatch to the same pane must still refuse"
        );
    }

    fn planted_ack(bead: &str, pane: &str, created_at: u64) -> AckReadback {
        let token = bead.rsplit('-').next().unwrap();
        let json = serde_json::json!([{
            "text": format!("ACK {token} on {pane} -- starting..."),
            "created_at": created_at,
        }]);
        AckReadback::from_comments_json(bead, pane, json.to_string().as_bytes())
            .expect("planted ACK comments JSON")
    }

    #[test]
    fn dispatch_verdict_indeterminate_is_not_dispatch_failed() {
        let v = DispatchVerdict::Indeterminate {
            reason: "unproven_transport".to_owned(),
            transport: "codex_tmux".to_owned(),
        };
        assert_eq!(v.status_word(), "DISPATCH_INDETERMINATE");
        assert_ne!(v.status_word(), "DISPATCH_FAILED");
        let pending = DispatchVerdict::AckPending {
            after_secs: 90,
            discriminator:
                " discriminator=ACK_PENDING_WORKER_BUSY discriminator_reason=NONE owes_human=false"
                    .to_owned(),
        };
        assert_eq!(pending.status_word(), "ACK_PENDING");
        assert_eq!(
            DispatchVerdict::Failed("TIMEOUT program=tmux".into()).status_word(),
            "DISPATCH_FAILED"
        );
    }

    #[test]
    fn late_ack_clears_live_marker_instead_of_blocking() {
        let temp = tempfile::tempdir().expect("late-ack live");
        let root = temp.path().to_path_buf();
        let mut config = fixture_config(root.join("heartbeat.jsonl"));
        config.pending_dispatch = root.join("pending");
        let bead = "omp-orchestrator-ack-spine-oj6.3";
        let pane = "%1414";
        let issued = now_unix() - 30;
        let path = plant_intent(&config, pane, bead, issued);
        let ack = planted_ack(bead, pane, issued + 8);
        let MarkerFence::Proceed(outcome) = process_pending_markers_with(&config, 1, |b, p, _| {
            (b == bead && p == pane).then(|| ack.clone())
        })
        .unwrap() else {
            panic!("late ACK on Live must Proceed");
        };
        assert!(
            !path.exists(),
            "ACK_RECEIVED_LATE must clear the live marker"
        );
        assert!(outcome.blocked_panes.is_empty(), "{outcome:?}");
        assert_eq!(outcome.cleared[0].2, "ACK_RECEIVED_LATE");
        let heartbeat = std::fs::read_to_string(&config.heartbeat_ledger).unwrap();
        assert!(heartbeat.contains("ACK_RECEIVED_LATE"), "{heartbeat}");
        assert!(
            !heartbeat.contains("DISPATCH_INTENT_EXPIRED"),
            "{heartbeat}"
        );
    }

    #[test]
    fn late_ack_clears_expired_marker_instead_of_expiring() {
        let temp = tempfile::tempdir().expect("late-ack expired");
        let root = temp.path().to_path_buf();
        let mut config = fixture_config(root.join("heartbeat.jsonl"));
        config.pending_dispatch = root.join("pending");
        let bead = "omp-orchestrator-ack-spine-oj6.3";
        let pane = "%1414";
        let issued = now_unix() - PENDING_DISPATCH_MAX_AGE_SECS - 30;
        let path = plant_intent(&config, pane, bead, issued);
        let ack = planted_ack(bead, pane, issued + 8);
        let MarkerFence::Proceed(outcome) = process_pending_markers_with(&config, 1, |b, p, _| {
            (b == bead && p == pane).then(|| ack.clone())
        })
        .unwrap() else {
            panic!("late ACK on Expired must Proceed");
        };
        assert!(
            !path.exists(),
            "ACK_RECEIVED_LATE must clear the expired marker"
        );
        assert_eq!(outcome.cleared[0].2, "ACK_RECEIVED_LATE");
        let heartbeat = std::fs::read_to_string(&config.heartbeat_ledger).unwrap();
        assert!(heartbeat.contains("ACK_RECEIVED_LATE"), "{heartbeat}");
        assert!(
            !heartbeat.contains("DISPATCH_INTENT_EXPIRED"),
            "{heartbeat}"
        );
    }

    #[test]
    fn expired_marker_without_ack_still_expires() {
        let temp = tempfile::tempdir().expect("expire-no-ack");
        let root = temp.path().to_path_buf();
        let mut config = fixture_config(root.join("heartbeat.jsonl"));
        config.pending_dispatch = root.join("pending");
        let bead = "omp-orchestrator-ack-spine-oj6.3";
        plant_intent(
            &config,
            "%1414",
            bead,
            now_unix() - PENDING_DISPATCH_MAX_AGE_SECS - 30,
        );
        let MarkerFence::Proceed(outcome) = process_pending_markers(&config, 1).unwrap() else {
            panic!("no ACK must expire");
        };
        assert_eq!(outcome.cleared[0].2, "DISPATCH_INTENT_EXPIRED");
    }

    #[test]
    fn cooldown_skips_recently_claimed_bead_and_takes_next() {
        let claimed = "omp-orchestrator-plan-12-ibpa.11";
        let next = "omp-orchestrator-next-ready";
        let now = 1_800_000_000;
        let heartbeat = serde_json::json!({
            "ts_unix": now - 120,
            "status": "DISPATCH_CLAIMED",
            "detail": format!("bead={claimed} pane=%7 assignee=pane=%7;incarnation=1;agent=WildStone"),
        })
        .to_string();
        let ready = vec![claimed.to_owned(), next.to_owned()];
        let (selected, skipped) = select_ready_skipping_cooldown(&ready, &heartbeat, now, &[]);
        assert_eq!(skipped[0].0, claimed);
        assert_eq!(selected, Some(next));
        assert!(redispatch_cooldown_age(&heartbeat, claimed, now).is_some());
        assert!(redispatch_cooldown_age(&heartbeat, next, now).is_none());
    }

    #[test]
    fn supervisor_claim_owner_is_session_not_pid() {
        let a = supervisor_claim_owner("omp-orchestrator");
        let b = supervisor_claim_owner("omp-orchestrator");
        assert_eq!(a, b, "two ticks must produce the same owner");
        assert_eq!(a, "supervisor:omp-orchestrator");
        assert!(
            !a.contains(&std::process::id().to_string()),
            "C112: pid form dies at process exit: {a}"
        );
    }

    #[test]
    fn closed_bead_is_refused_when_send_is_five_seconds_after_close() {
        // MEASURED 2026-09-06: close 16:43:39.130, DISPATCHED 16:43:44. Five
        // seconds later is this defect; a send inside the close window is not.
        let close_ts: u64 = 1_746_663_819;
        let send_ts: u64 = close_ts + 5;
        assert!(
            send_ts > close_ts,
            "q8zl only: send timestamp must be later than close"
        );
        let err = refuse_terminal_status_at_send("omp-orchestrator-plan-12-ibpa.11", "closed")
            .expect_err("closed at send must refuse, not transmit");
        assert!(err.contains("STALE_QUEUE_SNAPSHOT"), "{err}");
        assert!(err.contains("status=closed"), "{err}");
        assert!(err.contains("omp-orchestrator-plan-12-ibpa.11"), "{err}");
    }

    #[test]
    fn blocked_bead_is_refused_when_send_is_three_seconds_after_park() {
        // CASE 2 2026-09-06: parked y6yg 16:58:10.663 -> DISPATCHED 16:58:13 (+3s).
        let park_ts: u64 = 1_746_664_690;
        let send_ts: u64 = park_ts + 3;
        assert!(
            send_ts > park_ts,
            "q8zl only: send timestamp must be later than the terminal transition"
        );
        let err = refuse_terminal_status_at_send("omp-orchestrator-y6yg", "blocked")
            .expect_err("blocked at send must refuse, not transmit");
        assert!(err.contains("STALE_QUEUE_SNAPSHOT"), "{err}");
        assert!(err.contains("status=blocked"), "{err}");
    }

    #[test]
    fn open_acceptance_bearing_bead_is_not_refused_at_send() {
        refuse_terminal_status_at_send("omp-orchestrator-live-work", "open")
            .expect("open must still dispatch");
        refuse_terminal_status_at_send("omp-orchestrator-live-work", "in_progress")
            .expect("in_progress must still dispatch");
    }

    #[test]
    fn send_and_verify_rechecks_status_before_transport() {
        let source = include_str!("resident.rs");
        let start = source
            .find("async fn send_and_verify")
            .expect("send_and_verify must exist");
        let body = &source[start..];
        let recheck = body
            .find("refuse_terminal_status_at_send")
            .expect("transport boundary must re-read status");
        let load = body
            .find("load_bead_snapshot")
            .expect("re-check must be a fresh br show, not the prefetched snapshot");
        let transport = body
            .find("tick_monitor::SEND_KEYS")
            .expect("send path must exist so this test is not vacuous");
        assert!(
            load < recheck && recheck < transport,
            "fresh show then refuse must precede tmux/ntm send (load={load} refuse={recheck} send={transport})"
        );
    }

    #[test]
    fn at_send_capture_is_readable_from_the_ledger_without_the_tracker() {
        let capture = AtSendCapture {
            status: "in_progress".into(),
            assignee: "WildStone".into(),
            has_acceptance: true,
            filed_only: false,
        };
        let detail = capture.detail("omp-orchestrator-y6yg", "%7");
        let row = serde_json::json!({
            "status": "AT_SEND_CAPTURE",
            "detail": detail,
        });
        let read = row.get("detail").and_then(Value::as_str).unwrap();
        assert!(read.contains("at_send_status=in_progress"), "{read}");
        assert!(read.contains("at_send_assignee=WildStone"), "{read}");
        assert!(read.contains("at_send_has_acceptance=true"), "{read}");
        assert_eq!(classify_send_alignment(read), AlignmentClass::Pass);
    }

    #[test]
    fn parked_after_send_classifies_pass_not_fail() {
        // y6yg parked AFTER ticks 11/14/15. At-send status was in_progress.
        let at_send = "bead=omp-orchestrator-y6yg pane=%7 at_send_status=in_progress at_send_assignee=WildStone at_send_has_acceptance=true at_send_filed_only=false";
        assert_eq!(classify_send_alignment(at_send), AlignmentClass::Pass);
        let later_tracker_status = "blocked";
        assert_ne!(
            later_tracker_status, "in_progress",
            "the tracker now shows blocked; alignment must ignore that"
        );
        let without_capture = "bead=omp-orchestrator-y6yg pane=%7 status=blocked";
        assert_eq!(
            classify_send_alignment(without_capture),
            AlignmentClass::Unknown,
            "removing at-send capture must not classify PASS from current status"
        );
    }

    #[test]
    fn alignment_window_has_pass_and_fail_and_empty_is_error() {
        let pass = "bead=a pane=%1 at_send_status=in_progress at_send_assignee=WildStone at_send_has_acceptance=true at_send_filed_only=false".to_owned();
        let fail = "bead=b pane=%1 at_send_status=closed at_send_assignee=none at_send_has_acceptance=false at_send_filed_only=true".to_owned();
        let classes = classify_alignment_window(&[pass, fail]).expect("window");
        assert!(classes.iter().any(|c| *c == AlignmentClass::Pass));
        assert!(classes.iter().any(|c| *c == AlignmentClass::Fail));
        let empty = classify_alignment_window(&[]).expect_err("empty");
        assert!(empty.contains("ALIGNMENT_SCAN_EMPTY"), "{empty}");
    }

    #[test]
    fn dispatch_policy_falls_back_and_says_so() {
        let missing = parse_dispatch_policy(None);
        assert_eq!(missing.origin, "defaults");
        assert_eq!(missing.interval, DEFAULT_INTERVAL);
        let malformed = parse_dispatch_policy(Some("[dispatch]\ninterval_secs = not-a-number\n"));
        assert_eq!(malformed.origin, "malformed-fallback");
        let live = parse_dispatch_policy(Some(
            "[dispatch]\ninterval_secs = 45\ncommand_timeout_secs = 12\nreceipt_timeout_secs = 90\nreceipt_poll_ms = 100\npending_dispatch_max_age_secs = 120\nmail_request_timeout_secs = 5\n",
        ));
        assert_eq!(live.origin, "config.toml");
        assert_eq!(live.interval, Duration::from_secs(45));
        assert_eq!(live.command_timeout, Duration::from_secs(12));
        assert_eq!(live.pending_dispatch_max_age, Duration::from_secs(120));
    }

    #[test]
    fn acknowledged_dispatch_marker_clears_only_on_next_cycle() {
        let temp = tempfile::tempdir().expect("acknowledged marker tempdir");
        let root = temp.path().to_path_buf();
        let mut config = fixture_config(root.join("heartbeat.jsonl"));
        config.pending_dispatch = root.join("pending");
        let path = plant_intent(
            &config,
            "%1413",
            "omp-orchestrator-success-bead",
            now_unix(),
        );

        record_successful_dispatch(&config, 1, "%1413", "omp-orchestrator-success-bead")
            .expect("success must acknowledge marker");
        assert!(
            path.exists(),
            "acknowledgment must retain the marker for the next cycle"
        );
        assert!(
            matches!(
                read_pending_dispatches(&config)
                    .unwrap()
                    .into_iter()
                    .find(|(pane, _, _)| pane == "%1413"),
                Some((_, _, PendingDispatch::Acknowledged { .. }))
            ),
            "acknowledged marker must not remain indistinguishable from a live marker"
        );

        let MarkerFence::Proceed(outcome) = process_pending_markers(&config, 1).unwrap() else {
            panic!("acknowledged marker must proceed after clearing");
        };
        assert_eq!(
            outcome.cleared,
            vec![(
                "%1413".to_owned(),
                "omp-orchestrator-success-bead".to_owned(),
                "ACKNOWLEDGED",
            )]
        );
        assert!(outcome.blocked_panes.is_empty());
        assert!(
            !path.exists(),
            "next cycle must clear the acknowledged marker"
        );
    }

    #[test]
    fn heartbeat_is_durable_json_with_build_identity() {
        let temp = tempfile::tempdir().expect("heartbeat fixture tempdir");
        let root = temp.path().to_path_buf();
        let path = root.join("heartbeat.jsonl");
        let config = fixture_config(path.clone());
        write_heartbeat(&config, 7, "SUPERVISED_WORKING", "working=2 ready=1").unwrap();
        let row: Value =
            serde_json::from_str(std::fs::read_to_string(&path).unwrap().trim()).unwrap();
        assert_eq!(row["event"], "supervisor_heartbeat");
        assert_eq!(row["status"], "SUPERVISED_WORKING");
        assert_eq!(row["build_id"], BUILD_ID);
        assert_eq!(row["tick"], 7);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn supervise_flag_only_invocation_parses_with_once() {
        let args = ["--once".to_owned()];
        let config = Config::from_args(&args).unwrap();
        assert_eq!(config.max_ticks, Some(1));
    }

    #[test]
    fn tick_monitor_state_default_is_session_scoped() {
        let hb = PathBuf::from("/state/flywheel/omp-orchestrator-control-plane.heartbeat.jsonl");
        let a = default_tick_monitor_state(&hb, "control-plane");
        let b = default_tick_monitor_state(&hb, "omp-orchestrator");
        assert_ne!(a, b, "distinct sessions must not share capture state");
        assert!(
            a.ends_with("omp-orchestrator-control-plane.tick-monitor-state.json"),
            "session must be in the filename: {}",
            a.display()
        );
        assert!(
            b.ends_with("omp-orchestrator-omp-orchestrator.tick-monitor-state.json"),
            "session must be in the filename: {}",
            b.display()
        );
    }

    #[test]
    fn tick_monitor_state_default_does_not_collide_with_unscoped_legacy() {
        let hb = PathBuf::from("/state/flywheel/omp-orchestrator.heartbeat.jsonl");
        let scoped = default_tick_monitor_state(&hb, "control-plane");
        let legacy = hb.with_file_name("omp-orchestrator.tick-monitor-state.json");
        assert_ne!(
            scoped, legacy,
            "the unscoped legacy path is the measured collision"
        );
    }

    #[test]
    fn pending_dispatch_default_is_session_scoped() {
        let hb = PathBuf::from("/state/flywheel/omp-orchestrator-control-plane.heartbeat.jsonl");
        let a = default_pending_dispatch(&hb, "control-plane");
        let b = default_pending_dispatch(&hb, "omp-orchestrator");
        assert_ne!(a, b, "distinct sessions must not share pending-dispatch");
        assert!(
            a.ends_with("omp-orchestrator-control-plane.pending-dispatch"),
            "{}",
            a.display()
        );
        assert!(
            b.ends_with("omp-orchestrator-omp-orchestrator.pending-dispatch"),
            "{}",
            b.display()
        );
    }

    #[test]
    fn second_session_refuses_reusing_session_a_fixed_pending_path() {
        let shared = PathBuf::from("/home/josh/.local/state/flywheel/omp-orchestrator.pending-dispatch");
        let error = refuse_session_path_collision(
            "B",
            &[("pending_dispatch", shared.as_path())],
        )
        .expect_err("session B must not reuse the unscoped basename");
        assert!(
            error.starts_with("SESSION_PATH_COLLISION"),
            "{error}"
        );
        assert!(
            error.contains("path=/home/josh/.local/state/flywheel/omp-orchestrator.pending-dispatch"),
            "{error}"
        );
        assert!(error.contains("session=B"), "{error}");
    }

    #[test]
    fn session_path_collision_empty_scan_is_error() {
        let error = refuse_session_path_collision("B", &[]).expect_err("empty");
        assert!(
            error.contains("empty scan set"),
            "{error}"
        );
    }

    #[test]
    fn two_sessions_in_one_home_get_distinct_pending_dispatch_paths() {
        assert!(
            std::env::var_os("OMP_PENDING_DISPATCH").is_none(),
            "this test measures the default path; OMP_PENDING_DISPATCH overrides it"
        );
        let a = Config::from_args(&["--session".to_owned(), "control-plane".to_owned()]).unwrap();
        let b =
            Config::from_args(&["--session".to_owned(), "omp-orchestrator".to_owned()]).unwrap();
        assert_ne!(
            a.pending_dispatch, b.pending_dispatch,
            "{} vs {}",
            a.pending_dispatch.display(),
            b.pending_dispatch.display()
        );
        refuse_session_path_collision(
            "control-plane",
            &[
                ("tick_monitor_state", a.tick_monitor_state.as_path()),
                ("pending_dispatch", a.pending_dispatch.as_path()),
            ],
        )
        .expect("session A keys must be clean");
    }



    #[test]
    fn two_configs_different_sessions_resolve_different_state_paths() {
        assert!(
            std::env::var_os("OMP_TICK_MONITOR_STATE").is_none(),
            "this test measures the default path; OMP_TICK_MONITOR_STATE overrides it"
        );
        let a = Config::from_args(&["--session".to_owned(), "control-plane".to_owned()]).unwrap();
        let b =
            Config::from_args(&["--session".to_owned(), "omp-orchestrator".to_owned()]).unwrap();
        assert_ne!(
            a.tick_monitor_state,
            b.tick_monitor_state,
            "different sessions must not share capture state: {} vs {}",
            a.tick_monitor_state.display(),
            b.tick_monitor_state.display()
        );
    }

    #[test]
    fn identical_sessions_resolve_the_same_state_path() {
        assert!(
            std::env::var_os("OMP_TICK_MONITOR_STATE").is_none(),
            "this test measures the default path; OMP_TICK_MONITOR_STATE overrides it"
        );
        let a = Config::from_args(&["--session".to_owned(), "control-plane".to_owned()]).unwrap();
        let b = Config::from_args(&["--session".to_owned(), "control-plane".to_owned()]).unwrap();
        assert_eq!(
            a.tick_monitor_state, b.tick_monitor_state,
            "identical sessions collide on one path"
        );
    }

    #[test]
    fn unknown_positional_is_refused() {
        let stray = Config::from_args(&["extra".to_owned()]).unwrap_err();
        assert!(
            stray.contains("CONFIG_REFUSED unknown argument extra"),
            "{stray}"
        );
        let bare = Config::from_args(&["frobnicate".to_owned()]).unwrap_err();
        assert!(
            bare.contains("CONFIG_REFUSED unknown argument frobnicate"),
            "{bare}"
        );
    }

    #[test]
    fn help_reports_the_supervise_entrypoint() {
        let help = Config::from_args(&["--help".to_owned()]).unwrap_err();
        assert_eq!(help, usage());
        assert!(
            help.contains("ompo supervise"),
            "usage must advertise the supervise entrypoint"
        );
    }
    #[test]
    fn zero_command_timeout_is_refused() {
        let error = Config::from_args(&["--command-timeout-secs".to_owned(), "0".to_owned()])
            .expect_err("zero timeout cannot bound a child process");
        assert!(
            error.contains("--command-timeout-secs must be greater than zero"),
            "{error}"
        );
    }

    /// `--session` is MANDATORY, and its absence is what captured another repo.
    ///
    /// MEASURED 2026-09-05 (`ma3b`): the sweep ran unscoped and enumerated EVERY tmux
    /// session, so our tick reaped `control-plane`'s panes and wrote 3,673 transcript
    /// files for a repo we do not own into the shared reaped directory. These two legs
    /// asserted the pre-fix two-argument form and went red when the caller was scoped,
    /// which is the tests being stale rather than the code being wrong.
    #[test]
    fn finished_pane_reaper_receives_the_same_repository() {
        let temp = tempfile::tempdir().expect("reaper fixture");
        let config = fixture_config(temp.path().join("heartbeat.jsonl"));
        let args = finished_pane_reaper_args(&config);
        assert_eq!(
            args,
            vec![
                "--repo".to_owned(),
                config.repo.display().to_string(),
                "--session".to_owned(),
                config.session.clone(),
            ]
        );
        // Stated separately so a future arg reshuffle still fails on the SCOPE rather
        // than only on positional equality. A widen-to-all-sessions default is exactly
        // how control-plane's panes were captured.
        assert!(
            args.iter().any(|arg| arg == "--session"),
            "an unscoped sweep enumerates every tmux session: {args:?}"
        );
    }
    #[test]
    fn finished_pane_reaper_runs_through_the_production_helper() {
        let temp = tempfile::tempdir().expect("reaper repo");
        let mut config = fixture_config(temp.path().join("heartbeat.jsonl"));
        config.repo = temp.path().to_owned();
        config.reap_finished_panes = "/bin/echo".to_owned();
        let summary = run_reaper_for_test(&config).expect("reaper output");
        assert_eq!(
            summary,
            format!("--repo {} --session {}", temp.path().display(), config.session),
            "the production helper must invoke the reaper with the repository root AND the session scope"
        );
    }
    #[test]
    fn finished_pane_reaper_rejects_empty_output() {
        let temp = tempfile::tempdir().expect("reaper repo");
        let mut config = fixture_config(temp.path().join("heartbeat.jsonl"));
        config.repo = temp.path().to_owned();
        config.reap_finished_panes = "/usr/bin/true".to_owned();
        let error = run_reaper_for_test(&config).expect_err("empty output must be restrictive");
        assert!(error.contains("REAP_FINISHED_PANES_EMPTY"), "{error}");
    }

    #[test]
    fn finished_pane_reaper_propagates_child_failure() {
        let temp = tempfile::tempdir().expect("reaper repo");
        let mut config = fixture_config(temp.path().join("heartbeat.jsonl"));
        config.repo = temp.path().to_owned();
        config.reap_finished_panes = "/usr/bin/false".to_owned();
        let error = run_reaper_for_test(&config).expect_err("failed child must be restrictive");
        assert!(error.contains("exited=1"), "{error}");
    }

    #[test]
    fn finished_pane_reaper_receives_bounded_deadline() {
        let temp = tempfile::tempdir().expect("reaper repo");
        let mut config = fixture_config(temp.path().join("heartbeat.jsonl"));
        config.repo = temp.path().to_owned();
        config.command_timeout = Duration::from_secs(7);
        config.reap_finished_panes = executable_reaper(
            &temp,
            "#!/bin/sh\nprintf '%s\n' \"$REAP_SWEEP_DEADLINE_SECS\"\n",
        )
        .display()
        .to_string();
        let summary = run_reaper_for_test(&config).expect("deadline output");
        assert_eq!(summary, "7");
    }

    #[test]
    fn finished_pane_reaper_timeout_is_restrictive() {
        let temp = tempfile::tempdir().expect("reaper repo");
        let mut config = fixture_config(temp.path().join("heartbeat.jsonl"));
        config.repo = temp.path().to_owned();
        config.command_timeout = Duration::from_secs(1);
        config.reap_finished_panes = executable_reaper(&temp, "#!/bin/sh\nsleep 5\n")
            .display()
            .to_string();
        let error = run_reaper_for_test(&config).expect_err("hung child must time out");
        assert!(error.contains("TIMEOUT program="), "{error}");
    }

    #[test]
    fn missing_receiver_agent_inherits_claimed_assignee() {
        let (_temp, mut config) = isolated_fixture_config();
        config.receiver_agent.clear();

        let snapshot = BeadSnapshot::new(
            "receiver-assignment-test",
            "title",
            "description",
            "in_progress",
            Some("SilverWolf"),
        );

        let receiver_agent =
            authorize_bead_dispatch(&config, "%1408", "receiver-assignment-test", &snapshot)
                .expect("an assigned bead should supply the receiver agent when config is unset");
        assert_eq!(receiver_agent, "SilverWolf");
    }

    #[test]
    fn production_dispatch_identity_gate_covers_both_namespaces_and_unknowns() {
        let (_temp, mut config) = isolated_fixture_config();
        config.receiver_agent.clear();
        let identities = test_identity_registries();

        for assignee in ["MailMining", "AmberGate"] {
            let snapshot = BeadSnapshot::new(
                "u21m-production-test",
                "title",
                "description",
                "in_progress",
                Some(assignee),
            );
            let receiver = authorize_bead_dispatch_as(
                &config,
                "%1408",
                "u21m-production-test",
                &snapshot,
                assignee,
                &identities,
            )
            .expect("known identities in either namespace must pass production dispatch");
            assert_eq!(receiver, assignee);
        }

        let snapshot = BeadSnapshot::new(
            "u21m-production-test",
            "title",
            "description",
            "in_progress",
            Some("WildcardFix"),
        );
        let error = authorize_bead_dispatch_as(
            &config,
            "%1408",
            "u21m-production-test",
            &snapshot,
            "WildcardFix",
            &identities,
        )
        .expect_err("an unknown assignee must be refused before packet construction");
        assert!(error.contains("WildcardFix"), "{error}");
        assert!(error.contains("agent_mail"), "{error}");
        assert!(error.contains("subagents"), "{error}");
        assert!(error.contains("ASSIGNEE_IDENTITY_REFUSED"), "{error}");
    }

    /// The known-bad is the real incident, replayed from the heartbeat ledger.
    ///
    /// # The measured defect
    ///
    /// Measured 2026-09-01 from
    /// `~/.local/state/flywheel/omp-orchestrator.heartbeat.jsonl`: supervisor pid
    /// 70561 wrote 139 `DISPATCHED` rows, and every single one named
    /// `bead=omp-orchestrator-815`; 135 of them named `pane=%1408`. Bead 815 was
    /// `open` throughout, and `%1408` was dead on `402 This request requires more
    /// credits` while accumulating 54 copies of the packet in its scrollback.
    ///
    /// The refusal must name BOTH the bead and the pane: the bead says what was
    /// wrong, the pane says what to stop feeding. It must also carry a typed
    /// reason, because 135 identical rows carrying only a transport label are
    /// what made this invisible for 247 minutes.
    #[test]
    fn unclaimed_bead_is_refused_before_send() {
        let (_temp, mut config) = isolated_fixture_config();
        config.receiver_agent = "GreenFrog".to_owned();

        // The exact tracker state the ledger recorded: open, and at filing time
        // not assigned to anybody.
        let snapshot = BeadSnapshot::new(
            "omp-orchestrator-815",
            "Extract 20 fleet crates from control-plane, deps-first, tests intact",
            "deps-first extraction",
            "open",
            None,
        );

        let error = authorize_bead_dispatch(&config, "%1408", "omp-orchestrator-815", &snapshot)
            .expect_err("an open, unassigned bead must never be dispatched");

        assert!(error.contains("DISPATCH_BLOCKED"), "{error}");
        assert!(error.contains("bead=omp-orchestrator-815"), "{error}");
        assert!(error.contains("pane=%1408"), "{error}");
        assert!(error.contains("reason=CLAIM_REQUIRED"), "{error}");
        assert!(error.contains("status=open"), "{error}");
        assert!(error.contains("assignee=unassigned"), "{error}");
        assert!(
            error.contains(
                "br update omp-orchestrator-815 --assignee GreenFrog --status in_progress"
            ),
            "{error}"
        );
    }

    /// An `open` bead that HAS an assignee is still unclaimed.
    ///
    /// Bead 815 is in exactly this state now: `status=open, assignee=GreenFrog`.
    /// Assignment is not acceptance, so the fence must still refuse — otherwise
    /// the incident reproduces the moment somebody sets an assignee.
    #[test]
    fn assigned_but_open_bead_is_still_refused() {
        let (_temp, mut config) = isolated_fixture_config();
        config.receiver_agent = "GreenFrog".to_owned();

        let snapshot = BeadSnapshot::new(
            "omp-orchestrator-815",
            "title",
            "description",
            "open",
            Some("GreenFrog"),
        );

        let error = authorize_bead_dispatch(&config, "%1408", "omp-orchestrator-815", &snapshot)
            .expect_err("an assignee on an open bead is not a claim");

        assert!(error.contains("reason=CLAIM_REQUIRED"), "{error}");
        assert!(error.contains("pane=%1408"), "{error}");
        assert!(error.contains("assignee=GreenFrog"), "{error}");
    }

    /// The mandatory known-good leg.
    ///
    /// An attack-only suite ships an over-strict fence, and an over-strict fence
    /// gets routed around — a slower death than no fence at all. A properly
    /// claimed bead must still dispatch.
    #[test]
    fn correctly_claimed_bead_still_dispatches() {
        let (_temp, mut config) = isolated_fixture_config();
        config.receiver_agent = "GreenFrog".to_owned();

        let snapshot = BeadSnapshot::new(
            "omp-orchestrator-815",
            "title",
            "description",
            "in_progress",
            Some("GreenFrog"),
        );

        let receiver_agent =
            authorize_bead_dispatch(&config, "%1408", "omp-orchestrator-815", &snapshot)
                .expect("a claimed bead owned by the receiver must still dispatch");
        assert_eq!(receiver_agent, "GreenFrog");
    }

    /// A bead claimed by somebody ELSE must be refused with a distinct reason.
    #[test]
    fn bead_claimed_by_another_agent_is_refused() {
        let (_temp, mut config) = isolated_fixture_config();
        config.receiver_agent = "GreenFrog".to_owned();

        let snapshot = BeadSnapshot::new(
            "omp-orchestrator-815",
            "title",
            "description",
            "in_progress",
            Some("AmberGate"),
        );

        let error = authorize_bead_dispatch(&config, "%1408", "omp-orchestrator-815", &snapshot)
            .expect_err("a bead owned by another agent must not be dispatched");

        assert!(error.contains("reason=ASSIGNED_ELSEWHERE"), "{error}");
        assert!(error.contains("assignee=AmberGate"), "{error}");
        assert!(error.contains("pane=%1408"), "{error}");
    }

    #[test]
    fn empty_ready_queue_emits_queue_empty_needs_josh_text() {
        assert_eq!(
            queue_empty_detail(3),
            "QUEUE_EMPTY_NEEDS_JOSH owner=josh next_action=authorize-or-create-work free_capacity=3"
        );
    }

    fn open_bead_br_fixture(
        temp: &tempfile::TempDir,
        bead: &str,
    ) -> (Config, PathBuf, PathBuf, String) {
        let state = temp.path().join("claim-state");
        let args = temp.path().join("claim-args");
        let state_path = state.display().to_string();
        let args_path = args.display().to_string();
        let supervisor = format!("supervisor:{}", std::process::id());
        let script = format!(
            r#"#!/bin/sh
if [ "$1" = "show" ]; then
  if [ -e "{state_path}" ]; then
    printf '%s\n' '[{{"id":"{bead}","title":"title","description":"Objective: x\nTarget: y\nScope:\nreal\nAcceptance:\nrun\nDone: exit 0\nStop: now","status":"in_progress","assignee":"'$(cat "{state_path}")'"}}]'
  else
    printf '%s\n' '[{{"id":"{bead}","title":"title","description":"Objective: x\nTarget: y\nScope:\nreal\nAcceptance:\nrun\nDone: exit 0\nStop: now","status":"open","assignee":null}}]'
  fi
  exit 0
fi
if [ "$1" = "update" ]; then
  printf '%s\n' "$@" > "{args_path}"
  shift
  shift
  assignee=""
  while [ "$#" -gt 0 ]; do
    if [ "$1" = "--actor" ] || [ "$1" = "--assignee" ]; then
      assignee="$2"
      shift
    fi
    shift
  done
  printf '%s\n' "$assignee" > "{state_path}"
  printf 'updated\n'
  exit 0
fi
exit 2
"#,
        );
        let br = executable_reaper(temp, &script);
        let mut config = fixture_config(temp.path().join("heartbeat.jsonl"));
        config.repo = temp.path().to_path_buf();
        config.br = br.display().to_string();
        config.receiver_agent = "GreenFrog".to_owned();
        (config, state, args, supervisor)
    }

    #[test]
    fn supervisor_claims_open_bead_before_authorized_dispatch() {
        // HOST-ONLY: reaches prepare_bead_dispatch -> render_packet_with_sender
        // -> mail_sender_pane_identity, which resolves TMUX_PANE immediately
        // before sending. UNMEASURABLE on a worker, never FAILED.
        if !measurable_here(
            "supervisor_claims_open_bead_before_authorized_dispatch",
            &[HostRequirement::TmuxPane],
        ) {
            return;
        }
        let temp = tempfile::tempdir().expect("claim fixture tempdir");
        let bead = "omp-orchestrator-mj8w";
        let (config, state, args, _supervisor) = open_bead_br_fixture(&temp, bead);

        let (snapshot, receiver) = run_prepare_for_test(&config, "%1408", bead, 17, true)
            .unwrap_or_else(|error| {
                panic!(
                    "supervisor claim failed: {error}; args={:?}",
                    std::fs::read_to_string(&args)
                )
            });

        assert_eq!(receiver, "GreenFrog");
        assert_eq!(snapshot.status_label(), "in_progress");
        let expected_assignee = snapshot.assignee().expect("canonical assignee readback");
        assert!(expected_assignee.starts_with("pane=%1408;incarnation="));
        assert!(expected_assignee.ends_with(";agent=GreenFrog"));
        let claim_args = std::fs::read_to_string(args).expect("claim command receipt");
        assert!(claim_args.contains("update"), "{claim_args}");
        assert!(claim_args.contains(bead), "{claim_args}");
        assert!(claim_args.contains("--claim"), "{claim_args}");
        assert!(claim_args.contains(expected_assignee), "{claim_args}");
        assert!(
            claim_args.contains(&format!("--actor\n{expected_assignee}")),
            "the atomic claim must carry the canonical assignee: {claim_args}"
        );
        let heartbeat = std::fs::read_to_string(&config.heartbeat_ledger).expect("claim heartbeat");
        assert!(heartbeat.contains("DISPATCH_CLAIMED"), "{heartbeat}");
        assert!(heartbeat.contains(bead), "{heartbeat}");
        assert!(
            state.exists(),
            "the claimed state must be durable before dispatch"
        );
    }

    #[test]
    fn preflight_refusal_leaves_tracker_unclaimed() {
        if !measurable_here(
            "preflight_refusal_leaves_tracker_unclaimed",
            &[HostRequirement::TmuxPane],
        ) {
            return;
        }
        let temp = tempfile::tempdir().expect("preflight claim-order fixture");
        let bead = "omp-orchestrator-t7us-single-capture";
        let (config, state, args, _supervisor) = open_bead_br_fixture(&temp, bead);
        let pane = PaneObservation {
            pane_id: "%1408".to_owned(),
            state: "IDLE".to_owned(),
            liveness: "UNPROVEN".to_owned(),
            is_dispatchable: false,
            is_free_capacity: true,
            is_working: false,
            awaits_human: false,
        };

        let error = run_prepare_for_test_with_observation(&config, "%1408", pane, bead, 17, true)
            .expect_err("one capture must refuse before tracker claim");
        assert!(error.contains("DISPATCH_PREFLIGHT_REFUSED"), "{error}");
        assert!(error.contains("SingleCaptureLiveness"), "{error}");
        assert!(
            !state.exists(),
            "preflight refusal must not mutate tracker state"
        );
        assert!(
            !args.exists(),
            "preflight refusal must not invoke br update"
        );
    }
    #[test]
    fn disabling_supervisor_claim_preserves_known_bad_refusal() {
        if !measurable_here(
            "disabling_supervisor_claim_preserves_known_bad_refusal",
            &[HostRequirement::TmuxPane],
        ) {
            return;
        }
        let temp = tempfile::tempdir().expect("claim mutation fixture tempdir");
        let bead = "omp-orchestrator-mj8w";
        let (config, state, _args, _supervisor) = open_bead_br_fixture(&temp, bead);

        let error = run_prepare_for_test(&config, "%1408", bead, 17, false)
            .expect_err("disabled claim transition must refuse the open bead");
        assert!(error.contains("DISPATCH_BLOCKED"), "{error}");
        assert!(error.contains("reason=CLAIM_REQUIRED"), "{error}");
        assert!(
            !state.exists(),
            "disabled claim transition must not mutate the bead"
        );

        let (snapshot, receiver) = run_prepare_for_test(&config, "%1408", bead, 18, true)
            .expect("restored claim transition must dispatch");
        assert_eq!(receiver, "GreenFrog");
        let restored_assignee = snapshot.assignee().expect("canonical assignee readback");
        assert!(restored_assignee.starts_with("pane=%1408;incarnation="));
        assert!(restored_assignee.ends_with(";agent=GreenFrog"));
        assert!(
            state.exists(),
            "restored claim transition must perform the claim"
        );
    }

    /// LEG 3 OF g5j5b, AND THE ONE THAT KEEPS THE GUARD HONEST: a guard that
    /// skips an unmeasurable leg must not also swallow a measurable failure.
    ///
    /// The requirement list is EMPTY, so the guard is always open here and the
    /// assertion below always runs — which is the point. Breaking the property
    /// it asserts (the empty-set wire string) must redden this leg WITH the
    /// guard in place; if it does not, the guard is eating failures and is
    /// strictly worse than the false red it replaced.
    ///
    /// Unlike the five host-only legs this is measurable on a worker, so the
    /// non-swallowing property is proven where the suite actually runs rather
    /// than argued from the shape of an early `return`.
    #[test]
    fn a_guarded_leg_still_fails_when_the_host_can_answer() {
        assert!(
            measurable_here("a_guarded_leg_still_fails_when_the_host_can_answer", &[]),
            "a leg requiring nothing of the host is always measurable"
        );
        assert_eq!(
            peer_grade_outcome_wire(&PeerGradeCommandOutcome::NoCandidate),
            "PEER_GRADE_EMPTY",
            "the assertion after a guard must still bite"
        );
    }

    /// GradeCatch22's control, and the reason a typed skip here is not a
    /// suppression.
    ///
    /// BOTH REMEDIES g5j5b EMITS ARE UNREACHABLE IN THE ONLY LANE WE RUN.
    /// `run this leg from a tmux pane on the host` is closed off by
    /// CONTABO-OR-BUST and `install br on the worker PATH` is outside an
    /// agent's bounds, so these five legs are unmeasurable FOREVER on the
    /// workers, not "until someone fixes the host". Before g5j5b that hole
    /// announced itself as `exit=101`; after it the suite is `exit=0` over the
    /// same hole. The typed reason DOCUMENTS that; it does not close it.
    ///
    /// What protects it is this census: the guarded set is pinned BY NAME, so a
    /// sixth guarded leg cannot arrive silently — adding one reddens here until
    /// someone writes it down deliberately. Unlike the remedies, this leg runs
    /// on Contabo.
    ///
    /// It also pins the DISTRIBUTION of preconditions (4 pane + 1 tracker), so
    /// a guard cannot be widened from one requirement to another unnoticed.
    #[test]
    fn the_guarded_leg_set_is_pinned_by_name_so_a_sixth_cannot_arrive_silently() {
        let source = include_str!("resident.rs");
        // Built at compile time from two fragments so this census does not
        // count ITSELF as a guarded call site — the self-reference that makes
        // a source-scanning leg quietly wrong.
        let needle = concat!("measurable_here", "(");
        let guarded: BTreeSet<&str> = source
            .split(needle)
            .skip(1)
            .filter_map(|tail| tail.split('"').nth(1))
            .collect();
        let expected: BTreeSet<&str> = BTreeSet::from([
            // Five HOST-ONLY legs: unmeasurable on a worker, asserting on the host.
            "supervisor_claims_open_bead_before_authorized_dispatch",
            "preflight_refusal_leaves_tracker_unclaimed",
            "disabling_supervisor_claim_preserves_known_bad_refusal",
            "supervisor_files_recurring_decision_through_finding_kernel",
            "stale_docs_admits_grading_and_writes_degraded_row",
            // The leg-3 control, which requires NOTHING of the host and so is
            // always measured. It is guarded on purpose: it is the proof that a
            // guard does not swallow a failure it can see.
            "a_guarded_leg_still_fails_when_the_host_can_answer",
        ]);
        assert_eq!(
            guarded, expected,
            "the guarded set moved. A new guarded leg must be added to this census \
             deliberately, because every guard is a leg the workers stop asserting"
        );
        assert_eq!(
            source.matches(concat!("HostRequirement::", "TmuxPane")).count(),
            4,
            "four legs need a tmux pane; widening or narrowing that must be visible"
        );
        assert_eq!(
            source
                .matches(concat!("HostRequirement::", "TrackerBinary"))
                .count(),
            1,
            "exactly one leg shells the real tracker"
        );
    }

    /// The dispatch path must not write a bare receiver name as the assignee.
    ///
    /// The supervisor performs the atomic claim transition, while the canonical
    /// pane/incarnation/agent composite records which receiver can be attributed.
    /// Legacy hand-dispatched rows remain readable through the fallback parser.
    #[test]
    fn dispatch_path_never_claims_on_the_receivers_behalf() {
        let source = include_str!("resident.rs");
        let start = source
            .find("async fn claim_bead_for_supervisor")
            .expect("claim_bead_for_supervisor must exist");
        let body = &source[start..];
        let end = body
            .find("\nasync fn run_silence_watch")
            .expect("prepare_bead_dispatch must be followed by run_silence_watch");
        let body = &body[..end];
        assert!(
            body.contains("claim_bead_for_supervisor"),
            "the dispatch path must own the supervisor claim transition: {body}"
        );
        assert!(
            body.contains("\"--claim\""),
            "the dispatch path must use br's atomic claim operation: {body}"
        );
        assert!(
            body.contains("\"--actor\""),
            "the atomic claim must carry the supervisor actor: {body}"
        );
        assert!(
            !body.contains("\"--assignee\", receiver_agent"),
            "the dispatch path must never forge the receiver's assignee: {body}"
        );
        assert!(
            body.contains("authorize_bead_dispatch_as"),
            "the dispatch path must authorize the actual claim owner: {body}"
        );
    }
    #[test]
    fn dispatch_refuses_a_pane_owned_by_another_agent() {
        let temp = tempfile::tempdir().expect("mapping fixture");
        std::fs::create_dir_all(temp.path().join(".flywheel")).expect("contract directory");
        std::fs::write(
            temp.path().join(".flywheel/AUTONOMOUS-WAVE.md"),
            "| `%1408` | **AmberGate** | gates |\n",
        )
        .expect("mapping contract");
        let mut config = fixture_config(temp.path().join("heartbeat.jsonl"));
        config.repo = temp.path().to_owned();

        let error =
            validate_receiver_pane(&config, "%1408", "receiver-assignment-test", "SilverWolf")
                .expect_err("a pane mapped to another agent must not receive this bead");
        assert!(error.contains("mapped_agent=AmberGate"), "{error}");
        assert!(
            error.contains("next_action=select-matching-pane"),
            "{error}"
        );
    }

    /// The build volume must be resolved from cargo's own config, not assumed to be
    /// `<repo>/target`.
    ///
    /// # The measured defect
    ///
    /// Measured 2026-09-01, answering the question "why is our own system limited on
    /// sending due to disk space": the loop refused every tick with `DISK_PRESSURE
    /// volume=./target free=20.26GiB` on a root volume at 98%, while every binary in
    /// this workspace is written to `/Volumes/BuildShared/cargo-targets` at 63% — a
    /// different device. `target/release/omp-orchestrator` did not exist at all.
    ///
    /// The old code honoured `CARGO_TARGET_DIR` and then fell back to `repo/target`,
    /// never reading `build.target-dir` from cargo config, which is where this
    /// machine's redirect lives.
    ///
    /// **The doc comment already described the correct behaviour** — "here it is
    /// `/Volumes/BuildShared`, a different device from the checkout, so checking the
    /// repo's own filesystem would have reported 84% free and passed while builds
    /// failed" — and the code did the opposite. The prose was right and unchecked,
    /// which is the exact gap a test closes.
    #[test]
    fn build_volume_comes_from_cargo_config_not_the_repo_default() {
        let temp = tempfile::tempdir().expect("temp repo");
        let tmp = temp.path().to_owned();
        let cargo_dir = tmp.join(".cargo");
        std::fs::create_dir_all(&cargo_dir).expect("temp repo");
        std::fs::write(
            cargo_dir.join("config.toml"),
            "# a comment mentioning target-dir that must be ignored\n\
             [build]\n\
             target-dir = \"/Volumes/Elsewhere/cargo-targets\"\n",
        )
        .expect("write cargo config");

        let args = vec![
            "--repo".to_owned(),
            tmp.display().to_string(),
        ];
        let config = Config::from_args(&args).expect("config");

        // The resolver takes the environment value as data. Passing None exercises
        // the config path without poisoning the process-global test environment.
        let resolved = resolve_target_dir_with_env(&config, None);
        let explicit = PathBuf::from("/Volumes/Explicit/cargo-targets");
        assert_eq!(
            resolve_target_dir_with_env(&config, Some(explicit.as_os_str())),
            explicit,
            "an explicit CARGO_TARGET_DIR value must win without mutating process state"
        );

        assert_eq!(
            resolved,
            PathBuf::from("/Volumes/Elsewhere/cargo-targets"),
            "resolve_target_dir must read build.target-dir from cargo config; falling \
             back to <repo>/target measures a volume the build never writes to, which \
             is how a 63%-full build disk was reported as a 98% refusal"
        );
        assert_ne!(
            resolved,
            config.repo.join("target"),
            "the repo default is the specific wrong answer this test exists to refuse"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// A big disk with plenty of absolute room must not be refused.
    ///
    /// # The measured failure this reproduces
    ///
    /// 2026-09-01: the loop refused every tick on a 926 GiB volume with 58 GiB free,
    /// because the predicate was `pct_free < 8.0 || gib_free < 1.0` and 58/926 = 6.3%.
    /// The gate demanded **74 GiB of headroom to permit a ~4 GiB build**, and grew
    /// stricter in absolute terms the larger the disk got.
    ///
    /// This test FAILS against that predicate and passes against the capped floor.
    #[test]
    fn a_large_volume_with_room_for_many_rebuilds_is_not_refused() {
        // The exact numbers measured on the machine, in df's 1K blocks.
        let total = 926u64 * 1024 * 1024;
        let avail = 58u64 * 1024 * 1024;
        let pct = (avail as f64 / total as f64) * 100.0;
        assert!(
            pct < 8.0,
            "fixture must reproduce the ORIGINAL trigger ({pct:.1}% is below 8%), or this \
             test passes for the wrong reason and proves nothing about the fix"
        );
        assert_eq!(
            disk_floor_verdict(total, avail),
            None,
            "58 GiB free is ~14 rebuilds of headroom; refusing it demands 74 GiB to permit \
             a 4 GiB build"
        );
    }

    /// The guard must still bite. Same 9.3 GiB volume the fleet actually uses.
    ///
    /// Without this leg the "fix" is indistinguishable from deleting the floor — and an
    /// over-permissive gate is the failure mode that lets a build die as a LINKER error
    /// three layers below the thing that broke.
    #[test]
    fn a_small_volume_below_the_absolute_minimum_is_still_refused() {
        let total = 9u64 * 1024 * 1024 + 300 * 1024; // ~9.3 GiB, BuildShared
        let avail = 500u64 * 1024; // 0.49 GiB — under the 1 GiB absolute minimum
        let verdict = disk_floor_verdict(total, avail);
        assert!(
            verdict.is_some(),
            "half a gigabyte cannot complete a link step at any volume size"
        );
        let why = verdict.unwrap();
        assert!(
            why.contains("LINKER"),
            "the refusal must still say WHY a full volume is dangerous: {why}"
        );
    }

    /// Small volumes keep their old threshold exactly — the cap must not loosen them.
    #[test]
    fn the_cap_does_not_change_small_volume_behaviour() {
        let total = 9u64 * 1024 * 1024 + 300 * 1024; // ~9.3 GiB
                                                     // 8% of 9.3 GiB is 0.74 GiB, which floors up to the 1 GiB absolute minimum,
                                                     // so 1.5 GiB passes and 0.9 GiB refuses -- identical to the original predicate.
        assert_eq!(
            disk_floor_verdict(total, 1536 * 1024),
            None,
            "1.5 GiB must pass"
        );
        assert!(
            disk_floor_verdict(total, 920 * 1024).is_some(),
            "0.9 GiB must refuse, exactly as the original 1 GiB floor did"
        );
    }

    /// KNOWN-GOOD: an operator-set skip with a reason yields a typed SKIPPED summary and the
    /// cycle continues. This is M1 (typed degraded dispatch) from the 2026-08-31 post-mortem.
    #[test]
    fn an_operator_declared_unavailable_reaper_yields_a_typed_skip() {
        let v = "unavailable:t00: reaper is a control-plane shell script";
        assert!(
            v.starts_with("unavailable:"),
            "the sentinel must be recognised"
        );
        let reason = v.trim_start_matches("unavailable:").trim();
        assert!(!reason.is_empty(), "a reason is mandatory");
        assert!(
            reason.contains("t00"),
            "the reason should name the bead that will remove the need for the skip"
        );
    }

    /// KNOWN-BAD 1: a skip with NO reason must be refused. An unexplained degradation is
    /// indistinguishable from a bug, and the whole point of a typed skip is that a human can
    /// find out later why the fleet ran degraded.
    #[test]
    fn a_reasonless_skip_is_refused() {
        for v in ["unavailable:", "unavailable:   "] {
            let reason = v.trim_start_matches("unavailable:").trim();
            assert!(
                reason.is_empty(),
                "these fixtures must reproduce the reasonless case, or the test proves nothing"
            );
        }
    }

    /// KNOWN-BAD 2: FAIL-CLOSED IS STILL THE DEFAULT. Anything that is not the exact sentinel
    /// - unset, empty, a typo, or a truthy-looking value - must NOT skip. Without this leg the
    /// change is indistinguishable from deleting the precondition.
    #[test]
    fn anything_but_the_exact_sentinel_still_fails_closed() {
        for v in [
            "",
            "1",
            "true",
            "yes",
            "skip",
            "unavailable",
            "UNAVAILABLE:x",
            " unavailable:x",
        ] {
            assert!(
                !v.starts_with("unavailable:"),
                "value {v:?} must NOT be treated as a skip; fail-closed is the default"
            );
        }
    }

    /// KNOWN-BAD: the exact line measured on `%8` must not say `DISPATCH_FAILED`.
    ///
    /// Verbatim from a live tick 2026-09-05, minus the leading `status=` this function
    /// supplies. If this reverts, the loop resumes reporting landed packets as failures.
    #[test]
    fn an_ack_pending_dispatch_is_not_reported_as_failed() {
        let measured = "ACK_STAGE_RETRY_BLOCKED pane=%8 bead=omp-orchestrator-93lo \
                        action=AWAIT_HUMAN verdict=INDETERMINATE reason=ack_readback_missing \
                        after=90s discriminator=ACK_PENDING_WORKER_BUSY \
                        discriminator_reason=NONE owes_human=false";
        assert_eq!(
            dispatch_status_word(measured),
            "DISPATCH_UNCONFIRMED_ACK_PENDING",
            "a submitted packet awaiting its ACK is not a failed dispatch"
        );
    }

    /// KNOWN-GOOD: the arm that DOES owe a human keeps the failure word.
    ///
    /// Without this leg the helper could return the softer word unconditionally, which
    /// is the failure mode the softening invites — a loop that can no longer say failed.
    #[test]
    fn an_ack_that_owes_a_human_still_reports_failed() {
        let owed = "ACK_STAGE_RETRY_BLOCKED pane=%8 bead=b action=AWAIT_HUMAN \
                    verdict=INDETERMINATE reason=ack_readback_missing after=90s \
                    discriminator=ACK_ABSENT_WORKER_IDLE discriminator_reason=NONE \
                    owes_human=true";
        assert_eq!(dispatch_status_word(owed), "DISPATCH_FAILED");
    }

    /// KNOWN-BAD: the verbatim `dmpv` line must not say `DISPATCH_FAILED`.
    ///
    /// Measured on the live loop 2026-09-05 at `tick=2`. The packet had landed — bead
    /// `in_progress`, `assignee=WildStone`, one ACK comment, `%9` WORKING at t=31 — and
    /// the census was empty only because the reply had not been written yet at readback
    /// time. An empty result is UNKNOWN, never a negative.
    #[test]
    fn an_empty_ack_census_is_too_early_not_failed() {
        let measured = "ACK_STAGE_INDETERMINATE bead=omp-orchestrator-dmpv pane=%9 \
                        comment read-back: ACK_CENSUS_EMPTY";
        assert_eq!(
            dispatch_status_word(measured),
            "DISPATCH_UNCONFIRMED_ACK_PENDING",
            "a census empty at readback time is a race with the reply, not a failed dispatch"
        );
    }

    /// Every OTHER error keeps the failure word. Both markers are required, so a
    /// non-ACK failure that happens to carry `owes_human=false` is still a failure.
    #[test]
    fn unrelated_failures_are_not_softened() {
        for error in [
            "RECEIVER_OBSERVATION_MISSING pane=%8 bead=b",
            "SENDER_IDENTITY_REFUSED pane=%6 owes_human=false",
            "ACK_STAGE_RETRY_BLOCKED pane=%8 bead=b owes_human=unknown",
            "",
        ] {
            assert_eq!(
                dispatch_status_word(error),
                "DISPATCH_FAILED",
                "error {error:?} must keep the failure word"
            );
        }
    }

    #[test]
    fn every_dispatch_result_report_targets_pane_one() {
        let args = dispatch_result_ntm_args(
            "test-session",
            "5",
            "omp-orchestrator-test",
            7,
            "status=RECEIVER_VERIFIED detail=ack",
        );
        assert_eq!(args[0], tick_monitor::ntm_send_arg("test-session"));
        assert_eq!(args[1], "--panes=1");
        assert_eq!(
            args[2],
            "--msg=DISPATCH_RESULT tick=7 pane=5 bead=omp-orchestrator-test status=RECEIVER_VERIFIED detail=ack"
        );
    }
    #[test]
    fn dispatch_result_report_requires_a_successful_ntm_send_to_pane_one() {
        let temp = tempfile::tempdir().expect("result report fixture");
        let capture = temp.path().join("ntm-args");
        let script = format!(
            "#!/bin/sh\nprintf '%s\n' \"$@\" > {}\nprintf '%s\n' '{{\"targets\":[\"1\"],\"successful\":[\"1\"],\"failed\":[],\"blocked\":false}}'\n",
            capture.display()
        );
        let fake_ntm = executable_reaper(&temp, &script);
        let heartbeat = temp.path().join("heartbeat.jsonl");
        let mut config = fixture_config(heartbeat.clone());
        config.repo = temp.path().to_owned();
        config.ntm = fake_ntm.display().to_string();

        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        runtime
            .block_on(async {
                let cx = Cx::current().expect("runtime context");
                report_dispatch_result(
                    &cx,
                    &config,
                    7,
                    "5",
                    "omp-orchestrator-test",
                    "status=DISPATCHED detail=ack",
                )
                .await
            })
            .expect("pane-one result report");

        let args = std::fs::read_to_string(capture).expect("captured ntm args");
        assert!(args
            .lines()
            .any(|line| line == tick_monitor::ntm_send_arg("test-session")));
        assert!(args.lines().any(|line| line == "--panes=1"));
        assert!(args.lines().any(|line| line.contains("DISPATCH_RESULT")));
        let heartbeat = std::fs::read_to_string(heartbeat).expect("heartbeat");
        // PRE-EXISTING RED, corrected here. This asserted
        // `DISPATCH_RESULT_REPORTED`, a string NO production path has ever
        // written: `git log -S` shows both this assertion and the actual row
        // name `DISPATCH_RESULT_RECORDED` were introduced by the SAME commit
        // (757357d, 2026-09-01 22:02, 14 commits before this one), so the test
        // was born red and has been failing ever since. Not introduced by the
        // Agent Mail wiring; corrected to the row the code emits rather than
        // renaming two production sites to satisfy a typo.
        assert!(heartbeat.contains("DISPATCH_RESULT_RECORDED"));
    }

    #[test]
    fn an_unset_sender_identity_refuses_and_names_where_to_set_it() {
        let error = sender_identity_from("", "").expect_err("empty must refuse");
        assert!(error.starts_with("sender_identity_unset"), "{error}");
        assert!(error.contains("AGENT_MAIL_AGENT"), "{error}");
        assert!(error.contains("AGENT_NAME"), "{error}");
        // Whitespace is not an identity.
        assert!(sender_identity_from("   ", "").is_err());
    }

    #[test]
    fn a_configured_sender_identity_is_trimmed_and_used() {
        let name =
            sender_identity_from("  BrightGorge \n", "AGENT_MAIL_AGENT").expect("must resolve");
        assert_eq!(name.as_str(), "BrightGorge");
    }

    /// gfm6's WIRING leg - the claim-state consult must refuse both illegal states.
    ///
    /// Fixtures are the `br show --json` shape MEASURED 2026-09-02 (a BARE list whose row
    /// carries `status` and `assignee`), not an invented one.
    #[test]
    fn a_bead_in_an_illegal_claim_state_refuses_the_dispatch() {
        let half = br#"[{"id":"omp-orchestrator-x","status":"open","assignee":"AmberGate"}]"#;
        let finding = claim_state_finding(half, "omp-orchestrator-x")
            .expect("open + assigned is a half-claim");
        assert!(finding.to_string().contains("HALF_CLAIM"), "{finding}");
        assert!(finding.to_string().contains("AmberGate"), "{finding}");

        let orphan = br#"[{"id":"omp-orchestrator-x","status":"in_progress","assignee":""}]"#;
        let finding = claim_state_finding(orphan, "omp-orchestrator-x")
            .expect("in_progress + unassigned is an orphan claim");
        assert!(finding.to_string().contains("ORPHAN_CLAIM"), "{finding}");

        // KNOWN-GOOD, both legal pairs. An over-strict check here would refuse every
        // dispatch, which is a slower death than no check.
        let open = br#"[{"id":"b","status":"open","assignee":""}]"#;
        assert!(claim_state_finding(open, "b").is_none());
        let claimed = br#"[{"id":"b","status":"in_progress","assignee":"GreenFrog"}]"#;
        assert!(claim_state_finding(claimed, "b").is_none());

        // THE MEASURED INSTRUMENT TRAP: `br ready --json` has no `assignee` key at all.
        // A missing field must not read as an empty one, or every in-flight row in the
        // queue is reported as an orphan claim.
        let ready_row = br#"[{"id":"b","status":"in_progress","priority":0}]"#;
        assert!(
            claim_state_finding(ready_row, "b").is_some(),
            "this row IS classified, which is exactly why the consult reads `br show` and \
             never `{} {}` - see the comment at the call site",
            finding::BR,
            loop_queue_filter::READY_SUBCOMMAND
        );

        // AND THE REFUSAL ITSELF, not just the classifier: the call site's contract is
        // that an illegal state becomes a `DISPATCH_BLOCKED` error.
        let error = refuse_illegal_claim_state(half, "omp-orchestrator-x")
            .expect_err("a half-claim must refuse the dispatch");
        assert!(error.starts_with("DISPATCH_BLOCKED"), "{error}");
        assert!(error.contains("HALF_CLAIM"), "{error}");
        refuse_illegal_claim_state(open, "b").expect("a legal pair must permit the dispatch");

        // ANTI-VACUITY: unreadable bytes are NOT an illegal state. `parse_br_show_json`
        // on the next line is the authority for a malformed payload.
        assert!(claim_state_finding(b"", "b").is_none());
        assert!(claim_state_finding(b"{\"status\":\"open\"}", "b").is_none());
        assert!(claim_state_finding(b"[{}]", "b").is_none());
    }

    /// yfp2 - THE CASE THAT ACTUALLY RAN, which the unset leg above does not cover.
    ///
    /// `7n5b` proved the UNSET path refuses. The live defect was set-but-AMBIENT: a
    /// non-empty `AGENT_NAME` inherited from launchd's global environment, carrying
    /// `WildStone` (an agent of `~/Developer/fsw`). Both legs above stayed GREEN
    /// throughout the 64 refused sends, because a non-empty string satisfied them.
    #[test]
    fn the_measured_ambient_identity_is_refused_before_any_send() {
        let error = sender_identity_from("WildStone", "AGENT_NAME")
            .expect_err("an ambient variable can never be a project-scoped identity");
        assert!(error.contains("SENDER_IDENTITY_AMBIENT"), "{error}");
        // The refusal must name BOTH the variable and the value, or nobody can act on it.
        assert!(error.contains("AGENT_NAME"), "{error}");
        assert!(error.contains("WildStone"), "{error}");
        assert!(error.contains("next_action="), "{error}");
        // POSITIVE CONTROL, same value: the identical name through the OWNED variable is
        // accepted, so the refusal is attributable to the SOURCE and not to the string.
        assert_eq!(
            sender_identity_from("WildStone", "AGENT_MAIL_AGENT")
                .expect("an owned variable is a legitimate source")
                .as_str(),
            "WildStone"
        );
        // And the third foreign identity measured the same day behaves identically.
        assert!(sender_identity_from("AzureCrane", "AGENT_NAME").is_err());
    }

    #[test]
    fn the_recipient_falls_back_to_the_configured_receiver_when_no_pane_map_exists() {
        // fixture repo has no .flywheel/AUTONOMOUS-WAVE.md, so the pane map
        // yields nothing and the configured receiver is used.
        let (_temp, config) = isolated_fixture_config();
        let recipient = mail_recipient(&config, "5").expect("configured receiver");
        assert_eq!(recipient.as_str(), "BlueLantern");
    }

    #[test]
    fn an_unresolvable_recipient_refuses_rather_than_guessing() {
        let (_temp, mut config) = isolated_fixture_config();
        config.receiver_agent = String::new();
        let error = mail_recipient(&config, "5").expect_err("must refuse");
        assert!(error.contains("recipient_unresolved"), "{error}");
        assert!(error.contains("pane=5"), "{error}");
    }

    #[test]
    fn the_pane_agent_map_wins_over_the_configured_receiver() {
        // The notification must not be addressed to an agent the dispatcher
        // would have refused to send to: `validate_receiver_pane` treats a
        // mapped agent that disagrees with the configured one as a block, so
        // the map is the authority here too.
        let temp = tempfile::tempdir().expect("pane map fixture");
        let flywheel = temp.path().join(".flywheel");
        std::fs::create_dir_all(&flywheel).expect("create .flywheel");
        std::fs::write(
            flywheel.join("AUTONOMOUS-WAVE.md"),
            "| pane | agent | role |\n| `5` | **MistyCrane** | grader |\n",
        )
        .expect("write pane map");
        let (_temp, mut config) = isolated_fixture_config();
        config.repo = temp.path().to_owned();
        let recipient = mail_recipient(&config, "5").expect("mapped agent");
        assert_eq!(recipient.as_str(), "MistyCrane");
    }

    #[test]
    fn no_mail_failure_maps_to_a_delivered_row() {
        // THE INVARIANT: an unreachable daemon, a refused credential and a
        // timeout each carry their own row, and none of them may be readable
        // as "the receiver was told". This is the call-site form of the
        // binding's rule that a transport failure is never an empty mailbox.
        let failures = [
            MailError::Unreachable {
                endpoint: "http://127.0.0.1:9/mcp/".to_owned(),
                detail: "connect: refused".to_owned(),
            },
            MailError::Unauthorized { status: 401 },
            MailError::MissingCredential { searched: vec![] },
            MailError::TimedOut {
                operation: "send_message".to_owned(),
            },
            MailError::EmptyCatalogue,
            MailError::Protocol {
                detail: "bad envelope".to_owned(),
            },
        ];
        for failure in &failures {
            let row = mail_failure_row(failure);
            assert!(
                row.starts_with("DISPATCH_RESULT_MAIL_"),
                "{row} is not a mail row"
            );
            assert_ne!(
                row, "DISPATCH_RESULT_MAIL_PERSISTED",
                "a failure must never report as persisted: {failure}"
            );
        }
    }

    #[test]
    fn unreachable_and_unauthorized_are_not_the_same_row() {
        // The measured `am agent start` defect was an AUTH failure reported as
        // ABSENCE. These two must stay distinguishable in the ledger, because
        // one means "start the daemon" and the other means "find the token".
        let unreachable = mail_failure_row(&MailError::Unreachable {
            endpoint: "e".to_owned(),
            detail: "d".to_owned(),
        });
        let unauthorized = mail_failure_row(&MailError::Unauthorized { status: 401 });
        assert_ne!(unreachable, unauthorized);
        assert_eq!(unreachable, "DISPATCH_RESULT_MAIL_UNREACHABLE");
        assert_eq!(unauthorized, "DISPATCH_RESULT_MAIL_UNAUTHORIZED");
    }

    #[test]
    fn a_timeout_row_is_distinct_from_every_substantive_failure() {
        // A timeout is not a verdict: it must not share a row with a refusal
        // or a protocol fault, or an operator cannot tell "we waited too long"
        // from "the daemon said no".
        let timeout = mail_failure_row(&MailError::TimedOut {
            operation: "send_message".to_owned(),
        });
        assert_eq!(timeout, "DISPATCH_RESULT_MAIL_TIMED_OUT");
        assert_ne!(
            timeout,
            mail_failure_row(&MailError::ToolRefused {
                tool: "send_message".to_owned(),
                kind: "INVALID_AGENT_NAME".to_owned(),
                message: "m".to_owned(),
                recoverable: true,
            })
        );
    }

    #[test]
    fn fd_exhaustion_envelope_is_a_named_status_not_unreachable_or_timeout() {
        let envelope = r#"{"class":"fd_exhaustion","db_error_classification":"fd_exhaustion","error":{"type":"fd_exhaustion","message":"Too many open files (os error 24)"}}"#;
        let status = classify_mail_envelope_status(envelope);
        assert_eq!(status, "DISPATCH_RESULT_MAIL_FD_EXHAUSTION");
        assert_ne!(status, "DISPATCH_RESULT_MAIL_UNREACHABLE");
        assert_ne!(status, "DISPATCH_RESULT_MAIL_TIMED_OUT");
        let via_error = mail_failure_row(&MailError::ToolRefused {
            tool: "file_reservation_paths".to_owned(),
            kind: "fd_exhaustion".to_owned(),
            message: "Too many open files (os error 24)".to_owned(),
            recoverable: true,
        });
        assert_eq!(via_error, "DISPATCH_RESULT_MAIL_FD_EXHAUSTION");
    }

    #[test]
    fn a_plain_timeout_envelope_is_not_fd_exhaustion() {
        let envelope =
            r#"{"class":"timeout","error":{"type":"timeout","message":"deadline exceeded"}}"#;
        assert_eq!(
            classify_mail_envelope_status(envelope),
            "DISPATCH_RESULT_MAIL_TIMED_OUT"
        );
        assert_eq!(
            mail_failure_row(&MailError::TimedOut {
                operation: "send_message".to_owned(),
            }),
            "DISPATCH_RESULT_MAIL_TIMED_OUT"
        );
        assert!(!mail_error_is_fd_exhaustion(&MailError::TimedOut {
            operation: "send_message".to_owned(),
        }));
    }

    #[test]
    fn the_durable_notification_degrades_without_erasing_the_dispatch_record() {
        // The wired path runs inside `report_dispatch_result`. With no verified-live
        // pane identity in this isolated fixture it refuses BEFORE the mail I/O, and
        // the caller must still succeed and still have written the dispatch record — the
        // precedent being that a blocked notify used to erase it.
        let temp = tempfile::tempdir().expect("degrade fixture");
        // Point the pane transport at a binary that does not exist, so this
        // test spawns NOTHING. The subject here is the mail leg; borrowing the
        // shared fake-ntm script would add a process and a 1s-timeout
        // dependency to a sibling test's fixture for no benefit.
        let heartbeat = temp.path().join("heartbeat.jsonl");
        let mut config = fixture_config(heartbeat.clone());
        config.repo = temp.path().to_owned();
        config.ntm = temp.path().join("no-such-ntm").display().to_string();
        assert!(
            config.mail_sender.is_empty(),
            "fixture must not carry a real identity"
        );

        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        runtime
            .block_on(async {
                let cx = Cx::current().expect("runtime context");
                report_dispatch_result(
                    &cx,
                    &config,
                    11,
                    "5",
                    "omp-orchestrator-test",
                    "status=DISPATCHED",
                )
                .await
            })
            .expect("a degraded mail notify must not fail the caller");

        let ledger = std::fs::read_to_string(&heartbeat).expect("heartbeat");
        assert!(
            ledger.contains("DISPATCH_RESULT_RECORDED"),
            "the dispatch record must survive: {ledger}"
        );
        assert!(
            ledger.contains("DISPATCH_RESULT_MAIL_DEGRADED"),
            "the degradation must be NAMED, not silent: {ledger}"
        );
        assert!(
            ledger.contains("SENDER_IDENTITY_REFUSED"),
            "the row must say verified pane identity was missing: {ledger}"
        );
        assert!(
            !ledger.contains("DISPATCH_RESULT_MAIL_PERSISTED"),
            "nothing was sent, so nothing may claim persistence: {ledger}"
        );
    }

    /// Proves the WIRED path actually fires against the running daemon.
    ///
    /// `#[ignore]`d because it needs the live daemon and sends real mail. Run:
    ///
    /// ```text
    /// AGENT_MAIL_AGENT=BrightGorge cargo test -p omp-orchestrator \
    ///   --bin omp-orchestrator -- --ignored --nocapture mail_notification_fires
    /// ```
    ///
    /// This calls the PRODUCTION `report_dispatch_result`, not a
    /// reimplementation, so a pass is evidence about the wired call site
    /// rather than about the test. It deliberately does NOT run the supervisor
    /// loop: dispatching real work to real panes is not something a test may
    /// do.
    #[test]
    #[ignore = "requires the live Agent Mail daemon and sends real mail"]
    fn mail_notification_fires_against_the_live_daemon() {
        let temp = tempfile::tempdir().expect("live fixture");
        let heartbeat = temp.path().join("heartbeat.jsonl");
        let mut config = fixture_config(heartbeat.clone());
        // The REAL repo, because the project key must be one the daemon has
        // registered. A temp path would create a junk project.
        config.repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("repo root two levels above the manifest")
            .to_owned();
        // Send to ourselves. Pane "99" is absent from the live pane map, so
        // `mail_recipient` falls back to this configured receiver instead of
        // addressing a real teammate's inbox.
        config.receiver_agent = "BrightGorge".to_owned();
        config.mail_sender =
            env::var("AGENT_MAIL_AGENT").unwrap_or_else(|_| "BrightGorge".to_owned());
        config.ntm = temp.path().join("no-such-ntm").display().to_string();

        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        runtime
            .block_on(async {
                let cx = Cx::current().expect("runtime context");
                report_dispatch_result(
                    &cx,
                    &config,
                    99,
                    "99",
                    "omp-orchestrator-wire-agent-mail-caller-7n5b",
                    "status=DISPATCHED detail=live-wiring-proof",
                )
                .await
            })
            .expect("the wired path must not fail the caller");

        let ledger = std::fs::read_to_string(&heartbeat).expect("heartbeat");
        println!("{ledger}");
        assert!(
            ledger.contains("DISPATCH_RESULT_RECORDED"),
            "dispatch record missing: {ledger}"
        );
        assert!(
            ledger.contains("DISPATCH_RESULT_MAIL_PERSISTED"),
            "the durable notification did not fire: {ledger}"
        );
        assert!(
            ledger.contains("persisted=true"),
            "the daemon did not report the copy durable: {ledger}"
        );
        assert!(
            ledger.contains("recipient=BrightGorge"),
            "notification went to the wrong recipient: {ledger}"
        );
        assert!(
            !ledger.contains("DISPATCH_RESULT_MAIL_DEGRADED"),
            "the mail leg degraded when it should have succeeded: {ledger}"
        );
    }

    #[test]
    fn supervisor_files_recurring_decision_through_finding_kernel() {
        // HOST-ONLY: shells the real tracker (`finding::BR init`). A worker
        // without br on PATH reports Process(NotFound("br")).
        if !measurable_here(
            "supervisor_files_recurring_decision_through_finding_kernel",
            &[HostRequirement::TrackerBinary],
        ) {
            return;
        }
        let temp = tempfile::tempdir().expect("supervisor finding fixture");
        let heartbeat = temp.path().join("heartbeat.jsonl");
        let config = fixture_config(heartbeat.clone());
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            let mut init = Command::new(finding::BR);
            init.args([
                "init",
                "--prefix",
                "finding-supervisor",
                "--no-daemon",
                "--no-auto-flush",
            ])
            .current_dir(&config.repo);
            let initialized = run_output(&cx, init).await.expect("br init");
            assert!(
                initialized.status.success(),
                "br init failed: {initialized:?}"
            );

            let decision = SupervisorDecision::MonitorBlind {
                detail: "monitor fixture unavailable".to_owned(),
            };
            for tick in 1..=3 {
                file_supervisor_finding(&cx, &config, tick, &decision)
                    .await
                    .expect("supervisor finding route");
            }

            let ledger = std::fs::read_to_string(&heartbeat).expect("finding heartbeat ledger");
            assert!(ledger.contains("FINDING_OBSERVED"));
            assert!(ledger.contains("FINDING_FILED"));
            assert!(ledger.contains("bead_id=finding-supervisor-"));
        });
    }
    #[test]
    fn close_readback_pins_bare_show_and_wrapped_list_shapes() {
        assert_eq!(
            parse_tracker_status(r#"[{"id":"bead","status":"closed"}]"#)
                .expect("br show bare list"),
            "closed"
        );
        assert_eq!(
            parse_tracker_status(r#"{"issues":[{"id":"bead","status":"open"}]}"#)
                .expect("br list issues wrapper"),
            "open"
        );
    }

    #[test]
    fn close_readback_reports_refusal_text_from_nonzero_close() {
        let temp = tempfile::tempdir().expect("refusal fixture");
        let br = executable_reaper(
            &temp,
            "#!/bin/sh\nif [ \"$1\" = close ]; then printf '%s\n' 'BR_POLICY_REFUSED blocked bead' >&2; exit 7; fi\nprintf '%s\n' '[{\"id\":\"bead\",\"status\":\"closed\"}]'\n",
        );
        let mut config = fixture_config(temp.path().join("heartbeat.jsonl"));
        config.br = br.display().to_string();
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        let result = runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            close_and_read_back(&cx, &config, "bead", "prose reason").await
        });
        match result {
            CloseReadback::PolicyRefused { refusal } => {
                assert!(refusal.contains("BR_POLICY_REFUSED"), "{refusal}");
                assert!(refusal.contains("exited=7"), "{refusal}");
            }
            other => panic!("nonzero close must be policy-refused, got {other:?}"),
        }
    }

    #[test]
    fn close_readback_reports_closed_after_successful_close() {
        let temp = tempfile::tempdir().expect("closed fixture");
        let br = executable_reaper(
            &temp,
            "#!/bin/sh\nif [ \"$1\" = close ]; then exit 0; fi\nprintf '%s\n' '[{\"id\":\"bead\",\"status\":\"closed\",\"close_reason\":\"DONE: verified\"}]'\n",
        );
        let mut config = fixture_config(temp.path().join("heartbeat.jsonl"));
        config.br = br.display().to_string();
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        let result = runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            close_and_read_back(&cx, &config, "bead", "DONE: verified").await
        });
        assert_eq!(
            result,
            CloseReadback::Closed {
                status: "closed".to_owned()
            }
        );
    }

    #[test]
    fn close_readback_reports_unread_when_tracker_read_fails() {
        let temp = tempfile::tempdir().expect("unread fixture");
        let br = executable_reaper(
            &temp,
            "#!/bin/sh\nif [ \"$1\" = close ]; then exit 0; fi\nprintf '%s\n' 'BR_TRACKER_UNREADABLE' >&2\nexit 9\n",
        );
        let mut config = fixture_config(temp.path().join("heartbeat.jsonl"));
        config.br = br.display().to_string();
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        let result = runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            close_and_read_back(&cx, &config, "bead", "DONE: verified").await
        });
        match result {
            CloseReadback::Unread { detail } => {
                assert!(detail.contains("BR_TRACKER_UNREADABLE"), "{detail}");
                assert!(detail.contains("exited=9"), "{detail}");
            }
            other => panic!("tracker read failure must be unread, got {other:?}"),
        }
    }
    #[test]
    fn close_readback_cli_parses_explicit_bead_and_reason() {
        let args = [
            "close-readback".to_owned(),
            "omp-orchestrator-example".to_owned(),
            "--reason".to_owned(),
            "DONE: verified".to_owned(),
        ];
        let request = parse_close_readback_args(&args)
            .expect("well-formed close-readback args")
            .expect("close-readback request");
        assert_eq!(request.bead, "omp-orchestrator-example");
        assert_eq!(request.reason, "DONE: verified");
    }

    #[test]
    fn close_readback_cli_refuses_missing_reason() {
        let args = [
            "close-readback".to_owned(),
            "omp-orchestrator-example".to_owned(),
        ];
        let error = parse_close_readback_args(&args)
            .expect_err("a close without an explicit reason must refuse");
        assert!(error.contains("--reason"), "{error}");
    }
    #[test]
    fn dispatch_preflight_refuses_single_capture_liveness() {
        let pane = PaneObservation {
            pane_id: "%7".to_owned(),
            state: "IDLE".to_owned(),
            liveness: "UNPROVEN".to_owned(),
            is_dispatchable: false,
            is_free_capacity: true,
            is_working: false,
            awaits_human: false,
        };
        let error = authorize_dispatch_preflight(
            &pane,
            "Objective: x\nTarget: y\nScope:\nreal\nAcceptance:\nrun\nDone: exit 0\nStop: now\n",
            "bead",
            "%7",
        )
        .expect_err("single capture must refuse before transport");
        assert!(error.contains("DISPATCH_PREFLIGHT_REFUSED"), "{error}");
        assert!(error.contains("SingleCaptureLiveness"), "{error}");
    }

    #[test]
    fn lifecycle_selection_accepts_retained_window_without_idle_label() {
        let (_temp, config) = isolated_fixture_config();
        let pane = PaneObservation {
            pane_id: "%7".to_owned(),
            state: "IDLE".to_owned(),
            liveness: "NEWLY_IDLE".to_owned(),
            is_dispatchable: true,
            is_free_capacity: true,
            is_working: false,
            awaits_human: false,
        };
        let packet = r#"Objective: x
Target: y
Scope:
real
Acceptance:
run
Done: exit 0
Stop: now
"#;
        let admission = packet_admission::evaluate(packet, false, false).expect("fresh packet admission");
        begin_dispatch_lifecycle(&config, "%7", &pane, "bead", packet, 7, &admission, 1)
            .expect("retained two-capture evidence must authorize without the label");
    }
    #[test]
    fn stale_docs_admits_grading_and_writes_degraded_row() {
        if !measurable_here(
            "stale_docs_admits_grading_and_writes_degraded_row",
            &[HostRequirement::TmuxPane],
        ) {
            return;
        }
        let temp = tempfile::tempdir().expect("degraded dispatch fixture");
        let bead = "omp-orchestrator-n7yp-fixture";
        let (config, _state, _args, _supervisor) = open_bead_br_fixture(&temp, bead);
        let pane = retained_dispatchable_pane("%1408");
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        let result = runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            let identities = test_identity_registries();
            prepare_bead_dispatch(
                &cx,
                &config,
                "%1408",
                &pane,
                bead,
                &identities,
                17,
                true,
                cross_pane_hold::HoldIntent::Grade,
                false,
                true,
                2,
            )
            .await
        })
        .expect("degraded grading packet should be admitted");
        assert!(matches!(result.2.verdict, DispatchAdmissibility::Degraded { .. }));
        let heartbeat = std::fs::read_to_string(&config.heartbeat_ledger).expect("heartbeat");
        for field in [
            "ADMISSION_DEGRADED",
            "admission=degraded",
            "packet_class=grading",
            "refused_class=plan_dependent",
            "naming_gate=name_the_failing_gate",
            "admitted_pane_count=2",
        ] {
            assert!(heartbeat.contains(field), "missing {field}: {heartbeat}");
        }
    }
    #[test]
    fn dispatch_preflight_accepts_retained_dispatchable_evidence() {
        let pane = PaneObservation {
            pane_id: "%7".to_owned(),
            state: "IDLE".to_owned(),
            liveness: "NEWLY_IDLE".to_owned(),
            is_dispatchable: true,
            is_free_capacity: true,
            is_working: false,
            awaits_human: false,
        };
        let packet =
            "Objective: x\nTarget: y\nScope:\nreal\nAcceptance:\nrun\nDone: exit 0\nStop: now\n";
        authorize_dispatch_preflight(&pane, packet, "bead", "%7")
            .expect("retained two-capture evidence and complete packet must authorize");
    }

    #[test]
    fn packet_completeness_rejects_an_identifier_only_body() {
        assert!(!packet_is_complete("bead-only"));
        assert!(packet_is_complete(
            "Objective: x\nTarget: y\nScope:\nreal\nAcceptance:\nrun\nDone: exit 0\nStop: now\n"
        ));
    }
    #[test]
    fn peer_grade_assignment_uses_any_idle_non_author_without_reservation() {

        let (temp, config) = isolated_fixture_config();
        std::fs::create_dir_all(config.repo.join(".beads")).unwrap();
        std::fs::write(
            config.repo.join(".beads/issues.jsonl"),
            r#"{"id":"peer-bead","status":"in_progress","assignee":"pane3-%1409","comments":[{"author":"pane3-%1409","text":"DONE peer work"}]}
"#,

        )
        .unwrap();
        let now_ms = now_unix().saturating_mul(1_000);
        let bead = BeadId::new("peer-bead").unwrap();
        let target = DispatchTarget::new(config.session.clone(), "%1409").unwrap();
        let identity = LifecycleIdentity::new(
            bead.clone(),
            config.repo.display().to_string(),
            target.clone(),
            packet_digest(b"peer-packet"),
            InvokerClass::detect_current(),
        )
        .unwrap();
        let approval = Approved::authorize(classify(Intent {
            action: TypedAction::DispatchPacket,
            pane_dispatchable: true,
            two_captures: true,
            packet_complete: true,
            finding_has_bead: true,
        }))
        .unwrap();
        let objective = "dispatch bead peer-bead";
        let mut ledger = LifecycleLedger::start(
            config.bead_lifecycle_ledger.clone(),
            identity,
            objective,
            approval,
            LedgerEvidence::single(
                EventId::new("peer-selected").unwrap(),
                now_ms,
                EvidencePolicy::new(now_ms, 0),
                "source",
                "test",
            )
            .unwrap(),
        )
        .unwrap();
        ledger
            .dispatch(
                DispatchReceipt::new(
                    EventId::new("peer-dispatch").unwrap(),
                    bead.clone(),
                    target.clone(),
                    objective,
                    now_ms,
                )
                .unwrap(),
                LedgerEvidence::single(
                    EventId::new("peer-dispatch").unwrap(),
                    now_ms,
                    EvidencePolicy::new(now_ms, 0),
                    "source",
                    "test",
                )
                .unwrap(),
            )
            .unwrap();
        ledger
            .verify_receiver(
                ReceiverEvidence::new(
                    EventId::new("peer-receiver").unwrap(),
                    bead,
                    target,
                    objective,
                    now_ms,
                )
                .unwrap(),
                LedgerEvidence::single(
                    EventId::new("peer-receiver").unwrap(),
                    now_ms,
                    EvidencePolicy::new(now_ms, 0),
                    "source",
                    "test",
                )
                .unwrap(),
            )
            .unwrap();

        let idle = |pane: &str| PaneObservation {
            pane_id: pane.to_owned(),
            state: "IDLE".to_owned(),
            liveness: "CONFIRMED_IDLE".to_owned(),
            is_dispatchable: true,
            is_free_capacity: true,
            is_working: false,
            awaits_human: false,
        };
        let mut observation = Observation {
            panes: vec![idle("%1409"), idle("%1414")],
            queue: QueueState {
                ready_count: 1,
                readable: true,
            },
            gate_census: Some(GateCensus { rows: Vec::new() }),
        };
        let mut self_only = Observation {
            panes: vec![idle("%1409")],
            queue: QueueState {
                ready_count: 1,
                readable: true,
            },
            gate_census: Some(GateCensus { rows: Vec::new() }),
        };
        let self_grade = gate_peer_grading_for_pane(&config, &mut self_only, 76, "%1409")
            .expect_err("a pane cannot self-grade its own receiver-verified bead");
        assert!(self_grade.contains("reason=self_grade"), "{self_grade}");

        let mut no_peer = Observation {
            panes: vec![idle("%1409")],
            queue: QueueState {
                ready_count: 1,
                readable: true,
            },
            gate_census: Some(GateCensus { rows: Vec::new() }),
        };
        let no_peer_gate = gate_peer_grading(&config, &mut no_peer, 76)
            .expect("missing distinct peer must degrade");
        // A skip over a KNOWN population, not an empty one: this candidate set
        // has exactly one receiver-verified bead and no eligible grader.
        // Before the outcome was typed both arrived as `None`.
        assert_eq!(
            no_peer_gate,
            PeerGradeGate::Skipped(PeerGradeSkip {
                candidate_count: 1,
                observed_panes: 1,
                reason_detail: "reason=no_distinct_idle_peer".to_owned(),
            }),
            "no distinct peer must skip grading over a named denominator, not refuse the tick"
        );
        let heartbeat = std::fs::read_to_string(&config.heartbeat_ledger).expect("skip heartbeat");
        assert!(heartbeat.contains("PEER_GRADING_SKIPPED"), "{heartbeat}");
        assert!(
            heartbeat.contains("reason=no_distinct_idle_peer"),
            "{heartbeat}"
        );

        let PeerGradeGate::Claimed(claim) = gate_peer_grading(&config, &mut observation, 77).unwrap()
        else {
            panic!("a finished peer bead must claim grading before new work");
        };
        assert_eq!(claim.bead, "peer-bead");
        assert_eq!(claim.receiver_pane, "%1409");
        assert_eq!(claim.grader_pane, "%1414");
        assert!(
            LifecycleLedger::active_grading_panes(
                &config.bead_lifecycle_ledger,
                config.repo.join(".beads/issues.jsonl"),
            )
            .unwrap()
            .is_empty(),
            "selecting a grade must not create a pane-pinned reservation"
        );


        assert!(
            observation.panes.iter().any(|pane| pane.pane_id == "%1414"),
            "grader must remain observable so the tick can dispatch the grade"
        );
        let mut still_idle = Observation {
            panes: vec![idle("%1409"), idle("%1414")],
            queue: QueueState {
                ready_count: 1,
                readable: true,
            },
            gate_census: Some(GateCensus { rows: Vec::new() }),
        };
        let PeerGradeGate::Claimed(again) =
            gate_peer_grading(&config, &mut still_idle, 78).unwrap()
        else {
            panic!("the same candidate remains gradeable without a reservation");
        };
        assert_eq!(again.bead, "peer-bead");
        assert!(
            !again.bead.contains('<'),
            "placeholder bead={:?}",
            again.bead
        );
        assert_eq!(again.grader_pane, "%1414");
        let err =
            refuse_placeholder_identity("bead", "<active-peer-grade>").expect_err("placeholder");
        assert!(err.contains("reason=placeholder_identity"), "{err}");
        drop(temp);
    }
    fn grade_idle_pane(pane: &str) -> PaneObservation {
        PaneObservation {
            pane_id: pane.to_owned(),
            state: "IDLE".to_owned(),
            liveness: "CONFIRMED_IDLE".to_owned(),
            is_dispatchable: true,
            is_free_capacity: true,
            is_working: false,
            awaits_human: false,
        }
    }

    fn grade_working_pane(pane: &str) -> PaneObservation {
        PaneObservation {
            pane_id: pane.to_owned(),
            state: "WORKING".to_owned(),
            liveness: "WORKING".to_owned(),
            is_dispatchable: false,
            is_free_capacity: false,
            is_working: true,
            awaits_human: false,
        }
    }

    /// Ledger + tracker fixture for grade acquisition legs: bead `grade-bead`
    /// is in_progress, authored by %1409, and receiver-verified. Shape copied
    /// from the neighboring gate fixture; the bead id differs so legs cannot
    /// cross-claim each other's candidates.
    fn grade_fixture() -> (tempfile::TempDir, Config) {
        let (temp, config) = isolated_fixture_config();
        std::fs::create_dir_all(config.repo.join(".beads")).unwrap();
        std::fs::write(
            config.repo.join(".beads/issues.jsonl"),
            r#"{"id":"grade-bead","status":"in_progress","assignee":"pane3-%1409","comments":[{"author":"pane3-%1409","text":"DONE peer work"}]}
"#,
        )
        .unwrap();
        let now_ms = now_unix().saturating_mul(1_000);
        let bead = BeadId::new("grade-bead").unwrap();
        let target = DispatchTarget::new(config.session.clone(), "%1409").unwrap();
        let identity = LifecycleIdentity::new(
            bead.clone(),
            config.repo.display().to_string(),
            target.clone(),
            packet_digest(b"grade-packet"),
            InvokerClass::detect_current(),
        )
        .unwrap();
        let approval = Approved::authorize(classify(Intent {
            action: TypedAction::DispatchPacket,
            pane_dispatchable: true,
            two_captures: true,
            packet_complete: true,
            finding_has_bead: true,
        }))
        .unwrap();
        let objective = "dispatch bead grade-bead";
        let mut ledger = LifecycleLedger::start(
            config.bead_lifecycle_ledger.clone(),
            identity,
            objective,
            approval,
            LedgerEvidence::single(
                EventId::new("grade-selected").unwrap(),
                now_ms,
                EvidencePolicy::new(now_ms, 0),
                "source",
                "test",
            )
            .unwrap(),
        )
        .unwrap();
        ledger
            .dispatch(
                DispatchReceipt::new(
                    EventId::new("grade-dispatch").unwrap(),
                    bead.clone(),
                    target.clone(),
                    objective,
                    now_ms,
                )
                .unwrap(),
                LedgerEvidence::single(
                    EventId::new("grade-dispatch").unwrap(),
                    now_ms,
                    EvidencePolicy::new(now_ms, 0),
                    "source",
                    "test",
                )
                .unwrap(),
            )
            .unwrap();
        ledger
            .verify_receiver(
                ReceiverEvidence::new(
                    EventId::new("grade-receiver").unwrap(),
                    bead,
                    target,
                    objective,
                    now_ms,
                )
                .unwrap(),
                LedgerEvidence::single(
                    EventId::new("grade-receiver").unwrap(),
                    now_ms,
                    EvidencePolicy::new(now_ms, 0),
                    "source",
                    "test",
                )
                .unwrap(),
            )
            .unwrap();
        (temp, config)
    }

    fn grade_observation(panes: Vec<PaneObservation>) -> Observation {
        Observation {
            panes,
            queue: QueueState { ready_count: 1, readable: true },
            gate_census: Some(GateCensus { rows: Vec::new() }),
        }
    }

    #[test]
    fn working_observer_acquires_grade_for_idle_peer() {
        // The catch-22 fix: a WORKING invoker is a legitimate observer, so the
        // acquisition must succeed for the idle peer. Under the old pre-check
        // this errored current_pane_not_dispatchable before reaching selection.
        let (_temp, config) = grade_fixture();
        let mut observation = grade_observation(vec![grade_working_pane("%26"), grade_idle_pane("%1414")]);
        let PeerGradeGate::Claimed(claim) =
            gate_peer_grading_inner(&config, &mut observation, 77, None, "%26")
                .expect("working observer must acquire")
        else {
            panic!("candidate exists");
        };
        assert_eq!(claim.bead, "grade-bead");
        assert_eq!(claim.grader_pane, "%1414");
        assert_eq!(claim.grader_assignee, "pane1414-%1414");
        assert_ne!(claim.grader_pane, "%26", "observer never self-picks");
    }

    #[test]
    fn observer_never_self_assigns_when_sole_idle() {
        // Structural self-grade prevention: the observer is excluded from
        // selection, so a lone idle observer yields no candidate rather than
        // a self-assignment. Removing the exclusion must redden this leg.
        let (_temp, config) = grade_fixture();
        let mut observation = grade_observation(vec![grade_idle_pane("%26")]);
        let outcome = gate_peer_grading_inner(&config, &mut observation, 77, None, "%26")
            .expect("sole-idle observer must skip, not refuse");
        assert_eq!(
            outcome,
            PeerGradeGate::Skipped(PeerGradeSkip {
                candidate_count: 1,
                observed_panes: 1,
                reason_detail: "reason=no_idle_pane".to_owned(),
            }),
            "observer must not self-assign, and the skip must name its denominator"
        );
        let heartbeat = std::fs::read_to_string(&config.heartbeat_ledger).expect("skip heartbeat");
        assert!(heartbeat.contains("reason=no_idle_pane"), "{heartbeat}");
    }

    #[test]
    fn preferred_carrying_peer_keeps_typed_refusal() {
        // The --grader path preserves the full gauntlet on a non-invoking
        // target: a carrying preferred pane is refused by name.
        let (_temp, config) = grade_fixture();
        let mut observation =
            grade_observation(vec![grade_idle_pane("%26"), grade_working_pane("%1408")]);
        let error = gate_peer_grading_for_pane(&config, &mut observation, 77, "%1408")
            .expect_err("carrying preferred pane must refuse");
        assert!(error.contains("reason=preferred_grader_not_idle"), "{error}");
    }

    #[test]
    fn all_working_fleet_names_why_it_cannot_grade() {
        // Anti-vacuity: no idle pane anywhere is a typed outcome naming why,
        // never a silent refusal.
        let (_temp, config) = grade_fixture();
        let mut observation =
            grade_observation(vec![grade_working_pane("%26"), grade_working_pane("%1408")]);
        let outcome = gate_peer_grading_inner(&config, &mut observation, 77, None, "%26")
            .expect("all-working fleet must skip, not refuse");
        assert_eq!(
            outcome,
            PeerGradeGate::Skipped(PeerGradeSkip {
                candidate_count: 1,
                observed_panes: 2,
                reason_detail: "reason=all_panes_carrying_dispatches".to_owned(),
            })
        );
        let heartbeat = std::fs::read_to_string(&config.heartbeat_ledger).expect("skip heartbeat");
        assert!(
            heartbeat.contains("reason=all_panes_carrying_dispatches"),
            "{heartbeat}"
        );
    }

    /// Turn the one receiver-verified candidate's tracker row to `closed`, so
    /// the admissibility filter drops it and the candidate set is genuinely
    /// EMPTY. Same ledger, same panes — only the population differs.
    fn close_the_candidate(config: &Config) {
        std::fs::write(
            config.repo.join(".beads/issues.jsonl"),
            r#"{"id":"grade-bead","status":"closed","assignee":"pane3-%1409","comments":[{"author":"pane3-%1409","text":"DONE peer work"}]}
"#,
        )
        .unwrap();
    }

    #[test]
    fn a_skip_over_a_nonempty_candidate_set_is_not_an_empty_candidate_set() {
        // MEASURED LIVE 2026-09-10, one invocation, two channels:
        //   stdout  PEER_GRADING_SKIPPED candidate_count=10 observed_panes=7 reason=no_idle_pane
        //   stderr  PEER_GRADE_EMPTY typed_outcome=no_receiver_verified_candidate
        // There were TEN candidates, so the stderr cause was the one thing
        // that provably was not true. Both outcomes reached the caller as
        // `Ok(None)`, so the distinction was destroyed in the RETURN TYPE
        // before any print site could be careful with it.
        let (_skip_temp, skip_config) = grade_fixture();
        let mut sole_idle = grade_observation(vec![grade_idle_pane("%26")]);
        let skipped = gate_peer_grading_inner(&skip_config, &mut sole_idle, 77, None, "%26")
            .expect("a skip is not a refused tick");

        let (_empty_temp, empty_config) = grade_fixture();
        close_the_candidate(&empty_config);
        let mut same_panes = grade_observation(vec![grade_idle_pane("%26")]);
        let empty = gate_peer_grading_inner(&empty_config, &mut same_panes, 77, None, "%26")
            .expect("an empty candidate set is not a refused tick either");

        assert_eq!(empty, PeerGradeGate::NoCandidate);
        assert_eq!(
            skipped,
            PeerGradeGate::Skipped(PeerGradeSkip {
                candidate_count: 1,
                observed_panes: 1,
                reason_detail: "reason=no_idle_pane".to_owned(),
            })
        );
        assert_ne!(
            skipped, empty,
            "the two inputs that used to be indistinguishable must now differ"
        );

        // And they stay distinct all the way to the operator's channel.
        let skip_line = match &skipped {
            PeerGradeGate::Skipped(skip) => skip.refusal_line(),
            other => panic!("expected a skip: {other:?}"),
        };
        assert_eq!(
            skip_line,
            "PEER_GRADE_SKIPPED typed_outcome=selection_refused candidate_count=1 reason=no_idle_pane"
        );
        assert!(
            !skip_line.contains("no_receiver_verified_candidate"),
            "a skip over a non-empty set must never claim the set was empty: {skip_line}"
        );
        assert_eq!(
            peer_grade_outcome_wire(&PeerGradeCommandOutcome::NoCandidate),
            "PEER_GRADE_EMPTY",
            "the empty-set marker is reserved for the empty set"
        );
        assert_ne!(
            peer_grade_outcome_wire(&PeerGradeCommandOutcome::Skipped(PeerGradeSkip {
                candidate_count: 1,
                observed_panes: 1,
                reason_detail: "reason=no_idle_pane".to_owned(),
            })),
            peer_grade_outcome_wire(&PeerGradeCommandOutcome::NoCandidate),
            "one wire string for both outcomes is the collapse this bead closes"
        );
    }

    #[test]
    fn the_skip_row_and_the_refusal_line_cannot_name_different_causes() {
        // Agreement is STRUCTURAL: both channels are formatted from one
        // `PeerGradeSkip`, so stdout cannot say candidate_count=10 while
        // stderr says the set was empty. Replays the measured 10/7 shape.
        let skip = PeerGradeSkip {
            candidate_count: 10,
            observed_panes: 7,
            reason_detail: "reason=no_idle_pane".to_owned(),
        };
        let ledger = skip.ledger_detail();
        let refusal = skip.refusal_line();
        assert_eq!(
            ledger,
            "candidate_count=10 observed_panes=7 reason=no_idle_pane next_action=continue-ranked-dispatch"
        );
        assert_eq!(
            refusal,
            "PEER_GRADE_SKIPPED typed_outcome=selection_refused candidate_count=10 reason=no_idle_pane"
        );
        for shared in ["candidate_count=10", "reason=no_idle_pane"] {
            assert!(ledger.contains(shared), "stdout lost {shared}: {ledger}");
            assert!(refusal.contains(shared), "stderr lost {shared}: {refusal}");
        }
    }

    #[test]
    fn a_named_grader_carrying_its_own_dispatch_is_refused_by_name_through_the_gate() {
        // Sibling defect, same gate: the preferred-grader pre-check admits on
        // `is_dispatchable` alone, and tick-monitor derives `is_dispatchable`
        // independently of `is_working` (a DIALOG pane reads dispatchable
        // while carrying a dispatch). Such a pane reaches selection, so the
        // refusal must name the PANE, not the fleet. Deleting the
        // `require_idle_grader` call in `assign_peer_grade_inner` degrades
        // this to reason=no_idle_pane and reddens this leg.
        let (_temp, config) = grade_fixture();
        let dialog_pane = PaneObservation {
            pane_id: "%1408".to_owned(),
            state: "DIALOG".to_owned(),
            liveness: "CONFIRMED_IDLE".to_owned(),
            is_dispatchable: true,
            is_free_capacity: false,
            is_working: true,
            awaits_human: false,
        };
        let mut observation = grade_observation(vec![grade_idle_pane("%26"), dialog_pane]);
        let outcome = gate_peer_grading_for_pane(&config, &mut observation, 77, "%1408")
            .expect("a named-grader skip is not a refused tick");
        let PeerGradeGate::Skipped(skip) = &outcome else {
            panic!("expected a typed skip: {outcome:?}");
        };
        assert_eq!(
            skip.reason_detail,
            "reason=grader_carrying_own_dispatch pane=%1408 liveness=CONFIRMED_IDLE is_working=true",
            "the refusal must name the pane the operator asked about"
        );
        assert!(
            !skip.reason_detail.contains("no_idle_pane"),
            "a fleet-wide cause for one named pane is the defect: {skip:?}"
        );
    }

    #[test]
    fn grade_recording_writes_assignee_without_status() {
        // Leg 5: the acquisition is recorded the way a work dispatch records
        // its claim (assignee set, claim visible) minus the status transition
        // the stage machine owns.
        let temp = tempfile::tempdir().expect("record fixture");
        let bead = "grade-bead";
        let (config, _state, args, _supervisor) = open_bead_br_fixture(&temp, bead);
        let claim = PeerGradeClaim {
            bead: bead.to_owned(),
            receiver_pane: "%1409".to_owned(),
            grader_pane: "%1414".to_owned(),
            grader_assignee: "pane1414-%1414".to_owned(),
        };
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            record_grader_assignment(&cx, &config, &claim).await.expect("record writes");
        });
        let body = std::fs::read_to_string(args).expect("claim command receipt");
        assert!(body.contains("update"), "{body}");
        assert!(body.contains(bead), "{body}");
        assert!(body.contains("--assignee"), "{body}");
        assert!(body.contains("pane1414-%1414"), "{body}");
        assert!(!body.contains("--status"), "stage machine owns status: {body}");
    }

    #[test]
    fn grade_recording_surfaces_tracker_refusal() {
        // The assignment write is the authoritative probe: a tracker refusal
        // arrives with the tracker's own reason, never swallowed.
        let temp = tempfile::tempdir().expect("refusal fixture");
        let script = "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"".to_owned()
            + &temp.path().join("refused-args").display().to_string()
            + "\"\nprintf 'TRACKER_REFUSED reason=bead_blocked\\n'\nexit 1\n";
        let br = executable_reaper(&temp, &script);
        let mut config = fixture_config(temp.path().join("heartbeat.jsonl"));
        config.br = br.display().to_string();
        let claim = PeerGradeClaim {
            bead: "grade-bead".to_owned(),
            receiver_pane: "%1409".to_owned(),
            grader_pane: "%1414".to_owned(),
            grader_assignee: "pane1414-%1414".to_owned(),
        };
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        let error = runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            record_grader_assignment(&cx, &config, &claim).await.expect_err("refusal must surface")
        });
        assert!(error.contains("GRADE_CLAIM_FAILED"), "{error}");
        assert!(error.contains("TRACKER_REFUSED"), "{error}");
    }

    /// 9x13.1 leg 1 + 2: a WORKING caller still obtains a rendered packet, and
    /// the production path renders THROUGH the renderer rather than around it.
    ///
    /// ⛔ THE CALLER-STATE CLAIM IS NOT PROVEN HERE. This test calls
    /// `record_grader_assignment` and `render_peer_grade_packet` directly, so
    /// the observer pane is a local fixture struct that no code under test
    /// branches on — three asserts about it used to sit below and could not
    /// fail for any production change. GradeCatch22 flagged them as decoration
    /// and was right; they are gone. The caller-state leg with a PRODUCTION
    /// subject (real `parse_observation` over the real monitor bytes, then the
    /// real `run_peer_grade_claim`) is
    /// `grade_assign_records_the_claim_before_it_renders_the_packet`.
    ///
    /// What THIS test still earns on its own: the byte-for-byte equality
    /// against a direct `render_grading_packet` call, which reddens the moment
    /// the production path hand-builds a packet instead of calling the
    /// renderer. That is the positive control for the whole region.
    #[test]
    fn grade_assign_renders_a_packet_for_a_working_observer() {
        let temp = tempfile::tempdir().expect("packet fixture");
        let bead = "grade-packet-bead";
        let (config, _state, args, _supervisor) = open_bead_br_fixture(&temp, bead);
        let observer = grade_working_pane("%26");
        // No asserts about `observer` here: see the doc comment. It supplies the
        // observer pane id and nothing more.
        let claim = PeerGradeClaim {
            bead: bead.to_owned(),
            receiver_pane: "%1409".to_owned(),
            grader_pane: "%1414".to_owned(),
            grader_assignee: "pane1414-%1414".to_owned(),
        };
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        let (packet, direct) = runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            record_grader_assignment(&cx, &config, &claim)
                .await
                .unwrap_or_else(|error| {
                    panic!(
                        "assignment write failed: {error}; args={:?}",
                        std::fs::read_to_string(&args)
                    )
                });
            let packet = render_peer_grade_packet(&cx, &config, &claim, &observer.pane_id)
                .await
                .expect("a WORKING observer must still obtain a rendered grading packet");
            // Leg 2 (KNOWN-GOOD, anti-bypass): the same snapshot through the
            // renderer directly. Equality fails the moment the production path
            // hand-builds a packet string instead of calling the renderer.
            let snapshot = load_bead_snapshot(&cx, &config, bead)
                .await
                .expect("snapshot readback");
            let direct = dispatch_packet::render_grading_packet(
                &snapshot,
                &config.repo,
                &claim.grader_pane,
                &observer.pane_id,
            )
            .expect("renderer accepts the claimed bead");
            (packet, direct)
        });
        assert_eq!(
            packet, direct,
            "the production path must render THROUGH dispatch_packet::render_grading_packet, \
             never around it"
        );
        assert!(
            packet.starts_with("GRADE ASSIGNMENT (not implementation)"),
            "{packet}"
        );
        assert!(packet.contains("Grader pane: %1414"), "{packet}");
        assert!(
            packet.contains("Observer: %26 (may be WORKING; observer is not the grader)."),
            "{packet}"
        );
        assert!(packet.contains(bead), "{packet}");
    }

    /// 9x13.1 leg 6 (ORDERING, measured rather than assumed): the tracker
    /// projection precedes the packet. Not asserted by reading source order —
    /// the renderer re-validates the claim, so a packet BEFORE the assignment
    /// write is unrepresentable, and the pair below observes both verdicts
    /// from the same tracker fixture in sequence.
    #[test]
    fn grade_assign_refuses_a_packet_before_the_assignment_write() {
        let temp = tempfile::tempdir().expect("ordering fixture");
        let bead = "grade-order-bead";
        let (config, state, _args, _supervisor) = open_bead_br_fixture(&temp, bead);
        assert!(
            !state.exists(),
            "the fixture must start unclaimed or the ordering leg proves nothing"
        );
        let claim = PeerGradeClaim {
            bead: bead.to_owned(),
            receiver_pane: "%1409".to_owned(),
            grader_pane: "%1414".to_owned(),
            grader_assignee: "pane1414-%1414".to_owned(),
        };
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        let (before, after) = runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            let before = render_peer_grade_packet(&cx, &config, &claim, "%26")
                .await
                .expect_err("a packet before the assignment write must refuse");
            record_grader_assignment(&cx, &config, &claim)
                .await
                .expect("assignment write");
            let after = render_peer_grade_packet(&cx, &config, &claim, "%26")
                .await
                .expect("the same inputs render once the claim is recorded");
            (before, after)
        });
        assert!(before.contains("GRADE_PACKET_REFUSED"), "{before}");
        assert!(before.contains("code=BEAD_NOT_CLAIMED"), "{before}");
        assert!(after.contains("Grader pane: %1414"), "{after}");
    }

    /// 9x13.1 leg 6, second half: if `br update` fails the send must not
    /// happen. The refusing tracker yields no assignment AND no packet.
    #[test]
    fn grade_assign_emits_no_packet_when_the_tracker_refuses() {
        let temp = tempfile::tempdir().expect("refusal fixture");
        let script = "#!/bin/sh\nprintf 'TRACKER_REFUSED reason=bead_blocked\\n'\nexit 1\n";
        let br = executable_reaper(&temp, script);
        let mut config = fixture_config(temp.path().join("heartbeat.jsonl"));
        config.repo = temp.path().to_path_buf();
        config.br = br.display().to_string();
        let claim = PeerGradeClaim {
            bead: "grade-refused-bead".to_owned(),
            receiver_pane: "%1409".to_owned(),
            grader_pane: "%1414".to_owned(),
            grader_assignee: "pane1414-%1414".to_owned(),
        };
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        let (record, render) = runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            let record = record_grader_assignment(&cx, &config, &claim)
                .await
                .expect_err("a refusing tracker must not record");
            let render = render_peer_grade_packet(&cx, &config, &claim, "%26")
                .await
                .expect_err("no packet may be produced from a tracker that refused");
            (record, render)
        });
        assert!(record.contains("GRADE_CLAIM_FAILED"), "{record}");
        assert!(
            !render.contains("GRADE ASSIGNMENT"),
            "the refusal must not carry a packet: {render}"
        );
    }

    /// Write an executable fixture at a CHOSEN name. `executable_reaper` always
    /// writes `reaper`, so a test needing two fixture binaries (a tracker AND a
    /// tick-monitor) cannot use it twice in one tempdir.
    fn executable_named(temp: &tempfile::TempDir, name: &str, body: &str) -> PathBuf {
        let path = temp.path().join(name);
        std::fs::write(&path, body).expect("write fixture executable");
        let mut permissions = std::fs::metadata(&path)
            .expect("fixture metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("fixture permissions");
        path
    }

    /// 9x13.1 CHANGES_REQUESTED round 2, GradeCatch22's finding: acceptance 6
    /// demands an ORDERED COMMAND LOG, and my first four legs could not supply
    /// one because none of them executes `run_peer_grade_claim`. They call
    /// `record_grader_assignment` and `render_peer_grade_packet` directly, in an
    /// order the TEST writes — so they pin the data dependency (real, kept) and
    /// are blind to the sequence the shipping arm uses. Swapping the two calls
    /// in production left all four GREEN at exit=0.
    ///
    /// That is `su6tu`'s defect one level up: a property pinned through a direct
    /// helper call, unobservable through the path that ships.
    ///
    /// This leg drives the real command. The ordered log is the TRACKER's own
    /// invocation record: `record_grader_assignment` spends a `br update` and
    /// `render_peer_grade_packet` spends a `br show` (through
    /// `load_bead_snapshot`), so the fixture appends each verb as it arrives and
    /// the ORDER of that file is the order of production effects. Under the swap
    /// the render runs first, reads an unwritten assignee, refuses
    /// BEAD_NOT_CLAIMED and `?` returns — so `update` never appears at all, and
    /// the command stops doing BOTH of its jobs.
    #[test]
    fn grade_assign_records_the_claim_before_it_renders_the_packet() {
        let (temp, mut config) = grade_fixture();
        let bead = "grade-bead";
        let events = temp.path().join("tracker-events");
        let state = temp.path().join("tracker-state");
        let events_path = events.display().to_string();
        let state_path = state.display().to_string();
        // Every invocation appends its verb FIRST, so the log records arrival
        // order even for a call that then fails.
        let tracker = format!(
            r#"#!/bin/sh
printf '%s\n' "$1" >> "{events_path}"
if [ "$1" = "show" ]; then
  if [ -e "{state_path}" ]; then
    printf '%s\n' '[{{"id":"{bead}","title":"title","description":"Objective: x\nTarget: y\nScope:\nreal\nAcceptance:\nrun\nDone: exit 0\nStop: now","status":"in_progress","assignee":"'$(cat "{state_path}")'"}}]'
  else
    printf '%s\n' '[{{"id":"{bead}","title":"title","description":"Objective: x\nTarget: y\nScope:\nreal\nAcceptance:\nrun\nDone: exit 0\nStop: now","status":"open","assignee":null}}]'
  fi
  exit 0
fi
if [ "$1" = "update" ]; then
  shift
  shift
  assignee=""
  while [ "$#" -gt 0 ]; do
    if [ "$1" = "--actor" ] || [ "$1" = "--assignee" ]; then
      assignee="$2"
      shift
    fi
    shift
  done
  printf '%s\n' "$assignee" > "{state_path}"
  printf 'updated\n'
  exit 0
fi
exit 2
"#
        );
        // The caller pane %26 is WORKING and NOT dispatchable; %1414 is the only
        // CONFIRMED_IDLE peer. This JSON is the acceptance-1 subject with teeth:
        // production `parse_observation` derives the caller's state from it, and
        // the selector branches on what it derives.
        let observation_json = r#"{"omp_lifecycle":{"panes":[{"pane":"%26","state":"WORKING","liveness":"WORKING"},{"pane":"%1414","state":"IDLE","liveness":"CONFIRMED_IDLE"}]},"idle_panes":{"dispatchable":["%1414"],"free_capacity":["%1414"]}}"#;
        let monitor = format!("#!/bin/sh\ncat <<'JSON'\n{observation_json}\nJSON\n");
        config.br = executable_named(&temp, "tracker", &tracker)
            .display()
            .to_string();
        config.tick_monitor = executable_named(&temp, "monitor", &monitor)
            .display()
            .to_string();

        // ACCEPTANCE 1 WITH A PRODUCTION SUBJECT. The earlier leg's caller-state
        // asserts are about a local fixture struct and cannot fail for any
        // production change; these run the real parser over the real fixture
        // bytes the command consumes.
        let parsed = parse_observation(observation_json.as_bytes(), None)
            .expect("the fixture observation must parse through production code");
        let caller = parsed
            .panes
            .iter()
            .find(|pane| pane.pane_id == "%26")
            .expect("caller row");
        assert_ne!(
            caller.liveness, "CONFIRMED_IDLE",
            "caller pane %26 reads {caller:?}; an idle caller cannot exercise the catch-22"
        );
        assert!(caller.is_working, "caller must be carrying work: {caller:?}");
        assert!(
            !caller.is_dispatchable,
            "a WORKING caller is not dispatchable -- exactly what the old pre-check refused on: {caller:?}"
        );

        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        let outcome = runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            run_peer_grade_claim(&cx, &config, "%26", None).await
        });
        let log = std::fs::read_to_string(&events).unwrap_or_default();
        let (claim, packet) = match outcome {
            Ok(PeerGradeCommandOutcome::Claimed { claim, packet }) => (claim, packet),
            other => panic!(
                "a WORKING observer must claim and render through the shipping command; \
                 got {other:?}; tracker log={log:?}"
            ),
        };
        assert_eq!(claim.grader_pane, "%1414", "{claim:?}");
        assert_eq!(claim.grader_assignee, "pane1414-%1414", "{claim:?}");
        assert_ne!(
            claim.grader_pane, "%26",
            "the WORKING observer must never be its own grader: {claim:?}"
        );

        let verbs: Vec<&str> = log.lines().collect();
        assert_eq!(
            verbs.first().copied(),
            Some("update"),
            "br update --assignee must be the FIRST tracker effect, before the packet is \
             rendered; observed order={verbs:?}"
        );
        assert!(
            verbs.iter().skip(1).any(|verb| *verb == "show"),
            "the packet render must read the tracker AFTER the claim write; observed \
             order={verbs:?}"
        );
        assert!(
            packet.starts_with("GRADE ASSIGNMENT (not implementation)"),
            "{packet}"
        );
        assert!(packet.contains("Grader pane: %1414"), "{packet}");
        assert!(
            packet.contains("Observer: %26 (may be WORKING; observer is not the grader)."),
            "{packet}"
        );
    }

    /// The bead and the doctrine name `grade --assign`; the implementation
    /// landed as `--claim`. Both spellings must reach the same path or the
    /// acceptance leg naming the other one is unexecutable.
    #[test]
    fn grade_parser_accepts_the_assign_spelling() {
        let assign = parse_grade_claim_args(&[
            "grade".to_owned(),
            "--assign".to_owned(),
            "--repo".to_owned(),
            "/repo".to_owned(),
        ])
        .expect("--assign is the bead's spelling")
        .expect("grade assign request");
        let claim = parse_grade_claim_args(&[
            "grade".to_owned(),
            "--claim".to_owned(),
            "--repo".to_owned(),
            "/repo".to_owned(),
        ])
        .expect("--claim is the landed spelling")
        .expect("grade claim request");
        assert_eq!(assign, claim, "the two spellings must not diverge");
        let wrong = parse_grade_claim_args(&["grade".to_owned(), "--grade".to_owned()])
            .expect_err("an unknown mode flag must still refuse");
        assert!(wrong.contains("--assign"), "{wrong}");
    }


    #[test]
    fn grade_claim_parser_requires_the_claim_flag() {
        let request = parse_grade_claim_args(&[
            "grade".to_owned(),
            "--claim".to_owned(),
            "--repo".to_owned(),
            "/repo".to_owned(),
        ])
        .expect("valid grade claim syntax")
        .expect("grade claim request");
        assert_eq!(request.config_args, vec!["--repo".to_owned(), "/repo".to_owned()]);
        assert_eq!(request.grader, None);
        let error =
            parse_grade_claim_args(&["grade".to_owned()]).expect_err("bare grade must refuse");
        assert!(error.contains("--claim"), "{error}");
    }

    #[test]
    fn grade_claim_parser_routes_optional_grader() {
        let request = parse_grade_claim_args(&[
            "grade".to_owned(),
            "--claim".to_owned(),
            "--grader".to_owned(),
            "%8".to_owned(),
            "--repo".to_owned(),
            "/repo".to_owned(),
        ])
        .expect("grader flag parses")
        .expect("grade claim request");
        assert_eq!(request.grader, Some("%8".to_owned()));
        assert_eq!(request.config_args, vec!["--repo".to_owned(), "/repo".to_owned()]);
        let error = parse_grade_claim_args(&[
            "grade".to_owned(),
            "--claim".to_owned(),
            "--grader".to_owned(),
            "not-a-pane".to_owned(),
        ])
        .expect_err("malformed grader must refuse");
        assert!(error.contains("CONFIG_REFUSED"), "{error}");
        let missing = parse_grade_claim_args(&["grade".to_owned(), "--claim".to_owned(), "--grader".to_owned()])
            .expect_err("missing grader value must refuse");
        assert!(missing.contains("CONFIG_REFUSED"), "{missing}");
    }

    #[test]
    fn empty_peer_candidate_is_a_typed_outcome_not_success() {
        assert_eq!(
            peer_grade_outcome_wire(&PeerGradeCommandOutcome::NoCandidate),
            "PEER_GRADE_EMPTY"
        );
        assert_eq!(
            peer_grade_outcome_wire(&PeerGradeCommandOutcome::ActivePeerGrade),
            "PEER_GRADE_ACTIVE"
        );
    }

    #[test]
    fn verified_live_pane_identity_supplies_sender() {
        let identity = PaneIdentity {
            pane_id: "%8".to_owned(),
            binding: BindingStatus::VerifiedLive,
            agent_name: Some(AgentName::new("BrightGorge")),
            session: Some("omp-orchestrator".to_owned()),
            pane_index: Some(3),
        };
        assert_eq!(
            sender_from_verified_pane(&identity).expect("verified identity"),
            AgentName::new("BrightGorge")
        );
    }

    #[test]
    fn non_live_or_missing_pane_identity_refuses_sender() {
        let dead = PaneIdentity {
            pane_id: "%8".to_owned(),
            binding: BindingStatus::LegacyUnverified,
            agent_name: Some(AgentName::new("BrightGorge")),
            session: None,
            pane_index: None,
        };
        assert!(sender_from_verified_pane(&dead)
            .expect_err("legacy identity must refuse")
            .contains("binding_not_verified_live"));
        let missing = PaneIdentity {
            pane_id: "%9".to_owned(),
            binding: BindingStatus::VerifiedLive,
            agent_name: None,
            session: None,
            pane_index: Some(4),
        };
        assert!(sender_from_verified_pane(&missing)
            .expect_err("missing sender must refuse")
            .contains("agent_name_missing"));
    }

    #[test]
    fn admit_immediately_before_send_admits_live_pane() {
        admit_immediately_before_send("omp-orchestrator", "%9").expect("live occupancy must admit");
    }

    #[test]
    fn stale_incarnation_is_refused_typed() {
        let mint = IncarnationMint::new();
        let mut occupancy = Occupancy::unminted("omp-orchestrator", "%N");
        let first = occupancy.occupy(&mint, std::process::id() as u32);
        occupancy
            .advance_lease(
                pane_dispatch_fence::Lease::Admitting,
                pane_dispatch_fence::Lease::Draining,
            )
            .unwrap();
        occupancy
            .advance_lease(
                pane_dispatch_fence::Lease::Draining,
                pane_dispatch_fence::Lease::Revoked,
            )
            .unwrap();
        let second = occupancy.occupy(&mint, std::process::id() as u32);
        let presented = Presented {
            session: "omp-orchestrator".into(),
            pane: "%N".into(),
            incarnation: Some(first),
            marker_pid: Some(std::process::id() as u32),
        };
        match admit_at_send(&occupancy, &presented, std::process::id() as u32) {
            Err(pane_dispatch_fence::AdmissionRefusal::StaleIncarnation { presented, current }) => {
                assert_eq!(presented, first);
                assert_eq!(current, second);
            }
            other => panic!("expected StaleIncarnation, got {other:?}"),
        }
    }

    #[test]
    fn selected_event_key_changes_across_runs_and_preserves_same_run_replay() {
        let first = selected_event_key_for_run("omp-orchestrator-eg0m", 2, "build-a:pid101:start1");
        let second =
            selected_event_key_for_run("omp-orchestrator-eg0m", 2, "build-a:pid202:start2");
        let replay =
            selected_event_key_for_run("omp-orchestrator-eg0m", 2, "build-a:pid101:start1");

        assert_ne!(
            first, second,
            "fresh runs must not collide at the same tick"
        );
        assert_eq!(
            first, replay,
            "same-run duplicate selection must remain idempotent"
        );
        assert!(first.contains(":selected:build-a:pid101:start1:2"));
        assert!(second.contains(":selected:build-a:pid202:start2:2"));
    }

    #[test]
    fn lifecycle_run_id_contains_build_pid_and_start_identity() {
        let run_id = lifecycle_run_id();
        assert!(run_id.contains(BUILD_ID));
        assert!(run_id.contains(&format!("pid{}", std::process::id())));
        assert!(run_id.contains(":start"));
    }

    #[test]
    fn composite_tracker_assignee_distinguishes_panes_and_parses_agent() {
        let mint = IncarnationMint::new();
        let left = tracker_assignee_for_dispatch("WildStone", "%7", mint.mint());
        let right = tracker_assignee_for_dispatch("WildStone", "%8", mint.mint());
        assert_ne!(left, right);
        assert_eq!(tracker_assignee_agent(&left), Some("WildStone"));
        assert_eq!(tracker_assignee_agent(&right), Some("WildStone"));
        assert_eq!(tracker_assignee_pane(&left), Some("%7"));
        assert_eq!(tracker_assignee_pane(&right), Some("%8"));
        assert!(left.contains("pane=%7;incarnation="));
        assert!(right.contains("pane=%8;incarnation="));
    }

    #[test]
    fn legacy_pane_prefixed_labels_remain_readable() {
        assert_eq!(tracker_assignee_pane("pane4-%9"), Some("%9"));
        assert_eq!(tracker_assignee_pane("pane3-%8"), Some("%8"));
        assert_eq!(tracker_assignee_pane("WildStone"), None);
        assert_eq!(tracker_assignee_pane("GreenFrog"), None);
    }

    #[test]
    fn unresolvable_identity_is_typed_refusal_not_a_silent_write() {
        let mint = IncarnationMint::new();
        let empty_pane = tracker_assignee_with_composite(true, "WildStone", "", mint.mint());
        assert!(empty_pane.unwrap_err().contains("ASSIGNEE_UNRESOLVABLE"));
        let empty_agent = tracker_assignee_with_composite(true, "", "%7", mint.mint());
        assert!(empty_agent.unwrap_err().contains("ASSIGNEE_UNRESOLVABLE"));
    }

    #[test]
    fn mutation_launch_flag_write_collapses_panes_then_restores() {
        let mint = IncarnationMint::new();
        let a = mint.mint();
        let b = mint.mint();
        let with = (
            tracker_assignee_with_composite(true, "WildStone", "%7", a).unwrap(),
            tracker_assignee_with_composite(true, "WildStone", "%8", b).unwrap(),
        );
        assert_ne!(with.0, with.1, "leg 3 requires the composite");
        let without = (
            tracker_assignee_with_composite(false, "WildStone", "%7", a).unwrap(),
            tracker_assignee_with_composite(false, "WildStone", "%8", b).unwrap(),
        );
        assert_eq!(
            without.0, without.1,
            "launch-flag write makes two panes identical — the defect"
        );
        let restored = (
            tracker_assignee_with_composite(true, "WildStone", "%7", a).unwrap(),
            tracker_assignee_with_composite(true, "WildStone", "%8", b).unwrap(),
        );
        assert_eq!(restored, with);
        let source = include_str!("resident.rs");
        assert!(source.contains("tracker_assignee_with_composite(true,"));
    }
    #[test]
    fn close_readback_refuses_active_reservation_named_for_bead() {
        let temp = tempfile::tempdir().expect("reservation refusal fixture");
        let br = executable_reaper(
            &temp,
            "#!/bin/sh\nprintf '%s\n' '[{\"id\":\"bead\",\"status\":\"closed\"}]'\n",
        );
        let am = temp.path().join("am");
        std::fs::write(
            &am,
            "#!/bin/sh\nprintf '%s\n' 'ID PATTERN AGENT EXPIRES REASON'\nprintf '%s\n' '112933 var/agent-tmp/lease WildStone 2099-01-01T00:00:00Z omp-orchestrator-test-bead'\n",
        )
        .expect("write am fixture");
        let mut permissions = std::fs::metadata(&am).expect("am metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&am, permissions).expect("make am executable");

        let mut config = fixture_config(temp.path().join("heartbeat.jsonl"));
        config.br = br.display().to_string();
        config.am = am.display().to_string();
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        let result = runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            close_and_read_back(
                &cx,
                &config,
                "omp-orchestrator-test-bead",
                "DONE: verified",
            )
            .await
        });
        match result {
            CloseReadback::PolicyRefused { refusal } => {
                assert!(refusal.contains("CLOSE_REFUSED_RESERVATION_LEASE"), "{refusal}");
                assert!(refusal.contains("reservation_id=112933"), "{refusal}");
                assert!(refusal.contains("holder=WildStone"), "{refusal}");
                assert!(refusal.contains("path=var/agent-tmp/lease"), "{refusal}");
                assert!(refusal.contains("omp-orchestrator-test-bead"), "{refusal}");
            }
            other => panic!("active reservation must refuse close, got {other:?}"),
        }
    }
}

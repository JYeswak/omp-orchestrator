#![forbid(unsafe_code)]

//! The resident OMP supervisor.
//!
//! This executable owns the observation -> queue -> dispatch -> receiver receipt
//! loop. It is intentionally not a report-only monitor: a managed session may
//! idle only with a bound Josh authorization token.

use ack_stage::{
    assess as assess_ack_stage, AckAction, AckReadback, AckStageInput, AckStageResult,
    TransportReceipt,
};
use agent_mail_native::journey::{
    self as mail, AgentName, DeliveryReceipt, ProjectKey, SendRequest,
};
use agent_mail_native::{MailClient, MailError};
use agent_mail_native::identity::{resolve_pane_identity, BindingStatus, PaneIdentity};
use asupersync::process::{Command, Output};
use orchestration_tick_gate::{append_receipt, build_receipt, PaneDisposition, Receipt};
use asupersync::runtime::RuntimeBuilder;
use asupersync::time::{sleep, timeout};
use asupersync::Cx;
use finding::{BrPublisher, FindingError};
use finding_dispatch::{MaybeFinding, NotYet};
use dispatch_claim_fence::{
    authorize, authorize_with_identities, parse_br_show_json, BeadSnapshot, ClaimFenceError,
    DispatchIntent, IdentityRecord, IdentityRegistries,
};
use dispatch_silence_watch::SilenceVerdict;
use pane_dispatch_fence::{
    admit_at_send, IncarnationMint, Occupancy, Presented,
};
use ntm_fleet_monitor::parse_activity_json;
use omp_orchestrator::{
    applicable, census_gates, decide, dispatch_packet, read_idle_authorization, GateCensus,
    Observation, PaneObservation, QueueState, SupervisorDecision,
};
use omp_rpc_session::{
    run_session, OmpCommand, RpcError, RpcSessionConfig, NO_CLAIM_BOUNDARY, OMP_RPC_SCHEMA_VERSION,
    OMP_SURFACE,
};
use receiver_receipt::{
    escalate_non_delivery, observe_capture, ComposerEvidence, NonDeliveryEscalation,
    ObservationIdentity, PostSendObservation, ReceiptReason, ReceiptVerdict,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::sync::{LazyLock, Mutex};
use ack_spine::ledger::StepKind;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use subprocess_contract::run_output;
use lifecycle_event::{
    default_repo_journal, emit as emit_lifecycle, DurableJournal, Layer, LifecycleEvent, EmitOutcome,
    ReasonCode,
};
use lifecycle_monitor::{load_metrics, observe_layer, verify_artifact};
use ntm_fleet_monitor::{classify, Approved, Intent, TypedAction};
use ntm_fleet_monitor::bead_lifecycle::{
    BeadId, DispatchReceipt, DispatchTarget, EvidencePolicy, EventId, ReceiverEvidence, RedispatchPlan,
};
use ntm_fleet_monitor::bead_lifecycle::ledger::{
    packet_digest, InvokerClass, LedgerEvidence, LifecycleIdentity, LifecycleLedger,
};

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
#[derive(Debug)]
pub struct Config {
    repo: PathBuf,
    session: String,
    interval: Duration,
    run_subcommand: bool,
    command_timeout: Duration,
    max_ticks: Option<u64>,
    tick_monitor: String,
    br: String,
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
                traps_file = Some(PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| "CONFIG_REFUSED --traps-file requires a path".to_owned())?,
                ));
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
        return Err(
            "CONFIG_REFUSED close-readback requires BEAD --reason REASON".to_owned(),
        );
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

fn parse_grade_claim_args(args: &[String]) -> Result<Option<Vec<String>>, String> {
    if args.first().map(String::as_str) != Some("grade") {
        return Ok(None);
    }
    if args.get(1).map(String::as_str) != Some("--claim") {
        return Err("CONFIG_REFUSED grade requires --claim".to_owned());
    }
    Ok(Some(args[2..].to_vec()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PeerGradeCommandOutcome {
    Claimed(PeerGradeClaim),
    ActivePeerGrade,
    NoCandidate,
}

fn peer_grade_outcome_wire(outcome: &PeerGradeCommandOutcome) -> &'static str {
    match outcome {
        PeerGradeCommandOutcome::Claimed(_) => "PEER_GRADE_CLAIMED",
        PeerGradeCommandOutcome::ActivePeerGrade => "PEER_GRADE_ACTIVE",
        PeerGradeCommandOutcome::NoCandidate => "PEER_GRADE_EMPTY",
    }
}

async fn run_peer_grade_claim(
    cx: &Cx,
    config: &Config,
    grader_pane: &str,
) -> Result<PeerGradeCommandOutcome, String> {
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
    let mut observation = parse_observation(&monitor_bytes, census_gates(&config.repo))?;
    observation.panes.retain(|pane| {
        !config
            .exclude_panes
            .iter()
            .any(|excluded| excluded == &pane.pane_id)
    });
    if !observation
        .panes
        .iter()
        .any(|pane| pane.pane_id == grader_pane && pane.is_dispatchable && pane.liveness == "CONFIRMED_IDLE")
    {
        return Err(format!(
            "PEER_GRADING_REFUSED grader_pane={grader_pane} reason=current_pane_not_confirmed_idle"
        ));
    }
    match gate_peer_grading_for_pane(config, &mut observation, now_unix(), grader_pane)? {
        Some(claim) if claim.bead == "<active-peer-grade>" => {
            Ok(PeerGradeCommandOutcome::ActivePeerGrade)
        }
        Some(claim) => Ok(PeerGradeCommandOutcome::Claimed(claim)),
        None => Ok(PeerGradeCommandOutcome::NoCandidate),
    }
}

fn peer_grade_command_exit(outcome: PeerGradeCommandOutcome) -> std::process::ExitCode {
    match outcome {
        PeerGradeCommandOutcome::Claimed(claim) => {
            println!(
                "{} bead={} receiver_pane={} grader_pane={} experiment=self-service",
                peer_grade_outcome_wire(&PeerGradeCommandOutcome::Claimed(claim.clone())),
                claim.bead,
                claim.receiver_pane,
                claim.grader_pane,
            );
            std::process::ExitCode::SUCCESS
        }
        PeerGradeCommandOutcome::ActivePeerGrade => {
            eprintln!("PEER_GRADING_REFUSED reason=active_peer_grade");
            std::process::ExitCode::from(2)
        }
        PeerGradeCommandOutcome::NoCandidate => {
            eprintln!("PEER_GRADE_EMPTY typed_outcome=no_receiver_verified_candidate");
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
        // `run` is the explicit lifecycle entrypoint: `omp-orchestrator run
        // --once ...`. It consumes only the leading token; every flag after
        // it parses identically to the launchd flag-only invocation, and any
        // positional other than a leading `run` is refused in the match arm.
        let run_subcommand = args.first().map(String::as_str) == Some("run");
        let mut index = if run_subcommand { 1 } else { 0 };
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
                "--help" => return Err(usage().to_owned()),
                "--version" => {
                    return Err(format!(
                        "omp-orchestrator {} build_id={BUILD_ID}",
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
                heartbeat_ledger.with_file_name(format!("omp-orchestrator-{session}.bead-lifecycle.jsonl"))
            });
        let tick_monitor_state = env::var_os("OMP_TICK_MONITOR_STATE")
            .map(PathBuf::from)
            .unwrap_or_else(|| default_tick_monitor_state(&heartbeat_ledger, &session));
        let pending_dispatch = env::var_os("OMP_PENDING_DISPATCH")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                heartbeat_ledger.with_file_name("omp-orchestrator.pending-dispatch")
            });
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
        let resolved_sender =
            sender_identity::first_candidate(&|name| env::var(name).ok());
        Ok(Self {
            repo,
            session,
            interval,
            command_timeout,
            max_ticks,
            tick_monitor: env::var("OMP_TICK_MONITOR_BIN")
                .unwrap_or_else(|_| "tick-monitor".to_owned()),
            br: env::var("OMP_BR_BIN").unwrap_or_else(|_| "br".to_owned()),
            // The planning brain. Ranked selection is MANDATORY, so an absent `bv`
            // must surface as a typed QUEUE_UNRANKED refusal at the call site rather
            // than as a silent fall back to creation order.
            bv: env::var("OMP_BV_BIN").unwrap_or_else(|_| "bv".to_owned()),
            ntm: env::var("OMP_NTM_BIN").unwrap_or_else(|_| "ntm".to_owned()),
            tmux_tmpdir,
            run_subcommand,
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
        })
    }
}
fn usage() -> &'static str {
    "usage: omp-orchestrator [run] [--once|--max-ticks N] [--repo PATH] [--session NAME] [--interval-secs N] [--receiver-agent NAME] [--omp-quick] [--omp-binary PATH]\n       close-readback BEAD --reason REASON\n       dispatch render --bead BEAD --pane %N [--why-now TEXT] [--traps-file PATH]\n       grade --claim [--repo PATH] [--session NAME]\n       run is the explicit resident lifecycle entrypoint (observe -> ready queue -> dispatch -> receiver receipt); dispatch render emits the same packet without transport"
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

fn string_set(value: Option<&Value>) -> BTreeSet<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_observation(bytes: &[u8], gate_census: GateCensus) -> Result<Observation, String> {
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
        gate_census: Some(gate_census),
    })
}

fn parse_ready(bytes: &[u8]) -> Result<Vec<String>, String> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("QUEUE_UNREADABLE br ready JSON: {error}"))?;
    let rows = value
        .as_array()
        .ok_or_else(|| "QUEUE_UNREADABLE br ready did not return an array".to_owned())?;
    let mut ids = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let id = row
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.trim().is_empty())
            .ok_or_else(|| format!("QUEUE_UNREADABLE br ready row {index} has no non-empty id"))?;
        ids.push(id.to_owned());
    }
    Ok(ids)
}

/// Reorder the ready queue by `bv`'s graph triage instead of taking `br ready` order.
///
/// # `br ready` order is CREATION ORDER, and that is FIFO by another name
///
/// MEASURED 2026-09-06: `parse_ready` above filters empty ids and nothing else, and the
/// caller took `bead_ids.first()`. So selection was whatever `br` happened to return
/// first. Live consequence in one session: the loop dispatched `28dq` (P1), `dmpv` (P1),
/// `jg9x` (P1) and `xv30` (P1) while **26 P0 beads sat ready**, and it never consulted
/// the dependency graph at all — `git grep 'Command::new("bv")'` over `crates/*/src`
/// returned zero. `omp-orchestrator-2ceb` filed this independently as "selects work FIFO
/// by creation date and never consults bv".
///
/// # Why the graph and not just the priority integer
///
/// Priority alone re-creates easy-bead cherry-picking: twenty P0 leaves outrank one P0
/// articulation point whose closure unblocks them. `bv` scores PageRank over the
/// dependency DAG, so `unblocks_ids` is priced in. Order is (priority ASC, score DESC).
///
/// # What this deliberately does NOT do
///
/// `bv --robot-triage` returns a BOUNDED recommendation list (10 rows measured), not a
/// total order over 360 ready beads. So ranked rows go first and the remaining ready ids
/// keep their prior order behind them. This is a ranked HEAD, not a sorted queue, and
/// saying otherwise would overclaim.
///
/// Epic containers are excluded: AGENTS.md records that an epic's PageRank accumulates
/// from every child, so epics top the list and can never close until their children do.
/// Assigned rows are excluded so a dispatched bead is not re-offered.
fn rank_ready(triage: &[u8], ready: &[String]) -> Result<Vec<String>, String> {
    let value: Value = serde_json::from_slice(triage)
        .map_err(|error| format!("QUEUE_UNRANKED bv triage JSON: {error}"))?;
    let recs = value
        .get("triage")
        .and_then(|t| t.get("recommendations"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            // `.quick_ref.top_picks` is NOT an acceptable fallback: AGENTS.md measured it
            // reporting unblocks=0 and omitting high scorers.
            "QUEUE_UNRANKED bv triage has no .triage.recommendations array".to_owned()
        })?;
    let mut ranked: Vec<(u64, i64, String)> = Vec::new();
    for row in recs {
        let Some(id) = row.get("id").and_then(Value::as_str) else {
            continue;
        };
        if !ready.iter().any(|candidate| candidate == id) {
            continue;
        }
        if row
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind.eq_ignore_ascii_case("epic"))
        {
            continue;
        }
        if row
            .get("assignee")
            .and_then(Value::as_str)
            .is_some_and(|who| !who.trim().is_empty())
        {
            continue;
        }
        let priority = row.get("priority").and_then(Value::as_u64).unwrap_or(u64::MAX);
        // Scores are fractional; scale so the sort key stays integral and total.
        let score = row
            .get("score")
            .and_then(Value::as_f64)
            .map(|s| -((s * 1_000_000.0) as i64))
            .unwrap_or(0);
        ranked.push((priority, score, id.to_owned()));
    }
    ranked.sort();
    let mut out: Vec<String> = ranked.into_iter().map(|(_, _, id)| id).collect();
    for id in ready {
        if !out.iter().any(|chosen| chosen == id) {
            out.push(id.clone());
        }
    }
    Ok(out)
}

async fn capture_pane(cx: &Cx, config: &Config, pane: &str) -> Result<Vec<u8>, String> {
    let args = vec![
        "capture-pane".to_owned(),
        "-p".to_owned(),
        "-t".to_owned(),
        pane.to_owned(),
        "-S".to_owned(),
        "-14".to_owned(),
    ];
    require_success(
        "tmux capture-pane",
        invoke(cx, config, "tmux", &args).await?,
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
        invoke(cx, config, "tmux", &args).await?,
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
) -> (PostSendObservation, Option<String>) {
    let list_args = vec![
        "list-panes".to_owned(),
        "-t".to_owned(),
        config.session.clone(),
        "-F".to_owned(),
        "#{pane_id}".to_owned(),
    ];
    let list_output = match invoke(cx, config, "tmux", &list_args).await {
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
    let readback = AckReadback::from_comments_json(bead, pane, &bytes).map_err(|error| {
        format!("ACK_STAGE_INDETERMINATE bead={bead} pane={pane} comment read-back: {error}")
    })?;
    Ok(match dispatch_marker_issued_at(config, pane) {
        Some(issued_at) => readback.with_dispatch_issued_at(issued_at),
        None => readback,
    })
}

/// The `issued_at` recorded in this pane's pending-dispatch marker, if readable.
///
/// Returns `None` for a missing, unreadable, or non-numeric marker. `None` is a
/// DEGRADE, not a refusal: the recency guard is then inactive and the readback behaves as
/// it did before `y903`. Refusing here would convert every dispatch whose marker was
/// already cleared into a failure.
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
    let detail = serde_json::json!({
        "bead": bead,
        "pane": pane,
        "transport": transport.kind().label(),
        "raw_transport_json": transport.raw_json(),
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
    .map_err(|error| format!("SPINE_STEP_REFUSED kind={} bead={bead} {error}", kind.as_str()))
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
    omp_orchestrator::spine_emit::assert_cycle_emitted(ledger.steps_taken(), dispatched)?;
    ledger
        .assert_step_count()
        .map_err(|error| format!("SPINE_LEDGER_INCONSISTENT {error}"))?;
    if ledger.rows().is_empty() {
        return Ok(());
    }
    let path = omp_orchestrator::spine_emit::spine_ledger_path(&config.heartbeat_ledger);
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

/// How many times this bead has already been dispatched, from the heartbeat's own
/// `DISPATCHED` rows.
///
/// Measured 2026-09-02: 209 `DISPATCHED` rows across 27 distinct beads and 59
/// (bead, pane) pairs. So `Redispatched` — a kind that existed in the enum and in
/// zero rows — has real input the moment it is asked for.
fn prior_dispatch_count(config: &Config, bead: &str) -> usize {
    let Ok(text) = fs::read_to_string(&config.heartbeat_ledger) else {
        return 0;
    };
    let needle = format!("bead={bead} ");
    text.lines()
        .filter(|line| line.contains("\"DISPATCHED\""))
        // The trailing space matters: without it `bead=omp-orchestrator-2z2`
        // matches `bead=omp-orchestrator-2z2.1`, and a prefix collision would
        // inflate the count for every dotted child bead.
        .filter(|line| line.contains(&needle) || line.contains(&format!("bead={bead}\"")))
        .count()
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
async fn reconcile_completions(
    cx: &Cx,
    config: &Config,
    tick: u64,
) -> Result<usize, String> {
    const MAX_BEADS_PER_TICK: usize = 6;
    let Ok(heartbeat) = fs::read_to_string(&config.heartbeat_ledger) else {
        return Ok(0);
    };
    let spine_path = omp_orchestrator::spine_emit::spine_ledger_path(&config.heartbeat_ledger);
    let recorded =
        omp_orchestrator::spine_emit::recorded_closures(&fs::read_to_string(&spine_path).unwrap_or_default());
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
        let prior = omp_orchestrator::spine_emit::PriorDispatch {
            bead_id: bead.clone(),
            pane_id: pane.clone(),
            status: status.clone(),
            close_reason: close_reason.clone(),
            prior_dispatch_count: 0,
            already_recorded: false,
        };
        for kind in omp_orchestrator::spine_emit::completion_kinds(&prior) {
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

async fn close_and_read_back(
    cx: &Cx,
    config: &Config,
    bead: &str,
    reason: &str,
) -> CloseReadback {
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
async fn load_identity_registries(
    cx: &Cx,
    config: &Config,
) -> Result<IdentityRegistries, String> {
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

fn supervisor_claim_identity() -> String {
    format!("supervisor:{}", std::process::id())
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
    if let Some(agent) = snapshot
        .assignee()
        .filter(|agent| !agent.trim().is_empty() && !agent.starts_with("supervisor:"))
    {
        return Ok(agent.to_owned());
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
    let authorization = if claim_owner == receiver_agent {
        authorize_with_identities(
            &DispatchIntent::bead(bead, claim_owner),
            Some(snapshot),
            identities,
        )
    } else {
        authorize(&DispatchIntent::bead(bead, claim_owner), Some(snapshot)).and_then(|permit| {
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
    tick: u64,
    claim_enabled: bool,
) -> Result<(BeadSnapshot, String), String> {
    let supervisor = supervisor_claim_identity();
    // TWO claimable shapes, and the second one is why the fleet stalled.
    //
    // (1) open + UNASSIGNED -> the supervisor claims it to its own actor.
    // (2) open + ALREADY ASSIGNED TO THE RECEIVER -> the supervisor completes the
    //     transition to `in_progress` KEEPING that assignee.
    //
    // (2) is not a forge. K9's defect was the dispatch path asserting a claim on
    // the receiver's behalf when nobody owned the bead; here the receiver is
    // ALREADY the recorded owner and only the status transition is missing.
    //
    // MEASURED 2026-09-02: `br update <bead> --assignee X` without
    // `--status in_progress` leaves `open` + assigned -- a half-claim. Several
    // beads were left that way by hand-claims earlier in the session, and every
    // dispatch to them refused `CLAIM_REQUIRED status=open assignee=GreenFrog
    // receiver_agent=GreenFrog`, printing the exact repair command while being
    // unable to run it itself.
    let assignee = snapshot.assignee().unwrap_or("").to_owned();
    let open = snapshot.status_label() == "open";
    let unclaimed_open = open && snapshot.assignee().is_none();
    let half_claimed_by_receiver = open && assignee == receiver_agent && !assignee.is_empty();
    if !unclaimed_open && !half_claimed_by_receiver {
        return Ok((snapshot, assignee));
    }
    if !claim_enabled {
        return Err(format!(
            "DISPATCH_BLOCKED bead={bead} pane={pane} reason=CLAIM_REQUIRED status=open assignee=unassigned receiver_agent={receiver_agent} owner=josh next_action=enable-supervisor-claim"
        ));
    }

    // The half-claimed shape keeps the RECEIVER as owner; only the unassigned
    // shape claims to the supervisor actor. Two commands, two readbacks -- a
    // single readback expecting `supervisor` would reject the very transition it
    // just performed correctly.
    let (claim_args, expected_assignee) = if half_claimed_by_receiver {
        (
            vec![
                "update".to_owned(),
                bead.to_owned(),
                "--assignee".to_owned(),
                receiver_agent.to_owned(),
                "--status".to_owned(),
                "in_progress".to_owned(),
            ],
            receiver_agent.to_owned(),
        )
    } else {
        (
            vec![
                "update".to_owned(),
                bead.to_owned(),
                "--claim".to_owned(),
                "--actor".to_owned(),
                supervisor.clone(),
            ],
            supervisor.clone(),
        )
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
        "bead={bead} pane={pane} assignee={expected_assignee} receiver_agent={receiver_agent}"
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
/// An unclaimed open bead is claimed atomically by this supervisor as
/// supervisor:<pid>, never by forging the receiver's ownership. The receiver
/// remains the transport target and may reassign the bead on first contact.
async fn prepare_bead_dispatch(
    cx: &Cx,
    config: &Config,
    pane: &str,
    bead: &str,
    identities: &IdentityRegistries,
    tick: u64,
    claim_enabled: bool,
) -> Result<(BeadSnapshot, String), String> {
    let initial = load_bead_snapshot(cx, config, bead).await?;
    let receiver_agent = receiver_agent_for_dispatch(config, pane, bead, &initial)?;
    ensure_dispatch_receiver_identity(identities, bead, pane, &receiver_agent)?;
    validate_receiver_pane(config, pane, bead, &receiver_agent)?;
    let (snapshot, claim_owner) = claim_bead_for_supervisor(
        cx,
        config,
        pane,
        bead,
        initial,
        &receiver_agent,
        tick,
        claim_enabled,
    )
    .await?;
    let receiver_agent =
        authorize_bead_dispatch_as(config, pane, bead, &snapshot, &claim_owner, identities)?;
    Ok((snapshot, receiver_agent))
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
        two_captures: pane_observation.is_dispatchable
            && pane_observation.liveness == "CONFIRMED_IDLE",
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
) -> Result<LifecycleLedger, String> {
    let bead_id = BeadId::new(bead).map_err(|error| error.to_string())?;
    let target = DispatchTarget::new(config.session.clone(), pane)
        .map_err(|error| error.to_string())?;
    let now_ms = now_unix().saturating_mul(1_000);
    let intent = Intent {
        action: TypedAction::DispatchPacket,
        pane_dispatchable: pane_observation.is_dispatchable,
        two_captures: pane_observation.is_dispatchable
            && pane_observation.liveness == "CONFIRMED_IDLE",
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
    let selected_id = EventId::new(format!("{bead}:selected:{tick}"))
        .map_err(|error| error.to_string())?;
    let selected = LedgerEvidence::single(
        selected_id,
        now_ms,
        EvidencePolicy::new(now_ms, 0),
        "decision",
        format!("dispatch preflight passed pane={pane}"),
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
}

pub fn gate_peer_grading(
    config: &Config,
    observation: &mut Observation,
    tick: u64,
) -> Result<Option<PeerGradeClaim>, String> {
    gate_peer_grading_inner(config, observation, tick, None)
}

pub fn gate_peer_grading_for_pane(
    config: &Config,
    observation: &mut Observation,
    tick: u64,
    grader_pane: &str,
) -> Result<Option<PeerGradeClaim>, String> {
    gate_peer_grading_inner(config, observation, tick, Some(grader_pane))
}

fn gate_peer_grading_inner(
    config: &Config,
    observation: &mut Observation,
    tick: u64,
    preferred_grader_pane: Option<&str>,
) -> Result<Option<PeerGradeClaim>, String> {
    let active = LifecycleLedger::active_grading_panes(&config.bead_lifecycle_ledger)
        .map_err(|error| format!("PEER_GRADING_LEDGER_UNREADABLE error={error}"))?;
    if !active.is_empty() {
        observation
            .panes
            .retain(|pane| !active.contains(&pane.pane_id));
        return Ok(Some(PeerGradeClaim {
            bead: "<active-peer-grade>".to_owned(),
            receiver_pane: "<ledger>".to_owned(),
            grader_pane: active.into_iter().collect::<Vec<_>>().join(","),
        }));
    }

    let candidates = LifecycleLedger::receiver_verified_candidates(&config.bead_lifecycle_ledger)
        .map_err(|error| format!("PEER_GRADING_LEDGER_UNREADABLE error={error}"))?;
    let idle = observation
        .panes
        .iter()
        .filter(|pane| pane.is_dispatchable && pane.liveness == "CONFIRMED_IDLE")
        .collect::<Vec<_>>();
    for candidate in candidates {
        let Some(receiver_pane) = idle
            .iter()
            .find(|pane| pane.pane_id == candidate.identity.target.pane)
            .map(|pane| pane.pane_id.clone())
        else {
            continue;
        };
        let grader_pane = if let Some(preferred) = preferred_grader_pane {
            if preferred == receiver_pane.as_str() {
                return Err(format!(
                    "PEER_GRADING_REFUSED bead={} receiver_pane={} reason=self_grade",
                    candidate.identity.bead.as_str(),
                    receiver_pane,
                ));
            }
            if !idle.iter().any(|pane| pane.pane_id == preferred) {
                return Err(format!(
                    "PEER_GRADING_REFUSED bead={} grader_pane={} reason=preferred_grader_not_idle",
                    candidate.identity.bead.as_str(),
                    preferred,
                ));
            }
            preferred.to_owned()
        } else {
            let Some(grader_pane) = idle
                .iter()
                .find(|pane| pane.pane_id != receiver_pane)
                .map(|pane| pane.pane_id.clone())
            else {
                return Err(format!(
                    "PEER_GRADING_REFUSED bead={} receiver_pane={} reason=no_distinct_idle_peer",
                    candidate.identity.bead.as_str(),
                    receiver_pane,
                ));
            };
            grader_pane
        };
        let grader = idle
            .iter()
            .find(|pane| pane.pane_id == grader_pane)
            .ok_or_else(|| format!("PEER_GRADING_REFUSED grader_pane={grader_pane} observation_row_missing"))?;
        let intent = Intent {
            action: TypedAction::DispatchPacket,
            pane_dispatchable: grader.is_dispatchable,
            two_captures: grader.is_dispatchable && grader.liveness == "CONFIRMED_IDLE",
            packet_complete: true,
            finding_has_bead: true,
        };
        let approval = Approved::authorize(classify(intent))
            .map_err(|error| format!("PEER_GRADING_REFUSED reason={error:?}"))?;
        let now_ms = now_unix().saturating_mul(1_000);
        let event_id = EventId::new(format!(
            "peer-grade:{}:{}:{}",
            candidate.identity.bead.as_str(),
            tick,
            grader_pane
        ))
        .map_err(|error| error.to_string())?;
        let evidence = LedgerEvidence::new(
            event_id,
            now_ms,
            EvidencePolicy::new(now_ms, 0),
            [
                ("source", "omp-orchestrator"),
                ("receiver_event_id", candidate.receiver_event_id.as_str()),
                ("receiver_pane", receiver_pane.as_str()),
                ("grader_pane", grader_pane.as_str()),
            ],
        )
        .map_err(|error| error.to_string())?;
        let bead = candidate.identity.bead.as_str().to_owned();
        LifecycleLedger::claim_peer_grading(
            &config.bead_lifecycle_ledger,
            candidate,
            approval,
            grader_pane.clone(),
            evidence,
        )
        .map_err(|error| format!("PEER_GRADING_REFUSED error={error}"))?;
        observation
            .panes
            .retain(|pane| pane.pane_id != grader_pane);
        return Ok(Some(PeerGradeClaim {
            bead,
            receiver_pane,
            grader_pane,
        }));
    }
    Ok(None)
}

static PANE_OCCUPANCY_MINT: LazyLock<IncarnationMint> = LazyLock::new(IncarnationMint::new);
static PANE_OCCUPANCIES: LazyLock<Mutex<BTreeMap<String, Occupancy>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));

/// Live occupancy check immediately before tmux/ntm send. Not at enqueue.
fn admit_immediately_before_send(session: &str, pane: &str) -> Result<(), String> {
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
    let presented = Presented {
        session: session.to_owned(),
        pane: pane.to_owned(),
        incarnation: occupancy.current(),
        marker_pid: Some(live),
    };
    admit_at_send(occupancy, &presented, live).map_err(|refusal| {
        format!("DISPATCH_BLOCKED pane={pane} incarnation_refused={refusal:?}")
    })?;
    Ok(())
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
) -> Result<AckStageResult, String> {
    let packet = dispatch_packet::render_with_pane(
        snapshot,
        &config.repo,
        Some(pane),
        Some(receiver_agent),
        None,
        None,
    )
    .map_err(|error| format!("DISPATCH_PACKET_REFUSED bead={bead} pane={pane} error={error}"))?;
    authorize_dispatch_preflight(pane_observation, &packet, bead, pane)?;
    println!(
        "DISPATCH_PREFLIGHT verdict=autonomous bead={bead} pane={pane} pane_dispatchable={} two_captures={} packet_complete={}",
        pane_observation.is_dispatchable,
        pane_observation.is_dispatchable && pane_observation.liveness == "CONFIRMED_IDLE",
        packet_is_complete(&packet),
    );
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
    let mut lifecycle = begin_dispatch_lifecycle(config, pane, pane_observation, bead, &packet, tick)
        .map_err(|error| format!("LIFECYCLE_LEDGER_REFUSED bead={bead} pane={pane} error={error}"))?;
    let dispatch_at_ms = now_unix().saturating_mul(1_000);
    let dispatch_id = EventId::new(format!("{bead}:dispatch:{tick}"))
        .map_err(|error| error.to_string())?;
    let dispatch_receipt = DispatchReceipt::new(
        dispatch_id.clone(),
        BeadId::new(bead).map_err(|error| error.to_string())?,
        DispatchTarget::new(config.session.clone(), pane)
            .map_err(|error| error.to_string())?,
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
        .map_err(|error| format!("LIFECYCLE_LEDGER_REFUSED bead={bead} pane={pane} error={error}"))?;
    admit_immediately_before_send(&config.session, pane)?;
    let codex = receiver_is_codex(cx, config, pane).await?;
    let transport = if codex {
        let typed_args = vec![
            "send-keys".to_owned(),
            "-t".to_owned(),
            pane.to_owned(),
            "-l".to_owned(),
            packet.clone(),
        ];
        let typed_stdout = require_success(
            "tmux send-keys -l",
            invoke(cx, config, "tmux", &typed_args).await?,
        )?;
        let enter_args = vec![
            "send-keys".to_owned(),
            "-t".to_owned(),
            pane.to_owned(),
            "Enter".to_owned(),
        ];
        let enter_stdout = require_success(
            "tmux send-keys Enter",
            invoke(cx, config, "tmux", &enter_args).await?,
        )?;
        TransportReceipt::capture_codex(
            "tmux send-keys -l; tmux send-keys Enter",
            &typed_stdout,
            &enter_stdout,
            Some(0),
        )
    } else {
        let send_args = vec![
            format!("--robot-send={}", config.session),
            format!("--panes={pane}"),
            format!("--msg-file={}", staged.display()),
        ];
        let stdout = require_success(
            &config.ntm,
            invoke(cx, config, &config.ntm, &send_args).await?,
        )?;
        TransportReceipt::capture_ntm(&stdout).map_err(|error| {
            format!("DISPATCH_BLOCKED bead={bead} malformed ntm receipt: {error}")
        })?
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
        match invoke(cx, config, "tmux", &list_args).await {
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
        let (post_send, pane_capture) = post_send_observation(cx, config, pane).await;
        if let receiver_receipt::PostSendObservation::Present(observation) = &post_send {
            if first_working.is_none()
                && matches!(observation.state, receiver_receipt::PaneState::Working { .. })
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
            let receiver_id = EventId::new(format!("{bead}:receiver:{tick}"))
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
            return Ok(stage);
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
                        "send-keys".to_owned(),
                        "-t".to_owned(),
                        pane.to_owned(),
                        "-l".to_owned(),
                        packet.clone(),
                    ];
                    require_success(
                        "tmux resend send-keys -l",
                        invoke(cx, config, "tmux", &resend_args).await?,
                    )?;
                    let enter_args = vec![
                        "send-keys".to_owned(),
                        "-t".to_owned(),
                        pane.to_owned(),
                        "Enter".to_owned(),
                    ];
                    require_success(
                        "tmux resend Enter",
                        invoke(cx, config, "tmux", &enter_args).await?,
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
                        "send-keys".to_owned(),
                        "-t".to_owned(),
                        pane.to_owned(),
                        "Enter".to_owned(),
                    ];
                    require_success(
                        "tmux recovery Enter",
                        invoke(cx, config, "tmux", &enter_args).await?,
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
            return Err(format!(
                "ACK_STAGE_INDETERMINATE pane={pane} bead={bead} action={} verdict={} reason={} transport={}",
                stage.action.label(),
                stage.delivery.label(),
                reason,
                transport.kind().label(),
            ));
        }
        if retry_exhausted || Instant::now() >= deadline {
            let reason = stage
                .delivery
                .reason()
                .map(ToString::to_string)
                .unwrap_or_else(|| "unclassified".to_owned());
            // iis6: THE WINDOW DECIDES WHEN WE RE-CHECK, NOT WHETHER A HUMAN IS
            // CALLED. Before this, every expired wait returned
            // `ack_readback_missing` — the same verdict for a pane mid-tool-call and
            // a dead one. Observed timers sat at 120s, 1320s and 1440s inside a
            // single tool call, so no constant separates those states; the
            // discriminator is whether the timer ADVANCED across
            // `OBSERVATION_WINDOW_MIN_SECS`.
            let discriminated = match (&first_working, &latest) {
                (Some(first), Some(last)) => {
                    Some(receiver_receipt::classify_ack_wait(first, last))
                }
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
                .unwrap_or_else(|| " discriminator=NO_WORKING_CAPTURE owes_human=unknown".to_owned());
            let redispatch_at_ms = now_unix().saturating_mul(1_000);
            let redispatch_id = EventId::new(format!("{bead}:redispatch-required:{tick}"))
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
            return Err(format!(
                "ACK_STAGE_RETRY_BLOCKED pane={pane} bead={bead} action={action} verdict={} reason={} after={}s{discriminator}",
                stage.delivery.label(),
                reason,
                RECEIPT_TIMEOUT.as_secs(),
            ));
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
            eprintln!(
                "LIFECYCLE_MONITOR_{} {error}",
                layer.as_str()
            );
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

    let receipt = build_receipt(&mut Receipt {
            ts: now_unix() as u64,
            tick: tick,
            free_capacity: &free,
            attention: attention,
            dead: dead,
            source: "omp-orchestrator supervisor observation (tick-monitor + br ready)",
            dispositions: &dispositions,
            fallback_reason: &format!("decision={label}; this tick did not dispatch to this pane"),
            claims: vec![serde_json::json!({
            "figure": format!("free_capacity={} attention={attention} dead={}", free.len(), match dead { Some(count) => count.to_string(), None => "unmeasured".to_owned() }),
            "command": "./target/debug/omp-orchestrator --once --session <session> (this row's own tick)",
        })],
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
    let row = serde_json::json!({
        "ts_unix": now_unix(),
        "event": "supervisor_heartbeat",
        "build_id": BUILD_ID,
        "status": status,
        "tick": tick,
        "pid": std::process::id(),
        "repo": config.repo.display().to_string(),
        "session": config.session,
        "detail": detail,
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
    // the two that happened to fire today would leave the next refusal shape
    // unrecorded, which is the hand-maintained-list defect this repo keeps paying
    // for. One call here covers `GATE_UNWIRED`, `DISPATCH_BLOCKED … owner=josh`,
    // `MONITOR_BLIND`, `ACK_STAGE_RETRY_BLOCKED action=AWAIT_HUMAN`, and any future
    // shape — `classify_heartbeat` returns `None` for rows that address nobody, and
    // carries an UNCLASSIFIED request rather than dropping a row it did not expect.
    //
    // A LEDGER FAILURE MUST NOT KILL A TICK. The decision ledger is a record, not a
    // gate: refusing the heartbeat because the record failed would convert a
    // bookkeeping fault into a fleet outage. So the outcome is REPORTED on stderr and
    // the tick proceeds — and it is reported, not swallowed, because a writer that
    // fails silently is the defect this whole bead is about.
    if let Some(request) = decision_ledger::classify_heartbeat(status, detail, now_unix()) {
        let ledger = config.repo.join("docs/decisions.jsonl");
        match decision_ledger::append_request(&ledger, &request) {
            // Deduped is the common case by design: 188 identical ticks are ONE
            // question. Silent, or the log becomes the thing it replaced.
            Ok(outcome) if !outcome.wrote() => {}
            Ok(outcome) => {
                let _ = writeln!(
                    io::stderr(),
                    "S9_REQUEST_RECORDED id={} blocking={} question={}",
                    outcome.id(),
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
fn process_pending_markers(config: &Config, tick: u64) -> Result<MarkerFence, String> {
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
            PendingDispatch::Live { detail, age_secs } => {
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

    let out = std::process::Command::new("df")
        .args(["-k", &probe.display().to_string()])
        .output()
        .map_err(|e| format!("DISK_PRESSURE df failed to spawn: {e}"))?;
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
    config.pending_dispatch.with_file_name(format!("{base}.{slug}"))
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
    vec![
        "--repo".to_owned(),
        config.repo.display().to_string(),
        "--session".to_owned(),
        config.session.clone(),
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
        format!("--robot-send={session}"),
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
    identity
        .agent_name
        .clone()
        .ok_or_else(|| format!("SENDER_IDENTITY_REFUSED pane={} reason=agent_name_missing", identity.pane_id))
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
    let advisory_census = crate::census_gates(&config.repo);
    let advisory = advisory_census.advisory_gates();
    let overdue = omp_orchestrator::advisory_ratchet_overdue(
        now_unix(),
        omp_orchestrator::ADVISORY_CEILING_RECORDED_AT_UNIX,
        DEFAULT_INTERVAL.as_secs(),
        omp_orchestrator::ADVISORY_RATCHET_DEADLINE_TICKS,
        advisory.len(),
        omp_orchestrator::ADVISORY_CEILING,
    );
    println!(
        "CENSUS_ADVISORY count={} ceiling={} rows={} blocking={} {} \
         next_action=wire-or-retire-one-advisory-crate",
        advisory.len(),
        omp_orchestrator::ADVISORY_CEILING,
        advisory_census.rows.len(),
        advisory_census
            .rows
            .iter()
            .filter(|r| r.disposition.is_blocking())
            .count(),
        if overdue {
            "CENSUS_ADVISORY_RATCHET_OVERDUE owner=josh -- the advisory count has not \
             decreased inside the deadline; advisory-first has failed its own falsifier and \
             triage-first was the right call"
        } else {
            "ratchet=on-time"
        }
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
            write_heartbeat(config, tick, "SPINE_RECONCILE_DEGRADED", &one_line_detail(&error))?;
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
    let marker_blocked_panes: Vec<String> = match process_pending_markers(config, tick)? {
        MarkerFence::Proceed(outcome) => {
            if outcome.cleared.is_empty() && outcome.blocked_panes.is_empty() {
                write_heartbeat(
                    config,
                    tick,
                    "NO_PENDING_DISPATCH",
                    "owner=loop next_action=continue",
                )?;
            }
            outcome.blocked_panes
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
    match docs_are_stale(config)? {
        DocsVerdict::Fresh => {}
        DocsVerdict::Stale(why) => {
            write_heartbeat(config, tick, "DOCS_STALE", &why)?;
            let detail = format!(
                "DOCS_STALE owner=josh next_action=re-assemble-docs/PLAN.md detail={why} \
                 authority=HD-0001"
            );
            println!("{detail}");
            return Ok(());
        }
        // NAMED, never a silent green: a reader must be able to tell "this repo has no
        // assembly to be stale" from "the assembly is fresh". Both continue the tick; only
        // one of them is a measurement of an assembly.
        DocsVerdict::NotApplicable(why) => {
            write_heartbeat(config, tick, "DOCS_ASSEMBLY_NOT_APPLICABLE", &why)?;
            println!(
                "DOCS_ASSEMBLY_NOT_APPLICABLE owner=loop next_action=continue scope=repo \
                 detail={why} authority=HD-0001"
            );
        }
    }

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
    let mut observation = parse_observation(&monitor_bytes, census_gates(&config.repo))?;
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
        observation
            .panes
            .retain(|pane| !marker_blocked_panes.iter().any(|held| held == &pane.pane_id));
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
    let ready_args = vec!["ready".to_owned(), "--json".to_owned()];
    let ready_output = invoke(cx, config, &config.br, &ready_args).await?;
    let ready = require_success(&config.br, ready_output).map_err(|error| {
        format!("QUEUE_UNREADABLE owner=josh next_action=repair-br-or-escalate: {error}")
    })?;
    let ready_ids = parse_ready(&ready)?;
    // PRIORITY AND GRAPH ORDER, not creation order. A silent FIFO fallback is exactly
    // the defect (`2ceb`), so an unreadable ranking is a TYPED REFUSAL that stops the
    // cycle rather than a quiet degradation to whatever `br` returned first.
    let triage_args = vec!["--robot-triage".to_owned()];
    let bead_ids = match invoke(cx, config, &config.bv, &triage_args).await {
        Ok(output) => {
            let triage = require_success(&config.bv, output)
                .map_err(|error| format!("QUEUE_UNRANKED owner=josh next_action=repair-bv: {error}"))?;
            rank_ready(&triage, &ready_ids)?
        }
        Err(error) => {
            return Err(format!(
                "QUEUE_UNRANKED owner=josh next_action=repair-bv-or-escalate: {error}"
            ));
        }
    };
    observation.queue = QueueState {
        ready_count: bead_ids.len(),
        readable: true,
    };
    // DISK PRESSURE. Checked after observation and queue read, before any dispatch
    // step can write. A full build volume fails as a LINKER error that reads like a
    // code defect, but this gate must not suppress read-only pane observation.
    //
    // Measured 2026-09-01: /Volumes/BuildShared reached 99% and every gate stopped
    // with No space left on device (os error 28). The protection and 8% free / 1 GiB
    // threshold remain unchanged; only placement is different.
    //
    // A guard that observes and does not refuse is a note. This refuses before dispatch.
    if let Some(why) = disk_pressure(config)? {
        write_heartbeat(config, tick, "DISK_PRESSURE", &why)?;
        println!("DISK_PRESSURE owner=josh next_action=cargo-clean-or-grow-volume detail={why}");
        return Ok(());
    }
    let authorization = applicable(
        read_idle_authorization(&config.repo, now_unix()),
        &config.session,
        &observation.panes,
        &observation.queue,
    );
    let decision = decide(&observation, &authorization);
    if matches!(&decision, SupervisorDecision::Dispatch { .. }) {
        if let Some(claim) = gate_peer_grading(config, &mut observation, tick)? {
            let detail = format!(
                "bead={} receiver_pane={} grader_pane={} requirement=peer-grading-before-new-work",
                claim.bead, claim.receiver_pane, claim.grader_pane
            );
            write_heartbeat(config, tick, "PEER_GRADING_REQUIRED", &detail)?;
            println!(
                "PEER_GRADING_REQUIRED tick={tick} session={} {detail}",
                config.session
            );
            return Ok(());
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
            let bead = bead_ids.first().ok_or_else(|| {
                "QUEUE_UNREADABLE ready count changed before bead selection".to_owned()
            })?;
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
            let mut spine = ack_spine::ledger::StepLedger::new();
            emit_step(cx, &mut spine, StepKind::BeadSelected, bead, &pane, config, "selected from the bv-ordered ready queue").await?;
            let identities = load_identity_registries(cx, config).await?;
            let dispatch_result = async {
                let (snapshot, receiver_agent) =
                    prepare_bead_dispatch(cx, config, &pane, bead, &identities, tick, true).await?;
                let pane_observation = observation
                    .panes
                    .iter()
                    .find(|candidate| candidate.pane_id == pane)
                    .cloned()
                    .ok_or_else(|| format!("DISPATCH_PREFLIGHT_REFUSED bead={bead} pane={pane} observation_row_missing"))?;
                let dispatch_epoch = now_unix() as i64;
                write_dispatch_intent(config, &pane, bead)?;
                emit_step(cx, &mut spine, StepKind::FenceChecked, bead, &pane, config, "per-pane dispatch fence passed; intent written").await?;
                let before = capture_pane(cx, config, &pane).await?;
                emit_step(cx, &mut spine, StepKind::PacketRendered, bead, &pane, config, &format!("receiver={receiver_agent}")).await?;
                let prior = prior_dispatch_count(config, bead);
                let send = omp_orchestrator::spine_emit::send_kind(prior);
                let stage = send_and_verify(
                    cx,
                    config,
                    &pane,
                    &pane_observation,
                    bead,
                    &receiver_agent,
                    &snapshot,
                    &before,
                    tick,
                )
                .await?;
                emit_step(cx, &mut spine, send, bead, &pane, config, &format!("prior_dispatches={prior} verdict={}", stage.delivery.label())).await?;
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
                            clear_intent: true,
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
                            clear_intent: false,
                        })
                    }
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
                        clear_dispatch_intent(config, &pane)?;
                        write_heartbeat(
                            config,
                            tick,
                            "DISPATCH_INTENT_CLEARED",
                            &format!(
                                "pane={pane} bead={bead} reason=VERDICT_POSTED owner=loop next_action=continue"
                            ),
                        )?;
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

async fn recover_pending_findings(cx: &Cx, config: &Config, tick: u64) -> Result<(), String> {
    fs::create_dir_all(&config.finding_spool).map_err(|error| {
        format!(
            "FINDING_RECOVERY_REFUSED spool={} error={error}",
            config.finding_spool.display()
        )
    })?;
    let publisher = BrPublisher::new(config.br.clone(), config.repo.clone());
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
            let publisher = BrPublisher::new(config.br.clone(), config.repo.clone());
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
            return Err(error);
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
    let identities = load_identity_registries(cx, config).await?;
    let receiver_agent =
        receiver_agent_for_dispatch(config, &request.pane, &request.bead, &snapshot)?;
    ensure_dispatch_receiver_identity(&identities, &request.bead, &request.pane, &receiver_agent)?;
    authorize_bead_dispatch_as(
        config,
        &request.pane,
        &request.bead,
        &snapshot,
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
    dispatch_packet::render_with_pane(
        &snapshot,
        &config.repo,
        Some(&request.pane),
        None,
        request.why_now.as_deref(),
        traps.as_deref(),
    )
    .map_err(|error| {
        format!(
            "PACKET_RENDER_REFUSED bead={} pane={} error={error}",
            request.bead, request.pane
        )
    })
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

fn main() -> std::process::ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
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
        request.clone()
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
            eprintln!("SUPERVISOR_REFUSED runtime={error}");
            return std::process::ExitCode::from(1);
        }
    };
    if grade_request.is_some() {
        let grader_pane = env::var("TMUX_PANE").unwrap_or_default();
        if grader_pane.trim().is_empty() {
            eprintln!("PEER_GRADING_REFUSED reason=current_pane_unresolved");
            return std::process::ExitCode::from(2);
        }
        let outcome = runtime.block_on(async {
            let cx = Cx::current()
                .ok_or_else(|| "SUPERVISOR_REFUSED no runtime context".to_owned())?;
            run_peer_grade_claim(&cx, &config, &grader_pane).await
        });
        return match outcome {
            Ok(outcome) => peer_grade_command_exit(outcome),
            Err(error) => {
                eprintln!("{error}");
                std::process::ExitCode::from(2)
            }
        };
    }

    if let Some(request) = dispatch_request {
        let outcome = runtime.block_on(async {
            let cx = Cx::current()
                .ok_or_else(|| "SUPERVISOR_REFUSED no runtime context".to_owned())?;
            render_dispatch_command(&cx, &config, &request).await
        });
        return match outcome {
            Ok(packet) => {
                print!("{packet}");
                std::process::ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::ExitCode::from(1)
            }
        };
    }

    if let Some(request) = close_request {
        let outcome = runtime.block_on(async {
            let cx = Cx::current()
                .ok_or_else(|| "SUPERVISOR_REFUSED no runtime context".to_owned())?;
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
        let cx = Cx::current().ok_or_else(|| "SUPERVISOR_REFUSED no runtime context".to_owned())?;
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
            eprintln!("SUPERVISOR_REFUSED {error}");
            std::process::ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            tick_monitor: "tick-monitor".to_owned(),
            run_subcommand: false,
            br: "br".to_owned(),
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
        let (_g, config) = shape_fixture(&[("01-intro.md", "a section body long enough to probe")], None);
        match docs_are_stale(&config) {
            Ok(DocsVerdict::Stale(why)) => {
                assert!(why.contains("assembly absent"), "{why}");
                assert!(why.contains("1 numbered sections") || why.contains("while 1"), "{why}");
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
        let (_g1, fresh) = shape_fixture(&[("01-a.md", body)], Some(&format!("preamble\n{body}\n")));
        assert!(
            matches!(docs_are_stale(&fresh), Ok(DocsVerdict::Fresh)),
            "a section contained in the assembly is Fresh"
        );
        // STALE, fires-on-known-bad: same shape, section NOT in the assembly.
        let (_g2, stale) = shape_fixture(&[("01-a.md", body)], Some("preamble only\n"));
        match docs_are_stale(&stale) {
            Ok(DocsVerdict::Stale(why)) => assert!(why.contains("01-a.md"), "must NAME the section: {why}"),
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

    fn run_prepare_for_test(
        config: &Config,
        pane: &str,
        bead: &str,
        tick: u64,
        claim_enabled: bool,
    ) -> Result<(BeadSnapshot, String), String> {
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            let identities = test_identity_registries();
            prepare_bead_dispatch(&cx, config, pane, bead, &identities, tick, claim_enabled).await
        })
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
            GateCensus { rows: Vec::new() },
        )
        .unwrap();
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
            intent_clear_reason(
                "RECEIVER_OBSERVATION_MISSING pane=%1413 identity row absent"
            ),
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
        let ready = vec![next.to_owned(), "omp-orchestrator-third".to_owned()];
        let cleared_beads: Vec<String> = outcome.cleared.iter().map(|row| row.1.clone()).collect();
        let selected = select_next_bead(&ready, &cleared_beads).expect("ready queue");
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
        let path = plant_intent(
            &config,
            "%1413",
            "omp-orchestrator-first-bead",
            now_unix(),
        );
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
    fn run_subcommand_parses_with_once() {
        let args = ["run".to_owned(), "--once".to_owned()];
        let config = Config::from_args(&args).unwrap();
        assert!(config.run_subcommand);
        assert_eq!(config.max_ticks, Some(1));
    }

    #[test]
    fn flag_only_invocation_is_unchanged_for_launchd() {
        let args = ["--once".to_owned()];
        let config = Config::from_args(&args).unwrap();
        assert!(
            !config.run_subcommand,
            "flag-only form must not require the subcommand"
        );
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
    fn two_configs_different_sessions_resolve_different_state_paths() {
        assert!(
            std::env::var_os("OMP_TICK_MONITOR_STATE").is_none(),
            "this test measures the default path; OMP_TICK_MONITOR_STATE overrides it"
        );
        let a = Config::from_args(&["--session".to_owned(), "control-plane".to_owned()]).unwrap();
        let b = Config::from_args(&[
            "--session".to_owned(),
            "omp-orchestrator".to_owned(),
        ])
        .unwrap();
        assert_ne!(
            a.tick_monitor_state, b.tick_monitor_state,
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
        let stray = Config::from_args(&["run".to_owned(), "extra".to_owned()]).unwrap_err();
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
    fn help_reports_the_run_entrypoint() {
        let help = Config::from_args(&["--help".to_owned()]).unwrap_err();
        assert_eq!(help, usage());
        assert!(
            help.contains("[run]"),
            "usage must advertise the run subcommand"
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
    printf '[{{"id":"{bead}","title":"title","description":"description","status":"in_progress","assignee":"{supervisor}"}}]\n'
  else
    printf '[{{"id":"{bead}","title":"title","description":"description","status":"open","assignee":null}}]\n'
  fi
  exit 0
fi
if [ "$1" = "update" ]; then
  printf '%s\n' "$@" > "{args_path}"
  touch "{state_path}"
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
        let temp = tempfile::tempdir().expect("claim fixture tempdir");
        let bead = "omp-orchestrator-mj8w";
        let (config, state, args, supervisor) = open_bead_br_fixture(&temp, bead);

        let (snapshot, receiver) = run_prepare_for_test(&config, "%1408", bead, 17, true)
            .unwrap_or_else(|error| {
                panic!(
                    "supervisor claim failed: {error}; args={:?}",
                    std::fs::read_to_string(&args)
                )
            });

        assert_eq!(receiver, "GreenFrog");
        assert_eq!(snapshot.status_label(), "in_progress");
        assert_eq!(snapshot.assignee(), Some(supervisor.as_str()));
        let claim_args = std::fs::read_to_string(args).expect("claim command receipt");
        assert!(claim_args.contains("update"), "{claim_args}");
        assert!(claim_args.contains(bead), "{claim_args}");
        assert!(claim_args.contains(&supervisor), "{claim_args}");
        assert!(
            !claim_args.contains("GreenFrog"),
            "receiver must not be forged: {claim_args}"
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
    fn disabling_supervisor_claim_preserves_known_bad_refusal() {
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
        assert_eq!(
            snapshot.assignee(),
            Some(format!("supervisor:{}", std::process::id()).as_str())
        );
        assert!(
            state.exists(),
            "restored claim transition must perform the claim"
        );
    }

    /// The dispatch path must never forge a receiver-owned claim.
    ///
    /// An unclaimed bead may be claimed only by the supervisor identity. The
    /// receiver remains the transport target and is never written as the
    /// assignee by dispatch preparation.
    #[test]
    fn dispatch_path_never_claims_on_the_receivers_behalf() {
        let source = include_str!("main.rs");
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
            "run".to_owned(),
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

    /// KNOWN-BAD for `2ceb`: a P0 articulation point must outrank an OLDER P1 leaf.
    ///
    /// Under creation order the P1 leaf wins because it is first in `br ready`. This is
    /// the leg that fails on the pre-fix code, and it is the whole point of the bead:
    /// measured live, the loop dispatched four P1 beads while 26 P0s sat ready.
    #[test]
    fn ranking_prefers_a_p0_articulation_point_over_an_older_p1_leaf() {
        let ready = vec!["old-p1-leaf".to_owned(), "p0-articulation".to_owned()];
        let triage = br#"{"triage":{"recommendations":[
            {"id":"old-p1-leaf","priority":1,"score":0.9},
            {"id":"p0-articulation","priority":0,"score":0.2}
        ]}}"#;
        let ranked = rank_ready(triage, &ready).expect("ranked");
        assert_eq!(
            ranked.first().map(String::as_str),
            Some("p0-articulation"),
            "priority outranks both creation order and a higher graph score: {ranked:?}"
        );
    }

    /// Within one priority, the GRAPH score decides — otherwise priority alone
    /// re-creates easy-bead cherry-picking, where many P0 leaves outrank the one P0
    /// whose closure unblocks them.
    #[test]
    fn within_a_priority_the_graph_score_decides() {
        let ready = vec!["low".to_owned(), "high".to_owned()];
        let triage = br#"{"triage":{"recommendations":[
            {"id":"low","priority":0,"score":0.10},
            {"id":"high","priority":0,"score":0.80}
        ]}}"#;
        let ranked = rank_ready(triage, &ready).expect("ranked");
        assert_eq!(ranked.first().map(String::as_str), Some("high"));
    }

    /// Epics and already-assigned rows are excluded, and every ready id still survives.
    ///
    /// An epic's PageRank accumulates from every child, so it tops the list and can
    /// never close until its children do; an assigned bead must not be re-offered. But
    /// exclusion from the RANKED HEAD must not drop a bead from the queue entirely.
    #[test]
    fn epics_and_assigned_rows_are_not_ranked_but_are_not_lost() {
        let ready = vec!["epic".to_owned(), "taken".to_owned(), "free".to_owned()];
        let triage = br#"{"triage":{"recommendations":[
            {"id":"epic","priority":0,"score":0.99,"type":"epic"},
            {"id":"taken","priority":0,"score":0.98,"assignee":"pane3-%8"},
            {"id":"free","priority":2,"score":0.01}
        ]}}"#;
        let ranked = rank_ready(triage, &ready).expect("ranked");
        assert_eq!(ranked.first().map(String::as_str), Some("free"));
        assert_eq!(ranked.len(), 3, "no ready id may be dropped: {ranked:?}");
    }

    /// ANTI-VACUITY: a triage payload with no recommendations array is a typed REFUSAL,
    /// never a silent fall back to creation order. A quiet FIFO degradation is the
    /// defect `2ceb` names, and it would be indistinguishable from working ranking.
    #[test]
    fn missing_recommendations_is_a_typed_refusal_not_silent_fifo() {
        let ready = vec!["a".to_owned()];
        for payload in [
            &br#"{"triage":{}}"#[..],
            &br#"{"triage":{"quick_ref":{"top_picks":["a"]}}}"#[..],
            &br#"{}"#[..],
        ] {
            let error = rank_ready(payload, &ready).expect_err("must refuse");
            assert!(
                error.starts_with("QUEUE_UNRANKED"),
                "refusal must be typed and named: {error}"
            );
        }
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
        assert_eq!(args[0], "--robot-send=test-session");
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
        assert!(args.lines().any(|line| line == "--robot-send=test-session"));
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
             never `br ready` - see the comment at the call site"
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
        let temp = tempfile::tempdir().expect("supervisor finding fixture");
        let heartbeat = temp.path().join("heartbeat.jsonl");
        let config = fixture_config(heartbeat.clone());
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            let mut init = Command::new("br");
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
            close_and_read_back(&cx, &config, "bead", "prose reason")
                .await
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
            close_and_read_back(&cx, &config, "bead", "DONE: verified")
                .await
        });
        assert_eq!(result, CloseReadback::Closed { status: "closed".to_owned() });
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
            close_and_read_back(&cx, &config, "bead", "DONE: verified")
                .await
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
    fn lifecycle_selection_preflight_rejects_single_capture_independently() {
        let (_temp, config) = isolated_fixture_config();
        let pane = PaneObservation {
            pane_id: "%7".to_owned(),
            state: "IDLE".to_owned(),
            liveness: "UNPROVEN".to_owned(),
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
        let error = begin_dispatch_lifecycle(&config, "%7", &pane, "bead", packet, 7)
            .expect_err("the lifecycle selection site must reject one-capture liveness");
        assert!(error.contains("SingleCaptureLiveness"), "{error}");
    }

    #[test]
    fn dispatch_preflight_accepts_confirmed_idle_complete_packet() {
        let pane = PaneObservation {
            pane_id: "%7".to_owned(),
            state: "IDLE".to_owned(),
            liveness: "CONFIRMED_IDLE".to_owned(),
            is_dispatchable: true,
            is_free_capacity: true,
            is_working: false,
            awaits_human: false,
        };
        let packet = "Objective: x\nTarget: y\nScope:\nreal\nAcceptance:\nrun\nDone: exit 0\nStop: now\n";
        authorize_dispatch_preflight(&pane, packet, "bead", "%7")
            .expect("confirmed two-capture liveness and complete packet must authorize");
    }

    #[test]
    fn packet_completeness_rejects_an_identifier_only_body() {
        assert!(!packet_is_complete("bead-only"));
        assert!(packet_is_complete(
            "Objective: x\nTarget: y\nScope:\nreal\nAcceptance:\nrun\nDone: exit 0\nStop: now\n"
        ));
    }
    #[test]
    fn peer_grade_claim_blocks_new_work_until_a_distinct_pane_is_named() {
        let (temp, config) = isolated_fixture_config();
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

        let claim = gate_peer_grading(&config, &mut observation, 77)
            .unwrap()
            .expect("a finished peer bead must claim grading before new work");
        assert_eq!(claim.bead, "peer-bead");
        assert_eq!(claim.receiver_pane, "%1409");
        assert_eq!(claim.grader_pane, "%1414");
        assert_eq!(
            LifecycleLedger::active_grading_panes(&config.bead_lifecycle_ledger)
                .unwrap()
                .into_iter()
                .collect::<Vec<_>>(),
            vec!["%1414".to_owned()]
        );
        assert!(observation.panes.iter().all(|pane| pane.pane_id != "%1414"));
        drop(temp);
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
        assert_eq!(request, vec!["--repo".to_owned(), "/repo".to_owned()]);
        let error = parse_grade_claim_args(&["grade".to_owned()])
            .expect_err("bare grade must refuse");
        assert!(error.contains("--claim"), "{error}");
    }

    #[test]
    fn empty_peer_candidate_is_a_typed_outcome_not_success() {
        assert_eq!(peer_grade_outcome_wire(&PeerGradeCommandOutcome::NoCandidate), "PEER_GRADE_EMPTY");
        assert_eq!(peer_grade_outcome_wire(&PeerGradeCommandOutcome::ActivePeerGrade), "PEER_GRADE_ACTIVE");
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
        admit_immediately_before_send("omp-orchestrator", "%9")
            .expect("live occupancy must admit");
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
            Err(pane_dispatch_fence::AdmissionRefusal::StaleIncarnation {
                presented,
                current,
            }) => {
                assert_eq!(presented, first);
                assert_eq!(current, second);
            }
            other => panic!("expected StaleIncarnation, got {other:?}"),
        }
    }

}

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
use asupersync::process::{Command, Output};
use asupersync::runtime::RuntimeBuilder;
use asupersync::time::{sleep, timeout};
use asupersync::Cx;
use dispatch_claim_fence::{authorize, parse_br_show_json, BeadSnapshot, DispatchIntent};
use dispatch_silence_watch::SilenceVerdict;
use omp_orchestrator::{
    applicable, census_gates, decide, read_idle_authorization, GateCensus, Observation,
    PaneObservation, QueueState, SupervisorDecision,
};
use omp_rpc_session::{
    run_session, OmpCommand, RpcError, RpcSessionConfig, NO_CLAIM_BOUNDARY, OMP_RPC_SCHEMA_VERSION,
    OMP_SURFACE,
};
use receiver_receipt::{
    escalate_non_delivery, observe_capture, ComposerEvidence, NonDeliveryEscalation,
    PostSendObservation, ReceiptReason, ReceiptVerdict,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use subprocess_contract::run_output;

const DEFAULT_INTERVAL: Duration = Duration::from_secs(90);
const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const RECEIPT_TIMEOUT: Duration = Duration::from_secs(30);
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
#[derive(Debug)]
struct Config {
    repo: PathBuf,
    session: String,
    interval: Duration,
    run_subcommand: bool,
    command_timeout: Duration,
    max_ticks: Option<u64>,
    tick_monitor: String,
    br: String,
    ntm: String,
    tmux_tmpdir: PathBuf,
    exclude_panes: Vec<String>,
    heartbeat_ledger: PathBuf,
    tick_monitor_state: PathBuf,
    pending_dispatch: PathBuf,
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
        let tick_monitor_state = env::var_os("OMP_TICK_MONITOR_STATE")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                heartbeat_ledger.with_file_name("omp-orchestrator.tick-monitor-state.json")
            });
        let pending_dispatch = env::var_os("OMP_PENDING_DISPATCH")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                heartbeat_ledger.with_file_name("omp-orchestrator.pending-dispatch")
            });
        Ok(Self {
            repo,
            session,
            interval,
            command_timeout,
            max_ticks,
            tick_monitor: env::var("OMP_TICK_MONITOR_BIN")
                .unwrap_or_else(|_| "tick-monitor".to_owned()),
            br: env::var("OMP_BR_BIN").unwrap_or_else(|_| "br".to_owned()),
            ntm: env::var("OMP_NTM_BIN").unwrap_or_else(|_| "ntm".to_owned()),
            tmux_tmpdir,
            run_subcommand,
            exclude_panes,
            heartbeat_ledger,
            tick_monitor_state,
            pending_dispatch,
            receiver_agent,
            mail_sender: MAIL_IDENTITY_VARS
                .iter()
                .find_map(|key| {
                    env::var(key)
                        .ok()
                        .map(|value| value.trim().to_owned())
                        .filter(|value| !value.is_empty())
                })
                .unwrap_or_default(),
            omp_quick,
            reap_finished_panes: env::var("OMP_REAP_FINISHED_PANES_BIN")
                .unwrap_or_else(|_| "reap-finished-panes".to_owned()),
            omp_binary,
        })
    }
}
fn usage() -> &'static str {
    "usage: omp-orchestrator [run] [--once|--max-ticks N] [--repo PATH] [--session NAME] [--interval-secs N] [--receiver-agent NAME] [--omp-quick] [--omp-binary PATH]\n       `run` is the explicit resident lifecycle entrypoint (observe -> ready queue -> dispatch -> receiver receipt); the flag-only form is unchanged for launchd"
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
    (
        PostSendObservation::Present(observe_capture(pane, &text, now_unix())),
        Some(text),
    )
}

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
    AckReadback::from_comments_json(bead, pane, &bytes).map_err(|error| {
        format!("ACK_STAGE_INDETERMINATE bead={bead} pane={pane} comment read-back: {error}")
    })
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

async fn load_bead_snapshot(cx: &Cx, config: &Config, bead: &str) -> Result<BeadSnapshot, String> {
    let show_args = vec!["show".to_owned(), bead.to_owned(), "--json".to_owned()];
    let show = require_success(
        &config.br,
        invoke(cx, config, &config.br, &show_args).await?,
    )?;
    parse_br_show_json(&show).map_err(|error| format!("DISPATCH_BLOCKED bead={bead} {error}"))
}

fn receiver_agent_for_dispatch(
    config: &Config,
    bead: &str,
    snapshot: &BeadSnapshot,
) -> Result<String, String> {
    if !config.receiver_agent.trim().is_empty() {
        return Ok(config.receiver_agent.trim().to_owned());
    }
    if let Some(agent) = snapshot.assignee().filter(|agent| !agent.trim().is_empty()) {
        return Ok(agent.to_owned());
    }
    Err(format!(
        "DISPATCH_BLOCKED bead={bead} receiver agent is missing owner=josh next_action=claim-bead"
    ))
}

/// Authorizes one bead packet immediately before construction.
///
/// The refusal names the PANE as well as the bead. Measured 2026-09-01: pid
/// 70561 emitted 135 `DISPATCHED pane=%1408 bead=omp-orchestrator-815` rows in
/// one afternoon while that bead was `open`; an operator reading a refusal has
/// to know which pane to stop feeding, and the fence crate only knows tracker
/// fields, so the pane is joined here at the transport boundary.
fn authorize_bead_dispatch(
    config: &Config,
    pane: &str,
    bead: &str,
    snapshot: &BeadSnapshot,
) -> Result<String, String> {
    let receiver_agent = receiver_agent_for_dispatch(config, bead, snapshot)?;
    match authorize(&DispatchIntent::bead(bead, &receiver_agent), Some(snapshot)) {
        Ok(_) => Ok(receiver_agent),
        Err(error) => Err(format!(
            "DISPATCH_BLOCKED bead={bead} pane={pane} reason={} status={} assignee={} \
             receiver_agent={receiver_agent} owner=josh next_action=claim-bead command=\"{}\"",
            error.code(),
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
/// Prepares one bead dispatch, or refuses.
///
/// # The measured defect this shape exists to prevent
///
/// This function used to CLAIM the bead on the receiver's behalf when it
/// observed `status=open` — `br update <bead> --assignee <receiver> --status
/// in_progress` — reload the snapshot, and only then call the claim fence. The
/// fence could therefore never refuse an unclaimed bead: the dispatcher forged
/// the precondition immediately before checking for it, so `authorize` always
/// saw an `in_progress` bead owned by the receiver.
///
/// That inverts the fourth rule of AGENTS.md — file -> CLAIM -> dispatch. The
/// claim is an act by the agent that will do the work, and it is the only
/// evidence that anybody accepted the packet. A dispatcher that manufactures it
/// destroys the one signal the follow-up detector keys on (assigned +
/// in_progress + no comment since dispatch), which is why 135 re-dispatches of
/// `omp-orchestrator-815` to `%1408` produced no alert for 247 minutes on
/// 2026-09-01 (pid 70561).
///
/// So the snapshot is now read once and never written. An unclaimed bead is a
/// refusal that reaches the operator as a nonzero exit, not a claim.
async fn prepare_bead_dispatch(
    cx: &Cx,
    config: &Config,
    pane: &str,
    bead: &str,
) -> Result<(BeadSnapshot, String), String> {
    let snapshot = load_bead_snapshot(cx, config, bead).await?;
    let receiver_agent = receiver_agent_for_dispatch(config, bead, &snapshot)?;
    validate_receiver_pane(config, pane, bead, &receiver_agent)?;
    let receiver_agent = authorize_bead_dispatch(config, pane, bead, &snapshot)?;
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

async fn send_and_verify(
    cx: &Cx,
    config: &Config,
    pane: &str,
    bead: &str,
    snapshot: &BeadSnapshot,
    before: &[u8],
    tick: u64,
) -> Result<AckStageResult, String> {
    let title = snapshot.title();
    let body = snapshot.description();
    let packet = format!(
        "Objective: complete bead {bead}.\nTarget repository: {}\n\n=== {title} ===\n{body}\n",
        config.repo.display()
    );
    let staged = env::temp_dir().join(format!(
        "omp-orchestrator-dispatch-{}-{}-{}.txt",
        std::process::id(),
        pane.trim_start_matches('%'),
        bead
    ));
    fs::write(&staged, packet.as_bytes())
        .map_err(|error| format!("DISPATCH_BLOCKED bead={bead} stage packet: {error}"))?;
    let pre_observation = observe_capture(pane, &String::from_utf8_lossy(before), now_unix());
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
    let composer_rules = composer_typed::Rules::default();
    loop {
        cx.checkpoint()
            .map_err(|_| "CANCELLED while verifying receiver receipt".to_owned())?;
        let (post_send, pane_capture) = post_send_observation(cx, config, pane).await;
        let ack = read_ack_readback(cx, config, bead, pane).await?;
        let stage = assess_ack_stage(&AckStageInput {
            bead_id: bead.to_owned(),
            pane_id: pane.to_owned(),
            transport: transport.clone(),
            pre_send: pre_observation.clone(),
            post_send: post_send.clone(),
            ack,
            attempts_so_far,
        });
        if stage.is_confirmed() {
            return Ok(stage);
        }

        let recovery = match (
            &stage.delivery,
            &post_send,
            pane_capture.as_deref(),
        ) {
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
                    let resend_args = vec![
                        "send-keys".to_owned(),
                        "-t".to_owned(),
                        pane.to_owned(),
                        "-l".to_owned(),
                        packet.clone(),
                    ];
                    require_success("tmux resend send-keys -l", invoke(cx, config, "tmux", &resend_args).await?)?;
                    let enter_args = vec![
                        "send-keys".to_owned(),
                        "-t".to_owned(),
                        pane.to_owned(),
                        "Enter".to_owned(),
                    ];
                    require_success("tmux resend Enter", invoke(cx, config, "tmux", &enter_args).await?)?;
                    write_heartbeat(
                        config,
                        tick,
                        "RECEIVER_RECOVERY",
                        &format!("pane={pane} bead={bead} action=RESEND_DIRECT attempts={}", attempts_so_far + 1),
                    )?;
                    attempts_so_far += 1;
                }
                Some(NonDeliveryEscalation::SubmitParked) => {
                    let enter_args = vec![
                        "send-keys".to_owned(),
                        "-t".to_owned(),
                        pane.to_owned(),
                        "Enter".to_owned(),
                    ];
                    require_success("tmux recovery Enter", invoke(cx, config, "tmux", &enter_args).await?)?;
                    write_heartbeat(
                        config,
                        tick,
                        "RECEIVER_RECOVERY",
                        &format!("pane={pane} bead={bead} action=SUBMIT_PARKED attempts={}", attempts_so_far + 1),
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
            return Err(format!(
                "ACK_STAGE_RETRY_BLOCKED pane={pane} bead={bead} action={} verdict={} reason={} after={}s",
                stage.action.label(),
                stage.delivery.label(),
                reason,
                RECEIPT_TIMEOUT.as_secs(),
            ));
        }
        sleep(cx.now_for_observability(), RECEIPT_POLL).await;
    }
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
    Ok(())
}
fn read_pending_dispatch(config: &Config) -> Result<Option<String>, String> {
    match fs::read_to_string(&config.pending_dispatch) {
        Ok(text) => Ok(Some(if text.trim().is_empty() {
            "marker_exists_but_is_empty".to_owned()
        } else {
            text.trim().to_owned()
        })),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
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
fn docs_are_stale(config: &Config) -> Result<Option<String>, String> {
    let plan = config.repo.join("docs/PLAN.md");
    let dir = config.repo.join("docs/plan");
    let Ok(assembly) = fs::read_to_string(&plan) else {
        // No assembly at all is not a stale assembly — say which it is.
        return Ok(Some(format!("assembly absent path={}", plan.display())));
    };

    let Ok(entries) = fs::read_dir(&dir) else {
        return Err(format!("DOCS_STALE section dir unreadable path={}", dir.display()));
    };

    let mut scanned = 0usize;
    let mut missing = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else { continue };
        if !name.ends_with(".md") || !name.starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }
        let Ok(section) = fs::read_to_string(&path) else { continue };
        scanned += 1;
        // Compare a stable interior slice, not the whole file: the assembler trims
        // trailing whitespace, so an exact whole-file match would false-positive.
        let body = section.trim();
        let probe: String = body.chars().rev().take(240).collect::<Vec<_>>()
            .into_iter().rev().collect();
        if !probe.trim().is_empty() && !assembly.contains(probe.trim()) {
            missing.push(name.to_owned());
        }
    }

    // ANTI-VACUITY: zero sections scanned reports identically to a fresh assembly.
    if scanned == 0 {
        return Err(format!(
            "DOCS_STALE scanned zero sections in {} — an empty scan set cannot distinguish \
             fresh from broken",
            dir.display()
        ));
    }

    if missing.is_empty() {
        Ok(None)
    } else {
        Ok(Some(format!(
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
    if let Some(v) = std::env::var_os("CARGO_TARGET_DIR") {
        return PathBuf::from(v);
    }
    for cfg in [
        config.repo.join(".cargo/config.toml"),
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(".cargo/config.toml"),
    ] {
        let Ok(text) = std::fs::read_to_string(&cfg) else { continue };
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
            let Some(rest) = line.strip_prefix("target-dir") else { continue };
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

    let required_gib = (0.08 * total_gib).min(HEADROOM_CAP_GIB).max(ABSOLUTE_MIN_GIB);
    if free_gib < required_gib {
        return Some(format!(
            "free={free_gib:.2}GiB ({pct_free:.1}%) below floor {required_gib:.2}GiB \
             (8% of {total_gib:.0}GiB, capped at {HEADROOM_CAP_GIB:.0}GiB, min \
             {ABSOLUTE_MIN_GIB:.0}GiB); a build here fails as a LINKER error, not as disk-full"
        ));
    }
    None
}


fn write_dispatch_intent(config: &Config, pane: &str, bead: &str) -> Result<(), String> {
    if let Some(parent) = config.pending_dispatch.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "DISPATCH_BLOCKED pending marker parent={} error={error}",
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
    let mut marker = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&config.pending_dispatch)
        .map_err(|error| {
            format!(
                "DISPATCH_RETRY_BLOCKED pane={pane} bead={bead} marker={} error={error}",
                config.pending_dispatch.display()
            )
        })?;
    marker
        .write_all(&bytes)
        .and_then(|_| marker.write_all(b"\n"))
        .and_then(|_| marker.sync_data())
        .map_err(|error| {
            format!(
                "DISPATCH_BLOCKED pending marker write path={} error={error}",
                config.pending_dispatch.display()
            )
        })
}

fn clear_dispatch_intent(config: &Config) -> Result<(), String> {
    match fs::remove_file(&config.pending_dispatch) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "DISPATCH_CONFIRMED_BUT_MARKER_CLEAR_FAILED path={} error={error}",
            config.pending_dispatch.display()
        )),
    }
}
fn finished_pane_reaper_args(config: &Config) -> Vec<String> {
    vec!["--repo".to_owned(), config.repo.display().to_string()]
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
    oracle_skew: Option<i128>,
}

/// This orchestrator's own Agent Mail identity, for a signed FROM.
///
/// Read from [`Config::mail_sender`], which is resolved once at startup,
/// because the daemon REFUSES descriptive names (`INVALID_AGENT_NAME`: names
/// must be generated adjective+noun) so the supervisor cannot synthesise one.
///
/// An unset identity is a NAMED REFUSAL, never a silent skip. That is the
/// call-site form of the binding's empty-catalogue rule: an absent result must
/// not be readable as a healthy no-op, because a supervisor that quietly
/// stopped notifying looks exactly like one with nothing to report.
fn mail_sender_identity(config: &Config) -> Result<AgentName, String> {
    sender_identity_from(&config.mail_sender)
}

/// The environment variables consulted for the sender identity, in order.
const MAIL_IDENTITY_VARS: [&str; 2] = ["AGENT_MAIL_AGENT", "AGENT_NAME"];

/// Turn a configured identity into a usable one, or refuse. Pure, so the
/// refusal is testable without mutating process-global environment state.
fn sender_identity_from(configured: &str) -> Result<AgentName, String> {
    let trimmed = configured.trim();
    if trimmed.is_empty() {
        return Err(format!(
            "sender_identity_unset searched={}",
            MAIL_IDENTITY_VARS.join(",")
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
/// Agent Mail answers it. The receipt separates `persisted` (the copy exists
/// durably) from `signaled` (a message-id-bound signal receipt was appended)
/// from `acknowledged`, so a delivered-but-unnotified message is VISIBLE
/// instead of indistinguishable from a delivered one. Measured across three
/// separate messages on 2026-09-02 (40786, 40810, 40826), Agent Mail reported
/// `persisted=true signaled=false` every time, so that distinction is load
/// bearing rather than theoretical.
///
/// # Daemon primary, CLI as oracle
///
/// The write and the receipt read-back both go through the authenticated MCP
/// daemon. The CLI is consulted ONLY to cross-check the cursor reading, and a
/// daemon failure never falls back to it — a CLI fallback would silently paper
/// over an auth failure with a direct SQLite read, which is exactly the
/// fail-open that made `am agent start` report a running daemon as absent.
async fn notify_dispatch_result_durably(
    cx: &Cx,
    config: &Config,
    pane: &str,
    bead: &str,
    result: &str,
) -> Result<DurableNotice, String> {
    let sender = mail_sender_identity(config)?;
    let recipient = mail_recipient(config, pane)?;
    let project = ProjectKey::new(config.repo.display().to_string());
    let client = MailClient::discover().with_request_timeout(MAIL_REQUEST_TIMEOUT);

    let detail = one_line_detail(result);
    let body = format!(
        "Dispatch result for `{bead}`.\n\n\
         FROM: {sender}\nREPLY VIA: Agent Mail send_message to {sender}, project {project}\n\n\
         - pane: {pane}\n- bead: {bead}\n- build_id: {BUILD_ID}\n- result: {detail}\n\n\
         This is the DURABLE record of the dispatch result. The pane notification \
         is a courtesy and has been measured to report success without delivering.",
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

    // DIFFERENTIAL ORACLE, NEVER A FALLBACK: a CLI failure degrades the
    // cross-check to `None` and leaves the daemon's reading authoritative. It
    // is never substituted for the daemon's answer.
    let oracle_skew = match agent_mail_native::oracle::cli_position_now(cx, &project, &recipient)
        .await
    {
        Ok(cli) => Some(agent_mail_native::oracle::compare(&page, &cli).skew()),
        Err(_) => None,
    };

    Ok(DurableNotice {
        recipient: recipient.as_str().to_owned(),
        message_id: message_id.get(),
        persisted: delivery.persisted,
        signaled: recipient_row.is_some_and(|entry| entry.signaled),
        cursor: page.tail_cursor.get(),
        oracle_skew,
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

    let notify = notify_dispatch_result(cx, config, tick, pane, bead, result).await;
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
            let skew = notice
                .oracle_skew
                .map_or_else(|| "unavailable".to_owned(), |value| value.to_string());
            let summary = format!(
                "recipient={} message_id={} persisted={} signaled={} cursor={} oracle_skew={skew}",
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
async fn notify_dispatch_result(
    cx: &Cx,
    config: &Config,
    tick: u64,
    pane: &str,
    bead: &str,
    result: &str,
) -> Result<(), String> {
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
async fn run_cycle(cx: &Cx, config: &Config, tick: u64) -> Result<(), String> {
    write_heartbeat(config, tick, "CYCLE_STARTED", "phase=observe")?;
    if let Some(intent) = read_pending_dispatch(config)? {
        write_heartbeat(config, tick, "DISPATCH_RETRY_BLOCKED", &intent)?;
        let detail = format!(
            "DISPATCH_RETRY_BLOCKED owner=josh next_action=inspect-or-clear-pending-dispatch marker={} detail={intent}",
            config.pending_dispatch.display()
        );
        println!("{detail}");
        return Ok(());
    }

    // HD-0001 (docs/decisions.jsonl): "tick loop continues as long as we're keeping
    // our docs up to date". A buyer condition, so it gates the tick — and it is
    // checked AFTER the dispatch fence and BEFORE observation, because a stale
    // assembly makes every downstream dispatch send an agent to work from old
    // knowledge, which is the failure this condition exists to prevent.
    if let Some(why) = docs_are_stale(config)? {
        write_heartbeat(config, tick, "DOCS_STALE", &why)?;
        let detail = format!(
            "DOCS_STALE owner=josh next_action=re-assemble-docs/PLAN.md detail={why} \
             authority=HD-0001"
        );
        println!("{detail}");
        return Ok(());
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
                return Err("REAP_SWEEP_SKIP_REASON_EMPTY OMP_REAP_SWEEP=unavailable: requires a \
                            reason; an unexplained degradation is indistinguishable from a bug"
                    .to_owned());
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
    let bead_ids = parse_ready(&ready)?;
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
        println!(
            "DISK_PRESSURE owner=josh next_action=cargo-clean-or-grow-volume detail={why}"
        );
        return Ok(());
    }
    let authorization = applicable(
        read_idle_authorization(&config.repo, now_unix()),
        &config.session,
        &observation.panes,
        &observation.queue,
    );
    let decision = decide(&observation, &authorization);
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
            let dispatch_result = async {
                let (snapshot, receiver_agent) =
                    prepare_bead_dispatch(cx, config, &pane, bead).await?;
                let dispatch_epoch = now_unix() as i64;
                write_dispatch_intent(config, &pane, bead)?;
                let before = capture_pane(cx, config, &pane).await?;
                let stage = send_and_verify(cx, config, &pane, bead, &snapshot, &before, tick).await?;
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
            let report_detail = match &dispatch_result {
                Ok(outcome) => outcome.detail.clone(),
                Err(error) => format!("status=DISPATCH_FAILED detail={}", one_line_detail(error)),
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
            let outcome = dispatch_result?;
            if outcome.clear_intent {
                clear_dispatch_intent(config)?;
            }
        }
        SupervisorDecision::GateUnwired { unwired } => {
            // The remedy comes from the VARIANT, never from a literal here. The
            // supervisor printed next_action=repair-gate-trigger for an unextracted
            // crate because this string was hardcoded three hundred lines from the
            // state that produced it, and an operator following it would look for a
            // hook to fix and find nothing.
            let census = crate::census_gates(&config.repo);
            let mut parts = Vec::new();
            for name in &unwired {
                let action = census
                    .rows
                    .iter()
                    .find(|r| &r.gate == name)
                    .map(|r| (r.reachability.label(), r.reachability.next_action()))
                    .unwrap_or(("UNKNOWN", "investigate-census"));
                parts.push(format!("{name}[{}→{}]", action.0, action.1));
            }
            let detail = format!("unwired={} owner=josh", parts.join(" "));
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
            write_heartbeat(
                config,
                tick,
                "QUEUE_EMPTY_NEEDS_JOSH",
                &format!(
                    "free_capacity={free_capacity_count} next_action=authorize-or-create-work"
                ),
            )?;
            let detail = format!(
                "QUEUE_EMPTY_NEEDS_JOSH owner=josh next_action=authorize-or-create-work free_capacity={free_capacity_count}"
            );
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

fn main() -> std::process::ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let config = match Config::from_args(&args) {
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
        Config {
            repo: PathBuf::from("/tmp/omp-orchestrator-test-repo"),
            reap_finished_panes: "reap-finished-panes".to_owned(),
            omp_quick: false,
            session: "test-session".to_owned(),
            interval: Duration::from_secs(1),
            command_timeout: Duration::from_secs(1),
            max_ticks: Some(1),
            tick_monitor: "tick-monitor".to_owned(),
            run_subcommand: false,
            br: "br".to_owned(),
            ntm: "ntm".to_owned(),
            tmux_tmpdir: PathBuf::from("/tmp/omp-orchestrator-test-tmux"),
            exclude_panes: Vec::new(),
            heartbeat_ledger,
            tick_monitor_state: PathBuf::from("/tmp/omp-orchestrator-test-state"),
            pending_dispatch: PathBuf::from("/tmp/omp-orchestrator-test-pending"),
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
    fn run_reaper_for_test(config: &Config) -> Result<String, String> {
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        runtime.block_on(async {
            let cx = Cx::current().expect("runtime context");
            run_finished_pane_sweep(&cx, config).await
        })
    }

    fn executable_reaper(temp: &tempfile::TempDir, body: &str) -> PathBuf {
        let path = temp.path().join("reaper");
        std::fs::write(&path, body).expect("write reaper fixture");
        let mut permissions = std::fs::metadata(&path).expect("reaper metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("make reaper executable");
        path
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
    fn uncertain_dispatch_is_fenced_across_restarts() {
        let root = env::temp_dir().join(format!(
            "omp-orchestrator-pending-test-{}-{}",
            std::process::id(),
            now_unix()
        ));
        let pending = root.join("pending-dispatch");
        let mut config = fixture_config(root.join("heartbeat.jsonl"));
        config.pending_dispatch = pending.clone();
        write_dispatch_intent(&config, "%1413", "omp-orchestrator-test").unwrap();
        let intent = read_pending_dispatch(&config).unwrap().unwrap();
        assert!(intent.contains("omp-orchestrator-test"));
        let retry = write_dispatch_intent(&config, "%1414", "another-bead");
        assert!(retry.unwrap_err().contains("DISPATCH_RETRY_BLOCKED"));
        clear_dispatch_intent(&config).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn heartbeat_is_durable_json_with_build_identity() {
        let root = env::temp_dir().join(format!(
            "omp-orchestrator-heartbeat-test-{}-{}",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        ));
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
        std::fs::remove_dir(root).unwrap();
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
        assert!(!config.run_subcommand, "flag-only form must not require the subcommand");
        assert_eq!(config.max_ticks, Some(1));
    }

    #[test]
    fn unknown_positional_is_refused() {
        let stray = Config::from_args(&["run".to_owned(), "extra".to_owned()])
            .unwrap_err();
        assert!(stray.contains("CONFIG_REFUSED unknown argument extra"), "{stray}");
        let bare = Config::from_args(&["frobnicate".to_owned()]).unwrap_err();
        assert!(bare.contains("CONFIG_REFUSED unknown argument frobnicate"), "{bare}");
    }

    #[test]
    fn help_reports_the_run_entrypoint() {
        let help = Config::from_args(&["--help".to_owned()]).unwrap_err();
        assert_eq!(help, usage());
        assert!(help.contains("[run]"), "usage must advertise the run subcommand");
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

    #[test]
    fn finished_pane_reaper_receives_the_same_repository() {
        let config = fixture_config(PathBuf::from("/tmp/omp-orchestrator-reaper-heartbeat.jsonl"));
        assert_eq!(
            finished_pane_reaper_args(&config),
            vec!["--repo".to_owned(), config.repo.display().to_string()]
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
            format!("--repo {}", temp.path().display()),
            "the production helper must invoke the configured reaper with the repository root"
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
        let mut config = fixture_config(std::env::temp_dir().join("receiver-assignment-heartbeat.jsonl"));
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
        let mut config = fixture_config(std::env::temp_dir().join("claim-fence-known-bad.jsonl"));
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
        let mut config =
            fixture_config(std::env::temp_dir().join("claim-fence-open-assigned.jsonl"));
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
        let mut config = fixture_config(std::env::temp_dir().join("claim-fence-known-good.jsonl"));
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
        let mut config = fixture_config(std::env::temp_dir().join("claim-fence-elsewhere.jsonl"));
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

    /// The dispatch path must never write to the tracker.
    ///
    /// `prepare_bead_dispatch` used to run `br update <bead> --assignee
    /// <receiver> --status in_progress` when it saw an `open` bead, then reload
    /// the snapshot and only then call the fence — so the fence could never
    /// refuse. This is the source-level guard against that bypass returning:
    /// the dispatch preparation path holds no `--status in_progress` argument
    /// vector at all.
    #[test]
    fn dispatch_path_never_claims_on_the_receivers_behalf() {
        let source = include_str!("main.rs");
        let start = source
            .find("async fn prepare_bead_dispatch")
            .expect("prepare_bead_dispatch must exist");
        let body = &source[start..];
        let end = body
            .find("\nasync fn run_silence_watch")
            .expect("prepare_bead_dispatch must be followed by run_silence_watch");
        let body = &body[..end];
        assert!(
            !body.contains("in_progress"),
            "the dispatch path must not claim a bead on the receiver's behalf: {body}"
        );
        assert!(
            !body.contains("\"update\""),
            "the dispatch path must not write to the tracker: {body}"
        );
        assert!(
            body.contains("authorize_bead_dispatch"),
            "the dispatch path must call the claim fence: {body}"
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

        let error = validate_receiver_pane(
            &config,
            "%1408",
            "receiver-assignment-test",
            "SilverWolf",
        )
        .expect_err("a pane mapped to another agent must not receive this bead");
        assert!(error.contains("mapped_agent=AmberGate"), "{error}");
        assert!(error.contains("next_action=select-matching-pane"), "{error}");
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
        let tmp = std::env::temp_dir().join(format!("omp-tgt-{}", std::process::id()));
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

        // The env var wins, exactly as it does in cargo, so it must be absent here for
        // the config path to be the thing under test.
        let saved = std::env::var_os("CARGO_TARGET_DIR");
        // SAFETY-EQUIVALENT NOTE: single-threaded test, restored below. This crate
        // forbids unsafe; remove_var/set_var are safe in this edition.
        std::env::remove_var("CARGO_TARGET_DIR");
        let resolved = resolve_target_dir(&config);
        if let Some(v) = saved {
            std::env::set_var("CARGO_TARGET_DIR", v);
        }

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
        assert_eq!(disk_floor_verdict(total, 1536 * 1024), None, "1.5 GiB must pass");
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
        assert!(v.starts_with("unavailable:"), "the sentinel must be recognised");
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
        for v in ["", "1", "true", "yes", "skip", "unavailable", "UNAVAILABLE:x", " unavailable:x"] {
            assert!(
                !v.starts_with("unavailable:"),
                "value {v:?} must NOT be treated as a skip; fail-closed is the default"
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
        let error = sender_identity_from("").expect_err("empty must refuse");
        assert!(error.starts_with("sender_identity_unset"), "{error}");
        assert!(error.contains("AGENT_MAIL_AGENT"), "{error}");
        assert!(error.contains("AGENT_NAME"), "{error}");
        // Whitespace is not an identity.
        assert!(sender_identity_from("   ").is_err());
    }

    #[test]
    fn a_configured_sender_identity_is_trimmed_and_used() {
        let name = sender_identity_from("  BrightGorge \n").expect("must resolve");
        assert_eq!(name.as_str(), "BrightGorge");
    }

    #[test]
    fn the_recipient_falls_back_to_the_configured_receiver_when_no_pane_map_exists() {
        // fixture repo has no .flywheel/AUTONOMOUS-WAVE.md, so the pane map
        // yields nothing and the configured receiver is used.
        let config = fixture_config(PathBuf::from("/tmp/omp-orchestrator-test-heartbeat"));
        let recipient = mail_recipient(&config, "5").expect("configured receiver");
        assert_eq!(recipient.as_str(), "BlueLantern");
    }

    #[test]
    fn an_unresolvable_recipient_refuses_rather_than_guessing() {
        let mut config = fixture_config(PathBuf::from("/tmp/omp-orchestrator-test-heartbeat"));
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
        let mut config = fixture_config(PathBuf::from("/tmp/omp-orchestrator-test-heartbeat"));
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
        // The wired path runs inside `report_dispatch_result`. With no sender
        // identity configured it refuses BEFORE any I/O, and the caller must
        // still succeed and still have written the dispatch record — the
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
                report_dispatch_result(&cx, &config, 11, "5", "omp-orchestrator-test", "status=DISPATCHED")
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
            ledger.contains("sender_identity_unset"),
            "the row must say what was missing: {ledger}"
        );
        assert!(
            !ledger.contains("DISPATCH_RESULT_MAIL_PERSISTED"),
            "nothing was sent, so nothing may claim persistence: {ledger}"
        );
    }
}

#![forbid(unsafe_code)]

//! The resident OMP supervisor.
//!
//! This executable owns the observation -> queue -> dispatch -> receiver receipt
//! loop. It is intentionally not a report-only monitor: a managed session may
//! idle only with a bound Josh authorization token.

use ack_stage::{
    assess as assess_ack_stage, AckReadback, AckStageInput, AckStageResult, TransportReceipt,
};
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
use receiver_receipt::{observe_capture, PostSendObservation, ReceiptVerdict};
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
const BUILD_ID: &str = match option_env!("OMP_BUILD_ID") {
    Some(value) => value,
    None => "unversioned",
};
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
    omp_quick: bool,
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
                    command_timeout = Duration::from_secs(
                        args.get(index)
                            .ok_or_else(|| {
                                "CONFIG_REFUSED --command-timeout-secs requires seconds".to_owned()
                            })?
                            .parse::<u64>()
                            .map_err(|_| {
                                "CONFIG_REFUSED --command-timeout-secs is not an integer".to_owned()
                            })?,
                    );
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
            omp_quick,
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

async fn post_send_observation(cx: &Cx, config: &Config, pane: &str) -> PostSendObservation {
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
            return PostSendObservation::Missing;
        }
    };
    let list_bytes = match require_success("tmux list-panes", list_output) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("RECEIVER_OBSERVATION_MISSING pane={pane} phase=list error={error}");
            return PostSendObservation::Missing;
        }
    };
    let list_text = String::from_utf8_lossy(&list_bytes);
    let pane_ids: Vec<&str> = list_text
        .lines()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .collect();
    if pane_ids.is_empty() {
        return PostSendObservation::EmptyPaneList;
    }
    if !pane_ids.iter().any(|observed| *observed == pane) {
        return PostSendObservation::Absent;
    }
    let capture = match capture_pane(cx, config, pane).await {
        Ok(capture) => capture,
        Err(error) => {
            eprintln!("RECEIVER_OBSERVATION_MISSING pane={pane} phase=capture error={error}");
            return PostSendObservation::Missing;
        }
    };
    let text = String::from_utf8_lossy(&capture);
    PostSendObservation::Present(observe_capture(pane, &text, now_unix()))
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

fn authorize_bead_dispatch(
    config: &Config,
    bead: &str,
    snapshot: &BeadSnapshot,
) -> Result<String, String> {
    let receiver_agent = receiver_agent_for_dispatch(config, bead, snapshot)?;
    authorize(&DispatchIntent::bead(bead, &receiver_agent), Some(snapshot))
        .map(|_| receiver_agent)
        .map_err(|error| error.to_string())
}
async fn prepare_bead_dispatch(
    cx: &Cx,
    config: &Config,
    bead: &str,
) -> Result<(BeadSnapshot, String), String> {
    let mut snapshot = load_bead_snapshot(cx, config, bead).await?;
    let receiver_agent = receiver_agent_for_dispatch(config, bead, &snapshot)?;

    if snapshot.status_label() == "open" {
        let claim_args = vec![
            "update".to_owned(),
            bead.to_owned(),
            "--assignee".to_owned(),
            receiver_agent.clone(),
            "--status".to_owned(),
            "in_progress".to_owned(),
        ];
        require_success(
            &config.br,
            invoke(cx, config, &config.br, &claim_args).await?,
        )?;
        snapshot = load_bead_snapshot(cx, config, bead).await?;
    }

    let receiver_agent = authorize_bead_dispatch(config, bead, &snapshot)?;
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
    loop {
        cx.checkpoint()
            .map_err(|_| "CANCELLED while verifying receiver receipt".to_owned())?;
        let post_send = post_send_observation(cx, config, pane).await;
        let ack = read_ack_readback(cx, config, bead, pane).await?;
        let stage = assess_ack_stage(&AckStageInput {
            bead_id: bead.to_owned(),
            pane_id: pane.to_owned(),
            transport: transport.clone(),
            pre_send: pre_observation.clone(),
            post_send,
            ack,
            attempts_so_far: 0,
        });
        if stage.is_confirmed() {
            return Ok(stage);
        }
        // composer-typed receipt check: the pane's captured content must hold
        // typed operator text. An empty composer means the packet never
        // arrived, even if ntm reported delivery signals. This check fires
        // AFTER the ack stage so a confirmed ack still wins, and BEFORE the
        // deadline check so an empty composer is named in the final error.
        let pane_capture = capture_pane(cx, config, pane).await?;
        let pane_text = String::from_utf8_lossy(&pane_capture);
        let rules = composer_typed::Rules::default();
        if !composer_typed::is_typed(&pane_text, &rules) {
            if Instant::now() >= deadline {
                return Err(format!(
                    "COMPOSER_EMPTY pane={pane} bead={bead} — ntm reported delivery but the composer holds no typed text; \
                     the packet never arrived (sender exit is not a receipt)"
                ));
            }
            // First few polls may race the terminal renderer; retry.
            sleep(cx.now_for_observability(), RECEIPT_POLL).await;
            continue;
        }
        let awaiting_ack = matches!(
            stage.delivery,
            ReceiptVerdict::Indeterminate {
                reason: receiver_receipt::ReceiptReason::AckReadbackMissing,
                ..
            }
        );
        if !stage.action.is_retry() && !awaiting_ack {
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
        if Instant::now() >= deadline {
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

    let pct_free = (avail as f64 / total as f64) * 100.0;
    let gib_free = avail as f64 / 1024.0 / 1024.0;
    if pct_free < 8.0 || gib_free < 1.0 {
        return Ok(Some(format!(
            "volume={} free={gib_free:.2}GiB ({pct_free:.1}%) below floor 8%/1GiB; a build here \
             fails as a LINKER error, not as disk-full",
            probe.display()
        )));
    }
    Ok(None)
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
            let (snapshot, receiver_agent) = prepare_bead_dispatch(cx, config, bead).await?;
            let dispatch_epoch = now_unix() as i64;
            write_dispatch_intent(config, &pane, bead)?;
            let before = capture_pane(cx, config, &pane).await?;
            let stage = send_and_verify(cx, config, &pane, bead, &snapshot, &before, tick).await?;
            let silence = run_silence_watch(cx, config, bead, dispatch_epoch, &receiver_agent).await?;
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
                    clear_dispatch_intent(config)?;
                    println!(
                        "DISPATCHED tick={tick} session={} pane={pane} bead={bead} RECEIVER_RECEIPT={} ACK_ACTION={} SILENCE_VERDICT={silence}",
                        config.session,
                        stage.transport.kind().label(),
                        stage.action.label()
                    );
                }
                other => {
                    println!(
                        "DISPATCH_SILENCE tick={tick} session={} pane={pane} bead={bead} verdict={other} next_action=inspect-or-resolve-pending",
                        config.session
                    );
                }
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

    fn fixture_config(heartbeat_ledger: PathBuf) -> Config {
        Config {
            repo: PathBuf::from("/tmp/omp-orchestrator-test-repo"),
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
            omp_quick: false,
            omp_binary: PathBuf::from("omp"),
        }
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

        let receiver_agent = authorize_bead_dispatch(
            &config,
            "receiver-assignment-test",
            &snapshot,
        )
        .expect("an assigned bead should supply the receiver agent when config is unset");
        assert_eq!(receiver_agent, "SilverWolf");
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

}

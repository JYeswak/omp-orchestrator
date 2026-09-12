#![forbid(unsafe_code)]

//! Thin consumer for the committed control-plane `rch-reclaim-owner/v1` contract.
//!
//! The consumer owns no worker roster, admission policy, candidate selection,
//! deletion, exit aggregation, thresholds, paths, or deadlines beyond one opaque
//! outer safety bound. It submits one fixed-argv owner request, forwards the
//! owner's stdout byte-for-byte, and uses the owner's durable request state for
//! cancellation and stdout-loss recovery. Every refusal below is restrictive:
//! an ambiguous owner answer triggers durable STATUS before any machinery verdict.
//!
//! Scope note on stdin: the four verbs this consumer invokes
//! (`report|apply|status|cancel`) take validated scalar argv and no request
//! body. The owner's worker RPCs are a different surface that does read stdin;
//! this crate never invokes them, so nothing here offers stdin at all.

/// Sweep machinery (bead 4bem8): the shell-conformance port. Unwired by the
/// owner pivot in 026135c and rewired here -- an uncompiled module is not a
/// port, however green the suite reads without it.
pub mod model;
pub mod probe;

use asupersync::io::AsyncReadExt;
use asupersync::process::{Child, Command, ProcessError, ProcessGroupMode, ProcessSignalTarget, Stdio};
use asupersync::runtime::{JoinError, TaskHandle};
use asupersync::Cx;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

/// The control-plane owner protocol consumed by this crate.
pub const OWNER_SCHEMA: &str = "control-plane.rch-reclaim-owner/v1";
/// The only executable this crate starts, resolved through PATH.
pub const OWNER_BINARY: &str = "rch-reclaim-owner";
/// Restrictive local exit code for owner machinery failures.
pub const OWNER_MACHINERY_EXIT: u8 = 78;

/// Exact owner-published control command bound (owner protocol
/// `CONTROL_COMMAND_BOUND_SECS`). Bounds one status/cancel attempt, one
/// status poll quantum, and one reader join. It is a mirrored owner value,
/// not consumer policy.
pub const CONTROL_COMMAND_BOUND_SECS: u64 = 15;
/// Duration form of the mirrored owner control bound.
pub const CONTROL_COMMAND_BOUND: Duration = Duration::from_secs(CONTROL_COMMAND_BOUND_SECS);

/// Opaque outer safety bound, pinned to owner wire v1. It exceeds the
/// owner-published worst case (15 + 4*390 + 15 = 1590s) with margin for one
/// durable cancel plus status recovery. This is the ONLY bound the consumer
/// owns: no worker count, no per-worker formula, no roster, no thresholds.
/// Revisit if the owner version changes.
pub const CONSUMER_OUTER_BOUND_SECS: u64 = 1800;
/// Duration form of the opaque outer safety bound.
pub const CONSUMER_OUTER_BOUND: Duration = Duration::from_secs(CONSUMER_OUTER_BOUND_SECS);

/// Owner combined-output ceiling mirror: the owner caps a full response at
/// 4MiB per worker over 4 workers plus 1MiB, which is 17MiB. Collection
/// enforces it per stream during the read (see `drain_capped`), and the
/// combined total is refused above it. Anything larger cannot be a valid
/// owner response on any verb.
pub const MAX_OWNER_OUTPUT_BYTES: usize = 17 * 1024 * 1024;

/// RUN deadline: the owner-published worst case (15 + 4*390 + 15 = 1590s).
/// The initial owner wait races only to here; recovery (durable cancel,
/// drain, status) runs under the outer deadline reserved alongside it. Both
/// are computed upfront so a long RUN cannot starve recovery to zero.
pub const RUN_DEADLINE_SECS: u64 = 1590;
/// Duration form of the RUN deadline.
pub const RUN_DEADLINE: Duration = Duration::from_secs(RUN_DEADLINE_SECS);
const STREAM_CAP_PLUS_ONE: u64 = MAX_OWNER_OUTPUT_BYTES as u64 + 1;

/// Completion-poll quantum for the owned RUN wait. Only pacing, never policy:
/// each quantum re-checks caller cancellation and the outer deadline.
const RUN_POLL_QUANTUM: Duration = Duration::from_millis(50);

/// Bounded reap-after-kill polls (2ms each). SIGKILL delivery is prompt; the
/// cap exists so a kernel quirk cannot stall the path. The `kill_on_drop`
/// drop remains as backstop, never as plan.
const REAP_POLLS: u32 = 500;
const REAP_POLL_INTERVAL: Duration = Duration::from_millis(2);

/// The only modes accepted by the owner protocol.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OwnerMode {
    Report,
    Apply,
}

impl OwnerMode {
    /// The fixed local owner subcommand for this mode.
    #[must_use]
    pub const fn verb(self) -> &'static str {
        match self {
            Self::Report => "report",
            Self::Apply => "apply",
        }
    }
}

/// Exact owner request-id grammar (owner protocol `validate_request_id`):
/// length 1..=96 over `[A-Za-z0-9._:-]`. Checked before any spawn.
pub fn validate_request_id(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 96 {
        return Err(format!(
            "request_id length must be 1..=96; observed={}",
            value.len()
        ));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err("request_id contains characters outside [A-Za-z0-9._:-]".to_owned());
    }
    Ok(())
}

/// The result of one local owner process boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerProcessOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: Option<u8>,
}

/// The response bytes, owner exit, and recovery provenance forwarded to the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerForwardedResponse {
    pub raw_stdout: Vec<u8>,
    pub exit_code: u8,
    pub request_id: String,
    pub recovered_from_status: bool,
}

/// The typed reason for a restrictive local owner-machinery refusal. Only
/// pre-spawn failures and unrecoverable recovery states refuse directly; every
/// post-spawn ambiguity attempts durable STATUS first and lands, if at all, on
/// `RecoveryNeeded` with the composed detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerMachineryReason {
    Cancelled,
    Spawn,
    OwnerWait,
    RequestIdInvalid,
    RecoveryNeeded,
}

impl OwnerMachineryReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cancelled => "CANCELLED",
            Self::Spawn => "SPAWN",
            Self::OwnerWait => "OWNER_WAIT",
            Self::RequestIdInvalid => "REQUEST_ID_INVALID",
            Self::RecoveryNeeded => "RECOVERY_NEEDED",
        }
    }
}

/// A single restrictive local consumer refusal. The owner remains the authority
/// for owner outcomes and exit semantics; this type covers only inability to
/// invoke, correlate, or recover its state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumerError {
    reason: OwnerMachineryReason,
    detail: String,
    request_id: Option<String>,
}

impl ConsumerError {
    #[must_use]
    pub fn owner_machinery(reason: OwnerMachineryReason, detail: impl Into<String>) -> Self {
        Self {
            reason,
            detail: detail.into(),
            request_id: None,
        }
    }

    #[must_use]
    pub fn recovery_needed(request_id: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            reason: OwnerMachineryReason::RecoveryNeeded,
            detail: detail.into(),
            request_id: Some(request_id.into()),
        }
    }

    #[must_use]
    pub const fn reason(&self) -> OwnerMachineryReason {
        self.reason
    }

    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        OWNER_MACHINERY_EXIT
    }

    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }

    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }
}

impl fmt::Display for ConsumerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let request_id = self.request_id.as_deref().unwrap_or("-");
        write!(
            formatter,
            "CONTABO_RECLAIM_OWNER_MACHINERY_REFUSED class=OWNER_MACHINERY reason={} request_id={} detail={}",
            self.reason.as_str(),
            request_id,
            self.detail
        )
    }
}

impl std::error::Error for ConsumerError {}

/// A command-line parse failure, kept separate from an owner machinery refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    detail: String,
    help: bool,
}

impl CliError {
    fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
            help: false,
        }
    }

    fn help() -> Self {
        Self {
            detail: usage().to_owned(),
            help: true,
        }
    }

    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }

    #[must_use]
    pub const fn is_help(&self) -> bool {
        self.help
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl std::error::Error for CliError {}

/// Parsed arguments for the thin owner consumer CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliArgs {
    pub request_id: String,
    pub mode: OwnerMode,
    pub json: bool,
}

/// Return whether the caller requested the structured local error envelope.
#[must_use]
pub fn request_json(arguments: &[String]) -> bool {
    arguments.iter().any(|argument| argument == "--json")
}

/// Parse only the owner consumer's request id, mode, and local JSON error switch.
///
/// The request id is validated against the exact owner grammar here AND again
/// before any spawn: a second `--request-id` flag refuses rather than
/// last-wins, and empty/overlong/illegal ids never reach the owner.
pub fn parse_args(arguments: &[String]) -> Result<CliArgs, CliError> {
    let mut request_id: Option<String> = None;
    let mut mode = None;
    let mut json = false;
    let mut index = 0;

    while index < arguments.len() {
        match arguments[index].as_str() {
            "--json" => json = true,
            "--request-id" => {
                index += 1;
                let value = arguments.get(index).ok_or_else(|| {
                    CliError::new("REQUEST_ID_REQUIRED: --request-id needs a value")
                })?;
                if request_id.is_some() {
                    return Err(CliError::new(
                        "REQUEST_ID_MULTIPLE: --request-id given twice; pass exactly one",
                    ));
                }
                request_id = Some(value.clone());
            }
            value if value.starts_with("--request-id=") => {
                if request_id.is_some() {
                    return Err(CliError::new(
                        "REQUEST_ID_MULTIPLE: --request-id given twice; pass exactly one",
                    ));
                }
                request_id = Some(value[13..].to_owned());
            }
            "report" => set_mode(&mut mode, OwnerMode::Report)?,
            "apply" => set_mode(&mut mode, OwnerMode::Apply)?,
            "--help" | "-h" => return Err(CliError::help()),
            other => {
                return Err(CliError::new(format!(
                    "UNKNOWN_ARGUMENT: {other}; {}",
                    usage()
                )));
            }
        }
        index += 1;
    }

    let request_id = request_id.ok_or_else(|| CliError::new("REQUEST_ID_REQUIRED"))?;
    if let Err(detail) = validate_request_id(&request_id) {
        return Err(CliError::new(format!("REQUEST_ID_INVALID: {detail}")));
    }
    let mode = mode.ok_or_else(|| CliError::new("MODE_REQUIRED: choose report or apply"))?;
    Ok(CliArgs {
        request_id,
        mode,
        json,
    })
}

fn set_mode(mode: &mut Option<OwnerMode>, value: OwnerMode) -> Result<(), CliError> {
    if mode.replace(value).is_some() {
        return Err(CliError::new(
            "MODE_MULTIPLE: choose exactly one of report or apply",
        ));
    }
    Ok(())
}

/// The stable usage string for local parse failures and help output.
#[must_use]
pub const fn usage() -> &'static str {
    "usage: contabo-reclaim [--json] --request-id ID (report|apply)"
}

/// Render the historical structured parse-error envelope without invoking the owner.
#[must_use]
pub fn render_cli_error(error: &CliError, json: bool) -> String {
    if json {
        serde_json::json!({
            "schema": "contabo-reclaim/report-v1",
            "outcome": "ERROR",
            "error": error.to_string(),
        })
        .to_string()
    } else {
        error.to_string()
    }
}

/// Invoke the owner once with durable cancellation and status recovery.
///
/// Fixed argv only (`<report|apply> <request-id>`); the invoked verbs take no
/// request body. Raw valid owner stdout is forwarded byte-for-byte with the
/// owner's own exit code.
///
/// Two contexts: `cx` owns the RUN wait (caller cancellation aborts it);
/// `recovery_cx` must be independent of caller cancellation (a fresh request
/// context, never a clone) so CANCEL/STATUS/drain still proceed after the
/// caller is gone.
pub async fn consume(
    cx: &Cx,
    recovery_cx: &Cx,
    request_id: &str,
    mode: OwnerMode,
) -> Result<OwnerForwardedResponse, ConsumerError> {
    let owned_id = request_id.to_owned();
    // Same derivation as consume_with below (microseconds apart, immaterial
    // for bounds): the RUN adapter waits to the RUN deadline, recovery verbs
    // to the outer deadline.
    let started = Instant::now();
    let run_deadline = started + RUN_DEADLINE;
    let outer_deadline = started + CONSUMER_OUTER_BOUND;
    // The adapter invokes on the Cx it is handed: the caller Cx for RUN,
    // the owned task Cx for cancel/status (via settle factories), so abort
    // cancels exactly what the child and readers observe.
    consume_with(cx, recovery_cx, &owned_id, mode, move |task_cx, argv| {
        if matches!(argv.first().map(String::as_str), Some("cancel" | "status")) {
            invoke_owner(task_cx, argv, outer_deadline)
        } else {
            invoke_owner(task_cx, argv, run_deadline)
        }
    })
    .await
}

/// Injected owner process boundary used by tests and by the production adapter.
///
/// The invoker receives the Cx to invoke on plus exact argv and nothing else.
/// RUN is invoked on the caller Cx; cancel/status are invoked on the owned
/// settle-task Cx via per-attempt factories, so abort cancels exactly what
/// the child and readers observe. The adapter must be Clone (one owned
/// factory per attempt) and Send + 'static (factories cross into tasks).
/// Interrupted owner work is recovered only through durable cancel/status
/// calls; this consumer never signals an OS process.
pub async fn consume_with<F, Fut>(
    cx: &Cx,
    recovery_cx: &Cx,
    request_id: &str,
    mode: OwnerMode,
    invoke: F,
) -> Result<OwnerForwardedResponse, ConsumerError>
where
    F: FnMut(Cx, Vec<String>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Result<OwnerProcessOutput, ConsumerError>> + Send + 'static,
{
    if let Err(detail) = validate_request_id(request_id) {
        return Err(ConsumerError::owner_machinery(
            OwnerMachineryReason::RequestIdInvalid,
            detail,
        ));
    }
    checkpoint(cx)?;
    // Both budgets reserved BEFORE the RUN starts: the initial wait races
    // only to the RUN deadline, so recovery (cancel, drain, status) always
    // retains outer-budget time no matter how long the RUN ran.
    let started = Instant::now();
    let run_deadline = started + RUN_DEADLINE;
    let outer_deadline = started + CONSUMER_OUTER_BOUND;
    consume_with_deadline(cx, recovery_cx, request_id, mode, invoke, run_deadline, outer_deadline).await
}

async fn consume_with_deadline<F, Fut>(
    cx: &Cx,
    recovery_cx: &Cx,
    request_id: &str,
    mode: OwnerMode,
    mut invoke: F,
    run_deadline: Instant,
    outer_deadline: Instant,
) -> Result<OwnerForwardedResponse, ConsumerError>
where
    F: FnMut(Cx, Vec<String>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Result<OwnerProcessOutput, ConsumerError>> + Send + 'static,
{
    let mut owner = Box::pin(invoke(
        cx.clone(),
        vec![mode.verb().to_owned(), request_id.to_owned()],
    ));
    match await_owned(cx, &mut owner, run_deadline).await {
        RunSettled::Completed(Ok(output)) => {
            match validate_owner_bytes(request_id, mode, &output.stdout, &output.stderr, output.exit_code) {
                Ok(valid) => Ok(OwnerForwardedResponse {
                    raw_stdout: output.stdout,
                    exit_code: valid.exit_code,
                    request_id: request_id.to_owned(),
                    recovered_from_status: false,
                }),
                Err(ambiguous) => {
                    // The RUN spawned, so this ambiguity belongs to durable
                    // STATUS before any machinery verdict. No cancel: the run
                    // completed; only its bytes are suspect.
                    status_after_run(recovery_cx, request_id, mode, &mut invoke, outer_deadline, ambiguous).await
                }
            }
        }
        RunSettled::Completed(Err(error))
            if error.reason() == OwnerMachineryReason::Spawn =>
        {
            // Pre-spawn: nothing exists to recover. Immediate.
            Err(error)
        }
        RunSettled::Completed(Err(error)) => {
            // The RUN spawned but its wait failed; durable CANCEL then STATUS
            // characterize it. No drain is owed: the wait already resolved.
            let notes = vec![format!("owner wait: {}", error.detail())];
            recover_after_interrupt(recovery_cx, request_id, mode, &mut invoke, outer_deadline, notes, None).await
        }
        RunSettled::Interrupted => {
            // Caller cancel or outer expiry with the RUN still owned.
            recover_after_interrupt(recovery_cx, request_id, mode, &mut invoke, outer_deadline, Vec::new(), Some(&mut owner)).await
        }
    }
}

/// How one owned RUN wait resolved: completed with its value, or still owned
/// at caller-cancel/outer-deadline time.
enum RunSettled<T> {
    Completed(T),
    Interrupted,
}

/// Await one owned future to completion, watcher cancellation, or an absolute
/// deadline, in bounded quanta. Ownership never moves: on interrupt the caller
/// still holds the future and may drain it. The timeout wrapper only stops
/// polling per quantum; it never cancels the inner future.
async fn await_owned<Fut>(
    watcher: &Cx,
    owner: &mut Pin<Box<Fut>>,
    deadline: Instant,
) -> RunSettled<Fut::Output>
where
    Fut: Future + Send,
{
    loop {
        if watcher.is_cancel_requested() || Instant::now() >= deadline {
            return RunSettled::Interrupted;
        }
        match asupersync::time::timeout(
            asupersync::time::wall_now(),
            RUN_POLL_QUANTUM,
            &mut *owner,
        )
        .await
        {
            Ok(output) => return RunSettled::Completed(output),
            Err(_) => continue,
        }
    }
}

/// Await one owned control invoke to completion or deadline, then drain
/// briefly instead of dropping mid-flight: a concurrently completing answer
/// still counts. The grace runs against the OUTER recovery deadline, never
/// the already-expired attempt deadline (which would grant zero).
///
/// The invocation is built INSIDE the owned region task from a factory that
/// receives the task's Cx, never prebuilt on an independent Cx: production
/// passes `|task_cx| invoke_owner(task_cx, argv, deadline)`, so TaskHandle
/// abort cancels the same Cx the child and pipe readers observe. A prebuilt
/// future would observe the spawner's Cx while the abort lands on the task's.
///
/// On expiry the task is aborted EXPLICITLY and bounded-rejoined before
/// returning. The rejoin result propagates: a terminal task (joined value or
/// landed abort) reports `aborted and joined`; a rejoin that itself expires
/// reports nonterminal explicitly and is never discarded.
async fn settle_control<F, Fut>(
    spawner: &Cx,
    watcher: &Cx,
    make: F,
    attempt_deadline: Instant,
    outer_deadline: Instant,
    label: &'static str,
    rejoin_bound: Duration,
) -> Result<Fut::Output, String>
where
    F: FnOnce(Cx) -> Fut + Send + 'static,
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    let mut task = spawner
        .spawn(|task_cx| async move { make(task_cx).await })
        .map_err(|error| format!("{label} task spawn failed: {error:?}"))?;
    match poll_task_to_deadline(watcher, &mut task, attempt_deadline).await {
        TaskPoll::Completed(output) => Ok(output),
        TaskPoll::TaskFailed(detail) => Err(detail),
        TaskPoll::Expired => {
            let grace = std::cmp::min(outer_deadline, Instant::now() + CONTROL_COMMAND_BOUND);
            match poll_task_to_deadline(watcher, &mut task, grace).await {
                TaskPoll::Completed(output) => Ok(output),
                TaskPoll::TaskFailed(detail) => Err(detail),
                TaskPoll::Expired => {
                    task.abort();
                    match abort_rejoin_task(watcher, &mut task, rejoin_bound).await {
                        TaskTerminal::Joined(_) | TaskTerminal::Aborted => {
                            Err(format!("{label} exceeded bound; task aborted and joined"))
                        }
                        TaskTerminal::Failed(detail) => {
                            Err(format!("{label} exceeded bound; {detail}"))
                        }
                    }
                }
            }
        }
    }
}

/// How one owned task poll resolved: completed with its value, failed at
/// task level, or still running at the deadline.
enum TaskPoll<T> {
    Completed(T),
    TaskFailed(String),
    Expired,
}

/// Poll one owned task to completion, task failure, or deadline without ever
/// dropping it mid-flight. The caller owns expiry (grace, abort, rejoin).
async fn poll_task_to_deadline<T>(
    watcher: &Cx,
    task: &mut TaskHandle<T>,
    deadline: Instant,
) -> TaskPoll<T>
where
    T: Send + 'static,
{
    loop {
        if watcher.is_cancel_requested() {
            return TaskPoll::Expired;
        }
        if Instant::now() >= deadline {
            return TaskPoll::Expired;
        }
        match task.try_join() {
            Ok(Some(output)) => return TaskPoll::Completed(output),
            Ok(None) => {}
            Err(error) => {
                return TaskPoll::TaskFailed(format!(
                    "control task failed: {}",
                    join_error_name(&error)
                ))
            }
        }
        asupersync::time::sleep(
            asupersync::time::wall_now(),
            RUN_POLL_QUANTUM,
        )
        .await;
    }
}

/// What an abort plus bounded rejoin observed: a joined value, a landed
/// abort, or a failure. A rejoin that itself expires is Failed with
/// nonterminal wording; a panicked task is Failed with its own wording
/// (terminal but anomalous, never called nonterminal and never joined).
enum TaskTerminal<T> {
    Joined(T),
    Aborted,
    Failed(String),
}

/// Abort a task and re-join it under an explicit bound to own its terminal
/// state. Never an unbounded join: after abort the task is terminal, so the
/// bound guards scheduler delay only. The rejoin resolving proves the task's
/// owned future — the child handle and the pipe-reader handles — was dropped
/// inside the terminal boundary.
async fn abort_rejoin_task<T>(
    watcher: &Cx,
    handle: &mut TaskHandle<T>,
    rejoin_bound: Duration,
) -> TaskTerminal<T>
where
    T: Send + 'static,
{
    handle.abort();
    let rejoined =
        asupersync::time::timeout(asupersync::time::wall_now(), rejoin_bound, handle.join(watcher))
            .await;
    match rejoined {
        Ok(Ok(value)) => TaskTerminal::Joined(value),
        Ok(Err(JoinError::Cancelled(_))) => TaskTerminal::Aborted,
        Ok(Err(error)) => TaskTerminal::Failed(format!(
            "aborted task resolved {}",
            join_error_name(&error)
        )),
        Err(_) => TaskTerminal::Failed(format!(
            "task nonterminal after abort; rejoin exceeded bound={}s",
            rejoin_bound.as_secs()
        )),
    }
}

fn join_error_name(error: &JoinError) -> &'static str {
    match error {
        JoinError::Cancelled(_) => "cancelled",
        JoinError::Panicked(_) => "panicked",
        JoinError::PolledAfterCompletion => "polled-after-completion",
    }
}

fn remaining(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

/// Post-interrupt recovery shared by caller-cancel, outer expiry, and failed
/// owner waits: request durable CANCEL (strict, non-blocking), drain a still
/// owned RUN when one is handed over, then let durable STATUS characterize
/// whatever remains. Every failure below stays restrictive. All waits here
/// run under the recovery context, never the cancelled caller one.
async fn recover_after_interrupt<F, Fut>(
    recovery_cx: &Cx,
    request_id: &str,
    mode: OwnerMode,
    invoke: &mut F,
    deadline: Instant,
    mut notes: Vec<String>,
    owned: Option<&mut Pin<Box<Fut>>>,
) -> Result<OwnerForwardedResponse, ConsumerError>
where
    F: FnMut(Cx, Vec<String>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Result<OwnerProcessOutput, ConsumerError>> + Send + 'static,
{
    if let Some(note) = request_durable_cancel(recovery_cx, request_id, invoke, deadline).await {
        notes.push(note);
    }
    if let Some(owner) = owned {
        // Drain RUN: a concurrently completing owner still delivers its
        // terminal response instead of being discarded into STATUS. Bounded
        // by one control quantum over the remaining outer budget.
        let drain_deadline = std::cmp::min(deadline, Instant::now() + CONTROL_COMMAND_BOUND);
        match await_owned(recovery_cx, owner, drain_deadline).await {
            RunSettled::Completed(Ok(output)) => {
                match validate_owner_bytes(
                    request_id,
                    mode,
                    &output.stdout,
                    &output.stderr,
                    output.exit_code,
                ) {
                    Ok(valid) => {
                        return Ok(OwnerForwardedResponse {
                            raw_stdout: output.stdout,
                            exit_code: valid.exit_code,
                            request_id: request_id.to_owned(),
                            recovered_from_status: false,
                        });
                    }
                    Err(ambiguous) => notes.push(format!("drained run ambiguous: {ambiguous}")),
                }
            }
            RunSettled::Completed(Err(error)) => {
                notes.push(format!("drained run failed: {}", error.detail()));
            }
            RunSettled::Interrupted => {}
        }
    }
    status_recovery(recovery_cx, request_id, mode, invoke, deadline, notes).await
}

/// Direct-path ambiguity after a completed RUN: no cancel is owed (the run
/// finished; only its bytes are suspect), so STATUS alone characterizes it.
async fn status_after_run<F, Fut>(
    recovery_cx: &Cx,
    request_id: &str,
    mode: OwnerMode,
    invoke: &mut F,
    deadline: Instant,
    ambiguous: String,
) -> Result<OwnerForwardedResponse, ConsumerError>
where
    F: FnMut(Cx, Vec<String>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Result<OwnerProcessOutput, ConsumerError>> + Send + 'static,
{
    status_recovery(
        recovery_cx,
        request_id,
        mode,
        &mut *invoke,
        deadline,
        vec![format!("direct owner output ambiguous: {ambiguous}")],
    )
    .await
}

async fn status_recovery<F, Fut>(
    recovery_cx: &Cx,
    request_id: &str,
    mode: OwnerMode,
    invoke: &mut F,
    deadline: Instant,
    mut notes: Vec<String>,
) -> Result<OwnerForwardedResponse, ConsumerError>
where
    F: FnMut(Cx, Vec<String>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Result<OwnerProcessOutput, ConsumerError>> + Send + 'static,
{
    let status = match poll_status(recovery_cx, request_id, mode, &mut *invoke, deadline).await {
        Ok(status) => status,
        Err(detail) => {
            notes.push(format!("status recovery: {detail}"));
            return Err(ConsumerError::recovery_needed(request_id, notes.join("; ")));
        }
    };
    match forwarded_from_status(request_id, mode, &status) {
        Ok(response) => Ok(response),
        Err(detail) => {
            notes.push(format!("status forward: {detail}"));
            Err(ConsumerError::recovery_needed(request_id, notes.join("; ")))
        }
    }
}

/// One durable CANCEL attempt: strict positive response required, but never
/// blocking. Malformed, mismatched, negative, absent, or nonzero answers stay
/// restrictive as a note while STATUS characterizes the state.
async fn request_durable_cancel<F, Fut>(
    recovery_cx: &Cx,
    request_id: &str,
    invoke: &mut F,
    deadline: Instant,
) -> Option<String>
where
    F: FnMut(Cx, Vec<String>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Result<OwnerProcessOutput, ConsumerError>> + Send + 'static,
{
    let attempt_deadline = std::cmp::min(deadline, Instant::now() + CONTROL_COMMAND_BOUND);
    // One owned factory per attempt: the cancel invocation is built on the
    // settle task's Cx, so abort cancels exactly what the child observes.
    let argv = vec!["cancel".to_owned(), request_id.to_owned()];
    let output = match settle_control(
        recovery_cx,
        recovery_cx,
        {
            let mut invoke = invoke.clone();
            move |task_cx: Cx| invoke(task_cx, argv)
        },
        attempt_deadline,
        deadline,
        "durable cancel",
        CONTROL_COMMAND_BOUND,
    )
    .await
    {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            return Some(format!("durable cancel adapter: {}", error.detail()));
        }
        Err(detail) => {
            return Some(detail);
        }
    };
    parse_cancel_response(request_id, &output).err()
}

async fn poll_status<F, Fut>(
    recovery_cx: &Cx,
    request_id: &str,
    mode: OwnerMode,
    invoke: &mut F,
    deadline: Instant,
) -> Result<DurableStatusResponse, String>
where
    F: FnMut(Cx, Vec<String>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Result<OwnerProcessOutput, ConsumerError>> + Send + 'static,
{
    loop {
        if recovery_cx.is_cancel_requested() {
            return Err("status recovery observed recovery-context cancellation".to_owned());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "status recovery exceeded consumer outer bound={}s",
                CONSUMER_OUTER_BOUND_SECS
            ));
        }
        let attempt_deadline = std::cmp::min(deadline, Instant::now() + CONTROL_COMMAND_BOUND);
        // One owned factory per attempt, built on the settle task's Cx.
        let argv = vec!["status".to_owned(), request_id.to_owned()];
        let output = match settle_control(
            recovery_cx,
            recovery_cx,
            {
                let mut invoke = invoke.clone();
                move |task_cx: Cx| invoke(task_cx, argv)
            },
            attempt_deadline,
            deadline,
            "status",
            CONTROL_COMMAND_BOUND,
        )
        .await
        {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => {
                return Err(format!("status adapter: {}", error.detail()));
            }
            Err(detail) => {
                return Err(detail);
            }
        };
        match parse_durable_status(request_id, mode, &output) {
            Ok(status) => {
                if status.terminal.is_some()
                    && status.state.workers.iter().all(|worker| !worker.lease_active)
                {
                    return Ok(status);
                }
                asupersync::time::sleep(
                    asupersync::time::wall_now(),
                    std::cmp::min(CONTROL_COMMAND_BOUND, remaining(deadline)),
                )
                .await;
            }
            Err(detail) => return Err(detail),
        }
    }
}

/// Recover a durable terminal response after local stdout loss.
pub async fn recover_terminal_response(
    recovery_cx: &Cx,
    request_id: &str,
    mode: OwnerMode,
) -> Result<OwnerForwardedResponse, ConsumerError> {
    let owned_id = request_id.to_owned();
    let outer_deadline = Instant::now() + CONSUMER_OUTER_BOUND;
    // The adapter invokes on the Cx it is handed — the settle task's Cx per
    // status attempt — so abort cancels exactly what the child observes.
    recover_terminal_response_with(recovery_cx, &owned_id, mode, move |task_cx, argv| {
        invoke_owner(task_cx, argv, outer_deadline)
    })
    .await
}

/// Injected status-only recovery boundary for stdout-loss tests.
pub async fn recover_terminal_response_with<F, Fut>(
    recovery_cx: &Cx,
    request_id: &str,
    mode: OwnerMode,
    mut invoke: F,
) -> Result<OwnerForwardedResponse, ConsumerError>
where
    F: FnMut(Cx, Vec<String>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Result<OwnerProcessOutput, ConsumerError>> + Send + 'static,
{
    if let Err(detail) = validate_request_id(request_id) {
        return Err(ConsumerError::owner_machinery(
            OwnerMachineryReason::RequestIdInvalid,
            detail,
        ));
    }
    checkpoint(recovery_cx)?;
    let deadline = Instant::now() + CONSUMER_OUTER_BOUND;
    status_recovery(recovery_cx, request_id, mode, &mut invoke, deadline, Vec::new()).await
}

/// Combined owner output ceiling: stdout plus stderr must fit the 17MiB owner
/// response ceiling together. Oversize output cannot be a valid owner answer
/// on any verb; truncating it would corrupt correlation instead.
fn check_combined_size(stdout: &[u8], stderr: &[u8]) -> Result<(), String> {
    let total = stdout.len().saturating_add(stderr.len());
    if total > MAX_OWNER_OUTPUT_BYTES {
        return Err(format!(
            "owner combined output {total} bytes exceeds {MAX_OWNER_OUTPUT_BYTES} byte ceiling"
        ));
    }
    Ok(())
}

struct ValidatedOwnerResponse {
    exit_code: u8,
}

/// Validate one owner JSON answer: non-empty bytes, ceiling, exact
/// schema/request/mode correlation, and the exact owner outcome-to-exit
/// matrix. The outcome enum is closed by type: an unknown outcome string
/// fails the parse below rather than flowing anywhere.
fn validate_owner_bytes(
    request_id: &str,
    mode: OwnerMode,
    stdout: &[u8],
    stderr: &[u8],
    process_exit: Option<u8>,
) -> Result<ValidatedOwnerResponse, String> {
    check_combined_size(stdout, stderr).map_err(|detail| format!("oversize: {detail}"))?;
    let process_exit = process_exit.ok_or_else(|| "owner ended without a process exit code".to_owned())?;
    if stdout.iter().all(u8::is_ascii_whitespace) {
        return Err(format!(
            "owner stdout was empty; stderr={}",
            stderr_detail(stderr)
        ));
    }
    let response: OwnerResponse = serde_json::from_slice(stdout)
        .map_err(|error| format!("owner response JSON: {error}"))?;
    validate_owner_response(request_id, mode, &response)?;
    if response.exit_code != process_exit {
        return Err(format!(
            "owner process exit={} differs from response exit_code={}",
            process_exit, response.exit_code
        ));
    }
    Ok(ValidatedOwnerResponse {
        exit_code: response.exit_code,
    })
}

/// Exact owner outcome-to-exit matrix (owner `outcome.rs`: exits derive from
/// the outcome, so the pair must agree — independent closed sets are not
/// enough). `UnknownPartialEffects` additionally requires backing evidence:
/// a nested remote row carrying that outcome with nonempty effects.

fn validate_outcome_exit(
    outcome: &OwnerOutcome,
    exit_code: u8,
    workers: &[WorkerRunRecord],
) -> Result<(), String> {
    let agrees = match outcome {
        OwnerOutcome::Succeeded => exit_code == 0,
        OwnerOutcome::Partial => exit_code == 1,
        OwnerOutcome::BoundExceeded => exit_code == 124,
        OwnerOutcome::Cancelled => exit_code == 78,
        OwnerOutcome::Error => matches!(exit_code, 77 | 78 | 124),
        OwnerOutcome::UnknownPartialEffects => {
            matches!(exit_code, 1 | 77 | 78 | 124)
                && workers.iter().any(remote_backs_partial_effects)
        }
    };
    if agrees {
        Ok(())
    } else {
        Err(format!(
            "owner outcome {outcome:?} disagrees with exit {exit_code}"
        ))
    }
}

fn remote_backs_partial_effects(worker: &WorkerRunRecord) -> bool {
    worker.remote.as_ref().is_some_and(|remote| {
        remote.outcome == WorkerOutcome::UnknownPartialEffects && !remote.effects.is_empty()
    })
}

/// Exact top-level error coupling (owner `outcome.rs` + `operation_lock.rs`
/// release fold): the top error must be the kind that produces the paired
/// exit. The release fold (`apply_release_failure`, the ONLY writer of the
/// `OWNER_LOCK_RELEASE_FAILED` marker) preserves the prior error and appends
/// the marker while forcing exit 78 (or keeping prior 124) and outcome Error
/// (unless UPE). Hence preserved prior kinds are admissible at 78 ONLY with
/// the marker, and exit 124 with outcome Error admits ONLY the
/// BoundExceeded-plus-marker shape. A 77 row carrying the marker is
/// impossible (the fold always leaves 77).
fn validate_top_error(
    outcome: &OwnerOutcome,
    exit_code: u8,
    error: &Option<OperationError>,
) -> Result<(), String> {
    let kind = error.as_ref().map(|item| item.kind);
    let release_folded = error.as_ref().is_some_and(has_release_evidence);
    let agrees = match outcome {
        OwnerOutcome::Succeeded => exit_code == 0 && kind.is_none(),
        OwnerOutcome::Partial => exit_code == 1 && kind == Some(ErrorKind::WorkerReclaim),
        OwnerOutcome::BoundExceeded => exit_code == 124 && kind == Some(ErrorKind::BoundExceeded),
        OwnerOutcome::Cancelled => exit_code == 78 && kind == Some(ErrorKind::Cancelled),
        OwnerOutcome::Error => {
            // Owner exits form a closed five-value vocabulary (outcome.rs
            // error_exit: 0, 1, 77, 78, 124), so the final else is the full
            // complement, not a wildcard over a state enum. A u8 cannot be
            // exhaustively matched; the if-chain below is arm-identical.
            if exit_code == 77 {
                matches!(
                    kind,
                    Some(ErrorKind::RosterUnmeasurable | ErrorKind::LeaseInputUnmeasurable)
                ) && !release_folded
            } else if exit_code == 78 {
                // A lock-acquisition failure has no lease to release, so a
                // Lease kind never survives the fold: preserved priors are
                // WorkerReclaim, Cancelled, and Roster only.
                matches!(
                    kind,
                    Some(ErrorKind::OwnerMachinery | ErrorKind::Transport)
                ) || (matches!(
                    kind,
                    Some(
                        ErrorKind::WorkerReclaim
                            | ErrorKind::Cancelled
                            | ErrorKind::RosterUnmeasurable
                    )
                ) && release_folded)
            } else if exit_code == 124 {
                matches!(error, Some(item) if item.kind == ErrorKind::BoundExceeded)
                    && release_folded
            } else {
                false
            }
        }
        // UnknownPartialEffects: exit+backing enforced by the matrix arm;
        // the top error is severity-folded leftovers, unconstrained here.
        OwnerOutcome::UnknownPartialEffects => true,
    };
    if agrees {
        Ok(())
    } else {
        Err(format!(
            "top error {kind:?} disagrees with outcome {outcome:?} exit {exit_code}"
        ))
    }
}

/// Exact release-fold marker: the owner appends exactly
/// `OWNER_LOCK_RELEASE_FAILED message=<message> evidence=<evidence>`
/// (operation_lock.rs apply_release_failure, the ONLY writer). The line must
/// START with the marker prefix and carry the later ` evidence=` separator;
/// the marker as a substring anywhere else (quoted prose, a foreign code) is
/// not the fold and refuses.
fn has_release_evidence(error: &OperationError) -> bool {
    error.evidence.iter().any(|line| {
        line.strip_prefix("OWNER_LOCK_RELEASE_FAILED message=")
            .is_some_and(|rest| rest.contains(" evidence="))
    })
}

/// Top/worker backing: when workers are nonempty, every top error selected
/// from worker severity must still be present on a worker row with the same
/// kind AND code and a compatible worker outcome. True pre-worker shapes
/// (empty worker rows) are exempt for owner-level kinds only — no owner
/// seed constructor builds a WorkerReclaim or BoundExceeded top, so those
/// refuse with no rows. UnknownPartialEffects keeps its own backing arm.
///
/// Compatibility is exact per kind: a Cancelled top rides on a Cancelled
/// worker, a BoundExceeded top on a BoundExceeded worker, a WorkerReclaim
/// top on a Blocked or Error worker, and a worker-derived Roster, Lease,
/// Machinery, or Transport top on an Error worker. "Exact nested remote if
/// present" rides on `validate_worker_remote`, which every nested remote in
/// this response must also pass in the worker loop: outcome, exit, and error
/// kind agree as a triple, so no forwarded row can smuggle a contradictory
/// remote past this gate.
///
/// ONE exception: the release-created error. When a prior success (no top
/// error) fails at lock release, the fold SETS a fresh OwnerMachinery error
/// (owner lib.rs release_and_record + operation_lock.rs release_error,
/// code `OWNER_LOCK_RELEASE_FAILED`) that no worker sources. Kind+code
/// identify that constructor exactly; the exemption ends there and never
/// excuses any other nonempty unmatched kind.
fn validate_top_worker_coupling(response: &OwnerResponse) -> Result<(), String> {
    if response.outcome == OwnerOutcome::UnknownPartialEffects {
        return Ok(());
    }
    if response.workers.is_empty() {
        // True pre-worker shapes carry only owner-level kinds: every owner
        // seed constructor (durable_failure, recovery_needed_response,
        // refusal_response) emits with empty workers, and no seed builds a
        // WorkerReclaim or BoundExceeded top. Those two have no row to be
        // selected from and refuse; anything else is owner-level and passes.
        return match &response.error {
            None => Ok(()),
            Some(top) => match top.kind {
                ErrorKind::WorkerReclaim | ErrorKind::BoundExceeded => Err(format!(
                    "worker-severity top error {} with no worker rows",
                    top.code
                )),
                _ => Ok(()),
            },
        };
    }
    let top = match &response.error {
        None => return Ok(()),
        Some(error) => error,
    };
    if top.kind == ErrorKind::OwnerMachinery && top.code == "OWNER_LOCK_RELEASE_FAILED" {
        return Ok(());
    }
    let backed = response.workers.iter().any(|worker| {
        worker.error.as_ref().is_some_and(|worker_error| {
            worker_error.kind == top.kind
                && worker_error.code == top.code
                && backing_compatible(top.kind, worker.outcome)
        })
    });
    if backed {
        Ok(())
    } else {
        Err(format!(
            "worker-derived top error {} has no backing worker row",
            top.code
        ))
    }
}

fn backing_compatible(kind: ErrorKind, outcome: WorkerRunOutcome) -> bool {
    match kind {
        ErrorKind::Cancelled => outcome == WorkerRunOutcome::Cancelled,
        ErrorKind::BoundExceeded => outcome == WorkerRunOutcome::BoundExceeded,
        ErrorKind::WorkerReclaim => matches!(
            outcome,
            WorkerRunOutcome::Blocked | WorkerRunOutcome::Error
        ),
        ErrorKind::RosterUnmeasurable
        | ErrorKind::LeaseInputUnmeasurable
        | ErrorKind::OwnerMachinery
        | ErrorKind::Transport => outcome == WorkerRunOutcome::Error,
    }
}

fn validate_owner_response(
    request_id: &str,
    mode: OwnerMode,
    response: &OwnerResponse,
) -> Result<(), String> {
    if response.schema != OWNER_SCHEMA
        || response.request_id != request_id
        || response.mode != mode
    {
        return Err(format!(
            "response correlation mismatch: expected schema={OWNER_SCHEMA} request_id={request_id} mode={mode:?}"
        ));
    }
    validate_outcome_exit(&response.outcome, response.exit_code, &response.workers)?;
    validate_top_error(&response.outcome, response.exit_code, &response.error)?;
    validate_top_worker_coupling(&response)?;
    if response.workers.is_empty() {
        // Empty worker rows are valid ONLY for pre-roster failures (owner
        // operation lock, validation, or roster): the outcome must name one
        // and must carry its error. Any outcome implying visited workers
        // with none present is corruption, not an empty result. Enumerated
        // arm-by-arm with no wildcard: six outcomes times two error states
        // is twelve arms, so adding an outcome must decide here.
        match (&response.outcome, &response.error) {
            (OwnerOutcome::Error, Some(_)) | (OwnerOutcome::Cancelled, Some(_)) => {}
            (OwnerOutcome::Error, None)
            | (OwnerOutcome::Succeeded, None)
            | (OwnerOutcome::Succeeded, Some(_))
            | (OwnerOutcome::Partial, None)
            | (OwnerOutcome::Partial, Some(_))
            | (OwnerOutcome::BoundExceeded, None)
            | (OwnerOutcome::BoundExceeded, Some(_))
            | (OwnerOutcome::Cancelled, None)
            | (OwnerOutcome::UnknownPartialEffects, None)
            | (OwnerOutcome::UnknownPartialEffects, Some(_)) => {
                return Err(
                    "owner response carries no worker rows for an outcome requiring visited workers"
                        .to_owned(),
                )
            }
        }
    }
    // Severity fold: any worker error raises the exit above 0, so a
    // Succeeded response with a worker error anywhere is not an owner answer.
    if response.outcome == OwnerOutcome::Succeeded
        && response.workers.iter().any(|worker| worker.error.is_some())
    {
        return Err("succeeded response carries worker errors".to_owned());
    }
    for worker in &response.workers {
        validate_worker_record(request_id, mode, worker)?;
    }
    Ok(())
}

/// One worker row, fully required: every field the owner emits is typed, and
/// the nested remote answer carries its own exact backend coupling. Basic
/// outcome/error/remote consistency below; no roster cardinality (any nonzero
/// length) and no policy thresholds (values unjudged).
fn validate_worker_record(
    request_id: &str,
    mode: OwnerMode,
    worker: &WorkerRunRecord,
) -> Result<(), String> {
    // Basic outcome/error/remote consistency (owner record construction): an
    // unvisited worker carries no remote answer and no error; a succeeded
    // worker carries no error and never wraps a failed remote answer. The
    // release-failure shapes (BoundExceeded/Error/UPE with their exact
    // errors) all carry errors by construction and pass these gates.
    if worker.outcome == WorkerRunOutcome::NotVisited
        && (worker.remote.is_some() || worker.error.is_some())
    {
        return Err("unvisited worker carries a remote answer or error".to_owned());
    }
    if worker.outcome == WorkerRunOutcome::Succeeded && worker.error.is_some() {
        return Err("succeeded worker carries an error".to_owned());
    }
    if worker.outcome == WorkerRunOutcome::Succeeded {
        if let Some(remote) = &worker.remote {
            if remote.outcome != WorkerOutcome::Succeeded {
                return Err("succeeded worker wraps a non-succeeded remote answer".to_owned());
            }
        }
    }
    // Remaining fields required present by shape; values unjudged (no policy
    // thresholds, no roster judgments).
    let _ = (
        &worker.worker.id,
        &worker.worker.host,
        &worker.worker.user,
        &worker.prior_admission,
        &worker.drain.outcome,
        &worker.drain.observed_state,
        &worker.drain.used_slots,
        &worker.drain.evidence,
        &worker.restore.outcome,
        &worker.restore.restored_state,
        &worker.restore.evidence,
        &worker.outcome,
        &worker.error,
    );
    if let Some(remote) = &worker.remote {
        validate_worker_remote(request_id, mode, remote)?;
    }
    Ok(())
}

fn validate_worker_remote(
    request_id: &str,
    mode: OwnerMode,
    remote: &WorkerResponse,
) -> Result<(), String> {
    if remote.schema != OWNER_SCHEMA
        || remote.request_id != request_id
        || remote.mode != mode
    {
        return Err("nested remote schema, request, or mode mismatch".to_owned());
    }
    // Exact backend coupling (owner `backend.rs` worker validation): the
    // outcome, exit, and error kind must agree as a triple — enum membership
    // alone is not enough. Pressure is closed by type; anything outside
    // NORMAL/CRITICAL fails at parse.
    let error_kind = remote.error.as_ref().map(|error| error.kind);
    let valid = match remote.outcome {
        WorkerOutcome::Succeeded => remote.exit_code == 0 && remote.error.is_none(),
        WorkerOutcome::Refused => {
            remote.exit_code == 1 && error_kind == Some(ErrorKind::WorkerReclaim)
        }
        WorkerOutcome::Error => matches!(
            (remote.exit_code, error_kind),
            (1, Some(ErrorKind::WorkerReclaim)) | (78, Some(ErrorKind::OwnerMachinery))
        ),
        WorkerOutcome::Cancelled => {
            remote.exit_code == 78 && error_kind == Some(ErrorKind::Cancelled)
        }
        WorkerOutcome::BoundExceeded => {
            remote.exit_code == 124 && error_kind == Some(ErrorKind::BoundExceeded)
        }
        WorkerOutcome::UnknownPartialEffects => {
            // Owner exits are 0, 1, 77, 78, 124 and UPE admits 1, 78, 124;
            // the final else is the full complement on a u8, which cannot be
            // exhaustively matched. Arm-identical to the match it replaces.
            if remote.exit_code == 1 {
                error_kind == Some(ErrorKind::WorkerReclaim)
            } else if remote.exit_code == 78 {
                error_kind == Some(ErrorKind::Cancelled)
            } else if remote.exit_code == 124 {
                error_kind == Some(ErrorKind::BoundExceeded)
            } else {
                false
            }
        }
    };
    if !valid {
        return Err(format!(
            "nested remote outcome {:?} disagrees with exit {} and error {:?}",
            remote.outcome, remote.exit_code, error_kind
        ));
    }
    let _ = (
        &remote.owner_root,
        remote.idle_threshold_secs,
        &remote.candidates,
        &remote.effects,
        remote.pressure_mode,
    );
    Ok(())
}
fn parse_cancel_response(
    request_id: &str,
    output: &OwnerProcessOutput,
) -> Result<(), String> {
    check_combined_size(&output.stdout, &output.stderr)?;
    let process_exit = output
        .exit_code
        .ok_or_else(|| "cancel response had no process exit code".to_owned())?;
    if output.stdout.iter().all(u8::is_ascii_whitespace) {
        return Err("cancel response was empty".to_owned());
    }
    let response: CancelResponse = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("cancel response JSON: {error}"))?;
    if response.schema != OWNER_SCHEMA || response.request_id != request_id {
        return Err("cancel response correlation mismatch".to_owned());
    }
    if !response.cancellation_requested {
        return Err("cancel response did not confirm cancellation_requested".to_owned());
    }
    if response.exit_code != 0 || response.exit_code != process_exit {
        return Err(format!(
            "cancel response exit {} (process {}) is not the exact positive 0",
            response.exit_code, process_exit
        ));
    }
    Ok(())
}

fn parse_durable_status(
    request_id: &str,
    mode: OwnerMode,
    output: &OwnerProcessOutput,
) -> Result<DurableStatusResponse, String> {
    check_combined_size(&output.stdout, &output.stderr)?;
    let process_exit = output
        .exit_code
        .ok_or_else(|| "status response had no process exit code".to_owned())?;
    if output.stdout.iter().all(u8::is_ascii_whitespace) {
        return Err("status response was empty".to_owned());
    }
    let status: DurableStatusResponse = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("durable status JSON: {error}"))?;
    if status.schema != OWNER_SCHEMA
        || status.request_id != request_id
        || status.state.schema != OWNER_SCHEMA
        || status.state.request_id != request_id
        || status.state.mode != mode
    {
        return Err("durable status schema, request, or mode mismatch".to_owned());
    }
    // The status read itself always answers 0/0: the owner hardcodes the
    // stored body exit to 0 and emits it with process exit 0. Anything else
    // is a failed status read, not state. The nested terminal carries the
    // RUN's own exit independently and is never equated here.
    if process_exit != 0 {
        return Err(format!(
            "status process exit={process_exit} is not the required 0"
        ));
    }
    if status.exit_code != 0 {
        return Err(format!(
            "status body exit_code={} is not the required 0",
            status.exit_code
        ));
    }
    if status.state.workers.is_empty() {
        return Err("durable status carries no worker rows".to_owned());
    }
    Ok(status)
}

fn forwarded_from_status(
    request_id: &str,
    mode: OwnerMode,
    status: &DurableStatusResponse,
) -> Result<OwnerForwardedResponse, String> {
    // Terminal forward requires every durable worker lease inactive: a held
    // lease means the owner may still write, so no terminal row is final.
    if status
        .state
        .workers
        .iter()
        .any(|worker| worker.lease_active)
    {
        return Err("durable status still reports an active worker lease".to_owned());
    }
    let terminal = status
        .terminal
        .as_ref()
        .ok_or_else(|| "durable status has no terminal response".to_owned())?;
    validate_owner_response(request_id, mode, terminal)?;
    let raw_stdout = serde_json::to_vec(terminal)
        .map_err(|error| format!("durable terminal response encoding: {error}"))?;
    Ok(OwnerForwardedResponse {
        raw_stdout,
        exit_code: terminal.exit_code,
        request_id: request_id.to_owned(),
        recovered_from_status: true,
    })
}

/// Spawn the fixed-argv owner and wait under bounded local ownership.
///
/// The child runs in a FRESH LOCAL process group (`NewProcessGroup`) with
/// group-directed signals (`ProcessGroup` target): cancellation terminates
/// the group, never the pid alone. `kill_on_drop(true)` keeps the child
/// owned so abandonment terminates and reaps instead of orphaning. Both
/// pipes drain concurrently through region-owned capped reader tasks, so a
/// hostile child cannot deadlock the ~64 KiB pipe stall NOR allocate
/// unbounded memory: each stream stops at MAX+1 bytes during the read.
/// No remote kill, no background task, no nohup, no systemd: everything this
/// function can affect is its own group.
/// Owner verbs invoked here take validated scalar argv and no request body
/// (the worker RPC surface, which does read stdin, is out of scope and is
/// never invoked from this crate).
async fn invoke_owner(
    cx: Cx,
    argv: Vec<String>,
    deadline: Instant,
) -> Result<OwnerProcessOutput, ConsumerError> {
    checkpoint(&cx)?;
    let mut command = Command::new(OWNER_BINARY);
    command
        .args(&argv)
        .stdin(Stdio::Null)
        .stdout(Stdio::Pipe)
        .stderr(Stdio::Pipe)
        .process_group_mode(ProcessGroupMode::NewProcessGroup)
        .signal_target(ProcessSignalTarget::ProcessGroup)
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|error| {
        ConsumerError::owner_machinery(OwnerMachineryReason::Spawn, error.to_string())
    })?;
    let stdout_pipe = child.stdout().ok_or_else(|| {
        ConsumerError::owner_machinery(
            OwnerMachineryReason::Spawn,
            "owner stdout pipe unavailable after spawn",
        )
    })?;
    let stderr_pipe = child.stderr().ok_or_else(|| {
        ConsumerError::owner_machinery(
            OwnerMachineryReason::Spawn,
            "owner stderr pipe unavailable after spawn",
        )
    })?;
    let mut out_reader = cx
        .spawn(|_| async move { drain_capped(stdout_pipe).await })
        .map_err(|error| {
            ConsumerError::owner_machinery(
                OwnerMachineryReason::Spawn,
                format!("reader spawn failed: {error:?}"),
            )
        })?;
    let mut err_reader = match cx.spawn(|_| async move { drain_capped(stderr_pipe).await }) {
        Ok(handle) => handle,
        Err(error) => {
            // Group dead first: nobody will ever drain these pipes, so stop
            // the writers before owning reader terminals. Then abort and
            // bounded-rejoin the running reader; nothing leaks on this path.
            let _ = child.kill();
            reap_bounded(&mut child).await;
            out_reader.abort();
            let _ = abort_rejoin_reader(&cx, &mut out_reader).await;
            return Err(ConsumerError::owner_machinery(
                OwnerMachineryReason::Spawn,
                format!("reader spawn failed: {error:?}"),
            ));
        }
    };
    // Readers-first lifecycle: the unreaped child (and its group handle)
    // stays alive while the bounded reader joins run. Only after both pipes
    // reach EOF are the leader's exit collected and the child reaped. A
    // leader that exits first cannot strand anything: EOF follows. A leader
    // that never exits trips the reader deadline below, which terminates the
    // still-owned group — never after a reap that would drop the handle.
    let (outcome_out, outcome_err) = loop {
        if cx.is_cancel_requested() {
            // Caller cancel: terminate the group and reap FIRST (the handle
            // is live — no wait has run yet), then abort and bounded-rejoin
            // both readers to own their terminals.
            let _ = child.kill();
            reap_bounded(&mut child).await;
            out_reader.abort();
            err_reader.abort();
            let _ = abort_rejoin_reader(&cx, &mut out_reader).await;
            let _ = abort_rejoin_reader(&cx, &mut err_reader).await;
            return Err(ConsumerError::owner_machinery(
                OwnerMachineryReason::OwnerWait,
                "owner run cancelled by caller",
            ));
        }
        if Instant::now() >= deadline {
            // Reader deadline with pipes still open: silent leader or
            // lingering holder. Terminate the owned group and reap FIRST,
            // then abort and bounded-rejoin; refuse bounded without
            // claiming any terminal join.
            let _ = child.kill();
            reap_bounded(&mut child).await;
            out_reader.abort();
            err_reader.abort();
            let _ = abort_rejoin_reader(&cx, &mut out_reader).await;
            let _ = abort_rejoin_reader(&cx, &mut err_reader).await;
            return Err(ConsumerError::owner_machinery(
                OwnerMachineryReason::OwnerWait,
                "bounded refusal: reader join deadline with pipes open; group terminated+reaped, readers aborted+released",
            ));
        }
        // Non-blocking poll of both readers: a completed reader (bytes or
        // io failure) resolves here; a task-level failure escalates like a
        // deadline (group first, then readers).
        let out_poll = out_reader.try_join();
        let err_poll = err_reader.try_join();
        match (out_poll, err_poll) {
            (Ok(Some(outcome)), Ok(Some(outcome_err))) => break (outcome, outcome_err),
            (Err(_), _) | (_, Err(_)) => {
                let _ = child.kill();
                reap_bounded(&mut child).await;
                out_reader.abort();
                err_reader.abort();
                let _ = abort_rejoin_reader(&cx, &mut out_reader).await;
                let _ = abort_rejoin_reader(&cx, &mut err_reader).await;
                return Err(ConsumerError::owner_machinery(
                    OwnerMachineryReason::OwnerWait,
                    "reader task failed; group terminated+reaped before abort",
                ));
            }
            _ => {}
        }
        asupersync::time::sleep(
            asupersync::time::wall_now(),
            RUN_POLL_QUANTUM,
        )
        .await;
    };
    // Both readers resolved. A pipe-level failure with a live leader still
    // owns an untrustworthy boundary: terminate, reap, refuse.
    let stdout = match outcome_out {
        Ok(buf) => buf,
        Err(error) => {
            let _ = child.kill();
            reap_bounded(&mut child).await;
            out_reader.abort();
            err_reader.abort();
            let _ = abort_rejoin_reader(&cx, &mut out_reader).await;
            let _ = abort_rejoin_reader(&cx, &mut err_reader).await;
            return Err(ConsumerError::owner_machinery(
                OwnerMachineryReason::OwnerWait,
                format!("pipe read failed: {error}"),
            ));
        }
    };
    let stderr = match outcome_err {
        Ok(buf) => buf,
        Err(error) => {
            let _ = child.kill();
            reap_bounded(&mut child).await;
            out_reader.abort();
            err_reader.abort();
            let _ = abort_rejoin_reader(&cx, &mut out_reader).await;
            let _ = abort_rejoin_reader(&cx, &mut err_reader).await;
            return Err(ConsumerError::owner_machinery(
                OwnerMachineryReason::OwnerWait,
                format!("pipe read failed: {error}"),
            ));
        }
    };
    // Pipes EOF'd, so every writer is done: collect the leader's exit and
    // reap it. wait_async here returns promptly; its cancel escalation is a
    // backstop for the sliver where cancellation lands between the loop and
    // this wait.
    let status = match child.wait_async(&cx).await {
        Ok(status) => status,
        Err(error) => {
            let detail = if matches!(&error, ProcessError::Io(io_error) if io_error.kind() == std::io::ErrorKind::Interrupted) {
                "owner run cancelled by caller".to_owned()
            } else {
                process_error_detail(error)
            };
            return Err(ConsumerError::owner_machinery(
                OwnerMachineryReason::OwnerWait,
                detail,
            ));
        }
    };
    let total = stdout.len().saturating_add(stderr.len());
    if total > MAX_OWNER_OUTPUT_BYTES {
        // Over cap: terminate the local group (a member may still hold an
        // unrelated descriptor) and reap bounded; the bytes are refused,
        // never forwarded. Transient allocation never exceeded 2*(MAX+1).
        let _ = child.kill();
        reap_bounded(&mut child).await;
        return Err(ConsumerError::owner_machinery(
            OwnerMachineryReason::OwnerWait,
            format!(
                "owner combined output {total} bytes exceeds {MAX_OWNER_OUTPUT_BYTES} byte ceiling"
            ),
        ));
    }
    Ok(OwnerProcessOutput {
        stdout,
        stderr,
        exit_code: status
            .code()
            .and_then(|code| u8::try_from(code).ok()),
    })
}

/// Drain one pipe to a capped buffer. The `take` adapter stops the read at
/// MAX+1 bytes mid-stream, so the cap holds during collection, not after it.
async fn drain_capped<R>(reader: R) -> Result<Vec<u8>, std::io::Error>
where
    R: asupersync::io::AsyncRead + Unpin + Send + 'static,
{
    let mut taken = Box::new(reader.take(STREAM_CAP_PLUS_ONE));
    let mut buf = Vec::new();
    taken.read_to_end(&mut buf).await?;
    Ok(buf)
}

/// Abort a reader and re-join it under a control bound to own its terminal
/// state. Never an unbounded join: after abort the task is terminal, so the
/// bound guards scheduler delay only.
async fn abort_rejoin_reader(
    cx: &Cx,
    handle: &mut TaskHandle<Result<Vec<u8>, std::io::Error>>,
) -> Result<Vec<u8>, String> {
    handle.abort();
    let rejoined = asupersync::time::timeout(
        asupersync::time::wall_now(),
        CONTROL_COMMAND_BOUND,
        handle.join(cx),
    )
    .await;
    match rejoined {
        Ok(Ok(Ok(buf))) => Ok(buf),
        Ok(Ok(Err(error))) => Err(format!("pipe read failed: {error}")),
        Ok(Err(JoinError::Cancelled(_))) => Err("aborted reader resolved cancelled".to_owned()),
        Ok(Err(JoinError::Panicked(_))) => Err("aborted reader resolved panicked".to_owned()),
        Ok(Err(JoinError::PolledAfterCompletion)) => {
            Err("aborted reader resolved polled-after-completion".to_owned())
        }
        Err(_) => Err(format!(
            "aborted reader rejoin exceeded control bound={}s",
            CONTROL_COMMAND_BOUND.as_secs()
        )),
    }
}

/// Bounded reap after an explicit kill. SIGKILL delivery is prompt; the cap
/// exists so a kernel quirk cannot stall the path. `kill_on_drop` remains as
/// backstop on the handle drop, never as plan.
async fn reap_bounded(child: &mut Child) {
    for _ in 0..REAP_POLLS {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => return,
            Ok(None) => {
                asupersync::time::sleep(
                    asupersync::time::wall_now(),
                    REAP_POLL_INTERVAL,
                )
                .await
            }
        }
    }
}

fn checkpoint(cx: &Cx) -> Result<(), ConsumerError> {
    cx.checkpoint().map_err(|_| {
        ConsumerError::owner_machinery(
            OwnerMachineryReason::Cancelled,
            "consumer context cancelled",
        )
    })
}

fn process_error_detail(error: ProcessError) -> String {
    error.to_string()
}

fn stderr_detail(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr).trim().to_owned()
}

/// Minimal owner answer shapes. Lenient by design: the wire is versioned and
/// this consumer forwards bytes it need not fully model, so unvalidated
/// fields are ignored — but every field named in the validation contract is
/// typed here, and a sooner owner version that renames one fails closed at
/// parse. No roster cardinality and no policy thresholds anywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum OwnerOutcome {
    Succeeded,
    Partial,
    Error,
    UnknownPartialEffects,
    Cancelled,
    BoundExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum WorkerOutcome {
    Succeeded,
    Refused,
    Error,
    UnknownPartialEffects,
    Cancelled,
    BoundExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum WorkerRunOutcome {
    Succeeded,
    Blocked,
    Error,
    UnknownPartialEffects,
    Cancelled,
    BoundExceeded,
    NotVisited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum StepOutcome {
    Succeeded,
    Preserved,
    Refused,
    Error,
    Cancelled,
    BoundExceeded,
    NotAttempted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum AdmissionState {
    Enabled,
    Draining,
    Drained,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum CandidateDisposition {
    Eligible,
    NotIdle,
    Refused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum EffectKind {
    QuarantineIntent,
    Quarantined,
    RestoreIntent,
    Restored,
    PurgeIntent,
    Purged,
    PurgeInterrupted,
    RemoteOutcomeUnknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ReclaimPressureMode {
    Normal,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ErrorKind {
    RosterUnmeasurable,
    LeaseInputUnmeasurable,
    OwnerMachinery,
    Transport,
    WorkerReclaim,
    Cancelled,
    BoundExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct WorkerEndpoint {
    id: String,
    host: String,
    user: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct OperationError {
    kind: ErrorKind,
    code: String,
    message: String,
    evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct DrainRecord {
    outcome: StepOutcome,
    observed_state: Option<AdmissionState>,
    used_slots: Option<u32>,
    evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct RestoreRecord {
    outcome: StepOutcome,
    restored_state: Option<AdmissionState>,
    evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct PathEvidence {
    bytes_hex: String,
    display_lossy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct CandidateRecord {
    path: PathEvidence,
    disposition: CandidateDisposition,
    idle_secs: Option<u64>,
    bytes: Option<u64>,
    evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct EffectRecord {
    kind: EffectKind,
    original: Option<PathEvidence>,
    quarantine: Option<PathEvidence>,
    bytes: Option<u64>,
    evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct WorkerResponse {
    schema: String,
    request_id: String,
    mode: OwnerMode,
    outcome: WorkerOutcome,
    owner_root: String,
    idle_threshold_secs: u64,
    pressure_mode: ReclaimPressureMode,
    candidates: Vec<CandidateRecord>,
    effects: Vec<EffectRecord>,
    error: Option<OperationError>,
    exit_code: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct WorkerRunRecord {
    worker: WorkerEndpoint,
    prior_admission: Option<AdmissionState>,
    drain: DrainRecord,
    remote: Option<WorkerResponse>,
    restore: RestoreRecord,
    outcome: WorkerRunOutcome,
    error: Option<OperationError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct OwnerResponse {
    schema: String,
    request_id: String,
    mode: OwnerMode,
    outcome: OwnerOutcome,
    workers: Vec<WorkerRunRecord>,
    error: Option<OperationError>,
    exit_code: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct DurableWorkerState {
    worker_id: String,
    lease_active: bool,
    phase: DurablePhase,
    restore: Option<RestoreRecord>,
    effects: Vec<EffectRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum DurablePhase {
    Started,
    Precondition,
    Draining,
    LeaseOwned,
    Worker,
    Restoring,
    Complete,
    RecoveryNeeded,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct DurableOperationState {
    schema: String,
    request_id: String,
    mode: OwnerMode,
    workers: Vec<DurableWorkerState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct DurableStatusResponse {
    schema: String,
    request_id: String,
    state: DurableOperationState,
    terminal: Option<OwnerResponse>,
    exit_code: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct CancelResponse {
    schema: String,
    request_id: String,
    cancellation_requested: bool,
    exit_code: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_owner_contract_constants_are_stable() {
        assert_eq!(OWNER_SCHEMA, "control-plane.rch-reclaim-owner/v1");
        assert_eq!(OWNER_BINARY, "rch-reclaim-owner");
        assert_eq!(OWNER_MACHINERY_EXIT, 78);
        assert_eq!(CONTROL_COMMAND_BOUND_SECS, 15);
    }

    #[test]
    fn run_deadline_is_the_owner_worst_case_and_outer_covers_recovery() {
        // Owner wire v1 worst case: 15 + 4*390 + 15 = 1590s bounds the RUN;
        // the outer 1800s reserves recovery (cancel, drain, status) beyond it.
        assert_eq!(RUN_DEADLINE_SECS, 1590);
        assert_eq!(RUN_DEADLINE.as_secs(), RUN_DEADLINE_SECS);
        assert!(CONSUMER_OUTER_BOUND_SECS > RUN_DEADLINE_SECS);
    }

    #[test]
    fn request_id_grammar_matches_the_owner_exactly() {
        for valid in ["a", "req-1", "A._:-Z09", &"x".repeat(96)] {
            validate_request_id(valid)
                .unwrap_or_else(|_| panic!("owner grammar accepts {valid:?}"));
        }
        for invalid in ["", &"x".repeat(97), "has space", "semi;colon", "quo'te", "ünïcode"] {
            assert!(
                validate_request_id(invalid).is_err(),
                "owner grammar refuses {invalid:?}"
            );
        }
    }

    #[test]
    fn pending_control_observes_terminal_abort_and_join() {
        // A control invocation that never resolves on its own must not merely
        // return on expiry: settle aborts the owned task explicitly and
        // bounded-joins it before reporting. Both deadlines are already
        // expired, so the abort arm runs immediately and the test observes
        // its exact wording — not merely a return. The factory loops on the
        // task Cx exactly like production invoke_owner checkpoints and
        // readers do: a bare pending() never observes cancellation, so the
        // abort would honestly stay nonterminal (that is the other leg).
        let runtime = asupersync::runtime::RuntimeBuilder::current_thread()
            .build()
            .expect("test runtime");
        let detail = runtime.block_on(async {
            let cx = Cx::current().expect("ambient Cx");
            let past = Instant::now() - Duration::from_secs(1);
            settle_control(
                &cx,
                &cx,
                |task_cx: Cx| async move {
                    loop {
                        if task_cx.is_cancel_requested() {
                            return 7u8;
                        }
                        asupersync::time::sleep(
                            asupersync::time::wall_now(),
                            RUN_POLL_QUANTUM,
                        )
                        .await;
                    }
                },
                past,
                past,
                "test-control",
                CONTROL_COMMAND_BOUND,
            )
            .await
            .expect_err("pending control must expire")
        });
        assert!(
            detail.contains("task aborted and joined"),
            "terminal rejoin must be claimed exactly, got: {detail}"
        );
        assert!(
            !detail.contains("nonterminal"),
            "a landed abort is terminal, got: {detail}"
        );
    }

    #[test]
    fn expired_rejoin_reports_nonterminal_not_joined() {
        // The complement and the UNKNOWN leg: a truly cancellation-blind
        // pending task never observes the abort, so the bounded rejoin
        // expires and settle must say nonterminal explicitly, never claiming
        // a join. A zero rejoin bound expires without polling on a
        // current-thread runtime, so the leg is deterministic.
        let runtime = asupersync::runtime::RuntimeBuilder::current_thread()
            .build()
            .expect("test runtime");
        let detail = runtime.block_on(async {
            let cx = Cx::current().expect("ambient Cx");
            let past = Instant::now() - Duration::from_secs(1);
            settle_control(
                &cx,
                &cx,
                |_task_cx: Cx| std::future::pending::<u8>(),
                past,
                past,
                "test-control",
                Duration::ZERO,
            )
            .await
            .expect_err("pending control must expire")
        });
        assert!(
            detail.contains("nonterminal"),
            "an expired rejoin must say nonterminal, got: {detail}"
        );
        assert!(
            !detail.contains("aborted and joined"),
            "a nonterminal task must never claim joined, got: {detail}"
        );
    }

    #[test]
    fn ready_control_completes_through_the_owned_task() {
        // Positive arm: an already-ready invocation delivers its value
        // through the task, proving the task path is not abort-only.
        let runtime = asupersync::runtime::RuntimeBuilder::current_thread()
            .build()
            .expect("test runtime");
        let value = runtime.block_on(async {
            let cx = Cx::current().expect("ambient Cx");
            let future = Instant::now() + Duration::from_secs(60);
            settle_control(
                &cx,
                &cx,
                |_task_cx: Cx| async { 7u8 },
                future,
                future,
                "test-control",
                CONTROL_COMMAND_BOUND,
            )
            .await
            .expect("ready control completes")
        });
        assert_eq!(value, 7);
    }

    #[test]
    fn status_forward_names_lease_and_terminal_gates() {
        // Exact causality with no runtime: an active lease and an absent
        // terminal each refuse with their own message. The seam-level
        // negatives prove recovery refuses; this pins WHY.
        let leased: DurableStatusResponse = serde_json::from_value(serde_json::json!({
            "schema": OWNER_SCHEMA,
            "request_id": "req-unit",
            "state": {
                "schema": OWNER_SCHEMA,
                "request_id": "req-unit",
                "mode": "REPORT",
                "workers": [{"worker_id": "contabo-1", "lease_active": true, "phase": "WORKER", "restore": null, "effects": []}],
                "current_worker": "contabo-1",
                "phase": "COMPLETE"
            },
            "terminal": {
                "schema": OWNER_SCHEMA,
                "request_id": "req-unit",
                "mode": "REPORT",
                "outcome": "SUCCEEDED",
                "workers": [],
                "error": null,
                "exit_code": 0
            },
            "kill_authorized": true,
            "exit_code": 0
        }))
        .expect("leased status parses");
        let detail =
            forwarded_from_status("req-unit", OwnerMode::Report, &leased)
                .expect_err("active lease must refuse");
        assert!(
            detail.contains("lease"),
            "active lease refusal must name the lease, got: {detail}"
        );
        let unended: DurableStatusResponse = serde_json::from_value(serde_json::json!({
            "schema": OWNER_SCHEMA,
            "request_id": "req-unit",
            "state": {
                "schema": OWNER_SCHEMA,
                "request_id": "req-unit",
                "mode": "REPORT",
                "workers": [{"worker_id": "contabo-1", "lease_active": false, "phase": "WORKER", "restore": null, "effects": []}],
                "current_worker": "contabo-1",
                "phase": "COMPLETE"
            },
            "terminal": null,
            "kill_authorized": true,
            "exit_code": 0
        }))
        .expect("unterminated status parses");
        let detail =
            forwarded_from_status("req-unit", OwnerMode::Report, &unended)
                .expect_err("absent terminal must refuse");
        assert!(
            detail.contains("no terminal"),
            "absent terminal refusal must name it, got: {detail}"
        );
    }
}

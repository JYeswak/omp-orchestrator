#![forbid(unsafe_code)]

//! `inbox-monitor` — read the two Agent Mail surfaces and exit with a code an operator
//! cannot ignore.
//!
//! ```text
//! inbox-monitor --agent <NAME> [--project <PATH>] [--pane <SESSION>:<INDEX>] [--position-now]
//! ```
//!
//! Emits ONE line of JSON on stdout, appends the same row to
//! `$HOME/.local/state/flywheel/inbox-monitor/ledger.jsonl`, and exits with
//! `MonitorVerdict::exit_code()`.
//!
//! # The single most important behaviour in this binary
//!
//! Every child process goes through `subprocess_contract::bounded_output`, and both of its
//! restrictive outcomes — `TimedOut` and `Unspawned` — map to
//! [`MonitorVerdict::Unreachable`]. Never to `Clear`. A wedged `am`, a missing `am`, a
//! daemon that accepted the connection and then stopped answering: all of them mean the
//! verdict is ABSENT, and a monitor that reports "no mail" when it cannot look is a monitor
//! that is quietest exactly when it is blindest.
//!
//! # Ordering, and why the cursor moves last
//!
//! read feed -> read mailbox -> classify -> resolve pane -> APPEND LEDGER (fsync) -> advance
//! cursor. A crash anywhere before the append replays the same page on the next run, which
//! is a duplicate notification. A crash after the append but before the cursor write also
//! replays. The forbidden order is cursor-then-append, which SKIPS: a duplicate notification
//! is noise, a skipped one is the failure this crate exists to prevent.

use agent_mail_native::client::MailClient;
use agent_mail_native::error::MailError;
use agent_mail_native::journey::{fetch_inbox, AgentName, InboxRequest, ProjectKey};
use asupersync::runtime::RuntimeBuilder;
use asupersync::Cx;
use asupersync::time::sleep;
use inbox_monitor::{
    append_ledger, classify, cursor_path_in, home_dir, iso8601_utc, ledger_path_in, now_epoch_secs,
    parse_event_page, parse_inbox_rows, read_cursor, resolve_pane, unread, watch_step,
    write_cursor_atomic, DaemonArm, EventPage, InboxRow, MonitorVerdict, UnreachableReason,
    WatchOutcome, WatchStep, EXIT_AUTHORITIES_DISAGREE, EXIT_CLEAR, EXIT_CURSOR_BELOW_FLOOR,
    EXIT_CURSOR_REGRESSED, EXIT_MAIL_WAITING, EXIT_UNREACHABLE, EXIT_WATCH_TIMED_OUT,
    WATCH_BLIND_STREAK,
};
use std::path::Path;
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};
use subprocess_contract::{bounded_output, BoundedOutcome};

/// Bound for one `am` read. Generous relative to the measured 0.4–1.1 s server fetch, tight
/// enough that a wedged daemon becomes a typed `Unreachable` inside one tick.
const AM_DEADLINE: Duration = Duration::from_secs(20);
/// Bound for the health probe and the tmux listing — both are local and sub-second.
const PROBE_DEADLINE: Duration = Duration::from_secs(5);

/// The daemon health endpoint. Both halves are also `am`'s own defaults.
const HEALTH_HOST: &str = "127.0.0.1";
const HEALTH_PORT: u16 = 8765;

/// How many unread rows to ask the daemon for. Above the largest count either surface has
/// been measured to hold (128 total / 96 unread on 2026-09-02), because a limit BELOW the
/// true count would manufacture a disagreement out of pagination.
const DAEMON_INBOX_LIMIT: u32 = 500;

/// Default ceiling for `--watch`.
///
/// Five minutes, matching the documented default of the one blocking-wake kernel this fleet
/// already ships (`ntm --robot-wait`), so an operator moving between them does not have to
/// hold two numbers. It is a CEILING and not a cadence: the wake returns the instant mail
/// lands, so the bound only decides how long a quiet window lasts before it re-arms.
const DEFAULT_WATCH_BOUND: Duration = Duration::from_secs(300);

/// Default seconds between polls inside a `--watch`.
///
/// The measured cost of one full poll is two `am` reads plus a health probe, ~0.5–2.0 s
/// against a warm daemon. Five seconds keeps the duty cycle well under half while bounding
/// wake latency to a single-digit number of seconds, which is the latency budget the human
/// -in-the-loop defect actually needs closed: the operator was the notification, and he took
/// minutes.
const DEFAULT_WATCH_INTERVAL: Duration = Duration::from_secs(5);

fn usage() -> String {
    format!(
        "usage:\n  \
         inbox-monitor --agent <NAME> [--project <PATH>] [--pane <SESSION>:<INDEX>] \
         [--position-now]\n  \
         inbox-monitor --agent <NAME> --watch [--timeout <SECS>] [--interval <SECS>] \
         [--project <PATH>] [--pane <SESSION>:<INDEX>]\n\
         \n\
         one-shot (no --watch)  read both Agent Mail surfaces ONCE and exit with the \
         verdict's code.\n\
         --watch                BLOCK until an arrival, a loud fault, or the bound elapses.\n  \
         --timeout <SECS>     ceiling for the whole wait (default {}, minimum 1)\n  \
         --interval <SECS>    seconds between polls (default {}, minimum 1)\n\
         --position-now         establish a cursor baseline and make no mail claim. \
         Incompatible with --watch.\n\
         \n\
         exit codes: {} clear | {} mail waiting | {} unreachable | {} cursor regressed | \
         {} cursor below floor | {} authorities disagree | {} watch timed out | 64 usage\n\
         \n\
         A timeout is not a verdict: {} says no arrival was observed inside the bound, NOT \
         that the inbox is empty ({}), and NOT that the monitor could not look ({}).",
        DEFAULT_WATCH_BOUND.as_secs(),
        DEFAULT_WATCH_INTERVAL.as_secs(),
        EXIT_CLEAR,
        EXIT_MAIL_WAITING,
        EXIT_UNREACHABLE,
        EXIT_CURSOR_REGRESSED,
        EXIT_CURSOR_BELOW_FLOOR,
        EXIT_AUTHORITIES_DISAGREE,
        EXIT_WATCH_TIMED_OUT,
        EXIT_WATCH_TIMED_OUT,
        EXIT_CLEAR,
        EXIT_UNREACHABLE,
    )
}

struct Args {
    agent: String,
    project: String,
    pane: Option<(String, u32)>,
    position_now: bool,
    /// `None` is the one-shot. `Some((bound, interval))` is the blocking watch.
    ///
    /// A single `Option` over the PAIR rather than three loose fields, because a bound
    /// without a watch and an interval without a watch are both mis-invocations, and the
    /// only way to make them unrepresentable rather than silently ignored is to make the
    /// mode own them.
    watch: Option<(Duration, Duration)>,
}

fn parse_args(argv: &[String]) -> Result<Args, String> {
    let mut agent = std::env::var("AGENT_MAIL_AGENT")
        .ok()
        .or_else(|| std::env::var("AGENT_NAME").ok());
    let mut project = std::env::var("AGENT_MAIL_PROJECT").ok();
    let mut pane_spec = std::env::var("INBOX_MONITOR_PANE").ok();
    let mut position_now = false;
    let mut watch = false;
    let mut bound_secs: Option<u64> = None;
    let mut interval_secs: Option<u64> = None;

    let mut index = 0;
    while index < argv.len() {
        match argv[index].as_str() {
            "--agent" => {
                index += 1;
                agent = Some(
                    argv.get(index)
                        .cloned()
                        .ok_or_else(|| format!("--agent requires a value\n{}", usage()))?,
                );
            }
            "--project" => {
                index += 1;
                project = Some(
                    argv.get(index)
                        .cloned()
                        .ok_or_else(|| format!("--project requires a value\n{}", usage()))?,
                );
            }
            "--pane" => {
                index += 1;
                pane_spec = Some(
                    argv.get(index)
                        .cloned()
                        .ok_or_else(|| format!("--pane requires a value\n{}", usage()))?,
                );
            }
            "--position-now" => position_now = true,
            "--watch" => watch = true,
            "--timeout" => {
                index += 1;
                bound_secs = Some(parse_secs("--timeout", argv.get(index))?);
            }
            "--interval" => {
                index += 1;
                interval_secs = Some(parse_secs("--interval", argv.get(index))?);
            }
            "-h" | "--help" => return Err(usage()),
            other => return Err(format!("unknown argument {other:?}\n{}", usage())),
        }
        index += 1;
    }

    let agent = agent.ok_or_else(|| format!("--agent is required\n{}", usage()))?;
    let project = match project {
        Some(value) => value,
        None => std::env::current_dir()
            .map_err(|error| format!("cannot resolve the project from the cwd: {error}"))?
            .to_string_lossy()
            .into_owned(),
    };

    // `<session>:<pane_index>` — the index is recorded in the emitted row alongside the
    // pane_id it resolved to, because "notified pane 1" is undiagnosable and
    // "notified %1397, resolved from fleet:1" is evidence.
    let pane = match pane_spec {
        None => None,
        Some(spec) => {
            let (session, index_text) = spec.rsplit_once(':').ok_or_else(|| {
                format!("--pane wants <SESSION>:<INDEX>, got {spec:?}\n{}", usage())
            })?;
            let parsed = index_text.parse::<u32>().map_err(|_| {
                format!(
                    "--pane index must be an integer, got {index_text:?}\n{}",
                    usage()
                )
            })?;
            Some((session.to_string(), parsed))
        }
    };
    // Every one of these is a MIS-INVOCATION rather than a defaultable preference, and each
    // is refused with EX_USAGE instead of being silently coerced. The coercion is the
    // hazard: `--timeout 600` quietly ignored on a one-shot means an operator who believes
    // they armed a ten-minute wake actually armed nothing, and the run's exit 0 looks
    // exactly like a wake that fired and found an empty box.
    if !watch {
        if let Some(value) = bound_secs {
            return Err(format!(
                "--timeout {value} needs --watch; a one-shot has nothing to bound\n{}",
                usage()
            ));
        }
        if let Some(value) = interval_secs {
            return Err(format!(
                "--interval {value} needs --watch; a one-shot polls exactly once\n{}",
                usage()
            ));
        }
    }
    if watch && position_now {
        return Err(format!(
            "--watch and --position-now are contradictory: --position-now establishes a \
             baseline and deliberately makes NO mail claim, so a watch built on it could \
             never fire\n{}",
            usage()
        ));
    }

    let watch = if watch {
        let bound = Duration::from_secs(bound_secs.unwrap_or(DEFAULT_WATCH_BOUND.as_secs()));
        let interval =
            Duration::from_secs(interval_secs.unwrap_or(DEFAULT_WATCH_INTERVAL.as_secs()));
        if bound.is_zero() {
            return Err(format!(
                "--timeout must be at least 1 second; a zero bound is a watch that cannot \
                 observe anything and would report a timeout having read nothing\n{}",
                usage()
            ));
        }
        if interval.is_zero() {
            return Err(format!(
                "--interval must be at least 1 second; a zero interval is a spin loop \
                 against a live daemon\n{}",
                usage()
            ));
        }
        Some((bound, interval))
    } else {
        None
    };


    Ok(Args {
        agent,
        project,
        pane,
        position_now,
        watch,
    })
}

/// Parse a whole-second flag value, refusing the shapes that would otherwise become a
/// silent default: an absent value, a non-integer, and a negative.
fn parse_secs(flag: &str, value: Option<&String>) -> Result<u64, String> {
    let text = value.ok_or_else(|| format!("{flag} requires a value\n{}", usage()))?;
    text.parse::<u64>().map_err(|_| {
        format!(
            "{flag} wants a whole number of seconds, got {text:?}\n{}",
            usage()
        )
    })
}

/// The outcome of one bounded read, with the restrictive outcomes already collapsed into a
/// single diagnosable string. This is the ONLY place a `BoundedOutcome` is destructured, so
/// no future edit can add a fourth call site that quietly treats `TimedOut` as empty output.
enum Read {
    Ok(String),
    Failed(String),
}

fn bounded_read(what: &str, command: &mut Command, deadline: Duration) -> Read {
    match bounded_output(command, deadline) {
        BoundedOutcome::Completed(output) if output.status.success() => {
            Read::Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        }
        BoundedOutcome::Completed(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Read::Failed(format!(
                "{what} exited {} — {}",
                output
                    .status
                    .code()
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "by signal".to_string()),
                stderr.trim()
            ))
        }
        BoundedOutcome::TimedOut => Read::Failed(format!(
            "{what} exceeded its {}s deadline and its process group was killed; a timeout is \
             not a verdict",
            deadline.as_secs()
        )),
        BoundedOutcome::Unspawned(error) => {
            Read::Failed(format!("{what} could not be spawned: {error}"))
        }
    }
}

/// What the daemon health probe said. `Ready` is the only value that suppresses `--direct`.
fn probe_daemon_health() -> String {
    let mut command = Command::new("curl");
    command.args([
        "-s",
        "--max-time",
        "3",
        &format!("http://{HEALTH_HOST}:{HEALTH_PORT}/health"),
    ]);
    match bounded_read("curl /health", &mut command, PROBE_DEADLINE) {
        Read::Ok(body) => {
            let compact: String = body.chars().filter(|c| !c.is_whitespace()).collect();
            if compact.contains("\"status\":\"ready\"") {
                "ready".to_string()
            } else if compact.is_empty() {
                "unreachable:empty-body".to_string()
            } else {
                let head: String = compact.chars().take(120).collect();
                format!("not-ready:{head}")
            }
        }
        Read::Failed(detail) => format!("unreachable:{detail}"),
    }
}

/// Resolve the emitting pane at emit time. Returns `(pane_id, resolved_from)`.
fn resolve_emitting_pane(pane: &Option<(String, u32)>) -> (Option<String>, String) {
    let Some((session, index)) = pane else {
        // No --pane and no INBOX_MONITOR_PANE. TMUX_PANE is the pane_id of the pane we are
        // ACTUALLY in, which is strictly better provenance than an index — but it is not an
        // index, so the row says exactly where it came from.
        return match std::env::var("TMUX_PANE") {
            Ok(id) if !id.is_empty() => (Some(id), "env:TMUX_PANE".to_string()),
            _ => (None, "unresolved:no --pane and no TMUX_PANE".to_string()),
        };
    };

    let mut command = Command::new("tmux");
    command.args([
        "list-panes",
        "-a",
        "-F",
        "#{pane_id} #{session_name}:#{window_index}.#{pane_index}",
    ]);
    let resolved_from = format!("{session}:{index}");
    match bounded_read("tmux list-panes", &mut command, PROBE_DEADLINE) {
        Read::Ok(listing) => (resolve_pane(&listing, session, *index), resolved_from),
        Read::Failed(detail) => (None, format!("{resolved_from} (listing failed: {detail})")),
    }
}

fn am_command(project: &str, agent: &str, extra: &[&str]) -> Command {
    let mut command = Command::new("am");
    command.args(extra);
    command.args(["--project", project, "--agent", agent, "--json"]);
    command
}

/// The PRIMARY read: `fetch_inbox` over authenticated MCP.
///
/// # Why this is not a `curl` and not the `am` CLI
///
/// `agent-mail-native` already owns this call, and it owns the one distinction acceptance 3
/// of `omp-orchestrator-monitor-reads-oracle-y256` turns on: `MailError::Unauthorized`
/// (the daemon answered and refused the credential) is a DIFFERENT value from
/// `MailError::Unreachable` (nothing answered). Re-deriving that from a message string is
/// how "no listener on 127.0.0.1:8765" came to be printed about a daemon that was running
/// and answering `/health`.
///
/// `mark_read` is `false`, and that is load-bearing rather than tidy: the daemon's own
/// default is TRUE, so a monitor that omitted it would CONSUME the unread state it exists to
/// report. The bead's recorded "daemon: 0 unread, every row carries a read_ts" is exactly
/// the shape a consuming read leaves behind.
enum DaemonRead {
    Unread(usize),
    Failed {
        detail: String,
        reason: UnreachableReason,
    },
}

/// `&Cx` FIRST, and the runtime is NOT built here.
///
/// It used to be: this function opened its own `RuntimeBuilder::current_thread()` per call.
/// That was survivable for a one-shot and is a nested-runtime hazard the moment anything
/// calls it from inside an existing `block_on` — which is exactly what `--watch` does on
/// every poll. So the runtime is hoisted to `run`, there is now exactly ONE for the whole
/// process, and the caller's cancellation region owns this read.
async fn read_daemon_unread(cx: &Cx, project: &str, agent: &str) -> DaemonRead {
    if cx.checkpoint().is_err() {
        return DaemonRead::Failed {
            detail: "the caller's region was cancelled before the primary read".to_string(),
            reason: UnreachableReason::Indeterminate,
        };
    }
    let client = MailClient::discover().with_request_timeout(AM_DEADLINE);
    let request = InboxRequest {
        project: ProjectKey::new(project.to_owned()),
        agent: AgentName::new(agent.to_owned()),
        include_bodies: false,
        limit: Some(DAEMON_INBOX_LIMIT),
        unread_only: true,
        // NEVER true. See the doc above: the daemon's default consumes read state.
        mark_read: false,
    };
    let outcome: Result<usize, MailError> = fetch_inbox(cx, &client, &request)
        .await
        .map(|messages| messages.len());
    match outcome {
        Ok(count) => DaemonRead::Unread(count),
        Err(MailError::Unauthorized { status }) => DaemonRead::Failed {
            detail: format!(
                "the daemon ANSWERED and refused our credential (HTTP {status}); a 401 is not \
                 an absent listener"
            ),
            reason: UnreachableReason::Unauthorized { status },
        },
        Err(error @ MailError::Unreachable { .. }) => DaemonRead::Failed {
            detail: format!("nothing answered on the MCP endpoint: {error}"),
            reason: UnreachableReason::Absent,
        },
        Err(error) => DaemonRead::Failed {
            detail: format!("the daemon was reached but its answer was unusable: {error}"),
            reason: UnreachableReason::Indeterminate,
        },
    }
}

/// One observation, already reduced to what the row needs.
struct Observation {
    verdict: MonitorVerdict,
    page: Option<EventPage>,
    unread_count: usize,
    direct_read: bool,
    daemon_health: String,
    /// What the PRIMARY authority said, carried so the row can attribute both numbers even
    /// on a run where the two agreed.
    daemon_arm: DaemonArm,
}

/// `&Cx` first: the one async leg inside is the PRIMARY daemon read, and the caller's
/// region owns its cancellation. The two `am` reads and the health probe stay on the
/// deliberately-synchronous `bounded_output` kernel, which `subprocess-contract` documents
/// as "sync on purpose … gate bins run outside any asupersync runtime and must not grow an
/// async ripple to get the five-rule contract". Each of those is bounded by its OWN
/// deadline and kills its process GROUP, so no read here is unbounded whichever kernel
/// carries it.
async fn observe(cx: &Cx, args: &Args, persisted: Option<u64>) -> Observation {
    let daemon_health = probe_daemon_health();

    // THE PRIMARY READ, first. If the designated authority cannot be read the observation is
    // ABSENT — the CLI's answer is the differential oracle and is not promoted to primary
    // just because it happens to reply. That promotion is precisely the defect this bead
    // names: the monitor consuming the oracle while reporting it as the answer.
    let daemon_arm = match read_daemon_unread(cx, &args.project, &args.agent).await {
        DaemonRead::Unread(count) => DaemonArm::Unread(count),
        DaemonRead::Failed { detail, reason } => {
            return Observation {
                verdict: MonitorVerdict::Unreachable { detail, reason },
                page: None,
                unread_count: 0,
                direct_read: daemon_health != "ready",
                daemon_health,
                daemon_arm: DaemonArm::NotConsulted,
            }
        }
    };
    // `--direct` is a fallback, not a default: it bypasses the daemon's own view. It is
    // reached only when the health probe did not say ready, and the row says so.
    let direct_read = daemon_health != "ready";

    let cursor_arg;
    let mut events_args: Vec<&str> = vec!["inbox-events", "--limit", "200"];
    // `--after` and `--position-now` answer different questions: one resumes a stream, the
    // other discards it and reports the tail. Sending both would ask `am` to resolve a
    // contradiction we have not measured, so baseline mode sends only `--position-now`.
    if let (Some(cursor), false) = (persisted, args.position_now) {
        cursor_arg = cursor.to_string();
        events_args.push("--after");
        events_args.push(&cursor_arg);
    }
    if args.position_now {
        events_args.push("--position-now");
    }
    if direct_read {
        // MEASURED: `--direct` exists on `am inbox-events` and NOT on `am inbox`
        // (`am inbox --help`, 2026-09-02), so it is passed to exactly one of the two reads.
        events_args.push("--direct");
    }

    let mut events_command = am_command(&args.project, &args.agent, &events_args);
    let page = match bounded_read("am inbox-events", &mut events_command, AM_DEADLINE) {
        Read::Ok(text) => match parse_event_page(&text) {
            Ok(page) => page,
            Err(error) => {
                return Observation {
                    verdict: MonitorVerdict::Unreachable {
                        detail: error.to_string(),
                        reason: UnreachableReason::Indeterminate,
                    },
                    page: None,
                    unread_count: 0,
                    direct_read,
                    daemon_health,
                    daemon_arm,
                    }
            }
        },
        Read::Failed(detail) => {
            return Observation {
                verdict: MonitorVerdict::Unreachable {
                    detail,
                    reason: UnreachableReason::Indeterminate,
                },
                page: None,
                unread_count: 0,
                direct_read,
                daemon_health,
                daemon_arm,
                }
        }
    };

    // Baseline mode deliberately does not read the mailbox: it establishes a cursor with no
    // notification, so asking about unread state would only invite acting on it.
    if args.position_now {
        return Observation {
            verdict: MonitorVerdict::Clear,
            page: Some(page),
            unread_count: 0,
            direct_read,
            daemon_health,
            daemon_arm,
            };
    }

    let mut inbox_command = am_command(&args.project, &args.agent, &["inbox", "--unread"]);
    let rows: Vec<InboxRow> = match bounded_read("am inbox", &mut inbox_command, AM_DEADLINE) {
        Read::Ok(text) => match parse_inbox_rows(&text) {
            Ok(rows) => rows,
            Err(error) => {
                return Observation {
                    verdict: MonitorVerdict::Unreachable {
                        detail: error.to_string(),
                        reason: UnreachableReason::Indeterminate,
                    },
                    page: Some(page),
                    unread_count: 0,
                    direct_read,
                    daemon_health,
                    daemon_arm,
                    }
            }
        },
        Read::Failed(detail) => {
            return Observation {
                verdict: MonitorVerdict::Unreachable {
                    detail,
                    reason: UnreachableReason::Indeterminate,
                },
                page: Some(page),
                unread_count: 0,
                direct_read,
                daemon_health,
                daemon_arm,
                }
        }
    };

    let unread_count = unread(&rows).len();
    let verdict = classify(&page, &rows, persisted, daemon_arm);
    Observation {
        verdict,
        page: Some(page),
        unread_count,
        direct_read,
        daemon_health,
        daemon_arm,
        }
}

/// A mis-invocation, which is NOT an observation and therefore never wears a verdict code.
struct Usage(String);

/// What the emitted row says about the mode it came from, so ONE row builder serves both
/// the one-shot and the watch.
///
/// A second row builder is how the two modes' artifacts drift until a consumer has to know
/// which mode wrote a line before it can read it.
struct ModeFacts {
    /// `observe` | `position-now` | `watch`.
    mode: &'static str,
    /// The row's `verdict` field. NOT always the observation's own label: a watch that
    /// timed out cleanly must not write `clear` to the ledger.
    verdict_label: &'static str,
    /// The row's `exit_code` field.
    exit_code: u8,
    /// The stderr line for a human.
    human: String,
    /// Whether this terminal may advance the persisted cursor.
    advances_cursor: bool,
    /// Watch-only telemetry. `None` on a one-shot, so the field set of a one-shot row is
    /// byte-identical to what it was before the watch existed.
    watch: Option<WatchFacts>,
}

/// The watch-only half of a row.
struct WatchFacts {
    outcome: &'static str,
    polls: u32,
    waited_secs: u64,
    bound_secs: u64,
    interval_secs: u64,
    trailing_blind: u32,
}

/// Print the JSON row, print the human line, append the ledger, then MAYBE advance the
/// cursor — in that order, always.
///
/// Returns `Some(Unreachable)` when a durability step failed, which the caller must
/// propagate as the terminal: a row we could not make durable is not a verdict we may act
/// on.
fn emit(
    args: &Args,
    observation: &Observation,
    persisted: Option<u64>,
    cursor_file: &Path,
    ledger: &Path,
    facts: &ModeFacts,
) -> Option<MonitorVerdict> {
    let (resolved_pane, resolved_from) = resolve_emitting_pane(&args.pane);
    let now = now_epoch_secs();

    let row = serde_json::json!({
        "verdict": facts.verdict_label,
        "exit_code": facts.exit_code,
        "agent": args.agent,
        "project": args.project,
        "mode": facts.mode,
        "unread": observation.unread_count,
        "persisted_cursor": persisted,
        "tail_cursor": observation.page.as_ref().map(|page| page.tail_cursor),
        "next_cursor": observation.page.as_ref().map(|page| page.next_cursor),
        "oldest_available_cursor": observation.page.as_ref().map(|page| page.oldest_available_cursor),
        "events_in_page": observation.page.as_ref().map(|page| page.events.len()),
        "direct_read": observation.direct_read,
        "daemon_health": observation.daemon_health,
        "resolved_pane": resolved_pane,
        "resolved_pane_from": resolved_from,
        "notified_at": iso8601_utc(now),
        "notified_at_epoch": now,
        // The watch's own telemetry, and it is a SEPARATE object rather than five loose
        // top-level keys so a one-shot row keeps exactly the field set it always had. A
        // reader that has never heard of the watch sees `watch: null` and is unaffected.
        "watch": facts.watch.as_ref().map(|watch| serde_json::json!({
            "outcome": watch.outcome,
            "polls": watch.polls,
            "waited_secs": watch.waited_secs,
            "bound_secs": watch.bound_secs,
            "interval_secs": watch.interval_secs,
            // How many unobservable polls the window ended on. Nonzero beside
            // `outcome: "watch_timed_out"` would be the conflation this crate refuses, so
            // the number is on the row and can be asserted against the outcome.
            "trailing_blind": watch.trailing_blind,
        })),
        // EXHAUSTIVE, not `_ => None`. state-wildcard-lint named all four of these and it is
        // RIGHT: `observation.verdict` is a genuine five-variant state enum, so a catch-all
        // renders a NEWLY ADDED verdict as absent rather than forcing a decision here. That is
        // the same class as ntm coercing state=UNKNOWN into a busy claim — a non-answer wearing
        // a negative one. It already did its job: adding AuthoritiesDisagree failed to compile at
        // every one of these sites, which is exactly what the lint was for.
        "detail": match &observation.verdict {
            MonitorVerdict::Unreachable { detail, .. } => Some(detail.clone()),
            MonitorVerdict::Clear
            | MonitorVerdict::MailWaiting { .. }
            | MonitorVerdict::CursorRegressed { .. }
            | MonitorVerdict::CursorBelowFloor { .. }
            | MonitorVerdict::AuthoritiesDisagree { .. } => None,
        },
        // The typed reason, beside the free text. A consumer branching on WHY the monitor
        // could not observe must not have to substring-match `detail`: a 401 and an absent
        // listener have opposite remedies and were measurably confused for each other.
        "unreachable_reason": match &observation.verdict {
            MonitorVerdict::Unreachable { reason, .. } => Some(reason.label()),
            MonitorVerdict::Clear
            | MonitorVerdict::MailWaiting { .. }
            | MonitorVerdict::CursorRegressed { .. }
            | MonitorVerdict::CursorBelowFloor { .. }
            | MonitorVerdict::AuthoritiesDisagree { .. } => None,
        },
        // BOTH numbers, each attributed. A disagreement row that carries one of them is the
        // defect this verdict exists to remove.
        "daemon_unread": match &observation.verdict {
            MonitorVerdict::AuthoritiesDisagree { daemon_unread, .. } => Some(*daemon_unread),
            _other => match observation.daemon_arm {
                DaemonArm::Unread(count) => Some(count),
                DaemonArm::NotConsulted => None,
            },
        },
        "cli_unread": match &observation.verdict {
            MonitorVerdict::AuthoritiesDisagree { cli_unread, .. } => Some(*cli_unread),
            MonitorVerdict::Clear
            | MonitorVerdict::MailWaiting { .. }
            | MonitorVerdict::Unreachable { .. }
            | MonitorVerdict::CursorRegressed { .. }
            | MonitorVerdict::CursorBelowFloor { .. } => Some(observation.unread_count),
        },
        "primary_authority": "daemon:mcp/fetch_inbox",
        "oracle_authority": "cli:am inbox --unread",
        "oldest_from": match &observation.verdict {
            MonitorVerdict::MailWaiting { oldest_from, .. } => Some(oldest_from.clone()),
            MonitorVerdict::Clear
            | MonitorVerdict::Unreachable { .. }
            | MonitorVerdict::CursorRegressed { .. }
            | MonitorVerdict::CursorBelowFloor { .. }
            | MonitorVerdict::AuthoritiesDisagree { .. } => None,
        },
        "oldest_subject": match &observation.verdict {
            MonitorVerdict::MailWaiting { oldest_subject, .. } => Some(oldest_subject.clone()),
            MonitorVerdict::Clear
            | MonitorVerdict::Unreachable { .. }
            | MonitorVerdict::CursorRegressed { .. }
            | MonitorVerdict::CursorBelowFloor { .. }
            | MonitorVerdict::AuthoritiesDisagree { .. } => None,
        },
        // The floor the verdict was decided against, carried on the VERDICT and not only on
        // the page. A row that says "below floor" without both numbers is undiagnosable, and
        // `oldest_available_cursor` above is a property of the page — which a future edit
        // could stop emitting, or which is null on an Unreachable run. This field is the one
        // that cannot go absent while the verdict says the floor was breached.
        "oldest_available": match &observation.verdict {
            MonitorVerdict::AuthoritiesDisagree { .. } => None,
            MonitorVerdict::CursorBelowFloor { oldest_available, .. } => Some(*oldest_available),
            MonitorVerdict::Clear
            | MonitorVerdict::MailWaiting { .. }
            | MonitorVerdict::Unreachable { .. }
            | MonitorVerdict::CursorRegressed { .. } => None,
        },
    });
    let line = row.to_string();
    println!("{line}");

    // The human-readable line goes to stderr so stdout stays exactly one JSON row.
    let mut human = facts.human.clone();
    if observation.direct_read {
        human.push_str(
            " [DIRECT SQLITE READ — the daemon health probe did not say ready, so this \
             observation bypassed the daemon]",
        );
    }
    match &resolved_pane {
        Some(pane) => human.push_str(&format!(" [pane {pane}, resolved from {resolved_from}]")),
        None => human.push_str(&format!(" [pane UNRESOLVED, attempted {resolved_from}]")),
    }
    eprintln!("{human}");

    // APPEND FIRST. The cursor may only move after the row is durable.
    if let Err(error) = append_ledger(ledger, &line) {
        return Some(MonitorVerdict::Unreachable {
            detail: format!("ledger append failed, cursor NOT advanced: {error}"),
            reason: UnreachableReason::Indeterminate,
        });
    }

    // Only now. A crash between the read and the append replays; the forbidden order skips.
    //
    // The gate is a POSITIVE, exhaustive predicate on the terminal —
    // `MonitorVerdict::advances_cursor()` for a one-shot, `WatchOutcome::advances_cursor()`
    // for a watch — and NOT the `!matches!(.., Unreachable { .. })` that used to stand here.
    // A negative test opts every future variant IN by default, and that default is
    // self-defeating for the two cursor faults: a run that detects an unprovable position
    // and then persists a new one has repaired the SYMPTOM and kept the blindness. The
    // operator would see one loud exit, the cursor would silently re-base to whatever
    // position the server clamped to, and the next run would report Clear with the evidence
    // of the skip gone.
    //
    // TWO gates, and both are positive: the TERMINAL must be one that may advance, AND the
    // page must actually PROVE a position. The second is not redundant — see
    // `EventPage::provable_position`, which carries the measurement: an empty feed answers
    // `next_cursor: 0`, and persisting that zero is what turned a fresh recipient's first
    // message into CURSOR BELOW FLOOR instead of MAIL WAITING.
    if let Some(page) = &observation.page {
        if facts.advances_cursor {
            if let Some(position) = page.provable_position() {
                if let Err(error) = write_cursor_atomic(cursor_file, position) {
                    return Some(MonitorVerdict::Unreachable {
                        detail: format!(
                            "cursor write failed after a durable ledger append: {error}"
                        ),
                        reason: UnreachableReason::Indeterminate,
                    });
                }
            }
        }
    }

    None
}

/// A terminal reached before any row could be emitted. Prints the human line HERE, so
/// `main` never has to and no line is printed twice.
fn fail(verdict: MonitorVerdict) -> Terminal {
    eprintln!("{}", verdict.human_line());
    Terminal::OneShot(verdict)
}

/// The whole program's terminal, as ONE value.
///
/// Both modes converge here so that [`Terminal::exit_code`] is the single place a run
/// becomes an integer. A second conversion site is how a future edit adds a path that
/// silently exits 0.
enum Terminal {
    /// One observation.
    OneShot(MonitorVerdict),
    /// A bounded wake.
    Watch(WatchOutcome),
}

impl Terminal {
    /// THE ONLY terminal-to-code conversion in this binary.
    fn exit_code(&self) -> u8 {
        match self {
            Terminal::OneShot(verdict) => verdict.exit_code(),
            Terminal::Watch(outcome) => outcome.exit_code(),
        }
    }
}

/// One observation, emitted, and reduced to a terminal.
async fn one_shot(
    cx: &Cx,
    args: &Args,
    persisted: Option<u64>,
    cursor_file: &Path,
    ledger: &Path,
) -> Terminal {
    let observation = observe(cx, args, persisted).await;
    let facts = ModeFacts {
        mode: if args.position_now {
            "position-now"
        } else {
            "observe"
        },
        verdict_label: observation.verdict.label(),
        exit_code: observation.verdict.exit_code(),
        human: observation.verdict.human_line(),
        advances_cursor: observation.verdict.advances_cursor(),
        watch: None,
    };
    match emit(args, &observation, persisted, cursor_file, ledger, &facts) {
        Some(durability_failure) => Terminal::OneShot(durability_failure),
        None => Terminal::OneShot(observation.verdict),
    }
}

/// BLOCK until an arrival, a loud fault, or the bound elapses. THE WAKE.
///
/// # Why this polls, measured, rather than consuming an existing wake kernel
///
/// `agent-mail-native` already ships one blocking wake — `wake::wait_for_mail`, which drives
/// `ntm --robot-wait --wait-until=mail_pending` — and per `kernel_only_policy` a poll loop
/// beside an owned kernel would be exactly the handroll that keeps kernels broken. So it was
/// measured first, on this machine, on 2026-09-03:
///
/// ```text
/// ntm --robot-wait=omp-orchestrator --wait-until=mail_pending --timeout=45s   (armed first)
/// am mail send --to PlumTiger ...                                            (t + 4s, id 41429)
/// -> {"success":false,"error_code":"TIMEOUT","waited_seconds":46.05,"agents":[],
///     "cursor_info":{"observed_cursor":325041,"next_cursor":325045,"oldest_cursor":324968}}
/// ```
///
/// The mail landed, `inbox-monitor` saw it one second later as MAIL WAITING, and the wake
/// **did not fire** — it ran its ceiling out and reported `agents: []`. That kernel answers
/// "does an NTM PANE have mail pending", which needs a pane-to-agent binding this session
/// does not carry; it does not answer "has an Agent Mail message arrived for agent X". The
/// capability is therefore NOT one we own, and the poll below is not routing around it.
///
/// The NO-CLAIM: this measurement says the wake did not fire for THIS session and THIS
/// recipient in a 46 s window. It does not say `mail_pending` is broken for panes that do
/// carry the binding, and if that binding is later established this loop should be replaced
/// by the kernel rather than kept.
///
/// # The contract of the loop itself
///
/// `&Cx` first, one `cx.checkpoint()` per tick before the read and again before the sleep,
/// the sleep is `asupersync::time::sleep` inside the caller's region rather than a blocking
/// `thread::sleep`, and no task is detached. Every poll's subprocess work goes through the
/// same bounded, process-group-owning kernel the one-shot uses.
async fn watch(
    cx: &Cx,
    args: &Args,
    bound: Duration,
    interval: Duration,
    persisted: Option<u64>,
    cursor_file: &Path,
    ledger: &Path,
) -> Terminal {
    let started = Instant::now();
    let mut polls: u32 = 0;
    let mut consecutive_blind: u32 = 0;

    loop {
        if cx.checkpoint().is_err() {
            // Cancelled by our own region — a signal, a parent deadline. NOT a timeout and
            // NOT an empty inbox: we stopped looking before the question was answered.
            let verdict = MonitorVerdict::Unreachable {
                detail: format!(
                    "the watch region was cancelled after {} poll(s) and {}s; no arrival was \
                     observed and none was ruled out",
                    polls,
                    started.elapsed().as_secs()
                ),
                reason: UnreachableReason::Indeterminate,
            };
            return fail(verdict);
        }

        let observation = observe(cx, args, persisted).await;
        polls = polls.saturating_add(1);
        let step = watch_step(&observation.verdict, consecutive_blind, WATCH_BLIND_STREAK);
        let waited_secs = started.elapsed().as_secs();

        let outcome = match step {
            WatchStep::Stop => Some(WatchOutcome::Settled {
                verdict: observation.verdict.clone(),
                polls,
                waited_secs,
            }),
            WatchStep::Wait {
                consecutive_blind: streak,
            } => {
                consecutive_blind = streak;
                // The bound is checked AFTER the poll, so a watch always reads at least
                // once: a bound shorter than one read must report what it saw, not a
                // timeout it never looked through.
                if started.elapsed() >= bound {
                    Some(if consecutive_blind > 0 {
                        WatchOutcome::TimedOutBlind {
                            verdict: observation.verdict.clone(),
                            polls,
                            waited_secs,
                            bound_secs: bound.as_secs(),
                            trailing_blind: consecutive_blind,
                        }
                    } else {
                        WatchOutcome::TimedOut {
                            polls,
                            waited_secs,
                            bound_secs: bound.as_secs(),
                        }
                    })
                } else {
                    None
                }
            }
        };

        if let Some(outcome) = outcome {
            let facts = ModeFacts {
                mode: "watch",
                verdict_label: outcome.verdict_label(),
                exit_code: outcome.exit_code(),
                human: outcome.human_line(),
                advances_cursor: outcome.advances_cursor(),
                watch: Some(WatchFacts {
                    outcome: outcome.label(),
                    polls,
                    waited_secs,
                    bound_secs: bound.as_secs(),
                    interval_secs: interval.as_secs(),
                    trailing_blind: consecutive_blind,
                }),
            };
            return match emit(args, &observation, persisted, cursor_file, ledger, &facts) {
                Some(durability_failure) => Terminal::OneShot(durability_failure),
                None => Terminal::Watch(outcome),
            };
        }

        if cx.checkpoint().is_err() {
            let verdict = MonitorVerdict::Unreachable {
                detail: format!(
                    "the watch region was cancelled between polls after {polls} poll(s)"
                ),
                reason: UnreachableReason::Indeterminate,
            };
            return fail(verdict);
        }

        // ONE heartbeat line per non-terminal poll, on stderr, and NOT in the ledger.
        //
        // Measured need: a `--watch` that ran 21 polls over 45 s and reported a clean
        // timeout wrote exactly ONE row — the terminal one — which said `unread: 0` and left
        // no way to tell a mailbox that was quiet the whole time from one whose message was
        // seen-then-lost mid-window. That is a wake whose most important failure mode is
        // undiagnosable from its own artifact.
        //
        // Stderr rather than the ledger, because the ledger's contract is one durable row
        // per NOTIFICATION and 60 heartbeat rows per five-minute window would bury the rows
        // that matter. Stdout stays exactly one JSON row, unchanged.
        eprintln!(
            "inbox-monitor: watch poll {polls} at {}s/{}s — {} (daemon_unread={}, cli_unread={}, \
             blind_streak={consecutive_blind})",
            started.elapsed().as_secs(),
            bound.as_secs(),
            observation.verdict.label(),
            match observation.daemon_arm {
                DaemonArm::Unread(count) => count.to_string(),
                DaemonArm::NotConsulted => "not-consulted".to_string(),
            },
            observation.unread_count,
        );
        // Never sleep past the bound: a watch that overshoots its own ceiling has taken a
        // latency budget it was not given.
        let remaining = bound.saturating_sub(started.elapsed());
        sleep(cx.now_for_observability(), interval.min(remaining)).await;
    }
}

/// The whole program, reduced to the one thing the caller cares about: a terminal.
///
/// The asupersync runtime is built HERE, exactly once, and `&Cx` flows down from it. It used
/// to be built inside the primary daemon read, which is a nested-runtime hazard the moment a
/// caller wraps that read in a loop — and `--watch` is that caller.
fn run(argv: &[String]) -> Result<Terminal, Usage> {
    let args = parse_args(argv).map_err(Usage)?;

    let home = match home_dir() {
        Ok(home) => home,
        Err(error) => {
            return Ok(fail(MonitorVerdict::Unreachable {
                detail: error.to_string(),
                reason: UnreachableReason::Indeterminate,
            }))
        }
    };
    let cursor_file =
        cursor_path_in(&home, &args.agent).map_err(|error| Usage(error.to_string()))?;
    let ledger = ledger_path_in(&home);

    let persisted = match read_cursor(&cursor_file) {
        Ok(value) => value,
        Err(error) => {
            // A corrupt cursor is UNREACHABLE, not clear: we cannot say what has been seen.
            return Ok(fail(MonitorVerdict::Unreachable {
                detail: format!("cursor unreadable: {error}"),
                reason: UnreachableReason::Indeterminate,
            }));
        }
    };

    let runtime = match RuntimeBuilder::current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            return Ok(fail(MonitorVerdict::Unreachable {
                detail: format!("asupersync runtime could not be built: {error}"),
                reason: UnreachableReason::Indeterminate,
            }))
        }
    };

    Ok(runtime.block_on(async move {
        let cx = Cx::current().expect("runtime context inside block_on");
        match args.watch {
            None => one_shot(&cx, &args, persisted, &cursor_file, &ledger).await,
            Some((bound, interval)) => {
                watch(
                    &cx,
                    &args,
                    bound,
                    interval,
                    persisted,
                    &cursor_file,
                    &ledger,
                )
                .await
            }
        }
    }))
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match run(&argv) {
        // Every human line was already printed by `emit` or `fail`, exactly once.
        Ok(terminal) => ExitCode::from(terminal.exit_code()),
        Err(Usage(message)) => {
            eprintln!("inbox-monitor: {message}");
            // EX_USAGE (XC-064). Deliberately not one of this crate's verdict codes: a
            // mis-invocation is not an observation, and must not be readable as one.
            ExitCode::from(64)
        }
    }
}

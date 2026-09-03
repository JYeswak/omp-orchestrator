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
use inbox_monitor::{
    append_ledger, classify, cursor_path_in, home_dir, iso8601_utc, ledger_path_in, now_epoch_secs,
    parse_event_page, parse_inbox_rows, read_cursor, resolve_pane, unread, write_cursor_atomic,
    DaemonArm, EventPage, InboxRow, MonitorVerdict, UnreachableReason,
};
use std::process::{Command, ExitCode};
use std::time::Duration;
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

fn usage() -> String {
    "usage: inbox-monitor --agent <NAME> [--project <PATH>] [--pane <SESSION>:<INDEX>] \
     [--position-now]"
        .to_string()
}

struct Args {
    agent: String,
    project: String,
    pane: Option<(String, u32)>,
    position_now: bool,
}

fn parse_args(argv: &[String]) -> Result<Args, String> {
    let mut agent = std::env::var("AGENT_MAIL_AGENT")
        .ok()
        .or_else(|| std::env::var("AGENT_NAME").ok());
    let mut project = std::env::var("AGENT_MAIL_PROJECT").ok();
    let mut pane_spec = std::env::var("INBOX_MONITOR_PANE").ok();
    let mut position_now = false;

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

    Ok(Args {
        agent,
        project,
        pane,
        position_now,
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

fn read_daemon_unread(project: &str, agent: &str) -> DaemonRead {
    let runtime = match RuntimeBuilder::current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            return DaemonRead::Failed {
                detail: format!("asupersync runtime could not be built: {error}"),
                reason: UnreachableReason::Indeterminate,
            }
        }
    };
    let project = project.to_owned();
    let agent = agent.to_owned();
    let outcome: Result<usize, MailError> = runtime.block_on(async move {
        let cx = Cx::current().expect("runtime context inside block_on");
        let client = MailClient::discover().with_request_timeout(AM_DEADLINE);
        let request = InboxRequest {
            project: ProjectKey::new(project),
            agent: AgentName::new(agent),
            include_bodies: false,
            limit: Some(DAEMON_INBOX_LIMIT),
            unread_only: true,
            // NEVER true. See the doc above: the daemon's default consumes read state.
            mark_read: false,
        };
        let messages = fetch_inbox(&cx, &client, &request).await?;
        Ok(messages.len())
    });
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

fn observe(args: &Args, persisted: Option<u64>) -> Observation {
    let daemon_health = probe_daemon_health();

    // THE PRIMARY READ, first. If the designated authority cannot be read the observation is
    // ABSENT — the CLI's answer is the differential oracle and is not promoted to primary
    // just because it happens to reply. That promotion is precisely the defect this bead
    // names: the monitor consuming the oracle while reporting it as the answer.
    let daemon_arm = match read_daemon_unread(&args.project, &args.agent) {
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

/// The whole program, reduced to the one thing the caller cares about: a verdict.
///
/// Every failure path inside returns a `MonitorVerdict` rather than exiting, so there is
/// exactly ONE place in this binary where a verdict becomes an exit code. A second
/// conversion site is how a future edit adds a path that silently exits 0.
fn run(argv: &[String]) -> Result<MonitorVerdict, Usage> {
    let args = parse_args(argv).map_err(Usage)?;

    let home = match home_dir() {
        Ok(home) => home,
        Err(error) => {
            return Ok(MonitorVerdict::Unreachable {
                detail: error.to_string(),
                reason: UnreachableReason::Indeterminate,
            })
        }
    };
    let cursor_file =
        cursor_path_in(&home, &args.agent).map_err(|error| Usage(error.to_string()))?;
    let ledger = ledger_path_in(&home);

    let persisted = match read_cursor(&cursor_file) {
        Ok(value) => value,
        Err(error) => {
            // A corrupt cursor is UNREACHABLE, not clear: we cannot say what has been seen.
            return Ok(MonitorVerdict::Unreachable {
                detail: format!("cursor unreadable: {error}"),
                reason: UnreachableReason::Indeterminate,
            });
        }
    };

    let observation = observe(&args, persisted);
    let (resolved_pane, resolved_from) = resolve_emitting_pane(&args.pane);
    let now = now_epoch_secs();

    let row = serde_json::json!({
        "verdict": observation.verdict.label(),
        "exit_code": observation.verdict.exit_code(),
        "agent": args.agent,
        "project": args.project,
        "mode": if args.position_now { "position-now" } else { "observe" },
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
    let mut human = observation.verdict.human_line();
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
    if let Err(error) = append_ledger(&ledger, &line) {
        return Ok(MonitorVerdict::Unreachable {
            detail: format!("ledger append failed, cursor NOT advanced: {error}"),
            reason: UnreachableReason::Indeterminate,
        });
    }

    // Only now. A crash between the read and the append replays; the forbidden order skips.
    //
    // The gate is `MonitorVerdict::advances_cursor()` — a POSITIVE, exhaustive predicate on
    // the verdict — and NOT the `!matches!(.., Unreachable { .. })` that used to stand here.
    // A negative test opts every future variant IN by default, and that default is
    // self-defeating for the two cursor faults: a run that detects an unprovable position
    // and then persists a new one has repaired the SYMPTOM and kept the blindness. The
    // operator would see one loud exit, the cursor would silently re-base to whatever
    // position the server clamped to, and the next run would report Clear with the evidence
    // of the skip gone. So Clear and MailWaiting advance; Unreachable, CursorRegressed and
    // CursorBelowFloor abstain, and the state file stays as the operator's repair target.
    if let Some(page) = &observation.page {
        if observation.verdict.advances_cursor() {
            if let Err(error) = write_cursor_atomic(&cursor_file, page.next_cursor) {
                return Ok(MonitorVerdict::Unreachable {
                    detail: format!("cursor write failed after a durable ledger append: {error}"),
                    reason: UnreachableReason::Indeterminate,
                });
            }
        }
    }

    Ok(observation.verdict)
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match run(&argv) {
        Ok(verdict) => {
            if let MonitorVerdict::Unreachable { .. } = &verdict {
                eprintln!("{}", verdict.human_line());
            }
            // THE ONLY verdict-to-code conversion in this binary.
            ExitCode::from(verdict.exit_code())
        }
        Err(Usage(message)) => {
            eprintln!("inbox-monitor: {message}");
            // EX_USAGE (XC-064). Deliberately not one of this crate's verdict codes: a
            // mis-invocation is not an observation, and must not be readable as one.
            ExitCode::from(64)
        }
    }
}

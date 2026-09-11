#![forbid(unsafe_code)]

//! `ompo` — the umbrella CLI.
//!
//! Contract: `docs/contracts/umbrella_adapter_dispatch.md`. Bead `omp-orchestrator-jplf.7.2`.
//!
//! WHAT CHANGED AND WHY. Before this, `main` was `if command != Some("doctor") { usage; exit 2 }`
//! — ONE verb, no adapter addressing, no `init`, no capabilities enumeration. Measured
//! 2026-09-07: 85 workspace bin targets, 0 reachable through any umbrella verb, and the `init`
//! MECHANISM fully built at `ompo-start/src/inception.rs:556` with no command surface — which
//! is why `%7` returned CHANGES REQUESTED on both L2 observability beads for the single reason
//! "no `ompo init` command". The mechanism was wired; the operator could not reach it.
//! That is BUILT != WIRED at the COMMAND surface.
//!
//! EXIT CODES, and they are deliberately distinguishable:
//!   0  the verb succeeded
//!   1  the verb ran and the subject failed
//!   2  the INVOCATION was wrong — unknown verb, unknown adapter, missing argument
//!   3  the instrument could not run (roster unavailable, encode failure)
//!   4  an adapter could not be MEASURED — absent from `PATH`, or it exceeded the deadline.
//!      Distinct from `3` because "the instrument broke" and "the subject is unreachable"
//!      have different remedies; `adapter_exec::EXIT_UNMEASURABLE` owns that value.
//! `2` alone cannot separate an unknown verb from an unknown adapter, so every refusal below
//! names what it rejected. AGENTS.md gate rule 7: an exit code is not a message.

use ompo_doctor::adapter_exec;
use ompo_doctor::liveness::{self, Observation};
use ompo_doctor::omp_messages;
use ompo_doctor::omp_process;
use ompo_doctor::omp_state;
use ompo_doctor::omp_stats;
use ompo_doctor::provenance;
use ompo_doctor::state_triad;
use ompo_doctor::umbrella::{self, ProbeId};
use ompo_doctor::upstream_report;
use ompo_doctor::{current_repo, run_doctor, DoctorError};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::ExitCode;

/// Invocation error: the caller addressed something that does not exist.
const EXIT_BAD_INVOCATION: u8 = 2;
/// Instrument error: the umbrella itself could not answer.
const EXIT_INSTRUMENT: u8 = 3;

/// `--json` read from the WHOLE argv rather than from a positional scan.
///
/// The adapter axis can be addressed positionally (`ompo doctor pane-truth --json`) or by
/// flag (`ompo doctor --json --adapter pane-truth`), and a left-to-right parser reaches the
/// adapter before it has seen a trailing `--json`. Scanning the argv makes both orders
/// behave identically instead of one of them silently printing the human report.
fn wants_json(rest: &[String]) -> bool {
    rest.iter().any(|arg| arg == "--json")
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(verb) = args.first().map(String::as_str) else {
        eprintln!("{}", umbrella::usage());
        return ExitCode::from(EXIT_BAD_INVOCATION);
    };
    let rest = &args[1..];
    if let Some(code) = ompo_doctor::health_repair::dispatch(verb, rest) {
        return ExitCode::from(code);
    }
    if let Some(code) = ompo_doctor::selfdoc::dispatch(verb, rest) {
        return ExitCode::from(code);
    }

    match verb {
        "--help" | "-h" | "help" if rest.is_empty() => {
            println!("{}", umbrella::usage());
            ExitCode::SUCCESS
        }
        "help" => run_help(rest),
        "capabilities" => run_capabilities(rest),
        "parity" => run_parity(rest),
        "init" => run_init(rest),
        "start" => run_start(rest),
        "supervise" => run_supervise(rest),
        "portal" => run_portal(rest),
        "validate" => run_validate(rest),
        "audit" => run_audit(rest),
        "why" => run_why(rest),
        "upstream-report" => run_upstream_report(rest),
        "state" => run_state(rest),
        "stats" => run_stats(rest),
        "messages" => run_messages(rest),
        "ps" => run_ps(rest),
        "doctor" => run_doctor_verb(rest),
        other => {
            // NAMES the rejected verb. A bare usage dump leaves the caller unable to tell a
            // typo from an unimplemented verb.
            eprintln!(
                "ompo: UAD_UNKNOWN_VERB verb={other:?} reason=absent_from_verb_set verbs={:?}",
                umbrella::VERBS
            );
            eprintln!("{}", umbrella::usage());
            ExitCode::from(EXIT_BAD_INVOCATION)
        }
    }
}

fn liveness_json(observation: &Observation) -> Value {
    let sources = observation
        .verdict
        .sources()
        .iter()
        .map(|source| (source.name.clone(), ompo_start::liveness::source_json(source)))
        .collect::<serde_json::Map<String, Value>>();
    json!({
        "status": observation.verdict.status(),
        "reason_code": observation.verdict.reason_code(),
        "sources": sources,
        "all_fresh": ompo_start::liveness::all_fresh(observation.verdict.sources()),
        "pane_set_agreement": ompo_start::liveness::pane_set_agreement(observation.verdict.sources()),
    })
}

/// L3 monitor (vyzr): TUI/JSON ordered-ID parity plus the HD-0009 halt state,
/// computed from the same post-predicate steps the report renders. No second
/// schema: plain JSON values assembled beside the existing report fields. The
/// halt is engaged exactly when the L3-HD0009 row sits Blocked.
fn observability_json(steps: &[ompo_start::Step]) -> Value {
    let tui_ids: Vec<_> = ompo_start::tui_ordered_ids(steps);
    let json_ids: Vec<_> = ompo_start::json_ordered_ids(steps);
    // The boolean is the gate verdict, not a reimplementation: the typed
    // TuiOnly/Empty refusal lives in `check_id_parity`, asserted by its own legs.
    let parity_ok = ompo_start::check_id_parity(&tui_ids, &json_ids).is_ok();
    let hd0009 = steps.iter().find(|step| step.id == "L3-HD0009");
    let halt = hd0009.map(|step| {
        json!({
            "engaged": step.status == ompo_start::StepStatus::Blocked,
            "step": step.id,
            "reason_code": step.reason_code,
        })
    });
    // L3-OBS-HD0009 (n5tt): the variant name verbatim (`Blocked`), the token
    // the contract names. `step_json` uppercases for the row render; this field
    // is read against that contract, so it must not inherit that casing.
    let hd0009_status = hd0009.map(|step| format!("{:?}", step.status));
    json!({
        "parity_ok": parity_ok,
        // L3-OBS-COUNT (st8w): the array length, so a renderer that filters
        // rows shows up here as a count mismatch instead of a silent drop.
        "step_count": steps.len(),
        // L3-OBS-CURSOR (jb5m): `next_step`'s row, so the TUI cursor and this
        // field cannot disagree without `next_step` itself changing.
        "next_step_id": ompo_start::next_step(steps).map(|step| step.id),
        "hd0009_status": hd0009_status,
        "tui_ids": tui_ids,
        "json_ids": json_ids,
        "halt": halt,
    })
}

fn step_json(step: &ompo_start::Step) -> Value {
    json!({
        "id": step.id,
        "title": step.title,
        "status": format!("{:?}", step.status).to_ascii_uppercase(),
        "reason_code": step.reason_code,
        "next_command": step.next_command,
        "predicate": format!("{:?}", step.predicate),
    })
}

/// ompo supervise is the canonical resident observe -> dispatch -> receipt entrypoint.
/// The runtime lives in omp-orchestrator; this frontend only forwards the parsed tail.
fn run_supervise(rest: &[String]) -> ExitCode {
    omp_orchestrator::resident::run(rest.to_vec())
}
/// L3 observability writer (hilk): every successful `ompo start` run emits one
/// S1.L2 -> S1.L3 row at the step-advance chokepoint below. Observability never
/// fails the command: an emit error is reported on stderr and execution continues.
/// Shape copied from tick-monitor's `emit_l4`, pointed at the L3 transition.
fn emit_s1_l3_start(repo: &std::path::Path) {
    let code = match lifecycle_event::ReasonCode::new("START_OK") {
        Ok(code) => code,
        Err(error) => {
            eprintln!("LIFECYCLE_EVENT_EMIT_FAILED layer=L3 detail={error}");
            return;
        }
    };
    let event = lifecycle_event::LifecycleEvent::new(
        lifecycle_event::Layer::L3,
        "S1.L2",
        "S1.L3",
        "ompo",
        lifecycle_event::EmitOutcome::Emitted,
        code,
    );
    let path = lifecycle_event::default_repo_journal(repo);
    match lifecycle_event::DurableJournal::open(&path)
        .and_then(|journal| lifecycle_event::emit_one_host(&journal, event))
    {
        Ok(_) => {}
        Err(error) => eprintln!("LIFECYCLE_EVENT_EMIT_FAILED layer=L3 detail={error}"),
    }
}

/// L4 observability writer (tt62): records the live-verdict / spawn terminal of
/// `ompo start` as one S1.L3 -> S1.L4 row beside the S1.L3 start row. `spawn` is
/// the terminal object the spawn chain above computed; every arm sets `status`,
/// so an unrecognized value is a bug and refuses loudly rather than emitting fiction.
/// Shape copied from `emit_s1_l3_start`, pointed at the L4 transition.
fn emit_s1_l4_verdict(repo: &std::path::Path, spawn: &serde_json::Value) {
    let status = spawn
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("UNKNOWN");
    let exit_code = spawn
        .get("exit_code")
        .and_then(serde_json::Value::as_i64)
        .and_then(|code| i32::try_from(code).ok());
    let reason = match (status, exit_code) {
        ("NOT_REQUESTED", _) => "SPAWN_NOT_REQUESTED",
        ("NOT_NEEDED", _) => "LIVE",
        ("EXECUTED", _) => "SPAWN_EXECUTED",
        ("FAILED", _) => "SPAWN_FAILED",
        _ => {
            eprintln!(
                "LIFECYCLE_EVENT_EMIT_FAILED layer=L4 detail=unknown start terminal status={status:?}"
            );
            return;
        }
    };
    let code = match lifecycle_event::ReasonCode::new(reason) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("LIFECYCLE_EVENT_EMIT_FAILED layer=L4 detail={error}");
            return;
        }
    };
    let event = lifecycle_event::LifecycleEvent::new(
        lifecycle_event::Layer::L4,
        "S1.L3",
        "S1.L4",
        "ompo",
        lifecycle_event::EmitOutcome::Emitted,
        code,
    );
    let path = lifecycle_event::default_repo_journal(repo);
    match lifecycle_event::DurableJournal::open(&path)
        .and_then(|journal| lifecycle_event::emit_one_host(&journal, event))
    {
        Ok(_) => {}
        Err(error) => eprintln!("LIFECYCLE_EVENT_EMIT_FAILED layer=L4 detail={error}"),
    }
}

fn run_start(rest: &[String]) -> ExitCode {
    let mut repo = match current_repo() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("ompo start: {error}");
            return ExitCode::from(EXIT_INSTRUMENT);
        }
    };
    let mut session = "omp-orchestrator".to_owned();
    let mut json_output = false;
    let mut persona_a = false;
    let mut hd0010_decided = false;
    let mut spawn_requested = false;
    let mut agents: Vec<(String, String)> = Vec::new();
    let mut index = 0;
    while index < rest.len() {
        match rest[index].as_str() {
            "--json" => json_output = true,
            "--persona-a" => persona_a = true,
            "--hd-0010-decided" => hd0010_decided = true,
            "--spawn" => spawn_requested = true,
            "--repo" => {
                index += 1;
                let Some(value) = rest.get(index) else {
                    eprintln!("ompo start: UAD_MISSING_VALUE flag=--repo");
                    return ExitCode::from(EXIT_BAD_INVOCATION);
                };
                repo = PathBuf::from(value);
            }
            "--session" => {
                index += 1;
                let Some(value) = rest.get(index) else {
                    eprintln!("ompo start: UAD_MISSING_VALUE flag=--session");
                    return ExitCode::from(EXIT_BAD_INVOCATION);
                };
                session = value.clone();
            }
            "--cc" | "--cod" => {
                let flag = rest[index].clone();
                index += 1;
                let Some(value) = rest.get(index) else {
                    eprintln!("ompo start: UAD_MISSING_VALUE flag={flag}");
                    return ExitCode::from(EXIT_BAD_INVOCATION);
                };
                agents.push((flag, value.clone()));
            }
            value if value.starts_with("--cc=") || value.starts_with("--cod=") => {
                let (flag, value) = value.split_once('=').expect("equals checked");
                if value.is_empty() {
                    eprintln!("ompo start: UAD_MISSING_VALUE flag={flag}");
                    return ExitCode::from(EXIT_BAD_INVOCATION);
                }
                agents.push((flag.to_owned(), value.to_owned()));
            }
            "--help" | "-h" => {
                println!("{}", umbrella::usage());
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("ompo start: UAD_UNKNOWN_ARGUMENT argument={other:?}");
                return ExitCode::from(EXIT_BAD_INVOCATION);
            }
        }
        index += 1;
    }

    let observation = liveness::observe(&session);
    let live = observation.verdict.is_live();
    let mut steps = ompo_start::fixture_steps();
    ompo_start::apply_predicates(&mut steps, live, persona_a, hd0010_decided);

    let spawn = if !spawn_requested {
        json!({"status": "NOT_REQUESTED"})
    } else if persona_a {
        eprintln!("ompo start: L4_SPAWN_REFUSED reason=PERSONA_A_NO_SPAWN");
        return ExitCode::from(EXIT_BAD_INVOCATION);
    } else if live {
        json!({"status": "NOT_NEEDED", "reason_code": "L4_LIVE"})
    } else if !hd0010_decided {
        eprintln!("ompo start: L4_SPAWN_REFUSED reason=HD-0010_UNDECIDED");
        return ExitCode::from(EXIT_BAD_INVOCATION);
    } else {
        match liveness::spawn(&session, &repo, &agents) {
            Ok(report) => json!({
                "status": if report.exit_code == Some(0) { "EXECUTED" } else { "FAILED" },
                "command": report.command,
                "exit_code": report.exit_code,
                "stdout": report.stdout,
                "stderr": report.stderr,
            }),
            Err(error) => {
                eprintln!("ompo start: {error}");
                return ExitCode::from(1);
            }
        }
    };

    emit_s1_l3_start(&repo);
    emit_s1_l4_verdict(&repo, &spawn);
    let data = json!({
        "repo": repo.display().to_string(),
        "session": session,
        "step_count": steps.len(),
        "ordered_ids": ompo_start::json_ordered_ids(&steps),
        "next_step_id": ompo_start::next_step(&steps).map(|step| step.id),
        "next_command": ompo_start::json_next_command(&steps),
        "steps": steps.iter().map(step_json).collect::<Vec<_>>(),
        "liveness": liveness_json(&observation),
        "spawn": spawn,
        "observability": observability_json(&steps),
    });
    if json_output {
        match serde_json::to_string(&umbrella::envelope("start", "OK", data)) {
            Ok(text) => println!("{text}"),
            Err(error) => {
                eprintln!("ompo start: cannot encode: {error}");
                return ExitCode::from(EXIT_INSTRUMENT);
            }
        }
    } else {
        println!(
            "OMPO_START session={} steps={} liveness={} next={}",
            session,
            steps.len(),
            observation.verdict.status(),
            ompo_start::json_next_command(&steps).unwrap_or("none")
        );
        for step in &steps {
            println!(
                "  {} status={} reason_code={} next_command={}",
                step.id,
                format!("{:?}", step.status).to_ascii_uppercase(),
                step.reason_code.unwrap_or("none"),
                step.next_command.unwrap_or("none")
            );
        }
    }
    ExitCode::SUCCESS
}

fn run_portal(rest: &[String]) -> ExitCode {
    let mut repo = match current_repo() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("ompo portal: {error}");
            return ExitCode::from(EXIT_INSTRUMENT);
        }
    };
    let mut session = "omp-orchestrator".to_owned();
    let mut json_output = false;
    let mut index = 0;
    while index < rest.len() {
        match rest[index].as_str() {
            "--json" => json_output = true,
            "--repo" => {
                index += 1;
                let Some(value) = rest.get(index) else {
                    eprintln!("ompo portal: UAD_MISSING_VALUE flag=--repo");
                    return ExitCode::from(EXIT_BAD_INVOCATION);
                };
                repo = PathBuf::from(value);
            }
            "--session" => {
                index += 1;
                let Some(value) = rest.get(index) else {
                    eprintln!("ompo portal: UAD_MISSING_VALUE flag=--session");
                    return ExitCode::from(EXIT_BAD_INVOCATION);
                };
                session = value.clone();
            }
            "--help" | "-h" => {
                println!("{}", umbrella::usage());
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("ompo portal: UAD_UNKNOWN_ARGUMENT argument={other:?}");
                return ExitCode::from(EXIT_BAD_INVOCATION);
            }
        }
        index += 1;
    }

    let observation = liveness::observe(&session);
    let sources = observation
        .verdict
        .sources()
        .iter()
        .map(|source| (source.name.clone(), ompo_start::liveness::source_json(source)))
        .collect::<serde_json::Map<String, Value>>();
    let mut alerts = Vec::new();
    for source in observation.verdict.sources() {
        if !source.available || !source.fresh || source.age_ms.is_none() {
            alerts.push(json!({
                "severity": if source.available { "warn" } else { "error" },
                "summary": format!("{} source is not fresh and available", source.name),
                "action": format!("ompo portal --json --session {}", session),
            }));
        }
    }
    let inception_path = repo.join(".omp-orchestrator").join("inception.json");
    let inception = match ompo_start::inception::read_inception(&inception_path) {
        Ok(_) => {
            json!({"path": inception_path.display().to_string(), "status": "PRESENT", "readback": "PASS"})
        }
        Err(error) if !inception_path.exists() => {
            json!({"path": inception_path.display().to_string(), "status": "ABSENT", "readback": "REFUSE", "reason": error.to_string()})
        }
        Err(error) => {
            json!({"path": inception_path.display().to_string(), "status": "INVALID", "readback": "REFUSE", "reason": error.to_string()})
        }
    };
    let all_sources_fresh = observation
        .verdict
        .sources()
        .iter()
        .all(|source| source.available && source.fresh && source.age_ms.is_some());
    let available_sources = observation
        .verdict
        .sources()
        .iter()
        .filter(|source| source.available)
        .count();
    let input_manifest = if all_sources_fresh {
        json!({"state": "FULL", "source": "ntm,tick-monitor,agent-mail"})
    } else {
        json!({
            "state": "PARTIAL",
            "bound_kind": "available_liveness_sources",
            "bound_value": available_sources,
            "source": "ompo portal runtime"
        })
    };
    let one_next_action = if inception["status"] == "ABSENT" || inception["status"] == "INVALID" {
        json!({"command": format!("ompo init --repo {} --json", repo.display()), "reason_code": "L5_INCEPTION_READBACK"})
    } else if !observation.verdict.is_live() {
        json!({"command": format!("ompo start --repo {} --session {} --json", repo.display(), session), "reason_code": observation.verdict.reason_code()})
    } else {
        json!({"command": format!("ompo start --repo {} --session {} --json", repo.display(), session), "reason_code": "L3_REVIEW"})
    };
    let row = json!({
        "schema_id": ompo_start::portal::SCHEMA_ID,
        "schema_version": ompo_start::portal::SCHEMA_VERSION,
        "generated_at": now_millis(),
        "sources": sources,
        "_alerts": alerts,
        "one_next_action": one_next_action,
        "liveness": liveness_json(&observation),
        "inception": inception,
        "input_manifest": input_manifest,
        "queue": ompo_start::portal::queue_depth(&repo),
        "gates": ompo_start::portal::gates_verdict(&repo),
    });
    let row = match ompo_start::portal::seal(row) {
        Ok(row) => row,
        Err(error) => {
            eprintln!("ompo portal: {error}");
            return ExitCode::from(EXIT_INSTRUMENT);
        }
    };
    if json_output {
        match serde_json::to_string(&umbrella::envelope("portal", "OK", row)) {
            Ok(text) => println!("{text}"),
            Err(error) => {
                eprintln!("ompo portal: cannot encode: {error}");
                return ExitCode::from(EXIT_INSTRUMENT);
            }
        }
    } else {
        println!(
            "OMPO_PORTAL status={} data_hash={}",
            observation.verdict.status(),
            row["data_hash"]
        );
    }
    ExitCode::SUCCESS
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn state_error(
    command: &'static str,
    json_output: bool,
    error: state_triad::StateError,
) -> ExitCode {
    let detail = error.to_string();
    if json_output {
        let value = umbrella::envelope(
            command,
            "ERROR",
            json!({"reason_code": error.code(), "detail": detail}),
        );
        match serde_json::to_string(&value) {
            Ok(text) => println!("{text}"),
            Err(encode_error) => eprintln!("ompo {command}: cannot encode refusal: {encode_error}"),
        }
    } else {
        eprintln!("ompo {command}: {} detail={detail}", error.code());
    }
    ExitCode::from(EXIT_BAD_INVOCATION)
}

fn state_repo(command: &str) -> Result<PathBuf, ExitCode> {
    current_repo().map_err(|error| {
        eprintln!("ompo {command}: {error}");
        ExitCode::from(EXIT_INSTRUMENT)
    })
}

fn run_validate(rest: &[String]) -> ExitCode {
    let mut repo = match state_repo("validate") {
        Ok(path) => path,
        Err(code) => return code,
    };
    let mut thing = None;
    let mut json_output = false;
    let mut index = 0;
    while index < rest.len() {
        match rest[index].as_str() {
            "--json" => json_output = true,
            "--repo" => {
                index += 1;
                let Some(path) = rest.get(index) else {
                    eprintln!("ompo validate: UAD_MISSING_VALUE flag=--repo");
                    return ExitCode::from(EXIT_BAD_INVOCATION);
                };
                repo = PathBuf::from(path);
            }
            "--help" | "-h" => {
                println!("ompo validate <thing> [--repo PATH] [--json]");
                return ExitCode::SUCCESS;
            }
            value if value.starts_with("--") => {
                eprintln!("ompo validate: UAD_UNKNOWN_ARGUMENT argument={value:?}");
                return ExitCode::from(EXIT_BAD_INVOCATION);
            }
            value if thing.is_some() => {
                eprintln!("ompo validate: UAD_UNEXPECTED_ARGUMENT argument={value:?}");
                return ExitCode::from(EXIT_BAD_INVOCATION);
            }
            value => thing = Some(value),
        }
        index += 1;
    }
    let Some(thing) = thing else {
        return state_error(
            "validate",
            json_output,
            state_triad::StateError::MissingValue {
                command: "validate",
            },
        );
    };
    match state_triad::validate(&repo, thing) {
        Ok(report) => emit_state_report("validate", json_output, report),
        Err(error) => state_error("validate", json_output, error),
    }
}

fn run_audit(rest: &[String]) -> ExitCode {
    let mut repo = match state_repo("audit") {
        Ok(path) => path,
        Err(code) => return code,
    };
    let mut limit = 20usize;
    let mut json_output = false;
    let mut index = 0;
    while index < rest.len() {
        match rest[index].as_str() {
            "--json" => json_output = true,
            "--repo" => {
                index += 1;
                let Some(path) = rest.get(index) else {
                    eprintln!("ompo audit: UAD_MISSING_VALUE flag=--repo");
                    return ExitCode::from(EXIT_BAD_INVOCATION);
                };
                repo = PathBuf::from(path);
            }
            "--help" | "-h" => {
                println!("ompo audit [--limit N] [--repo PATH] [--json]");
                return ExitCode::SUCCESS;
            }
            "--limit" => {
                index += 1;
                let Some(value) = rest.get(index) else {
                    eprintln!("ompo audit: UAD_MISSING_VALUE flag=--limit");
                    return ExitCode::from(EXIT_BAD_INVOCATION);
                };
                limit = match value.parse() {
                    Ok(limit) => limit,
                    Err(_) => {
                        eprintln!("ompo audit: STATE_AUDIT_INVALID_LIMIT value={value:?}");
                        return ExitCode::from(EXIT_BAD_INVOCATION);
                    }
                };
            }
            value => {
                eprintln!("ompo audit: UAD_UNKNOWN_ARGUMENT argument={value:?}");
                return ExitCode::from(EXIT_BAD_INVOCATION);
            }
        }
        index += 1;
    }
    match state_triad::audit(&repo, limit) {
        Ok(report) => emit_state_report("audit", json_output, report),
        Err(error) => state_error("audit", json_output, error),
    }
}

fn run_why(rest: &[String]) -> ExitCode {
    let mut repo = match state_repo("why") {
        Ok(path) => path,
        Err(code) => return code,
    };
    let mut id = None;
    let mut json_output = false;
    let mut index = 0;
    while index < rest.len() {
        match rest[index].as_str() {
            "--json" => json_output = true,
            "--repo" => {
                index += 1;
                let Some(path) = rest.get(index) else {
                    eprintln!("ompo why: UAD_MISSING_VALUE flag=--repo");
                    return ExitCode::from(EXIT_BAD_INVOCATION);
                };
                repo = PathBuf::from(path);
            }
            "--help" | "-h" => {
                println!("ompo why <id> [--repo PATH] [--json]");
                return ExitCode::SUCCESS;
            }
            value if value.starts_with("--") => {
                eprintln!("ompo why: UAD_UNKNOWN_ARGUMENT argument={value:?}");
                return ExitCode::from(EXIT_BAD_INVOCATION);
            }
            value if id.is_some() => {
                eprintln!("ompo why: UAD_UNEXPECTED_ARGUMENT argument={value:?}");
                return ExitCode::from(EXIT_BAD_INVOCATION);
            }
            value => id = Some(value),
        }
        index += 1;
    }
    let Some(id) = id else {
        return state_error(
            "why",
            json_output,
            state_triad::StateError::MissingValue { command: "why" },
        );
    };
    match state_triad::why(&repo, id, umbrella::adapters()) {
        Ok(report) => emit_state_report("why", json_output, report),
        Err(error) => state_error("why", json_output, error),
    }
}

fn emit_state_report<T: serde::Serialize>(
    command: &'static str,
    json_output: bool,
    report: T,
) -> ExitCode {
    let data = match serde_json::to_value(report) {
        Ok(data) => data,
        Err(error) => {
            eprintln!("ompo {command}: cannot encode report: {error}");
            return ExitCode::from(EXIT_INSTRUMENT);
        }
    };
    if json_output {
        match serde_json::to_string(&umbrella::envelope(command, "OK", data)) {
            Ok(text) => println!("{text}"),
            Err(error) => {
                eprintln!("ompo {command}: cannot encode report: {error}");
                return ExitCode::from(EXIT_INSTRUMENT);
            }
        }
    } else {
        println!("OMPO_{} OK {}", command.to_ascii_uppercase(), data);
    }
    ExitCode::SUCCESS
}

/// ompo upstream-report <adapter> [--json] [--apply]
///
/// Printing is the DEFAULT and --apply is the gated opt-in, per the canonical standard: a
/// verb that writes to the tree on a bare invocation cannot be explored safely.
fn run_upstream_report(rest: &[String]) -> ExitCode {
    let json = wants_json(rest);
    let apply = rest.iter().any(|arg| arg == "--apply");
    let Some(adapter) = rest.iter().find(|arg| !arg.starts_with("--")) else {
        eprintln!(
            "ompo upstream-report: UAD_MISSING_VALUE argument=<adapter> \
             hint=ompo capabilities --json enumerates every adapter"
        );
        return ExitCode::from(EXIT_BAD_INVOCATION);
    };
    let verdict = match adapter_exec::execute(adapter) {
        Ok(verdict) => verdict,
        Err(error) => {
            eprintln!("ompo upstream-report: {error}");
            return ExitCode::from(EXIT_BAD_INVOCATION);
        }
    };
    let decision = upstream_report::classify(&verdict);
    if apply {
        let repo = match current_repo() {
            Ok(path) => path,
            Err(error) => {
                eprintln!("ompo upstream-report: {error}");
                return ExitCode::from(EXIT_INSTRUMENT);
            }
        };
        return match upstream_report::apply(&verdict, &decision, &repo) {
            Ok(applied) => {
                println!(
                    "{} path={}",
                    applied.reason_code(),
                    applied.path().display()
                );
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("ompo upstream-report: {error}");
                ExitCode::from(EXIT_BAD_INVOCATION)
            }
        };
    }
    if json {
        let value = upstream_report::envelope(&verdict, &decision);
        match serde_json::to_string(&value) {
            Ok(text) => {
                println!("{text}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("ompo upstream-report: cannot encode report: {error}");
                ExitCode::from(EXIT_INSTRUMENT)
            }
        }
    } else {
        match &decision {
            Ok(reportable) => {
                print!("{}", upstream_report::draft(&verdict, reportable));
                ExitCode::SUCCESS
            }
            Err(not) => {
                println!(
                    "{} adapter={} detail={}",
                    not.reason_code(),
                    adapter,
                    not.detail()
                );
                ExitCode::SUCCESS
            }
        }
    }
}
/// `ompo help <adapter>` — `LAW-UAD-EVERY-TARGET-ADDRESSABLE` and `LAW-UAD-UNKNOWN-IS-TWO`.
fn run_help(rest: &[String]) -> ExitCode {
    let Some(adapter) = rest.first() else {
        eprintln!("ompo help: UAD_MISSING_ADAPTER reason=no_adapter_named");
        eprintln!("{}", umbrella::usage());
        return ExitCode::from(EXIT_BAD_INVOCATION);
    };
    match umbrella::help_for(adapter) {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("ompo help: {error}");
            // An empty roster is an INSTRUMENT failure, not a caller mistake: the two must
            // not share an exit code or "the tool is broken" reads as "you typed it wrong".
            if error.starts_with("UAD_EMPTY_ROSTER") {
                ExitCode::from(EXIT_INSTRUMENT)
            } else {
                ExitCode::from(EXIT_BAD_INVOCATION)
            }
        }
    }
}

/// `ompo capabilities [--json]` — the golden artifact `LAW-UAD-CAPABILITIES-GOLDEN` pins.
fn run_capabilities(rest: &[String]) -> ExitCode {
    let json = rest.iter().any(|arg| arg == "--json");
    for arg in rest {
        if arg != "--json" {
            eprintln!("ompo capabilities: unknown argument {arg:?}");
            return ExitCode::from(EXIT_BAD_INVOCATION);
        }
    }
    let value = match umbrella::capabilities() {
        Ok(value) => value,
        Err(error) => {
            eprintln!("ompo capabilities: {error}");
            return ExitCode::from(EXIT_INSTRUMENT);
        }
    };
    if json {
        match serde_json::to_string(&value) {
            Ok(text) => {
                println!("{text}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("ompo capabilities: cannot encode: {error}");
                ExitCode::from(EXIT_INSTRUMENT)
            }
        }
    } else {
        // Structured =N values, never prose. `docs/plan/07-installability.md:124`.
        println!(
            "OMPO_CAPABILITIES adapters={} verbs={} probe_ids={}",
            value["data"]["adapter_count"],
            umbrella::VERBS.len(),
            value["data"]["probe_id_count"]
        );
        ExitCode::SUCCESS
    }
}

/// `ompo parity --installed PATH [--json]` compares an installed artifact's advertised verbs
/// against the canonical source umbrella. A malformed or unrunnable artifact is never current,
/// and neither is a verdict from a build that cannot name its own revision: `status` is the
/// VERDICT, `verb_set_status` the observation it rests on, and the two are published apart so
/// an UNKNOWN never hides the comparison that was actually performed.
fn run_parity(rest: &[String]) -> ExitCode {
    let mut installed: Option<PathBuf> = None;
    let mut json_output = false;
    let mut index = 0;
    while index < rest.len() {
        match rest[index].as_str() {
            "--json" => json_output = true,
            "--installed" => {
                index += 1;
                let Some(path) = rest.get(index) else {
                    eprintln!("ompo parity: UAD_MISSING_VALUE flag=--installed");
                    return ExitCode::from(EXIT_BAD_INVOCATION);
                };
                installed = Some(PathBuf::from(path));
            }
            "--help" | "-h" => {
                println!("{}", umbrella::usage());
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("ompo parity: unknown argument {other:?} hint=ompo parity --installed PATH [--json]");
                return ExitCode::from(EXIT_BAD_INVOCATION);
            }
        }
        index += 1;
    }
    let Some(installed) = installed else {
        eprintln!("ompo parity: UAD_MISSING_VALUE flag=--installed");
        return ExitCode::from(EXIT_BAD_INVOCATION);
    };
    let probe = provenance::probe_installed_verb_parity(&installed);
    let payload = json!({
        "status": probe.status,
        "exit_code": probe.exit_code,
        "reason_code": &probe.reason_code,
        "message": &probe.message,
        "detail": &probe.detail,
        "installed_path": &probe.installed_path,
        "source_revision": probe.provenance.source_revision,
        "build_commit": probe.provenance.build_commit,
        "provenance": probe.provenance,
        "installed_verbs": &probe.installed_verbs,
        "missing": &probe.missing,
        "unexpected": &probe.unexpected,
        "verb_set_status": probe.verb_set_status,
        "missing_provenance": &probe.missing_provenance,
        "unmeasured_axes": probe.unmeasured_axes,
    });
    if json_output {
        let envelope = umbrella::envelope("parity", probe.status, payload);
        match serde_json::to_string(&envelope) {
            Ok(text) if probe.exit_code == provenance::PARITY_EXIT_CURRENT => println!("{text}"),
            Ok(text) => eprintln!("{text}"),
            Err(error) => {
                eprintln!("ompo parity: UNMEASURED detail=cannot encode report: {error}");
                return ExitCode::from(provenance::PARITY_EXIT_UNMEASURED);
            }
        }
    } else if probe.exit_code == provenance::PARITY_EXIT_CURRENT {
        println!("OMPO_PARITY status={} verb_set_status={} installed_path={} source_revision={} build_commit={} missing_provenance={:?} unmeasured_axes={:?} message={} detail={}", probe.status, probe.verb_set_status, probe.installed_path, probe.provenance.source_revision, probe.provenance.build_commit, probe.missing_provenance, probe.unmeasured_axes, probe.message, probe.detail);
    } else {
        eprintln!("OMPO_PARITY status={} verb_set_status={} installed_path={} source_revision={} build_commit={} missing_provenance={:?} unmeasured_axes={:?} message={} detail={}", probe.status, probe.verb_set_status, probe.installed_path, probe.provenance.source_revision, probe.provenance.build_commit, probe.missing_provenance, probe.unmeasured_axes, probe.message, probe.detail);
    }
    ExitCode::from(probe.exit_code)
}

/// `ompo init` — the missing command surface over `ompo_start::inception::initialize`.
///
/// The mechanism is NOT reimplemented here. This routes to the existing write+reprobe
/// chokepoint whose idempotence is already proven by
/// `initialize_reprobes_and_second_run_has_zero_artifact_actions`; the defect was that no
/// operator could reach it.
fn run_init(rest: &[String]) -> ExitCode {
    let mut repo = match current_repo() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("ompo init: {error}");
            return ExitCode::from(EXIT_INSTRUMENT);
        }
    };
    let mut output: Option<PathBuf> = None;
    let mut json = false;
    let mut index = 0;
    while index < rest.len() {
        match rest[index].as_str() {
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", umbrella::usage());
                return ExitCode::SUCCESS;
            }
            "--repo" => {
                index += 1;
                let Some(path) = rest.get(index) else {
                    eprintln!("ompo init: UAD_MISSING_VALUE flag=--repo");
                    return ExitCode::from(EXIT_BAD_INVOCATION);
                };
                repo = PathBuf::from(path);
            }
            "--output" => {
                index += 1;
                let Some(path) = rest.get(index) else {
                    eprintln!("ompo init: UAD_MISSING_VALUE flag=--output");
                    return ExitCode::from(EXIT_BAD_INVOCATION);
                };
                output = Some(PathBuf::from(path));
            }
            other => {
                eprintln!("ompo init: unknown argument {other:?}");
                return ExitCode::from(EXIT_BAD_INVOCATION);
            }
        }
        index += 1;
    }
    let destination =
        output.unwrap_or_else(|| repo.join(".omp-orchestrator").join("inception.json"));

    match ompo_start::inception::initialize(&repo, &destination) {
        Ok(report) => {
            if json {
                let value = umbrella::envelope(
                    "init",
                    "OK",
                    serde_json::json!({
                        "artifact": destination.display().to_string(),
                        "actions": report.actions,
                        "backup": report.backup.as_ref().map(|p| p.display().to_string()),
                        "journal_rows": report.journal_rows,
                        "monitor_rows": report.monitor_rows,
                    }),
                );
                match serde_json::to_string(&value) {
                    Ok(text) => println!("{text}"),
                    Err(error) => {
                        eprintln!("ompo init: cannot encode: {error}");
                        return ExitCode::from(EXIT_INSTRUMENT);
                    }
                }
            } else {
                // actions=0 on a second run is the IDEMPOTENCE receipt, so it is a named
                // integer rather than a silent success.
                println!(
                    "OMPO_INIT artifact={} actions={} journal_rows={} monitor_rows={} backup={}",
                    destination.display(),
                    report.actions,
                    report.journal_rows,
                    report.monitor_rows,
                    report
                        .backup
                        .as_ref()
                        .map_or_else(|| "none".to_owned(), |p| p.display().to_string())
                );
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("ompo init: {error}");
            ExitCode::from(1)
        }
    }
}

/// `ompo doctor [--scope FAMILY]` — unchanged semantics, now one verb among several.
fn run_doctor_verb(rest: &[String]) -> ExitCode {
    // `capabilities` is reachable as a doctor sub-verb too, because
    // `docs/plan/07-installability.md:220-222` writes it as
    // `omp-orchestrator doctor capabilities --json`. Same answer, two spellings, ONE
    // implementation -- an alias is not a second definition.
    if rest.first().map(String::as_str) == Some("capabilities") {
        return run_capabilities(&rest[1..]);
    }
    let mut repo = match current_repo() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("ompo doctor: {error}");
            return ExitCode::from(EXIT_INSTRUMENT);
        }
    };
    let mut scope = "system".to_owned();
    let mut json = false;
    let mut index = 0;
    while index < rest.len() {
        match rest[index].as_str() {
            "--json" => json = true,
            "--repo" => {
                index += 1;
                let Some(path) = rest.get(index) else {
                    eprintln!("ompo doctor: UAD_MISSING_VALUE flag=--repo");
                    return ExitCode::from(EXIT_BAD_INVOCATION);
                };
                repo = PathBuf::from(path);
            }
            "--scope" => {
                index += 1;
                let Some(value) = rest.get(index) else {
                    eprintln!("ompo doctor: UAD_MISSING_VALUE flag=--scope");
                    return ExitCode::from(EXIT_BAD_INVOCATION);
                };
                scope = value.clone();
            }
            "--adapter" => {
                // UAD-ADDRESS, the ADAPTER axis, EXECUTED rather than refused. The
                // placeholder this replaces said `reason=per_adapter_probes_not_built`,
                // which was accurate until now: the flag named the missing capability.
                // `--json` is read from the whole argv, so flag order does not matter.
                index += 1;
                let Some(named) = rest.get(index).cloned() else {
                    eprintln!("ompo doctor: UAD_MISSING_VALUE flag=--adapter");
                    return ExitCode::from(EXIT_BAD_INVOCATION);
                };
                return ExitCode::from(adapter_exec::run_axis(&named, wants_json(rest)));
            }
            "--help" | "-h" => {
                println!("{}", umbrella::usage());
                return ExitCode::SUCCESS;
            }
            other => {
                // `docs/plan/07-installability.md:128` prescribes `doctor [<adapter>]`, so a
                // bare roster name is the PRESCRIBED spelling of the same axis as
                // `--adapter`; both route to ONE executor. A token that is neither the
                // roster selector nor a roster member keeps the old refusal, so a typo is
                // still named rather than swallowed.
                if adapter_exec::is_axis_selector(other) {
                    return ExitCode::from(adapter_exec::run_axis(other, wants_json(rest)));
                }
                eprintln!("ompo doctor: unknown argument {other:?}");
                return ExitCode::from(EXIT_BAD_INVOCATION);
            }
        }
        index += 1;
    }

    match run_doctor(&repo, &scope) {
        Ok(summary) if json => {
            let verdicts = summary.probes.clone();
            let value = umbrella::envelope(
                "doctor",
                summary.status,
                serde_json::json!({
                    "doctor_schema": summary.schema,
                    "run_id": summary.run_id,
                    "scope": summary.scope,
                    "status": summary.status,
                    "exit_code": summary.exit_code,
                    "probe_count": summary.probe_count,
                    "probes": summary.probes,
                    "verdicts": verdicts,
                    "remediation": summary.remediation,
                    "next_action": summary.next_action,
                    "lifecycle_events": summary.event_count,
                    "readback_lines": summary.readback_lines,
                    "journal": summary.lifecycle_journal.display().to_string(),
                    "artifact": ompo_doctor::ARTIFACT_REFERENCE,
                    // The VERIFIED readback of that artifact, not a restatement
                    // of the promise in `next_action`.
                    "artifact_readback": summary.report,
                    // L1's probe-answer metric WITH its denominator, the named
                    // UNPROBEABLE set, the STALE band's measurability, and a
                    // verdict (59up). A named metric with no verdict greens forever.
                    "metric": summary.metric,
                    "probe_id_root": ProbeId::new("omp.identity.binary.root")
                        .map(|id| id.as_str().to_owned())
                        .unwrap_or_default(),
                }),
            );
            match serde_json::to_string(&value) {
                Ok(text) => {
                    println!("{text}");
                    ExitCode::from(summary.exit_code)
                }
                Err(error) => {
                    eprintln!("ompo doctor: cannot encode report: {error}");
                    ExitCode::from(EXIT_INSTRUMENT)
                }
            }
        }
        Ok(summary) => {
            println!(
                "OMPO_DOCTOR scope={} status={} exit_code={} probes={} lifecycle_events={} readback_lines={} journal={}",
                summary.scope,
                summary.status,
                summary.exit_code,
                summary.probe_count,
                summary.event_count,
                summary.readback_lines,
                summary.lifecycle_journal.display()
            );
            ExitCode::from(summary.exit_code)
        }
        Err(error) => {
            eprintln!("ompo doctor: {error}");
            // An unknown scope name is a usage error (a name absent from the
            // roster), not a degraded verdict: the contract pins rc=2 here
            // and every other doctor failure stays rc=1.
            match error {
                DoctorError::UnsupportedScope(_) => ExitCode::from(EXIT_BAD_INVOCATION),
                _ => ExitCode::from(1),
            }
        }
    }
}

/// `ompo state [--json]` — the FIRST verb that reads OMP's OWN surface rather than ours.
///
/// Every other verb in this binary orchestrates workspace crates. This one drives OMP's
/// native `--mode=rpc` protocol through `omp-rpc-session` and issues OMP's real `get_state`
/// command, which is the protocol answer to the question `AGENTS.md`'s fifth rule says we
/// scrape from a braille spinner.
///
/// The runtime is built here rather than in the module so the async boundary is visible at
/// the CLI edge, and the pattern is copied from `omp-orchestrator/src/main.rs:6213` rather
/// than invented.
fn run_state(rest: &[String]) -> ExitCode {
    let mut json = false;
    let mut session = None;
    let mut session_dir = None;
    let mut index = 0;
    while index < rest.len() {
        match rest[index].as_str() {
            "--json" => json = true,
            "--session" => {
                index += 1;
                let Some(value) = rest.get(index) else {
                    eprintln!("ompo state: STATE_SESSION_REQUIRED detail=--session requires an existing session id");
                    return ExitCode::from(EXIT_BAD_INVOCATION);
                };
                if value.trim().is_empty() {
                    eprintln!("ompo state: STATE_SESSION_REQUIRED detail=--session requires a non-empty existing session id");
                    return ExitCode::from(EXIT_BAD_INVOCATION);
                }
                session = Some(value.as_str());
            }
            "--session-dir" => {
                index += 1;
                let Some(value) = rest.get(index) else {
                    eprintln!("ompo state: STATE_SESSION_DIR_REQUIRED detail=--session-dir requires a path");
                    return ExitCode::from(EXIT_BAD_INVOCATION);
                };
                if value.trim().is_empty() {
                    eprintln!("ompo state: STATE_SESSION_DIR_REQUIRED detail=--session-dir requires a non-empty path");
                    return ExitCode::from(EXIT_BAD_INVOCATION);
                }
                session_dir = Some(PathBuf::from(value));
            }
            arg => {
                eprintln!(
                    "ompo state: unknown argument {arg:?} hint=--session <existing-id> [--session-dir PATH] [--json] drives one bounded OMP mode=rpc session"
                );
                return ExitCode::from(EXIT_BAD_INVOCATION);
            }
        }
        index += 1;
    }
    let runtime = match asupersync::runtime::RuntimeBuilder::current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("ompo state: OMP_STATE_RUNTIME_UNAVAILABLE detail={error}");
            return ExitCode::from(EXIT_INSTRUMENT);
        }
    };
    let outcome = runtime.block_on(async {
        match asupersync::Cx::current() {
            Some(cx) => {
                Ok(omp_state::read_state(&cx, "omp", session, session_dir.as_deref()).await)
            }
            None => Err("no runtime context"),
        }
    });
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(detail) => {
            eprintln!("ompo state: OMP_STATE_RUNTIME_UNAVAILABLE detail={detail}");
            return ExitCode::from(EXIT_INSTRUMENT);
        }
    };
    if json {
        match serde_json::to_string(&omp_state::envelope(&outcome)) {
            Ok(text) if outcome.exit_code() == omp_state::EXIT_OK => println!("{text}"),
            Ok(text) => eprintln!("{text}"),
            Err(error) => {
                eprintln!("ompo state: cannot encode report: {error}");
                return ExitCode::from(EXIT_INSTRUMENT);
            }
        }
    } else if let omp_state::StateOutcome::Answered(state) = &outcome {
        println!("{}", omp_state::render(state));
    } else {
        eprintln!(
            "ompo state: {} detail={}",
            outcome.reason_code(),
            outcome.detail()
        );
    }
    ExitCode::from(outcome.exit_code())
}
/// ompo ps [--repo PATH] [--json] — read the project-scoped supervised daemon rows.
///
/// This is a read-only CLI child probe. It never connects to broker.sock and never invokes a
/// lifecycle mutation. The default scope is exactly the caller's current directory.
fn run_ps(rest: &[String]) -> ExitCode {
    let mut repo = match std::env::current_dir() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("ompo ps: OMP_PS_UNMEASURED detail=cannot read current directory: {error}");
            return ExitCode::from(omp_process::EXIT_UNMEASURED);
        }
    };
    let mut json_output = false;
    let mut index = 0;
    while index < rest.len() {
        match rest[index].as_str() {
            "--json" => json_output = true,
            "--repo" => {
                index += 1;
                let Some(path) = rest.get(index) else {
                    eprintln!("ompo ps: UAD_MISSING_VALUE flag=--repo");
                    return ExitCode::from(omp_process::EXIT_BAD_INVOCATION);
                };
                repo = PathBuf::from(path);
            }
            "--help" | "-h" => {
                println!("{}", umbrella::usage());
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!(
                    "ompo ps: unknown argument {other:?} hint=ompo ps [--repo PATH] [--json]"
                );
                return ExitCode::from(omp_process::EXIT_BAD_INVOCATION);
            }
        }
        index += 1;
    }

    let runtime = match asupersync::runtime::RuntimeBuilder::current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("ompo ps: OMP_PS_UNMEASURED detail=runtime unavailable: {error}");
            return ExitCode::from(omp_process::EXIT_UNMEASURED);
        }
    };
    let outcome = runtime.block_on(async {
        match asupersync::Cx::current() {
            Some(cx) => Ok(omp_process::read_processes(&cx, &repo).await),
            None => Err("no runtime context"),
        }
    });
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(detail) => {
            eprintln!("ompo ps: OMP_PS_UNMEASURED detail={detail}");
            return ExitCode::from(omp_process::EXIT_UNMEASURED);
        }
    };

    if json_output {
        match serde_json::to_string(&omp_process::envelope(&outcome)) {
            Ok(text) if outcome.exit_code() == omp_process::EXIT_OK => println!("{text}"),
            Ok(text) => eprintln!("{text}"),
            Err(error) => {
                eprintln!("ompo ps: OMP_PS_UNMEASURED detail=cannot encode report: {error}");
                return ExitCode::from(omp_process::EXIT_UNMEASURED);
            }
        }
    } else if let omp_process::ProcessProbeVerdict::Answered(scopes) = &outcome.verdict {
        println!("{}", omp_process::render(scopes));
    } else {
        eprintln!(
            "ompo ps: {} detail={}",
            outcome.reason_code(),
            outcome.detail()
        );
    }
    ExitCode::from(outcome.exit_code())
}

/// `tokens`, `toolCalls` or `premiumRequests` from OMP at all.
fn run_stats(rest: &[String]) -> ExitCode {
    for arg in rest {
        if arg != "--json" {
            eprintln!(
                "ompo stats: unknown argument {arg:?} \
                 hint=`ompo stats [--json]` drives one bounded OMP --mode=rpc session"
            );
            return ExitCode::from(EXIT_BAD_INVOCATION);
        }
    }
    let json = wants_json(rest);
    let runtime = match asupersync::runtime::RuntimeBuilder::current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("ompo stats: OMP_STATS_RUNTIME_UNAVAILABLE detail={error}");
            return ExitCode::from(EXIT_INSTRUMENT);
        }
    };
    let outcome = runtime.block_on(async {
        match asupersync::Cx::current() {
            Some(cx) => Ok(omp_stats::read_stats(&cx, "omp").await),
            None => Err("no runtime context"),
        }
    });
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(detail) => {
            eprintln!("ompo stats: OMP_STATS_RUNTIME_UNAVAILABLE detail={detail}");
            return ExitCode::from(EXIT_INSTRUMENT);
        }
    };
    if json {
        match serde_json::to_string(&omp_stats::envelope(&outcome)) {
            Ok(text) if outcome.exit_code() == omp_state::EXIT_OK => println!("{text}"),
            Ok(text) => eprintln!("{text}"),
            Err(error) => {
                eprintln!("ompo stats: cannot encode report: {error}");
                return ExitCode::from(EXIT_INSTRUMENT);
            }
        }
    } else if let omp_stats::StatsOutcome::Answered(stats) = &outcome {
        println!("{}", omp_stats::render(stats));
    } else {
        eprintln!(
            "ompo stats: {} detail={}",
            outcome.reason_code(),
            outcome.detail()
        );
    }
    ExitCode::from(outcome.exit_code())
}

/// `ompo messages [--json]` — OMP's message roles and counts over `--mode=rpc`.
///
/// The empty case is the one that matters: an EMPTY messages array is a SUCCESS with
/// `count=0` at exit 0, and a MISSING or non-array key is `NO_PAYLOAD` at exit 1. A verb that
/// used a non-zero exit to mean "ran fine, no results" is the failure being prevented.
fn run_messages(rest: &[String]) -> ExitCode {
    for arg in rest {
        if arg != "--json" {
            eprintln!(
                "ompo messages: unknown argument {arg:?} \
                 hint=`ompo messages [--json]` drives one bounded OMP --mode=rpc session"
            );
            return ExitCode::from(EXIT_BAD_INVOCATION);
        }
    }
    let json = wants_json(rest);
    let runtime = match asupersync::runtime::RuntimeBuilder::current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("ompo messages: OMP_MESSAGES_RUNTIME_UNAVAILABLE detail={error}");
            return ExitCode::from(EXIT_INSTRUMENT);
        }
    };
    let outcome = runtime.block_on(async {
        match asupersync::Cx::current() {
            Some(cx) => Ok(omp_messages::read_messages(&cx, "omp").await),
            None => Err("no runtime context"),
        }
    });
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(detail) => {
            eprintln!("ompo messages: OMP_MESSAGES_RUNTIME_UNAVAILABLE detail={detail}");
            return ExitCode::from(EXIT_INSTRUMENT);
        }
    };
    if json {
        match serde_json::to_string(&omp_messages::envelope(&outcome)) {
            Ok(text) if outcome.exit_code() == omp_state::EXIT_OK => println!("{text}"),
            Ok(text) => eprintln!("{text}"),
            Err(error) => {
                eprintln!("ompo messages: cannot encode report: {error}");
                return ExitCode::from(EXIT_INSTRUMENT);
            }
        }
    } else if let omp_messages::MessagesOutcome::Answered(messages) = &outcome {
        println!("{}", omp_messages::render(messages));
    } else {
        eprintln!(
            "ompo messages: {} detail={}",
            outcome.reason_code(),
            outcome.detail()
        );
    }
    ExitCode::from(outcome.exit_code())
}

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
//! `2` alone cannot separate an unknown verb from an unknown adapter, so every refusal below
//! names what it rejected. AGENTS.md gate rule 7: an exit code is not a message.

use ompo_doctor::umbrella::{self, ProbeId};
use ompo_doctor::{current_repo, run_doctor};
use std::path::PathBuf;
use std::process::ExitCode;

/// Invocation error: the caller addressed something that does not exist.
const EXIT_BAD_INVOCATION: u8 = 2;
/// Instrument error: the umbrella itself could not answer.
const EXIT_INSTRUMENT: u8 = 3;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(verb) = args.first().map(String::as_str) else {
        eprintln!("{}", umbrella::usage());
        return ExitCode::from(EXIT_BAD_INVOCATION);
    };
    let rest = &args[1..];

    match verb {
        "--help" | "-h" | "help" if rest.is_empty() => {
            println!("{}", umbrella::usage());
            ExitCode::SUCCESS
        }
        "help" => run_help(rest),
        "capabilities" => run_capabilities(rest),
        "init" => run_init(rest),
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
                // The OTHER addressing axis, and it is deliberately refused here rather than
                // silently ignored: `--scope` selects a probe FAMILY, an adapter selects a
                // workspace TARGET, and per-adapter doctor probes are not implemented. A
                // flag that is accepted and does nothing is worse than one that refuses.
                index += 1;
                let named = rest.get(index).cloned().unwrap_or_default();
                eprintln!(
                    "ompo doctor: UAD_ADAPTER_SCOPED_DOCTOR_UNIMPLEMENTED adapter={named:?} \
                     reason=per_adapter_probes_not_built hint=`ompo help {named}` resolves the \
                     name; `--scope` selects a probe family"
                );
                return ExitCode::from(EXIT_BAD_INVOCATION);
            }
            "--help" | "-h" => {
                println!("{}", umbrella::usage());
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("ompo doctor: unknown argument {other:?}");
                return ExitCode::from(EXIT_BAD_INVOCATION);
            }
        }
        index += 1;
    }

    match run_doctor(&repo, &scope) {
        Ok(summary) if json => {
            let value = umbrella::envelope(
                "doctor",
                "OK",
                serde_json::json!({
                    "scope": summary.scope,
                    "probes": summary.probe_count,
                    "lifecycle_events": summary.event_count,
                    "readback_lines": summary.readback_lines,
                    "journal": summary.lifecycle_journal.display().to_string(),
                    "artifact": ompo_doctor::ARTIFACT_REFERENCE,
                    "probe_id_root": ProbeId::new("omp.identity.binary.root")
                        .map(|id| id.as_str().to_owned())
                        .unwrap_or_default(),
                }),
            );
            match serde_json::to_string(&value) {
                Ok(text) => {
                    println!("{text}");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("ompo doctor: cannot encode report: {error}");
                    ExitCode::from(EXIT_INSTRUMENT)
                }
            }
        }
        Ok(summary) => {
            println!(
                "OMPO_DOCTOR scope={} probes={} lifecycle_events={} readback_lines={} journal={}",
                summary.scope,
                summary.probe_count,
                summary.event_count,
                summary.readback_lines,
                summary.lifecycle_journal.display()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("ompo doctor: {error}");
            ExitCode::from(1)
        }
    }
}

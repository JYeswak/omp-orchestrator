#![forbid(unsafe_code)]
//! `decision-ledger` — the S9 writer, and a replay that proves what was being lost.
//!
//! ```text
//! decision-ledger replay <heartbeat.jsonl> [--since <unix>] [--apply] [--ledger <path>]
//! decision-ledger request --question <q> --asked-by <who> --blocking <gate:x|bead:y> --clause <authority|exclusive_capability|taste>
//! decision-ledger decision --id HD-000N --decision <d> --decider <who> [--recorded-by <a>] [--ref <r>]
//! decision-ledger from-close --reason <close reason> --decision <d> --decider <who>
//! ```
//!
//! `replay` DEFAULTS TO DRY-RUN. Reading the day's heartbeat and appending 30 rows to the
//! decision ledger as a side effect of asking "what did we lose?" would be its own defect.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use decision_ledger::execution::{check_path, Probe};
use decision_ledger::{
    append_agent_disposition, append_request, dispatch_backfill_candidates, hd_reference, now_unix,
    read_rows, record_decision, replay, Decision, HumanClause, Request,
};
use serde_json::Value;

const USAGE: &str = "usage:\n  \
    decision-ledger replay <heartbeat.jsonl> [--since <unix>] [--apply] [--ledger <path>]\n  \
    decision-ledger reconcile-dispatch [--apply] [--ledger <path>]\n  \
    decision-ledger request --question <q> --asked-by <who> --blocking <gate:x|bead:y> --clause <authority|exclusive_capability|taste> [--ledger <p>]\n  \
    decision-ledger decision --id HD-000N --decision <d> --decider <who> [--recorded-by <a>] [--ref <r>] [--ledger <p>]\n  \
    decision-ledger from-close --reason <text> --decision <d> --decider <who> [--recorded-by <a>] [--ledger <p>]\n  \
    decision-ledger check-unexecuted [--ledger <p>] [--unpushed-count <n>]";

fn default_ledger() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(|root| root.join("docs/decisions.jsonl"))
        .unwrap_or_else(|| PathBuf::from("docs/decisions.jsonl"))
}

fn flag(args: &[String], name: &str) -> Option<String> {
    let index = args.iter().position(|arg| arg == name)?;
    args.get(index + 1).cloned()
}

fn fail(message: impl std::fmt::Display) -> ExitCode {
    eprintln!("{message}");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ledger = flag(&args, "--ledger").map_or_else(default_ledger, PathBuf::from);
    match args.first().map(String::as_str) {
        Some("replay") => cmd_replay(&args, &ledger),
        Some("reconcile-dispatch") => cmd_reconcile_dispatch(&args, &ledger),
        Some("request") => cmd_request(&args, &ledger),
        Some("decision") => cmd_decision(&args, &ledger),
        Some("from-close") => cmd_from_close(&args, &ledger),
        Some("check-unexecuted") => cmd_check_unexecuted(&args, &ledger),
        Some("--help" | "-h") | None => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => fail(format!("unknown command {other:?}\n{USAGE}")),
    }
}

/// Replay a heartbeat ledger into the requests it should have produced.
fn cmd_replay(args: &[String], ledger: &Path) -> ExitCode {
    let Some(path) = args.get(1).filter(|arg| !arg.starts_with("--")) else {
        return fail(format!("replay needs a heartbeat path\n{USAGE}"));
    };
    let since: u64 = flag(args, "--since")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let apply = args.iter().any(|arg| arg == "--apply");

    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => return fail(format!("REPLAY_UNREADABLE path={path} detail={error}")),
    };
    let mut rows: Vec<(String, String, u64)> = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let ts = value.get("ts_unix").and_then(Value::as_u64).unwrap_or(0);
        if ts < since {
            continue;
        }
        let status = value
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let detail = value
            .get("detail")
            .map(|detail| match detail {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default();
        rows.push((status, detail, ts));
    }

    let report = replay(&rows);
    // ANTI-VACUITY before any count is printed, so a broken path can never be read as a
    // clean day.
    if let Err(reason) = report.require_nonvacuous() {
        return fail(reason);
    }
    println!(
        "REPLAY rows_read={} human_addressed={} distinct={} mode={}",
        report.rows_read,
        report.human_addressed,
        report.distinct.len(),
        if apply { "APPLY" } else { "DRY-RUN" }
    );
    for entry in &report.distinct {
        println!(
            "  {:>5}x  {:<28} {}",
            entry.occurrences, entry.blocking, entry.question
        );
    }
    if report.distinct.is_empty() {
        println!(
            "  none -- {} rows were read and none addressed a human. That is a real state \
             and is NOT the same as a broken replay; rows_read is printed above so the two \
             are distinguishable.",
            report.rows_read
        );
    }
    if !apply {
        return ExitCode::SUCCESS;
    }
    let mut wrote = 0usize;
    for entry in &report.distinct {
        let request = Request {
            question: entry.question.clone(),
            asked_by: "omp-orchestrator-supervisor".to_owned(),
            blocking: entry.blocking.clone(),
            clause: Some(entry.clause),
            ts: entry.first_ts,
        };
        match append_request(ledger, &request) {
            Ok(outcome) => {
                println!(
                    "  {} {} {}",
                    if outcome.wrote() {
                        "APPENDED"
                    } else {
                        "DEDUPED "
                    },
                    outcome.id(),
                    entry.blocking
                );
                if outcome.wrote() {
                    wrote += 1;
                }
            }
            Err(error) => return fail(error),
        }
    }
    println!(
        "REPLAY_APPLIED appended={wrote} ledger={}",
        ledger.display()
    );
    ExitCode::SUCCESS
}

/// Backfill machine-owned dispatch rows without pretending they are human answers.
fn cmd_reconcile_dispatch(args: &[String], ledger: &Path) -> ExitCode {
    let rows = match read_rows(ledger) {
        Ok(rows) => rows,
        Err(error) => return fail(error),
    };
    let candidates = dispatch_backfill_candidates(&rows);
    let apply = args.iter().any(|arg| arg == "--apply");
    println!(
        "DISPATCH_BACKFILL candidates={} moved=0 remaining={} mode={}",
        candidates.len(),
        candidates.len(),
        if apply { "APPLY" } else { "DRY-RUN" }
    );
    if !apply {
        return ExitCode::SUCCESS;
    }
    let mut moved = 0usize;
    for (id, disposition, reason) in &candidates {
        match append_agent_disposition(ledger, id, *disposition, reason, now_unix()) {
            Ok(outcome) => {
                if outcome.wrote() {
                    moved += 1;
                }
                println!(
                    "DISPATCH_BACKFILL_ROW id={} disposition={} result={}",
                    id,
                    disposition.as_str(),
                    if outcome.wrote() {
                        "MOVED"
                    } else {
                        "ALREADY_RESOLVED"
                    }
                );
            }
            Err(error) => return fail(error),
        }
    }
    println!(
        "DISPATCH_BACKFILL moved={moved} remaining={}",
        candidates.len().saturating_sub(moved)
    );
    ExitCode::SUCCESS
}

fn cmd_request(args: &[String], ledger: &Path) -> ExitCode {
    let (Some(question), Some(asked_by), Some(blocking), Some(clause)) = (
        flag(args, "--question"),
        flag(args, "--asked-by"),
        flag(args, "--blocking"),
        flag(args, "--clause").and_then(|value| HumanClause::parse(&value)),
    ) else {
        return fail(format!(
            "request needs --question, --asked-by, --blocking, --clause=authority|exclusive_capability|taste\n{USAGE}"
        ));
    };
    let request = Request {
        question,
        asked_by,
        blocking,
        clause: Some(clause),
        ts: now_unix(),
    };
    match append_request(ledger, &request) {
        Ok(outcome) if outcome.wrote() => {
            println!("APPENDED {}", outcome.id());
            ExitCode::SUCCESS
        }
        Ok(outcome) => {
            println!(
                "DEDUPED {} -- this question is already open; 188 identical rows are one \
                 question asked 188 times",
                outcome.id()
            );
            ExitCode::SUCCESS
        }
        Err(error) => fail(error),
    }
}

fn cmd_decision(args: &[String], ledger: &Path) -> ExitCode {
    let (Some(id), Some(decision), Some(decider)) = (
        flag(args, "--id"),
        flag(args, "--decision"),
        flag(args, "--decider"),
    ) else {
        return fail(format!(
            "decision needs --id, --decision, --decider\n{USAGE}"
        ));
    };
    let record = Decision {
        id,
        decision,
        decider,
        recorded_by: flag(args, "--recorded-by").unwrap_or_else(|| "unattributed".to_owned()),
        transcript_ref: flag(args, "--ref").unwrap_or_default(),
    };
    match record_decision(ledger, &record) {
        Ok(()) => {
            println!("RECORDED {}", record.id);
            ExitCode::SUCCESS
        }
        Err(error) => fail(error),
    }
}

/// A bead close carrying `HD-…` records the answer; one without it records NOTHING.
fn cmd_from_close(args: &[String], ledger: &Path) -> ExitCode {
    let Some(reason) = flag(args, "--reason") else {
        return fail(format!("from-close needs --reason\n{USAGE}"));
    };
    let Some(id) = hd_reference(&reason) else {
        println!(
            "NO_HD_REFERENCE -- this close cites no HD- id, so nothing was recorded. A \
             close is not automatically a decision."
        );
        return ExitCode::SUCCESS;
    };
    let (Some(decision), Some(decider)) = (flag(args, "--decision"), flag(args, "--decider"))
    else {
        return fail(format!(
            "from-close found {id} but needs --decision and --decider to record it\n{USAGE}"
        ));
    };
    let record = Decision {
        id: id.clone(),
        decision,
        decider,
        recorded_by: flag(args, "--recorded-by").unwrap_or_else(|| "unattributed".to_owned()),
        transcript_ref: reason,
    };
    match record_decision(ledger, &record) {
        Ok(()) => {
            println!("RECORDED {id} from close reason");
            ExitCode::SUCCESS
        }
        Err(error) => fail(error),
    }
}

fn cmd_check_unexecuted(args: &[String], ledger: &Path) -> ExitCode {
    let probe = Probe {
        origin_main_unpushed: flag(args, "--unpushed-count").and_then(|v| v.parse().ok()),
    };
    match check_path(ledger, probe) {
        Ok(report) => {
            print!("{}", report.render());
            if report.unexecuted_count() == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

/// Count the rows a caller can expect to read back, for a quick health line.
#[allow(dead_code)]
fn rows_in(ledger: &Path) -> usize {
    read_rows(ledger).map(|rows| rows.len()).unwrap_or(0)
}

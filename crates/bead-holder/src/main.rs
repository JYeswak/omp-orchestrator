#![forbid(unsafe_code)]

//! `bead-holder` — answer, for one bead or the whole tracker: **who holds this, and is that
//! agent alive.**
//!
//! ```text
//! bead-holder audit                 # every in-flight bead; nonzero when anything is unanswered
//! bead-holder resolve <bead-id>     # one bead, end to end
//! ```
//!
//! # The I/O boundary
//!
//! Every fact comes from a kernel this repo already has: `.beads/issues.jsonl` for the
//! tracker (the same records `br` writes), `tick-monitor observe` for liveness, and
//! `git config user.name` + `$USER` for the non-agent default author set. Nothing here
//! maintains a mapping — membership is derived, per this bead's rule.

use bead_holder::{
    audit, resolve, AuditError, BeadRow, Liveness, LIVENESS_SOURCE,
};
use std::collections::{BTreeMap, BTreeSet};
use std::process::{Command, ExitCode};
use std::time::Duration;

/// Bounded: both children are local and sub-second in the healthy case.
const PROBE_DEADLINE: Duration = Duration::from_secs(30);

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mode = argv.first().map(String::as_str).unwrap_or("audit");

    let repo = match repo_root() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("BEAD_HOLDER_ERROR reason=repo_root detail=\"{error}\"");
            return ExitCode::from(2);
        }
    };
    let beads = match read_tracker(&repo) {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("BEAD_HOLDER_ERROR reason=tracker_unreadable detail=\"{error}\"");
            return ExitCode::from(2);
        }
    };
    let defaults = default_authors();
    // DERIVED, from every ACK row in the tracker. There is deliberately no roster file: a
    // hand-maintained list is the thing this bead forbids.
    let mut bindings = BTreeSet::new();
    for bead in &beads {
        bindings.extend(bead_holder::derive_bindings(
            &bead.id,
            &bead.comments,
            &defaults,
        ));
    }
    let roster = bead_holder::roster_from(&bindings);
    let panes = bead_holder::pane_map(&bindings);

    eprintln!(
        "BEAD_HOLDER default_authors=[{}] roster={} panes={}",
        defaults.iter().cloned().collect::<Vec<_>>().join(","),
        roster.len(),
        panes.len()
    );
    for (pane, agents) in &panes {
        if agents.len() > 1 {
            // NAMED, never resolved. One pane hosting two agent names is exactly the state
            // that made the tracker unable to answer, and a silent pick would hide it.
            eprintln!(
                "BEAD_HOLDER_PANE_AMBIGUOUS pane={pane} agents=[{}]",
                agents.iter().cloned().collect::<Vec<_>>().join(",")
            );
        }
    }

    let liveness = live_panes();
    let lookup = |pane: &str| -> Option<Liveness> { liveness.get(pane).cloned() };

    match mode {
        "resolve" => {
            let Some(id) = argv.get(1) else {
                eprintln!("usage: bead-holder resolve <bead-id>");
                return ExitCode::from(64);
            };
            let Some(bead) = beads.iter().find(|b| &b.id == id) else {
                eprintln!("BEAD_HOLDER_ERROR reason=unknown_bead bead={id}");
                return ExitCode::from(2);
            };
            let verdict = resolve(bead, &roster, &defaults, &lookup);
            println!("{verdict}");
            if verdict.is_answered() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        "audit" => match audit(&beads, &roster, &defaults, &lookup) {
            Err(error @ AuditError::EmptyScan) => {
                eprintln!("{error}");
                ExitCode::from(2)
            }
            Ok(report) => {
                for verdict in &report.verdicts {
                    if !verdict.is_answered() {
                        println!("{verdict}");
                    }
                }
                let counts = report
                    .by_label()
                    .into_iter()
                    .map(|(label, n)| format!("{label}={n}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                println!(
                    "BEAD_HOLDER_AUDIT scanned={} answered={} {counts} liveness_source={LIVENESS_SOURCE}",
                    report.verdicts.len(),
                    report.answered()
                );
                // POSITIVE CONTROL, and it decides the exit code: zero answered means the
                // CHECKER is broken, not that the tracker is clean. Reporting "all
                // unanswered" as a healthy scan is the vacuity defect one layer up.
                if report.answered() == 0 {
                    println!(
                        "BEAD_HOLDER_AUDIT_ERROR reason=NO_POSITIVE_CONTROL \
                         detail=\"not one bead resolved end to end, so this run says nothing \
                         about the tracker\" next_action=check-tick-monitor-and-ack-rows"
                    );
                    return ExitCode::from(2);
                }
                if report.answered() == report.verdicts.len() {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(1)
                }
            }
        },
        other => {
            eprintln!("usage: bead-holder [audit|resolve <bead-id>]  (got {other:?})");
            ExitCode::from(64)
        }
    }
}

fn repo_root() -> Result<std::path::PathBuf, String> {
    let mut command = Command::new("git");
    command.args(["rev-parse", "--show-toplevel"]);
    Ok(std::path::PathBuf::from(
        bounded(&mut command, "git rev-parse")?.trim(),
    ))
}

/// The non-agent default author set, DERIVED and case-folded.
///
/// The case fold is load-bearing: `git config user.name` answered `Josh` while the comment
/// author was `josh`, so a case-sensitive exclusion silently failed and reported every pane
/// as ambiguous.
fn default_authors() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut command = Command::new("git");
    command.args(["config", "user.name"]);
    if let Ok(name) = bounded(&mut command, "git config") {
        let name = name.trim().to_lowercase();
        if !name.is_empty() {
            out.insert(name);
        }
    }
    if let Ok(user) = std::env::var("USER") {
        let user = user.trim().to_lowercase();
        if !user.is_empty() {
            out.insert(user);
        }
    }
    out
}

/// Read the tracker from its own JSONL, newest record per id.
///
/// The export holds one record per write, so a later record supersedes an earlier one for
/// the same id. Merging rather than replacing keeps fields a partial record omitted.
fn read_tracker(repo: &std::path::Path) -> Result<Vec<BeadRow>, String> {
    let path = repo.join(".beads/issues.jsonl");
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let mut merged: BTreeMap<String, serde_json::Map<String, serde_json::Value>> = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(serde_json::Value::Object(row)) = serde_json::from_str(line) else {
            continue;
        };
        let Some(id) = row.get("id").and_then(serde_json::Value::as_str) else {
            continue;
        };
        merged.entry(id.to_owned()).or_default().extend(row);
    }
    Ok(merged
        .into_iter()
        .map(|(id, row)| BeadRow {
            id,
            status: row
                .get("status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            assignee: row
                .get("assignee")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            comments: row
                .get("comments")
                .and_then(serde_json::Value::as_array)
                .map(|rows| {
                    rows.iter()
                        .filter_map(|c| {
                            Some((
                                c.get("author")
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or_default()
                                    .to_owned(),
                                c.get("text")
                                    .and_then(serde_json::Value::as_str)?
                                    .to_owned(),
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect())
}

/// Per-pane liveness from `tick-monitor observe`, the ONLY accepted source.
///
/// A roster's `last_active` is not consulted anywhere in this binary. It was measured 15
/// hours wrong on an agent that had committed 20 minutes earlier, and two agents actively
/// working beads read `last_active = 2d`.
fn live_panes() -> BTreeMap<String, Liveness> {
    let mut out = BTreeMap::new();
    let mut command = Command::new("tick-monitor");
    command.arg("observe");
    let Ok(text) = bounded(&mut command, "tick-monitor observe") else {
        // Absent, not empty. Every pane then resolves LivenessUnavailable, which is the
        // fail-closed direction: an unreachable oracle must not read as a live pane.
        return out;
    };
    // READ THE PAYLOAD, do not guess its shape. Measured 2026-09-02: `observe` emits ONE
    // JSON object whose pane rows live at `omp_lifecycle.panes[]` as
    // `{pane, state, timer_secs, liveness, why, epoch}`. An earlier version of this
    // function assumed a whitespace `%pane STATE` line format and silently bound NOTHING —
    // the audit reported answered=0 and its own NO_POSITIVE_CONTROL error caught it.
    //
    // `liveness` is the field taken, not `state`: the bead asks for the pane's TWO-CAPTURE
    // liveness, and `state` is the single-capture label.
    let Ok(payload) = serde_json::from_str::<serde_json::Value>(&text) else {
        return out;
    };
    let Some(rows) = payload
        .get("omp_lifecycle")
        .and_then(|section| section.get("panes"))
        .and_then(serde_json::Value::as_array)
    else {
        return out;
    };
    for row in rows {
        let Some(pane) = row.get("pane").and_then(serde_json::Value::as_str) else {
            continue;
        };
        // A row without a `liveness` field is SKIPPED, never defaulted: a defaulted label
        // would let a pane whose liveness was never computed read as answered.
        let Some(liveness) = row.get("liveness").and_then(serde_json::Value::as_str) else {
            continue;
        };
        out.insert(
            pane.to_owned(),
            Liveness {
                state: liveness.to_owned(),
                source: LIVENESS_SOURCE.to_owned(),
            },
        );
    }
    out
}

fn bounded(command: &mut Command, what: &str) -> Result<String, String> {
    match subprocess_contract::bounded_output(command, PROBE_DEADLINE) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        }
        subprocess_contract::BoundedOutcome::Completed(output) => Err(format!(
            "{what} exited {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        subprocess_contract::BoundedOutcome::TimedOut => {
            Err(format!("{what} exceeded its deadline; a timeout is not a verdict"))
        }
        subprocess_contract::BoundedOutcome::Unspawned(error) => {
            Err(format!("{what} could not be spawned: {error}"))
        }
    }
}

#![forbid(unsafe_code)]

//! `doctrine-retirement-gate` — the thin CLI over the kernel.
//!
//! ```text
//! doctrine-retirement-gate [--repo <path>] [--json]
//! ```
//!
//! Exit lattice, per part 2 of the crate atom and pinned by `tests/retirement.rs`:
//!
//! ```text
//! 0  PASS              every span is non-empty and nothing marked dead is reachable unmarked
//! 1  REFUSED           an empty span, or a marked-dead string occurring unmarked elsewhere
//! 2  UNRUN             nothing was scanned — an empty scan is an ERROR, never a pass
//! 3  INSTRUMENT_ERROR  unreadable file, or unbalanced retirement spans
//! ```
//!
//! BOTH the code and the message are asserted on every refusal leg, per AGENTS.md gate rule 7:
//! a message-only assertion survives two causes collapsing into one code, and a code-only
//! assertion survives an unrelated breakage returning the same code.

use doctrine_retirement_gate::{scan, Document, Verdict, SCANNED_DOCUMENTS};
use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "doctrine-retirement-gate [--repo <path>] [--json]\n\
             \n\
             Refuses retracted doctrine that a substring match cannot tell from live doctrine.\n\
             exit 0 PASS | 1 REFUSED | 2 UNRUN | 3 INSTRUMENT_ERROR"
        );
        return ExitCode::from(0);
    }
    let json = args.iter().any(|arg| arg == "--json");
    let repo = flag(&args, "--repo").map_or_else(|| PathBuf::from("."), PathBuf::from);

    let verdict = match load(&repo) {
        Ok(documents) => scan(&documents),
        Err(reason) => Verdict::InstrumentError { reason },
    };
    emit(&verdict, json);
    ExitCode::from(verdict.exit_code())
}

/// Read every document in scope.
///
/// An unreadable named file is an INSTRUMENT_ERROR, not a pass and not a refusal: the gate could
/// not look. `workflow_shape.rs` carries the same rule for the same reason — an unreadable input
/// is the state in which nothing runs and nothing reports.
fn load(repo: &Path) -> Result<Vec<Document>, String> {
    let mut documents = Vec::new();
    for relative in SCANNED_DOCUMENTS {
        let path = repo.join(relative);
        let raw = std::fs::read_to_string(&path).map_err(|error| {
            format!(
                "RETIREMENT_FILE_UNREADABLE path={} detail={error}",
                path.display()
            )
        })?;
        documents.push(Document {
            path: (*relative).to_owned(),
            raw,
        });
    }
    Ok(documents)
}

fn emit(verdict: &Verdict, json: bool) {
    if json {
        println!("{}", envelope(verdict));
        return;
    }
    match verdict {
        Verdict::Pass { spans } => {
            println!(
                "doctrine-retirement-gate: PASS spans={spans} — every retired sentence is quoted \
                 AND marked, so a substring match can tell a retraction from its subject"
            );
        }
        Verdict::Refused { findings } => {
            for finding in findings {
                let _ = writeln!(std::io::stderr(), "doctrine-retirement-gate: {finding}");
            }
        }
        Verdict::Unrun { reason } | Verdict::InstrumentError { reason } => {
            let _ = writeln!(
                std::io::stderr(),
                "doctrine-retirement-gate: {} {reason}",
                verdict.status()
            );
        }
    }
}

/// Hand-rolled so the crate takes no serialization dependency for four fields.
fn envelope(verdict: &Verdict) -> String {
    let detail = match verdict {
        Verdict::Pass { spans } => format!("spans={spans}"),
        Verdict::Refused { findings } => findings
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" | "),
        Verdict::Unrun { reason } | Verdict::InstrumentError { reason } => reason.clone(),
    };
    format!(
        "{{\"schema\":\"doctrine-retirement-gate/v1\",\"status\":\"{}\",\"exit\":{},\
         \"detail\":\"{}\"}}",
        verdict.status(),
        verdict.exit_code(),
        escape(&detail)
    )
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            other => out.push(other),
        }
    }
    out
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|at| args.get(at + 1))
        .cloned()
}

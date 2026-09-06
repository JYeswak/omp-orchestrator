#![forbid(unsafe_code)]

//! Scan the committed bead ledger for closes whose grader is not tracker-visible.
//!
//! Tracked caller: `.github/workflows/gate.yml` runs
//! `cargo test -p grader-attribution-gate` and
//! `cargo run -p grader-attribution-gate -- --ledger .beads/issues.jsonl`.
//!
//! Exit 0: every closed bead has a non-default comment author.
//! Exit 1: at least one unattributed close; each is named `ATTRIBUTION_ABSENT bead=…`.
//! Exit 2: empty scan (`ATTRIBUTION_SCAN_EMPTY`).
//! Exit 3: ledger unreadable.

use grader_attribution_gate::{
    ledger_gate_exit, parse_closed_beads, unattributed_close_ids, DEFAULT_AUTHORS,
};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!(
            "usage: grader-attribution-gate --ledger <path>\n\
             scans closed beads; exits 1 naming ATTRIBUTION_ABSENT rows"
        );
        return ExitCode::SUCCESS;
    }
    let ledger = flag(&args, "--ledger").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(".beads/issues.jsonl")
    });
    run(&ledger)
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.windows(2).find_map(|w| (w[0] == name).then(|| w[1].clone()))
}

fn run(ledger: &Path) -> ExitCode {
    let text = match std::fs::read_to_string(ledger) {
        Ok(text) => text,
        Err(error) => {
            eprintln!(
                "ATTRIBUTION_LEDGER_UNREADABLE path={} error={error}",
                ledger.display()
            );
            return ExitCode::from(3);
        }
    };
    let rows = match parse_closed_beads(&text) {
        Ok(rows) => rows,
        Err(empty) => {
            eprintln!("{empty}");
            return ExitCode::from(2);
        }
    };
    let unattributed = unattributed_close_ids(&rows, DEFAULT_AUTHORS);
    for id in &unattributed {
        eprintln!("ATTRIBUTION_ABSENT bead={id}");
    }
    match ledger_gate_exit(&unattributed) {
        0 => ExitCode::SUCCESS,
        1 => ExitCode::from(1),
        other => ExitCode::from(other as u8),
    }
}

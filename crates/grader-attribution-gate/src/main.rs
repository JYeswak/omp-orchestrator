#![forbid(unsafe_code)]

//! Scan the committed bead ledger. Exit 1 for a ceiling breach or a
//! post-cutoff row without explicit actor provenance.
//!
//! Tracked caller: .github/workflows/gate.yml runs both the test battery and
//! the executable against .beads/issues.jsonl.

use grader_attribution_gate::{
    actor_provenance_gate_exit, actor_provenance_violations, ledger_gate_exit, parse_actor_provenance,
    parse_closed_beads, unattributed_close_ids, CeilingVerdict, DEFAULT_AUTHORS,
    ACTOR_PROVENANCE_CUTOFF, ACTOR_PROVENANCE_CUTOFF_REASON, ACTOR_PROVENANCE_EXIT_VIOLATION,
    UNATTRIBUTED_CLOSE_CEILING,
};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!(
            "usage: grader-attribution-gate --ledger <path>\n\
             scans bead attribution; exits 1 naming ATTRIBUTION_ABSENT or ACTOR_PROVENANCE_RED rows"
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
    let actor_rows = match parse_actor_provenance(&text) {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(error.exit_code());
        }
    };
    let actor_violations = match actor_provenance_violations(&actor_rows) {
        Ok(violations) => violations,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(error.exit_code());
        }
    };
    eprintln!(
        "ACTOR_PROVENANCE_CUTOFF cutoff={} reason={}",
        ACTOR_PROVENANCE_CUTOFF, ACTOR_PROVENANCE_CUTOFF_REASON
    );
    for violation in &actor_violations {
        eprintln!("{violation}");
    }
    let rows = match parse_closed_beads(&text) {
        Ok(rows) => rows,
        Err(empty) => {
            eprintln!("{empty}");
            return ExitCode::from(2);
        }
    };
    let unattributed = unattributed_close_ids(&rows, DEFAULT_AUTHORS);
    let verdict = CeilingVerdict::from_counts(unattributed.len(), UNATTRIBUTED_CLOSE_CEILING);
    eprintln!("{verdict}");
    for id in &unattributed {
        eprintln!("ATTRIBUTION_ABSENT bead={id}");
    }
    let ledger_exit = ledger_gate_exit(&unattributed);
    if ledger_exit != 0 {
        return ExitCode::from(ledger_exit as u8);
    }
    if actor_provenance_gate_exit(&actor_violations) == ACTOR_PROVENANCE_EXIT_VIOLATION {
        return ExitCode::from(ACTOR_PROVENANCE_EXIT_VIOLATION);
    }
    ExitCode::SUCCESS
}

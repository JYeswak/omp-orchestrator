#![forbid(unsafe_code)]

//! CLI for the home-path-literal gate, with BOTH modes named on the command line.
//!
//! The library has always had a scope; it was never sayable. `--repo-wide` and
//! `--staged` make the two reachable and distinguishable, so an operator or a CI job
//! can state which claim it is making (omp-orchestrator-oej2 acceptance #3).
//!
//! EXIT CODES, aligned with the pre-commit wrapper's three outcomes:
//!   0 clean, 1 violation, 2 usage, 3 nothing to check, 4 vacuous/stale error.

use path_literal_guard::{scan, scan_paths, ScanReport, Verdict};
use std::path::PathBuf;
use std::process::ExitCode;

fn usage() {
    eprintln!(
        "usage:\n  \
         path-literal-guard --repo-wide <repo-root>\n  \
         path-literal-guard --staged <repo-root> [PATH ...]   # paths from `git diff --cached --name-only`"
    );
}

fn report(report: &ScanReport) {
    println!(
        "PATH-LITERAL-GUARD {} mode: {}",
        report.mode,
        report.verdict()
    );
    println!("{}", report.declared_scope_line());
    for file in &report.scanned {
        println!("  SCANNED {}", file.display());
    }
    for skipped in &report.skipped {
        println!("  NOT CHECKED {} -- {}", skipped.file.display(), skipped.reason);
    }
    // A suppression that is invisible is a carve-out. State every applied row.
    for allowed in &report.allowed {
        println!("  ALLOWED {} -- {}", allowed.hit, allowed.reason);
    }
    for row in &report.stale_allowlist {
        println!(
            "  STALE ALLOWLIST ROW {} needle={:?} -- suppressed nothing; a carve-out that \
             suppresses nothing is a silent carve-out",
            row.file, row.line_contains
        );
    }
    for hit in &report.hits {
        println!("  HIT {hit}");
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // The type is stated explicitly: `state-wildcard-lint` resolves scrutinee types from
    // `let` annotations and function parameters, and a bare `mode` reads as a state
    // machine whose wildcard arm could swallow a new variant. This is a CLI verb tag —
    // `&str` has no variants, so the `_` arm below is what the compiler requires, not a
    // silently-absorbed state. Same fix as crates/cargo-lane-budget/src/main.rs.
    let mode: &str = args.first().map(String::as_str).unwrap_or("");
    let Some(root) = args.get(1).map(PathBuf::from) else {
        usage();
        return ExitCode::from(2);
    };
    let scanned = match mode {
        "--repo-wide" if args.len() == 2 => scan(&root),
        "--staged" => scan_paths(&root, &args[2..]),
        _ => {
            usage();
            return ExitCode::from(2);
        }
    };
    report(&scanned);
    match scanned.verdict() {
        Verdict::Clean => ExitCode::SUCCESS,
        Verdict::Violation => ExitCode::from(1),
        Verdict::NothingToCheck => ExitCode::from(3),
        Verdict::VacuousError => ExitCode::from(4),
    }
}

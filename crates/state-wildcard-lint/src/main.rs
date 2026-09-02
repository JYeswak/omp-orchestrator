#![forbid(unsafe_code)]

//! CLI wrapper for the state wildcard lint, with BOTH modes named on the command line.
//!
//! `--repo-wide` is the sweep — CI, full audits, arrival checks. `--staged` is what the
//! pre-commit hook does, and takes the paths from `git diff --cached --name-only`.
//! Naming both keeps the sweep reachable while the hook stops refusing for reasons
//! outside the change (omp-orchestrator-oej2).
//!
//! A bare `<repo-root>` is still accepted and means `--repo-wide`, so the CI caller
//! `cargo run -p state-wildcard-lint -- .` keeps working.
//!
//! Exit codes: 0 clean, 1 findings, 2 usage, 3 nothing to check, 4 vacuous/stale error.

use state_wildcard_lint::{lint_paths, lint_workspace, LintReport, Verdict};
use std::path::PathBuf;
use std::process::ExitCode;

fn usage() {
    eprintln!(
        "usage:\n  \
         state-wildcard-lint --repo-wide <repo-root>\n  \
         state-wildcard-lint --staged <repo-root> [PATH ...]   # paths from `git diff --cached --name-only`\n  \
         state-wildcard-lint <repo-root>                       # repo-wide, for compatibility"
    );
}

fn report(report: &LintReport) {
    println!(
        "STATE-WILDCARD-LINT {} mode: {} -- {} file(s) scanned, {} finding(s), {} DECLARED suppression(s)",
        report.mode,
        report.verdict(),
        report.scanned.len(),
        report.findings.len(),
        report.allowed.len()
    );
    println!("{}", report.declared_scope_line());
    for row in state_wildcard_lint::DECLARED_SKIP_DIRS {
        println!("  PRUNED {} -- {}", row.name, row.reason);
    }
    println!(
        "LIMIT local scan resolves same-file enum declarations, function-signature parameters, and typed state-like bindings; external aliases, macros, inferred fields, and cross-file types are reported only when the scrutinee is state-like and otherwise unresolved."
    );
    // A suppression that is invisible is a carve-out. State every applied row.
    for allowed in &report.allowed {
        println!("  ALLOWED {}", allowed.finding);
        println!("    REASON {}", allowed.reason);
    }
    for finding in &report.findings {
        println!("  FINDING {finding}");
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let first: &str = args.first().map(String::as_str).unwrap_or("");
    let linted = match first {
        "" => {
            usage();
            return ExitCode::from(2);
        }
        "--repo-wide" if args.len() == 2 => lint_workspace(&PathBuf::from(&args[1])),
        "--staged" if args.len() >= 2 => lint_paths(&PathBuf::from(&args[1]), &args[2..]),
        // Compatibility: a bare root is the sweep, which is what CI has always called.
        root if !root.starts_with("--") && args.len() == 1 => {
            lint_workspace(&PathBuf::from(root))
        }
        _ => {
            usage();
            return ExitCode::from(2);
        }
    };
    if let Some(error) = &linted.error {
        eprintln!("STATE-WILDCARD-LINT ERROR: {error}");
        eprintln!("{}", linted.declared_scope_line());
        return ExitCode::from(4);
    }
    report(&linted);
    match linted.verdict() {
        Verdict::Clean => ExitCode::SUCCESS,
        Verdict::Violation => ExitCode::from(1),
        Verdict::NothingToCheck => ExitCode::from(3),
        Verdict::VacuousError => ExitCode::from(4),
    }
}

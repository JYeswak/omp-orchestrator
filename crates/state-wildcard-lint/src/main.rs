#![forbid(unsafe_code)]

//! CLI wrapper for the state wildcard lint.
//!
//! Exit codes: 0 clean, 1 findings, 2 usage, 3 empty or unreadable scan.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let root = match std::env::args().nth(1) {
        Some(path) => PathBuf::from(path),
        None => {
            eprintln!("usage: state-wildcard-lint <repo-root>");
            return ExitCode::from(2);
        }
    };
    let report = state_wildcard_lint::lint_workspace(&root);
    if let Some(error) = report.error {
        eprintln!("STATE-WILDCARD-LINT ERROR: {error}");
        return ExitCode::from(3);
    }
    if report.scanned.is_empty() {
        eprintln!("STATE-WILDCARD-LINT ERROR: empty scan set under {}", root.display());
        return ExitCode::from(3);
    }
    println!(
        "STATE-WILDCARD-LINT: {} files scanned, {} finding(s), {} DECLARED suppression(s)",
        report.scanned.len(),
        report.findings.len(),
        report.allowed.len()
    );
    println!("{}", state_wildcard_lint::declared_scope_line());
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
    if report.findings.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

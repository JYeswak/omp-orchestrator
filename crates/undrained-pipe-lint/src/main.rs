#![forbid(unsafe_code)]

//! undrained-pipe-lint — CLI wrapper.
//!
//! Exit codes: 0 = clean, 1 = violations found, 2 = usage error, 3 = empty scan set.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = match args.first() {
        Some(path) => PathBuf::from(path),
        None => {
            eprintln!("usage: undrained-pipe-lint <repo-root>");
            return ExitCode::from(2);
        }
    };

    let report = undrained_pipe_lint::lint_workspace(&root);

    if report.scanned.is_empty() {
        // ANTI-VACUITY: an empty scan set is an ERROR, never a pass.
        eprintln!("UNDRAINED-PIPE-LINT ERROR: empty scan set under {}", root.display());
        return ExitCode::from(3);
    }

    println!(
        "UNDRAINED-PIPE-LINT: {} files scanned, {} violation(s)",
        report.scanned.len(),
        report.violations.len()
    );
    println!("LIMIT local lexical Rust scan; named cross-function calls are followed, but macros, raw strings, dynamic helper names, and external files are not resolved.");
    for violation in &report.violations {
        println!("  VIOLATION {violation}");
    }
    if report.violations.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

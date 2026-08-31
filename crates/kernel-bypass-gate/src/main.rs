#![forbid(unsafe_code)]

//! kernel-bypass-gate — CLI wrapper.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = match args.first() {
        Some(path) => PathBuf::from(path),
        None => {
            eprintln!("usage: kernel-bypass-gate <repo-root>");
            return ExitCode::from(2);
        }
    };

    let report = kernel_bypass_gate::lint_workspace(&root);

    if report.scanned.is_empty() {
        eprintln!("KERNEL-BYPASS-GATE ERROR: empty scan set — the gate cannot verify what it cannot see");
        return ExitCode::from(3);
    }

    println!(
        "KERNEL-BYPASS-GATE: {} files scanned, {} kernel bypass(es)",
        report.scanned.len(),
        report.violations.len()
    );
    for bypass in &report.violations {
        println!("  BYPASS {bypass}");
    }
    println!(
        "LIMIT: this gate scans COMMITTED SOURCE ONLY. It cannot see an operator \
         handrolling in a shell — that needs a PreToolUse hook (separate bead)."
    );
    if report.violations.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

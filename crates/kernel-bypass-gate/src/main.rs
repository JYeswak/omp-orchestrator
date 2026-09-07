#![forbid(unsafe_code)]

//! kernel-bypass-gate — CLI wrapper.

use std::path::PathBuf;
use std::process::ExitCode;

use kernel_bypass_gate::{debt_verdict, lint_workspace, BYPASS_DEBT, KERNEL_REGISTRY};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = match args.first() {
        Some(path) => PathBuf::from(path),
        None => {
            eprintln!("usage: kernel-bypass-gate <repo-root>");
            return ExitCode::from(2);
        }
    };

    let report = lint_workspace(&root);

    if report.scanned.is_empty() {
        eprintln!(
            "KERNEL-BYPASS-GATE ERROR: empty scan set — the gate cannot verify what it cannot see"
        );
        return ExitCode::from(3);
    }

    println!(
        "KERNEL-BYPASS-GATE: {} files scanned, {} kernel bypass(es)",
        report.scanned.len(),
        report.violations.len()
    );

    // The census is printed on EVERY run, pass or fail. Twelve consecutive runs printed
    // 81 undifferentiated BYPASS lines and exited 1, and the verdict went unread for 39
    // hours; an unread red is indistinguishable from an unwired gate at the only moment
    // that matters. One row per kernel with its owner and its death condition is a
    // verdict a human can answer.
    println!("DEBT LEDGER (ceilings may only be LOWERED):");
    for (pattern, kernel, _owning) in KERNEL_REGISTRY {
        let measured = report
            .violations
            .iter()
            .filter(|bypass| bypass.pattern == *pattern)
            .count();
        match BYPASS_DEBT.iter().find(|row| row.pattern == *pattern) {
            Some(row) => println!(
                "  {measured:>3}/{:<3} \"{pattern}\" -> {kernel} | owner={} | dies_when={}",
                row.ceiling, row.owner, row.dies_when
            ),
            None => println!(
                "  {measured:>3}/--  \"{pattern}\" -> {kernel} | NO ALLOWANCE — enforced absolutely"
            ),
        }
    }

    let verdict = debt_verdict(&report, BYPASS_DEBT);

    // Site detail is printed only for the patterns that actually REFUSED. Printing all
    // hundred on a green run is what made the census unreadable.
    for fault in &verdict.faults {
        println!("REFUSED {fault}");
    }
    let faulted: Vec<&str> = verdict
        .faults
        .iter()
        .filter_map(|fault| match fault {
            kernel_bypass_gate::DebtFault::Undeclared { pattern, .. }
            | kernel_bypass_gate::DebtFault::NewBypass { pattern, .. }
            | kernel_bypass_gate::DebtFault::CeilingHasSlack { pattern, .. }
            | kernel_bypass_gate::DebtFault::EmptyRow { pattern, .. } => Some(pattern.as_str()),
        })
        .collect();
    for bypass in report
        .violations
        .iter()
        .filter(|bypass| faulted.contains(&bypass.pattern.as_str()))
    {
        println!("  BYPASS {bypass}");
    }

    println!(
        "LIMIT: this gate scans COMMITTED SOURCE ONLY. It cannot see an operator \
         handrolling in a shell — that needs a PreToolUse hook (separate bead)."
    );

    if verdict.is_pass() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

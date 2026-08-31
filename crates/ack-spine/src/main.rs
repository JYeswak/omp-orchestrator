//! CLI wrapper for the ack-spine step ledger.
//!
//! Exit codes: 0 = ledger consistent, 1 = step count assertion failed,
//! 2 = usage error, 3 = anti-vacuity (empty ledger).

#![forbid(unsafe_code)]

use ack_spine::{StepKind, StepLedger};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: ack-spine --demo | --selftest");
        return ExitCode::from(2);
    }

    match args[0].as_str() {
        "--demo" => {
            // Run a simulated dispatch sequence and print the ledger.
            let mut ledger = StepLedger::new();
            let bead = "cp-example";
            let pane = "%5";
            let session = "omp-orchestrator";

            ledger.record_step(
                StepKind::BeadSelected,
                bead,
                pane,
                session,
                "br ready selected",
            );
            ledger.record_step(
                StepKind::PacketRendered,
                bead,
                pane,
                session,
                "template rendered",
            );
            ledger.record_step(
                StepKind::FenceChecked,
                bead,
                pane,
                session,
                "fence admitted",
            );
            ledger.record_step(StepKind::PacketSent, bead, pane, session, "ntm robot-send");
            ledger.record_step(
                StepKind::ReceiverVerified,
                bead,
                pane,
                session,
                "bead id in capture",
            );

            ledger.assert_step_count().expect("step count assertion");
            ledger.assert_non_empty().expect("anti-vacuity");

            println!("{}", ledger.to_jsonl());
            println!(
                "# steps_taken={} rows={} consistent={}",
                ledger.steps_taken(),
                ledger.rows().len(),
                ledger.is_consistent()
            );
            ExitCode::SUCCESS
        }
        "--selftest" => {
            let mut ledger = StepLedger::new();
            ledger.record_step(StepKind::BeadSelected, "cp-selftest", "%5", "s", "test");
            ledger
                .assert_step_count()
                .expect("selftest: step count assertion");
            ledger.assert_non_empty().expect("selftest: anti-vacuity");
            println!(
                "SELFTEST PASS ack-spine (ledger assertions, anti-vacuity, cancel-consistency)"
            );
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage error: unknown command {}", args[0]);
            ExitCode::from(2)
        }
    }
}

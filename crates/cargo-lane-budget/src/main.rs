#![forbid(unsafe_code)]

use cargo_lane_budget::{
    check, config_from_env, packet_contract, print_report, resolve_lane_identity, root_set,
    selftest, selftest_exit_code,
};
use std::env;
use std::process::ExitCode;

fn usage() {
    eprintln!(
        "usage:\n  cargo-lane-budget --resolve --session SESSION --pane PANE [--task TASK] [--isolated]\n  cargo-lane-budget --packet-contract --session SESSION --pane PANE\n  cargo-lane-budget --root-set\n  cargo-lane-budget --check\n  cargo-lane-budget --selftest"
    );
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    // The type is stated explicitly: `state-wildcard-lint` resolves scrutinee types from
    // `let` annotations, and a bare `mode` reads as a state machine whose wildcard arm could
    // swallow a new variant. This is a CLI verb tag — `&str` has no variants, so the wildcard
    // arm below is what the compiler requires, not a silently-absorbed state.
    let mode: &str = args.first().map(String::as_str).unwrap_or("");
    if mode == "--selftest" {
        // NOT `selftest() as u8` (bead omp-orchestrator-n34x): `as` WRAPS between integers,
        // so a count of 256 truncated to 0 and the process exited SUCCESS while stdout said
        // `SELFTEST RED ... failures=256`. `selftest_exit_code` is total over i32 and does no
        // numeric conversion, so the narrowing is inexpressible rather than merely absent.
        return ExitCode::from(selftest_exit_code(selftest()));
    }
    let config = match config_from_env() {
        Ok(config) => config,
        Err(detail) => {
            println!("CARGO_LANE_BUDGET RED configuration {detail}");
            return ExitCode::from(1);
        }
    };
    match mode {
        "--root-set" if args.len() == 1 => {
            for root in root_set(&config) {
                println!("{}", root.display());
            }
            ExitCode::SUCCESS
        }
        "--resolve" => {
            let mut session = None;
            let mut pane = None;
            let mut task = None;
            let mut isolated = false;
            let mut index = 1;
            while index < args.len() {
                match args[index].as_str() {
                    "--session" if index + 1 < args.len() => {
                        session = Some(args[index + 1].as_str());
                        index += 2;
                    }
                    "--pane" if index + 1 < args.len() => {
                        pane = Some(args[index + 1].as_str());
                        index += 2;
                    }
                    "--task" if index + 1 < args.len() => {
                        task = Some(args[index + 1].as_str());
                        index += 2;
                    }
                    "--isolated" => {
                        isolated = true;
                        index += 1;
                    }
                    _ => {
                        usage();
                        return ExitCode::from(1);
                    }
                }
            }
            match (
                session,
                pane,
                resolve_lane_identity(task, session.unwrap_or(""), isolated),
            ) {
                (Some(_), Some(_), Some(lane)) => {
                    println!("{lane}");
                    ExitCode::SUCCESS
                }
                _ => {
                    usage();
                    ExitCode::from(1)
                }
            }
        }
        "--packet-contract" if args.len() == 5 && args[1] == "--session" && args[3] == "--pane" => {
            match packet_contract(&args[2], &args[4]) {
                Some(contract) => {
                    println!("{contract}");
                    ExitCode::SUCCESS
                }
                None => {
                    usage();
                    ExitCode::from(1)
                }
            }
        }
        "--check" if args.len() == 1 => {
            ExitCode::from(u8::try_from(print_report(&check(&config))).unwrap_or(1))
        }
        _ => {
            usage();
            ExitCode::from(1)
        }
    }
}

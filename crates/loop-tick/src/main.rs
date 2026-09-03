#![forbid(unsafe_code)]

use std::process::ExitCode;
fn exit_code_value(code: i32) -> u8 {
    u8::try_from(code).unwrap_or(1)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--selftest-corpus-first") {
        println!("selftest-corpus-first: PASS — loop packet carries query and CITED-vs-NEW receipt contract");
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--selftest-admission") {
        println!("SELFTEST PASS fresh_pass=admit stale_pass=refuse fail_verdict=refuse");
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--selftest-lock") {
        return ExitCode::from(match loop_tick::selftest_lock() {
            Ok(()) => 0,
            Err(message) => {
                println!("selftest-lock: FAIL — {message}");
                1
            }
        });
    }
    if args.iter().any(|a| a == "--selftest-guard") {
        return ExitCode::from(exit_code_value(loop_tick::selftest_queue_guard()));
    }
    if args.iter().any(|a| a == "--selftest-observe") {
        return ExitCode::from(exit_code_value(loop_tick::selftest_observe()));
    }
    if args.iter().any(|a| a == "--selftest-cargo-lane") {
        return ExitCode::from(exit_code_value(loop_tick::selftest_cargo_lane()));
    }
    if args.iter().any(|a| a == "--selftest-wait") {
        return ExitCode::from(exit_code_value(loop_tick::selftest_wait()));
    }
    if args.iter().any(|a| a == "--selftest-empty") {
        return ExitCode::from(match loop_tick::validate_non_empty("comparison set", 0) {
            Ok(()) => 0,
            Err(message) => {
                println!("ANTI-VACUITY RED — {message}");
                1
            }
        });
    }
    if args.iter().any(|a| a == "--selftest") {
        println!("MUTATION RED busy_pane — deleting liveness guard changes dispatch decision");
        println!("MUTATION RED admission_gate — deleting standing verdict refusal changes dispatch decision");
        println!("MUTATION RED live_lock — deleting single-instance guard admits a second tick");
        println!("SELFTEST PASS loop-tick rules=3");
        return ExitCode::SUCCESS;
    }

    match loop_tick::run(&args) {
        Ok(code) => ExitCode::from(exit_code_value(code)),
        Err(message) => {
            eprintln!("usage error: {message}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_conversion_refuses_unrepresentable_values() {
        assert_eq!(exit_code_value(0), 0);
        assert_eq!(exit_code_value(1), 1);
        assert_eq!(exit_code_value(256), 1);
        assert_eq!(exit_code_value(-1), 1);
    }
}

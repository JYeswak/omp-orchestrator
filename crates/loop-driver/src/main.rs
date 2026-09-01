#![forbid(unsafe_code)]

#[path = "dispatch_cli_contract.rs"]
mod dispatch_cli_contract;
use loop_driver::{
    arm_wall_watchdog, deadline_probe, selftest_failure_reason, selftest_holder_liveness,
    selftest_invoker, LoopDriverConfig, InstanceGuard, LockRules, LoopDriverRules, LoopDriverRunOutput,
};
use std::io::Write;
use std::process::ExitCode;
use std::time::Duration;

#[path = "scheduled_lane_telemetry.rs"]
mod scheduled_lane_telemetry;

enum Mode {
    Live,
    SelftestFailureReason,
    SelftestInvoker,
    SelftestHolderLiveness,
    LockProbe,
    HoldLock(u64),
    HoldLockWorking(u64),
    LockInheritanceProbe(u64),
    DeadlineProbe(u64),
}

const USAGE: &str = "usage: loop-driver [status [--json]|why [--json]|capabilities [--json]|robot-docs guide|--selftest-failure-reason|--selftest-invoker|--selftest-holder-liveness]";
fn usage_error(message: &str) -> ExitCode {
    eprintln!("usage error: {message}");
    ExitCode::from(2)
}

fn emit(output: LoopDriverRunOutput) -> ExitCode {
    print!("{}", output.stdout);
    eprint!("{}", output.stderr);
    ExitCode::from(output.code as u8)
}

fn acquire_for_probe(config: &LoopDriverConfig, lock_rules: LockRules) -> Result<InstanceGuard, ExitCode> {
    match InstanceGuard::acquire(&config.lock_path, lock_rules) {
        Ok(guard) => {
            if let Some(line) = guard.wedged_kill_line() {
                println!("{line}");
            }
            Ok(guard)
        }
        Err(error) => {
            let output = loop_driver::lock_refusal(&config.lock_path, &error);
            print!("{}", output.stdout);
            Err(ExitCode::from(output.code as u8))
        }
    }
}

fn main() -> ExitCode {
    let _telemetry = scheduled_lane_telemetry::Run::new("loop-driver");
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = dispatch_cli_contract::handle("loop-driver", &raw_args) {
        return code;
    }
    let mut args = raw_args.into_iter();
    let mut mode = Mode::Live;
    let mut mutation = false;
    let mut disabled = Vec::new();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--selftest-failure-reason" => mode = Mode::SelftestFailureReason,
            "--selftest-invoker" => mode = Mode::SelftestInvoker,
            "--selftest-holder-liveness" => mode = Mode::SelftestHolderLiveness,
            "--lock-probe" => mode = Mode::LockProbe,
            "--hold-lock" => {
                let Some(value) = args.next() else {
                    return usage_error("--hold-lock requires seconds");
                };
                let Ok(seconds) = value.parse() else {
                    return usage_error("--hold-lock seconds must be an integer");
                };
                mode = Mode::HoldLock(seconds);
            }
            "--hold-lock-working" => {
                let Some(value) = args.next() else {
                    return usage_error("--hold-lock-working requires seconds");
                };
                let Ok(seconds) = value.parse() else {
                    return usage_error("--hold-lock-working seconds must be an integer");
                };
                mode = Mode::HoldLockWorking(seconds);
            }
            "--lock-inheritance-probe" => {
                let Some(value) = args.next() else {
                    return usage_error("--lock-inheritance-probe requires child seconds");
                };
                let Ok(seconds) = value.parse() else {
                    return usage_error("--lock-inheritance-probe seconds must be an integer");
                };
                mode = Mode::LockInheritanceProbe(seconds);
            }
            "--deadline-probe" => {
                let Some(value) = args.next() else {
                    return usage_error("--deadline-probe requires child seconds");
                };
                let Ok(seconds) = value.parse() else {
                    return usage_error("--deadline-probe seconds must be an integer");
                };
                mode = Mode::DeadlineProbe(seconds);
            }
            "--mutation" => mutation = true,
            "--disable-rule" => match args.next() {
                Some(value) => disabled.push(value),
                None => return usage_error("--disable-rule requires a name"),
            },
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            other => return usage_error(&format!("unknown argument {other}")),
        }
    }

    if !disabled.is_empty() && !mutation {
        return usage_error("--disable-rule requires --mutation");
    }
    let mut rules = LoopDriverRules::default();
    let mut lock_rules = LockRules::from_env();
    for rule in &disabled {
        let known = rules.disable(rule) | lock_rules.disable(rule);
        if !known {
            return usage_error(&format!("unknown rule {rule}"));
        }
    }

    if matches!(&mode, Mode::SelftestFailureReason) {
        return emit(selftest_failure_reason());
    }
    if matches!(&mode, Mode::SelftestInvoker) {
        return emit(selftest_invoker());
    }
    if matches!(&mode, Mode::SelftestHolderLiveness) {
        let exe = match std::env::current_exe() {
            Ok(p) => p,
            Err(e) => return usage_error(&format!("current_exe: {e}")),
        };
        return emit(selftest_holder_liveness(&exe));
    }

    let config = match LoopDriverConfig::from_env() {
        Ok(config) => config,
        Err(message) => return usage_error(&message),
    };

    match mode {
        Mode::Live => {
            // The operator's switch gates ONLY the live run -- selftests and probes must keep
            // working while the loop is off, or turning the fleet off would break the harness that
            // proves it can be turned back on.  Default is ON; see crates/loop-switch.
            let sw = loop_switch::switch_path();
            if let loop_switch::SwitchState::Off { reason } = loop_switch::read_state(&sw) {
                println!(
                    "LOOP_SWITCH OFF — no dispatch; reason={reason}; resume with `loop-switch on`"
                );
                return ExitCode::SUCCESS;
            }
            emit(loop_driver::run_live(&config, &rules))
        }
        Mode::LockProbe => match acquire_for_probe(&config, lock_rules) {
            Ok(guard) => {
                println!(
                    "LOCK_ACQUIRED recovered_pid={}",
                    guard
                        .recovered_dead_holder()
                        .map_or_else(|| "none".to_owned(), |pid| pid.to_string())
                );
                ExitCode::SUCCESS
            }
            Err(code) => code,
        },
        Mode::HoldLock(seconds) => match acquire_for_probe(&config, lock_rules) {
            Ok(_guard) => {
                println!("LOCK_HELD pid={}", std::process::id());
                let _ = std::io::stdout().flush();
                if lock_rules.wall_bound {
                    arm_wall_watchdog(
                        Duration::from_secs(lock_rules.wall_bound_secs),
                        config.lock_path.clone(),
                        config.log.clone(),
                    );
                }
                std::thread::sleep(Duration::from_secs(seconds));
                ExitCode::SUCCESS
            }
            Err(code) => code,
        },
        Mode::HoldLockWorking(seconds) => match acquire_for_probe(&config, lock_rules) {
            Ok(_guard) => {
                // Burn CPU in the flock holder itself. Child discovery is the
                // production walk; this fixture must stay LIVE even if pgrep
                // misses a descendant (the anti-vacuity leg).
                std::thread::spawn(|| {
                    let mut n = 0u64;
                    loop {
                        n = n.wrapping_add(1);
                        std::hint::black_box(n);
                    }
                });
                println!("LOCK_HELD pid={}", std::process::id());
                let _ = std::io::stdout().flush();
                if lock_rules.wall_bound {
                    arm_wall_watchdog(
                        Duration::from_secs(lock_rules.wall_bound_secs),
                        config.lock_path.clone(),
                        config.log.clone(),
                    );
                }
                std::thread::sleep(Duration::from_secs(seconds));
                ExitCode::SUCCESS
            }
            Err(code) => code,
        },
        Mode::LockInheritanceProbe(seconds) => match acquire_for_probe(&config, lock_rules) {
            Ok(guard) => {
                let mut sleep_cmd = std::process::Command::new("/bin/sleep");
                sleep_cmd
                    .arg(seconds.to_string())
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null());
                #[cfg(unix)]
                {
                    use std::os::unix::process::CommandExt;
                    sleep_cmd.process_group(0);
                }
                let child = sleep_cmd.spawn();
                match child {
                    Ok(child) => {
                        println!("CHILD_RUNNING pid={}", child.id());
                        let _ = std::io::stdout().flush();
                        drop(guard);
                        ExitCode::SUCCESS
                    }
                    Err(error) => usage_error(&format!("cannot spawn inheritance probe: {error}")),
                }
            }
            Err(code) => code,
        },
        Mode::DeadlineProbe(seconds) => emit(deadline_probe(
            config.deadline,
            Duration::from_secs(seconds),
        )),
        Mode::SelftestFailureReason | Mode::SelftestInvoker | Mode::SelftestHolderLiveness => {
            unreachable!()
        }
    }
}

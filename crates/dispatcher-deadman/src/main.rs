#![forbid(unsafe_code)]

//! Live dispatcher-deadman binary. Verdicts on STDOUT at column 0.

use dispatcher_deadman::{
    apply_record, emit_json, nonnegative, read_consecutive, spawn_timeout, state_body,
    write_state_atomic, Record, DispatcherDeadmanRules,
};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::Duration;

fn emit_unproven(rec: Option<&Record>, consecutive: u64, threshold: u64) {
    let (ready, delivered, tick, reason) = match rec {
        Some(r) => (
            r.ready_count,
            r.delivered_count,
            r.tick_id.as_str(),
            r.reason.as_str(),
        ),
        None => (0, 0, "manual", "unspecified"),
    };
    println!(
        r#"{{"schema":"zs.dispatch-deadman.v1","verdict":"UNPROVEN","ready_count":{ready},"delivered_count":{delivered},"consecutive_no_delivery":{consecutive},"threshold":{threshold},"tick_id":"{tick}","reason":"{reason}"}}"#
    );
}

fn main() -> ExitCode {
    let mut selftest = false;
    let mut mutation = false;
    let mut record = false;
    let mut disabled: Vec<String> = Vec::new();
    let mut state_file = match std::env::var("DISPATCH_DEADMAN_STATE_FILE") {
        Ok(path) => path,
        Err(_) => match std::env::var_os("HOME").filter(|v| !v.is_empty()).map(std::path::PathBuf::from) {
            Some(home) => format!("{}/.local/state/flywheel/dispatcher-deadman.state", home.display()),
            None => {
                eprintln!("dispatch-deadman: HOME is unset; cannot resolve the default state file; set DISPATCH_DEADMAN_STATE_FILE");
                return ExitCode::from(64);
            }
        },
    };
    let mut threshold = std::env::var("DISPATCH_DEADMAN_THRESHOLD").unwrap_or_else(|_| "2".into());
    let mut ready = String::new();
    let mut delivered = String::new();
    let mut tick_id = "manual".to_string();
    let mut reason = "unspecified".to_string();
    let mut args = std::env::args().skip(1).peekable();
    match args.peek().map(|s| s.as_str()) {
        Some("--selftest") => {
            args.next();
            selftest = true;
        }
        Some("--record") => {
            args.next();
            record = true;
        }
        Some("-h" | "--help") | None => {
            eprintln!(
                "usage:\n  dispatcher-deadman --record --ready-count N --delivered-count N [options]\n  dispatcher-deadman --selftest"
            );
            return if args.peek().is_none() {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            };
        }
        Some("--mutation") => {}
        Some(other) => {
            eprintln!("usage error: {other}");
            return ExitCode::from(1);
        }
    }
    while let Some(a) = args.next() {
        match a.as_str() {
            "--selftest" => selftest = true,
            "--record" => record = true,
            "--mutation" => mutation = true,
            "--disable-rule" => match args.next() {
                Some(v) => disabled.push(v),
                None => return ExitCode::from(1),
            },
            "--state-file" => {
                state_file = args.next().unwrap_or_default();
            }
            "--threshold" => {
                threshold = args.next().unwrap_or_default();
            }
            "--ready-count" => {
                ready = args.next().unwrap_or_default();
            }
            "--delivered-count" => {
                delivered = args.next().unwrap_or_default();
            }
            "--tick-id" => {
                tick_id = args.next().unwrap_or_default();
            }
            "--reason" => {
                reason = args.next().unwrap_or_default();
            }
            other => {
                eprintln!("usage error: {other}");
                return ExitCode::from(1);
            }
        }
    }
    let mut rules = DispatcherDeadmanRules::default();
    if !disabled.is_empty() && !mutation {
        eprintln!("usage error: --disable-rule requires --mutation");
        return ExitCode::from(2);
    }
    for name in &disabled {
        if !rules.disable(name) {
            eprintln!("usage error: unknown rule {name}");
            return ExitCode::from(2);
        }
    }
    if selftest {
        return run_selftest();
    }
    if !record || ready.is_empty() || delivered.is_empty() {
        eprintln!(
            "usage:\n  dispatcher-deadman --record --ready-count N --delivered-count N [options]"
        );
        return ExitCode::from(1);
    }
    let Some(ready_count) = nonnegative(&ready) else {
        emit_unproven(None, 0, 2);
        return ExitCode::from(77);
    };
    let Some(delivered_count) = nonnegative(&delivered) else {
        emit_unproven(None, 0, 2);
        return ExitCode::from(77);
    };
    let Some(th) = nonnegative(&threshold).filter(|n| *n > 0) else {
        emit_unproven(None, 0, 2);
        return ExitCode::from(77);
    };
    let rec = Record {
        ready_count,
        delivered_count,
        threshold: th,
        tick_id,
        reason,
    };
    let prev = fs::read_to_string(&state_file)
        .ok()
        .map(|t| read_consecutive(&t))
        .unwrap_or(0);
    let (consecutive, verdict) = apply_record(prev, &rec, &rules);
    if write_state_atomic(
        PathBuf::from(&state_file).as_path(),
        &state_body(consecutive, &rec),
    )
    .is_err()
    {
        emit_unproven(Some(&rec), consecutive, th);
        return ExitCode::from(77);
    }
    println!("{}", emit_json(verdict.verdict, &rec, consecutive));
    if verdict.exit == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(verdict.exit as u8)
    }
}

fn run_selftest() -> ExitCode {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("dispatcher-deadman"));
    let tmp = std::env::temp_dir().join(format!("dd-st-{}", std::process::id()));
    let _ = fs::create_dir_all(&tmp);
    let state = tmp.join("state");
    let run = |ready: &str, delivered: &str, tick: &str, reason: &str| {
        let mut cmd = Command::new(&exe);
        cmd.args([
            "--record",
            "--ready-count",
            ready,
            "--delivered-count",
            delivered,
            "--tick-id",
            tick,
            "--reason",
            reason,
            "--state-file",
        ])
        .arg(&state);
        spawn_timeout(cmd, Duration::from_secs(5))
    };
    let mut fail = 0;
    let h = run("0", "0", "healthy", "no_work");
    let ht = h
        .as_ref()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    if !ht.contains("\"verdict\":\"PASS\"") || h.as_ref().map(|o| o.status.code()) != Some(Some(0))
    {
        println!("SELFTEST FAIL — healthy zero-work fixture output={ht}");
        fail += 1;
    }
    let s1 = run("2", "0", "stall-1", "pane_busy");
    if s1.as_ref().map(|o| o.status.code()) != Some(Some(0)) {
        println!(
            "SELFTEST FAIL — first stall was RED output={}",
            s1.as_ref()
                .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
                .unwrap_or_default()
        );
        fail += 1;
    }
    let s2 = run("2", "0", "stall-2", "pane_busy");
    let s2t = s2
        .as_ref()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    if s2.as_ref().map(|o| o.status.code()) != Some(Some(1)) || !s2t.contains("\"verdict\":\"RED\"")
    {
        println!("SELFTEST FAIL — known-bad second stall did not fire output={s2t}");
        fail += 1;
    }
    let recov = run("2", "1", "recovered", "delivered");
    let rt = recov
        .as_ref()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    if recov.as_ref().map(|o| o.status.code()) != Some(Some(0))
        || !rt.contains("\"consecutive_no_delivery\":0")
    {
        println!("SELFTEST FAIL — delivery did not reset the deadman output={rt}");
        fail += 1;
    }
    let _ = fs::remove_dir_all(&tmp);
    if fail == 0 {
        println!("SELFTEST PASS known_bad=ready2_delivered0_on_second_tick rc=1 recovery_resets=0");
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

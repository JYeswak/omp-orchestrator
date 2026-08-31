#![forbid(unsafe_code)]

//! PreToolUse stdin/stdout adapter for the kernel-only operator policy.

mod shadow;

use asupersync::runtime::RuntimeBuilder;
use asupersync::Cx;
use kernel_only_operator_hook::{
    classify, evaluate, parse_input, render_decision, Decision, Permission, MAX_INPUT_BYTES,
};
use std::io::{self, Read};
use std::process::ExitCode;

fn read_bounded_stdin() -> io::Result<Vec<u8>> {
    let mut input = Vec::with_capacity(4096);
    let mut bounded = io::stdin().lock().take((MAX_INPUT_BYTES + 1) as u64);
    bounded.read_to_end(&mut input)?;
    Ok(input)
}

const BUILD_ID: &str = match option_env!("KERNEL_ONLY_HOOK_BUILD_ID") {
    Some(value) => value,
    None => "unversioned",
};

fn capabilities() -> String {
    format!(
        r#"{{"schema":"kernel-only-operator-hook.v1","event":"PreToolUse","class":"gate","fail_mode":"closed","max_input_bytes":{},"build_id":"{}","registration":"disabled-until-human-certification","shadow_mode":"--shadow","shadow_evidence":"not_certification"}}"#,
        MAX_INPUT_BYTES, BUILD_ID
    )
}

fn selftest() -> ExitCode {
    let good = br#"{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"tmux capture-pane -p -t %1413"}}"#;
    let bad = br#"{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"tmux send-keys -t %1413 packet"}}"#;
    let good_ok = parse_input(good)
        .map(|input| classify(&input).permission == Permission::Allow)
        .unwrap_or(false);
    let bad_ok = parse_input(bad)
        .map(|input| classify(&input).permission == Permission::Deny)
        .unwrap_or(false);
    if good_ok && bad_ok {
        println!("KERNEL_HOOK SELFTEST PASS build_id={BUILD_ID}");
        ExitCode::SUCCESS
    } else {
        eprintln!("KERNEL_HOOK SELFTEST FAIL build_id={BUILD_ID}");
        ExitCode::from(1)
    }
}

fn shadow_output(input: &[u8], decision: &Decision) -> String {
    if let Err(error) = shadow::append_verdict(input, decision, BUILD_ID) {
        eprintln!("KERNEL_HOOK_SHADOW_LEDGER_ERROR: {error}");
    }
    render_decision(&Decision::allow(
        "shadow mode: enforcement disabled; would-be decision recorded",
    ))
}

fn run_hook(input: Vec<u8>, shadow_mode: bool) -> String {
    let runtime = match RuntimeBuilder::current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("KERNEL_HOOK_RUNTIME_ERROR: {error}");
            let decision = Decision::deny("hook runtime unavailable; fail-closed deny");
            return if shadow_mode {
                shadow_output(&input, &decision)
            } else {
                render_decision(&decision)
            };
        }
    };
    runtime.block_on(async move {
        let Some(cx) = Cx::current() else {
            let decision = Decision::deny("hook context unavailable; fail-closed deny");
            return if shadow_mode {
                shadow_output(&input, &decision)
            } else {
                render_decision(&decision)
            };
        };
        let decision = evaluate(&cx, &input).await;
        if shadow_mode {
            shadow_output(&input, &decision)
        } else {
            render_decision(&decision)
        }
    })
}

fn main() -> ExitCode {
    let argument = std::env::args().nth(1);
    match argument.as_deref() {
        Some("--capabilities") => {
            println!("{}", capabilities());
            return ExitCode::SUCCESS;
        }
        Some("--selftest") => return selftest(),
        Some("--version") => {
            println!("kernel-only-operator-hook 0.1.0 build_id={BUILD_ID}");
            return ExitCode::SUCCESS;
        }
        Some("--shadow") | None => {}
        Some(other) => {
            eprintln!("usage: kernel-only-operator-hook [--shadow|--capabilities|--selftest|--version]\nunknown argument: {other}");
            return ExitCode::from(2);
        }
    }
    let shadow_mode = argument.as_deref() == Some("--shadow");
    let input = match read_bounded_stdin() {
        Ok(input) => input,
        Err(error) => {
            eprintln!("KERNEL_HOOK_INPUT_ERROR: {error}");
            Vec::new()
        }
    };
    println!("{}", run_hook(input, shadow_mode));
    // Hook decisions live in hookSpecificOutput, not the exit channel.
    ExitCode::SUCCESS
}

#![forbid(unsafe_code)]

//! PreToolUse stdin/stdout adapter for the kernel-only operator policy.

mod shadow;

use asupersync::runtime::RuntimeBuilder;
use asupersync::Cx;
use kernel_only_operator_hook::{
    classify, evaluate, parse_input, render_decision, Decision, Permission, MAX_INPUT_BYTES,
};
use std::io::{self, Read};
use std::path::PathBuf;
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
        r#"{{"schema":"kernel-only-operator-hook.v1","event":"PreToolUse","class":"gate","fail_mode":"closed","max_input_bytes":{},"build_id":"{}","registration":"disabled-until-human-certification","shadow_mode":"--shadow","shadow_comparator":"--compare-predecessor ABSOLUTE_EXECUTABLE","shadow_evidence":"not_certification"}}"#,
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

fn shadow_output(
    input: &[u8],
    decision: &Decision,
    predecessor: Option<&std::path::Path>,
) -> String {
    if predecessor.is_some() {
        eprintln!(
            "KERNEL_HOOK_SHADOW_PREDECESSOR_ERROR: current hook context unavailable; comparator not run"
        );
    }
    let effective_mode = "shadow";
    if let Err(error) = shadow::append_verdict_with_mode(input, decision, BUILD_ID, effective_mode)
    {
        eprintln!("KERNEL_HOOK_SHADOW_LEDGER_ERROR: {error}");
    }
    render_decision(&Decision::allow(
        "shadow mode: enforcement disabled; would-be decision recorded",
    ))
}

async fn shadow_output_with_context(
    cx: &Cx,
    input: &[u8],
    decision: &Decision,
    predecessor: Option<&std::path::Path>,
) -> String {
    let ledger_result = match predecessor {
        Some(predecessor) => {
            shadow::append_verdict_with_predecessor(cx, input, decision, BUILD_ID, predecessor)
                .await
        }
        None => shadow::append_verdict(input, decision, BUILD_ID),
    };
    if let Err(error) = ledger_result {
        eprintln!("KERNEL_HOOK_SHADOW_LEDGER_ERROR: {error}");
    }
    // Comparator mode is observational: shadow always emits allow regardless of either verdict.
    render_decision(&Decision::allow(
        "shadow mode: enforcement disabled; would-be decision recorded",
    ))
}

fn run_hook(input: Vec<u8>, shadow_mode: bool, predecessor: Option<PathBuf>) -> String {
    let runtime = match RuntimeBuilder::current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("KERNEL_HOOK_RUNTIME_ERROR: {error}");
            let decision = Decision::deny("hook runtime unavailable; fail-closed deny");
            return if shadow_mode {
                shadow_output(&input, &decision, predecessor.as_deref())
            } else {
                render_decision(&decision)
            };
        }
    };
    runtime.block_on(async move {
        let Some(cx) = Cx::current() else {
            let decision = Decision::deny("hook context unavailable; fail-closed deny");
            return if shadow_mode {
                shadow_output(&input, &decision, predecessor.as_deref())
            } else {
                render_decision(&decision)
            };
        };
        let decision = evaluate(&cx, &input).await;
        if shadow_mode {
            shadow_output_with_context(&cx, &input, &decision, predecessor.as_deref()).await
        } else {
            render_decision(&decision)
        }
    })
}

fn usage() -> &'static str {
    "usage: kernel-only-operator-hook [--shadow [--compare-predecessor ABSOLUTE_EXECUTABLE]|--capabilities|--selftest|--version]"
}

fn parse_hook_args(args: &[String]) -> Result<(bool, Option<PathBuf>), String> {
    let mut shadow_mode = false;
    let mut predecessor = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--shadow" => shadow_mode = true,
            "--compare-predecessor" => {
                index += 1;
                let path = args.get(index).ok_or_else(|| {
                    "--compare-predecessor requires an executable path".to_owned()
                })?;
                let path = PathBuf::from(path);
                if !path.is_absolute() {
                    return Err(
                        "--compare-predecessor requires an absolute executable path".to_owned()
                    );
                }
                predecessor = Some(path);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        index += 1;
    }
    if predecessor.is_some() && !shadow_mode {
        return Err("--compare-predecessor is only valid with --shadow".to_owned());
    }
    Ok((shadow_mode, predecessor))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() == 1 {
        match args[0].as_str() {
            "--capabilities" => {
                println!("{}", capabilities());
                return ExitCode::SUCCESS;
            }
            "--selftest" => return selftest(),
            "--version" => {
                println!("kernel-only-operator-hook 0.1.0 build_id={BUILD_ID}");
                return ExitCode::SUCCESS;
            }
            _ => {}
        }
    }
    let (shadow_mode, predecessor) = match parse_hook_args(&args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{}\n{}", usage(), error);
            return ExitCode::from(2);
        }
    };
    let input = match read_bounded_stdin() {
        Ok(input) => input,
        Err(error) => {
            eprintln!("KERNEL_HOOK_INPUT_ERROR: {error}");
            Vec::new()
        }
    };
    println!("{}", run_hook(input, shadow_mode, predecessor));
    // Hook decisions live in hookSpecificOutput, not the exit channel.
    ExitCode::SUCCESS
}

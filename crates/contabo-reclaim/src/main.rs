#![forbid(unsafe_code)]

use asupersync::runtime::RuntimeBuilder;
use asupersync::types::Budget;
use asupersync::Cx;
use contabo_reclaim::{
    consume, parse_args, recover_terminal_response, render_cli_error, request_json, CliError,
    ConsumerError, OWNER_MACHINERY_EXIT,
};
use std::io::Write;
use std::process::ExitCode;

fn print_consumer_error(error: &ConsumerError, json: bool) {
    if json {
        let envelope = serde_json::json!({
            "schema": "contabo-reclaim/report-v1",
            "outcome": "ERROR",
            "class": "OWNER_MACHINERY",
            "reason": error.reason().as_str(),
            "request_id": error.request_id(),
            "error": error.to_string(),
        });
        eprintln!("{}", envelope);
    } else {
        eprintln!("{error}");
    }
}

fn print_cli_error(error: &CliError, json: bool) {
    eprintln!("{}", render_cli_error(error, json));
}

async fn forward_response(
    recovery_cx: &Cx,
    response: contabo_reclaim::OwnerForwardedResponse,
    request_id: &str,
    response_mode: contabo_reclaim::OwnerMode,
    json: bool,
) -> ExitCode {
    let output_error = {
        let mut stdout = std::io::stdout().lock();
        match stdout.write_all(&response.raw_stdout) {
            Err(error) => Some(format!("forwarding owner stdout failed: {error}")),
            Ok(()) => stdout
                .flush()
                .err()
                .map(|error| format!("flushing owner stdout failed: {error}")),
        }
    };
    let Some(output_error) = output_error else {
        return ExitCode::from(response.exit_code);
    };
    // Local stdout is lost but the owner may hold the terminal response in
    // its durable journal; recover it there rather than inventing bytes.
    let recovery = recover_terminal_response(recovery_cx, request_id, response_mode).await;
    let recovery_detail = match recovery {
        Ok(recovered) => format!(
            "durable terminal response recovered; exit_code={}",
            recovered.exit_code
        ),
        Err(error) => format!("durable recovery unavailable: {error}"),
    };
    let refusal = ConsumerError::recovery_needed(
        request_id.to_owned(),
        format!("{output_error}; {recovery_detail}"),
    );
    print_consumer_error(&refusal, json);
    ExitCode::from(OWNER_MACHINERY_EXIT)
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let requested_json = request_json(&arguments);
    let parsed = match parse_args(&arguments) {
        Ok(parsed) => parsed,
        Err(error) if error.is_help() => {
            println!("{}", contabo_reclaim::usage());
            return ExitCode::from(0);
        }
        Err(error) => {
            print_cli_error(&error, requested_json);
            return ExitCode::from(2);
        }
    };
    let runtime = match RuntimeBuilder::current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            let refusal = ConsumerError::owner_machinery(
                contabo_reclaim::OwnerMachineryReason::OwnerWait,
                format!("runtime construction failed: {error}"),
            );
            print_consumer_error(&refusal, parsed.json);
            return ExitCode::from(OWNER_MACHINERY_EXIT);
        }
    };
    // Recovery runs on its own request context, reserved BEFORE the RUN
    // starts: after caller cancellation or outer expiry this Cx is still
    // live, so CANCEL/STATUS/drain proceed under explicit bounds instead of
    // tripping the cancelled caller context immediately. Bounded by the
    // consumer outer deadline, not by budget counters.
    let recovery_cx = runtime.request_cx_with_budget(Budget::INFINITE);
    runtime.block_on(async {
        let cx = match Cx::current() {
            Some(cx) => cx,
            None => {
                let refusal = ConsumerError::owner_machinery(
                    contabo_reclaim::OwnerMachineryReason::Cancelled,
                    "owner consumer has no ambient Cx",
                );
                print_consumer_error(&refusal, parsed.json);
                return ExitCode::from(OWNER_MACHINERY_EXIT);
            }
        };
        match consume(&cx, &recovery_cx, &parsed.request_id, parsed.mode).await {
            Ok(response) => {
                forward_response(
                    &recovery_cx,
                    response,
                    &parsed.request_id,
                    parsed.mode,
                    parsed.json,
                )
                .await
            }
            Err(error) => {
                print_consumer_error(&error, parsed.json);
                ExitCode::from(OWNER_MACHINERY_EXIT)
            }
        }
    })
}

#![forbid(unsafe_code)]

//! `omp-pane-context [--json] <pane-id|session>...`
//!
//! One line per pane, verdict first:
//!
//! ```text
//! PANE_CONTEXT pane=%27 label=jev:0.3 state=MEASURED percent=17.8 tokens=178268 window=1000000 \
//!   model=openai-codex/gpt-5.6-luna read_at=2026-09-25T17:58:28.910Z source=usage_anchor \
//!   profile=codex session_file=…
//! PANE_CONTEXT pane=%5 label=cfsios:0.0 state=UNKNOWN reason=NO_OMP_PROCESS detail=…
//! ```
//!
//! Exit: 0 every pane measured · 1 at least one pane UNKNOWN (NO_PAYLOAD) · 2 usage ·
//! 4 tmux/ps/HOME unreachable (the sensor never reached the subject).

use omp_pane_context::{read_target, OmpModelsCatalog, PaneContext, PaneObservation};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

const OBSERVE_TIMEOUT: Duration = Duration::from_secs(20);
const MODELS_TIMEOUT: Duration = Duration::from_secs(60);

fn line(pane: &PaneObservation, context: &PaneContext) -> String {
    match context {
        PaneContext::Measured(m) => format!(
            "PANE_CONTEXT pane={} label={} state=MEASURED percent={:.1} tokens={} window={} \
             model={}/{} read_at={} source={} profile={} session_file={}",
            pane.pane_id,
            pane.label,
            m.percent,
            m.tokens,
            m.context_window,
            m.provider,
            m.model,
            m.read_at,
            m.source.as_str(),
            m.profile.label(),
            m.session_file.display()
        ),
        PaneContext::Unknown { reason, detail } => format!(
            "PANE_CONTEXT pane={} label={} state=UNKNOWN reason={} detail={}",
            pane.pane_id,
            pane.label,
            reason.as_str(),
            detail
        ),
    }
}

fn row(pane: &PaneObservation, context: &PaneContext) -> Value {
    match context {
        PaneContext::Measured(m) => json!({
            "pane": pane.pane_id,
            "label": pane.label,
            "state": "MEASURED",
            "percent": (m.percent * 10.0).round() / 10.0,
            "tokens": m.tokens,
            "context_window": m.context_window,
            "model": format!("{}/{}", m.provider, m.model),
            "read_at": m.read_at,
            "source": m.source.as_str(),
            "profile": m.profile.label(),
            "session_file": m.session_file.display().to_string(),
        }),
        PaneContext::Unknown { reason, detail } => json!({
            "pane": pane.pane_id,
            "label": pane.label,
            "state": "UNKNOWN",
            "percent": null,
            "reason": reason.as_str(),
            "detail": detail,
        }),
    }
}

fn main() -> ExitCode {
    let mut json_out = false;
    let mut targets = Vec::new();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--json" => json_out = true,
            "--version" => {
                println!("omp-pane-context build_id={}", env!("OMP_BUILD_ID"));
                return ExitCode::SUCCESS;
            }
            "-h" | "--help" => {
                println!("usage: omp-pane-context [--json] <pane-id|session>...");
                return ExitCode::SUCCESS;
            }
            flag if flag.starts_with('-') => {
                eprintln!("usage error: unknown flag {flag}");
                return ExitCode::from(2);
            }
            target => targets.push(target.to_owned()),
        }
    }
    if targets.is_empty() {
        eprintln!("usage: omp-pane-context [--json] <pane-id|session>...");
        return ExitCode::from(2);
    }
    let Some(home) = std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    else {
        println!("PANE_CONTEXT_UNREACHABLE reason=HOME_UNSET");
        return ExitCode::from(4);
    };
    let catalog = OmpModelsCatalog::new("omp", &home, MODELS_TIMEOUT);
    let mut rows = Vec::new();
    let mut all_measured = true;
    for target in &targets {
        match read_target(target, &home, &catalog, OBSERVE_TIMEOUT) {
            Ok(readings) => {
                for (pane, context) in readings {
                    all_measured &= matches!(context, PaneContext::Measured(_));
                    if json_out {
                        rows.push(row(&pane, &context));
                    } else {
                        println!("{}", line(&pane, &context));
                    }
                }
            }
            Err(error) => {
                println!("PANE_CONTEXT_UNREACHABLE target={target} {error}");
                return ExitCode::from(4);
            }
        }
    }
    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&Value::Array(rows)).unwrap_or_else(|_| "[]".into())
        );
    }
    if all_measured {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

//! Thin observe/gate frontend. Logic lives in the lib.
//!
//! usage:
//!   lifecycle-monitor observe --journal PATH [--layer L0] [--metrics METRICS.toml]
//!   lifecycle-monitor gate --journal PATH

#![forbid(unsafe_code)]

use lifecycle_event::{DurableJournal, Layer, LifecycleEvent, EmitOutcome, ReasonCode};
use lifecycle_monitor::{
    gate_claimed_write_readback, gate_freshness_verdict, journal_for_host, load_metrics,
    observe_all, observe_layer, EXPECTED_METRIC_COUNT,
};
use std::path::PathBuf;
use std::process::ExitCode;

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        if arg == name {
            return it.next().map(String::as_str);
        }
    }
    None
}

fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("observe") => observe(&args[1..]),
        Some("gate") => gate(&args[1..]),
        _ => Err(
            "usage: lifecycle-monitor observe|gate --journal PATH [--layer L0] [--metrics PATH]"
                .to_owned(),
        ),
    }
}

fn observe(args: &[String]) -> Result<(), String> {
    let journal = flag(args, "--journal")
        .map(PathBuf::from)
        .unwrap_or_else(journal_for_host);
    let metrics_path = flag(args, "--metrics")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("METRICS.toml"));
    let specs = load_metrics(&metrics_path).map_err(|e| e.to_string())?;
    if specs.len() != EXPECTED_METRIC_COUNT {
        return Err(format!("expected {EXPECTED_METRIC_COUNT} metrics"));
    }
    if let Some(raw) = flag(args, "--layer") {
        let layer = Layer::parse(raw).map_err(|e| e.to_string())?;
        let stall = specs
            .iter()
            .find(|s| s.layer == layer)
            .map(|s| s.stall_after_ms)
            .ok_or_else(|| format!("no metric row for {}", layer.as_str()))?;
        let v = observe_layer(&journal, layer, stall).map_err(|e| e.to_string())?;
        println!(
            "layer={} state={} rows={} age_ms={} fresh={} reason={}",
            v.layer.as_str(),
            v.state.as_str(),
            v.row_count,
            v.age_ms,
            v.fresh,
            v.last_reason
        );
    } else {
        let vs = observe_all(&journal, &specs).map_err(|e| e.to_string())?;
        for v in vs {
            println!(
                "layer={} state={} rows={} age_ms={} fresh={} reason={}",
                v.layer.as_str(),
                v.state.as_str(),
                v.row_count,
                v.age_ms,
                v.fresh,
                v.last_reason
            );
        }
    }
    Ok(())
}

fn gate(args: &[String]) -> Result<(), String> {
    let journal_path = flag(args, "--journal")
        .map(PathBuf::from)
        .unwrap_or_else(journal_for_host);
    if args.iter().any(|a| a == "--known-bad") {
        let journal = DurableJournal::open(&journal_path).map_err(|e| e.to_string())?;
        let claimed = LifecycleEvent::new(
            Layer::L0,
            "HUMAN",
            "S1.L0",
            "lifecycle-monitor",
            EmitOutcome::Emitted,
            ReasonCode::new("GATE_PROBE").map_err(|e| e.to_string())?,
        );
        return match gate_claimed_write_readback(&journal, std::slice::from_ref(&claimed)) {
            Ok(()) => Err("known-bad readback unexpectedly succeeded".to_owned()),
            Err(error) => {
                println!("GATE_KNOWN_BAD_FIRED {error}");
                Ok(())
            }
        };
    }
    let metrics_path = flag(args, "--metrics")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("METRICS.toml"));
    let specs = load_metrics(&metrics_path).map_err(|e| e.to_string())?;
    let vs = observe_all(&journal_path, &specs).map_err(|e| e.to_string())?;
    // Freshness gate (3s6a): a stale layer vetoes the clean verdict. Absence
    // of this call is a silent pass over stale data -- the exact collapse.
    gate_freshness_verdict(&vs).map_err(|e| e.to_string())?;
    println!("GATE_OK layers={}", vs.len());
    Ok(())
}
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

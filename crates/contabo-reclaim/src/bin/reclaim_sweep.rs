//! Sweep trigger surface for the conformance-ported reclaimer.
//!
//! Thin on purpose: argument validation, one runtime, one fleet call, one
//! printed report, and the fleet exit code. Every decision (whitelist,
//! twin gate, integrity, vacuity) lives in `contabo_reclaim::model` and
use contabo_reclaim::model::{ReclaimError, ReclaimMode, WorkerSelection};
use contabo_reclaim::probe::run_selected;
use asupersync::runtime::RuntimeBuilder;
use asupersync::Cx;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: reclaim-sweep --base <abs-path> (--worker <id> | --all-workers) [--apply] [--json]
  default is a dry run that deletes nothing; pass --apply to delete";

fn parse_selection(arguments: &[String]) -> Result<SweepRequest, String> {
    let mut worker: Option<String> = None;
    let mut all = false;
    let mut apply = false;
    let mut base: Option<String> = None;
    let mut json = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--worker" => {
                index += 1;
                worker = arguments.get(index).cloned();
            }
            "--all-workers" => all = true,
            "--apply" => apply = true,
            "--base" => {
                index += 1;
                base = arguments.get(index).cloned();
            }
            "--json" => json = true,
            "--help" | "-h" => return Err("HELP".to_owned()),
            other => return Err(format!("unknown argument: {other}")),
        }
        index += 1;
    }
    let base = base.ok_or_else(|| "missing --base <abs-path>".to_owned())?;
    let selection = match (worker, all) {
        (Some(id), false) => WorkerSelection::One(
            contabo_reclaim::model::worker_by_id(&id)
                .map_err(|error| error.to_string())?,
        ),
        (None, true) => WorkerSelection::All,
        (Some(_), true) => {
            return Err(ReclaimError::MultipleWorkers.to_string());
        }
        (None, false) => {
            return Err(ReclaimError::MissingSelection.to_string());
        }
    };
    Ok(selection)
        .map(|selection| (selection, base, apply, json))
        .map(|(selection, base, apply, json)| SweepRequest {
            selection,
            base: PathBuf::from(base),
            mode: if apply {
                ReclaimMode::Apply
            } else {
                ReclaimMode::DryRun
            },
            json,
        })
}

struct SweepRequest {
    selection: WorkerSelection,
    base: PathBuf,
    mode: ReclaimMode,
    json: bool,
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let request = match parse_selection(&arguments) {
        Ok(request) => request,
        Err(detail) if detail == "HELP" => {
            println!("{USAGE}");
            return ExitCode::from(0);
        }
        Err(detail) => {
            eprintln!("reclaim-sweep: USAGE error={detail}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if !request.base.is_absolute() {
        eprintln!("reclaim-sweep: USAGE base must be absolute");
        return ExitCode::from(2);
    }
    let runtime = match RuntimeBuilder::current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("reclaim-sweep: RUNTIME detail={error:?}");
            return ExitCode::from(2);
        }
    };
    runtime.block_on(async {
        let cx = match Cx::current() {
            Some(cx) => cx,
            None => {
                eprintln!("reclaim-sweep: RUNTIME detail=no_cx");
                return ExitCode::from(2);
            }
        };
        match run_selected(&cx, &request.base, request.mode, &request.selection).await {
            Ok(fleet) => {
                if request.json {
                    match serde_json::to_string_pretty(&fleet) {
                        Ok(rendered) => println!("{rendered}"),
                        Err(error) => {
                            eprintln!("reclaim-sweep: RENDER detail={error}");
                            return ExitCode::from(2);
                        }
                    }
                } else {
                    println!(
                        "fleet outcome={:?} exit={} detail={}",
                        fleet.outcome,
                        fleet.exit_code(),
                        fleet.detail
                    );
                }
                ExitCode::from(fleet.exit_code())
            }
            Err(error) => {
                eprintln!("reclaim-sweep: FLEET_ERROR detail={error}");
                ExitCode::from(2)
            }
        }
    })
}

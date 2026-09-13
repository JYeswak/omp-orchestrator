//! Sweep trigger surface for the conformance-ported reclaimer.
//!
//! Thin on purpose: argument validation, one runtime, one fleet call, one
//! printed report, and the fleet exit code. Every decision (whitelist,
//! twin gate, integrity, vacuity) lives in `contabo_reclaim::model` and
use contabo_reclaim::model::{ReclaimError, ReclaimMode, WorkerSelection};
use contabo_reclaim::probe::{retire_export, run_selected};
use asupersync::runtime::RuntimeBuilder;
use asupersync::Cx;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: reclaim-sweep --base <abs-path> (--worker <id> | --all-workers) [--apply] [--json]
  default is a dry run that deletes nothing; pass --apply to delete
  retire: reclaim-sweep --base <abs-path> --worker <id> --retire <export-basename> [--apply] [--json]
  Retire drops the worker-side pools keyed to one reported grade export (bead
  4ftow). The export name is a basename, never a path; the worker is required
  and single -- one grade ran on one box, and --all-workers with --retire is
  refused. Unreported exports are never named and therefore never touched.";

fn parse_selection(arguments: &[String]) -> Result<SweepRequest, String> {
    let mut worker: Option<String> = None;
    let mut all = false;
    let mut apply = false;
    let mut base: Option<String> = None;
    let mut retire: Option<String> = None;
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
            "--retire" => {
                index += 1;
                retire = arguments.get(index).cloned();
            }
            "--help" | "-h" => return Err("HELP".to_owned()),
            other => return Err(format!("unknown argument: {other}")),
        }
        index += 1;
    }
    let base = base.ok_or_else(|| "missing --base <abs-path>".to_owned())?;
    if let Some(name) = retire {
        // Retire is single-worker by construction: one grade ran on one box.
        // --all-workers with --retire is a usage error, not a fleet retire.
        if all {
            return Err(
                "reclaim-sweep: USAGE --retire takes exactly one --worker, not --all-workers"
                    .to_owned(),
            );
        }
        let id = worker.ok_or_else(|| {
            "reclaim-sweep: USAGE --retire requires --worker <id> (the box the grade ran on)"
                .to_owned()
        })?;
        let worker = contabo_reclaim::model::worker_by_id(&id).map_err(|error| error.to_string())?;
        // Fail fast on the name before any spawn: an invalid export must
        // refuse here, never narrow to zero pools downstream and read clean.
        contabo_reclaim::model::parse_retire_export(&name).map_err(|error| error.to_string())?;
        return Ok(SweepRequest {
            selection: WorkerSelection::One(worker),
            base: PathBuf::from(base),
            mode: if apply {
                ReclaimMode::Apply
            } else {
                ReclaimMode::DryRun
            },
            json,
            retire: Some(name),
        });
    }
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
    Ok(SweepRequest {
        selection,
        base: PathBuf::from(base),
        mode: if apply {
            ReclaimMode::Apply
        } else {
            ReclaimMode::DryRun
        },
        json,
        retire: None,
    })
}

struct SweepRequest {
    selection: WorkerSelection,
    base: PathBuf,
    mode: ReclaimMode,
    json: bool,
    retire: Option<String>,
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
        if let Some(export) = request.retire {
            let WorkerSelection::One(worker) = request.selection else {
                eprintln!("reclaim-sweep: USAGE --retire takes exactly one --worker");
                return ExitCode::from(2);
            };
            match retire_export(&cx, &request.base, worker, &export, request.mode).await {
                Ok(report) => {
                    match contabo_reclaim::model::FleetReport::from_reports(
                        request.mode,
                        vec![report],
                    ) {
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
                            return ExitCode::from(fleet.exit_code());
                        }
                        Err(error) => {
                            eprintln!("reclaim-sweep: FLEET_ERROR detail={error}");
                            return ExitCode::from(2);
                        }
                    }
                }
                Err(error) => {
                    eprintln!("reclaim-sweep: FLEET_ERROR detail={error}");
                    return ExitCode::from(2);
                }
            }
        }
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

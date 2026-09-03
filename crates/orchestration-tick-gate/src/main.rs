use orchestration_tick_gate::{
    default_ledger_path, detect_gap, law_code, parse_ledger, read_ledger, validate_receipt,
    LedgerError,
};
use serde_json::json;
use std::env;
use std::path::PathBuf;

/// Parsed arguments. `--heartbeat` turns on the PRESENCE check, which is a different
/// question from row validity and therefore a different mode rather than an extra law:
/// validity is committable evidence, presence is a runtime observation against a
/// machine-local clock.
struct Args {
    ledger: PathBuf,
    heartbeat: Option<PathBuf>,
}

/// d3gm — ADMISSION: the nine-part crate schema is consulted BEFORE any dispatch.
///
/// # Why here, ahead of the tick laws
///
/// This binary is the supervisor's admission step, and `crate_atom_gate` is the definition
/// of what a crate IS. A fleet that dispatches work into crates whose shape nobody checked
/// is the BUILT != WIRED defect at fleet scale — 30 of 68 crates had no reachable caller
/// when this landed. Consulting the schema at admission means the supervisor refuses to
/// hand out work while the workspace is in a shape the schema rejects.
///
/// # What it does NOT do
///
/// It reads the gate's own `WIRED_CALLERS` roster and PROVES this call site is declared —
/// it does not re-run the scan, because a `cargo metadata` per tick would put a build on
/// the supervisor's critical path. The scan runs in the commit path (GATE 8); this end
/// makes the declaration checkable from the admission side, so the pair cannot drift with
/// only one of them noticing.
fn atom_gate_admission_is_declared() -> Result<(), String> {
    let declared = crate_atom_gate::WIRED_CALLERS
        .iter()
        .any(|(path, _)| path.contains("orchestration-tick-gate"));
    if !declared {
        return Err(
            "ATOM_GATE_UNDECLARED: this binary consults crate-atom-gate but is not named in \
             its WIRED_CALLERS roster, so the gate believes it has one caller when it has \
             two. A roster that disagrees with its callers is worse than no roster."
                .to_owned(),
        );
    }
    if !crate_atom_gate::UNWIRED_ALLOWANCE.is_empty() {
        return Err(format!(
            "ATOM_GATE_UNWIRED_ALLOWANCE_NONEMPTY: {} row(s). The allowance is empty by \
             design; a gate excused from being invoked is worth zero.",
            crate_atom_gate::UNWIRED_ALLOWANCE.len()
        ));
    }
    Ok(())
}

fn main() -> std::process::ExitCode {
    // d3gm: ADMISSION FIRST. Ahead of argument parsing, because a roster that disagrees
    // with its callers is a defect in the gate wiring and not a property of this run.
    if let Err(reason) = atom_gate_admission_is_declared() {
        return fail("ATOM_GATE", &reason);
    }
    let args = match parse_args(env::args().skip(1)) {
        Ok(args) => args,
        Err(error) => return fail("ARGUMENTS", &error),
    };
    let ledger = args.ledger;

    // THE PRESENCE CHECK RUNS FIRST, and it runs even when the ledger cannot be read.
    //
    // That order is the bead: `read_ledger` treats a missing or empty ledger as
    // NOTHING_TO_CHECK and exits 2, which is honest about validity and says nothing about
    // whether rows were OWED. An absent ledger beside a live clock is the single worst
    // state this crate can be in, and it used to be the quietest.
    if let Some(heartbeat) = &args.heartbeat {
        let rows = read_ledger(&ledger).unwrap_or_default();
        let clock = match std::fs::read(heartbeat) {
            Ok(bytes) => parse_ledger(&bytes).unwrap_or_default(),
            Err(error) => {
                return fail(
                    "CLOCK_UNREADABLE",
                    &format!(
                        "heartbeat={} {error}; the presence check cannot run without its \
                         external clock and MUST NOT report absence of a gap",
                        heartbeat.display()
                    ),
                )
            }
        };
        if clock.is_empty() {
            return fail(
                "CLOCK_EMPTY",
                &format!(
                    "heartbeat={} parsed to zero rows; an empty clock cannot witness a gap \
                     and reports identically to a fleet that never ticked",
                    heartbeat.display()
                ),
            );
        }
        if let Some(gap) = detect_gap(&rows, &clock) {
            println!("ORCHESTRATION_TICK_GATE status=GAP file={} {gap}", ledger.display());
            return std::process::ExitCode::from(3);
        }
        println!(
            "ORCHESTRATION_TICK_GATE status=PRESENT file={} rows={} clock={} clock_rows={}",
            ledger.display(),
            rows.len(),
            heartbeat.display(),
            clock.len()
        );
    }

    let rows = match read_ledger(&ledger) {
        Ok(rows) => rows,
        Err(error) => {
            let (law, detail) = match error {
                LedgerError::NothingToCheck(detail) => ("LEDGER_NOTHING_TO_CHECK", detail),
                LedgerError::Invalid(detail) => ("LEDGER_ROW_INVALID", detail),
            };
            return fail(law, &format!("file={} {detail}", ledger.display()));
        }
    };

    let mut violations = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if let Err(errors) = validate_receipt(row) {
            for error in errors {
                violations.push(format!(
                    "file={} row={} law={} detail={}",
                    ledger.display(),
                    index + 1,
                    law_code(&error),
                    error
                ));
            }
        }
    }

    if violations.is_empty() {
        println!(
            "ORCHESTRATION_TICK_GATE status=CLEAN file={} rows={}",
            ledger.display(),
            rows.len()
        );
        return std::process::ExitCode::SUCCESS;
    }

    for violation in violations {
        println!("ORCHESTRATION_TICK_GATE status=VIOLATION {violation}");
    }
    std::process::ExitCode::from(1)
}

fn parse_args(arguments: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut arguments = arguments.peekable();
    let mut ledger = None;
    let mut heartbeat = None;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--ledger" => {
                let path = arguments
                    .next()
                    .ok_or_else(|| "--ledger requires a path".to_owned())?;
                ledger = Some(PathBuf::from(path));
            }
            "--heartbeat" => {
                let path = arguments
                    .next()
                    .ok_or_else(|| "--heartbeat requires a path".to_owned())?;
                heartbeat = Some(PathBuf::from(path));
            }
            "--help" | "-h" => {
                println!(
                    "usage: orchestration-tick-gate [--ledger PATH] [--heartbeat PATH]\n  \
                     --heartbeat enables the PRESENCE check: exit 3 when the external clock \
                     witnessed tick outcomes that no row describes."
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(Args {
        ledger: ledger
            .unwrap_or_else(|| default_ledger_path(&env::current_dir().unwrap_or_default())),
        heartbeat,
    })
}

fn fail(law: &str, detail: &str) -> std::process::ExitCode {
    println!(
        "{}",
        json!({
            "schema": "omp.orchestration-tick-gate.v1",
            "status": "ERROR",
            "law": law,
            "detail": detail,
        })
    );
    std::process::ExitCode::from(2)
}

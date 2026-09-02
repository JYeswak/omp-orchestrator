use orchestration_tick_gate::{
    default_ledger_path, law_code, read_ledger, validate_receipt, LedgerError,
};
use serde_json::json;
use std::env;
use std::path::PathBuf;

fn main() -> std::process::ExitCode {
    let ledger = match parse_ledger_path(env::args().skip(1)) {
        Ok(path) => path,
        Err(error) => return fail("ARGUMENTS", &error),
    };

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

fn parse_ledger_path(arguments: impl Iterator<Item = String>) -> Result<PathBuf, String> {
    let mut arguments = arguments.peekable();
    let mut ledger = None;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--ledger" => {
                let path = arguments
                    .next()
                    .ok_or_else(|| "--ledger requires a path".to_owned())?;
                ledger = Some(PathBuf::from(path));
            }
            "--help" | "-h" => {
                println!("usage: orchestration-tick-gate [--ledger PATH]");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(ledger.unwrap_or_else(|| default_ledger_path(&env::current_dir().unwrap_or_default())))
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

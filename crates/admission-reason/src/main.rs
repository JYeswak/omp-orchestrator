#![forbid(unsafe_code)]

//! Live admission-reason binary. Verdicts on STDOUT at column 0.

use admission_reason::{explain, explain_checked, LedgerError, Rules};
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut selftest = false;
    let mut mutation = false;
    let mut publication_check = false;
    let mut ledger: Option<PathBuf> = None;
    let mut disabled: Vec<String> = Vec::new();
    let mut args = env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--selftest" => selftest = true,
            "--mutation" => mutation = true,
            "--publication-check" => publication_check = true,
            "--ledger" => {
                let Some(value) = args.next() else {
                    return report_ledger_error(LedgerError::Missing {
                        path: PathBuf::from("<missing>"),
                        reason: "--ledger requires a PATH value".to_owned(),
                    });
                };
                ledger = Some(PathBuf::from(value));
            }
            "--disable-rule" => match args.next() {
                Some(v) => disabled.push(v),
                None => {
                    eprintln!("usage: admission-reason [--ledger PATH] [--selftest]");
                    return ExitCode::from(2);
                }
            },
            "-h" | "--help" => {
                eprintln!("usage: admission-reason [--ledger PATH] [--selftest]");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("usage: admission-reason [--ledger PATH] [--selftest]");
                let _ = other;
                return ExitCode::from(2);
            }
        }
    }
    let mut rules = Rules::default();
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
        return run_selftest(&rules);
    }
    let path = match resolve_ledger_path(ledger) {
        Ok(path) => path,
        Err(error) => return report_ledger_error(error),
    };
    match explain_checked(&path, publication_check, &rules) {
        Ok(output) => print!("{output}"),
        Err(error) => return report_ledger_error(error),
    }
    ExitCode::SUCCESS
}

fn resolve_ledger_path(explicit: Option<PathBuf>) -> Result<PathBuf, LedgerError> {
    if let Some(path) = explicit {
        if path.as_os_str().is_empty() {
            return Err(LedgerError::Missing {
                path: PathBuf::from("<empty>"),
                reason: "--ledger PATH must not be empty".to_owned(),
            });
        }
        return Ok(path);
    }
    match env::var_os("CHECK_SH_LEDGER") {
        Some(value) if !value.is_empty() => Ok(PathBuf::from(value)),
        Some(_) => Err(LedgerError::Missing {
            path: PathBuf::from("<CHECK_SH_LEDGER>"),
            reason: "CHECK_SH_LEDGER is set but empty".to_owned(),
        }),
        None => Err(LedgerError::Missing {
            path: PathBuf::from("<CHECK_SH_LEDGER>"),
            reason: "--ledger was not supplied and CHECK_SH_LEDGER is unset".to_owned(),
        }),
    }
}

fn report_ledger_error(error: LedgerError) -> ExitCode {
    eprintln!("ERROR {}: {}", error.code(), error);
    ExitCode::from(3)
}

fn run_selftest(rules: &Rules) -> ExitCode {
    use std::fs;
    let tmp = std::env::temp_dir().join(format!("ar-st-{}", std::process::id()));
    let _ = fs::create_dir_all(&tmp);
    let mut rc = 0i32;
    let chk = |cond: bool, msg: &str, rc: &mut i32| {
        if !cond {
            println!("{msg}");
            *rc = 1;
        }
    };

    let red = tmp.join("red.json");
    fs::write(&red, r#"{"schema":"control-plane.check.v1","entries":[
 {"gate":"docs-staleness","verdict":"PASS","detail":"fine"},
 {"gate":"domain-closure","verdict":"RED","detail":"{\"violations\":[{\"code\":\"E003\",\"row\":\"zestgraph-capture\",\"detail\":\"drift anchor mismatch: row says aaa, artifact is bbb\"}]}"}]}"#).unwrap();
    let out = explain(&red, false, rules);
    chk(
        out.contains("E003"),
        "SELFTEST RED did not surface the E003 code",
        &mut rc,
    );
    chk(
        out.contains("drift anchor mismatch"),
        "SELFTEST RED did not surface the violation detail",
        &mut rc,
    );
    chk(
        out.contains("domain-closure"),
        "SELFTEST RED did not name the failing gate",
        &mut rc,
    );

    let reordered = tmp.join("reordered.json");
    fs::write(&reordered, r#"{"schema":"control-plane.check.v1","entries":[
 {"gate":"domain-closure","verdict":"RED","detail":"doctor: {\"violations\":[{\"detail\":\"missing harm class: \\\"irreversible\\\"\",\"row\":\"gate-x\",\"code\":\"E010\"}]}"}]}"#).unwrap();
    let out = explain(&reordered, false, rules);
    chk(
        out.contains("E010"),
        "SELFTEST RED structural parse lost E010",
        &mut rc,
    );
    chk(
        out.contains("missing harm class"),
        "SELFTEST RED structural parse lost detail",
        &mut rc,
    );
    chk(
        out.contains("row=gate-x"),
        "SELFTEST RED structural parse lost row",
        &mut rc,
    );

    let trunc = tmp.join("trunc.json");
    fs::write(&trunc, r#"{"schema":"control-plane.check.v1","entries":[
 {"gate":"domain-closure","verdict":"RED","detail":"{\"accepted_input_boundary\":\"local hooks_certified.toml, settings.json, and referenced artifact bytes\",\"command\":\"doctor\",\"external_writes\":false,\"live_hooks\":15,\"mode\":\"doctor\",\"next_falsifier\":\"mutat"}]}"#).unwrap();
    let out = explain(&trunc, false, rules);
    chk(
        out.contains("domain-closure"),
        "SELFTEST RED truncated detail lost the gate name",
        &mut rc,
    );
    chk(
        !out.trim().is_empty(),
        "SELFTEST RED truncated detail produced no explanation at all",
        &mut rc,
    );

    let now = chrono_like_now();
    let green = tmp.join("green.json");
    fs::write(&green, format!(r#"{{"schema":"control-plane.check.v1","entries":[
 {{"gate":"docs-staleness","verdict":"PASS","detail":"fine"}},
 {{"gate":"domain-closure","verdict":"PASS","detail":"ok"}}],"overall":"PASS","completed_ts":"{now}"}}"#)).unwrap();
    let out = explain(&green, false, rules);
    chk(
        out.is_empty(),
        "SELFTEST RED spoke on an all-PASS ledger",
        &mut rc,
    );

    let expired = tmp.join("expired.json");
    fs::write(&expired, r#"{"schema":"control-plane.check.v1","entries":[
 {"gate":"docs-staleness","verdict":"PASS","detail":"fine"},
 {"gate":"domain-closure","verdict":"PASS","detail":"ok"}],"overall":"PASS","completed_ts":"2020-01-01T00:00:00Z"}"#).unwrap();
    std::env::set_var("ADMISSION_FRESH_SECONDS", "900");
    let out = explain(&expired, false, rules);
    chk(
        out.contains("EXPIRED"),
        "SELFTEST RED did not classify an expired PASS",
        &mut rc,
    );
    chk(
        out.contains("age=") && out.contains("window="),
        "SELFTEST RED expired PASS omitted age/window",
        &mut rc,
    );
    chk(
        !out.contains("did not PASS"),
        "SELFTEST RED expired PASS used the RED-gate message",
        &mut rc,
    );

    let incomplete = tmp.join("incomplete.json");
    fs::write(
        &incomplete,
        r#"{"schema":"control-plane.check.v1","entries":[
 {"gate":"docs-staleness","verdict":"PASS","detail":"fine"},
 {"gate":"domain-closure","verdict":"PASS","detail":"ok"}]}"#,
    )
    .unwrap();
    let out = explain(&incomplete, false, rules);
    chk(
        out.contains("did not COMPLETE"),
        "SELFTEST RED incomplete all-PASS ledger was not named",
        &mut rc,
    );

    let unrun = tmp.join("unrun.json");
    fs::write(
        &unrun,
        r#"{"schema":"control-plane.check.v1","entries":[
 {"gate":"tests","verdict":"UNRUN","detail":"skipped-after-domain-closure"}]}"#,
    )
    .unwrap();
    let out = explain(&unrun, false, rules);
    chk(
        out.contains("UNRUN"),
        "SELFTEST RED did not report an UNRUN gate",
        &mut rc,
    );

    let absent = tmp.join("absent.json");
    let out = explain(&absent, false, rules);
    chk(
        out.contains("no check.sh ledger"),
        "SELFTEST RED a missing ledger was silent (reads as healthy)",
        &mut rc,
    );
    match explain_checked(&absent, false, rules) {
        Ok(_) => chk(
            false,
            "SELFTEST typed missing-ledger check unexpectedly passed",
            &mut rc,
        ),
        Err(error) => chk(
            error.code() == "ledger_missing"
                && error.to_string().contains(&absent.display().to_string())
                && error.to_string().contains("reason="),
            "SELFTEST typed missing-ledger error omitted code, path, or reason",
            &mut rc,
        ),
    }

    let _ = fs::remove_dir_all(&tmp);
    if rc == 0 {
        println!("SELFTEST PASS admission-reason distinguishes fresh PASS, expired PASS, incomplete ledgers, and RED gates; parses typed violations and handles truncated evidence");
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn chrono_like_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

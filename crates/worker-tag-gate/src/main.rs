//! Binary frontend for [`worker_tag_gate`]. The library is the single implementation.

use std::process::ExitCode;

use worker_tag_gate::{check, GateError, WorkerRow};

const DEFAULT_PATH: &str = ".config/rch/workers.toml";

fn usage() -> String {
    format!(
        "usage: worker-tag-gate [--workers <path>] [--selftest]\n\
         \n\
         Refuses an rch workers.toml in which a NON-DARWIN worker declares `{}`.\n\
         Default path: $HOME/{DEFAULT_PATH}\n\
         \n\
         exit 0  every worker's tags are admissible\n\
         exit 2  OS_DARWIN_ON_NON_DARWIN_HOST\n\
         exit 3  WORKERS_TOML_UNREADABLE   (UNKNOWN, never a pass)\n\
         exit 4  WORKERS_TOML_EMPTY        (anti-vacuity)\n\
         exit 64 usage error",
        WorkerRow::OS_DARWIN
    )
}

fn resolve(explicit: Option<String>) -> String {
    explicit.unwrap_or_else(|| match std::env::var("HOME") {
        Ok(home) => format!("{home}/{DEFAULT_PATH}"),
        Err(_) => DEFAULT_PATH.to_owned(),
    })
}

fn main() -> ExitCode {
    let mut path: Option<String> = None;
    let mut selftest = false;
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--workers" => match args.next() {
                Some(value) => path = Some(value),
                None => {
                    eprintln!("worker-tag-gate: --workers needs a path\n{}", usage());
                    return ExitCode::from(64);
                }
            },
            "--selftest" => selftest = true,
            "-h" | "--help" => {
                println!("{}", usage());
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("worker-tag-gate: unknown argument {other:?}\n{}", usage());
                return ExitCode::from(64);
            }
        }
    }

    if selftest {
        return run_selftest();
    }

    let target = resolve(path);
    let body = match std::fs::read_to_string(&target) {
        Ok(body) => body,
        Err(err) => {
            let error = GateError::Unreadable {
                path: target.clone(),
                detail: err.to_string(),
            };
            eprintln!("{error}");
            return ExitCode::from(error.exit_code());
        }
    };

    match check(&body, &target) {
        Ok(rows) => {
            let darwin_hosts = rows.iter().filter(|r| r.declares_os_darwin()).count();
            println!(
                "WORKER_TAGS_OK scanned={} os_darwin_declared_by={} path={}",
                rows.len(),
                darwin_hosts,
                target
            );
            for row in &rows {
                println!(
                    "  {} tags={:?} enabled={} darwin_host={}",
                    row.id,
                    row.tags,
                    row.enabled,
                    row.is_darwin_host()
                );
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(error.exit_code())
        }
    }
}

/// Fires-on-known-bad, both directions, with a known-GOOD leg. Asserts the MESSAGE **and** the
/// exit code per `AGENTS.md` rule 7 as corrected at `17f3357`.
fn run_selftest() -> ExitCode {
    let mut failures = 0usize;

    // KNOWN-GOOD: a real Mac may declare os:darwin; Linux boxes carry no such tag.
    let good = r#"
[[workers]]
id = "zestdata-local"
tags = ["os:darwin", "macos", "aarch64", "artifact"]
enabled = true

[[workers]]
id = "contabo-3"
tags = ["linux", "x86_64", "rust"]
enabled = true
"#;
    match check(good, "fixture:good") {
        Ok(rows) if rows.len() == 2 => println!("  ok   known-good: 2 rows, no offender"),
        Ok(rows) => {
            failures += 1;
            println!("  FAIL known-good: expected 2 rows, got {}", rows.len());
        }
        Err(error) => {
            failures += 1;
            println!("  FAIL known-good refused: {error}");
        }
    }

    // KNOWN-BAD: the exact shape measured on contabo-3 on 2026-09-07.
    let bad = r#"
[[workers]]
id = "contabo-3"
tags = ["linux", "x86_64", "rust", "os:darwin"]
enabled = true
"#;
    match check(bad, "fixture:bad") {
        Err(error @ GateError::DarwinTagOnNonDarwinHost { .. }) => {
            let text = error.to_string();
            let has_code = text.contains("OS_DARWIN_ON_NON_DARWIN_HOST");
            let has_id = text.contains("contabo-3");
            let has_reason = text.contains("REFUSE EVERY NATIVE BUILD");
            if has_code && has_id && has_reason && error.exit_code() == 2 {
                println!("  ok   known-bad: code+id+reason in message, exit=2");
            } else {
                failures += 1;
                println!(
                    "  FAIL known-bad message/exit: code={has_code} id={has_id} reason={has_reason} exit={}",
                    error.exit_code()
                );
            }
        }
        other => {
            failures += 1;
            println!("  FAIL known-bad did not fire: {other:?}");
        }
    }

    // ANTI-VACUITY: zero rows is an ERROR with its own code, never a pass.
    match check("# only a comment\n", "fixture:empty") {
        Err(error @ GateError::NoWorkers { .. }) if error.exit_code() == 4 => {
            println!("  ok   anti-vacuity: empty scan is WORKERS_TOML_EMPTY exit=4")
        }
        other => {
            failures += 1;
            println!("  FAIL anti-vacuity: {other:?}");
        }
    }

    // COMMENT STRIPPING: a commented-out tag must NOT be read as live.
    let commented = r#"
[[workers]]
id = "contabo-2"
tags = ["linux", "x86_64", "rust"]
# tags = ["linux", "x86_64", "rust", "os:darwin"]   <- retired, must not count
enabled = true
"#;
    match check(commented, "fixture:commented") {
        Ok(rows) if rows.len() == 1 && !rows[0].declares_os_darwin() => {
            println!("  ok   comment-stripping: commented tag not counted")
        }
        other => {
            failures += 1;
            println!("  FAIL comment-stripping: {other:?}");
        }
    }

    // DISTINCT CODES: no two causes may share an exit code.
    let codes = [
        GateError::DarwinTagOnNonDarwinHost { offenders: vec![] }.exit_code(),
        GateError::Unreadable {
            path: String::new(),
            detail: String::new(),
        }
        .exit_code(),
        GateError::NoWorkers {
            path: String::new(),
        }
        .exit_code(),
    ];
    let mut sorted = codes;
    sorted.sort_unstable();
    let distinct = sorted.windows(2).all(|w| w[0] != w[1]);
    if distinct {
        println!("  ok   distinct exit codes: {codes:?}");
    } else {
        failures += 1;
        println!("  FAIL exit codes collide: {codes:?}");
    }

    if failures == 0 {
        println!("SELFTEST OK 5 legs, 0 failures");
        ExitCode::SUCCESS
    } else {
        println!("SELFTEST FAILED {failures} leg(s)");
        ExitCode::from(1)
    }
}

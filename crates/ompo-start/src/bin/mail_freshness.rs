#![forbid(unsafe_code)]

//! `ompo-start-mail-freshness` — the consumer of [`ompo_start::mail`].
//!
//! Reads one `am robot status --json` envelope (a file path argument, or stdin)
//! and prints the Agent Mail source's freshness. This is the process-level
//! consumer of the pure derivation: a function nothing calls is a receipt for
//! work nobody reads.
//!
//! # Which clock is "now"
//!
//! The OBSERVER clock is this process's `SystemTime::now()`, read once, here —
//! the only place in the L4 mail path that reads a clock. The WRITER clock is the
//! `_meta.timestamp` stamped by the `am` host. `age_ms = observer − writer`.
//! `--now-ms <i64>` overrides the observer reading so a caller can replay a
//! captured envelope deterministically.
//!
//! # Exit contract
//!
//! * `0` — FRESH: both clocks read and ordered; `age_ms` printed.
//! * `2` — SILENT: the writer clock was absent, unparsable, or ahead of the
//!   observer. `age_ms=UNKNOWN` is printed — never `0`.
//! * `3` — UNREADABLE: the envelope itself could not be read. An instrument
//!   failure is not a verdict about Agent Mail.

use ompo_start::mail::{mail_freshness, MailFreshness, MAIL_SOURCE_NAME};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let index = args.iter().position(|arg| arg == name)?;
    args.get(index + 1).map(String::as_str)
}

/// The observer clock: Unix epoch milliseconds, read once.
fn observer_now_ms() -> Result<i64, String> {
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("L4_MAIL_OBSERVER_CLOCK_BEFORE_EPOCH — {error}"))?;
    i64::try_from(since.as_millis())
        .map_err(|_| "L4_MAIL_OBSERVER_CLOCK_OUT_OF_RANGE".to_owned())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let now_ms = match flag(&args, "--now-ms") {
        Some(raw) => match raw.parse::<i64>() {
            Ok(value) => value,
            Err(error) => {
                eprintln!("L4_MAIL_BAD_NOW_MS {raw:?} — {error}");
                return ExitCode::from(3);
            }
        },
        None => match observer_now_ms() {
            Ok(value) => value,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::from(3);
            }
        },
    };

    let envelope = match args.first().filter(|arg| !arg.starts_with("--")) {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("L4_MAIL_UNREADABLE_ENVELOPE {path} — {error}");
                return ExitCode::from(3);
            }
        },
        None => {
            let mut text = String::new();
            if let Err(error) = std::io::Read::read_to_string(&mut std::io::stdin(), &mut text) {
                eprintln!("L4_MAIL_UNREADABLE_ENVELOPE <stdin> — {error}");
                return ExitCode::from(3);
            }
            text
        }
    };

    match mail_freshness(&envelope, now_ms) {
        Ok(MailFreshness::Age { age_ms, writer_ms }) => {
            println!(
                "source={MAIL_SOURCE_NAME} status=FRESH age_ms={age_ms} writer_ms={writer_ms} observer_ms={now_ms}"
            );
            ExitCode::SUCCESS
        }
        Ok(silent @ MailFreshness::Silent { .. }) => {
            // UNKNOWN, never 0: a downstream reader must not be able to mistake an
            // unread clock for a fresh one.
            println!(
                "source={MAIL_SOURCE_NAME} status=SILENT age_ms=UNKNOWN reason={}",
                silent.reason_code()
            );
            ExitCode::from(2)
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(3)
        }
    }
}

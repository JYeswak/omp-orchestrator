//! CLI wrapper — the surface the conductor's cron invokes.
//!
//! Exit codes: 0 = any typed verdict was rendered (the verdict is on stdout
//! as `verdict=<DETECTOR_NAME>`), 2 = usage error, 3 = the subprocess
//! invocation itself failed (the caller must re-read, not retry blind).

#![forbid(unsafe_code)]

use dispatch_silence_watch::{classify, parse_bead_assignee};
use std::path::Path;
use std::process::{Command, ExitCode};
use std::time::Duration;

/// A tracker read-back is metadata, not work: 30s bounds a wedged `br`
/// without ever misreading the stall as a posted verdict.
const TRACKER_READ_DEADLINE: Duration = Duration::from_secs(30);

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 4 {
        eprintln!("usage: dispatch-silence-watch <bead-id> <session> <dispatch-assignee> <dispatch-epoch> <deadline-secs>");
        return ExitCode::from(2);
    }
    let bead_id = &args[0];
    let session = &args[1];
    let dispatch_assignee = &args[2];
    let dispatch_epoch: i64 = match args[3].parse() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("usage error: dispatch-epoch must be a unix timestamp integer");
            return ExitCode::from(2);
        }
    };
    let deadline_secs: i64 = match args.get(4).map(|s| s.parse()).transpose() {
        Ok(v) => v.unwrap_or(3600),
        Err(_) => {
            eprintln!("usage error: deadline-secs must be an integer");
            return ExitCode::from(2);
        }
    };

    let repo = Path::new(".");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // READ BACK the comments — the authority for VERDICT_POSTED. Bounded:
    // a wedged br must produce a typed TRACKER_ERROR, never an unbounded
    // stall and never an empty read that parses as "no verdict posted".
    let mut comments_command = Command::new("br");
    comments_command.args(["comments", "list", bead_id]);
    comments_command.current_dir(repo);
    let comments_output = match dispatch_silence_watch::tracker_read_from(
        subprocess_contract::bounded_output(&mut comments_command, TRACKER_READ_DEADLINE),
    ) {
        dispatch_silence_watch::TrackerRead::Read(text) => text,
        dispatch_silence_watch::TrackerRead::TrackerError(reason) => {
            eprintln!("TRACKER_ERROR: br comments list: {reason} for {bead_id}");
            return ExitCode::from(3);
        }
    };

    // READ BACK the bead's current assignee - same bounded, typed contract.
    let mut show_command = Command::new("br");
    show_command.args(["show", bead_id, "--json"]);
    show_command.current_dir(repo);
    let current_assignee = match dispatch_silence_watch::tracker_read_from(
        subprocess_contract::bounded_output(&mut show_command, TRACKER_READ_DEADLINE),
    ) {
        dispatch_silence_watch::TrackerRead::Read(text) => {
            parse_bead_assignee(&text, bead_id).unwrap_or_default()
        }
        dispatch_silence_watch::TrackerRead::TrackerError(reason) => {
            eprintln!("TRACKER_ERROR: br show: {reason} for {bead_id}");
            return ExitCode::from(3);
        }
    };
    let verdict = classify(
        &comments_output,
        &current_assignee,
        dispatch_assignee,
        dispatch_epoch,
        now,
        deadline_secs,
    );

    println!("bead={bead_id} verdict={verdict} detector={}", verdict.detector());
    if verdict == dispatch_silence_watch::SilenceVerdict::SilentPastDeadline {
        println!(
            "action=re-dispatch or escalate; dispatch_assignee={dispatch_assignee} dispatch_epoch={dispatch_epoch} deadline_secs={deadline_secs}"
        );
    }
    ExitCode::SUCCESS
}

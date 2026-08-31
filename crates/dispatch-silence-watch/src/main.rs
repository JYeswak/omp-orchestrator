//! CLI wrapper — the surface the conductor's cron invokes.
//!
//! Exit codes: 0 = any typed verdict was rendered (the verdict is on stdout
//! as `verdict=<DETECTOR_NAME>`), 2 = usage error, 3 = the subprocess
//! invocation itself failed (the caller must re-read, not retry blind).

#![forbid(unsafe_code)]

use dispatch_silence_watch::{classify, parse_bead_assignee};
use std::path::Path;
use std::process::{Command, ExitCode};

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

    // READ BACK the comments — the authority for VERDICT_POSTED.
    let comments = Command::new("br")
        .args(["comments", "list", bead_id])
        .current_dir(repo)
        .output();
    let comments_output = match comments {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout).into_owned()
        }
        Ok(out) => {
            // A nonzero exit from br comments list is a tracker error.
            eprintln!(
                "TRACKER_ERROR: br comments list exited {} for {}",
                out.status.code().unwrap_or(-1),
                bead_id
            );
            return ExitCode::from(3);
        }
        Err(err) => {
            eprintln!("TRACKER_ERROR: cannot spawn br comments list: {err}");
            return ExitCode::from(3);
        }
    };

    // READ BACK the bead's current assignee.
    let show = Command::new("br")
        .args(["show", bead_id, "--json"])
        .current_dir(repo)
        .output();
    let current_assignee = match show {
        Ok(out) if out.status.success() => {
            parse_bead_assignee(&String::from_utf8_lossy(&out.stdout), bead_id)
                .unwrap_or_default()
        }
        _ => {
            eprintln!("TRACKER_ERROR: br show failed for {}", bead_id);
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

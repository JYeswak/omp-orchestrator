#![forbid(unsafe_code)]

//! Decide-only binary. launchd fires this with no `--apply`.
//! `--apply` is refused. This process never sends via ntm and never writes br.

use m2_grading_lane::{
    append_oracle, apply_refused_record, decide_only_no_feed_record, default_oracle_path,
};
use std::process::ExitCode;

fn main() -> ExitCode {
    let apply = std::env::args().any(|a| a == "--apply");
    let oracle = match default_oracle_path() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("m2-grading-lane: {error}");
            return ExitCode::from(64);
        }
    };
    if apply {
        if let Err(error) = append_oracle(&oracle, &apply_refused_record()) {
            eprintln!("m2-grading-lane: {error}");
            return ExitCode::from(4);
        }
        eprintln!("m2-grading-lane: --apply refused without an explicit candidate feed");
        return ExitCode::from(2);
    }
    if let Err(error) = append_oracle(&oracle, &decide_only_no_feed_record()) {
        eprintln!("m2-grading-lane: {error}");
        return ExitCode::from(4);
    }
    eprintln!("m2-grading-lane: decide-only (no ntm send, no br write)");
    ExitCode::from(3)
}

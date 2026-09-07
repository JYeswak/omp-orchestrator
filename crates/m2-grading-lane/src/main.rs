#![forbid(unsafe_code)]

use m2_grading_lane::autoroute;
use std::process::ExitCode;

fn main() -> ExitCode {
    let apply = std::env::args().any(|a| a == "--apply");
    if apply {
        eprintln!("m2-grading-lane: --apply refused without an explicit candidate feed");
        return ExitCode::from(2);
    }
    let _ = autoroute;
    eprintln!("m2-grading-lane: decide-only (no ntm send, no br write)");
    ExitCode::from(0)
}

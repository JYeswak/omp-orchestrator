#![forbid(unsafe_code)]

//! Render the loop-coverage matrix without requiring the reader to remember cargo test
//! invocations. `--json` and `--markdown` print the executable `LOOP_COVERAGE` constant.

use loop_coverage::{render_json, render_markdown};
use std::process::ExitCode;

const USAGE: &str = "\
loop-coverage — typed coverage matrix for the dispatch loop (a MAP, not a gate)

USAGE:
    loop-coverage --json        robot-readable report
    loop-coverage --markdown    human-readable map (committed as docs/LOOP_COVERAGE_MATRIX.md)

This binary does not admit or refuse dispatch. It renders what the loop must guarantee
and how each layer is proven. See NO_CLAIM_BOUNDARY in the crate docs.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["--json"] => {
            println!("{}", render_json());
            ExitCode::SUCCESS
        }
        ["--markdown"] => {
            print!("{}", render_markdown());
            ExitCode::SUCCESS
        }
        ["--help"] | ["-h"] | [] => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        _ => {
            eprint!("{USAGE}");
            ExitCode::from(1)
        }
    }
}

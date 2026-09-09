#![forbid(unsafe_code)]

//! Render the loop-coverage matrix without requiring the reader to remember cargo test
//! invocations. `--json` and `--markdown` print the executable `LOOP_COVERAGE` constant.

use loop_coverage::{
    audit_external_authorities, cited_external_authorities, render_json, render_markdown,
};
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "\
loop-coverage — typed coverage matrix for the dispatch loop (a MAP, not a gate)

USAGE:
    loop-coverage --json        robot-readable report
    loop-coverage --audit       content citation audit (typed skips are materialized)
    loop-coverage --markdown    human-readable map (committed as docs/LOOP_COVERAGE_MATRIX.md)

This binary does not admit or refuse dispatch. It renders what the loop must guarantee
and how each layer is proven. See NO_CLAIM_BOUNDARY in the crate docs.
";

fn audit() -> ExitCode {
    let root = match Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
    {
        Ok(root) => root,
        Err(error) => {
            eprintln!("LOOP_COVERAGE_CONTENT_AUDIT status=ERROR reason=REPO_ROOT detail={error}");
            return ExitCode::from(2);
        }
    };
    let cited = cited_external_authorities();
    if cited.is_empty() {
        eprintln!("LOOP_COVERAGE_CONTENT_AUDIT status=ERROR reason=EMPTY_SCAN_SET");
        return ExitCode::from(2);
    }
    match audit_external_authorities(&cited, |path| root.join(path).exists()) {
        Ok(skipped) => {
            let status = if skipped.is_empty() {
                "COMPLETE"
            } else {
                "PARTIAL"
            };
            println!(
                "LOOP_COVERAGE_CONTENT_AUDIT status={status} cited={} resolved={} declared_absent={}",
                cited.len(),
                cited.len() - skipped.len(),
                skipped.len()
            );
            for check in skipped {
                println!(
                    "LOOP_COVERAGE_CONTENT_SKIP authority={} reason={}",
                    check.authority, check.reason
                );
            }
            ExitCode::SUCCESS
        }
        Err(errors) => {
            eprintln!("LOOP_COVERAGE_CONTENT_AUDIT status=REFUSED errors={errors:?}");
            ExitCode::from(1)
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["--audit"] => audit(),
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

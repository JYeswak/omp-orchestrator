#![forbid(unsafe_code)]

//! Emit one deterministic silent-success census JSON artifact.
//!
//! Usage: `silent-success-census [--repo <path>]`. Findings do not fail the command; an
//! unreadable or empty scan is an embedded `status = "error"` and exits nonzero.

use silent_success_census::{render_json, scan_workspace};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut root = PathBuf::from(".");
    while let Some(argument) = args.next() {
        if argument == "--repo" {
            match args.next() {
                Some(path) => root = PathBuf::from(path),
                None => {
                    eprintln!("usage: silent-success-census [--repo <repo-root>]");
                    return ExitCode::from(2);
                }
            }
        } else if argument == "--json" {
            // JSON is the only output format; retain the flag for explicit robot callers.
        } else if argument.starts_with('-') {
            eprintln!("usage: silent-success-census [--repo <repo-root>]");
            return ExitCode::from(2);
        } else {
            root = PathBuf::from(argument);
        }
    }

    let report = scan_workspace(&root);
    println!("{}", render_json(&report));
    if report.status == "error" {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

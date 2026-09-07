#![forbid(unsafe_code)]

use asupersync_conformance::{render_document, scan_repository, ASUPERSYNC_REV};
use std::path::PathBuf;
use std::process::ExitCode;

const COMMAND: &str =
    "cargo run --quiet -p asupersync-conformance -- --repo . --write ASUPERSYNC-CONFORMANCE.md";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut repo = PathBuf::from(".");
    let mut output = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--repo" => match args.next() {
                Some(path) => repo = PathBuf::from(path),
                None => return usage("--repo requires a path"),
            },
            "--write" => match args.next() {
                Some(path) => output = Some(PathBuf::from(path)),
                None => return usage("--write requires a path"),
            },
            "-h" | "--help" => {
                eprintln!("asupersync-conformance [--repo PATH] [--write PATH]");
                return ExitCode::SUCCESS;
            }
            other => return usage(&format!("unknown argument {other}")),
        }
    }

    let report = match scan_repository(&repo) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let document = render_document(&report, COMMAND, "working-tree");
    if let Some(path) = output {
        if let Err(error) = std::fs::write(&path, document) {
            eprintln!(
                "ASUPERSYNC_CONFORMANCE_ERROR reason=WRITE_FAILED path={} detail={error}",
                path.display()
            );
            return ExitCode::from(2);
        }
        println!(
            "ASUPERSYNC_CONFORMANCE_GENERATED crates={} raw_command_sites={} asupersync_rev={} output={} command={COMMAND}",
            report.crates.len(),
            report.spawn_sites.len(),
            ASUPERSYNC_REV,
            path.display()
        );
    } else {
        print!("{document}");
    }
    ExitCode::SUCCESS
}

fn usage(error: &str) -> ExitCode {
    eprintln!("ASUPERSYNC_CONFORMANCE_ERROR reason=USAGE detail={error}");
    eprintln!("asupersync-conformance [--repo PATH] [--write PATH]");
    ExitCode::from(2)
}

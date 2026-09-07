#![forbid(unsafe_code)]

use asupersync_conformance::{
    check_document, render_document, scan_repository, CheckVerdict, ASUPERSYNC_REV,
};
use std::path::PathBuf;
use std::process::ExitCode;

const COMMAND: &str =
    "cargo run --quiet -p asupersync-conformance -- --repo . --write ASUPERSYNC-CONFORMANCE.md";

/// `--check` drift. Distinct from `2` (usage/scan error) so a reader tells "the doc is stale"
/// from "I could not look", which are different facts.
const EXIT_DRIFT: u8 = 1;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut repo = PathBuf::from(".");
    let mut output = None;
    let mut check = None;
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
            // omp-orchestrator-fsu7. `gate.yml` implemented the check as
            // `--write` followed by `git diff --exit-code`, which has TWO defects: it
            // MUTATES the tree during a gate run, so a concurrent agent's `git status` shows a
            // modified file that is not theirs; and it compares against the WORKTREE, so in a
            // shared checkout the verdict depends on a peer's uncommitted work and is false in
            // EITHER direction. `--check` compares in memory, writes nothing, and never consults
            // git. `--write` remains the human regeneration verb.
            "--check" => match args.next() {
                Some(path) => check = Some(PathBuf::from(path)),
                None => return usage("--check requires a path"),
            },
            "-h" | "--help" => {
                eprintln!("asupersync-conformance [--repo PATH] [--write PATH | --check PATH]");
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
    if let Some(path) = check {
        // Read-compare-report. No write, no git. A missing file is DRIFT with its own reason
        // rather than an error, because "the doc was never generated" and "the doc is stale" are
        // the same remedy: run --write.
        let current = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                println!(
                    "ASUPERSYNC_CONFORMANCE_DRIFT reason=UNREADABLE path={} detail={error} \
                     remedy={COMMAND}",
                    path.display()
                );
                return ExitCode::from(EXIT_DRIFT);
            }
        };
        match check_document(&current, &document) {
            CheckVerdict::Current => {
                println!(
                    "ASUPERSYNC_CONFORMANCE_CURRENT crates={} raw_command_sites={} \
                     asupersync_rev={} path={}",
                    report.crates.len(),
                    report.spawn_sites.len(),
                    ASUPERSYNC_REV,
                    path.display()
                );
                return ExitCode::SUCCESS;
            }
            CheckVerdict::Stale {
                stored_bytes,
                regenerated_bytes,
                first_differing_line,
            } => {
                // Name WHAT differs, not merely that something does: a byte count alone sends the
                // reader to `git diff`, which is the worktree comparison this flag exists to avoid.
                println!(
                    "ASUPERSYNC_CONFORMANCE_DRIFT reason=STALE path={} stored_bytes={stored_bytes} \
                     regenerated_bytes={regenerated_bytes} \
                     first_differing_line={first_differing_line} remedy={COMMAND}",
                    path.display()
                );
                return ExitCode::from(EXIT_DRIFT);
            }
        }
    }
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

/// 1-indexed first differing line, or `0` when the difference is only trailing content.
///
/// A line number is what makes a drift report actionable without reaching for `git diff` — which
/// is the worktree comparison `--check` exists to avoid.
fn first_differing_line(stored: &str, regenerated: &str) -> usize {
    for (index, (a, b)) in stored.lines().zip(regenerated.lines()).enumerate() {
        if a != b {
            return index + 1;
        }
    }
    0
}

fn usage(error: &str) -> ExitCode {
    eprintln!("ASUPERSYNC_CONFORMANCE_ERROR reason=USAGE detail={error}");
    eprintln!("asupersync-conformance [--repo PATH] [--write PATH]");
    ExitCode::from(2)
}

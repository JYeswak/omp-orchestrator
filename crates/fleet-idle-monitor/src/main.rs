#![forbid(unsafe_code)]

//! Thin CLI over [`fleet_idle_monitor`]. argv, two files, stdin, and the exit lattice.
//!
//! The queue payload arrives on STDIN rather than from a spawned tracker child, and that is
//! deliberate: the defect this crate answers (`omp-orchestrator-47g0`) was a child spawned
//! with a cwd the caller did not choose. A reader that cannot pick its own repository cannot
//! pick the wrong one.
//!
//! ```text
//! br ready --json --limit 0 | FLEET_SESSION=<session> fleet-idle-monitor
//! ```
//!
//! Inputs, none of which has a repository-shaped default:
//!   FLEET_SESSION        the tmux session whose panes are being nudged. REQUIRED.
//!   FLEET_QUEUE_REPO     the checkout the queue was read from. Defaults to
//!                        <FLEET_DEVELOPER_ROOT>/<FLEET_SESSION>, and is RECONCILED either way.
//!   FLEET_DEVELOPER_ROOT the directory holding checkouts. Defaults to $HOME/Developer.

use fleet_idle_monitor::{bind_exit_code, exit_code, session_repo, tick};
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: br ready --json --limit 0 | FLEET_SESSION=<session> fleet-idle-monitor\n\
                     env: FLEET_SESSION (required) FLEET_QUEUE_REPO FLEET_DEVELOPER_ROOT";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return ExitCode::from(0);
    }
    if let Some(unknown) = args.first() {
        eprintln!("fleet-idle-monitor: unknown argument {unknown}\n{USAGE}");
        return ExitCode::from(64);
    }

    let session = std::env::var("FLEET_SESSION").unwrap_or_default();
    let developer_root = match std::env::var_os("FLEET_DEVELOPER_ROOT").filter(|v| !v.is_empty()) {
        Some(root) => PathBuf::from(root),
        None => match std::env::var_os("HOME").filter(|v| !v.is_empty()) {
            Some(home) => PathBuf::from(home).join("Developer"),
            None => {
                eprintln!("fleet-idle-monitor: neither FLEET_DEVELOPER_ROOT nor HOME is set\n{USAGE}");
                return ExitCode::from(64);
            }
        },
    };
    let repo = match std::env::var_os("FLEET_QUEUE_REPO").filter(|v| !v.is_empty()) {
        Some(repo) => PathBuf::from(repo),
        None => session_repo(&developer_root, &session),
    };

    let tracker_config =
        std::fs::read_to_string(repo.join(".beads/config.yaml")).unwrap_or_default();

    let mut payload = String::new();
    if let Err(error) = std::io::stdin().read_to_string(&mut payload) {
        println!("QUEUE_UNOBSERVABLE_STDIN: {error}");
        return ExitCode::from(69);
    }

    match tick(&session, &repo, &tracker_config, &payload) {
        Ok(decision) => {
            println!("{decision}");
            ExitCode::from(exit_code(&decision))
        }
        Err(refusal) => {
            println!("{refusal}");
            ExitCode::from(bind_exit_code(&refusal))
        }
    }
}

//! The workspace-load check as a standalone tool — the per-copy gate for
//! extraction (bead omp-orchestrator-workspace-load-gate-of3).
//!
//! WHY A BIN: when a workspace manifest is broken, `cargo test` and `cargo
//! build` cannot run at all — but a PREVIOUSLY BUILT binary still can. Build
//! this once while the workspace is healthy, then invoke the binary directly
//! after every crate copy; it answers the load question even when cargo
//! cannot. Exit codes: 0 = loaded, 4 = any unloadable/vacuous outcome. The
//! DETECTOR name is on stdout so a harness asserts the case, not the code.
//!
//! Usage: `workspace-load [REPO_ROOT]` (default: the current directory).

#![forbid(unsafe_code)]

use no_shell_gate::check_workspace_load;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let root: PathBuf = match std::env::args_os().nth(1) {
        Some(arg) => PathBuf::from(arg),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };
    let verdict = check_workspace_load(&root);
    match &verdict {
        no_shell_gate::WorkspaceLoad::Loaded { members } => {
            println!("detector={}", verdict.detector());
            println!("members={}", members.join(","));
            ExitCode::SUCCESS
        }
        other => {
            println!("detector={}", other.detector());
            match other {
                no_shell_gate::WorkspaceLoad::ManifestMissing { path } => {
                    println!("path={path}");
                }
                no_shell_gate::WorkspaceLoad::MemberUnreadable { manifest, detail } => {
                    println!("manifest={manifest}");
                    println!("detail={detail}");
                }
                no_shell_gate::WorkspaceLoad::MembersEmpty => {}
                no_shell_gate::WorkspaceLoad::Loaded { .. } => unreachable!(),
            }
            ExitCode::from(4)
        }
    }
}

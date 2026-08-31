//! The no-shell gate binary — the surface CI invokes. Exit codes are the
//! contract: 0 = clean, 1 = tracked `.sh`/`.py` found, 2 = the gate could not
//! render a verdict (git failure or empty scan set — never a pass).
//!
//! Usage: `no-shell-gate [REPO_ROOT]` — defaults to this repository's root
//! (two levels above the crate), override to check any repo.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use no_shell_gate::{check_repo, Verdict};

fn main() -> ExitCode {
    let root: PathBuf = match std::env::args_os().nth(1) {
        Some(arg) => PathBuf::from(arg),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."),
    };
    let root = match root.canonicalize() {
        Ok(root) => root,
        Err(err) => {
            eprintln!("error: repo root {}: {err}", root.display());
            return ExitCode::from(2);
        }
    };
    match check_repo(&root) {
        Ok(Verdict::Clean) => {
            println!("ok: no tracked .sh or .py files in {}", root.display());
            ExitCode::SUCCESS
        }
        Ok(Verdict::Violations(violations)) => {
            eprintln!(
                "error: {} tracked shell/python file(s) in {}:",
                violations.len(),
                root.display()
            );
            for violation in &violations {
                eprintln!("  {violation}");
            }
            eprintln!(
                "hint: port it to Rust — the exemption list is empty by design; \
                 there is no check.sh carve-out"
            );
            ExitCode::from(1)
        }
        Err(err) => {
            eprintln!("error: gate could not render a verdict: {err}");
            ExitCode::from(2)
        }
    }
}

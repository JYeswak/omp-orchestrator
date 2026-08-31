#![forbid(unsafe_code)]

//! installer — one-touch install with four-way identity proof.
//!
//! THE DECIDING LEG: identity is PROVEN at install time, not asserted. Four-way:
//!   git rev-parse HEAD == build_id in the artifact's strings
//!   == what --version reports == what the running process reports.
//! Install FAILS if any pair disagrees.

use installer::{
    build_workspace, check_build_fence, git_head, install_binary, verify_identity,
    InstallError, InstallTarget,
};
use std::path::PathBuf;
use std::process::ExitCode;

const BINARIES: &[&str] = &["omp-orchestrator", "tick-monitor", "pane-truth"];

fn usage() {
    eprintln!(
        "installer [--check | --install | --version] [--bin-dir PATH]\n\
         \n\
         --check     Verify four-way identity for all binaries. Exits 0 (consistent)\n\
         \x20            or 1 (mismatch/drift). This is the pre-install gate.\n\
         --install   Build workspace (release), verify identity, install to bin-dir.\n\
         --version   Print the installer's own version.\n\
         --bin-dir   Target directory for installed binaries (default: ~/.local/bin)."
    );
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crate lives two levels below repo root")
        .to_path_buf();

    let bin_dir = std::env::var("INSTALL_BIN_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs_home().unwrap_or_else(|| PathBuf::from("/Users/josh")).join(".local/bin")
        });

    let verb = args.first().map(String::as_str).unwrap_or("--check");

    match verb {
        "--check" => run_check(&repo_root, &bin_dir),
        "--install" => run_install(&repo_root, &bin_dir),
        "--version" => {
            println!("installer 0.1.0");
            ExitCode::SUCCESS
        }
        "-h" | "--help" => {
            usage();
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("installer: unknown verb {verb:?}");
            usage();
            ExitCode::from(2)
        }
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn run_check(repo_root: &PathBuf, bin_dir: &PathBuf) -> ExitCode {
    let head = match installer::git_head(repo_root) {
        Ok(sha) => sha,
        Err(error) => {
            eprintln!("INSTALLER ERROR: {error}");
            return ExitCode::from(3);
        }
    };
    let head_short = installer::git_rev_parse_short(repo_root).unwrap_or_default();

    println!("installer --check: HEAD={head_short}");

    let mut mismatches = 0usize;
    let mut checked = 0usize;

    for name in BINARIES {
        let binary = bin_dir.join(name);
        if !binary.exists() {
            println!("  {name}: NOT INSTALLED (skipped)");
            continue;
        }
        checked += 1;
        let check = installer::verify_identity(&binary, &head);
        println!("  {check}");
        if !check.consistent {
            mismatches += 1;
        }
    }

    if checked == 0 {
        eprintln!("INSTALLER ERROR: no binaries found in {} — nothing to check", bin_dir.display());
        return ExitCode::from(3);
    }

    if mismatches > 0 {
        eprintln!(
            "INSTALLER IDENTITY DRIFT: {mismatches}/{checked} binaries disagree with HEAD {head_short}"
        );
        return ExitCode::from(1);
    }

    println!("INSTALLER IDENTITY OK: {checked}/{checked} binaries consistent with HEAD {head_short}");
    ExitCode::SUCCESS
}

fn run_install(repo_root: &PathBuf, bin_dir: &PathBuf) -> ExitCode {
    // Build-in-flight fence.
    if let Err(error) = installer::check_build_fence(repo_root) {
        eprintln!("INSTALLER BLOCKED: {error}");
        return ExitCode::from(75);
    }

    // Build the workspace.
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "~/.cargo/bin/cargo".to_owned());
    let cargo = shellexpand_path(&cargo);
    if let Err(error) = installer::build_workspace(repo_root, &cargo) {
        eprintln!("INSTALLER BUILD FAILED: {error}");
        return ExitCode::from(2);
    }

    // Resolve HEAD after the build (the build does not change HEAD, but the check
    // must happen after the build to catch a concurrent commit during the build).
    let head = match installer::git_head(repo_root) {
        Ok(sha) => sha,
        Err(error) => {
            eprintln!("INSTALLER ERROR: {error}");
            return ExitCode::from(3);
        }
    };

    // Discover binaries from the target dir.
    let target_dir = repo_root.join("target/release");
    let mut installed = Vec::new();
    let mut identity_checks = Vec::new();

    for name in BINARIES {
        let source = target_dir.join(name);
        if !source.exists() {
            continue;
        }
        match installer::install_binary(&source, bin_dir, &head) {
            Ok(check) => {
                let detail = format!("{check}");
                installed.push(detail);
                identity_checks.push(check);
            }
            Err(error) => {
                eprintln!("INSTALLER ERROR: {error}");
                return ExitCode::from(1);
            }
        }
    }

    if installed.is_empty() {
        eprintln!(
            "INSTALLER ERROR: no binaries found in target/release — did the build produce any?"
        );
        return ExitCode::from(3);
    }

    // Summary.
    println!("INSTALLER: {} binaries installed to {}", installed.len(), bin_dir.display());
    for check in &identity_checks {
        println!("  {check}");
    }
    ExitCode::SUCCESS
}

fn shellexpand_path(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return format!("{}/{}", PathBuf::from(home).display(), &path[2..]);
        }
    }
    path.to_owned()
}

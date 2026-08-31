#![forbid(unsafe_code)]

//! installer — one-touch install with four-way identity proof.
//!
//! main wires the subprocess calls (git, cargo) to the lib's identity check.
//! Single writer per file: SilverWolf owns main.rs; pane 1 owns lib.rs.

use installer::{resolve_repo_ownership, verify_identity, RepoOwnership};
use std::path::PathBuf;
use std::process::ExitCode;

const BINARIES: &[&str] = &["omp-orchestrator", "tick-monitor", "pane-truth"];

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

    match args.first().map(String::as_str) {
        Some("--check") => run_check(&repo_root, &bin_dir),
        Some("--install") => run_install(&repo_root, &bin_dir),
        Some("--version") => {
            println!("installer 0.1.0");
            ExitCode::SUCCESS
        }
        Some("-h") | Some("--help") => {
            usage();
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("installer: unknown verb {other:?}");
            usage();
            ExitCode::from(2)
        }
        None => run_check(&repo_root, &bin_dir),
    }
}

fn usage() {
    eprintln!("installer [--check | --install | --version] [--bin-dir PATH]");
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
    let mut foreign = 0usize;

    for name in BINARIES {
        let binary = bin_dir.join(name);
        if !binary.exists() {
            println!("  {name}: NOT INSTALLED (skipped)");
            continue;
        }
        let ownership = installer::resolve_repo_ownership(repo_root, name);
        let check = installer::verify_identity(&binary, &head, &ownership);
        println!("  {check}");
        match (&ownership, check.consistent) {
            (RepoOwnership::Foreign { .. }, _) => foreign += 1,
            _ if check.consistent => {}
            _ => mismatches += 1,
        }
    }

    if mismatches > 0 {
        eprintln!(
            "INSTALLER IDENTITY DRIFT: {mismatches}/{checked} binaries disagree with HEAD {head_short}"
        );
        return ExitCode::from(1);
    }
    if foreign > 0 {
        println!(
            "INSTALLER: {foreign} foreign artifact(s) named — excluded from drift denominator"
        );
    }
    println!(
        "INSTALLER IDENTITY OK: {checked}/{checked} binaries consistent with HEAD {head_short}"
    );
    ExitCode::SUCCESS
}

fn run_install(repo_root: &PathBuf, bin_dir: &PathBuf) -> ExitCode {
    if let Err(error) = installer::check_build_fence(repo_root) {
        eprintln!("INSTALLER BLOCKED: {error}");
        return ExitCode::from(75);
    }

    let cargo = std::env::var("CARGO")
        .unwrap_or_else(|_| "~/.cargo/bin/cargo".to_owned());
    let cargo = shellexpand_path(&cargo);
    if let Err(error) = installer::build_workspace(repo_root, &cargo) {
        eprintln!("INSTALLER BUILD FAILED: {error}");
        return ExitCode::from(2);
    }

    let head = match installer::git_head(repo_root) {
        Ok(sha) => sha,
        Err(error) => {
            eprintln!("INSTALLER ERROR: {error}");
            return ExitCode::from(3);
        }
    };

    let target_dir = repo_root.join("target/release");
    let mut installed_count = 0usize;
    let mut identity_checks = Vec::new();

    for name in BINARIES {
        let source = target_dir.join(name);
        if !source.exists() {
            continue;
        }
        let ownership = installer::resolve_repo_ownership(repo_root, name);
        match installer::install_binary(&source, bin_dir, &head, &ownership) {
            Ok(check) => {
                println!("  INSTALLED {name}: {check}");
                installed_count += 1;
                identity_checks.push(check);
            }
            Err(error) => {
                eprintln!("INSTALLER ERROR: {error}");
                return ExitCode::from(1);
            }
        }
    }

    if installed_count == 0 {
        eprintln!("INSTALLER ERROR: no binaries found in target/release");
        return ExitCode::from(3);
    }

    println!("INSTALLER: {installed_count} binaries installed");
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

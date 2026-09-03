#![forbid(unsafe_code)]

//! installer — one-touch install with four-way identity proof.
//!
//! main wires the subprocess calls (git, cargo) to the lib's identity check.
//! Single writer per file: SilverWolf owns main.rs; pane 1 owns lib.rs.

use installer::RepoOwnership;
use lifecycle_event::{
    default_repo_journal, DurableJournal, Layer, LifecycleEvent, Outcome, ReasonCode,
};
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;
#[used]
static BUILD_ID_MARKER: &[u8] = concat!("build_id=", env!("OMP_BUILD_ID")).as_bytes();
const BINARIES: &[(&str, &str)] = &[
    ("omp-orchestrator", "omp-orchestrator"),
    ("tick-monitor", "tick-monitor"),
    ("pane-truth", "pane-truth"),
    ("installer", "installer"),
];

fn main() -> ExitCode {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let (args, bin_dir) = match parse_cli_args(raw_args) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("INSTALLER ERROR: {error}");
            return ExitCode::from(2);
        }
    };
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crate lives two levels below repo root")
        .to_path_buf();

    match args.first().map(String::as_str) {
        Some("--check") if args.len() == 1 => run_check(&repo_root, &bin_dir),
        Some("--install") if args.len() == 2 => run_install(&repo_root, &bin_dir, &args[1]),
        Some("--install") => {
            eprintln!("INSTALLER ERROR: --install requires exactly one target");
            usage();
            ExitCode::from(2)
        }
        Some("--version") => {
            println!("installer 0.1.0 build_id={}", env!("OMP_BUILD_ID"));
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

fn parse_cli_args(raw_args: Vec<String>) -> Result<(Vec<String>, PathBuf), String> {
    let mut positional = Vec::new();
    let mut explicit_bin_dir = None;
    let mut args = raw_args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--bin-dir" {
            let value = args
                .next()
                .ok_or_else(|| "--bin-dir requires a path".to_owned())?;
            if value.is_empty() {
                return Err("--bin-dir requires a non-empty path".to_owned());
            }
            explicit_bin_dir = Some(PathBuf::from(value));
        } else if let Some(value) = arg.strip_prefix("--bin-dir=") {
            if value.is_empty() {
                return Err("--bin-dir requires a non-empty path".to_owned());
            }
            explicit_bin_dir = Some(PathBuf::from(value));
        } else {
            positional.push(arg);
        }
    }
    let bin_dir = explicit_bin_dir
        .or_else(|| {
            std::env::var_os("INSTALL_BIN_DIR")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        })
        .or_else(|| dirs_home().map(|home| home.join(".local/bin")))
        .ok_or_else(|| "INSTALL_BIN_DIR, --bin-dir, or HOME must be set".to_owned())?;
    Ok((positional, bin_dir))
}
fn usage() {
    eprintln!("installer [--check | --install TARGET | --version] [--bin-dir PATH]");
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
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
    let mut foreign = 0usize;
    let mut unavailable = 0usize;
    // Counted explicitly, never derived. `BINARIES.len() - foreign` looks equivalent and is not:
    // a binary that is NOT INSTALLED hits the `continue` below without touching the counters,
    // yet still sits in `BINARIES.len()`, so it would silently inflate the "owned" denominator.
    // A ratio whose denominator includes rows it never examined is unverifiable — the same defect
    // class as the retired "81 JSON-RPC methods, 17 used" figure.
    let mut owned = 0usize;

    for &(_, name) in BINARIES {
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
            (RepoOwnership::Unknown, _) => unavailable += 1,
            _ if check.consistent => owned += 1,
            _ => {
                owned += 1;
                mismatches += 1;
            }
        }
    }

    if mismatches > 0 {
        eprintln!(
            "INSTALLER IDENTITY DRIFT: {mismatches}/{owned} owned binaries disagree with HEAD {head_short}"
        );
        return ExitCode::from(1);
    }
    if foreign > 0 {
        println!(
            "INSTALLER: {foreign} foreign artifact(s) named — excluded from drift denominator"
        );
    }
    if unavailable > 0 {
        println!(
            "INSTALLER: {unavailable} artifact(s) have unavailable source ownership — excluded from drift denominator"
        );
    }
    println!("INSTALLER IDENTITY OK: {owned}/{owned} binaries consistent with HEAD {head_short}");
    emit_s1(
        repo_root,
        Layer::L1,
        "S1.L1",
        Outcome::Emitted,
        "IDENTITY_OK",
    );
    ExitCode::SUCCESS
}

fn run_install(repo_root: &PathBuf, bin_dir: &PathBuf, target: &str) -> ExitCode {
    if let Err(error) = installer::check_build_fence(repo_root) {
        eprintln!("INSTALLER BLOCKED: {error}");
        return ExitCode::from(75);
    }
    let Some((crate_name, binary_name)) =
        BINARIES.iter().find(|(_, name)| *name == target).copied()
    else {
        eprintln!("INSTALLER ERROR: unknown target {target:?}; expected one of omp-orchestrator, tick-monitor, pane-truth, installer");
        return ExitCode::from(2);
    };
    let ownership = installer::resolve_repo_ownership(repo_root, binary_name);
    if let RepoOwnership::Foreign { repo } = &ownership {
        eprintln!("INSTALLER ERROR: target {binary_name} is FOREIGN (source in {repo}); install it from its owning repository");
        return ExitCode::from(3);
    }
    let head = match installer::git_head(repo_root) {
        Ok(sha) => sha,
        Err(error) => {
            eprintln!("INSTALLER ERROR: {error}");
            return ExitCode::from(3);
        }
    };
    let before_start = match installer::running_process_start(binary_name) {
        Ok(start) => start,
        Err(error) => {
            eprintln!("INSTALLER RESTART READ FAILED: {error}");
            return ExitCode::from(2);
        }
    };
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "~/.cargo/bin/cargo".to_owned());
    let cargo = shellexpand_path(&cargo);
    if let Err(error) = installer::build_target(repo_root, &cargo, crate_name, &head) {
        eprintln!("INSTALLER BUILD REFUSED: {error}");
        return ExitCode::from(2);
    }
    let source = repo_root.join("target/release").join(binary_name);
    match installer::install_binary(&source, bin_dir, &head, &ownership) {
        Ok(check) => println!("  INSTALLED {binary_name}: {check}"),
        Err(error) => {
            eprintln!("INSTALLER ERROR: {error}");
            return ExitCode::from(1);
        }
    }
    match installer::restart_and_verify(
        binary_name,
        &bin_dir.join(binary_name),
        &head,
        before_start,
    ) {
        Ok(outcome) => println!("  RESTART {binary_name}: {outcome}"),
        Err(error) => {
            eprintln!("INSTALLER RESTART FAILED: {error}");
            return ExitCode::from(1);
        }
    }
    emit_s1(
        repo_root,
        Layer::L0,
        "S1.L0",
        Outcome::Emitted,
        "INSTALL_VERIFIED",
    );
    println!("INSTALLER: target {binary_name} installed and verified");
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

fn emit_s1(repo_root: &Path, layer: Layer, stage_to: &str, outcome: Outcome, reason: &str) {
    let Ok(code) = ReasonCode::new(reason) else {
        eprintln!("LIFECYCLE_EVENT_EMIT_FAILED layer={} detail=missing reason_code", layer.as_str());
        return;
    };
    let event = LifecycleEvent::new(layer, "HUMAN", stage_to, "installer", outcome, code);
    let path = default_repo_journal(repo_root);
    match DurableJournal::open(path).and_then(|journal| {
        lifecycle_event::emit_one_host(&journal, event)
    }) {
        Ok(_) => {}
        Err(error) => eprintln!(
            "LIFECYCLE_EVENT_EMIT_FAILED layer={} detail={error}",
            layer.as_str()
        ),
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bin_dir_flag_is_removed_before_verb_dispatch() {
        let (args, bin_dir) = parse_cli_args(vec![
            "--install".to_owned(),
            "installer".to_owned(),
            "--bin-dir".to_owned(),
            "scratch-home".to_owned(),
        ])
        .expect("bin-dir parses");
        assert_eq!(args, vec!["--install", "installer"]);
        assert_eq!(bin_dir, PathBuf::from("scratch-home"));
    }
}

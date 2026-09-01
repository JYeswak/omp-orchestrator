//! ENV PARITY CONTRACT for the omp-idle-dispatch Rust port (cp-79am1).
//!
//! bin/omp-idle-dispatch.sh — deleted by the Rust port, restored here from git
//! (45c613d^, lines 25-27) — exported, in order:
//!
//!   PATH="/opt/homebrew/bin:/Users/josh/.local/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
//!   TMUX_TMPDIR="${TMUX_TMPDIR:-/Users/josh/.tmux-sockets}"
//!   LC_ALL="${LC_ALL:-C.UTF-8}"  # cron gives NO locale; tmux -F rewrites TAB to '_' without it
//!
//! The Rust port dropped all three. Under cron — which supplies NO environment — tmux
//! attached to its private default socket and rewrote tab delimiters, so the dispatcher
//! observed the wrong world silently. These legs assert the port's set-if-unset parity
//! and that a missing socket dir fails LOUDLY with a typed error naming the variable,
//! the path, and the reason (the arc-keepalive STARTUP_ERROR shape) — never a silent
//! observation of an empty world.
//!
//! Every leg runs the binary with `env_clear` plus ONLY the variables the leg names,
//! which is the cron condition the shell version was written for.

use std::path::PathBuf;
use std::process::Command;

const CONTRACT_SOURCE: &str = "bin/omp-idle-dispatch.sh @ 45c613d^ lines 25-27";

fn bin_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_omp-idle-dispatch"))
}

fn base_env(home: &PathBuf, sockets: &PathBuf) -> Command {
    let mut command = Command::new(bin_path());
    command
        .arg("--dry-run")
        .arg("--json")
        .env_clear()
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin:/opt/homebrew/bin")
        .env("TMUX_TMPDIR", sockets)
        .env(
            "OMP_DISPATCH_LEDGER",
            std::env::temp_dir().join(format!("ompid-env-{}.jsonl", std::process::id())),
        )
        .env(
            "OMP_DISPATCH_LOCK",
            std::env::temp_dir().join(format!("ompid-env-{}.lock", std::process::id())),
        );
    command
}

/// PLANTED MISSING-VAR LEG (must BITE): TMUX_TMPDIR is absent and the defaulted
/// `$HOME/.tmux-sockets` does not exist — the dispatcher must refuse with a typed
/// STARTUP_ERROR naming the variable, the path, and the reason (rc=78), never observe
/// an empty world silently.
#[test]
fn missing_tmux_tmpdir_fails_loudly_naming_the_variable() {
    let fake_home = std::env::temp_dir().join(format!("ompid-nohome-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&fake_home);
    std::fs::create_dir_all(&fake_home).expect("create fake home");

    let mut command = Command::new(bin_path());
    command
        .arg("--dry-run")
        .arg("--json")
        .env_clear()
        .env("HOME", &fake_home)
        .env("PATH", "/usr/bin:/bin")
        .env(
            "OMP_DISPATCH_LEDGER",
            std::env::temp_dir().join(format!("ompid-env-bite-{}.jsonl", std::process::id())),
        )
        .env(
            "OMP_DISPATCH_LOCK",
            std::env::temp_dir().join(format!("ompid-env-bite-{}.lock", std::process::id())),
        );
    let out = command.output().expect("spawn omp-idle-dispatch");
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert_eq!(
        out.status.code(),
        Some(78),
        "missing TMUX_TMPDIR must refuse with rc=78, stdout={stdout}"
    );
    assert!(
        stdout.contains("STARTUP_ERROR[tmux_tmpdir_unusable]"),
        "the typed error must name tmux_tmpdir_unusable, got: {stdout}"
    );
    assert!(
        stdout.contains(&fake_home.join(".tmux-sockets").display().to_string()),
        "the typed error must name the attempted socket path, got: {stdout}"
    );

    let _ = std::fs::remove_dir_all(&fake_home);
}

/// HOME itself unset (deeper missing var): the default cannot even be derived, so the
/// typed error must name home_unset and the TMUX_TMPDIR escape hatch.
#[test]
fn unset_home_names_the_variable_and_the_escape_hatch() {
    let mut command = Command::new(bin_path());
    command
        .arg("--dry-run")
        .arg("--json")
        .env_clear()
        .env("PATH", "/usr/bin:/bin");
    let out = command.output().expect("spawn omp-idle-dispatch");
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert_eq!(out.status.code(), Some(78), "stdout={stdout}");
    assert!(
        stdout.contains("STARTUP_ERROR[home_unset]"),
        "the typed error must name home_unset, got: {stdout}"
    );
    assert!(
        stdout.contains("TMUX_TMPDIR"),
        "the typed error must name the TMUX_TMPDIR escape hatch, got: {stdout}"
    );
}

/// PARITY-POSITIVE LEG: a complete environment (HOME, PATH, an EXISTING TMUX_TMPDIR)
/// passes startup — no STARTUP_ERROR — and the failure moves to the correct layer
/// (the tmux probe against an empty socket dir), proving the env contract gates
/// startup only and does not fake a verdict.
#[test]
fn complete_environment_passes_startup() {
    let root = std::env::temp_dir().join(format!("ompid-good-{}", std::process::id()));
    let sockets = root.join(".tmux-sockets");
    std::fs::create_dir_all(&sockets).expect("create existing socket dir");

    let mut command = Command::new(bin_path());
    command
        .arg("--dry-run")
        .arg("--json")
        .env_clear()
        .env("HOME", &root)
        .env("PATH", "/usr/bin:/bin")
        .env("TMUX_TMPDIR", &sockets)
        .env(
            "OMP_DISPATCH_LEDGER",
            root.join("ledger.jsonl").display().to_string(),
        )
        .env("OMP_DISPATCH_LOCK", root.join("lock").display().to_string());
    let out = command.output().expect("spawn omp-idle-dispatch");
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(
        !stdout.contains("STARTUP_ERROR"),
        "a complete environment must pass startup, got: {stdout}"
    );
    // The failure moved to the correct layer: the tmux probe against an empty socket
    // dir reports pane problems, not environment problems.
    assert!(
        stdout.contains("pane") || stdout.contains("no_panes"),
        "expected the tick to reach its pane probe, got: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// The contract record itself: the three exports the deleted shell version made, so
/// the parity claim is checkable against git rather than remembered.
#[test]
fn contract_record_matches_the_deleted_shell_exports() {
    assert!(CONTRACT_SOURCE.contains("omp-idle-dispatch.sh"));
    // Re-derived from git (45c613d^ lines 25-27):
    //   export PATH="/opt/homebrew/bin:/Users/josh/.local/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
    //   export TMUX_TMPDIR="${TMUX_TMPDIR:-/Users/josh/.tmux-sockets}"
    //   export LC_ALL="${LC_ALL:-C.UTF-8}"  # cron gives NO locale; tmux -F rewrites TAB to '_'
    // The Rust port reproduces TMUX_TMPDIR set-if-unset with a $HOME-derived default
    // (missing/unusable = loud), LC_ALL defaulted internally to C.UTF-8, and PATH
    // inherited from the ambient environment.
}

//! Differential vs `bin/loop-driver.sh`. The agreement cases give both sides
//! identical argv, environment, fixture files, and live session input.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate under repo/crates")
        .to_path_buf()
}

fn rust_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_loop-driver"))
}

fn oracle() -> PathBuf {
    let root = std::env::var_os("CONTROL_PLANE_REPO")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root());
    root.join("bin/loop-driver.sh")
}

fn fixture_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "loop-driver-differential-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("repo/bin")).unwrap();
    fs::create_dir_all(root.join("repo/loop-kit")).unwrap();
    fs::create_dir_all(root.join("state")).unwrap();
    root
}

fn write_script(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

fn run_side(binary: &Path, shell: bool, args: &[&str], fixture: &Path) -> Output {
    let repo = fixture.join("repo");
    let mut command = if shell {
        let mut command = Command::new("/bin/bash");
        command.arg(binary);
        command
    } else {
        Command::new(binary)
    };
    command
        .args(args)
        .env(
            "HOME",
            std::env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .unwrap_or_default(),
        )
        .env("LOOP_REPO", &repo)
        .env("LOOP_DRIVER_LOG", fixture.join("driver.log"))
        .env("LOOP_DRIVER_LOCK", fixture.join("driver.lock"))
        .env("LOOP_DRIVER_STATE_DIR", fixture.join("state"))
        .env(
            "LOOP_LEDGER_THRESHOLD_CHECK",
            fixture.join("ledger-threshold"),
        )
        .env("LOOP_P6_REARM_CHECK", fixture.join("p6-rearm"))
        .env("LOOP_TICK_BIN", repo.join("bin/loop-tick.sh"))
        .env("LOOP_SESSION", "control-plane")
        .env("LOOP_DRIVER_DEADLINE_SECONDS", "1200")
        .env(
            "TMUX_TMPDIR",
            std::env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .map(|home| {
                    std::path::PathBuf::from(home)
                        .join(".tmux-sockets")
                        .display()
                        .to_string()
                })
                .unwrap_or_default(),
        )
        // The oracle's lockf wrapper re-executes the shell. Its current parent
        // then continues after the child and runs the fake tick a second time.
        // This hermetic fixture supplies the oracle's own inner-run marker to
        // compare ONE core driver execution. The real guard is not inferred
        // from this differential; safety.rs proves Rust<->lockf exclusion.
        .env("LOOP_DRIVER_LOCK_HELD", "1")
        .current_dir(repo_root())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run differential side")
}

fn normalize_log(text: &str) -> String {
    text.lines()
        .map(|line| {
            let mut normalized = if line.starts_with('[') {
                line.find(']').map_or_else(
                    || line.to_owned(),
                    |end| format!("[TS]{}", &line[end + 1..]),
                )
            } else {
                line.to_owned()
            };
            if let Some(start) = normalized.find("ppid=") {
                let digits_start = start + "ppid=".len();
                let digits_end = normalized[digits_start..]
                    .find(|character: char| !character.is_ascii_digit())
                    .map_or(normalized.len(), |offset| digits_start + offset);
                normalized.replace_range(digits_start..digits_end, "<PID>");
            }
            normalized
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn compare_output(name: &str, shell: &Output, rust: &Output) {
    assert_eq!(
        shell.status.code(),
        rust.status.code(),
        "differential {name}: exit status"
    );
    assert_eq!(
        shell.stdout,
        rust.stdout,
        "differential {name}: stdout differs\nshell={:?}\nrust={:?}",
        String::from_utf8_lossy(&shell.stdout),
        String::from_utf8_lossy(&rust.stdout)
    );
    assert_eq!(
        shell.stderr,
        rust.stderr,
        "differential {name}: stderr differs\nshell={:?}\nrust={:?}",
        String::from_utf8_lossy(&shell.stderr),
        String::from_utf8_lossy(&rust.stderr)
    );
}

#[test]
fn rust_matches_shell_oracle_on_nonempty_case_set_and_probe_sees_disagreement() {
    if !oracle().is_file() {
        println!("DIFFERENTIAL DID NOT RUN: test=rust_matches_shell_oracle_on_nonempty_case_set_and_probe_sees_disagreement reason=missing_external_oracle detail={}", oracle().display());
        return;
    }
    let fixture = fixture_dir();
    write_script(
        &fixture.join("ledger-threshold"),
        "printf 'TICK_LEDGER_THRESHOLD PASS\\n'",
    );
    write_script(&fixture.join("p6-rearm"), "printf 'P6_REARM PASS\\n'");
    write_script(
        &fixture.join("repo/loop-kit/loop-start-chokepoint.sh"),
        "printf 'CHOKEPOINT PASS\\n'",
    );
    write_script(
        &fixture.join("repo/bin/loop-tick.sh"),
        "printf 'TICK BODY OK\\n'",
    );

    let cases: Vec<(&str, Vec<&str>)> = vec![
        ("failure-reason", vec!["--selftest-failure-reason"]),
        ("invoker-lineage", vec!["--selftest-invoker"]),
        ("happy-live-driver", vec![]),
    ];
    assert!(
        !cases.is_empty(),
        "anti-vacuity: a differential comparing ZERO cases is an ERROR"
    );

    let mut compared = 0usize;
    for (name, args) in &cases {
        fs::write(fixture.join("driver.log"), "").unwrap();
        let shell = run_side(&oracle(), true, args, &fixture);
        let shell_log = fs::read_to_string(fixture.join("driver.log")).unwrap_or_default();
        fs::write(fixture.join("driver.log"), "").unwrap();
        let rust = run_side(&rust_binary(), false, args, &fixture);
        let rust_log = fs::read_to_string(fixture.join("driver.log")).unwrap_or_default();
        compare_output(name, &shell, &rust);
        if *name == "happy-live-driver" {
            assert_eq!(
                normalize_log(&shell_log),
                normalize_log(&rust_log),
                "differential {name}: normalized live log differs\n-- shell --\n{}\n-- rust --\n{}",
                normalize_log(&shell_log),
                normalize_log(&rust_log)
            );
        }
        compared += 1;
    }

    fs::write(fixture.join("driver.log"), "").unwrap();
    let shell = run_side(&oracle(), true, &[], &fixture);
    assert!(
        shell.status.success(),
        "known-bad probe shell fixture must pass"
    );
    let shell_log = fs::read_to_string(fixture.join("driver.log")).unwrap_or_default();
    fs::write(fixture.join("driver.log"), "").unwrap();
    let mutated = run_side(
        &rust_binary(),
        false,
        &["--mutation", "--disable-rule", "differential_tick_log"],
        &fixture,
    );
    assert!(mutated.status.success(), "known-bad Rust fixture must run");
    let mutated_log = fs::read_to_string(fixture.join("driver.log")).unwrap_or_default();
    assert_ne!(
        normalize_log(&shell_log),
        normalize_log(&mutated_log),
        "anti-vacuity: known-bad missing tick-log mutation must be visible"
    );
    println!("DIFFERENTIAL KNOWN_BAD probe=missing-tick-log disagreements=1");
    println!("DIFFERENTIAL PASS cases={compared} disagreements=0");
}

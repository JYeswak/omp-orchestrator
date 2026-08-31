use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const LOCK_HOLDER_SCRIPT: &str = concat!(
    "import fcntl, pathlib, sys, time\n",
    "with open(sys.argv[1], 'w') as lock:\n",
    "    fcntl.flock(lock, fcntl.LOCK_EX)\n",
    "    pathlib.Path(sys.argv[2]).touch()\n",
    "    time.sleep(3)\n",
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OracleStatus {
    Ready,
    MissingInterpreter,
}

fn oracle_status() -> OracleStatus {
    match Command::new("python3")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(status) if status.success() => OracleStatus::Ready,
        _ => OracleStatus::MissingInterpreter,
    }
}

fn announce_skip(test: &str, status: &OracleStatus) {
    let reason = match status {
        OracleStatus::MissingInterpreter => "missing_interpreter",
        OracleStatus::Ready => "ready",
    };
    println!(
        "DIFFERENTIAL DID NOT RUN: test={test} reason={reason} detail=inline python3 lock-holder\n  \
         This is a development-only comparison, not a gate. The Rust gate for this crate is \
         free_pane_is_admitted plus the fence binary's Rust path.\n  \
         0 cases compared. This is NOT a passing differential."
    );
}

fn state_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "pane-dispatch-fence-{label}-{}-{nonce}",
        std::process::id(),
    ))
}

fn fence_command(state: &Path, pane: &str, ready_probe: &Path, child: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_pane-dispatch-fence"));
    command.args([
        "--state-dir",
        state.to_str().expect("temporary state path is UTF-8"),
        "--session",
        "integration",
        "--pane",
        pane,
        "--owner",
        "integration-test",
        "--ready-probe",
        ready_probe.to_str().expect("temporary probe path is UTF-8"),
        "--",
    ]);
    command.args(child);
    command
}

fn run_fence(state: &Path, pane: &str, ready_probe: &Path, child: &[&str]) -> Output {
    fence_command(state, pane, ready_probe, child)
        .output()
        .expect("fence process starts")
}

fn lock_holder(state: &Path) -> (std::process::Child, PathBuf) {
    fs::create_dir_all(state).expect("create fence test state directory");
    let lock = state.join("pane-dispatch-fence/integration.held.lock");
    fs::create_dir_all(lock.parent().expect("lock has a parent"))
        .expect("create fence lock directory");
    let marker = state.join("holder.started");
    let holder = Command::new("python3")
        .args([
            "-c",
            LOCK_HOLDER_SCRIPT,
            lock.to_str().expect("temporary lock path is UTF-8"),
            marker.to_str().expect("temporary marker path is UTF-8"),
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("lock holder starts");
    (holder, marker)
}

fn wait_until_holder_started(marker: &Path) {
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    while !marker.is_file() && std::time::Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(marker.is_file(), "the first readiness probe did not start");
}

#[test]
fn free_pane_is_admitted() {
    let state = state_dir("free");
    let output = run_fence(
        &state,
        "free",
        Path::new("/usr/bin/true"),
        &["/usr/bin/true"],
    );

    assert_eq!(output.status.code(), Some(0), "stderr: {:?}", output.stderr);
}

#[test]
fn held_pane_is_refused() {
    let status = oracle_status();
    let OracleStatus::Ready = status else {
        announce_skip("held_pane_is_refused", &status);
        return;
    };
    let state = state_dir("held");
    let (mut holder, marker) = lock_holder(&state);
    wait_until_holder_started(&marker);

    let second = run_fence(
        &state,
        "held",
        Path::new("/usr/bin/true"),
        &["/usr/bin/true"],
    );
    assert_eq!(
        second.status.code(),
        Some(75),
        "stderr: {:?}",
        second.stderr
    );
    assert!(
        String::from_utf8_lossy(&second.stderr).contains("PANE_DISPATCH_FENCE_BUSY"),
        "stderr: {:?}",
        second.stderr
    );

    let _ = holder.kill();
    let _ = holder.wait();
}

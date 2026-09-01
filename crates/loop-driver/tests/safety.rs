use loop_driver::{InstanceGuard, LockMetadata, LockRules, EXIT_CONCURRENT, EXIT_DEADLINE};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_loop-driver"))
}

fn fixture_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "loop-driver-{label}-{}-{nonce}.lock",
        std::process::id()
    ))
}

fn start_holder(lock: &Path, seconds: u64) -> Child {
    let mut child = Command::new(binary())
        .args(["--hold-lock", &seconds.to_string()])
        .env("LOOP_DRIVER_LOCK", lock)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn Rust lock holder");
    let line = {
        let mut stdout = child.stdout.take().expect("holder stdout");
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line).expect("read holder receipt");
        line
    };
    assert!(
        line.starts_with("LOCK_HELD pid="),
        "holder did not acquire the lock: {line:?}"
    );
    child
}

#[test]
fn single_instance_guard_refuses_second_live_instance() {
    let lock = fixture_path("live");
    let mut holder = start_holder(&lock, 4);
    let holder_pid = holder.id();
    let output = Command::new(binary())
        .arg("--lock-probe")
        .env("LOOP_DRIVER_LOCK", &lock)
        .output()
        .expect("run second instance");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(EXIT_CONCURRENT),
        "single-instance guard: a second LIVE instance must be REFUSED; stdout={stdout:?} stderr={:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.starts_with("LOOP_DRIVER_REFUSED code=75 reason=live_instance"),
        "single-instance guard verdict must be on stdout at column 0: {stdout:?}"
    );
    assert!(
        stdout.contains(&format!("holder_pid={holder_pid}"))
            && stdout.contains("holder_elapsed=")
            && !stdout.contains("holder_pid=unknown")
            && !stdout.contains("holder_elapsed=unknown")
            && (stdout.contains("holder_liveness=LIVE")
                || stdout.contains("holder_liveness=WEDGED")),
        "single-instance guard refusal must name the holder pid, elapsed, and LIVE or WEDGED: {stdout:?}"
    );
    assert!(
        output.stderr.is_empty(),
        "verdicts belong on stdout; stderr is usage errors only"
    );
    let _ = holder.kill();
    let _ = holder.wait();
}

#[test]
fn stale_dead_holder_is_recovered_and_acquired() {
    let lock = fixture_path("stale");
    let metadata = LockMetadata {
        pid: 4_000_000,
        started_unix_ms: 1,
    };
    fs::write(
        &lock,
        format!("{}\n", serde_json::to_string(&metadata).unwrap()),
    )
    .expect("write stale metadata");
    let guard = InstanceGuard::acquire(&lock, LockRules::default()).unwrap_or_else(|error| {
        panic!("stale recovery: a WEDGED OLD/dead holder must be ACQUIRED: {error}")
    });
    assert_eq!(
        guard.recovered_dead_holder(),
        Some(metadata.pid),
        "stale recovery: acquisition must name the dead holder it recovered"
    );
}

#[test]
fn rust_lock_interoperates_with_the_shell_oracles_lockf() {
    let lock = fixture_path("lockf");
    let mut rust_holder = start_holder(&lock, 4);
    let lockf_probe = Command::new("/usr/bin/lockf")
        .args(["-s", "-t", "0"])
        .arg(&lock)
        .arg("/usr/bin/true")
        .status()
        .expect("run lockf probe");
    assert_eq!(
        lockf_probe.code(),
        Some(EXIT_CONCURRENT),
        "serialization boundary: lockf must see the Rust-held lock; otherwise shell and Rust could both dispatch"
    );
    let _ = rust_holder.kill();
    let _ = rust_holder.wait();

    let ready = fixture_path("lockf-ready");
    let script = format!("printf ready > '{}'; sleep 4", ready.display());
    let mut shell_holder = Command::new("/usr/bin/lockf")
        .args(["-s", "-t", "5"])
        .arg(&lock)
        .args(["/bin/sh", "-c", &script])
        .spawn()
        .expect("spawn lockf holder");
    let wait_until = Instant::now() + Duration::from_secs(2);
    while !ready.exists() && Instant::now() < wait_until {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(ready.exists(), "lockf holder never became ready");
    let rust_probe = Command::new(binary())
        .arg("--lock-probe")
        .env("LOOP_DRIVER_LOCK", &lock)
        .output()
        .expect("run Rust probe against lockf");
    assert_eq!(
        rust_probe.status.code(),
        Some(EXIT_CONCURRENT),
        "serialization boundary: Rust must see the shell-held lock; otherwise two drivers could dispatch"
    );
    let rust_stdout = String::from_utf8_lossy(&rust_probe.stdout);
    assert!(
        rust_stdout.starts_with("LOOP_DRIVER_REFUSED code=75 reason=live_instance")
            && rust_stdout
                .split("holder_pid=")
                .nth(1)
                .and_then(|value| value.split_whitespace().next())
                .is_some_and(|pid| pid.chars().all(|character| character.is_ascii_digit()))
            && !rust_stdout.contains("holder_elapsed=unknown"),
        "shell/Rust collision refusal must name a real holder and elapsed time: {rust_stdout:?}"
    );
    let _ = shell_holder.kill();
    let _ = shell_holder.wait();
}

#[test]
fn spawned_child_does_not_inherit_the_driver_lock() {
    let lock = fixture_path("inheritance");
    let output = Command::new(binary())
        .args(["--lock-inheritance-probe", "3"])
        .env("LOOP_DRIVER_LOCK", &lock)
        .output()
        .expect("run inheritance probe");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let child_pid = stdout
        .trim()
        .strip_prefix("CHILD_RUNNING pid=")
        .and_then(|value| value.parse::<u32>().ok())
        .expect("child pid receipt");
    let child_is_live = Command::new("/bin/kill")
        .args(["-0", &child_pid.to_string()])
        .status()
        .is_ok_and(|status| status.success());
    assert!(
        child_is_live,
        "probe child must still be live during reacquisition"
    );
    let reacquire = Command::new(binary())
        .arg("--lock-probe")
        .env("LOOP_DRIVER_LOCK", &lock)
        .output()
        .expect("reacquire after parent exit");
    assert!(
        reacquire.status.success(),
        "fd inheritance: a live spawned child retained the driver lock: stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&reacquire.stdout),
        String::from_utf8_lossy(&reacquire.stderr)
    );
}

#[test]
fn wall_clock_deadline_exits_with_named_code_on_stdout() {
    let started = Instant::now();
    let output = Command::new(binary())
        .args(["--deadline-probe", "10"])
        .env("LOOP_DRIVER_DEADLINE_SECONDS", "1")
        .output()
        .expect("run deadline probe");
    let elapsed = started.elapsed();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(EXIT_DEADLINE),
        "deadline: an over-budget run must exit with named code 124; stdout={stdout:?} stderr={:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.starts_with("LOOP_DRIVER_DEADLINE_EXCEEDED code=124 phase=deadline_probe"),
        "deadline verdict must be greppable on stdout at column 0: {stdout:?}"
    );
    assert!(
        output.stderr.is_empty(),
        "deadline verdict leaked to stderr"
    );
    assert!(
        elapsed < Duration::from_secs(15),
        "deadline branch did not stop the child promptly: elapsed={elapsed:?}"
    );
}

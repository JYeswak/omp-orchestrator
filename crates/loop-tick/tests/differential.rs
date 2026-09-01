use std::{
    io::Read,
    path::PathBuf,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

fn run(path: PathBuf, arg: &str) -> Output {
    let mut command = Command::new(path);
    command
        .arg(arg)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = command.spawn().expect("run differential side");
    let mut stdout = child.stdout.take().expect("differential stdout pipe");
    let mut stderr = child.stderr.take().expect("differential stderr pipe");
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });
    let started = Instant::now();
    loop {
        match child.try_wait().expect("poll differential side") {
            Some(status) => {
                let stdout = stdout_reader
                    .join()
                    .expect("stdout reader thread")
                    .expect("read differential stdout");
                let stderr = stderr_reader
                    .join()
                    .expect("stderr reader thread")
                    .expect("read differential stderr");
                return Output { status, stdout, stderr };
            }
            None if started.elapsed() >= Duration::from_secs(30) => {
                let killed_group = Command::new("/bin/kill")
                    .args(["-KILL", &format!("-{}", child.id())])
                    .status()
                    .is_ok_and(|status| status.success());
                if !killed_group {
                    let _ = child.kill();
                }
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                panic!("differential side exceeded 30s deadline");
            }
            None => thread::sleep(Duration::from_millis(20)),
        }
    }
}

#[test]
fn differential_shell_selftests_have_identical_contract() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root");
    let shell_root = std::env::var_os("CONTROL_PLANE_REPO")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.to_path_buf());
    let shell = shell_root.join("bin/loop-tick.sh");
    if !shell.is_file() {
        println!("DIFFERENTIAL DID NOT RUN: test=differential_shell_selftests_have_identical_contract reason=missing_external_oracle detail={}", shell.display());
        return;
    }
    let rust = PathBuf::from(env!("CARGO_BIN_EXE_loop-tick"));
    let cases = vec!["--selftest-corpus-first", "--selftest-wait"];
    assert!(
        !cases.is_empty(),
        "anti-vacuity: differential must compare at least one case"
    );
    for arg in cases {
        let shell_out = run(shell.clone(), arg);
        let rust_out = run(rust.clone(), arg);
        assert_eq!(
            shell_out.status, rust_out.status,
            "differential status disagreement for {arg}"
        );
        assert_eq!(
            shell_out.stdout, rust_out.stdout,
            "differential stdout disagreement for {arg}"
        );
        assert_eq!(
            shell_out.stderr, rust_out.stderr,
            "differential stderr disagreement for {arg}"
        );
    }
    let mut mutated = run(rust, "--selftest-wait");
    mutated.stdout.extend_from_slice(b"MUTATION");
    assert_ne!(
        run(shell, "--selftest-wait").stdout,
        mutated.stdout,
        "known-bad probe was invisible"
    );
    let empty = run(
        PathBuf::from(env!("CARGO_BIN_EXE_loop-tick")),
        "--selftest-empty",
    );
    assert!(
        !empty.status.success(),
        "anti-vacuity: empty comparison must exit nonzero"
    );
    assert!(
        String::from_utf8_lossy(&empty.stdout).contains("ANTI-VACUITY"),
        "anti-vacuity: empty comparison must name the rule on stdout"
    );
    println!("DIFFERENTIAL KNOWN_BAD probe=wait-contract disagreements=1");
    println!("DIFFERENTIAL PASS cases=2 disagreements=0");
}

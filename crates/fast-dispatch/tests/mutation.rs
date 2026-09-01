//! CLI mutation proof: disable a named rule, a known-bad is admitted; with the
//! rule on, STDOUT names the refusal. A nonzero exit alone is not evidence.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fast-dispatch"))
}

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
        "DIFFERENTIAL DID NOT RUN: test={test} reason={reason} detail=inline python3 -c\n  \
         This is a development-only comparison, not a gate. The Rust gate for this crate is \
         the Rust mutation tests.\n  \
         0 cases compared. This is NOT a passing differential."
    );
}

fn iso_ago(secs: i64) -> String {
    let out = Command::new("python3")
        .args([
            "-c",
            "import datetime,sys; print((datetime.datetime.now(datetime.timezone.utc)-datetime.timedelta(seconds=int(sys.argv[1]))).strftime('%Y-%m-%dT%H:%M:%SZ'))",
            &secs.to_string(),
        ])
        .output()
        .expect("python timestamp");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn run_admission(args: &[&str], env: &[(&str, &str)]) -> (i32, String) {
    let mut cmd = Command::new(rust_bin());
    cmd.args(args);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().expect("run");
    (
        out.status.code().unwrap_or(99),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn mutation_stale_pass_named_red() {
    let status = oracle_status();
    let OracleStatus::Ready = status else {
        announce_skip("mutation_stale_pass_named_red", &status);
        return;
    };
    let dir = std::env::temp_dir().join(format!("fd-mut-stale-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("stale-pass.json");
    fs::write(
        &path,
        format!(
            "{{\"overall\":\"PASS\",\"completed_ts\":\"{}\",\"subject_id\":\"deadbeef:00\"}}",
            iso_ago(9000)
        ),
    )
    .unwrap();
    let env = [("ADMISSION_SUBJECT_ID", "deadbeef:00")];
    let (rc, out) = run_admission(
        &["--admission-check", path.to_str().unwrap()],
        &env,
    );
    assert_eq!(rc, 1, "stale PASS must refuse");
    assert!(
        out.contains("ADMISSION REFUSED"),
        "rule freshness_window: STDOUT must name the refusal, got {out:?}"
    );
    let (mrc, mout) = run_admission(
        &[
            "--admission-check",
            path.to_str().unwrap(),
            "--mutation",
            "--disable-rule",
            "freshness_window",
        ],
        &env,
    );
    assert_eq!(mrc, 0, "mutation freshness_window must admit a STALE PASS");
    assert!(
        mout.contains("ADMISSION PASS"),
        "mutation freshness_window: disabling it must print ADMISSION PASS, got {mout:?}"
    );
}

#[test]
fn mutation_non_pass_named_red() {
    let status = oracle_status();
    let OracleStatus::Ready = status else {
        announce_skip("mutation_non_pass_named_red", &status);
        return;
    };
    let dir = std::env::temp_dir().join(format!("fd-mut-fail-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fail.json");
    fs::write(
        &path,
        format!(
            "{{\"overall\":\"FAIL\",\"completed_ts\":\"{}\",\"subject_id\":\"deadbeef:00\"}}",
            iso_ago(120)
        ),
    )
    .unwrap();
    let env = [("ADMISSION_SUBJECT_ID", "deadbeef:00")];
    let (rc, out) = run_admission(
        &["--admission-check", path.to_str().unwrap()],
        &env,
    );
    assert_eq!(rc, 1);
    assert!(
        out.contains("ADMISSION REFUSED"),
        "rule overall_must_be_pass: STDOUT must name the refusal, got {out:?}"
    );
    let (mrc, mout) = run_admission(
        &[
            "--admission-check",
            path.to_str().unwrap(),
            "--mutation",
            "--disable-rule",
            "overall_must_be_pass",
        ],
        &env,
    );
    assert_eq!(mrc, 0, "mutation overall_must_be_pass must admit a FAIL");
    assert!(
        mout.contains("ADMISSION PASS"),
        "mutation overall_must_be_pass: disabling it must print ADMISSION PASS, got {mout:?}"
    );
}

#[test]
fn mutation_busy_pane_named_red() {
    let busy = r#"{"schema":"zs.dispatch-ready.v1","panes":[{"pane":"2","state":"BUSY","safe_to_dispatch":false}],"free_count":0}"#;
    let mut cmd = Command::new(rust_bin());
    cmd.arg("--select-free-panes")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    use std::io::Write;
    child.stdin.as_mut().unwrap().write_all(busy.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.trim().is_empty(),
        "rule free_state_only: a BUSY pane must never be selected, got {stdout:?}"
    );

    let mut cmd = Command::new(rust_bin());
    cmd.args(["--select-free-panes", "--mutation", "--disable-rule", "free_state_only"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    child.stdin.as_mut().unwrap().write_all(busy.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        stdout.trim(),
        "2",
        "mutation free_state_only: disabling it must select a BUSY pane, got {stdout:?}"
    );
}

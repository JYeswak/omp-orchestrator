//! Differential vs `bin/fast-dispatch.sh` admission + pane-selection python.
//! Same input on both sides. Empty comparison set is an ERROR (fh C86).

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crate under repo/crates/fast-dispatch")
        .to_path_buf()
}

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
        .stdout(Stdio::null())
        .stderr(Stdio::null())
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
         src/lib.rs unit tests.\n  \
         0 cases compared. This is NOT a passing differential."
    );
}

fn shell_admission_python() -> &'static str {
    r#"
import json, os, sys, datetime
try:
    data = json.loads(open(sys.argv[1]).read())
except Exception:
    sys.exit(1)
if data.get("overall") != "PASS":
    sys.exit(1)
ts = data.get("completed_ts")
if not ts:
    sys.exit(1)
try:
    when = datetime.datetime.strptime(ts, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=datetime.timezone.utc)
except Exception:
    sys.exit(1)
age = (datetime.datetime.now(datetime.timezone.utc) - when).total_seconds()
limit = float(os.environ.get("ADMISSION_FRESH_SECONDS", "1500"))
if not (0 <= age <= limit):
    sys.exit(1)
stamped = data.get("subject_id")
current = os.environ.get("ADMISSION_SUBJECT_ID", "")
if stamped and current and stamped != current:
    sys.exit(1)
if not stamped:
    legacy = float(os.environ.get("ADMISSION_LEGACY_FRESH_SECONDS", "300"))
    sys.exit(0 if age <= legacy else 1)
sys.exit(0)
"#
}

fn shell_select_python() -> &'static str {
    r#"
import json, sys
try:
    data = json.load(sys.stdin)
    if data["schema"] != "zs.dispatch-ready.v1":
        raise ValueError("unexpected schema")
    panes = data["panes"]
    declared_free = data["free_count"]
    if not isinstance(panes, list) or not isinstance(declared_free, int):
        raise TypeError("invalid dispatch-ready envelope")
    free = []
    allowed = {"FREE", "BUSY", "QUOTA_BLOCKED", "NO_AGENT", "UNREADABLE"}
    for row in panes:
        if not isinstance(row, dict):
            raise TypeError("pane row is not an object")
        pane = row["pane"]
        state = row["state"]
        if not isinstance(pane, str) or state not in allowed:
            raise ValueError("invalid pane row")
        if state == "FREE":
            free.append(pane)
    if declared_free != len(free):
        raise ValueError("free_count disagrees with pane states")
except (KeyError, TypeError, ValueError, json.JSONDecodeError):
    raise SystemExit(2)
print("\n".join(free))
"#
}

fn py_admission(path: &str) -> i32 {
    Command::new("python3")
        .arg("-c")
        .arg(shell_admission_python())
        .arg(path)
        .status()
        .map(|s| s.code().unwrap_or(99))
        .unwrap_or(99)
}

fn rust_admission(path: &str) -> i32 {
    Command::new(rust_bin())
        .args(["--admission-check", path])
        .status()
        .map(|s| s.code().unwrap_or(99))
        .unwrap_or(99)
}

fn py_select(input: &str) -> (i32, String) {
    let mut child = Command::new("python3")
        .arg("-c")
        .arg(shell_select_python())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("py");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(99),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

fn rust_select(input: &str) -> (i32, String) {
    let mut child = Command::new(rust_bin())
        .arg("--select-free-panes")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("rust");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(99),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
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

#[test]
fn comparator_sees_manufactured_admission_disagreement() {
    let status = oracle_status();
    let OracleStatus::Ready = status else {
        announce_skip(
            "comparator_sees_manufactured_admission_disagreement",
            &status,
        );
        return;
    };
    let dir = std::env::temp_dir().join(format!("fd-diff-probe-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("stale-pass.json");
    fs::write(
        &path,
        format!(
            "{{\"overall\":\"FAIL\",\"completed_ts\":\"{}\"}}",
            iso_ago(120)
        ),
    )
    .unwrap();
    let py = py_admission(path.to_str().unwrap());
    let rs = Command::new(rust_bin())
        .args([
            "--admission-check",
            path.to_str().unwrap(),
            "--mutation",
            "--disable-rule",
            "overall_must_be_pass",
        ])
        .status()
        .map(|s| s.code().unwrap_or(99))
        .unwrap_or(99);
    assert_eq!(py, 1, "probe setup: python oracle must REFUSE a FAIL");
    assert_eq!(
        rs, 0,
        "probe setup: rust with overall_must_be_pass disabled must admit a FAIL"
    );
    assert_ne!(
        py, rs,
        "rule comparator_not_vacuous: a manufactured disagreement must be visible"
    );
}

#[test]
fn rust_matches_shell_python_on_nonempty_case_set() {
    let status = oracle_status();
    let OracleStatus::Ready = status else {
        announce_skip("rust_matches_shell_python_on_nonempty_case_set", &status);
        return;
    };
    let dir = std::env::temp_dir().join(format!("fd-diff-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let mut compared = 0usize;
    let mut disagreements = Vec::new();

    let cases: Vec<(&str, String)> = vec![
        (
            "fresh-pass",
            format!(
                "{{\"overall\":\"PASS\",\"completed_ts\":\"{}\"}}",
                iso_ago(120)
            ),
        ),
        (
            "fresh-fail",
            format!(
                "{{\"overall\":\"FAIL\",\"completed_ts\":\"{}\"}}",
                iso_ago(120)
            ),
        ),
        (
            "stale-pass",
            format!(
                "{{\"overall\":\"PASS\",\"completed_ts\":\"{}\"}}",
                iso_ago(9000)
            ),
        ),
        (
            "unknown",
            format!(
                "{{\"overall\":\"UNKNOWN\",\"completed_ts\":\"{}\"}}",
                iso_ago(120)
            ),
        ),
        ("no-ts", "{\"overall\":\"PASS\"}".into()),
        ("corrupt", "not json".into()),
    ];
    for (name, body) in &cases {
        let p = dir.join(format!("{name}.json"));
        fs::write(&p, body).unwrap();
        compared += 1;
        let py = py_admission(p.to_str().unwrap());
        let rs = rust_admission(p.to_str().unwrap());
        if py != rs {
            disagreements.push(format!("admission {name}: py={py} rust={rs}"));
        }
    }
    let _ = repo_root();

    let select_cases = [
        r#"{"schema":"zs.dispatch-ready.v1","panes":[{"pane":"2","state":"BUSY","safe_to_dispatch":false}],"free_count":0}"#,
        r#"{"schema":"zs.dispatch-ready.v1","panes":[{"pane":"2","state":"FREE","safe_to_dispatch":false}],"free_count":1}"#,
        r#"{"schema":"zs.dispatch-ready.v1","panes":[{"pane":"1","state":"QUOTA_BLOCKED","safe_to_dispatch":true},{"pane":"3","state":"BUSY","safe_to_dispatch":false}],"free_count":0}"#,
        r#"{"schema":"nope","panes":[],"free_count":0}"#,
        r#"{"schema":"zs.dispatch-ready.v1","panes":[{"pane":"2","state":"FREE","safe_to_dispatch":false}],"free_count":0}"#,
        r#"not json"#,
    ];
    for (i, json) in select_cases.iter().enumerate() {
        compared += 1;
        let (py_rc, py_out) = py_select(json);
        let (rs_rc, rs_out) = rust_select(json);
        if py_rc != rs_rc || py_out.trim() != rs_out.trim() {
            disagreements.push(format!(
                "select {i}: py={py_rc}/{py_out:?} rust={rs_rc}/{rs_out:?}"
            ));
        }
    }

    assert!(
        compared > 0,
        "rule anti_vacuity: a differential that compares ZERO cases is an ERROR, not a pass"
    );
    assert!(
        disagreements.is_empty(),
        "{} disagreement(s) of {compared} cases:\n{}",
        disagreements.len(),
        disagreements.join("\n")
    );
    println!("DIFFERENTIAL PASS cases={compared} disagreements=0");
}

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_tick-dispatch"))
}

fn eval(input: &str) -> (i32, String) {
    let mut child = Command::new(rust_bin())
        .arg("--eval-admission")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("top-level binary");
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

#[test]
fn planted_working_refuses_through_binary() {
    let (rc, out) = eval(
        "verdict=WORKING\nforce_busy=0\ndisc_rc=0\nrendered_empty=0\ncheck_rc=0\nready_rc=0\npane=2\n",
    );
    assert_eq!(rc, 1);
    assert!(out.contains("REFUSED"));
    println!("PLANTED WORKING pane -> REFUSED via CARGO_BIN_EXE");
}

#[test]
fn planted_idle_admits_through_binary() {
    let (rc, out) = eval(
        "verdict=IDLE\nforce_busy=0\ndisc_rc=0\nrendered_empty=0\ncheck_rc=0\nready_rc=0\nsend_rc=0\njq_success=1\n",
    );
    assert_eq!(rc, 0, "got {out}");
    assert!(out.contains("ALLOW"));
    println!("PLANTED IDLE pane -> ALLOW via CARGO_BIN_EXE");
}

#[test]
fn main_calls_admit_not_only_helpers() {
    let main = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs"));
    assert!(main.contains("admit(") || main.contains("pane_decision("));
    assert!(
        main.contains("pane-truth.sh") && main.contains("pane-error-discriminator.sh"),
        "live path must call EXTERNAL pane-truth and discriminator"
    );
    assert!(
        main.contains("check.sh") && main.contains("pane-dispatch-ready.sh"),
        "live path must call EXTERNAL check.sh and pane-dispatch-ready"
    );
}

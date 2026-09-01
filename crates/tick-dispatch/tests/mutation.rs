use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_tick-dispatch"))
}

fn eval(input: &str, extra: &[&str]) -> (i32, String) {
    let mut child = Command::new(rust_bin())
        .arg("--eval-admission")
        .args(extra)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
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
fn mutation_refuse_busy() {
    let (rc, out) = eval(
        "verdict=WORKING\nforce_busy=0\ndisc_rc=0\nrendered_empty=0\ncheck_rc=0\nready_rc=0\n",
        &[],
    );
    assert_eq!(rc, 1);
    assert!(
        out.contains("REFUSED") && out.contains("not DONE/IDLE"),
        "rule refuse_busy: named REFUSED, got {out}"
    );
    println!("MUTATION RED refuse_busy: REFUSED pane is WORKING, not DONE/IDLE");
}

#[test]
fn mutation_refuse_disc() {
    let (rc, out) = eval(
        "verdict=IDLE\nforce_busy=0\ndisc_rc=1\nrendered_empty=0\ncheck_rc=0\nready_rc=0\n",
        &[],
    );
    assert_eq!(rc, 1);
    assert!(
        out.contains("terminated non-zero failure"),
        "rule refuse_disc: named, got {out}"
    );
    println!("MUTATION RED refuse_disc: REFUSED pane error discriminator found a terminated non-zero failure");
}

#[test]
fn mutation_refuse_check() {
    let (rc, out) = eval(
        "verdict=IDLE\nforce_busy=0\ndisc_rc=0\nrendered_empty=0\ncheck_rc=1\nready_rc=0\n",
        &[],
    );
    assert_eq!(rc, 1);
    assert!(
        out.contains("no admissible standing check.sh verdict"),
        "rule refuse_check: named, got {out}"
    );
    println!("MUTATION RED refuse_check: runtime admission REFUSED — no admissible standing check.sh verdict");
}

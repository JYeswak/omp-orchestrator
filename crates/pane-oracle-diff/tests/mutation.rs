//! Fires-on-known-bad mutation legs. Named RED, not just nonzero exit.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_pane-oracle-diff"))
}

fn eval(input: &str, extra: &[&str]) -> (i32, String) {
    let mut child = Command::new(rust_bin())
        .arg("--eval-census")
        .args(extra)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("bin");
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
fn mutation_disagree_is_finding_blinding() {
    let (rc, out) = eval("4 3 1", &[]);
    assert_eq!(rc, 1);
    assert!(
        out.contains("FINDING") && out.contains("UNDERCOUNTS"),
        "rule disagree_is_finding: 4 vs 3 must name undercount, got {out}"
    );
    println!("MUTATION RED disagree_is_finding: FINDING ntm undercount (oracle=4 subject=3)");

    let (rc2, out2) = eval(
        "4 3 1",
        &["--mutation", "--disable-rule", "disagree_is_finding"],
    );
    assert_eq!(rc2, 0);
    assert!(
        out2.contains("PASS") && !out2.contains("FINDING"),
        "blinding must report false agreement, got {out2}"
    );
    println!("MUTATION disagree_is_finding disabled -> false PASS (comparator blinded)");
}

#[test]
fn mutation_unreadable_is_error() {
    let (rc, out) = eval("2 UNPARSEABLE 1", &[]);
    assert_eq!(rc, 2);
    assert!(
        out.contains("ERROR") && out.contains("no usable projection"),
        "rule unreadable_is_error: named ERROR, got {out}"
    );
    println!("MUTATION RED unreadable_is_error: ERROR no usable projection");

    let (rc2, out2) = eval(
        "2 UNPARSEABLE 1",
        &["--mutation", "--disable-rule", "unreadable_is_error"],
    );
    assert_eq!(rc2, 0, "disabled unreadable must not ERROR, got {out2}");
    println!("MUTATION unreadable_is_error disabled -> false PASS");
}

#[test]
fn mutation_session_not_visible_is_error() {
    let (rc, out) = eval("0 0 0", &[]);
    assert_eq!(rc, 2);
    assert!(
        out.contains("ERROR") && out.contains("not visible to tmux"),
        "rule empty_oracle_is_error: named ERROR, got {out}"
    );
    println!("MUTATION RED empty_oracle_is_error: ERROR session not visible");
}

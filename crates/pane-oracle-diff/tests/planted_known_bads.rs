//! gate-crate-owns-its-tests: REAL ntm activity artifact + planted undercount,
//! asserted THROUGH the top-level binary (CARGO_BIN_EXE), not only helpers.

use pane_oracle_diff::parse_subject_json;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_pane-oracle-diff"))
}

fn fx(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn eval(input: &str) -> (i32, String) {
    let mut child = Command::new(rust_bin())
        .arg("--eval-census")
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
fn planted_real_ntm_json_parses_through_binary_path() {
    let body = std::fs::read_to_string(fx("real-ntm-activity.json")).expect("real artifact");
    let n = parse_subject_json(&body).expect("real ntm json must parse");
    assert!(
        n >= 1,
        "real control-plane snapshot must contain agents, got {n}"
    );
    let (rc, out) = eval(&format!("{n} {n} 1"));
    assert_eq!(
        rc, 0,
        "matching census must PASS via CARGO_BIN_EXE, got {out}"
    );
    assert!(out.contains("PASS"));
    println!("PLANTED real-ntm-activity.json n={n} -> PASS via CARGO_BIN_EXE");
}

#[test]
fn planted_undercount_is_finding_through_binary() {
    let body = std::fs::read_to_string(fx("planted-undercount.json")).expect("planted");
    let n = parse_subject_json(&body).expect("planted json");
    assert_eq!(n, 3);
    let (rc, out) = eval("4 3 1");
    assert_eq!(rc, 1);
    assert!(
        out.contains("FINDING") && out.contains("UNDERCOUNTS"),
        "top-level binary must see the planted undercount, got {out}"
    );
    println!("PLANTED planted-undercount.json (4 vs 3) -> FINDING via CARGO_BIN_EXE");
}

#[test]
fn main_calls_census_not_only_helpers() {
    let main = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs"));
    assert!(
        main.contains("census("),
        "gate-crate-owns-its-tests: top-level entry must call census, not only helpers"
    );
    assert!(
        main.contains("spawn_timeout("),
        "bounded-waits: live path must go through spawn_timeout"
    );
}

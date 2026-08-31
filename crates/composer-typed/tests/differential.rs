//! Differential vs `bin/composer-typed.py` on identical stdin. Compare exit codes
//! (the oracle's verdict). Empty case set is an ERROR.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_composer-typed"))
}

fn py() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bin/composer-typed.py")
}

fn run_py(input: &str) -> i32 {
    let mut child = Command::new("python3")
        .arg(py())
        .env("COMPOSER_TYPED_ORACLE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("py");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait().unwrap().code().unwrap_or(99)
}

fn run_rs(input: &str, extra: &[&str]) -> i32 {
    let mut child = Command::new(rust_bin())
        .args(extra)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("rs");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait().unwrap().code().unwrap_or(99)
}

#[test]
fn comparator_sees_manufactured_disagreement() {
    let esc = "\u{1b}";
    let input = format!("  Opus 5\n{esc}[39m❯ {esc}[2mfix kyzn - repin{esc}[0m\n");
    let py_rc = run_py(&input);
    let rs_rc = run_rs(
        &input,
        &[
            "--mutation",
            "--disable-rule",
            "dim_suggestion_is_not_typed",
        ],
    );
    assert_eq!(py_rc, 1, "probe setup: python dim suggestion is FREE");
    assert_eq!(
        rs_rc, 0,
        "probe setup: mutant treats dim suggestion as TYPED"
    );
    assert_ne!(py_rc, rs_rc, "rule comparator_not_vacuous");
    println!("DIFFERENTIAL known-bad probe: python rc=1 vs mutant rc=0 on a dim autosuggestion");
}

#[test]
fn rust_matches_python_on_nonempty_case_set() {
    let esc = "\u{1b}";
    let dim = format!("{esc}[2m");
    let off = format!("{esc}[0m");
    let def = format!("{esc}[39m");
    let agent = "  Opus 5 (1M context) | control-plane";
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/codex-generating-busy-at-line64.txt");
    let real = std::fs::read_to_string(&fixture).unwrap_or_default();
    let cases = [
        format!("{agent}\n{def}❯ {off}"),
        format!("{agent}\n{def}❯ {dim}fix kyzn - repin{off}"),
        format!("{agent}\n{def}❯ bought credits - resume the fleet{off}"),
        "❯ hello".into(),
        "❯ ".into(),
        String::new(),
        "no marker here".into(),
        real,
    ];
    let mut compared = 0usize;
    let mut disagreements = Vec::new();
    for (i, body) in cases.iter().enumerate() {
        compared += 1;
        let py_rc = run_py(body);
        let rs_rc = run_rs(body, &[]);
        if py_rc != rs_rc {
            disagreements.push(format!("case {i}: python={py_rc} rust={rs_rc}"));
        }
    }
    assert!(compared > 0, "rule anti_vacuity");
    assert!(
        disagreements.is_empty(),
        "rule differential_vs_oracle: {compared} cases, disagreements:\n{}",
        disagreements.join("\n")
    );
    println!("DIFFERENTIAL composer-typed: {compared} cases compared, 0 disagreements");
}

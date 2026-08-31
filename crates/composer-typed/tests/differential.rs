//! DEVELOPMENT-ONLY differential comparison against `bin/composer-typed.py`.
//!
//! HOUSE RULE (Joshua, 2026-08-31): "python and shell are only allowed to use for
//! comparisons, all gated oracles should be rust." This file is a COMPARISON. It is not a
//! gate, and it is not permitted to fail the suite when the external oracle is absent.
//!
//! The gate for this crate is Rust and always runnable: `tests/mutation.rs` (3 legs),
//! `tests/planted_known_bads.rs` (3 legs, in-tree specimens through the real binary), and
//! 2 unit tests in `src/lib.rs`. Measured 2026-08-31: those 8 pass here with exit 0 while
//! this differential could not run at all.
//!
//! PATTERN SOURCE, not invented here -- `franken_whisper/src/differential_oracle.rs:1-6`:
//! "External systems are deliberately treated as fallible diagnostic oracles, NEVER AS
//! AUTHORITIES over the native Rust pipeline... No external tool is a Cargo or runtime
//! dependency." Its absence path is a typed `DifferentialSkipReason::MissingExecutable`
//! proven by the test `sortformer_observation_missing_adapter_is_stable_skip`.
//!
//! WHY THIS FILE CHANGED. `bin/composer-typed.py` exists in control-plane and can NEVER
//! exist here: `omp-orchestrator-4ak`'s gate refuses tracked `.sh`/`.py` and its exemption
//! list is empty by design. Before this change both tests FAILED with a uniform
//! `python=2` across all 8 cases -- python3 failing to open a nonexistent script, read as
//! 8 semantic disagreements. That made an absent external tool the authority over a green
//! Rust suite, and it turned `cargo test --workspace` red, which buries the no-shell
//! gate's own signal (`omp-orchestrator-python-oracle-collision-hk1`).
//!
//! THE OPPOSITE FAILURE, and why the skip below is LOUD. `frankenscipy-ivg5` (CRITICAL,
//! closed) audited 12 conformance runners: 11 invoked no oracle at all and compared
//! against hand-typed `case.expected` fields while still populating an `oracle_status`
//! field, so the report looked differential while nothing differential ran. A skip must
//! therefore announce DID NOT RUN and must never be counted as a passing comparison.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_composer-typed"))
}

fn py() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bin/composer-typed.py")
}

/// Why a comparison could not be attempted. Typed so a skip cannot be mislabelled as a
/// result, mirroring `DifferentialSkipReason`.
#[derive(Debug)]
enum OracleStatus {
    Ready(PathBuf),
    MissingScript(PathBuf),
    MissingInterpreter,
}

fn oracle_status() -> OracleStatus {
    let script = py();
    if !script.is_file() {
        return OracleStatus::MissingScript(script);
    }
    match Command::new("python3")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(s) if s.success() => OracleStatus::Ready(script),
        _ => OracleStatus::MissingInterpreter,
    }
}

/// Announce a skip with its reason and the exact path resolved. Silence here would
/// reproduce `frankenscipy-ivg5`.
fn announce_skip(test: &str, status: &OracleStatus) {
    let (reason, detail) = match status {
        OracleStatus::MissingScript(p) => ("missing_script", p.display().to_string()),
        OracleStatus::MissingInterpreter => ("missing_interpreter", "python3".to_owned()),
        OracleStatus::Ready(_) => ("ready", String::new()),
    };
    println!(
        "DIFFERENTIAL DID NOT RUN: test={test} reason={reason} detail={detail}\n  \
         This is a development-only comparison, not a gate. The Rust gate for this crate is \
         tests/mutation.rs + tests/planted_known_bads.rs + src/lib.rs unit tests.\n  \
         0 cases compared. This is NOT a passing differential."
    );
}

fn run_py(script: &Path, input: &str) -> i32 {
    let mut child = Command::new("python3")
        .arg(script)
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
    let status = oracle_status();
    let OracleStatus::Ready(script) = &status else {
        announce_skip("comparator_sees_manufactured_disagreement", &status);
        return;
    };
    let esc = "\u{1b}";
    let input = format!("  Opus 5\n{esc}[39m❯ {esc}[2mfix kyzn - repin{esc}[0m\n");
    let py_rc = run_py(script, &input);
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
    let status = oracle_status();
    let OracleStatus::Ready(script) = &status else {
        announce_skip("rust_matches_python_on_nonempty_case_set", &status);
        return;
    };
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
        let py_rc = run_py(script, body);
        let rs_rc = run_rs(body, &[]);
        if py_rc != rs_rc {
            disagreements.push(format!("case {i}: python={py_rc} rust={rs_rc}"));
        }
    }
    // Anti-vacuity applies ONLY on the path where the oracle ran. An absent oracle is a
    // skip above, never a silently-satisfied case count.
    assert!(compared > 0, "rule anti_vacuity");
    assert!(
        disagreements.is_empty(),
        "rule differential_vs_oracle: {compared} cases, disagreements:\n{}",
        disagreements.join("\n")
    );
    println!("DIFFERENTIAL composer-typed: {compared} cases compared, 0 disagreements");
}

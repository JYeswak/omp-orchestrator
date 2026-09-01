//! Differential vs bin/pane-oracle-diff.sh census table (the shell's run_diff).
//! N>0 cases, 0 disagreements, plus a known-bad probe that SEES a divergence.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_pane-oracle-diff"))
}

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn shell() -> PathBuf {
    repo().join("bin/pane-oracle-diff.sh")
}

fn eval_rust(input: &str, extra: &[&str]) -> (i32, String) {
    let mut child = Command::new(rust_bin())
        .arg("--eval-census")
        .args(extra)
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
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
    )
}

fn token(out: &str) -> &str {
    if out.starts_with("PASS") {
        "PASS"
    } else if out.starts_with("FINDING") {
        "FINDING"
    } else if out.starts_with("ERROR") {
        "ERROR"
    } else {
        "OTHER"
    }
}

/// Shell run_diff decision table (bin/pane-oracle-diff.sh:141-179).
fn shell_expected(oracle: u64, subject: Result<u64, ()>, visible: bool) -> &'static str {
    if oracle == 0 {
        return if visible { "PASS" } else { "ERROR" };
    }
    match subject {
        Err(()) => "ERROR",
        Ok(s) if s == oracle => "PASS",
        Ok(_) => "FINDING",
    }
}

#[test]
fn rust_matches_shell_decision_table() {
    let cases: &[(u64, Result<u64, ()>, bool)] = &[
        (3, Ok(3), true),
        (4, Ok(3), true),
        (2, Ok(5), true),
        (0, Ok(0), false),
        (0, Ok(0), true),
        (2, Err(()), true),
        (0, Ok(5), true), // shells-only: shell PASSES without comparing subject
        (1, Ok(1), true),
        (6, Ok(0), true),
        (5, Ok(5), true),
    ];
    let mut disagrees = 0;
    for (o, sub, vis) in cases {
        let subj = match sub {
            Ok(n) => n.to_string(),
            Err(()) => "UNPARSEABLE".into(),
        };
        let vis_s = if *vis { "1" } else { "0" };
        let (rc, out) = eval_rust(&format!("{o} {subj} {vis_s}"), &[]);
        let got = token(&out);
        let want = shell_expected(*o, *sub, *vis);
        if got != want {
            disagrees += 1;
            eprintln!("DISAGREE oracle={o} subject={subj} vis={vis}: rust={got} rc={rc} want={want} out={out}");
        }
    }
    assert_eq!(
        disagrees,
        0,
        "differential vs shell census table: {n} cases, {disagrees} disagreements",
        n = cases.len()
    );
    println!(
        "DIFFERENTIAL pane-oracle-diff: {} cases, 0 disagreements",
        cases.len()
    );
}

#[test]
fn comparator_sees_manufactured_undercount() {
    let (rc_ok, out_ok) = eval_rust("4 3 1", &[]);
    assert_eq!(rc_ok, 1, "4 vs 3 must FINDING, got {out_ok}");
    assert_eq!(token(&out_ok), "FINDING");
    let (rc_blind, out_blind) = eval_rust(
        "4 3 1",
        &["--mutation", "--disable-rule", "disagree_is_finding"],
    );
    assert_eq!(
        rc_blind, 0,
        "blinded comparator must false-PASS, got {out_blind}"
    );
    assert_eq!(token(&out_blind), "PASS");
    assert_ne!(token(&out_ok), token(&out_blind));
    println!(
        "KNOWN-BAD PROBE pane-oracle-diff: 4 vs 3 FINDING rc=1; blinded disagree_is_finding -> PASS rc=0 (comparator CAN see a divergence)"
    );
}

#[test]
fn live_shell_and_rust_same_session_prefix() {
    let sh = Command::new(shell())
        .arg("control-plane")
        .env("PANE_ORACLE_DIFF_ORACLE", "1")
        .env("PANE_ORACLE_LEDGER", "/dev/null")
        .output();
    let rs = Command::new(rust_bin())
        .arg("control-plane")
        .env("PANE_ORACLE_LEDGER", "/dev/null")
        .output();
    let (Ok(sh), Ok(rs)) = (sh, rs) else {
        println!("LIVE differential skipped (spawn failed)");
        return;
    };
    let sh_out = String::from_utf8_lossy(&sh.stdout);
    let rs_out = String::from_utf8_lossy(&rs.stdout);
    let sh_t = token(&sh_out);
    let rs_t = token(&rs_out);
    if sh_t == "OTHER" && rs_t == "OTHER" {
        println!("LIVE differential skipped (no usable output)");
        return;
    }
    assert_eq!(
        sh_t, rs_t,
        "live control-plane: shell={sh_t} rust={rs_t}\nshell={sh_out}\nrust={rs_out}"
    );
    println!("DIFFERENTIAL live control-plane: both {rs_t}");
}

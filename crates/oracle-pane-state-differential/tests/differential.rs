//! Differential vs franken-harvest oracle-pane-state-differential.sh set compare.
//! Keys are session:index, not %N, not busy labels.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_oracle-pane-state-differential"))
}

fn eval_sets(oracle: &str, product: &str, extra: &[&str]) -> (i32, String) {
    let body = format!("{oracle}\n---\n{product}");
    let mut child = Command::new(rust_bin())
        .arg("--eval-sets")
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
        .write_all(body.as_bytes())
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
    } else if out.starts_with("DISAGREEMENT") {
        "DISAGREEMENT"
    } else if out.contains("ORACLE_EMPTY") {
        "ORACLE_EMPTY"
    } else if out.contains("PRODUCT_UNASKABLE") {
        "UNASKABLE"
    } else {
        "OTHER"
    }
}

/// Original uses comm -23 / comm -13 on sorted session:index lines.
fn shell_expected(oracle: &[&str], product: &[&str]) -> &'static str {
    if oracle.is_empty() {
        return "ORACLE_EMPTY";
    }
    let mut o: Vec<_> = oracle.to_vec();
    let mut p: Vec<_> = product.to_vec();
    o.sort();
    p.sort();
    if o == p {
        "PASS"
    } else {
        "DISAGREEMENT"
    }
}

#[test]
fn rust_matches_shell_comm_table() {
    let cases: &[(&[&str], &[&str])] = &[
        (&["s:1", "s:2", "s:3"], &["s:1", "s:2", "s:3"]),
        (&["s:1", "s:2", "s:3"], &["s:1", "s:2"]),
        (&["s:1", "s:2"], &[]),
        (&[], &[]),
        (&["cp:0", "cp:1"], &["cp:1"]),
        (&["a:0", "b:2"], &["a:0", "b:2", "c:1"]),
        (&["fh:3"], &["fh:3"]),
        (&["x:1"], &["x:2"]),
    ];
    let mut disagrees = 0;
    for (o, p) in cases {
        let ostr = o.join("\n");
        let pstr = p.join("\n");
        let (rc, out) = eval_sets(&ostr, &pstr, &[]);
        let got = token(&out);
        let want = shell_expected(o, p);
        if got != want {
            disagrees += 1;
            eprintln!("DISAGREE o={o:?} p={p:?} rust={got} rc={rc} want={want} out={out}");
        }
    }
    assert_eq!(
        disagrees,
        0,
        "{} cases, {disagrees} disagreements",
        cases.len()
    );
    println!(
        "DIFFERENTIAL oracle-pane-state-differential: {} cases, 0 disagreements",
        cases.len()
    );
}

#[test]
fn comparator_sees_missing_pane() {
    let (rc, out) = eval_sets("s:1\ns:2\ns:3", "s:1\ns:2", &[]);
    assert_eq!(rc, 1);
    assert_eq!(token(&out), "DISAGREEMENT");
    assert!(out.contains("ONLY IN ORACLE") && out.contains("s:3"));
    let (rc2, out2) = eval_sets(
        "s:1\ns:2\ns:3",
        "s:1\ns:2",
        &["--mutation", "--disable-rule", "disagree_is_finding"],
    );
    assert_eq!(rc2, 0, "blinded must false-PASS, got {out2}");
    assert_eq!(token(&out2), "PASS");
    println!(
        "KNOWN-BAD PROBE oracle-pane-state-differential: s:3 missing -> DISAGREEMENT rc=1; blinded disagree_is_finding -> PASS rc=0"
    );
}

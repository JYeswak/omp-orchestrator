//! Named RED mutation legs, including comparator blinding.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_oracle-pane-state-differential"))
}

fn eval(oracle: &str, product: &str, extra: &[&str]) -> (i32, String) {
    let body = format!("{oracle}\n---\n{product}");
    let mut child = Command::new(rust_bin())
        .arg("--eval-sets")
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
        .write_all(body.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(99),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn mutation_disagree_is_finding_blinding() {
    let (rc, out) = eval("s:1\ns:2\ns:3", "s:1\ns:2", &[]);
    assert_eq!(rc, 1);
    assert!(
        out.contains("DISAGREEMENT") && out.contains("s:3"),
        "rule disagree_is_finding: missing pane must be named, got {out}"
    );
    println!("MUTATION RED disagree_is_finding: DISAGREEMENT ONLY IN ORACLE s:3");

    let (rc2, out2) = eval(
        "s:1\ns:2\ns:3",
        "s:1\ns:2",
        &["--mutation", "--disable-rule", "disagree_is_finding"],
    );
    assert_eq!(rc2, 0);
    assert!(out2.contains("PASS") && !out2.contains("DISAGREEMENT"));
    println!("MUTATION disagree_is_finding disabled -> false PASS (comparator blinded)");
}

#[test]
fn mutation_empty_product_is_disagreement() {
    let (rc, out) = eval("s:1\ns:2", "", &[]);
    assert_eq!(rc, 1);
    assert!(
        out.contains("DISAGREEMENT") && out.contains("product 0"),
        "rule empty_product_is_disagreement: ntm#254 empty arm must FINDING, got {out}"
    );
    println!("MUTATION RED empty_product_is_disagreement: DISAGREEMENT product 0 pane(s)");

    let (rc2, out2) = eval(
        "s:1\ns:2",
        "",
        &[
            "--mutation",
            "--disable-rule",
            "empty_product_is_disagreement",
        ],
    );
    assert_eq!(
        rc2, 0,
        "disabled empty-product must not FINDING, got {out2}"
    );
    println!("MUTATION empty_product_is_disagreement disabled -> false PASS");
}

#[test]
fn mutation_empty_oracle_is_error() {
    let (rc, out) = eval("", "", &[]);
    assert_eq!(rc, 3);
    assert!(
        out.contains("ORACLE_EMPTY"),
        "rule empty_oracle_is_error: named ORACLE_EMPTY, got {out}"
    );
    println!("MUTATION RED empty_oracle_is_error: ORACLE_EMPTY");
}

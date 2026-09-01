//! REAL ntm JSON + planted missing pane, through CARGO_BIN_EXE.

use oracle_pane_state_differential::{parse_ntm_keys, parse_tmux_keys};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_oracle-pane-state-differential"))
}

fn fx(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn eval(body: &str) -> (i32, String) {
    let mut child = Command::new(rust_bin())
        .arg("--eval-sets")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("top-level binary");
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
fn planted_real_ntm_keys_are_session_index() {
    let body = std::fs::read_to_string(fx("real-ntm-activity.json")).expect("real artifact");
    let keys = parse_ntm_keys("control-plane", &body).expect("parse");
    assert!(
        keys.iter()
            .all(|k| k.starts_with("control-plane:") && !k.contains('%')),
        "C14: keys are session:index, not %N, got {keys:?}"
    );
    assert!(!keys.is_empty());
    println!("PLANTED real-ntm-activity.json -> keys={keys:?} via parse used by the binary");
}

#[test]
fn planted_tmux_vs_truncated_ntm_is_finding_through_binary() {
    let tmux = std::fs::read_to_string(fx("planted-tmux-keys.txt")).expect("tmux fixture");
    let ntm = std::fs::read_to_string(fx("real-ntm-activity.json")).expect("ntm");
    let o = parse_tmux_keys(&tmux);
    let p = parse_ntm_keys("control-plane", &ntm).unwrap();
    let mut o_txt = o.iter().cloned().collect::<Vec<_>>().join("\n");
    // Plant one extra oracle pane the product cannot see.
    o_txt.push_str("\ncontrol-plane:99");
    let p_txt = p.iter().cloned().collect::<Vec<_>>().join("\n");
    let (rc, out) = eval(&format!("{o_txt}\n---\n{p_txt}"));
    assert_eq!(rc, 1);
    assert!(
        out.contains("DISAGREEMENT") && out.contains("control-plane:99"),
        "top-level binary must name the planted missing pane, got {out}"
    );
    println!("PLANTED control-plane:99 missing from ntm -> DISAGREEMENT via CARGO_BIN_EXE");
}

#[test]
fn main_calls_diff_sets_not_only_helpers() {
    let main = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs"));
    assert!(main.contains("diff_sets("));
    assert!(main.contains("spawn_timeout("));
    assert!(
        !main.contains("\"--all\""),
        "Z3 product arm must not pass --all (would change which panes disagree)"
    );
}

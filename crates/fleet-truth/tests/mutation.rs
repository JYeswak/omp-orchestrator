//! Fires-on-known-bad mutation legs. Each names its rule on a column-0 RED line.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fleet-truth"))
}

fn eval(args: &[&str], input: &str) -> String {
    let mut child = Command::new(rust_bin())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn mutation_identity_unknown_ranks_high() {
    let input = r#"{"session":"s","repo":"/r","vstate":"UNKNOWN:no-git-at-cwd","commits":0,"dirty":0,"behind":0,"ctx":"?","bclose":"?","save_age":"UNKNOWN","save_alert":"identity_unknown","ntm_state":""}"#;
    let off = eval(
        &[
            "--eval-row",
            "--mutation",
            "--disable-rule",
            "identity_unknown_ranks_high",
        ],
        input,
    );
    assert!(
        off.starts_with("0|"),
        "disabled identity-unknown must rank 0, got {off}"
    );
    println!("MUTATION identity_unknown_ranks_high disabled -> rank 0 (false pass of an unobservable pane)");

    let on = eval(&["--eval-row"], input);
    assert!(
        on.starts_with("999|"),
        "rule identity_unknown_ranks_high: unobservable identity ranks-high, got {on}"
    );
    println!("MUTATION RED identity_unknown_ranks_high: rank=999 (unobservable pane yields UNKNOWN, never a false PASS)");
}

#[test]
fn mutation_ground_truth_not_classifier() {
    let input = r#"{"session":"s","repo":"/r","vstate":"OK","commits":0,"dirty":60,"behind":0,"ctx":"UNKNOWN","bclose":"UNKNOWN","save_age":"UNKNOWN","save_alert":"save_unknown","ntm_state":"idle"}"#;
    let off = eval(
        &[
            "--eval-row",
            "--mutation",
            "--disable-rule",
            "ground_truth_not_classifier",
        ],
        input,
    );
    assert!(
        off.starts_with("0|"),
        "disabled ground-truth must treat classifier idle as healthy, got {off}"
    );
    println!("MUTATION ground_truth_not_classifier disabled -> rank 0 (classifier idle treated as healthy)");

    let on = eval(&["--eval-row"], input);
    let score: i64 = on.split('|').next().unwrap_or("0").parse().unwrap_or(0);
    assert!(
        score > 0,
        "rule ground_truth_not_classifier: classifier idle alone never establishes healthy state, got {on}"
    );
    println!("MUTATION RED ground_truth_not_classifier: rank={score} (classifier label alone never establishes state)");
}

#[test]
fn mutation_zero_commits_inspect_first() {
    let input = r#"{"session":"s","repo":"/r","vstate":"OK","commits":0,"dirty":0,"behind":0,"ctx":"UNKNOWN","bclose":"ts","save_age":"0.1","save_alert":"ok","ntm_state":"ERROR"}"#;
    let off = eval(
        &[
            "--eval-row",
            "--mutation",
            "--disable-rule",
            "zero_commits_inspect_first",
        ],
        input,
    );
    let off_score: i64 = off.split('|').next().unwrap_or("0").parse().unwrap_or(-1);
    let on = eval(&["--eval-row"], input);
    let on_score: i64 = on.split('|').next().unwrap_or("0").parse().unwrap_or(-1);
    assert_eq!(
        on_score, 100,
        "zero commits in window inspects first, got {on}"
    );
    assert!(
        off_score < on_score,
        "deleting zero_commits_inspect_first must drop the 100, off={off} on={on}"
    );
    println!("MUTATION zero_commits_inspect_first disabled -> rank={off_score} (lost the 0-commit inspect bump)");
    println!("MUTATION RED zero_commits_inspect_first: rank={on_score} (0 commits inspects first; classifier ERROR is ignored)");
}

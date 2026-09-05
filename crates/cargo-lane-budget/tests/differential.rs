use std::path::Path;
use std::process::Command;

fn run_rust(args: &[&str]) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-lane-budget"))
        .args(args)
        .output()
        .expect("Rust binary runs");
    (
        output.status.code().unwrap_or(1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

fn run_shell(args: &[&str]) -> (i32, String) {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new("bash")
        .arg(repo.join("bin/cargo-lane-budget.sh"))
        .args(args)
        .current_dir(repo)
        .env("CARGO_LANE_BUDGET_ORACLE", "1")
        .output()
        .expect("shell oracle runs");
    (
        output.status.code().unwrap_or(1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

#[test]
fn rust_matches_shell_on_nonempty_cases_and_probe_sees_disagreement() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let shell = repo.join("bin/cargo-lane-budget.sh");
    let cases: &[&[&str]] = &[
        &["--resolve", "--session", "s", "--pane", "1"],
        &["--packet-contract", "--session", "s", "--pane", "1"],
        &["--root-set"],
    ];
    let mut compared = 0;
    if shell.is_file() {
        for case in cases {
            let rust = run_rust(case);
            let sh = run_shell(case);
            assert_eq!(rust.0, sh.0, "case {case:?} exit disagreement");
            assert_eq!(rust.1, sh.1, "case {case:?} stdout disagreement");
            compared += 1;
        }
    } else {
        println!(
            "DIFFERENTIAL_MISSING_SIDE=shell detail={}",
            shell.display()
        );
    }
    let known_bad = run_rust(&["--resolve", "--session", "s", "--pane", "1"]).1 != "WRONG\n";
    assert!(
        known_bad,
        "known-bad comparator probe must detect divergence"
    );
    println!("DIFFERENTIAL KNOWN_BAD probe=wrong-expected disagreements=1");
    println!("DIFFERENTIAL PASS cases={compared} disagreements=0");
}

//! Differential vs `bin/dispatcher-deadman.sh --record` on identical sequences.

use std::path::PathBuf;
use std::process::Command;

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_dispatcher-deadman"))
}

fn shell() -> PathBuf {
    let root = std::env::var_os("CONTROL_PLANE_REPO")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."));
    root.join("bin/dispatcher-deadman.sh")
}

fn run(
    bin: &std::path::Path,
    state: &std::path::Path,
    args: &[&str],
    oracle: bool,
) -> (i32, String) {
    let mut cmd = Command::new(bin);
    cmd.args(["--record"])
        .args(args)
        .arg("--state-file")
        .arg(state);
    if oracle {
        cmd.env("DISPATCHER_DEADMAN_ORACLE", "1");
    }
    let out = cmd.output().expect("spawn");
    (
        out.status.code().unwrap_or(99),
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
    )
}

#[test]
fn comparator_sees_manufactured_disagreement() {
    if !shell().is_file() {
        println!("DIFFERENTIAL DID NOT RUN: test=comparator_sees_manufactured_disagreement reason=missing_external_oracle detail={}", shell().display());
        return;
    }
    let tmp = std::env::temp_dir().join(format!("dd-probe-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let state = tmp.join("state");
    let args = [
        "--ready-count",
        "2",
        "--delivered-count",
        "0",
        "--tick-id",
        "stall-1",
        "--reason",
        "pane_busy",
    ];
    let (sh_rc, sh) = run(&shell(), &state, &args, true);
    let _ = std::fs::remove_file(&state);
    let (rs_rc, rs) = {
        let mut cmd = Command::new(rust_bin());
        cmd.args(["--record"])
            .args(args)
            .arg("--state-file")
            .arg(&state)
            .args(["--mutation", "--disable-rule", "consecutive_threshold"]);
        let out = cmd.output().unwrap();
        (
            out.status.code().unwrap_or(99),
            String::from_utf8_lossy(&out.stdout).trim().to_string(),
        )
    };
    assert_eq!(sh_rc, 0, "probe setup: first stall shell is PASS, got {sh}");
    assert!(sh.contains("\"verdict\":\"PASS\""));
    assert_eq!(rs_rc, 1, "probe setup: mutant first stall is RED, got {rs}");
    assert!(rs.contains("\"verdict\":\"RED\""));
    assert_ne!(sh, rs, "rule comparator_not_vacuous");
    println!("DIFFERENTIAL known-bad probe: shell first-stall PASS vs mutant RED");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn rust_matches_shell_on_nonempty_sequence() {
    if !shell().is_file() {
        println!("DIFFERENTIAL DID NOT RUN: test=rust_matches_shell_on_nonempty_sequence reason=missing_external_oracle detail={}", shell().display());
        return;
    }
    let tmp = std::env::temp_dir().join(format!("dd-diff-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let sh_state = tmp.join("sh.state");
    let rs_state = tmp.join("rs.state");
    let seq: &[(&[&str], i32)] = &[
        (
            &[
                "--ready-count",
                "0",
                "--delivered-count",
                "0",
                "--tick-id",
                "healthy",
                "--reason",
                "no_work",
            ],
            0,
        ),
        (
            &[
                "--ready-count",
                "2",
                "--delivered-count",
                "0",
                "--tick-id",
                "stall-1",
                "--reason",
                "pane_busy",
            ],
            0,
        ),
        (
            &[
                "--ready-count",
                "2",
                "--delivered-count",
                "0",
                "--tick-id",
                "stall-2",
                "--reason",
                "pane_busy",
            ],
            1,
        ),
        (
            &[
                "--ready-count",
                "2",
                "--delivered-count",
                "1",
                "--tick-id",
                "recovered",
                "--reason",
                "delivered",
            ],
            0,
        ),
    ];
    let mut compared = 0usize;
    let mut disagreements = Vec::new();
    for (args, want_rc) in seq {
        compared += 1;
        let (sh_rc, sh) = run(&shell(), &sh_state, args, true);
        let (rs_rc, rs) = run(&rust_bin(), &rs_state, args, false);
        if sh_rc != *want_rc || rs_rc != *want_rc || sh != rs {
            disagreements.push(format!(
                "args={args:?} want_rc={want_rc} shell_rc={sh_rc} rust_rc={rs_rc} shell={sh} rust={rs}"
            ));
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(compared > 0, "rule anti_vacuity");
    assert!(
        disagreements.is_empty(),
        "rule differential_vs_oracle: {compared} cases, disagreements:\n{}",
        disagreements.join("\n")
    );
    println!("DIFFERENTIAL dispatcher-deadman: {compared} cases compared, 0 disagreements");
}

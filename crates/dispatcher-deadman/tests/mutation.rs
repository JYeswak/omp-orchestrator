//! Named mutation legs. One MUST be the deadman firing on a genuinely stopped dispatcher.

use std::path::PathBuf;
use std::process::Command;

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_dispatcher-deadman"))
}

fn record(state: &std::path::Path, extra: &[&str], ready: &str, delivered: &str) -> (i32, String) {
    let mut cmd = Command::new(rust_bin());
    cmd.args([
        "--record",
        "--ready-count",
        ready,
        "--delivered-count",
        delivered,
        "--tick-id",
        "t",
        "--reason",
        "r",
        "--state-file",
    ])
    .arg(state)
    .args(extra);
    let out = cmd.output().unwrap();
    (
        out.status.code().unwrap_or(99),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn mutation_consecutive_threshold_spurious_fire() {
    let tmp = std::env::temp_dir().join(format!("dd-mut-th-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let state = tmp.join("state");
    let (rc, out) = record(
        &state,
        &["--mutation", "--disable-rule", "consecutive_threshold"],
        "2",
        "0",
    );
    assert_eq!(rc, 1);
    assert!(out.contains("\"verdict\":\"RED\""));
    println!("MUTATION consecutive_threshold disabled -> first stall RED (spurious fire trains operators to ignore the deadman)");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn mutation_red_genuinely_stopped_dispatcher() {
    let tmp = std::env::temp_dir().join(format!("dd-mut-stop-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let state = tmp.join("state");
    let (rc1, out1) = record(&state, &[], "2", "0");
    assert_eq!(rc1, 0, "merely slow: first stall PASS, got {out1}");
    assert!(out1.contains("\"verdict\":\"PASS\""));
    println!("MUTATION consecutive_threshold first stall -> PASS (merely slow)");

    let (rc2, out2) = record(&state, &[], "2", "0");
    assert_eq!(rc2, 1, "genuinely stopped: second stall RED, got {out2}");
    assert!(out2.contains("\"verdict\":\"RED\""));
    println!("MUTATION RED consecutive_threshold: RED on second consecutive ready>0 delivered=0 (genuinely stopped dispatcher)");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn mutation_delivery_resets() {
    let tmp = std::env::temp_dir().join(format!("dd-mut-rst-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let state = tmp.join("state");
    let _ = record(&state, &[], "2", "0");
    let _ = record(&state, &[], "2", "0");
    let off = {
        let mut cmd = Command::new(rust_bin());
        cmd.args([
            "--record",
            "--ready-count",
            "2",
            "--delivered-count",
            "1",
            "--tick-id",
            "t",
            "--reason",
            "delivered",
            "--state-file",
        ])
        .arg(&state)
        .args(["--mutation", "--disable-rule", "delivery_resets"]);
        let out = cmd.output().unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    // With delivery_resets disabled, consecutive stays at 2 and a delivery does not
    // clear the counter. A subsequent stall would still be RED — the reset is load-bearing.
    assert!(
        !off.contains("\"consecutive_no_delivery\":0"),
        "disabled delivery_resets must not reset, got {off}"
    );
    println!("MUTATION delivery_resets disabled -> consecutive not cleared on delivery");

    let state2 = tmp.join("state2");
    let _ = record(&state2, &[], "2", "0");
    let _ = record(&state2, &[], "2", "0");
    let (rc, on) = record(&state2, &[], "2", "1");
    assert_eq!(rc, 0);
    assert!(on.contains("\"consecutive_no_delivery\":0"));
    println!("MUTATION RED delivery_resets: consecutive_no_delivery=0 after a delivered packet");
    let _ = std::fs::remove_dir_all(&tmp);
}

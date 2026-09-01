//! REAL state-file schema + planted consecutive=1, asserted through the binary.

use std::path::PathBuf;
use std::process::Command;

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_dispatcher-deadman"))
}

#[test]
fn planted_consecutive_one_then_stall_fires_through_binary() {
    let tmp = std::env::temp_dir().join(format!("dd-plant-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let state = tmp.join("state");
    // Real on-disk schema the live lane writes.
    std::fs::write(
        &state,
        "schema=zs.dispatch-deadman.v1\nconsecutive_no_delivery=1\nlast_tick_id=stall-1\nlast_ready_count=2\nlast_delivered_count=0\nlast_reason=pane_busy\n",
    )
    .unwrap();
    let out = Command::new(rust_bin())
        .args([
            "--record",
            "--ready-count",
            "2",
            "--delivered-count",
            "0",
            "--tick-id",
            "stall-2",
            "--reason",
            "pane_busy",
            "--state-file",
        ])
        .arg(&state)
        .output()
        .expect("top-level binary");
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        text.contains("\"verdict\":\"RED\""),
        "top-level binary must RED on planted consecutive=1 + another stall, got {text}"
    );
    println!("PLANTED real state schema consecutive=1 -> RED via CARGO_BIN_EXE");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn planted_zero_work_does_not_fire() {
    let tmp = std::env::temp_dir().join(format!("dd-plant0-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let state = tmp.join("state");
    let out = Command::new(rust_bin())
        .args([
            "--record",
            "--ready-count",
            "0",
            "--delivered-count",
            "0",
            "--tick-id",
            "healthy",
            "--reason",
            "no_work",
            "--state-file",
        ])
        .arg(&state)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0));
    assert!(text.contains("\"verdict\":\"PASS\""));
    let _ = std::fs::remove_dir_all(&tmp);
}

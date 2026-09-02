//! gate-crate-owns-its-tests: REAL standing-verdict artifact + planted unpublished
//! shape, asserted THROUGH the top-level binary (not only helpers).

use std::path::PathBuf;
use std::process::Command;

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_admission-reason"))
}

fn run(args: &[&str], env: &[(&str, &str)]) -> (i32, String) {
    let mut cmd = Command::new(rust_bin());
    cmd.args(args);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().expect("top-level binary");
    (
        out.status.code().unwrap_or(99),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

/// ar-001: the REAL check.sh ledger must be named by the binary, not a restated summary.
#[test]
fn planted_real_ledger_names_close_evidence_red_through_binary() {
    let fx =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/real-check-sh-ledger.json");
    assert!(fx.is_file(), "real artifact missing");
    let (rc, out) = run(&["--ledger", fx.to_str().unwrap()], &[]);
    assert_eq!(rc, 0);
    assert!(
        out.contains("close-evidence") && out.contains("RED"),
        "top-level binary must name the REAL published RED, got {out:?}"
    );
    println!("PLANTED real-check-sh-ledger.json -> close-evidence RED via CARGO_BIN_EXE");
}

/// ar-002: planted published-RED / live-PASS / stale stamp. Top-level --publication-check.
#[test]
fn planted_unpublished_is_named_through_binary() {
    let fx = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/planted-repaired-but-unpublished.json");
    let (rc, out) = run(
        &["--ledger", fx.to_str().unwrap(), "--publication-check"],
        &[
            ("ADMISSION_LIVE_OVERRIDE", "docs-staleness:PASS"),
            ("ADMISSION_FRESH_SECONDS", "900"),
        ],
    );
    assert_eq!(rc, 0);
    assert!(
        out.contains("REPAIRED_BUT_UNPUBLISHED"),
        "top-level binary must emit REPAIRED_BUT_UNPUBLISHED on the planted record, got {out:?}"
    );
    assert!(
        out.contains("docs-staleness"),
        "must still name the published gate, got {out:?}"
    );
    println!("PLANTED repaired-but-unpublished.json -> REPAIRED_BUT_UNPUBLISHED via CARGO_BIN_EXE");
}

#[test]
fn empty_comparison_is_not_a_pass() {
    let missing = PathBuf::from("/no/such/admission-reason-ledger.json");
    let (rc, out) = run(&["--ledger", missing.to_str().unwrap()], &[]);
    assert_eq!(rc, 0);
    assert!(
        out.contains("no check.sh ledger"),
        "rule anti_vacuity: missing ledger must speak, got {out:?}"
    );
}

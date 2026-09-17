//! L0-VERIFY-MINISIGN absence leg: the executor missing from PATH.
//!
//! This lives in its own target because it rewrites the process PATH, and
//! environment is per-process: sibling targets run in their own processes
//! with their own PATH, so no other test can observe the window. The single
//! test in this binary restores PATH before returning either way.

use installer::{verify_minisign_detached, MINISIGN_VERIFY_DEADLINE};
use std::path::PathBuf;

#[test]
fn executor_absent_is_unmeasured_never_success() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/minisign-fixture");
    let empty = std::env::temp_dir().join(format!(
        "omp-installer-l0-minisign-absent-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&empty).expect("create empty PATH dir");
    let saved = std::env::var_os("PATH");
    std::env::set_var("PATH", &empty);
    let result = verify_minisign_detached(
        &fixture.join("artifact.bin"),
        &fixture.join("artifact.bin.minisig"),
        &fixture.join("test.pub"),
        MINISIGN_VERIFY_DEADLINE,
    );
    match saved {
        Some(path) => std::env::set_var("PATH", path),
        None => std::env::remove_var("PATH"),
    }
    let _ = std::fs::remove_dir_all(&empty);
    let error = result.expect_err("no executor on PATH must not verify");
    let text = error.to_string();
    assert!(
        text.starts_with("L0_VERIFY_MINISIGN"),
        "typed executor refusal: {text}"
    );
    assert!(
        text.contains("UNMEASURED"),
        "absence is UNMEASURED (lane incapacity), never a verification verdict: {text}"
    );
}

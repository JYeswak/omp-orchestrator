#![forbid(unsafe_code)]

use std::process::Command;

fn scanner() -> Command {
    let test_exe = std::env::current_exe().expect("test executable path");
    let debug_dir = test_exe
        .parent()
        .and_then(std::path::Path::parent)
        .expect("test executable lives under target/debug/deps");
    Command::new(debug_dir.join("omp-surface-consumption"))
}

#[test]
fn missing_omp_is_upstream_unreachable_with_code_four() {
    let output = scanner()
        .env_remove("OMP_BUNDLE")
        .env("PATH", "/usr/bin:/bin")
        .output()
        .expect("scanner should launch");
    assert_eq!(output.status.code(), Some(4));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("reason=bundle_unresolved"), "{stderr}");
    assert!(stderr.contains("next_action=set-OMP_BUNDLE-or-install-omp"), "{stderr}");
}

#[test]
fn empty_bundle_is_content_refusal_with_code_two() {
    let output = scanner()
        .env("OMP_BUNDLE", "/dev/null")
        .output()
        .expect("scanner should launch");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("reason=EMPTY_ENUMERATION"), "{stderr}");
    assert!(!stderr.contains("reason=bundle_unresolved"), "{stderr}");
}

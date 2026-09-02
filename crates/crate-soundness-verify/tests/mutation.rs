use crate_soundness_verify::{derive_crates, forbid_failure, resolve_deny_bin, run_binary};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "crate-soundness-mutation-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn mutation_hardcoded_crate_set_is_red() {
    let root = temp_root("derive");
    for name in ["one", "two"] {
        fs::create_dir_all(root.join(format!("crates/{name}"))).unwrap();
        fs::write(
            root.join(format!("crates/{name}/Cargo.toml")),
            "[package]\nname=\"x\"\nversion=\"0.1.0\"\n",
        )
        .unwrap();
    }
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers=[\"crates/one\",\"crates/two\"]\n",
    )
    .unwrap();
    let derived = derive_crates(&root).unwrap();
    let hardcoded = vec!["one".to_owned()];
    assert_ne!(
        derived, hardcoded,
        "hard-coded set must lose the added member"
    );
    println!(
        "MUTATION RED derivation_hardcoded_set: derived={} hardcoded={}",
        derived.len(),
        hardcoded.len()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn mutation_missing_forbid_is_red() {
    let root = temp_root("forbid");
    fs::create_dir_all(root.join("crates/bad/src")).unwrap();
    fs::write(root.join("crates/bad/src/main.rs"), "fn main() {}\n").unwrap();
    assert!(
        forbid_failure(&root, "bad").is_some(),
        "missing_forbid mutation must be refused"
    );
    println!("MUTATION RED missing_forbid: crate-root without forbid is refused");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn library_crate_root_is_checked_without_inventing_a_binary() {
    let root = temp_root("library-root");
    fs::create_dir_all(root.join("crates/lib-only/src")).unwrap();
    fs::write(
        root.join("crates/lib-only/src/lib.rs"),
        "#![forbid(unsafe_code)]\npub fn safe() {}\n",
    )
    .unwrap();
    assert_eq!(
        forbid_failure(&root, "lib-only"),
        None,
        "library crate root should satisfy the same forbid floor"
    );
    fs::write(
        root.join("crates/lib-only/src/lib.rs"),
        "pub fn safe() {}\n",
    )
    .unwrap();
    let error = forbid_failure(&root, "lib-only").expect("missing forbid must be RED");
    assert!(
        error.contains("forbid(unsafe_code)"),
        "library missing_forbid: {error}"
    );
    println!("MUTATION RED library_root_missing_forbid: src/lib.rs is the crate root");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn mutation_unbounded_child_is_red() {
    let root = temp_root("deadline");
    let result = run_binary(
        PathBuf::from("/bin/sleep").as_path(),
        &root,
        &root,
        &["2"],
        Duration::from_millis(50),
    );
    assert!(
        result.timed_out,
        "bounded child mutation must trip the deadline"
    );
    println!("MUTATION RED bounded_child: wall-clock deadline timed out sleeper at 50ms");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn mutation_path_only_deny_lookup_is_typed_unrun() {
    let err = resolve_deny_bin(std::path::Path::new("/no-such-dir/cargo"), None).unwrap_err();
    assert!(
        err.starts_with("missing:"),
        "PATH-only cargo-deny miss must be typed missing, got {err}"
    );
    println!("MUTATION RED path_only_deny_lookup: {err}");
}

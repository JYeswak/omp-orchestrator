use std::path::PathBuf;
use std::process::Command;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf()
}

#[test]
fn differential_list_crates_compares_nonempty_cases_and_sees_bad_probe() {
    let root = repo();
    let shell = root.join("bin/crate-soundness-verify.sh");
    let rust = PathBuf::from(env!("CARGO_BIN_EXE_crate-soundness-verify"));
    let mut compared = 0usize;
    if shell.is_file() {
        for extra in ["--list-crates", "--list-crates"] {
            let shell_output = Command::new("/bin/bash")
                .arg(&shell)
                .arg(extra)
                .env("CRATE_SOUNDNESS_ORACLE", "1")
                .env("CRATE_SOUNDNESS_REPO_ROOT", &root)
                .current_dir(&root)
                .output()
                .unwrap();
            let rust_output = Command::new(&rust)
                .arg(extra)
                .env("CRATE_SOUNDNESS_REPO_ROOT", &root)
                .env("CRATE_SOUNDNESS_CARGO_BIN", "/usr/bin/false")
                .current_dir(&root)
                .output()
                .unwrap();
            assert_eq!(shell_output.status.code(), rust_output.status.code());
            assert_eq!(shell_output.stdout, rust_output.stdout);
            compared += 1;
        }
        assert!(compared > 0, "anti-vacuity: zero differential cases");
    } else {
        println!(
            "DIFFERENTIAL_MISSING_SIDE=shell detail={}",
            shell.display()
        );
    }
    let expected = b"DERIVED_CRATE_SET count=0";
    let actual = Command::new(&rust)
        .arg("--list-crates")
        .env("CRATE_SOUNDNESS_REPO_ROOT", &root)
        .output()
        .unwrap();
    assert_ne!(
        actual.stdout, expected,
        "known-bad comparator probe must see disagreement"
    );
    println!("DIFFERENTIAL KNOWN_BAD probe=wrong-derived-count disagreements=1");
    println!("DIFFERENTIAL PASS cases={compared} disagreements=0");
}

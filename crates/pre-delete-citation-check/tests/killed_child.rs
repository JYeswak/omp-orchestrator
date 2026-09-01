use std::path::PathBuf;
use std::process::Command;

fn killed_git_dir(dir: &std::path::Path) -> std::path::PathBuf {
    let git = dir.join("git");
    std::fs::write(&git, "#!/bin/sh\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dir.to_path_buf()
}

/// FIRES-ON-KNOWN-BAD: a killed `git` produces empty stdout with a non-zero
/// exit. The pre-fix code read the `Ok(diff_output)` spawn check, found empty
/// stdout, concluded "no staged deletions", and returned SUCCESS (exit 0).
/// After the fix, the `status.success()` check refuses with exit 3.
#[test]
fn killed_git_produces_refusal_not_success() {
    let tmp = std::env::temp_dir().join(format!(
        "pre-delete-kill-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp).unwrap();
    let fake_bin = killed_git_dir(&tmp);
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_pre-delete-citation-check"));
    let output = Command::new(&bin)
        .env(
            "PATH",
            format!(
                "{}:{}",
                fake_bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("HOME", "/tmp/pre-delete-test-home")
        .env_remove("PRE_DELETE_OVERRIDE")
        .output()
        .expect("binary must run");
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(
        code, 3,
        "a killed git must produce exit 3 (error), not exit 0 (success). stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("killed child"),
        "the refusal must name the killed-child reason"
    );
    std::fs::remove_dir_all(&tmp).ok();
}

/// KNOWN-GOOD: a working `git` and empty staged deletions → SUCCESS (exit 0).
/// Proves the fix did not make the gate over-strict.
#[test]
fn working_git_with_no_deletions_passes() {
    let tmp = std::env::temp_dir().join(format!("pre-delete-good-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let repo = tmp.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    Command::new("git").args(["init"]).current_dir(&repo).output().unwrap();
    Command::new("git").args(["config", "user.email", "t@t"]).current_dir(&repo).output().unwrap();
    Command::new("git").args(["config", "user.name", "t"]).current_dir(&repo).output().unwrap();
    std::fs::write(repo.join("f.txt"), "hello").unwrap();
    Command::new("git").args(["add", "f.txt"]).current_dir(&repo).output().unwrap();
    Command::new("git").args(["commit", "-m", "init"]).current_dir(&repo).output().unwrap();

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_pre-delete-citation-check"));
    let output = Command::new(&bin)
        .current_dir(&repo)
        .env_remove("PRE_DELETE_OVERRIDE")
        .output()
        .expect("binary must run");
    assert_eq!(
        output.status.code(),
        Some(0),
        "a clean repo with no staged deletions must pass"
    );
    std::fs::remove_dir_all(&tmp).ok();
}

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_REPO: AtomicU64 = AtomicU64::new(0);
struct TempRepo {
    path: PathBuf,
}

impl std::ops::Deref for TempRepo {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run_git(repo: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .expect("git should spawn")
}

fn init_repo() -> TempRepo {
    let path = std::env::temp_dir().join(format!(
        "omp-commit-message-{}-{}",
        std::process::id(),
        NEXT_REPO.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).expect("temporary git repository");
    assert!(run_git(&path, &["init", "--quiet"]).status.success());
    for (key, value) in [
        ("user.name", "commit-message-test"),
        ("user.email", "test@example.invalid"),
    ] {
        assert!(run_git(&path, &["config", key, value]).status.success());
    }
    let hook = path.join(".git/hooks/commit-msg");
    fs::copy(env!("CARGO_BIN_EXE_pre-commit-gate"), &hook)
        .expect("install commit-msg hook fixture");
    let mut permissions = fs::metadata(&hook).expect("hook metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&hook, permissions).expect("make hook executable");
    fs::write(path.join("tracked.txt"), "content\n").expect("tracked fixture");
    assert!(run_git(&path, &["add", "tracked.txt"]).status.success());
    TempRepo { path }
}

fn commit_with_source(repo: &Path, source: &[u8], message: &[u8]) -> Output {
    let source_path = repo.join("message-source.txt");
    let message_path = repo.join("message-file.txt");
    fs::write(&source_path, source).expect("message source");
    fs::write(&message_path, message).expect("message file");
    Command::new("git")
        .args([
            "commit",
            "--allow-empty-message",
            "--quiet",
            "--no-gpg-sign",
            "-F",
        ])
        .arg(&message_path)
        .env("OMP_MSG_SRC", &source_path)
        .current_dir(repo)
        .output()
        .expect("git commit should spawn")
}

#[test]
fn ordinary_file_message_commits_and_round_trips_unchanged() {
    let repo = init_repo();
    let source = b"fix: ordinary message\n";
    let output = commit_with_source(&repo, source, source);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let log = run_git(&repo, &["log", "-1", "--format=%B"]);
    assert!(
        log.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&log.stderr)
    );
    let receipt = log
        .stdout
        .strip_suffix(b"\n")
        .expect("git log format terminator");
    assert_eq!(
        receipt, source,
        "git's message receipt must match the source bytes"
    );
}

#[test]
fn shell_substitution_families_are_refused_when_the_receipt_differs() {
    for token in ["`date`", "$(git status --short)", "$VERSION"] {
        let repo = init_repo();
        let source = format!("fix: preserve {token}\n");
        let output = commit_with_source(&repo, source.as_bytes(), b"fix: preserve token\n");
        assert!(!output.status.success(), "token {token:?} must be refused");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("MESSAGE MISMATCH"),
            "token {token:?}: {stderr}"
        );
        assert!(
            !run_git(&repo, &["rev-parse", "HEAD"]).status.success(),
            "a refused commit must not create a commit"
        );
    }
}

#[test]
fn empty_message_is_an_error_not_a_clean_round_trip() {
    let repo = init_repo();
    let output = commit_with_source(&repo, b"", b"");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("empty commit message"), "{stderr}");
}

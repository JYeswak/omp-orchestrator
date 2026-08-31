use commit_build_fence::{BuildRegistration, RegistrationStore};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn fresh_repo(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("commit-build-fence-{name}-{nonce}"));
    fs::create_dir_all(&dir).expect("create repo");
    run_git(&dir, &["init", "-q"]);
    run_git(&dir, &["config", "user.email", "fence@example.invalid"]);
    run_git(&dir, &["config", "user.name", "commit-fence-test"]);
    fs::write(dir.join("README.md"), "baseline\n").expect("write baseline");
    run_git(&dir, &["add", "--", "README.md"]);
    run_git(&dir, &["commit", "--quiet", "-m", "chore: baseline [test]"]);
    dir
}

fn run_git(dir: &Path, args: &[&str]) -> Output {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("spawn git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn run_git_with_store(dir: &Path, store: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("OMP_BUILD_REGISTRATION", store)
        .output()
        .expect("spawn git with fence store")
}
fn run_fence(args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_commit-build-fence"))
        .args(args)
        .output()
        .expect("spawn commit fence binary")
}

fn install_hook(dir: &Path) -> PathBuf {
    let hook = dir.join(".git/hooks/pre-commit");
    fs::copy(env!("CARGO_BIN_EXE_commit-build-fence"), &hook).expect("install fence hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))
            .expect("make hook executable");
    }
    hook
}

fn current_head(dir: &Path) -> String {
    String::from_utf8(run_git(dir, &["rev-parse", "HEAD"]).stdout)
        .expect("head utf8")
        .trim()
        .to_owned()
}

fn stage_file(dir: &Path, name: &str, contents: &str) {
    fs::write(dir.join(name), contents).expect("write staged file");
    run_git(dir, &["add", "--", name]);
}

fn store_for(dir: &Path) -> PathBuf {
    dir.join(".git/omp-build-registration.json")
}
fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs()
}

#[test]
fn real_hook_refuses_active_registration_with_actionable_identity() {
    let dir = fresh_repo("active");
    let store_path = store_for(&dir);
    RegistrationStore::empty()
        .save_atomic(&store_path)
        .expect("initialize explicit empty store");
    install_hook(&dir);

    stage_file(&dir, "clean.rs", "fn main() {}\n");
    let allowed = run_git_with_store(
        &dir,
        &store_path,
        &["commit", "--quiet", "-m", "chore: clean [test]"],
    );
    assert!(
        allowed.status.success(),
        "known-good commit refused: {}",
        String::from_utf8_lossy(&allowed.stderr)
    );

    let repo = dir.canonicalize().expect("canonical repo");
    let mut store = RegistrationStore::load(&store_path).expect("load store");
    let started_at_unix = now_unix();
    store
        .register(BuildRegistration {
            build_id: "build-live-42".to_owned(),
            repo: repo.display().to_string(),
            head: current_head(&dir),
            holder: "agent-blue".to_owned(),
            started_at_unix,
            expires_at_unix: started_at_unix + 1_800,
        })
        .expect("register build");
    store.save_atomic(&store_path).expect("save active store");

    stage_file(&dir, "blocked.rs", "fn blocked() {}\n");
    let head_before = current_head(&dir);
    let refused = run_git_with_store(
        &dir,
        &store_path,
        &["commit", "--quiet", "-m", "feat: blocked [test]"],
    );
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(!refused.status.success(), "active build must refuse commit");
    assert!(
        stderr.contains("COMMIT_FENCE_REFUSED"),
        "missing refusal marker: {stderr}"
    );
    assert!(
        stderr.contains("build-live-42"),
        "missing build id: {stderr}"
    );
    assert!(stderr.contains("agent-blue"), "missing holder: {stderr}");
    assert!(
        stderr.contains(&head_before),
        "missing current HEAD: {stderr}"
    );
    assert_eq!(
        current_head(&dir),
        head_before,
        "HEAD read-back must remain stable"
    );
}

#[test]
fn real_hook_allows_commit_with_valid_empty_store() {
    let dir = fresh_repo("good");
    let store_path = store_for(&dir);
    RegistrationStore::empty()
        .save_atomic(&store_path)
        .expect("initialize explicit empty store");
    install_hook(&dir);
    stage_file(&dir, "good.rs", "fn good() {}\n");

    let output = run_git_with_store(
        &dir,
        &store_path,
        &["commit", "--quiet", "-m", "chore: good [test]"],
    );
    assert!(
        output.status.success(),
        "valid empty store must allow commit: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn real_hook_treats_missing_store_as_error() {
    let dir = fresh_repo("missing");
    let store_path = dir.join(".git/missing-registration.json");
    install_hook(&dir);
    stage_file(&dir, "missing.rs", "fn missing() {}\n");

    let output = run_git_with_store(
        &dir,
        &store_path,
        &["commit", "--quiet", "-m", "feat: missing store [test]"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "missing store must fail closed");
    assert!(
        stderr.contains("COMMIT_FENCE_ERROR"),
        "missing typed error: {stderr}"
    );
    assert!(
        stderr.contains("registration_store_missing"),
        "missing-store reason absent: {stderr}"
    );
}
#[test]
fn cli_registration_expiry_and_release_are_durable() {
    let dir = fresh_repo("cli");
    let repo = dir.canonicalize().expect("canonical repo");
    let store = dir.join("build-registration.json");
    let repo_arg = repo.display().to_string();
    let store_arg = store.display().to_string();
    let init = run_fence(&[
        "init".to_owned(),
        "--repo".to_owned(),
        repo_arg.clone(),
        "--store".to_owned(),
        store_arg.clone(),
    ]);
    assert!(
        init.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );

    let head = current_head(&dir);
    let register = run_fence(&[
        "register".to_owned(),
        "--repo".to_owned(),
        repo_arg.clone(),
        "--store".to_owned(),
        store_arg.clone(),
        "--build-id".to_owned(),
        "build-cli-1".to_owned(),
        "--holder".to_owned(),
        "agent-blue".to_owned(),
        "--head".to_owned(),
        head.clone(),
        "--now".to_owned(),
        "100".to_owned(),
        "--ttl-secs".to_owned(),
        "1".to_owned(),
    ]);
    assert!(
        register.status.success(),
        "register failed: {}",
        String::from_utf8_lossy(&register.stderr)
    );

    let expired_check = run_fence(&[
        "check".to_owned(),
        "--repo".to_owned(),
        repo_arg.clone(),
        "--store".to_owned(),
        store_arg.clone(),
        "--head".to_owned(),
        head,
        "--now".to_owned(),
        "101".to_owned(),
    ]);
    assert!(
        expired_check.status.success(),
        "expired registration must clear the fence: {}",
        String::from_utf8_lossy(&expired_check.stderr)
    );

    let mut loaded = RegistrationStore::load(&store).expect("load registered store");
    let event = loaded
        .release("build-cli-1", &repo_arg, "agent-blue", 200)
        .expect("release expired registration");
    loaded.save_atomic(&store).expect("save release event");
    let reread = RegistrationStore::load(&store).expect("read released store");
    assert!(reread.registrations.is_empty());
    assert_eq!(reread.events, vec![event]);
}

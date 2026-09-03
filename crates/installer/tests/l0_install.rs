use installer::{
    check_build_fence, classify_restart_postcondition, git_head, git_rev_parse_short,
    install_binary, resolve_repo_ownership, verify_identity, InstallError, RepoOwnership,
    RestartPostcondition,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before UNIX_EPOCH")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "omp-installer-l0-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated test directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("installer crate must be two levels below repository root")
        .to_path_buf()
}

fn built_installer() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_installer"))
}

fn build_id() -> String {
    option_env!("OMP_BUILD_ID").unwrap_or("unavailable").to_owned()
}

fn identity_head() -> String {
    build_id()
}

#[test]
fn real_cli_version_identity() {
    let output = Command::new(built_installer())
        .arg("--version")
        .output()
        .expect("run the real installer binary");
    assert!(output.status.success(), "--version failed: {output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("build_id="), "missing build identity: {stdout}");
}

#[test]
fn real_git_identity_and_non_repo_refusal() {
    let outside = TempDir::new("not-a-repo");
    let error = git_head(outside.path()).expect_err("non-repository must refuse");
    assert!(matches!(error, InstallError::NotAGitRepo { .. }), "wrong refusal: {error}");
    let error = git_rev_parse_short(outside.path())
        .expect_err("non-repository short HEAD must refuse");
    assert!(matches!(error, InstallError::NotAGitRepo { .. }), "wrong short refusal: {error}");
}

#[test]
fn real_build_fence_refuses_marker() {
    let repo = TempDir::new("build-fence");
    check_build_fence(repo.path()).expect("no marker is admissible");
    fs::write(repo.path().join(".build_in_flight"), "owner=real-test\n")
        .expect("write build fence marker");
    let error = check_build_fence(repo.path()).expect_err("marker must refuse installation");
    assert!(matches!(error, InstallError::BuildInFlight { .. }), "wrong refusal: {error}");
}

#[test]
fn real_repo_ownership_is_scoped() {
    let root = repo_root();
    assert_eq!(resolve_repo_ownership(&root, "installer"), RepoOwnership::ThisRepo);
    assert_eq!(
        resolve_repo_ownership(&root, "definitely-not-a-real-crate"),
        RepoOwnership::Unknown
    );
}

#[test]
fn real_identity_probe_rejects_non_installer() {
    let check = verify_identity(
        Path::new("/usr/bin/true"),
        &identity_head(),
        &RepoOwnership::Unknown,
    );
    assert!(
        !check.consistent,
        "an unrelated system binary cannot match installer identity: {check:?}"
    );
}

#[test]
fn real_atomic_install_publishes_complete_binary() {
    let target = TempDir::new("atomic-success");
    let result = install_binary(
        &built_installer(),
        target.path(),
        &identity_head(),
        &RepoOwnership::ThisRepo,
    )
    .expect("real installer must publish its own verified binary");
    assert!(result.consistent, "published identity did not read back: {result:?}");
    let installed = target.path().join("installer");
    let metadata = fs::metadata(&installed).expect("published binary must exist");
    assert!(metadata.is_file() && metadata.len() > 0, "published artifact is incomplete");
    #[cfg(unix)]
    assert!(
        std::os::unix::fs::PermissionsExt::mode(&metadata.permissions()) & 0o111 != 0,
        "published artifact is not executable"
    );
}

#[test]
fn real_atomic_install_rejects_identity_mismatch_and_cleans_stage() {
    let target = TempDir::new("identity-mismatch");
    let error = install_binary(
        &built_installer(),
        target.path(),
        "0000000-not-the-real-head",
        &RepoOwnership::ThisRepo,
    )
    .expect_err("wrong HEAD must refuse the real installer artifact");
    assert!(matches!(error, InstallError::IdentityMismatch { .. }), "wrong refusal: {error}");
    assert!(!target.path().join("installer").exists(), "mismatched artifact was published");
    let staged = fs::read_dir(target.path())
        .expect("read isolated install directory")
        .map(|entry| entry.expect("read directory entry").path())
        .collect::<Vec<_>>();
    assert!(staged.is_empty(), "failed identity check left staged artifacts: {staged:?}");
}

#[test]
fn real_path_collision_refuses_before_replace() {
    let target = TempDir::new("path-collision");
    let existing = target.path().join("installer");
    fs::write(&existing, b"pre-existing-owner").expect("write existing destination owner");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&existing).expect("stat existing owner").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&existing, permissions).expect("make existing owner executable");
    }
    let result = install_binary(&built_installer(), target.path(), &identity_head(), &RepoOwnership::ThisRepo);
    assert!(result.is_err(), "real installer replaced a pre-existing destination owner");
    assert_eq!(fs::read(&existing).expect("read destination owner"), b"pre-existing-owner");
}

#[test]
fn real_restart_postcondition_lattice_is_restrictive() {
    assert_eq!(
        classify_restart_postcondition(Some(10), Some(11), true),
        RestartPostcondition::Verified
    );
    assert_eq!(
        classify_restart_postcondition(Some(10), Some(10), true),
        RestartPostcondition::NotRestarted
    );
    assert_eq!(
        classify_restart_postcondition(Some(10), None, true),
        RestartPostcondition::NotRunning
    );
    assert_eq!(
        classify_restart_postcondition(Some(10), Some(11), false),
        RestartPostcondition::IdentityMismatch
    );
}

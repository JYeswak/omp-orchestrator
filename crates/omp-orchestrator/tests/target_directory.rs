use omp_orchestrator::target_directory::{
    ensure_owner_record, read_owner_record, reap_target, resolve_target_directory, OwnerRecord,
    ReapDecision, TargetDirectoryError,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

fn owner(path: &std::path::Path) -> OwnerRecord {
    OwnerRecord::new(
        "omp-orchestrator",
        path,
        "nightly-aarch64-apple-darwin",
        4242,
    )
}

fn wait_for_open_file(pid: u32, path: &Path) {
    let lsof = if Path::new("/usr/sbin/lsof").exists() {
        "/usr/sbin/lsof"
    } else {
        "lsof"
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let output = Command::new(lsof)
            .args(["-nP", "-p", &pid.to_string()])
            .output()
            .expect("spawn lsof");
        if String::from_utf8_lossy(&output.stdout).contains(&path.display().to_string()) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "pid {pid} never opened {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn explicit_unowned_target_is_refused_before_use() {
    let repo = tempfile::tempdir().unwrap();
    let registered = tempfile::tempdir().unwrap();
    let candidate = PathBuf::from("/tmp/sw-target-b");
    let error = resolve_target_directory(
        repo.path(),
        Some(&candidate),
        &[registered.path().to_path_buf()],
    )
    .expect_err("an unowned explicit target must not be accepted");
    assert!(matches!(error, TargetDirectoryError::Unowned { .. }));
}

#[test]
fn repo_owned_target_with_owner_record_is_usable() {
    let repo = tempfile::tempdir().unwrap();
    let target = repo.path().join("target");
    let record = owner(&target);
    ensure_owner_record(&target, &record).unwrap();
    let resolved = resolve_target_directory(repo.path(), Some(&target), &[]).unwrap();
    assert_eq!(resolved.path, target);
    assert_eq!(read_owner_record(&target).unwrap(), record);
}

#[test]
fn registered_target_requires_owner_record() {
    let repo = tempfile::tempdir().unwrap();
    let registered = tempfile::tempdir().unwrap();
    let target = registered.path().join("qfa-job");
    fs::create_dir_all(&target).unwrap();
    let error = resolve_target_directory(
        repo.path(),
        Some(&target),
        &[registered.path().to_path_buf()],
    )
    .expect_err("registered root membership alone is not ownership");
    assert!(matches!(error, TargetDirectoryError::OwnerMissing { .. }));
}

#[test]
fn active_target_is_never_deleted() {
    let target = tempfile::tempdir().unwrap();
    let held = target.path().join("active.rmeta");
    fs::write(&held, b"held").unwrap();
    let record = owner(target.path());
    ensure_owner_record(target.path(), &record).unwrap();
    let mut child = Command::new("/usr/bin/tail")
        .args(["-f", held.to_str().unwrap()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    wait_for_open_file(child.id(), &held);
    let decision = reap_target(target.path(), true).unwrap();
    assert!(matches!(decision, ReapDecision::SkippedActive { .. }));
    assert!(target.path().exists());
    child.kill().unwrap();
    child.wait().unwrap();
}

#[test]
fn inactive_owned_target_can_be_reaped() {
    let target = tempfile::tempdir().unwrap();
    let path = target.path().to_path_buf();
    fs::write(path.join("debug-artifact"), b"reapable").unwrap();
    ensure_owner_record(&path, &owner(&path)).unwrap();
    let decision = reap_target(&path, true).unwrap();
    assert!(matches!(decision, ReapDecision::Removed { .. }));
    assert!(!path.exists());
}

#[test]
fn empty_reap_scan_is_an_error() {
    assert!(matches!(
        omp_orchestrator::target_directory::reap_targets(&[], false),
        Err(TargetDirectoryError::EmptyScan)
    ));
}

#[test]
fn repository_cargo_policy_forces_the_owned_target() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace repository root");
    let config = repo.join(".cargo/config.toml");
    let text = fs::read_to_string(&config).expect("repo Cargo target policy");
    assert!(text.contains("target-dir = \"target\""));
    assert!(text.contains("force = true"));
    let target = repo.join("target");
    let record = read_owner_record(&target).expect("build script owner record");
    assert_eq!(record.owner, "omp-orchestrator");
    assert_eq!(record.target, target.display().to_string());
}

#[test]
fn scoped_reaper_refuses_forged_owner_outside_allowed_roots() {
    let repo = tempfile::tempdir().unwrap();
    let registered = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let path = target.path().to_path_buf();
    ensure_owner_record(&path, &owner(&path)).unwrap();
    let error = omp_orchestrator::target_directory::reap_target_in_scope(
        &path,
        repo.path(),
        &[registered.path().to_path_buf()],
        true,
    )
    .expect_err("a forged owner record outside allowed roots must not be reaped");
    assert!(matches!(error, TargetDirectoryError::Unowned { .. }));
    assert!(path.exists());
}

use installer::{
    check_build_fence, classify_agent_scan, classify_restart_postcondition, git_head,
    git_rev_parse_short, install_binary, install_binary_with_durability, merge_hooks,
    catalog_artifact_dir, parse_cosign_version, publish_atomic, publish_atomic_durable,
    refuse_path_collisions, resolve_platform_triple, resolve_repo_ownership, restart_and_verify,
    running_process_start, seal_install_report, select_fallback_artifact, stage_artifact_stream,
    probe_build_id_string, verify_identity, verify_minisign_policy, verify_sigstore_policy,
    AgentOutcome, ArtifactEntry, DurabilityMetric, DurabilityStage, FullFsyncObservation, HookWrite,
    IdentityCheck, InstallError, MetricVerdict, RepoOwnership, RestartPostcondition,
    SigstoreTrust, COSIGN_CVE_FLOOR, COSIGN_CVE_ID,
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
fn real_probe_rejects_packed_sentinel_fixture() {
    let dir = TempDir::new("packed-sentinel");
    let binary = dir.path().join("unstamped-installer");
    fs::write(
        &binary,
        b"build_id=absentunavailableunversionedmarker\n",
    )
    .expect("packed sentinel fixture");
    assert_eq!(
        probe_build_id_string(&binary),
        None,
        "the parser must reject the packed sentinel token"
    );
    assert_ne!(
        probe_build_id_string(&built_installer()).as_deref(),
        Some("absentunavailableunversionedmarker")
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
    let error = install_binary(
        &built_installer(),
        target.path(),
        &identity_head(),
        &RepoOwnership::ThisRepo,
    )
    .expect_err("real installer replaced a pre-existing destination owner");
    // `is_err()` alone was satisfied by the incidental IdentityMismatch a host with
    // no derivable build id produces, so this test passed there while the clobber it
    // names went unmeasured. Name the refusal.
    assert!(
        matches!(error, InstallError::DestinationNotOurs { .. }),
        "refusal must name the collision, not an incidental identity failure: {error}"
    );
    assert_eq!(fs::read(&existing).expect("read destination owner"), b"pre-existing-owner");
    // BEFORE replace, not merely instead of it: nothing may be staged beside the owner.
    let entries: Vec<_> = fs::read_dir(target.path())
        .expect("read isolated install directory")
        .map(|entry| entry.expect("read directory entry").file_name())
        .collect();
    assert_eq!(
        entries.len(),
        1,
        "refusing before replace must leave the owner alone in the directory: {entries:?}"
    );
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

#[test]
fn installed_identity_readback() {
    // Composed install/restart boundary from existing mechanism only — no
    // duplicate identity system. install_binary publishes, verify_identity
    // reads the installed bytes back, and the restart boundary observes
    // without spawning (installer is launchd-unmanaged, so reads are Ok and
    // terminal-free on every platform, Linux workers included).
    let target = TempDir::new("identity-readback");
    let check = install_binary(
        &built_installer(),
        target.path(),
        &identity_head(),
        &RepoOwnership::ThisRepo,
    )
    .expect("publish for readback");
    assert!(check.consistent, "published identity did not verify: {check:?}");
    let installed = target.path().join("installer");
    // READBACK: installed bytes still tied to the verified build identity.
    let reread = verify_identity(&installed, &identity_head(), &RepoOwnership::ThisRepo);
    assert!(reread.consistent, "installed bytes lost identity: {reread:?}");
    assert_eq!(reread.build_id_in_binary, check.build_id_in_binary);
    // Restart boundary: read observes, restart characterizes, neither spawns.
    let start = running_process_start("installer").expect("read must not fail");
    assert_eq!(start, None);
    let post = restart_and_verify("installer", &installed, &identity_head(), start)
        .expect("boundary must not fail");
    assert_eq!(post, RestartPostcondition::NotRunning);
    // STAMPED versus UNSTAMPED pin: the real artifact verifies, a fixture
    // without identity never does.
    let stamped = verify_identity(&built_installer(), &identity_head(), &RepoOwnership::ThisRepo);
    assert!(stamped.consistent, "stamped artifact must verify: {stamped:?}");
    let bare = target.path().join("unstamped");
    fs::write(&bare, b"no identity here\n").expect("unstamped fixture");
    let unstamped = verify_identity(&bare, &identity_head(), &RepoOwnership::ThisRepo);
    assert!(!unstamped.consistent, "unidentity must not verify: {unstamped:?}");
    // KNOWN-BAD shape: altering installed bytes after publication breaks the
    // readback. verify_identity reports (consistent=false) rather than
    // refusing — the refusal lives in install_binary's IdentityMismatch arm,
    // pinned by real_atomic_install_rejects_identity_mismatch_and_cleans_stage.
    fs::write(&installed, b"tampered-bytes").expect("tamper");
    let tampered = verify_identity(&installed, &identity_head(), &RepoOwnership::ThisRepo);
    assert!(!tampered.consistent, "tampered bytes still verify: {tampered:?}");
}

#[test]
fn artifact_staging_complete_same_directory_temp() {
    let dir = TempDir::new("artifact-staging-complete");
    let payload = b"complete-archive-bytes";
    let staged = stage_artifact_stream(dir.path(), "installer", payload.as_slice(), payload.len() as u64)
        .expect("complete stream stages");
    assert_eq!(staged.parent(), Some(dir.path()));
    assert!(staged
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.contains(".staged.")));
    assert_eq!(fs::read(&staged).expect("read staged"), payload);
    assert!(!dir.path().join("installer").exists(), "must not publish before rename");
}

#[test]
fn artifact_staging_interrupt_leaves_no_publishable_temp() {
    let dir = TempDir::new("artifact-staging-interrupt");
    let data: &'static [u8] = b"0123456789abcdef";
    let error = stage_artifact_stream(
        dir.path(),
        "installer",
        std::io::Cursor::new(&data[..4]),
        data.len() as u64,
    )
    .expect_err("interrupted stream must not stage");
    match error {
        InstallError::IoError { detail, .. } => {
            assert!(detail.contains("STREAM_INCOMPLETE"), "{detail}");
        }
        other => panic!("expected STREAM_INCOMPLETE, got {other:?}"),
    }
    let leftovers: Vec<_> = fs::read_dir(dir.path())
        .expect("read dest")
        .map(|e| e.expect("entry").file_name())
        .collect();
    assert!(
        leftovers.is_empty(),
        "interrupted stream left publishable temp: {leftovers:?}"
    );
}

#[test]
fn minisign_policy_valid_passes() {
    verify_minisign_policy(true, true, true).expect("valid .minisig must pass");
}

#[test]
fn minisign_policy_require_absent_refuses() {
    let error = verify_minisign_policy(false, false, true)
        .expect_err("absent .minisig under --require-minisign must refuse");
    let text = error.to_string();
    assert!(
        text.starts_with("L0_MINISIGN_REFUSED"),
        "must be typed L0_MINISIGN_REFUSED, not a warning: {text}"
    );
}

#[test]
fn minisign_policy_require_invalid_refuses() {
    let error = verify_minisign_policy(true, false, true)
        .expect_err("invalid signature under --require-minisign must refuse");
    assert!(
        error.to_string().starts_with("L0_MINISIGN_REFUSED"),
        "{error}"
    );
}

#[test]
fn required_minisign_missing_refuses() {
    let error = verify_minisign_policy(false, true, true)
        .expect_err("T04: missing .minisig is refuse not PASS");
    assert!(error.to_string().contains("L0_MINISIGN_REFUSED"));
}

#[test]
fn atomic_publication_renames_complete_staged() {
    let dir = TempDir::new("atomic-publication-complete");
    let payload = b"complete-publish-bytes";
    let dest = dir.path().join("installer");
    let staged = stage_artifact_stream(
        dir.path(),
        "installer",
        payload.as_slice(),
        payload.len() as u64,
    )
    .expect("stage");
    publish_atomic(&staged, &dest, payload.len() as u64).expect("rename complete staged");
    assert_eq!(fs::read(&dest).expect("published"), payload);
    assert!(!staged.exists(), "staged temp must be consumed by rename");
}

#[test]
fn atomic_publication_refuses_mutated_staged() {
    let dir = TempDir::new("atomic-publication-mutated");
    let payload = b"verified-bytes";
    let dest = dir.path().join("installer");
    let staged = stage_artifact_stream(
        dir.path(),
        "installer",
        payload.as_slice(),
        payload.len() as u64,
    )
    .expect("stage");
    let mut mutated = payload.to_vec();
    mutated.push(b'X');
    fs::write(&staged, &mutated).expect("mutate after verification");
    let error = publish_atomic(&staged, &dest, payload.len() as u64)
        .expect_err("mutated staged must refuse rename");
    match error {
        InstallError::IoError { detail, .. } => {
            assert!(detail.contains("ATOMIC_REFUSED"), "{detail}");
        }
        other => panic!("expected ATOMIC_REFUSED, got {other:?}"),
    }
    assert!(!dest.exists(), "partial/mutated dest must not appear");
}

#[test]
fn atomic_publication_refuses_different_parent() {
    let src_dir = TempDir::new("atomic-publication-other-parent-src");
    let dest_dir = TempDir::new("atomic-publication-other-parent-dest");
    let payload = b"complete-but-wrong-dir";
    let staged = src_dir.path().join(".installer.staged.cross-dir");
    fs::write(&staged, payload).expect("write staged in other dir");
    let dest = dest_dir.path().join("installer");
    let error = publish_atomic(&staged, &dest, payload.len() as u64)
        .expect_err("cross-directory staged file must refuse");
    match error {
        InstallError::IoError { detail, .. } => {
            assert_eq!(
                detail,
                "ATOMIC_REFUSED: staged file is not in the destination directory",
                "{detail}"
            );
        }
        other => panic!("expected parent-mismatch ATOMIC_REFUSED, got {other:?}"),
    }
    assert!(!dest.exists(), "cross-dir rename must not publish dest");
}

#[test]
fn atomic_publication_refuses_non_staged_filename() {
    let dir = TempDir::new("atomic-publication-not-staged");
    let payload = b"complete-but-not-staged-name";
    let staged = dir.path().join("installer.tmp");
    fs::write(&staged, payload).expect("write non-staged complete file");
    let dest = dir.path().join("installer");
    let error = publish_atomic(&staged, &dest, payload.len() as u64)
        .expect_err("filename lacking .staged. must refuse");
    match error {
        InstallError::IoError { detail, .. } => {
            assert_eq!(detail, "ATOMIC_REFUSED: not a staged temporary", "{detail}");
        }
        other => panic!("expected not-staged ATOMIC_REFUSED, got {other:?}"),
    }
    assert!(!dest.exists(), "non-staged rename must not publish dest");
    assert_eq!(fs::read(&staged).expect("source intact"), payload);
}

#[test]
fn path_collision_lists_every_hit() {
    let a = TempDir::new("path-hit-a");
    let b = TempDir::new("path-hit-b");
    fs::write(a.path().join("installer"), b"owner-a").expect("hit a");
    fs::write(b.path().join("installer"), b"owner-b").expect("hit b");
    let path_env = format!("{}:{}", a.path().display(), b.path().display());
    let error = refuse_path_collisions("installer", &path_env, None)
        .expect_err("two PATH owners must refuse");
    match error {
        InstallError::PathCollision { hits } => {
            assert_eq!(hits.len(), 2, "{hits:?}");
            assert!(hits.iter().any(|h| h.ends_with("/installer")));
            let text = InstallError::PathCollision { hits: hits.clone() }.to_string();
            assert!(text.starts_with("L0_PATH_COLLISION"), "{text}");
            assert!(text.contains(&hits[0]) && text.contains(&hits[1]), "{text}");
        }
        other => panic!("expected PathCollision, got {other:?}"),
    }
}

/// Live-host observation, macOS only: /usr/sbin/installer is Apple platform
/// truth, not a portable fixture. Linux never treats this branch as
/// acceptance; the deterministic foreign-fixture test below is the portable
/// leg. The is_file guard stays: absence on macOS is an observation, while
/// absence of a CREATED fixture (below) is ERROR.
#[cfg(target_os = "macos")]
#[test]
fn path_collision_usr_sbin_first_refuses_without_overwrite() {
    let usr_sbin = Path::new("/usr/sbin/installer");
    if !usr_sbin.is_file() {
        eprintln!("skip: /usr/sbin/installer absent on this host");
        return;
    }
    let owned = TempDir::new("path-owned");
    let dest = owned.path().join("installer");
    fs::write(&dest, b"ours").expect("owned dest");
    let path_env = format!("/usr/sbin:{}", owned.path().display());
    let before = fs::read(&dest).expect("read before");
    let error = refuse_path_collisions("installer", &path_env, Some(&dest))
        .expect_err("/usr/sbin/installer on PATH must refuse");
    match error {
        InstallError::PathCollision { hits } => {
            assert!(
                hits.iter().any(|h| h == "/usr/sbin/installer"),
                "{hits:?}"
            );
            assert!(!hits.iter().any(|h| Path::new(h) == dest.as_path()));
        }
        other => panic!("expected PathCollision, got {other:?}"),
    }
    assert_eq!(fs::read(&dest).expect("read after"), before, "no overwrite");
}

#[test]
fn foreign_path_collision_refuses_before_replace() {
    // Deterministic cross-platform fixture: a FOREIGN TempDir holding an
    // `installer` binary first on PATH, plus an owned destination carrying
    // our bytes. No live-host paths anywhere; every fixture here is created
    // by the test, so absence is ERROR (expect), never a skip. Wired through
    // refuse_path_collisions — the gate before replace.
    let foreign = TempDir::new("path-foreign");
    let foreign_hit = foreign.path().join("installer");
    fs::write(&foreign_hit, b"foreign-owner").expect("foreign hit");
    let owned = TempDir::new("path-owned-f");
    let dest = owned.path().join("installer");
    fs::write(&dest, b"ours").expect("owned dest");
    let path_env = format!("{}:{}", foreign.path().display(), owned.path().display());
    // ONE exact foreign hit: the owned destination is filtered from the
    // refusal because it is ours, not a collision.
    let error = refuse_path_collisions("installer", &path_env, Some(&dest))
        .expect_err("foreign installer on PATH must refuse");
    match error {
        InstallError::PathCollision { hits } => {
            assert_eq!(hits, vec![foreign_hit.display().to_string()], "{hits:?}");
            let text = InstallError::PathCollision { hits: hits.clone() }.to_string();
            assert!(text.starts_with("L0_PATH_COLLISION"), "{text}");
            assert!(text.contains(&hits[0]), "{text}");
        }
        other => panic!("expected PathCollision, got {other:?}"),
    }
    assert_eq!(fs::read(&dest).expect("read after"), b"ours", "no overwrite");
    // Negative control: the owned destination alone on PATH is no collision.
    let owned_only = owned.path().display().to_string();
    refuse_path_collisions("installer", &owned_only, Some(&dest))
        .expect("owned-only PATH is no collision");
}

#[test]
fn hook_merge_backups_before_write() {
    let dir = TempDir::new("hook-merge-ok");
    let hook = dir.path().join("pre-commit");
    fs::write(&hook, b"pre-merge").expect("seed");
    let backups = merge_hooks(
        &[HookWrite {
            path: hook.clone(),
            merged: b"post-merge".to_vec(),
        }],
        None,
    )
    .expect("merge");
    assert_eq!(backups.len(), 1);
    assert!(backups[0].backup.file_name().unwrap().to_string_lossy().contains(".bak."));
    assert_eq!(backups[0].path, hook);
    assert!(backups[0].existed);
    assert_eq!(fs::read(&backups[0].backup).expect("backup"), b"pre-merge");
    assert_eq!(fs::read(&hook).expect("hook"), b"post-merge");
}

#[test]
fn hook_merge_injected_failure_restores_pre_merge_bytes() {
    let dir = TempDir::new("hook-merge-fail");
    let first = dir.path().join("pre-commit");
    let second = dir.path().join("post-commit");
    fs::write(&first, b"first-orig").expect("seed first");
    fs::write(&second, b"second-orig").expect("seed second");
    let error = merge_hooks(
        &[
            HookWrite {
                path: first.clone(),
                merged: b"first-new".to_vec(),
            },
            HookWrite {
                path: second.clone(),
                merged: b"second-new".to_vec(),
            },
        ],
        Some(1),
    )
    .expect_err("fail the second hook write");
    match error {
        InstallError::HookMergeFailed { backups } => {
            assert_eq!(backups.len(), 2);
            let text = InstallError::HookMergeFailed {
                backups: backups.clone(),
            }
            .to_string();
            assert!(text.starts_with("L0_HOOK_MERGE"), "{text}");
            assert!(text.contains(&backups[0]) && text.contains(&backups[1]), "{text}");
        }
        other => panic!("expected HookMergeFailed, got {other:?}"),
    }
    assert_eq!(fs::read(&first).expect("first"), b"first-orig");
    assert_eq!(fs::read(&second).expect("second"), b"second-orig");
}

#[test]
fn hook_merge_backup_records_prior_presence() {
    // Backup coverage is one-to-one with mutation coverage: each record
    // names its mutated path, and prior presence travels on the record —
    // never inferred from bytes afterward. A new path gets a record with
    // existed=false and no backup file (there are no prior bytes to keep).
    let dir = TempDir::new("hook-merge-presence");
    let existing = dir.path().join("pre-commit");
    let new = dir.path().join("post-commit");
    fs::write(&existing, b"orig").expect("seed");
    assert!(!new.exists());
    let backups = merge_hooks(
        &[
            HookWrite {
                path: existing.clone(),
                merged: b"new-orig".to_vec(),
            },
            HookWrite {
                path: new.clone(),
                merged: b"new-file".to_vec(),
            },
        ],
        None,
    )
    .expect("merge");
    assert_eq!(backups.len(), 2);
    assert_eq!(backups[0].path, existing);
    assert!(backups[0].existed);
    assert_eq!(fs::read(&backups[0].backup).expect("backup"), b"orig");
    assert_eq!(backups[1].path, new);
    assert!(!backups[1].existed);
    assert!(
        !backups[1].backup.exists(),
        "no backup file for a path with no prior bytes"
    );
    assert_eq!(fs::read(&new).expect("new hook"), b"new-file");
}

#[test]
fn hook_merge_rollback_deletes_new_path_residue() {
    // KNOWN-BAD LEG: deleting the remove_file arm (restoring original bytes
    // for every row, including new paths) leaves the new path behind as an
    // empty file and this leg goes RED. Rollback restores existing bytes and
    // deletes new residue; it never empty-byte fake-restores. Three writes so
    // the injected failure lands after both the restore arm and the delete
    // arm have fired (fail_after fires at loop top, so it must name a live
    // index: with two writes Some(2) never fires and the merge returns Ok).
    let dir = TempDir::new("hook-merge-rollback-new");
    let existing = dir.path().join("pre-commit");
    let new = dir.path().join("post-commit");
    let later = dir.path().join("post-receive");
    fs::write(&existing, b"keep-me").expect("seed");
    fs::write(&later, b"untouched").expect("seed later");
    let error = merge_hooks(
        &[
            HookWrite {
                path: existing.clone(),
                merged: b"changed".to_vec(),
            },
            HookWrite {
                path: new.clone(),
                merged: b"residue".to_vec(),
            },
            HookWrite {
                path: later.clone(),
                merged: b"never-written".to_vec(),
            },
        ],
        Some(2),
    )
    .expect_err("injected failure after both arms fired");
    match error {
        InstallError::HookMergeFailed { backups } => assert_eq!(backups.len(), 3),
        other => panic!("expected HookMergeFailed, got {other:?}"),
    }
    assert_eq!(fs::read(&existing).expect("existing"), b"keep-me");
    assert!(
        !new.exists(),
        "rolled-back new path must be absent, not an empty file"
    );
    assert_eq!(fs::read(&later).expect("later"), b"untouched");
}

#[test]
fn hook_merge_empty_writes_is_typed_error() {
    // Merging nothing is a typed error, never a vacuous Ok with zero backups.
    match merge_hooks(&[], None).expect_err("empty write set must refuse") {
        InstallError::EmptyHookWrites => {}
        other => panic!("expected EmptyHookWrites, got {other:?}"),
    }
    let text = InstallError::EmptyHookWrites.to_string();
    assert!(text.starts_with("L0_HOOK_MERGE_EMPTY"), "{text}");
}

#[test]
fn platform_triple_resolver() {
    let cases = [
        (("macos", "aarch64", None), "aarch64-apple-darwin"),
        (("macos", "x86_64", None), "x86_64-apple-darwin"),
        (("linux", "aarch64", Some("gnu")), "aarch64-unknown-linux-gnu"),
        (("linux", "x86_64", Some("gnu")), "x86_64-unknown-linux-gnu"),
    ];
    for ((os, arch, libc), expected) in cases {
        let resolved = resolve_platform_triple(os, arch, libc).expect("supported tuple");
        assert_eq!(resolved.artifact_triple, expected);
        assert_eq!(resolved.fallback, None);
    }
    let musl = resolve_platform_triple("linux", "x86_64", Some("musl"))
        .expect("musl fallback tuple");
    assert_eq!(musl.artifact_triple, "x86_64-unknown-linux-gnu");
    assert_eq!(musl.fallback, Some("musl-to-gnu"));

    let error = resolve_platform_triple("windows", "x86_64", None)
        .expect_err("unsupported tuple must refuse");
    assert!(matches!(error, InstallError::PlatformTripleUnsupported { .. }));
    assert_eq!(error.to_string().split_whitespace().next(), Some("L0-PLATFORM-TRIPLE"));
}

#[test]
fn musl_fallback_requires_gnu_artifact() {
    // Bead mauc: linux/x86_64/musl takes the gnu artifact ONLY when the
    // catalog proves a nonempty existing file for it. resolve_platform_triple
    // stays pure selection; this composes selection with the catalog before
    // any staging.
    let dir = TempDir::new("musl-fallback");
    let gnu = dir.path().join("ompo");
    fs::write(&gnu, b"gnu-artifact").expect("gnu artifact");
    let gnu_triple = "x86_64-unknown-linux-gnu";
    let musl =
        resolve_platform_triple("linux", "x86_64", Some("musl")).expect("musl tuple");
    assert_eq!(musl.fallback, Some("musl-to-gnu"));
    // KNOWN-GOOD: the catalog names the gnu artifact, present and nonempty.
    let catalog = vec![ArtifactEntry {
        triple: gnu_triple.to_owned(),
        binary: "ompo".to_owned(),
        path: gnu.clone(),
    }];
    let selected =
        select_fallback_artifact(&musl, &catalog, "ompo").expect("gnu fallback present");
    assert_eq!(selected, gnu);
    // WITHOUT it the fallback refuses with the typed error, never a blind join.
    let error =
        select_fallback_artifact(&musl, &[], "ompo").expect_err("absent gnu must refuse");
    assert!(
        matches!(error, InstallError::ArtifactUnavailable { .. }),
        "wrong refusal: {error:?}"
    );
    assert_eq!(error.to_string().split_whitespace().next(), Some("L0-PLATFORM-TRIPLE"));
}

#[test]
fn fallback_artifact_refuses_absent_wrong_unreadable_empty() {
    // Each catalog defect refuses typed while the others stay green: absent
    // row, wrong arch/triple, unreadable file, empty file.
    // KNOWN-BAD LEG (unreadable): deleting the metadata existence check
    // forwards a missing file and this leg goes RED.
    let dir = TempDir::new("fallback-defects");
    let gnu_triple = "x86_64-unknown-linux-gnu";
    let musl =
        resolve_platform_triple("linux", "x86_64", Some("musl")).expect("musl tuple");
    let present = dir.path().join("ompo");
    fs::write(&present, b"bytes").expect("present artifact");
    let hollow_file = dir.path().join("hollow-blob");
    fs::write(&hollow_file, b"").expect("empty blob");
    let wrong = ArtifactEntry {
        triple: "aarch64-apple-darwin".to_owned(),
        binary: "ompo".to_owned(),
        path: present.clone(),
    };
    let unreadable = ArtifactEntry {
        triple: gnu_triple.to_owned(),
        binary: "ompo".to_owned(),
        path: dir.path().join("pane-truth"),
    };
    let hollow = ArtifactEntry {
        triple: gnu_triple.to_owned(),
        binary: "ompo".to_owned(),
        path: hollow_file,
    };
    for (name, catalog) in [
        ("absent-row", Vec::new()),
        ("wrong-arch-triple", vec![wrong]),
        ("unreadable-file", vec![unreadable]),
        ("empty-file", vec![hollow]),
    ] {
        let error = match select_fallback_artifact(&musl, &catalog, "ompo") {
            Err(error) => error,
            Ok(_) => panic!("{name} must refuse"),
        };
        assert!(
            matches!(error, InstallError::ArtifactUnavailable { .. }),
            "{name} wrong refusal: {error:?}"
        );
        assert_eq!(
            error.to_string().split_whitespace().next(),
            Some("L0-PLATFORM-TRIPLE"),
            "{name}"
        );
    }
    // The empty arm refuses for emptiness, not mere absence: re-run it alone
    // and pin the detail word.
    let hollow_alone = ArtifactEntry {
        triple: gnu_triple.to_owned(),
        binary: "ompo".to_owned(),
        path: dir.path().join("hollow-blob"),
    };
    let error = select_fallback_artifact(&musl, &[hollow_alone], "ompo")
        .expect_err("empty file must refuse");
    assert!(error.to_string().contains("is empty"), "{error}");
}

#[test]
fn catalog_scan_tags_files_with_resolved_triple() {
    // The directory scan tags every regular file with the resolved triple
    // and skips subdirectories; an unreadable directory is a typed error.
    let dir = TempDir::new("catalog-scan");
    fs::write(dir.path().join("ompo"), b"a").expect("artifact a");
    fs::write(dir.path().join("tick-monitor"), b"b").expect("artifact b");
    fs::create_dir(dir.path().join("nested")).expect("subdir");
    let catalog =
        catalog_artifact_dir(dir.path(), "x86_64-unknown-linux-gnu").expect("scan");
    assert_eq!(catalog.len(), 2);
    for row in &catalog {
        assert_eq!(row.triple, "x86_64-unknown-linux-gnu");
    }
    let names: Vec<&str> = catalog.iter().map(|row| row.binary.as_str()).collect();
    assert!(names.contains(&"ompo") && names.contains(&"tick-monitor"));
    let error = catalog_artifact_dir(&dir.path().join("absent"), "x86_64-unknown-linux-gnu")
        .expect_err("unreadable catalog must refuse");
    assert!(
        matches!(error, InstallError::ArtifactUnavailable { .. }),
        "wrong refusal: {error:?}"
    );
}
#[test]
fn zero_agents_is_error_not_clean() {
    let error = classify_agent_scan(&[]).expect_err("empty-scan must not pass");
    assert!(
        matches!(error, InstallError::EmptyAgentScan),
        "wrong refusal: {error}"
    );
    let text = error.to_string();
    assert!(
        text.contains("L0_EMPTY_SCAN"),
        "empty-scan ERROR must name the reason code: {text}"
    );
    let lower = text.to_ascii_lowercase();
    assert!(!lower.contains("clean"), "must not report clean: {text}");
    assert!(!text.contains("PASS"), "must not report PASS: {text}");

    let scan = classify_agent_scan(&["omp"]).expect("one family is a scan, not empty");
    assert_eq!(scan.families, ["omp"]);
}

fn sample_identity() -> IdentityCheck {
    IdentityCheck {
        binary_name: "installer".to_owned(),
        repo_ownership: RepoOwnership::ThisRepo,
        head_sha: "abc123def".to_owned(),
        build_id_in_binary: Some("abc123def".to_owned()),
        version_output: Some("installer 0.1.0 build_id=abc123def".to_owned()),
        consistent: true,
    }
}

#[test]
fn install_report_contains_outcomes_backups_path_hits_digest_and_identity() {
    let detected = classify_agent_scan(&["omp", "codex"]).expect("scan");
    let report = seal_install_report(
        &detected,
        vec![
            AgentOutcome {
                family: "omp".to_owned(),
                outcome: "installed".to_owned(),
            },
            AgentOutcome {
                family: "codex".to_owned(),
                outcome: "installed".to_owned(),
            },
        ],
        vec![PathBuf::from("/tmp/pre-commit.bak.1")],
        vec![PathBuf::from("/usr/sbin/installer")],
        sample_identity(),
    )
    .expect("complete report is success");
    let text = report.to_string();
    assert!(text.starts_with("L0-REPORT"), "{text}");
    assert!(
        text.contains("omp=installed") && text.contains("codex=installed"),
        "{text}"
    );
    assert!(text.contains("backups=/tmp/pre-commit.bak.1"), "{text}");
    assert!(text.contains("path_hits=/usr/sbin/installer"), "{text}");
    assert!(
        text.contains("digest=") && !report.digest.is_empty(),
        "{text}"
    );
    assert!(
        text.contains("identity=installer") && text.contains("HEAD=abc123def"),
        "{text}"
    );
}

#[test]
fn install_report_refuses_when_one_detected_agent_is_dropped() {
    let detected = classify_agent_scan(&["omp", "codex"]).expect("scan");
    let error = seal_install_report(
        &detected,
        vec![AgentOutcome {
            family: "omp".to_owned(),
            outcome: "installed".to_owned(),
        }],
        vec![PathBuf::from("/tmp/pre-commit.bak.1")],
        vec![PathBuf::from("/usr/sbin/installer")],
        sample_identity(),
    )
    .expect_err("dropping codex must fail completeness");
    match error {
        InstallError::IncompleteInstallReport { missing } => {
            assert_eq!(missing, vec!["codex".to_owned()]);
            let text = InstallError::IncompleteInstallReport { missing }.to_string();
            assert!(text.starts_with("L0-REPORT"), "{text}");
            assert!(text.contains("codex"), "{text}");
        }
        other => panic!("expected IncompleteInstallReport, got {other:?}"),
    }
}


// ── omp-orchestrator-j9ngc: THE FLAGSHIP MUST BE INSIDE THE PROOF ─────────────────
//
// MEASURED 2026-09-10 at HEAD 08e9bc3, `~/.local/bin/installer --check` (unpiped,
// rc=1) opened with `omp-orchestrator: NOT INSTALLED (skipped)` while the flagship
// `ompo` — 7,869,104 bytes, installed, running — was never probed. The installed
// artifact (build_id 2a862a2) carried a roster the cutover 07dad3f had already
// renamed. A skipped row reads identically to a passing one, so the hole was
// invisible from the output.

/// Create a throwaway git repository with exactly one commit and return its full sha.
///
/// The identity check reads HEAD from git, and the remote build worker's synced tree
/// has a `.git` with NO commits (measured: `git rev-parse HEAD` exits 128 there). So
/// the CLI test carries its own repository and hands it to the child through GIT_DIR
/// rather than depending on the tree it happens to be built in.
fn temp_git_repo_with_one_commit(root: &Path) -> String {
    fs::create_dir_all(root).expect("create the throwaway repository directory");
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(["-c", "user.email=test@invalid", "-c", "user.name=test"])
            .args(args)
            .current_dir(root)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    };
    git(&["init", "-q"]);
    git(&["commit", "-q", "--allow-empty", "-m", "identity fixture"]);
    let head = git(&["rev-parse", "HEAD"]);
    assert_eq!(head.len(), 40, "expected a full sha, got {head:?}");
    head
}

/// A stamped artifact the identity probe can read: `strings` finds the build id, and
/// the file is deliberately not executable, so the `--version` leg is absent and the
/// row reports `legs=build_id`.
fn write_stamped_artifact(path: &Path, build_id: &str) {
    fs::write(path, format!("build_id={build_id}\nfixture artifact\n"))
        .expect("write the stamped fixture artifact");
}

#[test]
fn real_roster_covers_the_flagship_and_names_only_bin_targets_this_workspace_builds() {
    let root = repo_root();
    assert!(
        installer::OWNED_BINARIES
            .iter()
            .any(|&(krate, bin)| krate == "ompo-doctor" && bin == "ompo"),
        "the flagship must be IN the roster, not outside the proof: {:?}",
        installer::OWNED_BINARIES
    );
    for &(crate_name, bin) in installer::OWNED_BINARIES {
        assert!(
            installer::crate_declares_bin(&root, crate_name, bin),
            "roster entry ({crate_name}, {bin}) names a bin target this workspace does \
             not build; {crate_name} declares {:?}",
            installer::declared_bin_targets(&root, crate_name)
        );
    }
    // NEGATIVE CONTROL. The pair the INSTALLED installer still carries must be
    // REJECTED, or the loop above proves only that the instrument always says yes.
    assert!(
        !installer::crate_declares_bin(&root, "omp-orchestrator", "omp-orchestrator"),
        "the instrument cannot say NO: crates/omp-orchestrator declares {:?}",
        installer::declared_bin_targets(&root, "omp-orchestrator")
    );
    // ...and it must still say YES to the bin that crate does build, or it is merely
    // broken rather than discriminating.
    assert!(
        installer::crate_declares_bin(&root, "omp-orchestrator", "omp-target-dir"),
        "declared targets: {:?}",
        installer::declared_bin_targets(&root, "omp-orchestrator")
    );
}

#[test]
fn real_sweep_names_every_roster_entry_and_a_stale_flagship_is_a_mismatch() {
    let root = repo_root();
    let bins = TempDir::new("flagship-sweep");
    let head = "a".repeat(40);
    let wrong = "b".repeat(40);
    let flagship: &[(&str, &str)] = &[("ompo-doctor", "ompo")];

    // ABSENT: still NAMED, and named as something other than coverage.
    let absent = installer::sweep_installed_identity(&root, bins.path(), &head, flagship);
    assert_eq!(absent.rows.len(), 1, "{absent:?}");
    let row = absent.rows[0].to_string();
    assert!(
        row.starts_with("ompo:") && row.contains("NOT coverage"),
        "an absent flagship must be named and disclaimed: {row}"
    );
    assert_eq!(absent.probed, 0, "nothing was compared: {absent:?}");
    assert_eq!(absent.exit_code(), 0, "{absent:?}");

    // STALE: the flagship disagrees with HEAD -> MISMATCH for `ompo` SPECIFICALLY.
    let flagship_path = bins.path().join("ompo");
    write_stamped_artifact(&flagship_path, &wrong);
    let drifted = installer::sweep_installed_identity(&root, bins.path(), &head, flagship);
    let row = drifted
        .row_for("ompo")
        .expect("the flagship must have a row")
        .to_string();
    assert!(row.starts_with("ompo:"), "the row must NAME the flagship: {row}");
    assert!(row.contains("MISMATCH"), "a stale flagship must MISMATCH: {row}");
    assert!(row.contains("legs=build_id"), "the legs must be named: {row}");
    assert_eq!(drifted.probed, 1, "{drifted:?}");
    assert_eq!(drifted.mismatches, 1, "{drifted:?}");
    assert_eq!(
        drifted.exit_code(),
        1,
        "drift must map to a NONZERO exit: {drifted:?}"
    );

    // RESTORED: a fresh flagship reports OK, so the MISMATCH above was a measurement
    // and not an instrument that can only fail.
    write_stamped_artifact(&flagship_path, &head);
    let fresh = installer::sweep_installed_identity(&root, bins.path(), &head, flagship);
    let row = fresh
        .row_for("ompo")
        .expect("the flagship must have a row")
        .to_string();
    assert!(row.contains("IDENTITY OK"), "{row}");
    assert_eq!(fresh.mismatches, 0, "{fresh:?}");
    assert_eq!(fresh.exit_code(), 0, "{fresh:?}");
}

#[test]
fn real_sweep_refuses_a_roster_entry_naming_an_artifact_this_workspace_does_not_build() {
    let root = repo_root();
    let bins = TempDir::new("roster-stale");
    let head = "c".repeat(40);
    // The exact pair the installed artifact carries. A file of that name EXISTS and is
    // stamped with the right build id: a roster entry naming a deleted bin target is
    // stale whether or not something answers to the name, so integrity is checked
    // before installation state.
    write_stamped_artifact(&bins.path().join("omp-orchestrator"), &head);
    let stale = installer::sweep_installed_identity(
        &root,
        bins.path(),
        &head,
        &[("omp-orchestrator", "omp-orchestrator")],
    );
    let row = stale.rows[0].to_string();
    assert!(row.contains("ROSTER STALE"), "{row}");
    assert_eq!(stale.roster_stale, 1, "{stale:?}");
    assert_eq!(stale.probed, 0, "a stale entry can never be probed: {stale:?}");
    assert!(stale.drifted(), "{stale:?}");
    assert_eq!(stale.exit_code(), 1, "{stale:?}");
}

#[test]
fn real_cli_check_exits_nonzero_for_a_stale_flagship_and_zero_when_all_agree() {
    let fixture = TempDir::new("cli-check");
    let repo = fixture.path().join("repo");
    let head = temp_git_repo_with_one_commit(&repo);
    let bins = fixture.path().join("bin");
    fs::create_dir_all(&bins).expect("create the fixture install directory");

    let run = |bins: &Path| {
        let output = Command::new(built_installer())
            .arg("--check")
            .arg("--bin-dir")
            .arg(bins)
            .env("GIT_DIR", repo.join(".git"))
            .env("GIT_WORK_TREE", &repo)
            .output()
            .expect("run the real installer binary");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        // Captured by cargo unless `-- --nocapture`, where it becomes the pasteable
        // OBSERVED output this bead's coverage clause asks for. An assertion proves the
        // rows are right; only the rows themselves show WHAT was checked.
        println!(
            "OBSERVED installer --check --bin-dir {} exit={:?}\n{text}",
            bins.display(),
            output.status.code()
        );
        (output.status.code(), text)
    };

    // Every roster binary present and fresh EXCEPT the flagship, which is one commit
    // behind. The one row that must go red is `ompo`.
    for &(_, bin) in installer::OWNED_BINARIES {
        write_stamped_artifact(&bins.join(bin), &head);
    }
    write_stamped_artifact(&bins.join("ompo"), &"d".repeat(40));
    let (code, text) = run(&bins);
    assert_eq!(code, Some(1), "a stale flagship must exit 1:\n{text}");
    assert!(
        text.lines().any(|line| line.trim().starts_with("ompo:")
            && line.contains("MISMATCH")),
        "the flagship must be a CHECKED row reporting MISMATCH:\n{text}"
    );
    assert!(
        !text.contains("NOT INSTALLED (skipped)"),
        "no row may report the retired benign skip:\n{text}"
    );

    // RESTORE the flagship byte-for-byte to a fresh stamp: the same command now exits
    // 0 and says how many artifacts it actually compared.
    write_stamped_artifact(&bins.join("ompo"), &head);
    let (code, text) = run(&bins);
    assert_eq!(code, Some(0), "a fresh roster must exit 0:\n{text}");
    assert!(
        text.contains(&format!(
            "INSTALLER IDENTITY OK: {}/{}",
            installer::OWNED_BINARIES.len(),
            installer::OWNED_BINARIES.len()
        )),
        "every roster binary must be counted as probed:\n{text}"
    );

    // AN EMPTY INSTALL DIRECTORY IS NOT AN OK. This is the one invocation anything
    // actually runs (gate.yml -> gate-runner --run -> the declared check in this
    // crate's Cargo.toml, against an empty {scratch}), and it used to print
    // `INSTALLER IDENTITY OK: 0/0`.
    let empty = fixture.path().join("empty");
    fs::create_dir_all(&empty).expect("create the empty install directory");
    let (code, text) = run(&empty);
    assert_eq!(code, Some(0), "roster integrity held, so this is not a red:\n{text}");
    assert!(
        text.contains("INSTALLER IDENTITY UNPROVEN: 0 of"),
        "zero probed artifacts must not render as an identity proof:\n{text}"
    );
    assert!(
        !text.contains("INSTALLER IDENTITY OK"),
        "0/0 must never read as OK:\n{text}"
    );
}

/// The exact token `strings` recovered from the INSTALLED flagship on 2026-09-11.
/// `nogit-<epoch>` is what `crates/installer/build.rs:39` stamps when the build host
/// cannot resolve HEAD; the trailing run is neighbouring rodata the marker is packed
/// against, which is why the token is not clipped at the epoch.
const MEASURED_UNDERIVED_TOKEN: &str = "nogit-1789100480ts_unixsupervisor_heartbeatdispatch_action";

#[test]
fn real_underived_build_id_is_unstamped_and_a_wrong_commit_is_still_a_mismatch() {
    // The classifier, with both controls: a generated id names no commit, a real sha
    // does, and an instrument that answered the same for both would be broken.
    assert!(
        !installer::token_names_a_commit(MEASURED_UNDERIVED_TOKEN),
        "the underived fallback id must not read as a commit"
    );
    assert!(
        !installer::token_names_a_commit("nogit-1789100480"),
        "the unpacked fallback id must not read as a commit either"
    );
    assert!(
        installer::token_names_a_commit(&"b".repeat(40)),
        "a 40-hex sha must read as a commit, or the classifier only says NO"
    );
    assert!(
        !installer::token_names_a_commit("bbbb"),
        "four hex characters are narrower than the shortest abbreviation accepted"
    );

    let root = repo_root();
    let bins = TempDir::new("unstamped-flagship");
    let head = "a".repeat(40);
    let flagship: &[(&str, &str)] = &[("ompo-doctor", "ompo")];
    let flagship_path = bins.path().join("ompo");

    // THE MEASURED STATE. The artifact carries an underived id and no `--version`
    // leg (the fixture is not executable), so NOTHING was compared.
    write_stamped_artifact(&flagship_path, MEASURED_UNDERIVED_TOKEN);
    let report = installer::sweep_installed_identity(&root, bins.path(), &head, flagship);
    let row = report
        .row_for("ompo")
        .expect("the flagship must have a row")
        .to_string();
    assert!(
        row.contains("UNSTAMPED") && row.contains("UNMEASURED rather than drifted"),
        "an artifact that names no commit must not be rendered as drift: {row}"
    );
    assert!(!row.contains("MISMATCH"), "{row}");
    assert_eq!(report.unstamped, 1, "{report:?}");
    assert_eq!(
        report.mismatches, 0,
        "an unstamped artifact is not a disagreeing one: {report:?}"
    );
    assert_eq!(report.probed, 1, "the legs WERE read: {report:?}");
    assert!(report.drifted(), "UNSTAMPED is still a finding: {report:?}");
    assert_eq!(
        report.exit_code(),
        3,
        "instrument error, not degraded: {report:?}"
    );

    // NEGATIVE CONTROL on the report, not just the token: a COMMIT-shaped id that
    // disagrees with HEAD must still be a MISMATCH exiting 1, or the change has
    // merely renamed every failure.
    write_stamped_artifact(&flagship_path, &"b".repeat(40));
    let drifted = installer::sweep_installed_identity(&root, bins.path(), &head, flagship);
    assert_eq!(drifted.mismatches, 1, "{drifted:?}");
    assert_eq!(drifted.unstamped, 0, "{drifted:?}");
    assert_eq!(drifted.exit_code(), 1, "{drifted:?}");

    // ...and a fresh stamp is still clean, so neither verdict is an instrument that
    // can only fail.
    write_stamped_artifact(&flagship_path, &head);
    let fresh = installer::sweep_installed_identity(&root, bins.path(), &head, flagship);
    assert_eq!(fresh.unstamped, 0, "{fresh:?}");
    assert_eq!(fresh.exit_code(), 0, "{fresh:?}");
}

#[test]
fn real_cli_check_names_an_unstamped_artifact_without_calling_it_drift() {
    let fixture = TempDir::new("cli-unstamped");
    let repo = fixture.path().join("repo");
    let head = temp_git_repo_with_one_commit(&repo);
    let bins = fixture.path().join("bin");
    fs::create_dir_all(&bins).expect("create the fixture install directory");
    for &(_, bin) in installer::OWNED_BINARIES {
        write_stamped_artifact(&bins.join(bin), &head);
    }
    write_stamped_artifact(&bins.join("ompo"), MEASURED_UNDERIVED_TOKEN);

    let output = Command::new(built_installer())
        .arg("--check")
        .arg("--bin-dir")
        .arg(&bins)
        .env("GIT_DIR", repo.join(".git"))
        .env("GIT_WORK_TREE", &repo)
        .output()
        .expect("run the real installer binary");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    println!("OBSERVED installer --check exit={:?}\n{text}", output.status.code());

    assert_eq!(
        output.status.code(),
        Some(3),
        "an unstamped-only report is an instrument error, not degraded:\n{text}"
    );
    assert!(
        text.contains("INSTALLER IDENTITY UNSTAMPED: 1/"),
        "the unstamped count must be reported:\n{text}"
    );
    assert!(
        !text.contains("INSTALLER IDENTITY DRIFT"),
        "nothing disagreed with HEAD, so nothing may be called drift:\n{text}"
    );
    assert!(
        !text.contains("INSTALLER IDENTITY OK"),
        "an unmeasured identity is never an OK:\n{text}"
    );
}

// ── B07 / T09 / T10 / T22: L0 DURABILITY ──────────────────────────────────────
//
// MEASURED 2026-09-11 at HEAD 2774c6a: `publish_atomic` was a bare
// `std::fs::rename` and the strings fsync, sync_all, sync_data and F_FULLFSYNC
// did not appear ANYWHERE under crates/installer. A rename whose parent
// directory is never synchronized can be lost on power failure with the OLD
// name still resolving, which is precisely the `install(1)` escape the L0
// contract's retained cross-attack names: it syncs the destination descriptor,
// never its parent. Every leg below is nonzero and the injected-failure legs
// are deterministic on any platform.

/// Stage a complete artifact the publication seam will accept.
fn staged_payload(dir: &Path, payload: &[u8]) -> PathBuf {
    stage_artifact_stream(dir, "installer", payload, payload.len() as u64).expect("stage payload")
}

#[test]
fn durability_publication_requires_every_platform_applicable_step() {
    let dir = TempDir::new("durability-known-good");
    let payload = b"durable-publication-bytes";
    let dest = dir.path().join("installer");
    let staged = staged_payload(dir.path(), payload);
    let mut metric = DurabilityMetric::default();

    let record = publish_atomic_durable(&staged, &dest, payload.len() as u64, &mut metric, None)
        .expect("a complete staged artifact must publish durably");

    assert!(record.file_synced, "file sync was not performed: {record:?}");
    assert!(record.renamed, "rename was not performed: {record:?}");
    assert!(
        record.parent_synced,
        "parent directory sync was not performed: {record:?}"
    );
    // Darwin is the only platform with F_FULLFSYNC. A Linux lane must say
    // UNMEASURED with its platform, never invent a pass or a failure.
    match record.fullfsync {
        FullFsyncObservation::Applied => {
            assert!(
                cfg!(target_vendor = "apple"),
                "only a Darwin host may report F_FULLFSYNC APPLIED: {record:?}"
            );
        }
        FullFsyncObservation::Unmeasured { platform, reason } => {
            assert!(
                !cfg!(target_vendor = "apple"),
                "a Darwin host must APPLY F_FULLFSYNC, not report it UNMEASURED: {record:?}"
            );
            assert_eq!(platform, std::env::consts::OS);
            assert!(!reason.is_empty(), "UNMEASURED must carry its reason");
            let text = FullFsyncObservation::Unmeasured { platform, reason }.to_string();
            assert!(text.starts_with("UNMEASURED platform="), "{text}");
        }
    }

    assert_eq!(fs::read(&dest).expect("published destination"), payload);
    assert!(!staged.exists(), "staged temp must be consumed by the rename");
    assert_eq!(metric.atomic_rename_attempts, 1, "{metric:?}");
    assert_eq!(metric.parent_fsync_successes, 1, "{metric:?}");
    assert_eq!(metric.coverage(), Some(1.0), "{metric:?}");
    assert_eq!(metric.verdict(), MetricVerdict::Green, "{metric:?}");
    assert!(metric.invariant_holds(), "{metric:?}");
}

#[test]
fn parent_directory_fsync_is_required() {
    let dir = TempDir::new("durability-parent-fsync");
    let payload = b"parent-fsync-required-bytes";
    let dest = dir.path().join("installer");
    let staged = staged_payload(dir.path(), payload);
    let mut metric = DurabilityMetric::default();

    let error = publish_atomic_durable(
        &staged,
        &dest,
        payload.len() as u64,
        &mut metric,
        Some(DurabilityStage::ParentSync),
    )
    .expect_err("a publication whose parent sync fails is not durable and must refuse");

    match error {
        InstallError::DurabilityRefused { stage, ref detail } => {
            assert_eq!(stage, DurabilityStage::ParentSync, "wrong stage: {error}");
            assert!(
                detail.contains(&dir.path().display().to_string()),
                "the refusal must name the parent directory: {detail}"
            );
        }
        other => panic!("expected DurabilityRefused at parent_sync, got {other:?}"),
    }
    let text = error.to_string();
    assert!(
        text.starts_with("L0_DURABILITY_REFUSED stage=parent_sync"),
        "the reason code must be typed and name its stage: {text}"
    );

    // FAIL-CLOSED, and the metric is what proves it: the rename was attempted
    // (the file is published) but no success may be claimed for it.
    assert_eq!(metric.atomic_rename_attempts, 1, "{metric:?}");
    assert_eq!(metric.parent_fsync_successes, 0, "{metric:?}");
    assert_eq!(metric.verdict(), MetricVerdict::Red, "{metric:?}");

    // A missing parent sync is not a licence to skip the earlier steps: the
    // same call with no injection is the known-good control.
    let clean = TempDir::new("durability-parent-fsync-control");
    let clean_dest = clean.path().join("installer");
    let clean_staged = staged_payload(clean.path(), payload);
    let mut clean_metric = DurabilityMetric::default();
    publish_atomic_durable(
        &clean_staged,
        &clean_dest,
        payload.len() as u64,
        &mut clean_metric,
        None,
    )
    .expect("the same publication with no injected failure must pass");
    assert_eq!(clean_metric.verdict(), MetricVerdict::Green, "{clean_metric:?}");
}

#[test]
fn fullfsync_failure_is_restrictive() {
    let dir = TempDir::new("durability-fullfsync");
    let payload = b"fullfsync-restrictive-bytes";
    let dest = dir.path().join("installer");
    let staged = staged_payload(dir.path(), payload);
    let mut metric = DurabilityMetric::default();

    let error = publish_atomic_durable(
        &staged,
        &dest,
        payload.len() as u64,
        &mut metric,
        Some(DurabilityStage::FullFsync),
    )
    .expect_err("a failed full synchronization must refuse, never downgrade to a pass");

    match error {
        InstallError::DurabilityRefused { stage, ref detail } => {
            assert_eq!(stage, DurabilityStage::FullFsync, "wrong stage: {error}");
            assert!(
                detail.contains(&format!("platform={}", std::env::consts::OS)),
                "the refusal must name the platform it was observed on: {detail}"
            );
        }
        other => panic!("expected DurabilityRefused at fullfsync, got {other:?}"),
    }
    assert!(
        error.to_string().starts_with("L0_DURABILITY_REFUSED stage=fullfsync"),
        "{error}"
    );

    // RESTRICTIVE means nothing was published and nothing was counted: the
    // full-sync stage runs BEFORE the rename, so no destination may exist.
    assert!(
        !dest.exists(),
        "a refused full synchronization published the destination anyway"
    );
    assert_eq!(metric.atomic_rename_attempts, 0, "{metric:?}");
    assert_eq!(metric.parent_fsync_successes, 0, "{metric:?}");
    assert_eq!(
        metric.verdict(),
        MetricVerdict::Unmeasured,
        "no attempt means UNMEASURED, never a passing zero: {metric:?}"
    );

    // The file-sync stage is a DISTINCT stage with its own name, so a caller
    // can tell which of the two synchronization steps refused.
    let file_sync_dir = TempDir::new("durability-file-sync");
    let file_sync_dest = file_sync_dir.path().join("installer");
    let file_sync_staged = staged_payload(file_sync_dir.path(), payload);
    let mut file_sync_metric = DurabilityMetric::default();
    let file_sync_error = publish_atomic_durable(
        &file_sync_staged,
        &file_sync_dest,
        payload.len() as u64,
        &mut file_sync_metric,
        Some(DurabilityStage::FileSync),
    )
    .expect_err("a failed file sync must refuse");
    assert!(
        file_sync_error
            .to_string()
            .starts_with("L0_DURABILITY_REFUSED stage=file_sync"),
        "{file_sync_error}"
    );
    assert!(!file_sync_dest.exists(), "a refused file sync published anyway");

    // And the rename stage too, which is the third distinct name.
    let rename_dir = TempDir::new("durability-rename");
    let rename_dest = rename_dir.path().join("installer");
    let rename_staged = staged_payload(rename_dir.path(), payload);
    let mut rename_metric = DurabilityMetric::default();
    let rename_error = publish_atomic_durable(
        &rename_staged,
        &rename_dest,
        payload.len() as u64,
        &mut rename_metric,
        Some(DurabilityStage::Rename),
    )
    .expect_err("a failed rename must refuse");
    assert!(
        rename_error
            .to_string()
            .starts_with("L0_DURABILITY_REFUSED stage=rename"),
        "{rename_error}"
    );
    assert!(!rename_dest.exists(), "a refused rename published anyway");
    assert_eq!(
        rename_metric.atomic_rename_attempts, 1,
        "a refused rename is still an ATTEMPT: {rename_metric:?}"
    );
    assert_eq!(rename_metric.verdict(), MetricVerdict::Red, "{rename_metric:?}");
}

#[test]
fn durability_metric_counts_missing_parent_sync() {
    // ANTI-VACUITY FIRST: an empty metric is UNMEASURED, never a passing 1.0
    // and never a RED zero. A zero denominator that renders as GREEN is how a
    // durability metric reports success for installs that never happened.
    let empty = DurabilityMetric::default();
    assert_eq!(empty.coverage(), None, "{empty:?}");
    assert_eq!(empty.verdict(), MetricVerdict::Unmeasured, "{empty:?}");
    assert_eq!(MetricVerdict::Unmeasured.to_string(), "UNMEASURED");

    // One rename attempt recorded with NO parent sync: coverage is below 1.0
    // and the verdict is RED. This is the exact known-bad the row names.
    let dir = TempDir::new("durability-metric-missing-parent");
    let payload = b"metric-missing-parent-bytes";
    let dest = dir.path().join("installer");
    let staged = staged_payload(dir.path(), payload);
    let mut metric = DurabilityMetric::default();
    publish_atomic_durable(
        &staged,
        &dest,
        payload.len() as u64,
        &mut metric,
        Some(DurabilityStage::ParentSync),
    )
    .expect_err("parent sync failure must refuse");

    let coverage = metric.coverage().expect("an attempted rename is measured");
    assert!(
        coverage < 1.0,
        "a rename without parent sync must not report full coverage: {coverage}"
    );
    assert_eq!(coverage, 0.0, "{metric:?}");
    assert_eq!(metric.verdict(), MetricVerdict::Red, "{metric:?}");
    assert_eq!(MetricVerdict::Red.to_string(), "RED");
    assert!(metric.invariant_holds(), "{metric:?}");

    // KNOWN-GOOD CONTROL so the metric is not RED for everything: a second
    // publication that DOES sync its parent lifts coverage off the floor
    // without reaching 1.0, and a third leaves it at 1.0 only when every
    // attempt synced.
    let good = TempDir::new("durability-metric-good");
    let good_dest = good.path().join("installer");
    let good_staged = staged_payload(good.path(), payload);
    publish_atomic_durable(
        &good_staged,
        &good_dest,
        payload.len() as u64,
        &mut metric,
        None,
    )
    .expect("clean publication");
    assert_eq!(metric.atomic_rename_attempts, 2, "{metric:?}");
    assert_eq!(metric.parent_fsync_successes, 1, "{metric:?}");
    assert_eq!(metric.coverage(), Some(0.5), "{metric:?}");
    assert_eq!(
        metric.verdict(),
        MetricVerdict::Red,
        "one missing parent sync in two attempts is still RED: {metric:?}"
    );

    let all_good = DurabilityMetric {
        atomic_rename_attempts: 2,
        parent_fsync_successes: 2,
    };
    assert_eq!(all_good.coverage(), Some(1.0), "{all_good:?}");
    assert_eq!(all_good.verdict(), MetricVerdict::Green, "{all_good:?}");
    assert_eq!(MetricVerdict::Green.to_string(), "GREEN");

    // The invariant that makes the ratio attributable at all.
    let impossible = DurabilityMetric {
        atomic_rename_attempts: 1,
        parent_fsync_successes: 2,
    };
    assert!(
        !impossible.invariant_holds(),
        "more parent syncs than rename attempts must violate the invariant"
    );
}

#[test]
fn production_install_path_routes_through_the_durability_seam() {
    // WIRING PROOF. `installer main` -> `run_install` -> `install_binary` ->
    // `replace_atomic` -> the durability seam. A unit test on the helper cannot
    // see whether production calls it, so this drives the real install entry
    // point and reads the metric it accumulated. Deleting the production
    // parent-sync call turns this RED while the helper tests stay green.
    let target = TempDir::new("durability-wiring");
    let mut metric = DurabilityMetric::default();
    let check = install_binary_with_durability(
        &built_installer(),
        target.path(),
        &identity_head(),
        &RepoOwnership::ThisRepo,
        &mut metric,
        None,
    )
    .expect("the real installer must publish its own verified binary durably");
    assert!(check.consistent, "published identity did not read back: {check:?}");
    assert!(
        target.path().join("installer").is_file(),
        "production install did not publish the artifact"
    );
    assert_eq!(
        metric.atomic_rename_attempts, 1,
        "production publish did not reach the rename counter: {metric:?}"
    );
    assert_eq!(
        metric.parent_fsync_successes, 1,
        "production publish renamed without syncing the parent directory: {metric:?}"
    );
    assert_eq!(metric.verdict(), MetricVerdict::Green, "{metric:?}");

    // And the production path refuses, restrictively and typed, when a
    // durability stage fails — no success-shaped return for a publication that
    // is not durable.
    let refused = TempDir::new("durability-wiring-refused");
    let mut refused_metric = DurabilityMetric::default();
    let error = install_binary_with_durability(
        &built_installer(),
        refused.path(),
        &identity_head(),
        &RepoOwnership::ThisRepo,
        &mut refused_metric,
        Some(DurabilityStage::ParentSync),
    )
    .expect_err("production install must refuse a non-durable publication");
    assert!(
        error
            .to_string()
            .starts_with("L0_DURABILITY_REFUSED stage=parent_sync"),
        "{error}"
    );
    assert_eq!(refused_metric.atomic_rename_attempts, 1, "{refused_metric:?}");
    assert_eq!(refused_metric.parent_fsync_successes, 0, "{refused_metric:?}");
    assert_eq!(refused_metric.verdict(), MetricVerdict::Red, "{refused_metric:?}");
}

// ── B05 / T05 / T06: L0 SIGSTORE COSIGN POLICY ────────────────────────────────
//
// MEASURED 2026-09-11 at HEAD 2774c6a: the strings cosign, sigstore,
// L0_SIGSTORE_REFUSED and CVE-2026-22703 appeared NOWHERE under
// crates/installer. The existing signature coverage is minisign, which is a
// DIFFERENT mechanism — a minisign leg cannot evidence a cosign floor, and the
// three sigstore beads' named selectors ran zero tests and exited 0.

fn expected_keyless() -> SigstoreTrust {
    SigstoreTrust::CertificateIdentity {
        identity: "release@omp-orchestrator.invalid".to_owned(),
        issuer: "https://token.actions.githubusercontent.com".to_owned(),
    }
}

#[test]
fn sigstore_policy_allows_supported_cosign_and_expected_identity() {
    let expected = expected_keyless();
    let verdict = verify_sigstore_policy(Some(COSIGN_CVE_FLOOR), &expected, &expected, true)
        .expect("cosign exactly AT the floor with the expected identity must pass");
    assert_eq!(verdict.floor.to_string(), COSIGN_CVE_FLOOR);
    assert_eq!(verdict.cosign_version, verdict.floor, "{verdict:?}");
    assert_eq!(verdict.trust, expected, "{verdict:?}");

    // ABOVE the floor passes too, so the comparison is an ordering and not an
    // equality that would refuse every future cosign release.
    let above = verify_sigstore_policy(Some("9.99.99"), &expected, &expected, true)
        .expect("a cosign release above the floor must pass");
    assert!(above.cosign_version > above.floor, "{above:?}");

    // A `v` prefix and a pre-release suffix of the fixing release both parse.
    verify_sigstore_policy(Some("v9.0.0"), &expected, &expected, true).expect("v prefix parses");
    verify_sigstore_policy(Some("9.0.0-rc.1"), &expected, &expected, true)
        .expect("pre-release of an above-floor release parses");

    // LOCAL-KEY policy is the contract's other trust mode and it also passes.
    let key = SigstoreTrust::LocalKey {
        key_id: "omp-release-2026".to_owned(),
    };
    let key_verdict = verify_sigstore_policy(Some("9.0.0"), &key, &key, true)
        .expect("a matching local key must pass");
    assert_eq!(key_verdict.trust, key, "{key_verdict:?}");
}

#[test]
fn sigstore_policy_refuses_absent_unparseable_and_invalid_bundle() {
    let expected = expected_keyless();

    // ABSENT cosign is a refusal, never a skip: a verifier that is not there
    // has not verified anything.
    let error = verify_sigstore_policy(None, &expected, &expected, true)
        .expect_err("absent cosign must refuse");
    let text = error.to_string();
    assert!(text.starts_with("L0_SIGSTORE_REFUSED"), "{text}");
    assert!(text.contains(COSIGN_CVE_ID), "the refusal must name its reason: {text}");

    // A version this code cannot ORDER is never approved.
    for raw in ["", "two.four.one", "2.4", "2.4.1.5", "not-a-version"] {
        let error = verify_sigstore_policy(Some(raw), &expected, &expected, true)
            .expect_err("an unorderable cosign version must refuse");
        let text = error.to_string();
        assert!(
            text.starts_with("L0_SIGSTORE_REFUSED") && text.contains("unparseable"),
            "raw {raw:?} produced {text}"
        );
        assert_eq!(parse_cosign_version(raw), None, "raw {raw:?} must not parse");
    }

    // An INVALID bundle under an allowed cosign and the expected identity is
    // still a refusal — the version floor and the identity are preconditions
    // for trusting the signature, not substitutes for it.
    let error = verify_sigstore_policy(Some("9.0.0"), &expected, &expected, false)
        .expect_err("an invalid bundle must refuse");
    assert!(
        error.to_string().starts_with("L0_SIGSTORE_REFUSED"),
        "{error}"
    );
}

#[test]
fn cosign_below_cve_floor_refuses() {
    let expected = expected_keyless();
    let floor = parse_cosign_version(COSIGN_CVE_FLOOR).expect("the floor constant must parse");

    // One patch below the floor: the smallest possible below-floor version, so
    // the test cannot pass by being far away from the boundary.
    let just_below = format!("{}.{}.{}", floor.major, floor.minor, floor.patch - 1);
    let error = verify_sigstore_policy(Some(&just_below), &expected, &expected, true)
        .expect_err("cosign below the CVE floor must refuse");
    let text = error.to_string();
    assert!(
        text.starts_with("L0_SIGSTORE_REFUSED"),
        "the refusal must be typed: {text}"
    );
    // NAMING THE VERSION is the acceptance's own clause: the presented version,
    // the floor, and the advisory all appear, so the refusal is actionable
    // rather than a bare code.
    assert!(text.contains(&just_below), "must name the presented version: {text}");
    assert!(text.contains(COSIGN_CVE_FLOOR), "must name the floor: {text}");
    assert!(text.contains(COSIGN_CVE_ID), "must name the advisory: {text}");
    assert!(matches!(error, InstallError::SigstoreRefused { .. }), "{error:?}");

    // Lower still refuses, and a below-floor PRE-RELEASE refuses too — the
    // suffix must not become an escape hatch under the floor.
    for raw in ["0.0.1", "1.99.99", "2.0.0", "2.4.0-rc.9"] {
        if parse_cosign_version(raw).expect("fixture parses") >= floor {
            continue;
        }
        let error = verify_sigstore_policy(Some(raw), &expected, &expected, true)
            .expect_err("every below-floor version must refuse");
        assert!(
            error.to_string().contains(COSIGN_CVE_ID),
            "raw {raw} produced {error}"
        );
    }

    // KNOWN-GOOD BOUNDARY CONTROL so the gate is not over-strict: the floor
    // itself is ALLOWED. A test that refused everything would satisfy the
    // assertions above while breaking every install.
    verify_sigstore_policy(Some(COSIGN_CVE_FLOOR), &expected, &expected, true)
        .expect("cosign exactly at the floor is allowed, not refused");
}

#[test]
fn sigstore_identity_mismatch_refuses() {
    let expected = expected_keyless();

    // A VALID signature from the WRONG identity is not trusted. `bundle_valid`
    // is true throughout: this test is about identity, and passing a false
    // bundle here would let the refusal come from the wrong clause.
    let wrong_identity = SigstoreTrust::CertificateIdentity {
        identity: "attacker@elsewhere.invalid".to_owned(),
        issuer: "https://token.actions.githubusercontent.com".to_owned(),
    };
    let error = verify_sigstore_policy(Some("9.0.0"), &wrong_identity, &expected, true)
        .expect_err("a foreign certificate identity must refuse");
    let text = error.to_string();
    assert!(text.starts_with("L0_SIGSTORE_REFUSED"), "{text}");
    assert!(
        text.contains("attacker@elsewhere.invalid")
            && text.contains("release@omp-orchestrator.invalid"),
        "the refusal must name BOTH the presented and the required identity: {text}"
    );

    // THE ISSUER IS PART OF THE IDENTITY. The same identity string minted by a
    // different issuer is a different principal, and this is the leg a
    // field-by-field comparison of only the identity would miss.
    let wrong_issuer = SigstoreTrust::CertificateIdentity {
        identity: "release@omp-orchestrator.invalid".to_owned(),
        issuer: "https://attacker-oidc.invalid".to_owned(),
    };
    let error = verify_sigstore_policy(Some("9.0.0"), &wrong_issuer, &expected, true)
        .expect_err("the right identity from the wrong issuer must refuse");
    assert!(
        error.to_string().contains("https://attacker-oidc.invalid"),
        "{error}"
    );

    // TRUST MODE is not a string comparison: a local key cannot satisfy a
    // keyless policy by presenting a matching-looking value.
    let local = SigstoreTrust::LocalKey {
        key_id: "release@omp-orchestrator.invalid".to_owned(),
    };
    let error = verify_sigstore_policy(Some("9.0.0"), &local, &expected, true)
        .expect_err("a local key cannot satisfy a keyless identity policy");
    let text = error.to_string();
    assert!(
        text.contains("trust mode mismatch")
            && text.contains("local-key")
            && text.contains("keyless"),
        "{text}"
    );

    // And a mismatched local key under a local-key policy refuses by key id.
    let want_key = SigstoreTrust::LocalKey {
        key_id: "omp-release-2026".to_owned(),
    };
    let error = verify_sigstore_policy(Some("9.0.0"), &local, &want_key, true)
        .expect_err("a foreign local key must refuse");
    assert!(
        error.to_string().contains("local key mismatch"),
        "{error}"
    );

    // KNOWN-GOOD CONTROL: the expected identity against itself passes, so the
    // identity gate is not refusing everything.
    verify_sigstore_policy(Some("9.0.0"), &expected, &expected, true)
        .expect("the expected identity must pass");
}

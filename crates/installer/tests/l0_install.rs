use installer::{
    check_build_fence, classify_agent_scan, classify_restart_postcondition, git_head,
    git_rev_parse_short, install_binary, merge_hooks, publish_atomic, refuse_path_collisions,
    resolve_platform_triple, resolve_repo_ownership, seal_install_report, stage_artifact_stream,
    verify_identity, verify_minisign_policy, verify_sha256, verify_sha256_before_install, AgentOutcome, HookWrite, IdentityCheck, InstallError, Sha256FailureClass,
    RepoOwnership, RestartPostcondition,
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

struct CutReader {
    data: &'static [u8],
    pos: usize,
    cut: usize,
}

impl std::io::Read for CutReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.cut {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "stream interrupted",
            ));
        }
        let n = (self.cut - self.pos).min(buf.len()).min(self.data.len() - self.pos);
        buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

#[test]
fn artifact_staging_interrupt_leaves_no_publishable_temp() {
    let dir = TempDir::new("artifact-staging-interrupt");
    let data: &'static [u8] = b"0123456789abcdef";
    let error = stage_artifact_stream(
        dir.path(),
        "installer",
        CutReader {
            data,
            pos: 0,
            cut: 4,
        },
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

const B03_PAYLOAD: &[u8] = b"B03 fixed buffer SHA-256 payload\n";
const B03_DIGEST: &str = "bdd2a7291457c6a5e371324772061f751ba7774d8d7057895f5d5ea8daa773f1";
const B03_LARGE_DIGEST: &str = "c4707d7fddcaeaac63ce0019ad6bc0a74fb5c309f62093bfd53d261d4c15cdd6";

#[test]
fn sha256_known_good_returns_lowercase_digest() {
    let dir = TempDir::new("sha256-known-good");
    let source = dir.path().join("artifact");
    fs::write(&source, B03_PAYLOAD).expect("write artifact");
    let expected = B03_DIGEST.to_ascii_uppercase();
    let actual = verify_sha256(&source, Some(&expected)).expect("known digest must pass");
    assert_eq!(actual, B03_DIGEST);
}

#[test]
fn sha256_streams_across_fixed_buffer_boundary() {
    let dir = TempDir::new("sha256-buffer-boundary");
    let source = dir.path().join("artifact");
    let mut payload = vec![b'A'; 16_385];
    payload[8_192] = b'B';
    fs::write(&source, payload).expect("write large artifact");
    assert_eq!(
        verify_sha256(&source, Some(B03_LARGE_DIGEST)).expect("large digest must pass"),
        B03_LARGE_DIGEST
    );
}

#[test]
fn sha256_one_byte_mutation_refuses_before_destination_write() {
    let dir = TempDir::new("sha256-mutation");
    let source = dir.path().join("artifact");
    let destination = dir.path().join("installed");
    fs::write(&source, b"B03 fixed buffer SHA-256 payloae\n").expect("write mutated artifact");
    let error = verify_sha256(&source, Some(B03_DIGEST)).expect_err("mutation must refuse");
    let message = error.to_string();
    match error {
        InstallError::Sha256Refused { class, detail } => {
            assert_eq!(class, Sha256FailureClass::Mismatch);
            assert_eq!(
                detail,
                format!("digest mismatch path={} expected={} actual=59bad0e5db3924bd2147190a7cc8f2b9c00a0c16a356c4cb778edac262f55b84", source.display(), B03_DIGEST)
            );
        }
        other => panic!("expected SHA-256 mismatch, got {other:?}"),
    }
    assert_eq!(
        message,
        format!("L0_SHA256_REFUSED: digest mismatch path={} expected={} actual=59bad0e5db3924bd2147190a7cc8f2b9c00a0c16a356c4cb778edac262f55b84", source.display(), B03_DIGEST)
    );
    assert!(!destination.exists(), "digest refusal must not write destination");
}

#[test]
fn sha256_expected_digest_validation_is_typed_and_exact() {
    let dir = TempDir::new("sha256-expected-validation");
    let source = dir.path().join("artifact");
    fs::write(&source, B03_PAYLOAD).expect("write artifact");
    let cases = [
        (None, Sha256FailureClass::MissingExpected, "L0_SHA256_REFUSED: missing expected digest"),
        (Some("  "), Sha256FailureClass::EmptyExpected, "L0_SHA256_REFUSED: empty expected digest"),
        (
            Some("abc"),
            Sha256FailureClass::MalformedExpected,
            "L0_SHA256_REFUSED: malformed expected digest length=3; expected 64 hexadecimal characters",
        ),
    ];
    for (expected, expected_class, expected_message) in cases {
        let error = verify_sha256(&source, expected).expect_err("invalid digest must refuse");
        let message = error.to_string();
        match error {
            InstallError::Sha256Refused { class, .. } => assert_eq!(class, expected_class),
            other => panic!("expected typed SHA refusal, got {other:?}"),
        }
        assert_eq!(message, expected_message);
    }
}

#[test]
fn sha256_missing_source_refuses_with_distinct_class() {
    let dir = TempDir::new("sha256-missing-source");
    let source = dir.path().join("missing");
    let error = verify_sha256(&source, Some(B03_DIGEST)).expect_err("missing source must refuse");
    let message = error.to_string();
    match error {
        InstallError::Sha256Refused { class, detail } => {
            assert_eq!(class, Sha256FailureClass::SourceMissing);
            assert_eq!(detail, format!("source missing path={}", source.display()));
        }
        other => panic!("expected missing-source refusal, got {other:?}"),
    }
    assert_eq!(
        message,
        format!("L0_SHA256_REFUSED: source missing path={}", source.display())
    );
}

#[test]
fn sha256_read_error_refuses_with_distinct_class() {
    let dir = TempDir::new("sha256-read-error");
    let error = verify_sha256(dir.path(), Some(B03_DIGEST)).expect_err("directory read must refuse");
    let message = error.to_string();
    match error {
        InstallError::Sha256Refused { class, detail } => {
            assert_eq!(class, Sha256FailureClass::ReadFailed);
            assert!(detail.starts_with(&format!("read failure path={}: ", dir.path().display())));
        }
        other => panic!("expected read-failure refusal, got {other:?}"),
    }
    assert!(message.starts_with("L0_SHA256_REFUSED: read failure"));
}

#[test]
fn sha256_verification_seam_blocks_mismatched_install_action() {
    let dir = TempDir::new("sha256-seam-mismatch");
    let source = dir.path().join("artifact");
    let destination = dir.path().join("installed");
    fs::write(&source, b"B03 fixed buffer SHA-256 payloae\n").expect("write mutated artifact");
    let mut action_called = false;
    let error = verify_sha256_before_install(&source, Some(B03_DIGEST), || {
        action_called = true;
        fs::write(&destination, b"published")
            .map_err(|error| InstallError::IoError {
                path: destination.display().to_string(),
                detail: error.to_string(),
            })
    })
    .expect_err("mismatched digest must block install action");
    assert!(matches!(
        error,
        InstallError::Sha256Refused {
            class: Sha256FailureClass::Mismatch,
            ..
        }
    ));
    assert!(!action_called, "install action ran after digest refusal");
    assert!(!destination.exists(), "mismatched digest wrote destination");
}

#[test]
fn sha256_verification_seam_allows_matching_install_action() {
    let dir = TempDir::new("sha256-seam-match");
    let source = dir.path().join("artifact");
    let destination = dir.path().join("installed");
    fs::write(&source, B03_PAYLOAD).expect("write artifact");
    let mut action_called = false;
    verify_sha256_before_install(&source, Some(B03_DIGEST), || {
        action_called = true;
        fs::write(&destination, b"published")
            .map_err(|error| InstallError::IoError {
                path: destination.display().to_string(),
                detail: error.to_string(),
            })
    })
    .expect("matching digest must allow install action");
    assert!(action_called, "matching digest did not reach install action");
    assert_eq!(fs::read(&destination).expect("published destination"), b"published");
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
    assert!(backups[0].file_name().unwrap().to_string_lossy().contains(".bak."));
    assert_eq!(fs::read(&backups[0]).expect("backup"), b"pre-merge");
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


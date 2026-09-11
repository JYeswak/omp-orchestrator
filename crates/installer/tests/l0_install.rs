use installer::{
    check_build_fence, classify_agent_scan, classify_restart_postcondition, git_head,
    git_rev_parse_short, install_binary, merge_hooks, publish_atomic, refuse_path_collisions,
    resolve_platform_triple, resolve_repo_ownership, seal_install_report, stage_artifact_stream,
    probe_build_id_string, verify_identity, verify_minisign_policy, AgentOutcome, HookWrite, IdentityCheck, InstallError,
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

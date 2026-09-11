#![forbid(unsafe_code)]

//! L5 inception durability laws, observed through the PRODUCTION public seam
//! (`ompo_start::inception::write_inception_trusted`) rather than asserted
//! against source text.
//!
//! WHY NOT A SOURCE-TEXT ASSERTION: a leg that greps for `sync_all` passes on a
//! file where the call was MOVED AFTER the rename, which is the exact defect the
//! ordering laws exist to prevent. Both legs below inject a real filesystem
//! failure at the staging seam and read the typed outcome plus the on-disk state.
//!
//! WHY NOT A PERMISSION INJECTION: measured 2026-09-11, the remote build lane
//! runs as ROOT, so `chmod 0o500` on the output directory leaves it writable and
//! a mode-based known-bad cannot be expressed there at all. The injection used
//! here is `ENAMETOOLONG`: production stages at `.<dest-name>.<pid>.<nanos>.tmp`
//! beside the destination, so a destination name near `NAME_MAX` makes the
//! STAGING name exceed it while the destination name itself stays legal. Root
//! does not bypass `NAME_MAX`.
//!
//! Laws and their beads:
//!   v809  inception_rename_same_dir        staging is a temp in the SAME directory
//!   p0jn  crash_between_write_and_rename   a failed publication truncates nothing
//!
//! NOT COVERED HERE, deliberately, and reported BLOCKED rather than faked:
//!   q7jz  fsync of the file fd BEFORE the rename. No failure between the temp
//!         write and the rename is injectable through the public API, so the
//!         ORDER of that call is not observable at this seam.
//!   u7qq  parent-directory fsync failure. The only injection is denying READ on
//!         the parent, which root ignores; on this lane it is inexpressible.
//! Both need an injectable sync seam or an in-module unit test.

use ompo_start::inception::{required_control_files, write_inception_trusted, InceptionError};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A destination name long enough that production's staging name cannot fit in
/// `NAME_MAX` (255 on ext4/overlayfs/tmpfs/APFS) while the destination name can:
/// staging adds a leading dot, the pid, a nanosecond stamp and `.tmp`.
const LONG_STEM_LEN: usize = 240;

fn run_git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .expect("git fixture command spawn");
    assert!(
        output.status.success(),
        "git fixture command {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A stamped-enough repository: every required control file present, a real git
/// commit so identity resolves, and an AGENTS.md the trusted path accepts.
fn fixture() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("fixture directory");
    let root = directory.path().to_owned();
    let controls = required_control_files();
    assert!(
        !controls.is_empty(),
        "ANTI-VACUITY: required_control_files() is empty, the fixture would assert nothing"
    );
    for relative in controls {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture parent");
        }
        fs::write(&path, "fixture\n").expect("fixture control file");
    }
    run_git(&root, &["init", "-q"]);
    run_git(&root, &["add", "."]);
    run_git(
        &root,
        &[
            "-c",
            "user.name=ompo-start-test",
            "-c",
            "user.email=ompo-start-test@example.invalid",
            "commit",
            "-qm",
            "fixture",
        ],
    );
    let out_dir = root.join(".omp-orchestrator");
    fs::create_dir_all(&out_dir).expect("output directory");
    (directory, out_dir)
}

/// Every staging file left behind in `dir` for destination `dest_name`.
fn staging_residue(dir: &Path, dest_name: &str) -> Vec<PathBuf> {
    let prefix = format!(".{dest_name}.");
    fs::read_dir(dir)
        .expect("read output directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".tmp"))
        })
        .collect()
}

/// The destination whose staging name cannot exist, plus a proof that the
/// injection is EFFECTIVE on this filesystem: a staging-shaped name must be
/// unusable while the destination name is usable. An injection this host ignores
/// is a TYPED FAILURE here, never a silent pass.
fn long_destination(out_dir: &Path) -> (PathBuf, String) {
    let dest_name = format!("{}.json", "i".repeat(LONG_STEM_LEN));
    let destination = out_dir.join(&dest_name);

    // The probe must mirror production's staging shape, INCLUDING its length:
    // `.<dest>.<pid>.<19-digit nanos>.tmp`. A shorter probe fits inside NAME_MAX
    // while the real staging name does not, which is exactly how this leg
    // mis-fired on its first remote run.
    let staging_probe = out_dir.join(format!(
        ".{dest_name}.{}.{}.tmp",
        std::process::id(),
        1_000_000_000_000_000_000_u64
    ));
    assert!(
        fs::write(&staging_probe, b"probe").is_err(),
        "UNMEASURED_NAME_LIMIT_INJECTION_INEFFECTIVE: {} accepted a staging-shaped name of {} bytes, \
         so this host does not enforce NAME_MAX and the known-bad cannot be expressed",
        out_dir.display(),
        staging_probe
            .file_name()
            .and_then(|name| name.to_str())
            .map_or(0, str::len)
    );
    let destination_probe = fs::write(&destination, b"probe");
    assert!(
        destination_probe.is_ok(),
        "the destination name itself must be legal, otherwise the leg would prove only that \
         the destination is unwritable: {destination_probe:?}"
    );
    fs::remove_file(&destination).expect("clear destination probe");
    (destination, dest_name)
}

/// KNOWN-GOOD, mandatory: an uninjected write publishes a complete artifact and
/// leaves NO staging residue. Without this leg the two injected legs would ship
/// an over-strict gate that nothing has to satisfy.
#[test]
fn inception_publishes_and_leaves_no_staging_residue() {
    let (directory, out_dir) = fixture();
    let output = out_dir.join("inception.json");
    write_inception_trusted(directory.path(), &output).expect("known-good write");
    let published = fs::read_to_string(&output).expect("published artifact");
    assert!(
        published.trim_start().starts_with('{') && published.contains("\"schema_version\""),
        "published artifact is not a complete JSON manifest"
    );
    assert_eq!(
        staging_residue(&out_dir, "inception.json"),
        Vec::<PathBuf>::new(),
        "a successful publication must consume its staging file"
    );
}

/// LAW v809 — staging happens in the SAME DIRECTORY as the destination, so
/// publication is a same-filesystem rename and never a cross-device copy.
///
/// OBSERVATION: the typed refusal carries THE PATH PRODUCTION TRIED TO STAGE AT.
/// That path is production's own choice of staging location, read back rather
/// than inferred from source.
#[test]
fn inception_rename_same_dir() {
    let (directory, out_dir) = fixture();
    let (destination, dest_name) = long_destination(&out_dir);

    let result = write_inception_trusted(directory.path(), &destination);
    let InceptionError::Write { path, detail } = result.expect_err("staging must refuse") else {
        panic!("expected a typed Write refusal naming the staging path");
    };

    let staged_parent = path.parent().expect("staging path has a parent");
    assert_eq!(
        staged_parent,
        out_dir.as_path(),
        "staging directory {} is not the destination directory {}; a cross-directory temp makes \
         publication a copy across filesystems, not an atomic rename (detail: {detail})",
        staged_parent.display(),
        out_dir.display()
    );
    let staged_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("staging file name");
    assert!(
        staged_name.starts_with(&format!(".{dest_name}.")) && staged_name.ends_with(".tmp"),
        "staging name {staged_name} is not the dotted per-process temp shape for {dest_name}"
    );
    // The destination-absence claim belongs to p0jn, NOT here: keeping it in both
    // legs would make one mutation redden two legs and neither would be
    // attributable. This leg asserts the staging LOCATION only.
}

/// LAW p0jn — a publication that fails before the rename truncates nothing: the
/// destination is absent rather than partially written, and no staging file is
/// abandoned in the output directory.
///
/// OBSERVATION: same injection, different reading. This leg reads the
/// DESTINATION and the DIRECTORY and never the error's path, so it is
/// independent of v809's staging-location claim: a mutation that stages in
/// `std::env::temp_dir()` reddens v809 and leaves this leg green.
#[test]
fn crash_between_write_and_rename() {
    let (directory, out_dir) = fixture();
    let (destination, dest_name) = long_destination(&out_dir);

    let result = write_inception_trusted(directory.path(), &destination);
    assert!(
        result.is_err(),
        "an unusable staging name must not report a successful publication"
    );
    assert!(
        !destination.exists(),
        "no truncated artifact: the destination must be absent after a failed publication"
    );
    assert_eq!(
        staging_residue(&out_dir, &dest_name),
        Vec::<PathBuf>::new(),
        "a failed publication must abandon no staging file in the output directory"
    );
}

#![forbid(unsafe_code)]

//! L2 observability METRIC target (zo6l): actions on a second run, and the
//! backups_written : files_mutated obligation.
//!
//! WHY A SEPARATE TARGET: `tests/l2_ecosystem.rs` is held by another worker in
//! this shared checkout, and a new file converts a file collision into nothing
//! at all. The fixture is duplicated deliberately rather than shared, so this
//! target cannot be reddened by an edit to theirs.
//!
//! WHY NOT A SEVENTH `METRICS.toml` ROW: `lifecycle_monitor::load_metrics`
//! refuses any row count other than `EXPECTED_METRIC_COUNT` (= 6), so a new row
//! would break all eleven of its callers. L2's row
//! (`MET-L2-INIT-READBACK-FAILURE-RATE`) already exists; what was missing was
//! an EMISSION carrying the counts and a verdict.

use ompo_start::inception::{initialize, list_backups, PROJECT_AGENTS_OWNERSHIP_STAMP};
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

fn run_git(repo: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    command.current_dir(repo).args(args);
    match subprocess_contract::bounded_output(&mut command, Duration::from_secs(10)) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => {}
        other => panic!("git fixture command failed: {other:?}"),
    }
}

fn repository_fixture() -> TempDir {
    let directory = tempfile::tempdir().expect("fixture directory");
    std::fs::create_dir(directory.path().join("docs")).expect("docs directory");
    for name in ["CLAUDE.md", "Cargo.toml", "README.md", "SCHEMAS.toml"] {
        std::fs::write(directory.path().join(name), b"fixture\n").expect("control file");
    }
    std::fs::write(
        directory.path().join("AGENTS.md"),
        format!("fixture {PROJECT_AGENTS_OWNERSHIP_STAMP}\n"),
    )
    .expect("stamped AGENTS.md");
    std::fs::write(directory.path().join("docs/decisions.jsonl"), b"{}\n")
        .expect("decision ledger");
    run_git(directory.path(), &["init", "-q"]);
    run_git(directory.path(), &["add", "."]);
    run_git(
        directory.path(),
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
    directory
}

/// THE ACCEPTANCE'S MAIN CLAUSE: two runs with identical repository, policy,
/// template and probe hashes. The second writes NOTHING, and it says so with a
/// verdict rather than leaving a reader to infer zero from silence.
#[test]
fn second_identical_run_mutates_nothing_and_emits_the_verdict() {
    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");

    let first = initialize(repository.path(), &output).expect("first init");
    let second = initialize(repository.path(), &output).expect("second init");

    assert_eq!(second.actions, 0, "identical hashes earn zero actions");
    assert_eq!(second.files_mutated, 0);
    assert_eq!(second.backups_written, 0, "a no-op must not snapshot");
    assert!(second.preexisting, "the artifact existed before the second run");
    assert_eq!(second.backup_ratio_verdict, "BACKUP_RATIO_OK_NO_MUTATION");
    // The counts are EMITTED on the mutating run too, so the metric has a
    // denominator on both passes rather than only when something changed.
    assert_eq!(first.files_mutated, 1);
}

/// THE ASYMMETRY, PINNED DELIBERATELY: a virgin repo mutates one file and has
/// NOTHING to snapshot, so the honest reading is 0:1, not 1:1. A flat 1:1
/// assertion would redden here and look like a defect in the writer.
#[test]
fn a_virgin_write_is_zero_to_one_and_is_not_a_ratio_violation() {
    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");

    let first = initialize(repository.path(), &output).expect("first init");

    assert!(!first.preexisting, "nothing existed before the first run");
    assert_eq!(first.files_mutated, 1);
    assert_eq!(first.backups_written, 0, "there was nothing to snapshot");
    assert_eq!(first.backup_ratio_verdict, "BACKUP_RATIO_OK_VIRGIN_0_TO_1");
    assert!(
        list_backups(&output).expect("backup listing").is_empty(),
        "a virgin write leaves no snapshot on disk either"
    );
}

/// KNOWN-BAD FROM THE ACCEPTANCE: change a hash between runs. EXPECT a NONZERO
/// action, never a green zero — and the 1:1 obligation binds here, because
/// pre-existing content was superseded.
#[test]
fn a_changed_hash_is_a_nonzero_action_with_a_one_to_one_backup() {
    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");

    initialize(repository.path(), &output).expect("first init");
    std::fs::write(&output, b"tampered-policy-hash\n").expect("change the hash between runs");

    let third = initialize(repository.path(), &output).expect("third init");

    assert_ne!(third.actions, 0, "a changed hash must not report a green zero");
    assert_eq!(third.files_mutated, 1);
    assert_eq!(
        third.backups_written, third.files_mutated,
        "superseding pre-existing content obliges exactly one snapshot"
    );
    assert!(third.preexisting);
    assert_eq!(third.backup_ratio_verdict, "BACKUP_RATIO_OK_1_TO_1");
    assert!(
        third.backup.is_some(),
        "the snapshot path must be reported, not merely counted"
    );
    assert_eq!(
        list_backups(&output).expect("backup listing").len(),
        1,
        "the counted snapshot exists on disk"
    );
}

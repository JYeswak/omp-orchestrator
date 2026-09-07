#![forbid(unsafe_code)]

//! Named L2 target for the S1 layer gate.
//!
//! This target exercises the real inception writer and readback contract with an isolated
//! repository fixture. It is invoked directly as Cargo's `--test l2_ecosystem` target.

use ompo_start::inception::{initialize, read_inception, InceptionError, SCHEMA_VERSION};
use tempfile::TempDir;

fn repository_fixture() -> TempDir {
    let directory = tempfile::tempdir().expect("fixture directory");
    std::fs::create_dir(directory.path().join(".git")).expect("git marker");
    std::fs::create_dir(directory.path().join("docs")).expect("docs directory");
    for name in ["AGENTS.md", "CLAUDE.md", "README.md", "Cargo.toml", "SCHEMAS.toml"] {
        std::fs::write(directory.path().join(name), b"fixture\n").expect("control file");
    }
    std::fs::write(directory.path().join("docs/decisions.jsonl"), b"{}\n")
        .expect("decision ledger");
    directory
}

#[test]
fn l2_named_target_initializes_and_reads_back_identity() {
    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");

    let first = initialize(repository.path(), &output).expect("first init");
    assert_eq!(first.actions, 1, "first init must write the artifact");
    assert_eq!(first.manifest.schema_version, SCHEMA_VERSION);
    assert_eq!(first.manifest.repo_identity.git_marker, "directory");
    assert_eq!(first.manifest.trust_status.reason_code, "TRUST_DECISION_REQUIRED");

    let readback = read_inception(&output).expect("inception readback");
    assert_eq!(readback.repo_identity, first.manifest.repo_identity);
    assert!(readback.control_files_complete);

    let second = initialize(repository.path(), &output).expect("second init");
    assert_eq!(second.actions, 0, "unchanged init must be idempotent");
}

#[test]
fn l2_named_target_refuses_missing_control_files() {
    let repository = repository_fixture();
    std::fs::remove_file(repository.path().join("SCHEMAS.toml")).expect("remove control file");
    let output = repository.path().join(".omp-orchestrator/inception.json");

    let error = initialize(repository.path(), &output).expect_err("missing control file must refuse");
    assert!(matches!(error, InceptionError::MissingControlFiles(_)));
    assert!(!output.exists(), "refused init must not write the artifact");
}

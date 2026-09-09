#![forbid(unsafe_code)]

//! Named L2 target for the S1 layer gate.
//!
//! This target exercises the real inception writer and readback contract with an isolated
//! repository fixture. It is invoked directly as Cargo's `--test l2_ecosystem` target.

use ompo_start::inception::{initialize, read_inception, InceptionError, SCHEMA_VERSION};
use std::path::Path;
use std::process::Command;
use serde_json::Value;
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
    for name in ["AGENTS.md", "CLAUDE.md", "README.md", "Cargo.toml", "SCHEMAS.toml"] {
        std::fs::write(directory.path().join(name), b"fixture\n").expect("control file");
    }
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

#[test]
fn l2_named_target_initializes_and_reads_back_identity() {
    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");

    let first = initialize(repository.path(), &output).expect("first init");
    assert_eq!(first.actions, 1, "first init must write the artifact");
    assert_eq!(first.manifest.schema_version, SCHEMA_VERSION);
    assert_eq!(first.manifest.repo_identity.git_marker, "directory");
    assert!(!first.manifest.project_id.is_empty());
    assert!(!first.manifest.repo_identity.source_revision.is_empty());
    assert!(!first.manifest.repo_identity.host_identity.is_empty());
    assert_eq!(first.manifest.trust_status.reason_code, "TRUST_DECISION_REQUIRED");

    let readback = read_inception(&output).expect("inception readback");
    assert_eq!(readback.project_id, first.manifest.project_id);
    assert_eq!(readback.repo_identity, first.manifest.repo_identity);
    assert!(readback.control_files_complete);

    let second = initialize(repository.path(), &output).expect("second init");
    assert_eq!(second.actions, 0, "unchanged init must be idempotent");
}

#[cfg(unix)]
#[test]
fn equivalent_symlink_paths_share_identity() {
    let repository = repository_fixture();
    let alias_root = tempfile::tempdir().expect("alias parent");
    let alias = alias_root.path().join("repo-alias");
    std::os::unix::fs::symlink(repository.path(), &alias).expect("repo symlink");
    let first = initialize(
        repository.path(),
        &repository.path().join(".omp-orchestrator/inception.json"),
    )
    .expect("real path init");
    let second = initialize(
        &alias,
        &alias.join(".omp-orchestrator/inception.json"),
    )
    .expect("symlink path init");
    assert_eq!(first.manifest.project_id, second.manifest.project_id);
    assert_eq!(first.manifest.repo_identity, second.manifest.repo_identity);
}

#[test]
fn readback_refuses_empty_and_missing_identity_fields() {
    let fields = [
        ("project_id", false, "missing required keys: project_id"),
        ("canonical_path", true, "repo_identity.canonical_path is missing"),
        ("source_revision", true, "repo_identity.source_revision is missing"),
        ("git_marker", true, "repo_identity.git_marker is missing"),
        ("host_identity", true, "repo_identity.host_identity is missing"),
    ];
    for (field, nested, detail) in fields {
        let repository = repository_fixture();
        let output = repository.path().join(".omp-orchestrator/inception.json");
        initialize(repository.path(), &output).expect("write inception");
        let mut value: Value = serde_json::from_str(
            &std::fs::read_to_string(&output).expect("read inception"),
        )
        .expect("valid JSON");
        if nested {
            value
                .get_mut("repo_identity")
                .and_then(Value::as_object_mut)
                .expect("repo identity object")
                .insert(field.to_owned(), Value::String(String::new()));
        } else {
            value
                .as_object_mut()
                .expect("manifest object")
                .remove(field);
        }
        std::fs::write(&output, serde_json::to_vec_pretty(&value).expect("encode mutation"))
            .expect("write mutation");
        let error = read_inception(&output).expect_err("identity omission must refuse");
        assert_eq!(
            error.to_string(),
            format!("INCEPTION_READBACK_FAILED path={} detail={detail}", output.display())
        );
    }
}

#[test]
fn readback_refuses_empty_or_missing_manifest_objects() {
    let cases = ["", "{}"];
    for contents in cases {
        let repository = repository_fixture();
        let output = repository.path().join(".omp-orchestrator/inception.json");
        std::fs::create_dir_all(output.parent().expect("artifact parent")).expect("parent");
        std::fs::write(&output, contents).expect("write malformed artifact");
        let error = read_inception(&output).expect_err("empty manifest must refuse");
        assert!(
            error.to_string().starts_with("INCEPTION_READBACK_FAILED"),
            "{error}"
        );
    }

    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");
    initialize(repository.path(), &output).expect("write inception");
    let mut value: Value = serde_json::from_str(
        &std::fs::read_to_string(&output).expect("read inception"),
    )
    .expect("valid JSON");
    value
        .as_object_mut()
        .expect("manifest object")
        .insert("repo_identity".to_owned(), Value::Null);
    std::fs::write(&output, serde_json::to_vec_pretty(&value).expect("encode mutation"))
        .expect("write mutation");
    let error = read_inception(&output).expect_err("null repo identity must refuse");
    assert_eq!(
        error.to_string(),
        format!(
            "INCEPTION_READBACK_FAILED path={} detail=repo_identity is not an object",
            output.display()
        )
    );
}
#[test]
fn nonexistent_root_refuses_canonicalization() {
    let root = tempfile::tempdir().expect("root parent").path().join("missing");
    let output = root.join(".omp-orchestrator/inception.json");
    let error = initialize(&root, &output).expect_err("nonexistent root must refuse");
    assert!(matches!(error, InceptionError::RepositoryUnreadable { .. }));
    assert!(error.to_string().contains("INCEPTION_REPOSITORY_UNREADABLE"));
}
#[test]
fn l2_named_target_refuses_missing_control_files() {
    let repository = repository_fixture();
    std::fs::remove_file(repository.path().join("SCHEMAS.toml")).expect("remove control file");
    let output = repository.path().join(".omp-orchestrator/inception.json");

    let error = initialize(repository.path(), &output).expect_err("missing control file must refuse");
    assert!(matches!(error, InceptionError::MissingControlFiles(_)));
    assert!(!output.exists(), "refused init must not write the artifact");
    std::fs::write(repository.path().join("SCHEMAS.toml"), b"fixture\n")
        .expect("restore control file");
    std::fs::remove_dir_all(repository.path().join(".git")).expect("remove git");
    let identity_error = initialize(repository.path(), &output)
        .expect_err("non-git identity must refuse");
    assert!(matches!(
        identity_error,
        InceptionError::IdentityUnavailable {
            field: "source_revision",
            ..
        }
    ));
}

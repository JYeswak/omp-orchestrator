//! xobj3 legs: doctor scope axis. Health advertises scopes; doctor must accept
//! every advertised scope, and an unknown scope name is a usage error (rc=2),
//! never a degraded verdict (rc=1).

use ompo_doctor::health_repair::health;
use ompo_doctor::{run_doctor, DoctorError};
use std::path::{Path, PathBuf};
use std::process::Command;

const CONTROL_FILES: &[&str] = &[
    "AGENTS.md",
    "CLAUDE.md",
    "README.md",
    "Cargo.toml",
    "SCHEMAS.toml",
    "docs/decisions.jsonl",
];

fn valid_artifact() -> String {
    r#"{"schema_version":"inception.v1","project_id":"fixture","repo_identity":{"canonical_path":"/tmp/x","git_marker":".git","source_revision":"abc","host_identity":"fixture"},"control_files":{"AGENTS.md":true,"CLAUDE.md":true,"README.md":true,"Cargo.toml":true,"SCHEMAS.toml":true,"docs/decisions.jsonl":true},"host_capabilities":{"os":"x","arch":"y","filesystem":"z"},"required_tools":["git","cargo","br","bv","ntm","am","jq"],"trust_status":{"status":"ok","reason_code":"FIXTURE","control_files_complete":true}}"#.to_owned()
}

/// Fixture repo with all control files; `artifact` selects a readable
/// inception artifact (true) or a missing one (false).
fn fixture_repo(label: &str, artifact: bool) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("doctor-scope-{label}-{}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear stale fixture");
    }
    for relative in CONTROL_FILES {
        let path = dir.join(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("fixture dirs");
        std::fs::write(&path, "fixture\n").expect("control file");
    }
    if artifact {
        let path = dir.join(".omp-orchestrator/inception.json");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("artifact dir");
        std::fs::write(&path, valid_artifact()).expect("artifact");
    }
    dir
}

fn doctor_bin(repo: &Path, args: &[&str]) -> std::process::Output {
    let mut full = vec![
        "doctor".to_owned(),
        "--repo".to_owned(),
        repo.to_str().expect("utf8").to_owned(),
    ];
    full.extend(args.iter().map(|arg| arg.to_string()));
    Command::new(env!("CARGO_BIN_EXE_ompo"))
        .args(&full)
        .output()
        .expect("ompo-doctor must launch")
}

#[test]
fn inception_scope_accepted_with_named_probes() {
    let repo = fixture_repo("good", true);
    let summary = run_doctor(&repo, "inception").expect("advertised scope must not refuse");
    assert_eq!(summary.scope, "inception");
    let names: Vec<&str> = summary
        .probes
        .iter()
        .map(|probe| probe.name.as_str())
        .collect();
    assert_eq!(names, vec!["control_files", "inception_artifact"]);
    assert_eq!(summary.exit_code, 0);
    assert_eq!(summary.status, "OK");
    println!("XOBJ3_GOOD scope=inception exit=0");
    std::fs::remove_dir_all(repo).expect("fixture cleanup");
}

#[test]
fn bogus_scope_refused_with_typed_reason() {
    let repo = fixture_repo("bogus", true);
    let error = run_doctor(&repo, "bogus-xyz").expect_err("unknown scope must refuse");
    assert!(
        matches!(error, DoctorError::UnsupportedScope(_)),
        "typed refusal, got {error}"
    );
    assert_eq!(
        error.to_string(),
        "L1_DOCTOR_UNSUPPORTED_SCOPE scope=bogus-xyz"
    );
    println!("XOBJ3_BOGUS reason=L1_DOCTOR_UNSUPPORTED_SCOPE");
    std::fs::remove_dir_all(repo).expect("fixture cleanup");
}

#[test]
fn degraded_artifact_still_accepted_with_actionable_scope() {
    // Mirrors the live incident: health advertises inception exactly when the
    // artifact is unreadable, so doctor must accept that state, not refuse it.
    let repo = fixture_repo("degraded", false);
    let scopes = health(&repo).actionable_scopes();
    assert!(
        !scopes.is_empty(),
        "degraded fixture must advertise at least one scope"
    );
    assert_eq!(scopes, vec!["inception"]);
    let summary =
        run_doctor(&repo, "inception").expect("degraded state is a verdict, not a refusal");
    assert_eq!(summary.exit_code, 1);
    assert_eq!(summary.status, "DEGRADED");
    let artifact = summary
        .probes
        .iter()
        .find(|probe| probe.name == "inception_artifact")
        .expect("artifact probe");
    assert_eq!(artifact.status, "UNREADABLE");
    println!("XOBJ3_DEGRADED scopes={scopes:?} exit=1");
    std::fs::remove_dir_all(repo).expect("fixture cleanup");
}

#[test]
fn health_scopes_subset_of_doctor_accepts() {
    // Durable anti-divergence: every scope health names must doctor-accept.
    // A future scope added to health without a doctor arm fails here, not in
    // production behind a refusal.
    let repo = fixture_repo("axis", false);
    let scopes = health(&repo).actionable_scopes();
    assert!(
        !scopes.is_empty(),
        "axis leg needs at least one advertised scope"
    );
    for scope in scopes {
        run_doctor(&repo, scope)
            .unwrap_or_else(|_| panic!("health-advertised scope refused: {scope}"));
    }
    println!("XOBJ3_AXIS reconciled");
    std::fs::remove_dir_all(repo).expect("fixture cleanup");
}

#[test]
fn cli_bogus_scope_exits_two_with_reason() {
    // Message AND exit together: the mutation leg for this pin collapses the
    // code 2 -> 1 while the token stays unchanged.
    let repo = fixture_repo("cli-bogus", true);
    let output = doctor_bin(&repo, &["--scope", "bogus-xyz-nonexistent", "--json"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(2),
        "usage refusal must exit 2: {stderr}"
    );
    assert!(
        stderr.contains("L1_DOCTOR_UNSUPPORTED_SCOPE"),
        "reason pinned: {stderr}"
    );
    assert!(
        stderr.contains("bogus-xyz-nonexistent"),
        "scope named: {stderr}"
    );
    println!("XOBJ3_CLI_BOGUS exit=2");
    std::fs::remove_dir_all(repo).expect("fixture cleanup");
}

#[test]
fn cli_inception_scope_exits_zero_without_refusal() {
    let repo = fixture_repo("cli-good", true);
    let output = doctor_bin(&repo, &["--scope", "inception", "--json"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stdout={stdout} stderr={stderr}");
    assert!(
        !stderr.contains("UNSUPPORTED_SCOPE"),
        "must not refuse: {stderr}"
    );
    assert!(stdout.contains("\"scope\":\"inception\""), "{stdout}");
    println!("XOBJ3_CLI_GOOD exit=0");
    std::fs::remove_dir_all(repo).expect("fixture cleanup");
}

#[test]
fn cli_degraded_inception_exits_one() {
    let repo = fixture_repo("cli-degraded", false);
    let output = doctor_bin(&repo, &["--scope", "inception", "--json"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "degraded verdict, not refusal: {stderr}"
    );
    println!("XOBJ3_CLI_DEGRADED exit=1");
    std::fs::remove_dir_all(repo).expect("fixture cleanup");
}

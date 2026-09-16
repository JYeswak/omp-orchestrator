#![forbid(unsafe_code)]

//! Named L2 target for the S1 layer gate.
//!
//! This target exercises the real inception writer and readback contract with an isolated
//! repository fixture. It is invoked directly as Cargo's `--test l2_ecosystem` target.

use lifecycle_event::{
    default_repo_journal, DurableJournal, EmitOutcome, Layer, LifecycleEvent, ReasonCode,
};
use lifecycle_monitor::{gate_claimed_write_readback, observe_layer, verify_artifact, LayerState};
use ompo_start::inception::{
    hook_source_identity_report, initialize, initialize_gated, list_backups, read_inception,
    restore_backup, verify_post_write_predicates, CargoWorkspaceError, HookIdentityStatus,
    InceptionError, TrustedInitConsent, TrustedInitDecision, PROJECT_AGENTS_OWNERSHIP_STAMP,
    SCHEMA_VERSION,
};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;
fn marker_exists(project: &str, marker: &str) -> bool {
    Path::new(project).join(marker).exists()
}

fn read_http_json(stream: &mut std::net::TcpStream) -> Value {
    use std::io::Read as _;
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 4096];
    let (body_start, content_length) = loop {
        let read = stream.read(&mut chunk).expect("read mock request");
        assert!(read > 0, "mock request ended before headers");
        bytes.extend_from_slice(&chunk[..read]);
        let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&bytes[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().expect("content length"))
            })
            .expect("mock request content-length");
        break (header_end + 4, content_length);
    };
    while bytes.len() < body_start + content_length {
        let read = stream.read(&mut chunk).expect("read mock body");
        assert!(read > 0, "mock request ended before body");
        bytes.extend_from_slice(&chunk[..read]);
    }
    serde_json::from_slice(&bytes[body_start..body_start + content_length])
        .expect("mock request JSON")
}

fn handle_agent_mail_request(mut stream: std::net::TcpStream) {
    use std::io::Write as _;
    let request = read_http_json(&mut stream);
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let (project, pane) = if method == "resources/read" {
        let uri = request["params"]["uri"].as_str().expect("resource uri");
        (
            uri.strip_prefix("resource://agents/")
                .expect("agent registry resource")
                .to_owned(),
            "%59".to_owned(),
        )
    } else {
        (
            request["params"]["arguments"]["project_key"]
                .as_str()
                .expect("project key")
                .to_owned(),
            request["params"]["arguments"]["pane_id"]
                .as_str()
                .expect("pane id")
                .to_owned(),
        )
    };

    if marker_exists(&project, ".agent-mail-unavailable") {
        let body = br#"{"error":"fixture unavailable"}"#;
        let response = format!(
            "HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(response.as_bytes()).expect("write status");
        stream.write_all(body).expect("write unavailable body");
        return;
    }

    let envelope = if method == "resources/read" {
        if marker_exists(&project, ".agent-mail-project-missing") {
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32602, "message": "Project not found"}
            })
        } else {
            let actual_project = if marker_exists(&project, ".agent-mail-project-mismatch") {
                "/different/project"
            } else {
                project.as_str()
            };
            let agents = if marker_exists(&project, ".agent-mail-agent-missing") {
                Vec::<Value>::new()
            } else {
                vec![serde_json::json!({"name": "BlackMeadow"})]
            };
            let payload = if marker_exists(&project, ".agent-mail-malformed") {
                "not-json".to_owned()
            } else {
                serde_json::json!({
                    "project": {"slug": "fixture", "human_key": actual_project},
                    "agents": agents,
                })
                .to_string()
            };
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {"contents": [{"uri": format!("resource://agents/{project}"), "text": payload}]}
            })
        }
    } else if method == "tools/call"
        && request["params"]["name"].as_str() == Some("resolve_pane_identity")
    {
        let binding = if marker_exists(&project, ".agent-mail-unknown") {
            "future-binding"
        } else {
            "verified-live"
        };
        let resolved_agent = if marker_exists(&project, ".agent-mail-pane-mismatch") {
            "OtherAgent"
        } else {
            "BlackMeadow"
        };
        let payload = serde_json::json!({
            "pane_id": pane,
            "binding": binding,
            "agent_name": resolved_agent,
            "session": "omp-orchestrator",
            "pane_index": 2,
        });
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {"content": [{"type": "text", "text": payload.to_string()}]}
        })
    } else {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": -32601, "message": "fixture method not found"}
        })
    };
    let body = serde_json::to_vec(&envelope).expect("mock response JSON");
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .expect("write headers");
    stream.write_all(&body).expect("write response");
}

fn ensure_agent_mail_fixture_server() {
    static URL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let url = URL.get_or_init(|| {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind Agent Mail mock");
        let address = listener.local_addr().expect("mock address");
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let stream = stream.expect("accept Agent Mail mock request");
                std::thread::spawn(move || handle_agent_mail_request(stream));
            }
        });
        format!("http://{address}/mcp/")
    });
    std::env::set_var("AM_MCP_URL", url);
    std::env::set_var("AGENT_MAIL_BEARER_TOKEN", "fixture-token");
    std::env::set_var("AGENT_MAIL_AGENT", "BlackMeadow");
    std::env::set_var("TMUX_PANE", "%59");
}
fn ensure_rch_fixture_program() {
    use std::os::unix::fs::PermissionsExt as _;
    static BIN: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    let binary = BIN.get_or_init(|| {
        let home = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .expect("test HOME");
        let directory = home
            .join(".local/state/zeststream/scratch/omp-orchestrator")
            .join(format!("ompo-start-test-{}", std::process::id()))
            .join("bx3q");
        std::fs::create_dir_all(&directory).expect("RCH fixture directory");
        std::fs::write(
            directory.join(".owner.json"),
            format!(
                "{{\"session\":\"omp-orchestrator\",\"owner\":\"ompo-start-test-{}\",\"job\":\"bx3q\"}}\n",
                std::process::id()
            ),
        )
        .expect("RCH fixture owner metadata");
        let binary = directory.join("rch");
        let script = br#"#!/bin/sh
case "$*" in
  *"doctor --reliability"*)
    printf '%s\n' '{"api_version":"1.0","command":"doctor.reliability","success":true,"data":{"scope":["topology","convergence"],"diagnostics":[{"category":"topology","check_name":"workers_config","severity":"pass","code":"RCH-R003","message":"fixture topology ready","details":"fixture"},{"category":"repo_presence","check_name":"repo_convergence","severity":"info","code":"RCH-R303","message":"No worker repo-convergence records were reported","details":"status=unknown, total=0, ready=0, converging=0, drifting=0, failed=0, stale=0"}]}}'
    ;;
  *"status --workers"*)
    printf '%s\n' '{"api_version":"1.0","command":"status","success":true,"data":{"convergence":{"status":"unknown","workers":[],"summary":{"total_workers":0,"ready":0,"drifting":0,"converging":0,"failed":0,"stale":0}}}}'
    ;;
  *"diagnose --dry-run"*)
    if [ -f .rch-project-excluded ]; then
      printf '%s\n' '{"api_version":"1.0","command":"diagnose","success":true,"data":{"classification":{"is_compilation":true},"decision":{"would_intercept":false,"reason":"project_excluded"},"worker_selection":{"reason":"no_admissible_workers","diagnostics":{"active_project_exclusion_count":1,"workers":[{"worker_id":"fixture-worker","active_project_excluded":true}]}}}}'
    else
      printf '%s\n' '{"api_version":"1.0","command":"diagnose","success":true,"data":{"classification":{"is_compilation":true},"decision":{"would_intercept":true,"reason":"fixture offload eligible"},"worker_selection":{"reason":"selected"}}}'
    fi
    ;;
  *) exit 64 ;;
esac
"#;
        std::fs::write(&binary, script).expect("RCH fixture program");
        let mut permissions = std::fs::metadata(&binary).expect("RCH fixture metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&binary, permissions).expect("RCH fixture executable");
        binary
    });
    let directory = binary.parent().expect("RCH fixture parent");
    let mut paths =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect::<Vec<_>>();
    if !paths.iter().any(|path| path == directory) {
        paths.insert(0, directory.to_owned());
        std::env::set_var("PATH", std::env::join_paths(paths).expect("fixture PATH"));
    }
}
fn run_git(repo: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    command.current_dir(repo).args(args);
    match subprocess_contract::bounded_output(&mut command, Duration::from_secs(10)) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => {}
        other => panic!("git fixture command failed: {other:?}"),
    }
}

fn explicit_trusted_init_consent(root: &Path, decision_id: &str) -> TrustedInitConsent {
    let repository_scope = root.canonicalize().expect("canonical consent scope");
    let mut command = Command::new("git");
    command
        .current_dir(&repository_scope)
        .args(["rev-parse", "HEAD"]);
    let output = match subprocess_contract::bounded_output(&mut command, Duration::from_secs(10)) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => output,
        other => panic!("consent revision command failed: {other:?}"),
    };
    let source_revision = String::from_utf8(output.stdout)
        .expect("UTF-8 consent revision")
        .trim()
        .to_owned();
    let policy = std::fs::read(repository_scope.join("AGENTS.md")).expect("consent policy bytes");
    let mut policy_sha256 = String::with_capacity(64);
    for byte in Sha256::digest(policy) {
        use std::fmt::Write as _;
        write!(policy_sha256, "{byte:02x}").expect("write policy digest");
    }
    TrustedInitConsent::Explicit {
        decision_id: decision_id.to_owned(),
        repository_scope,
        source_revision,
        policy_sha256,
        template_path: root.join("template.md"),
        template_input: input_manifest::InputManifest::full(),
    }
}

fn foreign_policy_fixture() -> TempDir {
    let repository = repository_fixture();
    std::fs::write(
        repository.path().join("CLAUDE.md"),
        b"fixture omp-orchestrator\n",
    )
    .expect("stamped CLAUDE.md");
    std::fs::write(repository.path().join("AGENTS.md"), b"foreign policy\n")
        .expect("foreign AGENTS.md");
    std::fs::write(repository.path().join("template.md"), b"template source\n")
        .expect("template source");
    run_git(repository.path(), &["add", "template.md"]);
    run_git(
        repository.path(),
        &[
            "-c",
            "user.name=ompo-start-test",
            "-c",
            "user.email=ompo-start-test@example.invalid",
            "commit",
            "-qm",
            "template source",
        ],
    );
    repository
}

fn assert_no_init_residue(root: &Path, output: &Path) {
    assert!(
        !output.exists(),
        "refusal wrote output {}",
        output.display()
    );
    assert!(
        list_backups(output)
            .expect("list refusal backups")
            .is_empty(),
        "refusal wrote backup residue for {}",
        output.display()
    );
    assert!(
        !default_repo_journal(root).exists(),
        "refusal wrote lifecycle residue"
    );
}

fn write_project_agents_stamp(root: &Path) {
    let stamp = format!("fixture {}\n", PROJECT_AGENTS_OWNERSHIP_STAMP);
    assert!(
        !stamp.trim().is_empty(),
        "canonical cbl7 project-agent stamp"
    );
    std::fs::write(root.join("AGENTS.md"), stamp).expect("stamped AGENTS.md");
}

const HOOK_SOURCE_PATH: &str = "crates/agent-mail-native/src/lib.rs";
const HOOK_SOURCE_BYTES: &[u8] = b"pub fn hook_fixture() {}\n";
const HOOK_SOURCE_DIGEST: &str = "5e336085e3231b40af66b75c24fc5af9a2a6d1747ff999165537001919632576";

fn installed_hook(root: &Path) -> std::path::PathBuf {
    root.join(".git/hooks/pre-commit")
}

fn write_hook_manifest(root: &Path, digest: &str) {
    let hook = installed_hook(root);
    std::fs::create_dir_all(hook.parent().expect("hook parent")).expect("hook directory");
    // Real Mach-O `strings` glues the final digest to adjacent rodata. The
    // fixture carries that boundary so exact-match does not depend on a newline.
    std::fs::write(
        hook,
        format!("{HOOK_SOURCE_PATH} {digest}adjacent-rodata\n"),
    )
    .expect("hook manifest artifact");
}

fn commit_hook_source_change(root: &Path, bytes: &[u8]) {
    std::fs::write(root.join(HOOK_SOURCE_PATH), bytes).expect("changed hook source");
    run_git(root, &["add", HOOK_SOURCE_PATH]);
    run_git(
        root,
        &[
            "-c",
            "user.name=ompo-start-test",
            "-c",
            "user.email=ompo-start-test@example.invalid",
            "commit",
            "-qm",
            "hook source changed",
        ],
    );
}

fn repository_fixture() -> TempDir {
    ensure_agent_mail_fixture_server();
    ensure_rch_fixture_program();
    let directory = tempfile::tempdir().expect("fixture directory");
    std::fs::create_dir(directory.path().join("docs")).expect("docs directory");
    for name in ["CLAUDE.md", "README.md", "SCHEMAS.toml"] {
        std::fs::write(directory.path().join(name), b"fixture\n").expect("control file");
    }
    std::fs::create_dir(directory.path().join("src")).expect("fixture src directory");
    std::fs::write(
        directory.path().join("src/lib.rs"),
        b"pub fn fixture() {}\n",
    )
    .expect("fixture library");
    std::fs::write(
        directory.path().join("Cargo.toml"),
        b"[package]\nname = \"ompo-start\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
    )
    .expect("Cargo metadata fixture");
    write_project_agents_stamp(directory.path());
    // zb2p companion: the gated trust entry requires a ready tracker
    // (bead zb2p). Initialize `.beads` here so gated legs measure their
    // own gate, not the tracker gate.
    std::fs::create_dir(directory.path().join(".beads")).expect("beads dir");
    std::fs::write(
        directory.path().join(".beads/issues.jsonl"),
        "{\"id\":\"fixture-0001\",\"title\":\"fixture\"}\n",
    )
    .expect("beads issues");
    // yhia companion: the gated trust entry requires a satisfied
    // toolchain pin (bead yhia). Declare the repository pin here so
    // gated legs measure their own gate, not the pin gate.
    std::fs::write(
        directory.path().join("rust-toolchain.toml"),
        "[toolchain]\nchannel = \"stable\"\n",
    )
    .expect("toolchain pin");
    std::fs::write(directory.path().join("docs/decisions.jsonl"), b"{}\n")
        .expect("decision ledger");
    let hook_source = directory.path().join(HOOK_SOURCE_PATH);
    std::fs::create_dir_all(hook_source.parent().expect("hook source parent"))
        .expect("hook source directory");
    std::fs::write(&hook_source, HOOK_SOURCE_BYTES).expect("hook source");
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
    write_hook_manifest(directory.path(), HOOK_SOURCE_DIGEST);
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
    assert_eq!(
        first.manifest.trust_status.reason_code,
        "TRUST_DECISION_REQUIRED"
    );

    let readback = read_inception(&output).expect("inception readback");
    assert_eq!(readback.project_id, first.manifest.project_id);
    assert_eq!(readback.repo_identity, first.manifest.repo_identity);
    assert!(readback.control_files_complete);

    let second = initialize(repository.path(), &output).expect("second init");
    assert_eq!(second.actions, 0, "unchanged init must be idempotent");
}
#[test]
fn second_init_reopens_on_policy_hash_drift() {
    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");
    let first = initialize(repository.path(), &output).expect("first init");
    let original_artifact = std::fs::read(&output).expect("first artifact bytes");
    let first_policy_hash = first.manifest.trust_status.policy_sha256.clone();

    let mut policy = std::fs::read(repository.path().join("AGENTS.md"))
        .expect("policy bytes");
    policy.extend_from_slice(b"\n# policy hash drift\n");
    std::fs::write(repository.path().join("AGENTS.md"), policy)
        .expect("changed policy bytes");

    let second = initialize(repository.path(), &output).expect("policy drift repair");
    assert_ne!(
        second.manifest.trust_status.policy_sha256,
        first_policy_hash,
        "policy hash must be re-derived"
    );
    assert_eq!(second.actions, 1, "policy drift must reopen repair");
    let backup = second.backup.expect("policy drift must preserve prior artifact");
    assert_eq!(
        std::fs::read(backup).expect("backup bytes"),
        original_artifact,
        "policy drift backup must preserve the prior artifact"
    );
    assert_eq!(second.backup_ratio_verdict, "BACKUP_RATIO_OK_1_TO_1");
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
    let second = initialize(&alias, &alias.join(".omp-orchestrator/inception.json"))
        .expect("symlink path init");
    assert_eq!(first.manifest.project_id, second.manifest.project_id);
    assert_eq!(first.manifest.repo_identity, second.manifest.repo_identity);
}

#[test]
fn readback_refuses_empty_and_missing_identity_fields() {
    let fields = [
        ("project_id", false),
        ("canonical_path", true),
        ("source_revision", true),
        ("git_marker", true),
        ("host_identity", true),
    ];
    for (field, nested) in fields {
        let repository = repository_fixture();
        let output = repository.path().join(".omp-orchestrator/inception.json");
        initialize(repository.path(), &output).expect("write inception");
        let mut value: Value =
            serde_json::from_str(&std::fs::read_to_string(&output).expect("read inception"))
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
        std::fs::write(
            &output,
            serde_json::to_vec_pretty(&value).expect("encode mutation"),
        )
        .expect("write mutation");
        let error = read_inception(&output).expect_err("identity omission must refuse");
        match (field, error) {
            ("project_id", InceptionError::ReadbackMissingKey { key, .. }) => {
                assert_eq!(key, "project_id");
            }
            (field, InceptionError::ReadbackEmpty { key, .. }) => {
                assert_eq!(key, format!("repo_identity.{field}"));
            }
            (field, error) => panic!("wrong typed refusal for {field}: {error}"),
        }
    }
}
#[test]
fn foreign_root_or_revision_refuses_before_success_evidence() {
    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");
    let first = initialize(repository.path(), &output).expect("healthy init");
    verify_post_write_predicates(repository.path(), &first.manifest, &output)
        .expect("healthy identity passes the real post-write guard");

    let current_root = repository.path().canonicalize().expect("canonical root");
    let current_revision = first.manifest.repo_identity.source_revision.clone();
    let cases = [
        (
            "canonical_path",
            current_root
                .join("foreign-repository")
                .display()
                .to_string(),
        ),
        ("source_revision", "0".repeat(current_revision.len())),
    ];
    for (field, foreign) in cases {
        let mut foreign_manifest = first.manifest.clone();
        match field {
            "canonical_path" => foreign_manifest.repo_identity.canonical_path = foreign.clone(),
            "source_revision" => foreign_manifest.repo_identity.source_revision = foreign.clone(),
            _ => unreachable!("case table is closed"),
        }
        let error = verify_post_write_predicates(repository.path(), &foreign_manifest, &output)
            .expect_err("foreign identity must refuse before success evidence");
        match error {
            InceptionError::Readback { detail, .. } => {
                assert!(detail.contains("POST_WRITE_PREDICATE_CHANGED"));
                assert!(detail.contains("predicate=identity"));
                assert!(detail.contains(&format!("expected={foreign}")));
                assert!(detail.contains("provided="));
                assert!(detail.contains(field));
            }
            error => panic!("foreign {field} used wrong refusal: {error}"),
        }
    }
}

#[test]
fn readback_refuses_empty_or_missing_manifest_objects() {
    for contents in ["", "{}"] {
        let repository = repository_fixture();
        let output = repository.path().join(".omp-orchestrator/inception.json");
        std::fs::create_dir_all(output.parent().expect("artifact parent")).expect("parent");
        std::fs::write(&output, contents).expect("write malformed artifact");
        let error = read_inception(&output).expect_err("empty manifest must refuse");
        if contents.is_empty() {
            assert!(matches!(error, InceptionError::ReadbackMalformed { .. }));
        } else {
            assert!(matches!(
                error,
                InceptionError::ReadbackMissingKey { key, .. } if key == "schema_version"
            ));
        }
    }

    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");
    initialize(repository.path(), &output).expect("write inception");
    let mut value: Value =
        serde_json::from_str(&std::fs::read_to_string(&output).expect("read inception"))
            .expect("valid JSON");
    value
        .as_object_mut()
        .expect("manifest object")
        .insert("repo_identity".to_owned(), Value::Null);
    std::fs::write(
        &output,
        serde_json::to_vec_pretty(&value).expect("encode mutation"),
    )
    .expect("write mutation");
    let error = read_inception(&output).expect_err("null repo identity must refuse");
    assert!(matches!(
        error,
        InceptionError::ReadbackWrongType { key, expected, found, .. }
            if key == "repo_identity" && expected == "object" && found == "null"
    ));
}

#[test]
fn nonexistent_root_refuses_canonicalization() {
    let root = tempfile::tempdir()
        .expect("root parent")
        .path()
        .join("missing");
    let output = root.join(".omp-orchestrator/inception.json");
    let error = initialize(&root, &output).expect_err("nonexistent root must refuse");
    assert!(matches!(error, InceptionError::RepositoryUnreadable { .. }));
    assert!(error
        .to_string()
        .contains("INCEPTION_REPOSITORY_UNREADABLE"));
}
#[test]
fn l2_named_target_refuses_missing_control_files() {
    let repository = repository_fixture();
    std::fs::remove_file(repository.path().join("SCHEMAS.toml")).expect("remove control file");
    let output = repository.path().join(".omp-orchestrator/inception.json");

    let error =
        initialize(repository.path(), &output).expect_err("missing control file must refuse");
    assert!(matches!(error, InceptionError::MissingControlFiles(_)));
    assert!(!output.exists(), "refused init must not write the artifact");
    std::fs::write(repository.path().join("SCHEMAS.toml"), b"fixture\n")
        .expect("restore control file");
    std::fs::remove_dir_all(repository.path().join(".git")).expect("remove git");
    let identity_error =
        initialize(repository.path(), &output).expect_err("non-git identity must refuse");
    assert!(matches!(
        identity_error,
        InceptionError::IdentityUnavailable {
            field: "source_revision",
            ..
        }
    ));
}

/// Typed scan for the one S1.L2 row the init write+reprobe chokepoint owes.
///
/// ANTI-VACUITY, and the reason this is a typed refusal rather than an
/// `Option`: zero rows is an ERROR, never a pass. The exit polarity is NOT
/// invented here -- it reuses lifecycle-monitor's existing authority
/// (`error_exit_code`, crates/lifecycle-monitor/src/main.rs:204): an absent or
/// empty scan exits 2, a real content violation exits 1. A second convention
/// would let the two disagree about what "nothing was written" means, which is
/// the collapse this layer exists to prevent.
#[derive(Debug, PartialEq, Eq)]
struct L2ScanRefusal {
    reason_code: &'static str,
    exit_code: u8,
    detail: String,
}

impl std::fmt::Display for L2ScanRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} exit={} {}",
            self.reason_code, self.exit_code, self.detail
        )
    }
}

const L2_EMIT_STAGE_TO: &str = "S1.L2";

/// Every S1.L2 row in `journal`, or a typed refusal naming the missing stage.
///
/// Blank lines are skipped rather than parsed: JSONL tolerates a trailing
/// newline, and treating one as an unparseable row would report a FORMAT fault
/// where the real answer is "the row is absent".
fn scan_s1_l2_rows(journal: &Path) -> Result<Vec<Value>, L2ScanRefusal> {
    let text = std::fs::read_to_string(journal).map_err(|error| L2ScanRefusal {
        reason_code: "L2_SCAN_UNREADABLE_JOURNAL",
        exit_code: 2,
        detail: format!(
            "journal={} stage_to={L2_EMIT_STAGE_TO} detail={error}",
            journal.display()
        ),
    })?;
    let mut rows = Vec::new();
    let mut scanned = 0_usize;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        scanned += 1;
        let value: Value = serde_json::from_str(line).map_err(|error| L2ScanRefusal {
            reason_code: "L2_SCAN_UNPARSEABLE_ROW",
            exit_code: 1,
            detail: format!("journal={} detail={error}", journal.display()),
        })?;
        if value.get("stage_to").and_then(Value::as_str) == Some(L2_EMIT_STAGE_TO) {
            rows.push(value);
        }
    }
    if rows.is_empty() {
        return Err(L2ScanRefusal {
            reason_code: "L2_SCAN_ZERO_S1_L2_ROWS",
            exit_code: 2,
            detail: format!(
                "journal={} stage_to={L2_EMIT_STAGE_TO} missing rows_scanned={scanned}",
                journal.display()
            ),
        });
    }
    Ok(rows)
}

/// The shape the chokepoint's row must carry.
///
/// A row that EXISTS with the wrong content is a real violation and exits 1 --
/// distinct from absence's 2. Without the distinction a grader cannot tell "the
/// writer was suppressed" from "the writer wrote the wrong thing", and those
/// need different repairs.
fn require_init_row_shape(row: &Value) -> Result<(), L2ScanRefusal> {
    for (key, expected) in [
        ("layer", "L2"),
        ("stage_from", "S1.L1"),
        ("stage_to", L2_EMIT_STAGE_TO),
        ("actor", "ompo-init"),
        ("outcome", "emitted"),
        ("reason_code", "INIT_REPROBE_OK"),
    ] {
        let found = row.get(key).and_then(Value::as_str).unwrap_or("<absent>");
        if found != expected {
            return Err(L2ScanRefusal {
                reason_code: "L2_SCAN_ROW_CONTENT_MISMATCH",
                exit_code: 1,
                detail: format!("key={key} expected={expected} found={found}"),
            });
        }
    }
    Ok(())
}

/// y7pq positive leg: the init write+reprobe chokepoint -- `initialize_inner`'s
/// `emit_init_event` call, which sits AFTER `write_atomic` and AFTER the
/// `read_inception` re-probe in crates/ompo-start/src/inception.rs -- emits
/// exactly ONE S1.L2 LifecycleEvent, with before/after evidence either side of
/// the single call.
#[test]
fn l2_init_chokepoint_emits_one_lifecycle_row_with_before_after_evidence() {
    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");
    let journal = default_repo_journal(repository.path());

    // BEFORE: no journal at all, pinned as a typed absence on BOTH message and
    // exit so the leg cannot pass by the journal being unreadable for some
    // unrelated reason.
    let before = scan_s1_l2_rows(&journal).expect_err("no journal before init");
    assert_eq!(
        before.reason_code, "L2_SCAN_UNREADABLE_JOURNAL",
        "before: {before}"
    );
    assert_eq!(before.exit_code, 2, "before: {before}");

    let report = initialize(repository.path(), &output).expect("accepted init");

    // AFTER: exactly one row, carrying the chokepoint's shape.
    let after = scan_s1_l2_rows(&journal).expect("one S1.L2 row after init");
    assert_eq!(after.len(), 1, "one row per chokepoint pass");
    require_init_row_shape(&after[0]).expect("chokepoint row shape");

    // RE-PROBE evidence: the writer's own readback count and the monitor's
    // INDEPENDENT re-read of the same journal agree. `journal_rows` comes from
    // the emit readback, `monitor_rows` from lifecycle_monitor::verify_artifact;
    // equal counts are what makes the re-probe an observation rather than a
    // restatement of the write's return code.
    assert_eq!(report.journal_rows, 1, "emit readback saw one row");
    assert_eq!(report.monitor_rows, 1, "monitor re-read saw one row");
}

/// y7pq KNOWN-BAD leg: suppress the event writer and the missing row is
/// DETECTED, naming the stage_to it owed.
///
/// Pins BOTH the message and the exit code: a message-only assertion survives
/// an exit-code collapse, and a code-only assertion survives an unrelated 101.
#[test]
fn l2_suppressed_event_writer_is_detected_naming_the_missing_stage_to() {
    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");
    let journal = default_repo_journal(repository.path());
    initialize(repository.path(), &output).expect("accepted init");

    let emitted = scan_s1_l2_rows(&journal).expect("row exists before suppression");
    let mut foreign = emitted[0].clone();
    foreign["layer"] = Value::String("L5".to_owned());
    foreign["stage_to"] = Value::String("S1.L5".to_owned());

    // Suppression modelled on disk: the L2 row is gone and a foreign-layer row
    // remains, so the refusal proves "the L2 row is missing" rather than "the
    // file is empty". Those are different failures and an empty-file check
    // would conflate them.
    std::fs::write(
        &journal,
        format!(
            "{}\n",
            serde_json::to_string(&foreign).expect("encode foreign row")
        ),
    )
    .expect("suppressed journal");

    let refusal = scan_s1_l2_rows(&journal).expect_err("suppressed writer must be detected");
    assert_eq!(refusal.reason_code, "L2_SCAN_ZERO_S1_L2_ROWS", "{refusal}");
    assert_eq!(refusal.exit_code, 2, "{refusal}");
    assert!(
        refusal.detail.contains("stage_to=S1.L2"),
        "refusal must name the missing stage_to: {refusal}"
    );
    assert!(
        refusal.detail.contains("rows_scanned=1"),
        "refusal must show the journal was non-empty: {refusal}"
    );
}

/// ANTI-VACUITY, measured rather than asserted in prose: absence and a real
/// content violation carry DIFFERENT exits, and the untouched row still passes
/// so the check is not over-strict.
#[test]
fn absent_row_and_content_violation_carry_distinct_exit_codes() {
    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");
    let journal = default_repo_journal(repository.path());
    initialize(repository.path(), &output).expect("accepted init");
    let row = scan_s1_l2_rows(&journal).expect("emitted row")[0].clone();

    // KNOWN-GOOD: the row the chokepoint actually wrote passes untouched.
    require_init_row_shape(&row).expect("known-good row must pass");

    let mut tampered = row.clone();
    tampered["reason_code"] = Value::String("INIT_REPROBE_LIED".to_owned());
    let violation = require_init_row_shape(&tampered).expect_err("content violation");
    assert_eq!(
        violation.reason_code, "L2_SCAN_ROW_CONTENT_MISMATCH",
        "{violation}"
    );
    assert_eq!(violation.exit_code, 1, "{violation}");
    assert!(
        violation.detail.contains("expected=INIT_REPROBE_OK"),
        "violation must name the reason_code it expected: {violation}"
    );

    std::fs::remove_file(&journal).expect("remove journal");
    let absent = scan_s1_l2_rows(&journal).expect_err("absent journal");
    assert_ne!(
        absent.exit_code, violation.exit_code,
        "absence must not share a real violation's exit: {absent} vs {violation}"
    );
}

/// The keys SCHEMAS.toml:158 declares required for the inception artifact.
///
/// Restated from the DECLARATION rather than imported from the writer's private
/// `REQUIRED_KEYS`: if the two ever disagree, this leg is the one that should
/// redden. A test that imports the writer's own constant proves only that the
/// writer agrees with itself.
const DECLARED_REQUIRED_KEYS: &[&str] = &[
    "schema_version",
    "project_id",
    "repo_identity",
    "control_files",
    "host_capabilities",
    "required_tools",
    "trust_status",
];

/// 0pc9 MONITOR obligation: the re-probe happens after EVERY write, including
/// the write that turns out to be a no-op, and zero second-run actions are
/// earned ONLY by an identical hash.
///
/// The obligation is on the CALLER, so the leg watches the caller: it is not
/// enough that a probe function exists. Three passes are measured -- a real
/// write, an identical no-op, and a changed hash -- and the monitor's
/// INDEPENDENT re-read advances on all three. A re-probe that only ran when
/// bytes changed would leave the idempotent path unobserved, which is the path
/// that runs every time after the first.
#[test]
fn l2_reprobe_follows_every_write_and_only_identical_hashes_are_zero_action() {
    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");
    let journal = default_repo_journal(repository.path());

    let first = initialize(repository.path(), &output).expect("first init");
    assert_eq!(first.actions, 1, "first pass must write the artifact");
    assert_eq!(
        first.monitor_rows,
        verify_artifact(&journal).expect("independent re-read"),
        "the monitor count must be re-derivable by a third party"
    );

    // Identical bytes: zero ARTIFACT actions, but the re-probe still ran.
    let second = initialize(repository.path(), &output).expect("second init");
    assert_eq!(second.actions, 0, "identical hash earns zero actions");
    assert_eq!(second.backup, None, "a no-op write must not snapshot");
    assert_eq!(
        second.monitor_rows,
        first.monitor_rows + 1,
        "the re-probe must run after the no-op write too"
    );

    // Changed hash: the second run is NOT zero-action, and the prior bytes are
    // preserved. Repair, never a silent overwrite.
    std::fs::write(&output, b"tampered\n").expect("change the artifact hash");
    let third = initialize(repository.path(), &output).expect("third init");
    assert_eq!(third.actions, 1, "a changed hash must not be zero-action");
    let backup = third.backup.expect("a changed hash must snapshot first");
    assert_eq!(
        std::fs::read(&backup).expect("backup bytes"),
        b"tampered\n",
        "the snapshot must hold the bytes that were replaced"
    );
    assert_eq!(
        third.monitor_rows,
        second.monitor_rows + 1,
        "the re-probe must run after the repairing write"
    );

    // The freshness-bearing read of the same journal: an observation with an
    // AGE, never a bare boolean.
    let verdict = observe_layer(&journal, Layer::L2, 600_000).expect("L2 observation");
    assert_eq!(verdict.state, LayerState::Progressing);
    assert_eq!(verdict.row_count, 3, "one row per chokepoint pass");
    assert_eq!(verdict.last_reason, "INIT_REPROBE_OK");
    assert!(verdict.fresh, "a just-written row cannot be stale");
}

/// 0pc9 KNOWN-BAD: skip the re-probe and success is REFUSED -- never granted by
/// the write's return code.
///
/// `gate_claimed_write_readback` is the production gate for exactly this: it
/// takes a write that CLAIMS to have happened and demands the artifact prove
/// it. The claimed row is never written here, so a caller that trusted its own
/// return code would report success over an empty journal.
#[test]
fn l2_write_claimed_without_a_reprobe_is_refused_not_believed() {
    let directory = tempfile::tempdir().expect("journal fixture");
    let journal_path = directory.path().join("claimed.jsonl");
    let journal = DurableJournal::open(&journal_path).expect("open journal");
    let claimed = LifecycleEvent::new(
        Layer::L2,
        "S1.L1",
        "S1.L2",
        "ompo-init",
        EmitOutcome::Emitted,
        ReasonCode::new("INIT_REPROBE_OK").expect("non-empty reason code"),
    );

    let refusal = gate_claimed_write_readback(&journal, std::slice::from_ref(&claimed))
        .expect_err("a claimed write with no artifact must refuse");
    let rendered = refusal.to_string();
    assert!(
        rendered.contains("READBACK"),
        "refusal must name the readback it could not make: {rendered}"
    );

    // KNOWN-GOOD, so the gate is not merely always-refusing: once the row is
    // really on disk the same claim passes.
    lifecycle_event::emit_one_host(&journal, claimed.clone()).expect("real emit");
    gate_claimed_write_readback(&journal, std::slice::from_ref(&claimed))
        .expect("a real write must pass the same gate");
}

/// up37 ARTIFACT-with-readback: an accepted init writes repo_identity,
/// control_files and trust_status, snapshots the bytes it replaces, and every
/// declared field survives a READ BACK from disk.
///
/// The readback is the acceptance. A write that returned success is not
/// evidence, so nothing here trusts `InitReport` alone -- each field is re-read
/// off the file, and the backup is verified by re-hashing its bytes against the
/// SHA in its own filename.
#[test]
fn l2_accepted_init_writes_backups_and_reads_back_every_declared_field() {
    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");

    let first = initialize(repository.path(), &output).expect("accepted init");
    assert!(
        list_backups(&output).expect("backup listing").is_empty(),
        "a first write replaces nothing and must not invent a snapshot"
    );

    // Replace the artifact so the write path has something to snapshot.
    std::fs::write(&output, b"superseded\n").expect("supersede artifact");
    initialize(repository.path(), &output).expect("replacing init");

    let backups = list_backups(&output).expect("backup listing");
    assert_eq!(
        backups.len(),
        1,
        "one replacement, one content-keyed backup"
    );
    assert_eq!(
        std::fs::read(&backups[0].path).expect("backup bytes"),
        b"superseded\n"
    );
    restore_backup(&output, &backups[0]).expect("an intact backup must restore");

    // Re-run so the artifact on disk is the manifest again, then READ BACK.
    let restored = initialize(repository.path(), &output).expect("post-restore init");
    let readback = read_inception(&output).expect("readback from disk");
    assert_eq!(readback.repo_identity, restored.manifest.repo_identity);
    assert_eq!(readback.project_id, first.manifest.project_id);
    assert!(
        readback.control_files_complete,
        "control_files must read back complete"
    );
    assert_eq!(
        restored.manifest.trust_status.reason_code, "TRUST_DECISION_REQUIRED",
        "trust_status must carry an explicit decision, not a default"
    );

    let on_disk: Value =
        serde_json::from_str(&std::fs::read_to_string(&output).expect("artifact text"))
            .expect("artifact is JSON");
    for key in DECLARED_REQUIRED_KEYS {
        assert!(
            on_disk.get(*key).is_some(),
            "declared required key {key} missing from the artifact on disk"
        );
    }
}

/// up37 KNOWN-BAD: damage the BACKUP before it is read back and the restore is
/// refused, naming the integrity failure.
///
/// The field arm of this acceptance is already covered by
/// `readback_refuses_empty_and_missing_identity_fields`. This is the backup
/// arm, and it is the one that matters for a restore: a corrupted backup
/// restored silently would leave the operator believing the artifact had been
/// recovered when it had been destroyed.
#[test]
fn l2_damaged_backup_refuses_restore_instead_of_recovering_garbage() {
    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");
    initialize(repository.path(), &output).expect("accepted init");
    std::fs::write(&output, b"superseded\n").expect("supersede artifact");
    initialize(repository.path(), &output).expect("replacing init");

    let backups = list_backups(&output).expect("backup listing");
    assert_eq!(backups.len(), 1);
    let entry = backups[0].clone();

    // KNOWN-GOOD first: intact, it restores. Without this the refusal below
    // could be satisfied by a restore path that never works at all.
    let good = restore_backup(&output, &entry).expect("intact backup restores");
    assert_eq!(good.actions, 1);
    assert_eq!(good.restored_from, Some(entry.path.clone()));

    // Now damage the backup's CONTENT while leaving the SHA in its filename.
    std::fs::write(&entry.path, b"not the bytes the name claims\n").expect("damage backup");
    let refusal =
        restore_backup(&output, &entry).expect_err("a damaged backup must refuse to restore");
    let rendered = refusal.to_string();
    assert!(
        rendered.contains("backup integrity failed"),
        "refusal must name the integrity failure: {rendered}"
    );
    assert!(
        rendered.contains(&entry.content_sha),
        "refusal must name the SHA the filename claimed: {rendered}"
    );

    // And a missing backup is a DIFFERENT failure from a damaged one.
    std::fs::remove_file(&entry.path).expect("remove backup");
    let missing = restore_backup(&output, &entry).expect_err("a missing backup must refuse");
    assert!(
        missing.to_string().contains("backup read failed"),
        "missing and damaged must not collapse into one message: {missing}"
    );
}

/// L2-TEST-GIT-REPO (contract s1_l2_ecosystem.md `L2-BUILD-GIT-REPO`):
/// `git_repo_toplevel` yields the canonical top-level path for a real
/// repository and a typed halt with remediation outside one. Uses the
/// real production check -- never a copy of its arms: a copy would agree
/// with the subject by construction.
///
/// Upward-search note: git resolves parent checkouts, so the production
/// check ceilings the search at the argument's parent (per-spawn env,
/// thread-safe, no process-global games): a real repository carries its
/// own `.git`, found before any ascent, while a bare directory inside a
/// checkout then reads 128 deterministically on every lane.
///
/// KNOWN-BAD: invert the mapping (a refusal reads as identity) and the
/// halt arm fails: a bare tempdir would certify as a repository.
/// Message AND exit are pinned on the mutation run.
#[test]
fn non_repo_halts() {
    use ompo_start::inception::git_repo_toplevel;
    // Healthy: a real repository yields its canonical top level.
    let repository = repository_fixture();
    let top = git_repo_toplevel(repository.path()).expect("real repo resolves");
    assert_eq!(
        top,
        repository.path().canonicalize().expect("canonical"),
        "healthy branch yields the canonical top level"
    );
    // Halt: outside git, typed halt with remediation, never a guessed path.
    let bare = tempfile::tempdir().expect("bare fixture");
    let error = git_repo_toplevel(bare.path()).expect_err("non-repo must halt");
    let text = error.to_string();
    assert!(
        text.starts_with("INCEPTION_IDENTITY_UNAVAILABLE"),
        "halt must be typed, got: {text}"
    );
    assert!(
        text.contains("remedy:"),
        "halt must carry remediation, got: {text}"
    );
}

/// L2-TEST-REMOTE-PERSONA-A (bead nqac): Persona A with no git remote
/// records an explicit `remote_optional=true` allowance and continues;
/// absence is never a silent universal success. Uses the real
/// `persona_remote_policy` over live `git remote -v` observations --
/// never a copy of its arms. (No Persona B/C variants exist in this
/// tree; non-Persona-A cells cover that population by construction.)
///
/// KNOWN-BAD: drop the Persona A allowance (never optional) and the
/// no-remote Persona A cell fails: a local-only subject would halt
/// where the contract grants continuation. Message AND exit are pinned
/// on the mutation run.
#[test]
fn persona_a_local_only_remote_rule() {
    use ompo_start::inception::{persona_remote_policy, PersonaRemote};
    // No remote anywhere here: repository_fixture never adds one, so the
    // no-remote cells observe a real absence, not an injected boolean.
    let repository = repository_fixture();
    // Persona A, no remote: explicit allowance with continuation.
    let allowed = persona_remote_policy(repository.path(), true).expect("policy answers");
    assert_eq!(
        allowed,
        PersonaRemote {
            remote_optional: true,
            remote_present: false,
            reason_code: "PERSONA_A_LOCAL_ONLY",
        },
        "Persona A with no remote must carry the explicit allowance"
    );
    // Non-Persona-A, no remote: restrictive, unchanged by this rule.
    let required = persona_remote_policy(repository.path(), false).expect("policy answers");
    assert_eq!(
        required.remote_optional, false,
        "a missing remote stays required off Persona A"
    );
    assert_eq!(
        required.reason_code, "REMOTE_REQUIRED",
        "the restrictive branch names its reason, got {}",
        required.reason_code
    );
    // Persona A WITH a remote: nothing to allow, still restrictive-shaped
    // (adding a remote needs no network: `remote -v` only reads config).
    run_git(
        repository.path(),
        &["remote", "add", "origin", "https://example.invalid/x.git"],
    );
    let present = persona_remote_policy(repository.path(), true).expect("policy answers");
    assert_eq!(
        present.remote_optional, false,
        "a present remote needs no allowance"
    );
}

/// nqac wiring rework: the reachable L2 production entry consumes the
/// existing Persona A policy before initialization continues and carries the
/// exact verdict it used. Both healthy remote states reach the artifact; the
/// local-only state is never inferred from ambient environment state.
///
/// KNOWN-BAD: bypass `persona_remote_policy` at `initialize_gated` or
/// replace its result with a fabricated local-only record. This leg then
/// fails on the returned policy record while the standalone policy matrix
/// above remains the known-good control. Message AND exit are pinned on the
/// mutation run.
#[test]
fn persona_a_remote_policy_is_carried_by_gated_entry() {
    use ompo_start::inception::{initialize_gated, PersonaRemote};

    let remote = repository_fixture();
    std::fs::write(
        remote.path().join("CLAUDE.md"),
        b"fixture omp-orchestrator\n",
    )
    .expect("stamped claude");
    run_git(
        remote.path(),
        &["remote", "add", "origin", "https://example.invalid/x.git"],
    );
    let remote_output = remote
        .path()
        .join(".omp-orchestrator/init-gated-remote-present.json");
    let remote_report =
        initialize_gated(remote.path(), &remote_output, &TrustedInitConsent::Absent)
            .expect("remote-present Persona A proceeds");
    assert_eq!(
        remote_report.persona_remote,
        Some(PersonaRemote {
            remote_optional: false,
            remote_present: true,
            reason_code: "REMOTE_REQUIRED",
        }),
        "the gated entry must carry the live remote-present policy verdict"
    );
    assert!(remote_output.is_file(), "remote-present init must continue");

    let local = repository_fixture();
    std::fs::write(
        local.path().join("CLAUDE.md"),
        b"fixture omp-orchestrator\n",
    )
    .expect("stamped claude");
    let local_output = local
        .path()
        .join(".omp-orchestrator/init-gated-local-only.json");
    let local_report = initialize_gated(local.path(), &local_output, &TrustedInitConsent::Absent)
        .expect("local-only Persona A proceeds");
    assert_eq!(
        local_report.persona_remote,
        Some(PersonaRemote {
            remote_optional: true,
            remote_present: false,
            reason_code: "PERSONA_A_LOCAL_ONLY",
        }),
        "local-only continuation must carry its explicit Persona A allowance"
    );
    assert!(local_output.is_file(), "local-only init must continue");
}

/// L2-TEST-REMOTE-PERSONA-BC (bead mxro): a non-optional policy without
/// an observed remote halts shared dispatch with a named remediation;
/// every other record passes through. Reuses the nqac `PersonaRemote`
/// policy -- no second remote detector, no new persona variants (none
/// exist in this tree; non-Persona-A covers that population).
///
/// KNOWN-BAD: drop the halt (always continue) and the restrictive cell
/// below passes silently: a fleet subject with no remote would advance
/// with no evidence anything was required. Message AND exit are pinned
/// on the mutation run.
#[test]
fn persona_bc_missing_remote_halts_dispatch() {
    use ompo_start::inception::{persona_remote_policy, require_remote_for_dispatch};
    let repository = repository_fixture();
    // Allowance passes through: Persona A local-only continues.
    let allowed = persona_remote_policy(repository.path(), true).expect("policy answers");
    require_remote_for_dispatch(&allowed).expect("allowance continues");
    // Present remote passes through on every persona.
    run_git(
        repository.path(),
        &["remote", "add", "origin", "https://example.invalid/x.git"],
    );
    for persona_a in [true, false] {
        let present = persona_remote_policy(repository.path(), persona_a).expect("policy answers");
        require_remote_for_dispatch(&present).expect("a present remote continues");
    }
    // Restrictive branch with nothing behind it: non-Persona-A, no
    // remote. Remove the remote again and require the named halt.
    run_git(repository.path(), &["remote", "remove", "origin"]);
    let required = persona_remote_policy(repository.path(), false).expect("policy answers");
    assert_eq!(
        required.remote_optional, false,
        "non-Persona-A without remote stays restrictive"
    );
    let error =
        require_remote_for_dispatch(&required).expect_err("missing remote must halt dispatch");
    let text = error.to_string();
    assert!(
        text.contains("REMOTE_REQUIRED"),
        "halt must name the required branch, got: {text}"
    );
    assert!(
        text.contains("remedy:"),
        "halt must carry remediation, got: {text}"
    );
}

/// mxro wiring rework: the shared-dispatch entry applies the existing
/// required-remote gate to its one policy observation. The preceding
/// `persona_bc_missing_remote_halts_dispatch` matrix owns the Persona A and
/// remote-present known-good controls; keeping them there avoids a second copy.
///
/// KNOWN-BAD: bypass the enforcement call inside the entry while retaining
/// the policy observation. Exactly this named leg fails, while the companion
/// behavior matrix remains green. Message AND exit are pinned on mutation.
#[test]
fn required_remote_gate_is_wired_to_shared_dispatch_entry() {
    use ompo_start::inception::remote_policy_for_shared_dispatch;

    let repository = repository_fixture();
    let error = remote_policy_for_shared_dispatch(repository.path(), false)
        .expect_err("non-Persona-A without a remote must halt before dispatch");
    let text = error.to_string();
    assert!(
        text.contains("REMOTE_REQUIRED"),
        "shared-dispatch halt must name the required-remote branch, got: {text}"
    );
    assert!(
        text.contains("remedy:") && text.contains("configure a remote"),
        "shared-dispatch halt must carry named remediation, got: {text}"
    );
}

/// tqs8 healthy selector: the artifact's canonical manifest equals the
/// authoritative source set at the resolved HEAD commit.
#[test]
fn hook_identity_exact_manifest_matches_head() {
    let report = hook_source_identity_report(repository_fixture().path());
    match (report.status, report.source_commit, report.manifest_rows) {
        (HookIdentityStatus::ExactMatch, Some(_), 1) => {}
        other => panic!("exact identity must name one-row HEAD authority: {other:?}"),
    }
}

/// tqs8 restrictive selector: absence is distinct from unreadable bytes.
#[test]
fn hook_identity_missing_hook_is_typed() {
    let repository = repository_fixture();
    std::fs::remove_file(installed_hook(repository.path())).expect("remove hook");
    let report = hook_source_identity_report(repository.path());
    assert_eq!(report.status, HookIdentityStatus::MissingHook, "{report:?}");
    assert_eq!(report.status.reason_code(), "HOOK_IDENTITY_MISSING");
}

/// tqs8 restrictive selector: an existing path that cannot be read is not a
/// missing hook and never inherits the missing-hook remedy.
#[test]
fn hook_identity_unreadable_hook_is_typed() {
    let repository = repository_fixture();
    let hook = installed_hook(repository.path());
    std::fs::remove_file(&hook).expect("remove hook file");
    std::fs::create_dir(&hook).expect("directory mask is unreadable as a file");
    let report = hook_source_identity_report(repository.path());
    assert_eq!(
        report.status,
        HookIdentityStatus::UnreadableHook,
        "{report:?}"
    );
    assert_eq!(report.status.reason_code(), "HOOK_IDENTITY_UNREADABLE");
}

/// tqs8 restrictive selector: the hook may exist while the repository has no
/// HEAD authority at all.
#[test]
fn hook_identity_absent_source_commit_is_typed() {
    let repository = tempfile::tempdir().expect("fixture directory");
    write_hook_manifest(repository.path(), HOOK_SOURCE_DIGEST);
    let report = hook_source_identity_report(repository.path());
    assert_eq!(
        report.status,
        HookIdentityStatus::SourceCommitAbsent,
        "{report:?}"
    );
    assert_eq!(report.status.reason_code(), "HOOK_SOURCE_COMMIT_ABSENT");
}

/// tqs8 restrictive selector: a HEAD reference with no commit is present but
/// unresolvable, not absent.
#[test]
fn hook_identity_unresolvable_source_commit_is_typed() {
    let repository = tempfile::tempdir().expect("fixture directory");
    run_git(repository.path(), &["init", "-q"]);
    write_hook_manifest(repository.path(), HOOK_SOURCE_DIGEST);
    let report = hook_source_identity_report(repository.path());
    assert_eq!(
        report.status,
        HookIdentityStatus::SourceCommitUnresolvable,
        "{report:?}"
    );
    assert_eq!(
        report.status.reason_code(),
        "HOOK_SOURCE_COMMIT_UNRESOLVABLE"
    );
}

/// tqs8 restrictive selector: a valid artifact manifest naming different
/// source bytes is a content mismatch, never an unproven artifact.
#[test]
fn hook_identity_content_mismatch_names_changed_source() {
    let repository = repository_fixture();
    commit_hook_source_change(repository.path(), b"pub fn hook_fixture_changed() {}\n");
    let report = hook_source_identity_report(repository.path());
    assert_eq!(
        report.status,
        HookIdentityStatus::ContentMismatch,
        "{report:?}"
    );
    assert!(report.detail.contains(HOOK_SOURCE_PATH), "{report:?}");
    assert_eq!(report.status.reason_code(), "HOOK_IDENTITY_HEAD_MISMATCH");
}

/// tqs8 restrictive selector: readable bytes with no canonical manifest do
/// not establish any source identity.
#[test]
fn hook_identity_unproven_artifact_is_typed() {
    let repository = repository_fixture();
    std::fs::write(
        installed_hook(repository.path()),
        b"opaque artifact without a source stamp\n",
    )
    .expect("unstamped hook");
    let report = hook_source_identity_report(repository.path());
    assert_eq!(
        report.status,
        HookIdentityStatus::UnprovenArtifact,
        "{report:?}"
    );
    assert_eq!(
        report.status.reason_code(),
        "HOOK_IDENTITY_UNPROVEN_ARTIFACT"
    );
}

/// tqs8 wiring selector: the reachable gated entry consumes the hook verdict
/// after the existing Persona A gate and before `initialize` can write trust.
///
/// KNOWN-BAD: bypassing the verdict match at `initialize_gated` makes this
/// exact leg fail because mismatched source reaches a trust artifact. The
/// report-level restrictive selectors above remain green.
#[test]
fn hook_identity_verdict_gates_reachable_l2_entry() {
    use ompo_start::inception::initialize_gated;

    let repository = repository_fixture();
    std::fs::write(
        repository.path().join("CLAUDE.md"),
        b"fixture omp-orchestrator\n",
    )
    .expect("stamped claude");
    commit_hook_source_change(repository.path(), b"pub fn hook_fixture_changed() {}\n");
    let output = repository
        .path()
        .join(".omp-orchestrator/hook-refused.json");
    match initialize_gated(repository.path(), &output, &TrustedInitConsent::Absent) {
        Err(InceptionError::HookIdentityRefused {
            status: HookIdentityStatus::ContentMismatch,
            detail,
            ..
        }) => assert!(detail.contains(HOOK_SOURCE_PATH), "{detail}"),
        other => panic!("mismatched hook must stop the gated entry: {other:?}"),
    }
    assert!(!output.exists(), "refusal must precede the trust write");
}

/// L2-ENTRY-GIT-REPO (bead e0li rework): the L2 operator entry gates on
/// the repository check before downstream state continues. A real
/// repository proceeds through `initialize` (canonical root); a bare
/// directory halts typed with remediation and writes nothing -- no
/// artifact, no journal side effects from a flow that never started.
/// Uses the real `initialize_gated` entry, never a copy of its arms.
///
/// KNOWN-BAD: bypassing the gate (delegating straight to `initialize`)
/// greens the bare directory below and this leg fails: downstream state
/// would continue without a repository, which is the unwired adoption
/// this row exists to prevent. Message AND exit are pinned on the
/// mutation run.
#[test]
fn gated_entry_requires_git_repo() {
    use ompo_start::inception::initialize_gated;
    // Healthy: a real repository proceeds with actions recorded.
    let repository = repository_fixture();
    // qruz companion: the gated entry now also requires a stamped
    // CLAUDE.md. Stamp it so this leg keeps measuring the git gate,
    // not the stamp gate.
    std::fs::write(
        repository.path().join("CLAUDE.md"),
        b"fixture omp-orchestrator\n",
    )
    .expect("stamped claude");
    let output = repository.path().join(".omp-orchestrator/inception.json");
    let report = initialize_gated(repository.path(), &output, &TrustedInitConsent::Absent)
        .expect("real repo proceeds");
    assert!(
        output.is_file(),
        "a gated real repo must produce its artifact"
    );
    let _ = report;
    // Halt: a bare directory halts typed with remediation BEFORE any
    // downstream write -- the artifact must not exist afterwards.
    let bare = tempfile::tempdir().expect("bare fixture");
    let bare_output = bare.path().join(".omp-orchestrator/inception.json");
    let error = initialize_gated(bare.path(), &bare_output, &TrustedInitConsent::Absent)
        .expect_err("non-repo must halt");
    let text = error.to_string();
    assert!(
        text.starts_with("INCEPTION_IDENTITY_UNAVAILABLE"),
        "halt must be typed, got: {text}"
    );
    assert!(
        text.contains("remedy:"),
        "halt must carry remediation, got: {text}"
    );
    assert!(
        !bare_output.exists(),
        "halted entry must write nothing, found {}",
        bare_output.display()
    );
}

/// L2-TEST-AGENTS-STAMP (bead 43x7): the control-file probe reports
/// AGENTS.md stamp identity, live source revision, and a typed status.
/// Uses the real `agents_stamp_report` over real fixtures -- stamp
/// token referenced, never copied; no line count pinned anywhere.
///
/// KNOWN-BAD: blind the detector (never see the token) and the stamped
/// arms fail while the unstamped arms stay green: a missing stamp
/// would certify. Message AND exit are pinned on the mutation run.
#[test]
fn agents_stamp_reports_identity_revision_and_status() {
    use ompo_start::inception::{agents_stamp_report, AgentsStampStatus};
    fn live_head(repo: &std::path::Path) -> String {
        let output = Command::new("git")
            .current_dir(repo)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("git rev-parse runs");
        assert!(output.status.success(), "fixture must be a live repository");
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }
    // Healthy: stamped file in a live repo reports identity, live
    // revision, and Stamped status together.
    let repository = repository_fixture();
    let report = agents_stamp_report(repository.path());
    assert!(report.stamp_present, "the fixture token must be seen");
    assert_eq!(
        report.source_revision.as_deref(),
        Some(live_head(repository.path()).as_str()),
        "revision must track live HEAD, not a constant"
    );
    assert_eq!(
        report.status,
        AgentsStampStatus::Stamped,
        "stamped plus revision is Stamped"
    );
    // Revision disagreement: a new commit moves HEAD and the report
    // must track it, never the recorded value.
    run_git(
        repository.path(),
        &[
            "-c",
            "user.name=ompo-start-test",
            "-c",
            "user.email=ompo-start-test@example.invalid",
            "commit",
            "--allow-empty",
            "-qm",
            "second",
        ],
    );
    let moved = agents_stamp_report(repository.path());
    assert_ne!(
        moved.source_revision.as_deref(),
        report.source_revision.as_deref(),
        "the report must track HEAD across commits"
    );
    assert!(moved.stamp_present, "stamp survives unrelated commits");
    // Corrupt the stamp (token gone, file otherwise intact): identity
    // lost, revision still observed, status Unstamped.
    std::fs::write(repository.path().join("AGENTS.md"), b"foreign stuff\n")
        .expect("corruption lands");
    let corrupt = agents_stamp_report(repository.path());
    assert!(!corrupt.stamp_present, "a tokenless file must not certify");
    assert!(
        corrupt.source_revision.is_some(),
        "revision observes independently of the stamp"
    );
    assert_eq!(
        corrupt.status,
        AgentsStampStatus::Unstamped,
        "tokenless is Unstamped"
    );
    // Stamped file with no usable git: a `.git` FILE pointing nowhere is
    // fatal locally, so no upward search can rescue it -- revision is
    // deterministically unknown on every lane, unlike a bare tempdir
    // (which resolves parent checkouts on some workers).
    let nogit = tempfile::tempdir().expect("no-git fixture");
    std::fs::write(
        nogit.path().join("AGENTS.md"),
        b"fixture omp-orchestrator\n",
    )
    .expect("stamped non-repo file");
    std::fs::write(nogit.path().join(".git"), b"gitdir: /nonexistent/e0li\n")
        .expect("broken gitdir");
    let report = agents_stamp_report(nogit.path());
    assert!(
        report.stamp_present && report.source_revision.is_none(),
        "non-git stamp keeps identity without revision, got {report:?}"
    );
    assert_eq!(
        report.status,
        AgentsStampStatus::GitUnavailable,
        "stamp without git is GitUnavailable"
    );
}

/// L2-TEST-CLAUDE-STAMP (bead qruz): the reachable L2 trust-flow entry
/// admits a stamped CLAUDE.md and refuses every other stamp state
/// before trust-dependent continuation. Uses the real
/// `initialize_gated` entry end to end -- never a copy of its arms:
/// a copy would agree with the subject by construction.
///
/// KNOWN-BAD: bypass the stamp gate at the entry (proceed regardless)
/// and the restrictive arms below pass silently: unstamped trust would
/// advance with no evidence anything was required. Message AND exit
/// are pinned on the mutation run.
#[test]
fn claude_stamp_gates_trust_entry() {
    use ompo_start::inception::{claude_stamp_report, initialize_gated, AgentsStampStatus};
    // Healthy: stamped CLAUDE.md in a live repo reaches the trust
    // branch -- initialize runs and writes the artifact.
    let repository = repository_fixture();
    std::fs::write(
        repository.path().join("CLAUDE.md"),
        b"fixture omp-orchestrator\n",
    )
    .expect("stamped claude");
    let output = repository.path().join(".omp-orchestrator/init-gated.json");
    initialize_gated(repository.path(), &output, &TrustedInitConsent::Absent)
        .expect("stamped entry proceeds");
    assert!(output.exists(), "a trusted entry writes its artifact");
    // Restrictive matrix: every non-Stamped state refuses typed before
    // initialize runs, with the file and the remedy named.
    let cases: Vec<(&str, Box<dyn Fn(&std::path::Path)>)> = vec![
        (
            "empty",
            Box::new(|root| {
                std::fs::write(root.join("CLAUDE.md"), b"").expect("empty file");
            }),
        ),
        (
            "foreign",
            Box::new(|root| {
                std::fs::write(root.join("CLAUDE.md"), b"foreign stuff\n").expect("foreign file");
            }),
        ),
        (
            "unreadable",
            Box::new(|root| {
                // Hermetic: a previous arm may have left a directory here.
                let path = root.join("CLAUDE.md");
                if path.is_dir() {
                    std::fs::remove_dir_all(&path).expect("clear dir");
                } else {
                    let _ = std::fs::remove_file(&path);
                }
                std::fs::create_dir(&path).expect("directory mask");
            }),
        ),
        (
            "missing",
            Box::new(|root| {
                let path = root.join("CLAUDE.md");
                if path.is_dir() {
                    std::fs::remove_dir_all(&path).expect("clear dir");
                } else {
                    std::fs::remove_file(&path).expect("remove file");
                }
            }),
        ),
    ];
    for (name, arrange) in cases {
        arrange(repository.path());
        let output = repository
            .path()
            .join(format!(".omp-orchestrator/init-gated-{name}.json"));
        let error = initialize_gated(repository.path(), &output, &TrustedInitConsent::Absent)
            .expect_err("unstamped must refuse");
        let text = error.to_string();
        assert!(
            text.contains("HUMAN_HALT") && text.contains("CLAUDE.md"),
            "{name} refusal must be typed and name the file, got: {text}"
        );
        assert!(
            text.contains("remedy=") || text.contains("stamp it"),
            "{name} refusal must carry remediation, got: {text}"
        );
        assert!(
            !output.exists(),
            "a refused entry must write nothing, found {}",
            output.display()
        );
    }
    // GitUnavailable is a report-level state: a stamped file with no
    // usable git observes identity without revision.
    let nogit = tempfile::tempdir().expect("no-git fixture");
    std::fs::write(
        nogit.path().join("CLAUDE.md"),
        b"fixture omp-orchestrator\n",
    )
    .expect("stamped non-repo file");
    std::fs::write(nogit.path().join(".git"), b"gitdir: /nonexistent/qruz\n")
        .expect("broken gitdir");
    let report = claude_stamp_report(nogit.path());
    assert_eq!(
        report.status,
        AgentsStampStatus::GitUnavailable,
        "stamp without git is GitUnavailable"
    );
}

/// L2-TEST-AGENTS-STAMP (bead 43x7): the reachable L2 trust-flow entry
/// admits a stamped AGENTS.md and refuses non-consentable AGENTS states
/// before trust-dependent continuation. Foreign nonempty policy is owned by
/// the trusted-init consent tests below. Reuses the shared stamp core and
/// the `initialize_gated` seam beside the CLAUDE report -- no duplicate
/// core, no second stamp vocabulary.
///
/// KNOWN-BAD: bypass the AGENTS report at the entry (proceed regardless)
/// and the restrictive arms below pass silently: unstamped trust would
/// advance with no evidence anything was required. Message AND exit
/// are pinned on the mutation run.
#[test]
fn agents_stamp_gates_trust_entry() {
    use ompo_start::inception::{agents_stamp_report, initialize_gated, AgentsStampStatus};
    // Healthy: stamped AGENTS.md and stamped CLAUDE.md in a live repo
    // reach the trust branch -- initialize runs and writes the artifact.
    // (The fixture stamps AGENTS.md; CLAUDE.md is stamped here so the
    // CLAUDE gate passes and this leg measures the AGENTS gate alone.)
    let repository = repository_fixture();
    std::fs::write(
        repository.path().join("CLAUDE.md"),
        b"fixture omp-orchestrator\n",
    )
    .expect("stamped claude");
    let output = repository
        .path()
        .join(".omp-orchestrator/init-gated-agents.json");
    initialize_gated(repository.path(), &output, &TrustedInitConsent::Absent)
        .expect("stamped entry proceeds");
    assert!(output.exists(), "a trusted entry writes its artifact");
    // Restrictive matrix: empty, corrupt, unreadable, and missing policy
    // cannot be consented to and still refuse before initialize runs. CLAUDE.md
    // stays stamped throughout, so each refusal is the AGENTS gate firing.
    let cases: Vec<(&str, Box<dyn Fn(&std::path::Path)>)> = vec![
        (
            "empty",
            Box::new(|root| {
                std::fs::write(root.join("AGENTS.md"), b"").expect("empty file");
            }),
        ),
        (
            "corrupt",
            Box::new(|root| {
                std::fs::write(root.join("AGENTS.md"), b"\xff\xfe invalid \x00 bytes\n")
                    .expect("corrupt file");
            }),
        ),
        (
            "unreadable",
            Box::new(|root| {
                // Hermetic: a previous arm may have left a directory here.
                let path = root.join("AGENTS.md");
                if path.is_dir() {
                    std::fs::remove_dir_all(&path).expect("clear dir");
                } else {
                    let _ = std::fs::remove_file(&path);
                }
                std::fs::create_dir(&path).expect("directory mask");
            }),
        ),
        (
            "missing",
            Box::new(|root| {
                let path = root.join("AGENTS.md");
                if path.is_dir() {
                    std::fs::remove_dir_all(&path).expect("clear dir");
                } else {
                    std::fs::remove_file(&path).expect("remove file");
                }
            }),
        ),
    ];
    for (name, arrange) in cases {
        arrange(repository.path());
        let output = repository
            .path()
            .join(format!(".omp-orchestrator/init-gated-agents-{name}.json"));
        let error = initialize_gated(repository.path(), &output, &TrustedInitConsent::Absent)
            .expect_err("unstamped must refuse");
        let text = error.to_string();
        assert!(
            text.contains("HUMAN_HALT") && text.contains("AGENTS.md"),
            "{name} refusal must be typed and name the file, got: {text}"
        );
        assert!(
            text.contains("remedy=") || text.contains("stamp it"),
            "{name} refusal must carry remediation, got: {text}"
        );
        assert!(
            !output.exists(),
            "a refused entry must write nothing, found {}",
            output.display()
        );
    }
    // GitUnavailable is a report-level state: a stamped file with no
    // usable git observes identity without revision.
    let nogit = tempfile::tempdir().expect("no-git fixture");
    std::fs::write(
        nogit.path().join("AGENTS.md"),
        b"fixture omp-orchestrator\n",
    )
    .expect("stamped non-repo file");
    std::fs::write(nogit.path().join(".git"), b"gitdir: /nonexistent/43x7r\n")
        .expect("broken gitdir");
    let report = agents_stamp_report(nogit.path());
    assert_eq!(
        report.status,
        AgentsStampStatus::GitUnavailable,
        "stamp without git is GitUnavailable"
    );
}

/// L2-TEST-BEADS (bead zb2p): the reachable L2 trust-flow entry admits a
/// ready tracker and refuses every other tracker state before
/// initialization or dispatch can continue. Consumes the read-only
/// [`beads_init_report`] probe at `initialize_gated` -- never a second
/// tracker client, and fixtures are isolated tempdirs, never the live
/// tracker. Project identity is the first row's id prefix (see the
/// reporter's NO-CLAIM); writability is permission-bit evidence.
///
/// KNOWN-BAD: bypass the beads report at the entry (proceed regardless)
/// and the restrictive arms below pass silently: uninitialized trust
/// would advance with no evidence anything was required. Message AND
/// exit are pinned on the mutation run.
#[test]
fn beads_init_gates_trust_entry() {
    use ompo_start::inception::{beads_init_report, initialize_gated, BeadsInitStatus};
    use std::os::unix::fs::PermissionsExt;
    // Healthy: stamped control files plus an initialized isolated
    // tracker reach the trust branch -- initialize runs, the artifact
    // lands, and the report names the fixture project identity.
    // (The fixture stamps AGENTS.md and initializes `.beads`; CLAUDE.md
    // is stamped here so the earlier gates pass and this leg measures
    // the tracker gate alone.)
    let repository = repository_fixture();
    std::fs::write(
        repository.path().join("CLAUDE.md"),
        b"fixture omp-orchestrator\n",
    )
    .expect("stamped claude");
    let report = beads_init_report(repository.path());
    assert_eq!(
        report.status,
        BeadsInitStatus::Ready,
        "fixture tracker is ready"
    );
    assert_eq!(
        report.project_identity.as_deref(),
        Some("fixture"),
        "report names the fixture project identity"
    );
    assert!(
        report.readable && report.writable,
        "ready means readable and writable"
    );
    let output = repository
        .path()
        .join(".omp-orchestrator/init-gated-beads.json");
    initialize_gated(repository.path(), &output, &TrustedInitConsent::Absent)
        .expect("stamped entry proceeds");
    assert!(output.exists(), "a trusted entry writes its artifact");
    // Restrictive matrix: every non-Ready tracker state refuses typed
    // before initialize runs, with the tracker path and the remedy
    // named. Control files stay stamped throughout, so each refusal is
    // the tracker gate firing.
    let cases: Vec<(&str, Box<dyn Fn(&std::path::Path)>)> = vec![
        (
            "missing",
            Box::new(|root| {
                let dir = root.join(".beads");
                if dir.is_dir() {
                    std::fs::remove_dir_all(&dir).expect("remove beads dir");
                }
            }),
        ),
        (
            "uninitialized-empty",
            Box::new(|root| {
                // Hermetic: a previous arm may have removed the dir or left a
                // directory mask at the file path.
                let dir = root.join(".beads");
                if !dir.is_dir() {
                    std::fs::create_dir(&dir).expect("recreate beads dir");
                }
                let file = dir.join("issues.jsonl");
                if file.is_dir() {
                    std::fs::remove_dir_all(&file).expect("clear mask");
                }
                std::fs::write(&file, b"").expect("empty issues");
            }),
        ),
        (
            "uninitialized-absent",
            Box::new(|root| {
                let file = root.join(".beads/issues.jsonl");
                if file.is_dir() {
                    std::fs::remove_dir_all(&file).expect("clear mask");
                } else {
                    let _ = std::fs::remove_file(&file);
                }
            }),
        ),
        (
            "unreadable",
            Box::new(|root| {
                // (no chmod hazard): identity can never be established.
                let file = root.join(".beads/issues.jsonl");
                if !file.is_dir() {
                    let _ = std::fs::remove_file(&file);
                    std::fs::create_dir(&file).expect("directory mask");
                }
            }),
        ),
        (
            "unreadable-garbage",
            Box::new(|root| {
                // Valid UTF-8 with no usable id: initialized bytes, missing
                // project identity. Treated as unreadable -- identity cannot
                // be established either way.
                let file = root.join(".beads/issues.jsonl");
                if file.is_dir() {
                    std::fs::remove_dir_all(&file).expect("clear mask");
                }
                std::fs::write(&file, b"not json at all\n").expect("garbage issues");
            }),
        ),
        (
            "unwritable",
            Box::new(|root| {
                // Permission-bit evidence, read -- never an access proof, so
                // this holds for every uid including root (see reporter).
                let file = root.join(".beads/issues.jsonl");
                if file.is_dir() {
                    std::fs::remove_dir_all(&file).expect("clear mask");
                }
                std::fs::write(&file, "{\"id\":\"fixture-0001\"}\n").expect("restore issues");
                let mut permissions = std::fs::metadata(&file).expect("metadata").permissions();
                permissions.set_mode(0o444);
                std::fs::set_permissions(&file, permissions).expect("deny write bits");
            }),
        ),
    ];
    for (name, arrange) in cases {
        arrange(repository.path());
        let output = repository
            .path()
            .join(format!(".omp-orchestrator/init-gated-beads-{name}.json"));
        let error = initialize_gated(repository.path(), &output, &TrustedInitConsent::Absent)
            .expect_err("unready must refuse");
        let text = error.to_string();
        assert!(
            text.contains("HUMAN_HALT") && text.contains(".beads"),
            "{name} refusal must be typed and name the tracker, got: {text}"
        );
        assert!(
            text.contains("br init") || text.contains("readable and writable"),
            "{name} refusal must carry remediation, got: {text}"
        );
        assert!(
            !output.exists(),
            "a refused entry must write nothing, found {}",
            output.display()
        );
    }
}

/// L2-TEST-RUST-TOOLCHAIN (bead yhia): the reachable L2 trust-flow entry
/// admits a satisfied toolchain pin and refuses every other pin state
/// before initialization can continue. Consumes the read-only
/// [`toolchain_pin_report`] probe at `initialize_gated`: the declared
/// `channel` checked against the live `rustc --version` with the same
/// match vocabulary as the doctor's pin diagnostic (which this crate
/// cannot depend on -- the doctor depends on it). Fixtures are isolated
/// tempdirs, never the live repo; the pin file is never copied, only
/// declared per arm.
///
/// KNOWN-BAD: bypass the pin report at the entry (proceed regardless)
/// and the restrictive arms below pass silently: mismatched trust would
/// advance with no evidence anything was required. Message AND exit
/// are pinned on the mutation run.
#[test]
fn toolchain_pin_gates_trust_entry() {
    use ompo_start::inception::{initialize_gated, toolchain_pin_report, ToolchainPinStatus};
    // Healthy: the repository pin (`stable`) against the lane's default
    // toolchain reaches the trust branch -- initialize runs and the
    // artifact lands. (The fixture declares the repository pin; stamps
    // and tracker are ready so the earlier gates pass and this leg
    // measures the pin gate alone. The active line rides along in every
    // message below, so a lane defaulting elsewhere fails LOUDLY with
    // its cause named instead of mysteriously.)
    let repository = repository_fixture();
    std::fs::write(
        repository.path().join("CLAUDE.md"),
        b"fixture omp-orchestrator\n",
    )
    .expect("stamped claude");
    let report = toolchain_pin_report(repository.path());
    let active = report
        .active
        .clone()
        .unwrap_or_else(|| "<unreadable>".to_owned());
    assert_eq!(
        report.status,
        ToolchainPinStatus::Ready,
        "repository pin is satisfied on this lane (active={active})"
    );
    assert_eq!(
        report.declared.as_deref(),
        Some("stable"),
        "report names the declared pin"
    );
    let output = repository
        .path()
        .join(".omp-orchestrator/init-gated-toolchain.json");
    initialize_gated(repository.path(), &output, &TrustedInitConsent::Absent)
        .expect("stamped entry proceeds");
    assert!(output.exists(), "a trusted entry writes its artifact");
    // Restrictive matrix: every non-Ready pin state refuses typed
    // before initialize runs, with the pin file and the remedy named.
    // Earlier gates stay satisfied throughout, so each refusal is the
    // pin gate firing.
    let cases: Vec<(&str, Box<dyn Fn(&std::path::Path)>)> = vec![
        (
            "missing",
            Box::new(|root| {
                let file = root.join("rust-toolchain.toml");
                if file.is_dir() {
                    std::fs::remove_dir_all(&file).expect("clear mask");
                } else {
                    std::fs::remove_file(&file).expect("remove pin");
                }
            }),
        ),
        (
            "unreadable",
            Box::new(|root| {
                // A directory at the file path fails the read for every uid
                // (no chmod hazard): the pin can never be established.
                let file = root.join("rust-toolchain.toml");
                if !file.is_dir() {
                    let _ = std::fs::remove_file(&file);
                    std::fs::create_dir(&file).expect("directory mask");
                }
            }),
        ),
        (
            "unparseable",
            Box::new(|root| {
                // Present but unquoted channel: malformed, not a pin.
                let file = root.join("rust-toolchain.toml");
                if file.is_dir() {
                    std::fs::remove_dir_all(&file).expect("clear mask");
                }
                std::fs::write(&file, "[toolchain]\nchannel = stable\n").expect("bare pin");
            }),
        ),
        (
            "unpinned-absent",
            Box::new(|root| {
                let file = root.join("rust-toolchain.toml");
                if file.is_dir() {
                    std::fs::remove_dir_all(&file).expect("clear mask");
                }
                std::fs::write(&file, "[toolchain]\nprofile = \"minimal\"\n")
                    .expect("pinless file");
            }),
        ),
        (
            "unpinned-empty",
            Box::new(|root| {
                let file = root.join("rust-toolchain.toml");
                if file.is_dir() {
                    std::fs::remove_dir_all(&file).expect("clear mask");
                }
                std::fs::write(&file, "[toolchain]\nchannel = \"\"\n").expect("empty pin");
            }),
        ),
        (
            "mismatched",
            Box::new(|root| {
                // A declared channel the lane's default toolchain cannot
                // satisfy: stable rustc is never beta.
                let file = root.join("rust-toolchain.toml");
                if file.is_dir() {
                    std::fs::remove_dir_all(&file).expect("clear mask");
                }
                std::fs::write(&file, "[toolchain]\nchannel = \"beta\"\n").expect("beta pin");
            }),
        ),
    ];
    for (name, arrange) in cases {
        arrange(repository.path());
        let output = repository.path().join(format!(
            ".omp-orchestrator/init-gated-toolchain-{name}.json"
        ));
        let error = initialize_gated(repository.path(), &output, &TrustedInitConsent::Absent)
            .expect_err("unsatisfied must refuse");
        let text = error.to_string();
        assert!(
            text.contains("HUMAN_HALT") && text.contains("rust-toolchain.toml"),
            "{name} refusal must be typed and name the pin file, got: {text}"
        );
        assert!(
            text.contains("rustup") || text.contains("declare the channel"),
            "{name} refusal must carry remediation, got: {text}"
        );
        assert!(
            !output.exists(),
            "a refused entry must write nothing, found {}",
            output.display()
        );
    }
}

fn do8n_metadata(package: &str, members: &[&str]) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "packages": [{"name": package, "id": format!("path+file:///fixture#{package}@0.1.0")}],
        "workspace_members": members,
    }))
    .expect("metadata fixture JSON")
}

#[test]
fn cargo_workspace_current_member_is_derived() {
    use input_manifest::InputManifest;
    use ompo_start::inception::{cargo_workspace_member_report, CURRENT_WORKSPACE_PACKAGE};
    let report = cargo_workspace_member_report(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        &InputManifest::full(),
    )
    .expect("fresh Cargo metadata contains the current crate");
    assert!(!report.package_ids.is_empty(), "package set is nonempty");
    assert!(
        !report.workspace_member_ids.is_empty(),
        "workspace-member set is nonempty"
    );
    assert!(
        report.package_ids.contains(&report.current_package_id)
            && report
                .workspace_member_ids
                .contains(&report.current_package_id),
        "{CURRENT_WORKSPACE_PACKAGE} must occur in both derived sets"
    );
}

#[test]
fn cargo_workspace_missing_cargo_is_typed() {
    use input_manifest::InputManifest;
    use ompo_start::inception::cargo_workspace_member_report_with_program;
    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");
    let missing = repository.path().join("definitely-missing-cargo");
    let error = cargo_workspace_member_report_with_program(
        repository.path(),
        &InputManifest::full(),
        &missing,
    )
    .expect_err("missing Cargo must halt");
    assert!(matches!(error, CargoWorkspaceError::CargoMissing { .. }));
    let text = error.to_string();
    assert!(
        text.contains("CARGO_WORKSPACE_CARGO_MISSING") && text.contains("remedy="),
        "missing Cargo needs distinct remediation: {text}"
    );
    assert!(!output.exists(), "a Cargo refusal writes no trust artifact");
}

#[test]
fn cargo_workspace_metadata_failure_is_typed() {
    use input_manifest::InputManifest;
    use ompo_start::inception::cargo_workspace_member_report_with_program;
    let repository = repository_fixture();
    let error = cargo_workspace_member_report_with_program(
        repository.path(),
        &InputManifest::full(),
        Path::new("false"),
    )
    .expect_err("nonzero metadata command must halt");
    assert!(matches!(error, CargoWorkspaceError::MetadataFailed { .. }));
    assert!(error
        .to_string()
        .contains("CARGO_WORKSPACE_METADATA_FAILED"));
}

#[test]
fn cargo_workspace_malformed_json_is_typed() {
    use input_manifest::InputManifest;
    use ompo_start::inception::cargo_workspace_member_from_output;
    let error = cargo_workspace_member_from_output(b"not-json", &InputManifest::full())
        .expect_err("malformed metadata must halt");
    assert!(matches!(
        error,
        CargoWorkspaceError::MetadataMalformed { .. }
    ));
}

#[test]
fn cargo_workspace_empty_packages_is_typed() {
    use input_manifest::InputManifest;
    use ompo_start::inception::cargo_workspace_member_from_output;
    let metadata = serde_json::to_vec(&serde_json::json!({
        "packages": [],
        "workspace_members": ["fixture-member"],
    }))
    .expect("metadata JSON");
    let error = cargo_workspace_member_from_output(&metadata, &InputManifest::full())
        .expect_err("empty packages must halt");
    assert!(matches!(
        error,
        CargoWorkspaceError::Empty { field: "packages" }
    ));
}

#[test]
fn cargo_workspace_empty_members_is_typed() {
    use input_manifest::InputManifest;
    use ompo_start::inception::cargo_workspace_member_from_output;
    let metadata = do8n_metadata("ompo-start", &[]);
    let error = cargo_workspace_member_from_output(&metadata, &InputManifest::full())
        .expect_err("empty workspace members must halt");
    assert!(matches!(
        error,
        CargoWorkspaceError::Empty {
            field: "workspace_members"
        }
    ));
}

#[test]
fn cargo_workspace_current_crate_absent_is_typed() {
    use input_manifest::InputManifest;
    use ompo_start::inception::cargo_workspace_member_from_output;
    let id = "path+file:///fixture#different@0.1.0";
    let metadata = do8n_metadata("different", &[id]);
    let error = cargo_workspace_member_from_output(&metadata, &InputManifest::full())
        .expect_err("missing current crate must halt");
    assert!(matches!(
        error,
        CargoWorkspaceError::CurrentCrateAbsent { .. }
    ));
    assert!(error
        .to_string()
        .contains("CARGO_WORKSPACE_CURRENT_CRATE_ABSENT"));
}

fn cargo_input_refusal(input: &input_manifest::InputManifest) -> CargoWorkspaceError {
    ompo_start::inception::cargo_workspace_member_from_output(b"{}", input)
        .expect_err("non-FULL input must halt before parsing")
}

#[test]
fn cargo_workspace_non_full_inputs_are_typed() {
    let cases = [
        (
            input_manifest::InputManifest::partial("package_head", 1, "do8n-fixture")
                .expect("valid bounded input"),
            true,
            "CARGO_WORKSPACE_INPUT_PARTIAL",
        ),
        (
            input_manifest::InputManifest::refused("fixture withheld metadata")
                .expect("valid refused input"),
            false,
            "CARGO_WORKSPACE_INPUT_REFUSED",
        ),
    ];
    for (input, partial, reason_code) in cases {
        let error = cargo_input_refusal(&input);
        assert_eq!(
            matches!(error, CargoWorkspaceError::PartialInput { .. }),
            partial,
            "PARTIAL and REFUSED remain distinct: {error}"
        );
        assert!(
            error.to_string().contains(reason_code),
            "wrong cause: {error}"
        );
    }
}
/// do8n wiring leg: the derived member verdict is consumed immediately after
/// git canonicalization. Every pre-existing trust gate retains its relative
/// order and no trust artifact exists when Cargo membership refuses.
#[test]
fn cargo_workspace_member_verdict_gates_l2_entry() {
    use ompo_start::inception::initialize_gated;
    let repository = repository_fixture();
    std::fs::write(
        repository.path().join("Cargo.toml"),
        b"[package]\nname = \"different\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
    )
    .expect("foreign Cargo package");
    let output = repository
        .path()
        .join(".omp-orchestrator/cargo-member-refused.json");
    let error = initialize_gated(repository.path(), &output, &TrustedInitConsent::Absent)
        .expect_err("foreign current package must halt at the L2 entry");
    assert!(
        matches!(
            &error,
            InceptionError::CargoWorkspace(CargoWorkspaceError::CurrentCrateAbsent { .. })
        ),
        "Cargo member gate must be the refusing layer, got: {error}"
    );
    assert!(
        error
            .to_string()
            .contains("CARGO_WORKSPACE_CURRENT_CRATE_ABSENT"),
        "wrong gated-entry cause: {error}"
    );
    assert!(!output.exists(), "Cargo refusal must precede trust writes");
}

fn agent_mail_case(marker: Option<&str>) -> TempDir {
    ensure_agent_mail_fixture_server();
    let directory = tempfile::tempdir().expect("Agent Mail case directory");
    if let Some(marker) = marker {
        std::fs::write(directory.path().join(marker), b"fixture\n")
            .expect("Agent Mail case marker");
    }
    directory
}

/// Exact registered identity is the known-good control. Every restrictive
/// response remains a separate typed cause with remediation; the input arms
/// halt before any service call.
#[test]
fn agent_mail_registration_exact_control_and_restrictive_matrix() {
    use input_manifest::InputManifest;
    use ompo_start::inception::{agent_mail_registration_report, AgentMailRegistrationError};

    let exact = agent_mail_case(None);
    let report = agent_mail_registration_report(exact.path(), &InputManifest::full())
        .expect("canonical project, roster, and pane binding agree");
    assert_eq!(
        (
            report.project.as_str(),
            report.pane_id.as_str(),
            report.agent.as_str()
        ),
        (
            exact.path().to_string_lossy().as_ref(),
            "%59",
            "BlackMeadow"
        )
    );

    type KindCheck = fn(&AgentMailRegistrationError) -> bool;
    let cases: [(&str, KindCheck, &str); 7] = [
        (
            ".agent-mail-project-missing",
            |error| matches!(error, AgentMailRegistrationError::ProjectMissing { .. }),
            "AGENT_MAIL_PROJECT_MISSING",
        ),
        (
            ".agent-mail-agent-missing",
            |error| matches!(error, AgentMailRegistrationError::AgentMissing { .. }),
            "AGENT_MAIL_AGENT_MISSING",
        ),
        (
            ".agent-mail-project-mismatch",
            |error| matches!(error, AgentMailRegistrationError::ProjectMismatch { .. }),
            "AGENT_MAIL_PROJECT_MISMATCH",
        ),
        (
            ".agent-mail-pane-mismatch",
            |error| matches!(error, AgentMailRegistrationError::PaneAgentMismatch { .. }),
            "AGENT_MAIL_PANE_AGENT_MISMATCH",
        ),
        (
            ".agent-mail-unavailable",
            |error| matches!(error, AgentMailRegistrationError::ServiceUnavailable { .. }),
            "AGENT_MAIL_SERVICE_UNAVAILABLE",
        ),
        (
            ".agent-mail-malformed",
            |error| matches!(error, AgentMailRegistrationError::MalformedResponse { .. }),
            "AGENT_MAIL_RESPONSE_MALFORMED",
        ),
        (
            ".agent-mail-unknown",
            |error| matches!(error, AgentMailRegistrationError::Unknown { .. }),
            "AGENT_MAIL_REGISTRATION_UNKNOWN",
        ),
    ];
    for (marker, is_expected_kind, code) in cases {
        let subject = agent_mail_case(Some(marker));
        let error = agent_mail_registration_report(subject.path(), &InputManifest::full())
            .expect_err("restrictive registration state must halt");
        assert!(is_expected_kind(&error), "wrong typed cause: {error}");
        let text = error.to_string();
        assert!(text.contains(code) && text.contains("remedy="), "{text}");
    }

    let bounded = agent_mail_case(None);
    let partial =
        InputManifest::partial("registry_rows", 1, "4xwl-fixture").expect("valid partial input");
    let refused =
        InputManifest::refused("fixture withheld registration").expect("valid refused input");
    let partial_error = agent_mail_registration_report(bounded.path(), &partial)
        .expect_err("PARTIAL registration input must halt before I/O");
    let refused_error = agent_mail_registration_report(bounded.path(), &refused)
        .expect_err("REFUSED registration input must halt before I/O");
    assert!(matches!(
        partial_error,
        AgentMailRegistrationError::PartialInput { .. }
    ));
    assert!(matches!(
        refused_error,
        AgentMailRegistrationError::RefusedInput { .. }
    ));
    assert!(partial_error
        .to_string()
        .contains("AGENT_MAIL_INPUT_PARTIAL"));
    assert!(refused_error
        .to_string()
        .contains("AGENT_MAIL_INPUT_REFUSED"));
}

/// 4xwl wiring leg: preserve all previous L2 gates, then consume the read-only
/// registration verdict after hook identity and before `initialize` writes.
/// Ignoring this Result lets the mismatch reach a trust artifact and reddens
/// only this entry leg; the exact-registration matrix above remains green.
#[test]
fn agent_mail_registration_verdict_gates_l2_entry() {
    use ompo_start::inception::{initialize_gated, AgentMailRegistrationError, InceptionError};
    let repository = repository_fixture();
    std::fs::write(
        repository.path().join("CLAUDE.md"),
        b"fixture omp-orchestrator\n",
    )
    .expect("stamped CLAUDE.md");
    std::fs::write(
        repository.path().join(".agent-mail-project-mismatch"),
        b"fixture\n",
    )
    .expect("project mismatch marker");
    let output = repository
        .path()
        .join(".omp-orchestrator/agent-mail-refused.json");
    match initialize_gated(repository.path(), &output, &TrustedInitConsent::Absent) {
        Err(InceptionError::AgentMailRegistration(
            AgentMailRegistrationError::ProjectMismatch { expected, actual },
        )) => assert_ne!(expected, actual, "mismatch cause collapsed"),
        Ok(report) => panic!("registration bypass wrote trust: {report:?}"),
        Err(other) => panic!("wrong L2 refusal: {other}"),
    }
    assert_eq!(
        std::fs::metadata(&output)
            .expect_err("refusal wrote no artifact")
            .kind(),
        std::io::ErrorKind::NotFound
    );
}
fn bx3q_doctor(no_records: bool) -> Vec<u8> {
    let (severity, code, message) = if no_records {
        (
            "info",
            "RCH-R303",
            "No worker repo-convergence records were reported",
        )
    } else {
        ("pass", "RCH-R300", "fixture convergence rows available")
    };
    serde_json::to_vec(&serde_json::json!({
        "api_version": "1.0",
        "command": "doctor.reliability",
        "success": true,
        "data": {
            "scope": ["topology", "convergence"],
            "diagnostics": [
                {
                    "category": "topology",
                    "check_name": "workers_config",
                    "severity": "pass",
                    "code": "RCH-R003",
                    "message": "fixture topology ready",
                    "details": "fixture",
                },
                {
                    "category": "repo_presence",
                    "check_name": "repo_convergence",
                    "severity": severity,
                    "code": code,
                    "message": message,
                    "details": if no_records { "status=unknown, total=0" } else { "status=ready" },
                },
            ],
        },
    }))
    .expect("doctor fixture JSON")
}

fn bx3q_worker(
    worker_id: &str,
    drift_state: &str,
    required_repos: &[&str],
    synced_repos: &[&str],
    missing_repos: &[&str],
) -> Value {
    serde_json::json!({
        "worker_id": worker_id,
        "drift_state": drift_state,
        "required_repos": required_repos,
        "synced_repos": synced_repos,
        "missing_repos": missing_repos,
    })
}

fn bx3q_status(status: &str, workers: Vec<Value>) -> Vec<u8> {
    let count = |state: &str| {
        workers
            .iter()
            .filter(|worker| worker["drift_state"].as_str() == Some(state))
            .count()
    };
    serde_json::to_vec(&serde_json::json!({
        "api_version": "1.0",
        "command": "status",
        "success": true,
        "data": {
            "convergence": {
                "status": status,
                "summary": {
                    "total_workers": workers.len(),
                    "ready": count("ready"),
                    "drifting": count("drifting"),
                    "converging": count("converging"),
                    "failed": count("failed"),
                    "stale": count("stale"),
                },
                "workers": workers,
            },
        },
    }))
    .expect("status fixture JSON")
}

fn bx3q_diagnose(excluded: bool) -> Vec<u8> {
    let selection = if excluded {
        serde_json::json!({
            "reason": "no_admissible_workers",
            "diagnostics": {
                "active_project_exclusion_count": 1,
                "workers": [{
                    "worker_id": "worker-a",
                    "active_project_excluded": true,
                }],
            },
        })
    } else {
        serde_json::json!({"reason": "selected"})
    };
    serde_json::to_vec(&serde_json::json!({
        "api_version": "1.0",
        "command": "diagnose",
        "success": true,
        "data": {
            "classification": {"is_compilation": true},
            "decision": {
                "would_intercept": !excluded,
                "reason": if excluded { "project_excluded" } else { "offload eligible" },
            },
            "worker_selection": selection,
        },
    }))
    .expect("diagnose fixture JSON")
}

#[test]
fn rch_lane_mapped_and_unknown_are_distinct() {
    use input_manifest::InputManifest;
    use ompo_start::inception::{rch_lane_report_from_outputs, RchLaneState};
    let repo = Path::new("/repo");
    let mapped_status = bx3q_status(
        "ready",
        vec![
            bx3q_worker("worker-a", "ready", &["repo"], &["repo"], &[]),
            bx3q_worker("worker-b", "ready", &["other"], &["other"], &[]),
        ],
    );
    let mapped = rch_lane_report_from_outputs(
        repo,
        &InputManifest::full(),
        &bx3q_doctor(false),
        &mapped_status,
        &bx3q_diagnose(false),
    )
    .expect("one topology-wide convergence row maps the repo");
    assert_eq!(
        mapped.state,
        RchLaneState::Mapped {
            workers: std::collections::BTreeSet::from(["worker-a".to_owned()]),
        }
    );

    let unknown = rch_lane_report_from_outputs(
        repo,
        &InputManifest::full(),
        &bx3q_doctor(true),
        &bx3q_status("unknown", Vec::new()),
        &bx3q_diagnose(false),
    )
    .expect("zero convergence records are explicit UNKNOWN, not absence or mapped");
    match unknown.state {
        RchLaneState::Unknown { cause } => {
            assert!(
                cause.contains("No worker repo-convergence records"),
                "{cause}"
            );
        }
        other => panic!("UNKNOWN was coerced to {other:?}"),
    }
}

#[test]
fn rch_lane_restrictive_causes_and_entry_wiring() {
    use input_manifest::InputManifest;
    use ompo_start::inception::{rch_lane_report_from_outputs, RchLaneError};
    let repo = Path::new("/repo");
    let doctor = bx3q_doctor(false);
    let diagnose = bx3q_diagnose(false);

    let malformed = rch_lane_report_from_outputs(
        repo,
        &InputManifest::full(),
        b"not-json",
        &bx3q_status("unknown", Vec::new()),
        &diagnose,
    )
    .expect_err("malformed doctor output must halt");
    assert!(matches!(
        malformed,
        RchLaneError::MalformedOutput {
            surface: "doctor",
            ..
        }
    ));

    let absent = rch_lane_report_from_outputs(
        repo,
        &InputManifest::full(),
        &doctor,
        &bx3q_status(
            "ready",
            vec![bx3q_worker(
                "worker-a",
                "ready",
                &["other"],
                &["other"],
                &[],
            )],
        ),
        &diagnose,
    )
    .expect_err("a known topology with no project row must halt");
    assert!(matches!(absent, RchLaneError::ProjectRowAbsent { .. }));

    let contradictory = rch_lane_report_from_outputs(
        repo,
        &InputManifest::full(),
        &doctor,
        &bx3q_status(
            "drifting",
            vec![bx3q_worker(
                "worker-a",
                "drifting",
                &["repo"],
                &["repo"],
                &["repo"],
            )],
        ),
        &diagnose,
    )
    .expect_err("synced and missing is contradictory");
    assert!(matches!(
        contradictory,
        RchLaneError::ContradictoryTopology { .. }
    ));

    let excluded = rch_lane_report_from_outputs(
        repo,
        &InputManifest::full(),
        &doctor,
        &bx3q_status(
            "ready",
            vec![bx3q_worker("worker-a", "ready", &["repo"], &["repo"], &[])],
        ),
        &bx3q_diagnose(true),
    )
    .expect_err("active project exclusion must halt");
    assert!(matches!(excluded, RchLaneError::ProjectExcluded { .. }));

    let partial =
        InputManifest::partial("convergence_rows", 1, "bx3q-fixture").expect("valid partial input");
    let refused = InputManifest::refused("fixture withheld topology").expect("valid refused input");
    assert!(matches!(
        rch_lane_report_from_outputs(repo, &partial, b"", b"", b""),
        Err(RchLaneError::PartialInput { .. })
    ));
    assert!(matches!(
        rch_lane_report_from_outputs(repo, &refused, b"", b"", b""),
        Err(RchLaneError::RefusedInput { .. })
    ));
    let entry_repo = repository_fixture();
    std::fs::write(
        entry_repo.path().join("CLAUDE.md"),
        b"fixture omp-orchestrator\n",
    )
    .expect("stamped CLAUDE.md");
    std::fs::write(
        entry_repo.path().join(".rch-project-excluded"),
        b"fixture\n",
    )
    .expect("project exclusion marker");
    let artifact = entry_repo.path().join(".omp-orchestrator/rch-refused.json");
    let entry_result = ompo_start::inception::initialize_gated(
        entry_repo.path(),
        &artifact,
        &TrustedInitConsent::Absent,
    );
    assert!(
        matches!(
            entry_result,
            Err(ompo_start::inception::InceptionError::RchLane(
                RchLaneError::ProjectExcluded { .. }
            ))
        ),
        "production entry discarded the RCH lane refusal: {entry_result:?}"
    );
    assert!(!artifact.try_exists().expect("artifact existence probe"));
}

#[test]
fn rch_process_missing_and_timeout_are_distinct() {
    use input_manifest::InputManifest;
    use ompo_start::inception::{rch_lane_report_with_program, RchLaneError};
    use std::os::unix::fs::PermissionsExt as _;
    let repository = tempfile::tempdir().expect("RCH process fixture");
    let missing = repository.path().join("missing-rch");
    assert!(matches!(
        rch_lane_report_with_program(repository.path(), &InputManifest::full(), &missing),
        Err(RchLaneError::MissingRch { .. })
    ));

    let slow = repository.path().join("slow-rch");
    std::fs::write(&slow, b"#!/bin/sh\nsleep 30\n").expect("slow RCH fixture");
    let mut permissions = std::fs::metadata(&slow)
        .expect("slow metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&slow, permissions).expect("slow executable");
    assert!(matches!(
        rch_lane_report_with_program(repository.path(), &InputManifest::full(), &slow),
        Err(RchLaneError::Timeout { surface: "doctor" })
    ));
}

#[test]
fn rch_lane_unknown_is_carried_by_gated_entry() {
    use ompo_start::inception::{initialize_gated, RchLaneState};
    let repository = repository_fixture();
    std::fs::write(
        repository.path().join("CLAUDE.md"),
        b"fixture omp-orchestrator\n",
    )
    .expect("stamped CLAUDE.md");
    let output = repository.path().join(".omp-orchestrator/rch-unknown.json");
    let outcome = initialize_gated(repository.path(), &output, &TrustedInitConsent::Absent);
    let carried_unknown = outcome.as_ref().is_ok_and(|report| {
        report
            .rch_lane
            .as_ref()
            .is_some_and(|lane| matches!(lane.state, RchLaneState::Unknown { .. }))
    });
    assert!(
        carried_unknown,
        "gated report lost exact UNKNOWN: {outcome:?}"
    );
    assert!(
        ompo_start::inception::read_inception(&output).is_ok(),
        "UNKNOWN continuation did not produce readable trust state"
    );
}

/// L2-TEST-TRUSTED-INIT: the reachable production entry treats foreign policy
/// without an explicit consent record as a typed halt before any output,
/// backup, or lifecycle write.
#[test]
fn foreign_policy_without_opt_in_is_read_only() {
    let repository = foreign_policy_fixture();
    let output = repository
        .path()
        .join(".omp-orchestrator/jlna-missing-consent.json");
    let error = initialize_gated(repository.path(), &output, &TrustedInitConsent::Absent)
        .expect_err("foreign policy without consent must halt");
    assert!(
        matches!(error, InceptionError::TrustedInitConsentMissing { .. }),
        "wrong refusal: {error:?}"
    );
    let message = error.to_string();
    assert!(
        message.starts_with("HUMAN_HALT TRUSTED_INIT_CONSENT_MISSING")
            && message.contains("remedy="),
        "missing consent needs typed remediation: {message}"
    );
    assert_no_init_residue(repository.path(), &output);
}

#[test]
fn trusted_init_consent_causes_are_distinct() {
    let legacy = foreign_policy_fixture();
    let legacy_output = legacy.path().join(".omp-orchestrator/jlna-foreign.json");
    let foreign = initialize(legacy.path(), &legacy_output)
        .expect_err("legacy entry must expose foreign policy");
    assert!(matches!(foreign, InceptionError::UntrustedAgentsMd { .. }));
    assert!(
        foreign.to_string().contains("TRUSTED_INIT_FOREIGN_POLICY"),
        "foreign policy cause collapsed: {foreign}"
    );
    assert_no_init_residue(legacy.path(), &legacy_output);

    let malformed_repo = foreign_policy_fixture();
    let malformed_output = malformed_repo
        .path()
        .join(".omp-orchestrator/jlna-malformed.json");
    let mut malformed =
        explicit_trusted_init_consent(malformed_repo.path(), "valid-before-mutation");
    let TrustedInitConsent::Explicit { decision_id, .. } = &mut malformed else {
        unreachable!("helper returns explicit consent")
    };
    decision_id.clear();
    let malformed_error = initialize_gated(malformed_repo.path(), &malformed_output, &malformed)
        .expect_err("malformed consent must halt");
    assert!(matches!(
        malformed_error,
        InceptionError::TrustedInitConsentMalformed {
            field: "decision_id",
            ..
        }
    ));
    assert!(malformed_error
        .to_string()
        .contains("TRUSTED_INIT_CONSENT_MALFORMED"));
    assert_no_init_residue(malformed_repo.path(), &malformed_output);

    let scope_repo = foreign_policy_fixture();
    let scope_output = scope_repo.path().join(".omp-orchestrator/jlna-scope.json");
    let mut wrong_scope = explicit_trusted_init_consent(scope_repo.path(), "scope-decision");
    let TrustedInitConsent::Explicit {
        repository_scope, ..
    } = &mut wrong_scope
    else {
        unreachable!("helper returns explicit consent")
    };
    *repository_scope = Path::new("/different/repository").to_owned();
    let scope_error = initialize_gated(scope_repo.path(), &scope_output, &wrong_scope)
        .expect_err("wrong consent scope must halt");
    assert!(matches!(
        scope_error,
        InceptionError::TrustedInitConsentScopeMismatch { .. }
    ));
    assert!(scope_error
        .to_string()
        .contains("TRUSTED_INIT_CONSENT_SCOPE_MISMATCH"));
    assert_no_init_residue(scope_repo.path(), &scope_output);

    let stale_repo = foreign_policy_fixture();
    let stale_output = stale_repo.path().join(".omp-orchestrator/jlna-stale.json");
    let stale = explicit_trusted_init_consent(stale_repo.path(), "stale-decision");
    std::fs::write(
        stale_repo.path().join("AGENTS.md"),
        b"changed foreign policy\n",
    )
    .expect("policy changes after consent");
    let stale_error = initialize_gated(stale_repo.path(), &stale_output, &stale)
        .expect_err("replayed consent must halt");
    assert!(matches!(
        stale_error,
        InceptionError::TrustedInitConsentStaleOrReplayed { .. }
    ));
    assert!(stale_error
        .to_string()
        .contains("TRUSTED_INIT_CONSENT_STALE_OR_REPLAYED"));
    assert_no_init_residue(stale_repo.path(), &stale_output);
}

#[test]
fn valid_scoped_trusted_init_consent_proceeds() {
    let repository = foreign_policy_fixture();
    let output = repository
        .path()
        .join(".omp-orchestrator/jlna-valid-consent.json");
    let consent = explicit_trusted_init_consent(repository.path(), "jlna-valid");
    let report = initialize_gated(repository.path(), &output, &consent)
        .expect("current scoped consent must proceed");
    assert!(output.is_file(), "valid consent did not produce output");
    assert!(matches!(
        report.trusted_init,
        TrustedInitDecision::ExplicitConsent { ref decision_id, .. }
            if decision_id == "jlna-valid"
    ));
}

#[test]
fn trusted_init_records_template_identity_before_write() {
    let repository = foreign_policy_fixture();
    let output = repository
        .path()
        .join(".omp-orchestrator/8qaz-template.json");
    let consent = explicit_trusted_init_consent(repository.path(), "8qaz-valid");
    let report = initialize_gated(repository.path(), &output, &consent)
        .expect("valid template identity consent proceeds");
    let readback = read_inception(&output).expect("durable readback");
    let identity = readback
        .template_identity
        .as_ref()
        .expect("template identity is durable");
    let TrustedInitConsent::Explicit { template_path, .. } = &consent else {
        unreachable!("helper returns explicit consent")
    };
    assert_eq!(
        identity.canonical_path,
        template_path.canonicalize().unwrap().display().to_string()
    );
    assert_eq!(identity.source_sha256.len(), 64);
    assert!(!identity.source_revision.is_empty());
    assert_eq!(report.manifest.template_identity.as_ref(), Some(identity));
}

#[test]
fn template_identity_restrictions_are_typed_and_prewrite() {
    use input_manifest::InputManifest;
    use ompo_start::inception::{
        capture_template_identity, verify_template_identity_current, TemplateIdentityError,
    };

    let repository = foreign_policy_fixture();
    let missing = capture_template_identity(
        repository.path(),
        &repository.path().join("missing-template.md"),
        &InputManifest::full(),
    )
    .expect_err("missing template must refuse");
    assert!(matches!(missing, TemplateIdentityError::Missing { .. }));

    let escape = capture_template_identity(
        repository.path(),
        Path::new("/etc/hosts"),
        &InputManifest::full(),
    )
    .expect_err("authority escape must refuse");
    assert!(matches!(
        escape,
        TemplateIdentityError::AuthorityRootEscape { .. }
    ));

    let partial = InputManifest::partial("template_files", 1, "8qaz-test").expect("partial input");
    let partial_error = capture_template_identity(
        repository.path(),
        &repository.path().join("template.md"),
        &partial,
    )
    .expect_err("partial input must refuse");
    assert!(matches!(
        partial_error,
        TemplateIdentityError::PartialInput { .. }
    ));

    let refused = InputManifest::refused("template source withheld").expect("refused input");
    let refused_error = capture_template_identity(
        repository.path(),
        &repository.path().join("template.md"),
        &refused,
    )
    .expect_err("refused input must refuse");
    assert!(matches!(
        refused_error,
        TemplateIdentityError::RefusedInput { .. }
    ));

    let empty = repository.path().join("empty-template.md");
    std::fs::write(&empty, b"").expect("empty template");
    let empty_error = capture_template_identity(repository.path(), &empty, &InputManifest::full())
        .expect_err("empty template must refuse");
    assert!(matches!(empty_error, TemplateIdentityError::Empty { .. }));

    let identity = capture_template_identity(
        repository.path(),
        &repository.path().join("template.md"),
        &InputManifest::full(),
    )
    .expect("baseline identity");
    std::fs::write(
        repository.path().join("template.md"),
        b"changed template
",
    )
    .expect("change template after capture");
    let changed = verify_template_identity_current(repository.path(), &identity)
        .expect_err("changed source must refuse");
    assert!(matches!(
        changed,
        TemplateIdentityError::SourceChanged { .. }
    ));

    let unresolvable = tempfile::tempdir().expect("unresolvable fixture");
    let unresolvable_template = unresolvable.path().join("template.md");
    std::fs::write(
        &unresolvable_template,
        b"template source
",
    )
    .expect("template bytes");
    run_git(unresolvable.path(), &["init", "-q"]);
    let revision_error = capture_template_identity(
        unresolvable.path(),
        &unresolvable_template,
        &InputManifest::full(),
    )
    .expect_err("missing HEAD must refuse");
    assert!(matches!(
        revision_error,
        TemplateIdentityError::RevisionUnresolvable { .. }
    ));
}

#[test]
fn template_source_change_is_refused_before_artifact_write() {
    let repository = foreign_policy_fixture();
    let output = repository
        .path()
        .join(".omp-orchestrator/8qaz-source-change.json");
    let consent = explicit_trusted_init_consent(repository.path(), "8qaz-source-change");
    use ompo_start::inception::TemplateIdentityError;
    std::fs::write(
        repository.path().join("template.md"),
        b"changed before write
",
    )
    .expect("change template before init");
    let error = initialize_gated(repository.path(), &output, &consent)
        .expect_err("source change must refuse before write");
    assert!(
        matches!(error, InceptionError::TemplateIdentity(TemplateIdentityError::SourceChanged { .. }))
            || matches!(error, InceptionError::TemplateIdentity(_))
    );
    assert_no_init_residue(repository.path(), &output);
}

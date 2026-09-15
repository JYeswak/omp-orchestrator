#![forbid(unsafe_code)]

//! L2-BUILD-INCEPTION-FOUNDATION (bead 4228): one accepted `ompo init`
//! emits `.omp-orchestrator/inception.json` AND a linked `stage="S1"`
//! `FOUNDATION.jsonl` row before reporting success.
//!
//! The defect this closes: `append_s1_foundation` was fully built with a
//! standalone schema/idempotency proof (`l5_foundation.rs`) and exactly one
//! production caller (`src/bin/foundation_append.rs::main`) -- BUILT but not
//! composed. No accepted-init caller existed, so the S1 row never landed on
//! the operator path. The composition lives in `run_init`
//! (`crates/ompo-doctor/src/main.rs`); these legs drive the SHIPPED `ompo`
//! binary, so a green here proves the CLI trigger, not a helper.
//!
//! Fixture shape mirrors `repository_fixture` + the Agent Mail / RCH fixture
//! servers in `crates/ompo-start/tests/l2_ecosystem.rs` (the proven passing
//! shape for `initialize_gated`): same control files, same stamp rule (token
//! REFERENCED via import, never copied), same hook manifest boundary, same
//! mock env. Duplicated rather than shared because integration test targets
//! cannot import each other's helpers.
//!
//! KNOWN-BAD (single-edit mutation): delete/bypass ONLY the
//! `append_init_foundation` call in `run_init`. The named linkage leg below
//! must RED with a `FOUNDATION_LINKAGE` message and exit 101, while the
//! refused-neither control in this file AND the standalone writer control
//! (`-p ompo-start --test l5_foundation`) stay green.

use ompo_start::inception::PROJECT_AGENTS_OWNERSHIP_STAMP;
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

const HOOK_SOURCE_PATH: &str = "crates/agent-mail-native/src/lib.rs";
const HOOK_SOURCE_BYTES: &[u8] = b"pub fn hook_fixture() {}\n";

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        use std::fmt::Write as _;
        write!(hex, "{byte:02x}").expect("hex write");
    }
    hex
}

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
        let agents = vec![serde_json::json!({"name": "BlackMeadow"})];
        let payload = serde_json::json!({
            "project": {"slug": "fixture", "human_key": project.as_str()},
            "agents": agents,
        })
        .to_string();
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {"contents": [{"uri": format!("resource://agents/{project}"), "text": payload}]}
        })
    } else if method == "tools/call"
        && request["params"]["name"].as_str() == Some("resolve_pane_identity")
    {
        let payload = serde_json::json!({
            "pane_id": pane,
            "binding": "verified-live",
            "agent_name": "BlackMeadow",
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

static MOCK_URL: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
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

fn ensure_agent_mail_fixture_server() {
    std::env::set_var("AM_MCP_URL", MOCK_URL.as_str());
    std::env::set_var("AGENT_MAIL_BEARER_TOKEN", "fixture-token");
    std::env::set_var("AGENT_MAIL_AGENT", "BlackMeadow");
    std::env::set_var("TMUX_PANE", "%59");
}

static RCH_BIN: std::sync::LazyLock<PathBuf> = std::sync::LazyLock::new(|| {
    use std::os::unix::fs::PermissionsExt as _;
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .expect("test HOME");
    let directory = home
        .join(".local/state/zeststream/scratch/omp-orchestrator")
        .join(format!("ompo-doctor-test-{}", std::process::id()))
        .join("init-foundation-4228");
    std::fs::create_dir_all(&directory).expect("RCH fixture directory");
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
    let mut permissions = std::fs::metadata(&binary)
        .expect("RCH fixture metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&binary, permissions).expect("RCH fixture executable");
    binary
});

fn ensure_rch_fixture_program() {
    let directory = RCH_BIN.parent().expect("RCH fixture parent");
    let mut paths =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect::<Vec<_>>();
    if !paths.iter().any(|path| path == directory) {
        paths.insert(0, directory.to_owned());
        std::env::set_var("PATH", std::env::join_paths(paths).expect("fixture PATH"));
    }
}

fn run_git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(repo)
        .args(args)
        .status()
        .expect("git must exist for repo fixtures");
    assert!(status.success(), "git {args:?} failed in fixture");
}

/// Passing-shape repo for the accepted gated entry: mirrors the proven
/// `repository_fixture` (control files, stamp rule, beads, pin, hook
/// manifest) plus the qruz CLAUDE.md stamp so the leg measures composition,
/// not the stamp gate.
fn gated_repo_fixture() -> TempDir {
    ensure_agent_mail_fixture_server();
    ensure_rch_fixture_program();
    let directory = tempfile::tempdir().expect("fixture directory");
    std::fs::create_dir(directory.path().join("docs")).expect("docs directory");
    for name in ["CLAUDE.md", "README.md", "SCHEMAS.toml"] {
        std::fs::write(directory.path().join(name), b"fixture\n").expect("control file");
    }
    // Stamp rule: the token is REFERENCED, never copied.
    let stamped = format!("fixture {PROJECT_AGENTS_OWNERSHIP_STAMP}\n");
    std::fs::write(directory.path().join("AGENTS.md"), &stamped).expect("stamped AGENTS.md");
    std::fs::write(directory.path().join("CLAUDE.md"), &stamped).expect("stamped CLAUDE.md");
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
    std::fs::create_dir(directory.path().join(".beads")).expect("beads dir");
    std::fs::write(
        directory.path().join(".beads/issues.jsonl"),
        "{\"id\":\"fixture-0001\",\"title\":\"fixture\"}\n",
    )
    .expect("beads issues");
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
    for args in [
        ["init", "-q"].as_slice(),
        ["add", "."].as_slice(),
        [
            "-c",
            "user.name=ompo-doctor-test",
            "-c",
            "user.email=ompo-doctor-test@example.invalid",
            "commit",
            "-qm",
            "fixture",
        ]
        .as_slice(),
    ] {
        run_git(directory.path(), args);
    }
    // Installed hook manifest: digest computed at runtime from the committed
    // bytes (never transcribed); the glued `adjacent-rodata` suffix carries
    // the Mach-O `strings` boundary the proven fixture depends on.
    let digest = sha256_hex(HOOK_SOURCE_BYTES);
    let hook = directory.path().join(".git/hooks/pre-commit");
    std::fs::create_dir_all(hook.parent().expect("hook parent")).expect("hook directory");
    std::fs::write(
        &hook,
        format!("{HOOK_SOURCE_PATH} {digest}adjacent-rodata\n"),
    )
    .expect("hook manifest artifact");
    directory
}

fn ompo(args: &[&str]) -> (Option<i32>, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ompo"))
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("ANTI-VACUITY: ompo did not execute: {error}"));
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// NAMED LINKAGE LEG (mutation target): one accepted `ompo init` publishes
/// inception AND appends exactly one linked stage=S1 row; an identical rerun
/// stays one row. Deleting/bypassing ONLY the `append_init_foundation` call
/// must RED this leg with a `FOUNDATION_LINKAGE` message while the
/// refused-neither control below and the standalone writer control
/// (`-p ompo-start --test l5_foundation`) stay green.
#[test]
fn composed_init_emits_linked_foundation_row_and_stays_idempotent() {
    let repo = gated_repo_fixture();
    let root = repo.path();
    let inception = root.join(".omp-orchestrator/inception.json");
    let foundation = root.join("docs/plan/FOUNDATION.jsonl");

    let (code, stdout, stderr) = ompo(&["init", "--repo", &root.display().to_string(), "--json"]);
    assert_eq!(
        code,
        Some(0),
        "FOUNDATION_LINKAGE: first init must succeed, stderr: {stderr}"
    );
    let value: Value = serde_json::from_str(&stdout).expect("FOUNDATION_LINKAGE: init emits JSON");
    assert!(
        value
            .get("data")
            .and_then(|d| d.get("foundation_rows"))
            .is_some(),
        "FOUNDATION_LINKAGE: the init receipt must carry foundation_rows -- \
         without the composition this key is absent and the CLI is unwired: {stdout}"
    );
    assert_eq!(
        value["data"]["foundation_rows"], 1,
        "FOUNDATION_LINKAGE: first init links exactly one row: {stdout}"
    );
    assert_eq!(
        value["data"]["foundation_appended"], true,
        "FOUNDATION_LINKAGE: first init must append: {stdout}"
    );
    assert!(
        inception.is_file(),
        "FOUNDATION_LINKAGE: inception artifact missing after accepted init"
    );
    let text = std::fs::read_to_string(&foundation)
        .expect("FOUNDATION_LINKAGE: foundation artifact missing after accepted init");
    let rows = ompo_start::s1_rows_citing_inception(&text);
    assert_eq!(
        rows.len(),
        1,
        "FOUNDATION_LINKAGE: exactly one stage=S1 row must cite inception.json, got {}",
        rows.len()
    );
    let refs = rows[0]
        .get("output_refs")
        .and_then(Value::as_array)
        .expect("FOUNDATION_LINKAGE: linked row carries output_refs");
    assert!(
        refs.iter()
            .any(|r| r.as_str() == Some(ompo_start::INCEPTION_REF)),
        "FOUNDATION_LINKAGE: row must cite the exact inception path {refs:?}"
    );

    // Identical rerun: success again, still exactly one row (idempotency).
    let (code2, stdout2, stderr2) =
        ompo(&["init", "--repo", &root.display().to_string(), "--json"]);
    assert_eq!(
        code2,
        Some(0),
        "FOUNDATION_LINKAGE: second init must succeed, stderr: {stderr2}"
    );
    let value2: Value =
        serde_json::from_str(&stdout2).expect("FOUNDATION_LINKAGE: rerun emits JSON");
    assert_eq!(
        value2["data"]["foundation_appended"], false,
        "FOUNDATION_LINKAGE: rerun must not duplicate the row: {stdout2}"
    );
    let text2 = std::fs::read_to_string(&foundation).expect("FOUNDATION_LINKAGE: rerun reads back");
    assert_eq!(
        ompo_start::s1_rows_citing_inception(&text2).len(),
        1,
        "FOUNDATION_LINKAGE: rerun must leave exactly one linked row"
    );
    // Custom output path: the composed row must cite the emitted path, not the default constant.
    let custom_repo = gated_repo_fixture();
    let custom_root = custom_repo.path();
    let custom_inception = custom_root.join(".omp-orchestrator/custom-inception.json");
    let (custom_code, custom_stdout, custom_stderr) = ompo(&[
        "init",
        "--repo",
        &custom_root.display().to_string(),
        "--output",
        &custom_inception.display().to_string(),
        "--json",
    ]);
    assert_eq!(
        custom_code,
        Some(0),
        "FOUNDATION_LINKAGE: custom-output init must succeed, stderr: {custom_stderr}"
    );
    let custom_value: Value =
        serde_json::from_str(&custom_stdout).expect("FOUNDATION_LINKAGE: custom init JSON");
    assert_eq!(custom_value["data"]["foundation_rows"], 1);
    let custom_foundation = custom_root.join("docs/plan/FOUNDATION.jsonl");
    let custom_text = std::fs::read_to_string(&custom_foundation)
        .expect("FOUNDATION_LINKAGE: custom foundation artifact");
    let custom_rows = ompo_start::s1_rows_citing_inception(&custom_text);
    let custom_ref = custom_inception.display().to_string();
    assert_eq!(custom_rows.len(), 1);
    assert!(
        custom_rows[0]
            .get("output_refs")
            .and_then(Value::as_array)
            .is_some_and(|refs| refs.iter().any(|r| r.as_str() == Some(custom_ref.as_str()))),
        "FOUNDATION_LINKAGE: custom row must cite emitted inception path {custom_ref}"
    );
    println!(
        "READBACK composed init ok rows=1 idempotent foundation={}",
        foundation.display()
    );
}

/// CONTROL (green under the linkage mutation): a refused init emits NEITHER
/// artifact. The refusal arm never reaches the composition, so removing the
/// append call cannot move this leg -- it proves the binary is intact, not
/// that the linkage exists.
#[test]
fn refused_init_emits_neither_artifact() {
    let bare = tempfile::tempdir().expect("bare fixture");
    let inception = bare.path().join(".omp-orchestrator/inception.json");
    let foundation = bare.path().join("docs/plan/FOUNDATION.jsonl");
    let (code, _stdout, stderr) = ompo(&["init", "--repo", &bare.path().display().to_string()]);
    assert!(
        code.is_some_and(|c| c != 0),
        "a bare directory must refuse init"
    );
    assert!(
        stderr.contains("ompo init:"),
        "the refusal must be typed by the init command, got: {stderr}"
    );
    assert!(
        !inception.exists(),
        "refused init must not publish inception"
    );
    assert!(
        !foundation.exists(),
        "refused init must not append foundation"
    );
    println!("READBACK refused init emits neither artifact");
}

/// NAMED ATOMICITY LEG 1 (mutation target): virgin inception plus an
/// injected FOUNDATION failure removes the new artifact and reports both
/// causes. Bypassing the rollback call must RED this leg (the new
/// inception file remains) while the healthy composed control stays green.
#[test]
fn rollback_removes_virgin_inception_on_foundation_failure() {
    let repo = gated_repo_fixture();
    let root = repo.path();
    let inception = root.join(".omp-orchestrator/inception.json");
    let foundation = root.join("docs/plan/FOUNDATION.jsonl");
    // Real filesystem seam: a directory at the artifact path fails the
    // append read with EISDIR on every uid -- no mocks, no permissions.
    std::fs::create_dir_all(&foundation).expect("directory seam at foundation path");
    let (code, _stdout, stderr) = ompo(&["init", "--repo", &root.display().to_string()]);
    assert_eq!(
        code,
        Some(1),
        "ROLLBACK_ATOMIC: foundation failure must refuse with exit 1, stderr: {stderr}"
    );
    assert!(
        stderr.contains("FOUNDATION_APPEND_FAILED"),
        "ROLLBACK_ATOMIC: the original foundation cause must survive, got: {stderr}"
    );
    assert!(
        stderr.contains("ROLLBACK_OK inception=removed foundation=unchanged"),
        "ROLLBACK_ATOMIC: the rollback receipt must name both outcomes, got: {stderr}"
    );
    assert!(
        !inception.exists(),
        "ROLLBACK_ATOMIC: virgin inception must be removed after rollback"
    );
    assert!(
        foundation.is_dir(),
        "ROLLBACK_ATOMIC: the foundation seam must stand untouched"
    );
    println!("READBACK virgin rollback removed inception, foundation unchanged");
}

/// NAMED ATOMICITY LEG 2 (mutation target): pre-existing inception plus an
/// injected FOUNDATION failure restores exact pre-call bytes, and a later
/// retry with the seam fixed succeeds. Bypassing the rollback call must RED
/// this leg (replaced bytes persist) while the healthy control stays green.
#[test]
fn rollback_restores_preexisting_inception_byte_exact_and_retry_succeeds() {
    let repo = gated_repo_fixture();
    let root = repo.path();
    let inception = root.join(".omp-orchestrator/inception.json");
    let foundation = root.join("docs/plan/FOUNDATION.jsonl");
    let (clean, _, clean_stderr) = ompo(&["init", "--repo", &root.display().to_string()]);
    assert_eq!(
        clean,
        Some(0),
        "ROLLBACK_ATOMIC: clean init must succeed first, stderr: {clean_stderr}"
    );
    // Tamper so the rerun takes the replace path (actions == 1, backup).
    // The rollback target is these exact pre-call bytes, not a good state.
    std::fs::write(&inception, b"tampered\n").expect("tamper inception");
    let tampered_hash = sha256_hex(b"tampered\n");
    std::fs::remove_file(&foundation).expect("remove foundation file");
    std::fs::create_dir(&foundation).expect("directory seam at foundation path");
    let (code, _stdout, stderr) = ompo(&["init", "--repo", &root.display().to_string()]);
    assert_eq!(
        code,
        Some(1),
        "ROLLBACK_ATOMIC: foundation failure must refuse with exit 1, stderr: {stderr}"
    );
    assert!(
        stderr.contains("ROLLBACK_OK inception=restored foundation=unchanged"),
        "ROLLBACK_ATOMIC: the rollback receipt must name both outcomes, got: {stderr}"
    );
    let restored = std::fs::read(&inception).expect("ROLLBACK_ATOMIC: inception readable");
    assert_eq!(
        sha256_hex(&restored),
        tampered_hash,
        "ROLLBACK_ATOMIC: pre-existing inception must be byte-exact pre-call state"
    );
    assert!(
        foundation.is_dir(),
        "ROLLBACK_ATOMIC: the foundation seam must stand untouched"
    );
    // Fix the seam: the retry deterministically succeeds with linkage.
    std::fs::remove_dir(&foundation).expect("remove directory seam");
    let (retry, _, retry_stderr) = ompo(&["init", "--repo", &root.display().to_string()]);
    assert_eq!(
        retry,
        Some(0),
        "ROLLBACK_ATOMIC: retry after seam fix must succeed, stderr: {retry_stderr}"
    );
    let text =
        std::fs::read_to_string(&foundation).expect("ROLLBACK_ATOMIC: foundation readable");
    assert_eq!(
        ompo_start::s1_rows_citing_inception(&text).len(),
        1,
        "ROLLBACK_ATOMIC: retry must link exactly one row"
    );
    println!("READBACK replace rollback byte-exact, retry linked");
}

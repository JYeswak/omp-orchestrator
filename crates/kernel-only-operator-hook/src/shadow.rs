#![forbid(unsafe_code)]

//! Append-only shadow evidence for the Stage 4 operator hook.
//!
//! Shadow evidence is diagnostic only: it is not hook certification and never changes the
//! fail-closed default. The append happens synchronously in the caller's asupersync future; no
//! detached task is created, and a ledger failure cannot turn shadow mode into enforcement.

use asupersync::process::{Command, Output};
use asupersync::time::timeout;
use asupersync::Cx;
use kernel_only_operator_hook::{Decision, Permission};
use serde::Serialize;
use serde_json::Value;
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use subprocess_contract::{run_output, RunError};

pub const DEFAULT_LEDGER_PATH: &str =
    "/Users/josh/.local/state/flywheel/kernel-only-operator-hook-shadow-verdicts.jsonl";
const SCHEMA_VERSION: &str = "kernel-only-operator-hook-shadow.v1";
const EVENT: &str = "PreToolUse";
const HOOK_ID: &str = "kernel-only-operator-hook";
const STAGE: u8 = 4;
const PREDECESSOR_DEADLINE: Duration = Duration::from_millis(250);

#[cfg(target_os = "macos")]
const PREDECESSOR_SCRIPT: &str = "printf '%s' \"$1\" | /usr/bin/base64 -D | \"$0\" --json";
#[cfg(not(target_os = "macos"))]
const PREDECESSOR_SCRIPT: &str = "printf '%s' \"$1\" | /usr/bin/base64 -d | \"$0\" --json";

#[derive(Debug, Serialize)]
struct ShadowVerdict<'a> {
    schema_version: &'static str,
    event: &'static str,
    hook_id: &'static str,
    stage: u8,
    build_id: &'a str,
    ts_unix: u64,
    pid: u32,
    effective_mode: &'a str,
    tool_name: String,
    command_sha256: String,
    input_sha256: String,
    session_id: String,
    turn_id: String,
    tool_use_id: String,
    transcript_path: String,
    cwd: String,
    rust_permission: &'static str,
    rust_reason: &'a str,
    predecessor_outcome: String,
    predecessor_reason_code: String,
    predecessor_exit_status: String,
    predecessor_stdout_sha256: String,
    predecessor_stderr_sha256: String,
    parity: String,
    review_status: &'static str,
    live_bash_parity: &'static str,
}

#[derive(Debug)]
struct InputMetadata {
    tool_name: String,
    command_sha256: String,
    input_sha256: String,
    session_id: String,
    turn_id: String,
    tool_use_id: String,
    transcript_path: String,
    cwd: String,
}

#[derive(Debug)]
struct PredecessorObservation {
    outcome: String,
    reason_code: String,
    exit_status: String,
    stdout_sha256: String,
    stderr_sha256: String,
    parity: String,
}

impl PredecessorObservation {
    fn not_configured() -> Self {
        Self {
            outcome: "not_configured".to_owned(),
            reason_code: "not_configured".to_owned(),
            exit_status: "not_run".to_owned(),
            stdout_sha256: String::new(),
            stderr_sha256: String::new(),
            parity: "not_applicable".to_owned(),
        }
    }

    fn unavailable(outcome: &str, reason_code: &str, exit_status: &str) -> Self {
        Self {
            outcome: outcome.to_owned(),
            reason_code: reason_code.to_owned(),
            exit_status: exit_status.to_owned(),
            stdout_sha256: String::new(),
            stderr_sha256: String::new(),
            parity: "unknown".to_owned(),
        }
    }
}

/// Append one JSON object with one O_APPEND write. The command is hashed, never stored.
pub fn append_verdict(input: &[u8], decision: &Decision, build_id: &str) -> io::Result<()> {
    append_verdict_mode(
        input,
        decision,
        build_id,
        "shadow",
        PredecessorObservation::not_configured(),
    )
}

/// Append a shadow row with an explicit effective mode and no predecessor result.
pub fn append_verdict_with_mode(
    input: &[u8],
    decision: &Decision,
    build_id: &str,
    effective_mode: &str,
) -> io::Result<()> {
    append_verdict_mode(
        input,
        decision,
        build_id,
        effective_mode,
        PredecessorObservation::not_configured(),
    )
}

fn append_verdict_mode(
    input: &[u8],
    decision: &Decision,
    build_id: &str,
    effective_mode: &str,
    predecessor: PredecessorObservation,
) -> io::Result<()> {
    let metadata = input_metadata(input);
    let verdict = ShadowVerdict {
        schema_version: SCHEMA_VERSION,
        event: EVENT,
        hook_id: HOOK_ID,
        stage: STAGE,
        build_id,
        ts_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0),
        pid: std::process::id(),
        effective_mode,
        tool_name: metadata.tool_name,
        command_sha256: metadata.command_sha256,
        input_sha256: metadata.input_sha256,
        session_id: metadata.session_id,
        turn_id: metadata.turn_id,
        tool_use_id: metadata.tool_use_id,
        transcript_path: metadata.transcript_path,
        cwd: metadata.cwd,
        rust_permission: permission_name(decision.permission),
        rust_reason: &decision.reason,
        predecessor_outcome: predecessor.outcome,
        predecessor_reason_code: predecessor.reason_code,
        predecessor_exit_status: predecessor.exit_status,
        predecessor_stdout_sha256: predecessor.stdout_sha256,
        predecessor_stderr_sha256: predecessor.stderr_sha256,
        parity: predecessor.parity,
        review_status: "unknown",
        live_bash_parity: "not_applicable",
    };

    let mut line = serde_json::to_vec(&verdict)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    line.push(b'\n');

    let path = ledger_path();
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let mut ledger = OpenOptions::new().create(true).append(true).open(path)?;
    ledger.write_all(&line)
}

/// Compare the exact bounded hook bytes with an existing predecessor, then append only
/// non-sensitive metadata and hashes. The comparison is observational and cannot affect the
/// Rust decision or the shadow allow response.
pub async fn append_verdict_with_predecessor(
    cx: &Cx,
    input: &[u8],
    decision: &Decision,
    build_id: &str,
    predecessor: &Path,
) -> io::Result<()> {
    let observation = compare_predecessor(cx, input, decision, predecessor).await;
    append_verdict_mode(input, decision, build_id, "shadow", observation)
}

async fn compare_predecessor(
    cx: &Cx,
    input: &[u8],
    decision: &Decision,
    predecessor: &Path,
) -> PredecessorObservation {
    // run_output owns the child process group and drains stdout and stderr. Because it exposes no
    // stdin override, the wrapper decodes this in-memory base64 argument into the predecessor's
    // stdin; the original bytes are never written to a file or a ledger row.
    let encoded_input = base64_encode(input);
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", PREDECESSOR_SCRIPT])
        .arg(predecessor)
        .arg(encoded_input);

    match timeout(
        cx.now_for_observability(),
        PREDECESSOR_DEADLINE,
        run_output(cx, command),
    )
    .await
    {
        Ok(Ok(output)) => predecessor_observation(output, decision),
        Ok(Err(RunError::Timeout)) => {
            eprintln!(
                "KERNEL_HOOK_SHADOW_PREDECESSOR_TIMEOUT: path={} deadline_ms={}",
                predecessor.display(),
                PREDECESSOR_DEADLINE.as_millis()
            );
            PredecessorObservation::unavailable("timeout", "deadline_exceeded", "timeout")
        }
        Ok(Err(error)) => {
            eprintln!(
                "KERNEL_HOOK_SHADOW_PREDECESSOR_ERROR: path={} error={error}",
                predecessor.display()
            );
            PredecessorObservation::unavailable("error", "process_error", "error")
        }
        Err(_) => {
            eprintln!(
                "KERNEL_HOOK_SHADOW_PREDECESSOR_TIMEOUT: path={} deadline_ms={}",
                predecessor.display(),
                PREDECESSOR_DEADLINE.as_millis()
            );
            PredecessorObservation::unavailable("timeout", "deadline_exceeded", "timeout")
        }
    }
}

fn predecessor_observation(output: Output, decision: &Decision) -> PredecessorObservation {
    let stdout_sha256 = sha256_hex(&output.stdout);
    let stderr_sha256 = sha256_hex(&output.stderr);
    let exit_status = output
        .status
        .code()
        .map_or_else(|| "signal".to_owned(), |code| code.to_string());

    let value = match serde_json::from_slice::<Value>(&output.stdout) {
        Ok(value) => value,
        Err(_) => {
            return PredecessorObservation {
                outcome: if output.status.success() {
                    "invalid_output".to_owned()
                } else {
                    "exit_failure".to_owned()
                },
                reason_code: if output.status.success() {
                    "invalid_json".to_owned()
                } else {
                    format!("exit_{exit_status}")
                },
                exit_status,
                stdout_sha256,
                stderr_sha256,
                parity: "unknown".to_owned(),
            };
        }
    };
    let Some(predecessor_permission) = predecessor_permission(&value) else {
        return PredecessorObservation {
            outcome: if output.status.success() {
                "invalid_output".to_owned()
            } else {
                "exit_failure".to_owned()
            },
            reason_code: if output.status.success() {
                "missing_permission_decision".to_owned()
            } else {
                format!("exit_{exit_status}")
            },
            exit_status,
            stdout_sha256,
            stderr_sha256,
            parity: "unknown".to_owned(),
        };
    };

    let outcome = permission_name(predecessor_permission).to_owned();
    let parity = if predecessor_permission == decision.permission {
        "match"
    } else {
        "mismatch"
    };
    PredecessorObservation {
        outcome,
        reason_code: predecessor_reason_code(&value, predecessor_permission),
        exit_status,
        stdout_sha256,
        stderr_sha256,
        parity: parity.to_owned(),
    }
}

fn predecessor_permission(value: &Value) -> Option<Permission> {
    let envelope = value.get("hookSpecificOutput").unwrap_or(value);
    let permission = envelope
        .get("permissionDecision")
        .and_then(Value::as_str)
        .or_else(|| value.get("decision").and_then(Value::as_str))?;
    match permission {
        "allow" => Some(Permission::Allow),
        "deny" | "block" => Some(Permission::Deny),
        _ => None,
    }
}

fn predecessor_reason_code(value: &Value, permission: Permission) -> String {
    value
        .get("reason_code")
        .or_else(|| value.get("reasonCode"))
        .and_then(Value::as_str)
        .filter(|reason| !reason.is_empty())
        .map(sanitize_metadata)
        .unwrap_or_else(|| match permission {
            Permission::Allow => "predecessor_allow".to_owned(),
            Permission::Deny => "predecessor_deny".to_owned(),
        })
}

fn input_metadata(input: &[u8]) -> InputMetadata {
    let value = serde_json::from_slice::<Value>(input).ok();
    let tool_name = value
        .as_ref()
        .and_then(|value| first_text(value, &["tool_name", "toolName", "tool"]))
        .map(sanitize_tool_name)
        .unwrap_or_else(|| "unknown".to_owned());
    let command_sha256 = value
        .as_ref()
        .and_then(|value| value.get("tool_input"))
        .and_then(|value| value.get("command"))
        .and_then(Value::as_str)
        .map(|command| sha256_hex(command.as_bytes()))
        .unwrap_or_else(|| sha256_hex(&[]));
    InputMetadata {
        tool_name,
        command_sha256,
        input_sha256: sha256_hex(input),
        session_id: metadata_text(value.as_ref(), &["session_id", "sessionId"]),
        turn_id: metadata_text(value.as_ref(), &["turn_id", "turnId"]),
        tool_use_id: metadata_text(value.as_ref(), &["tool_use_id", "toolUseId"]),
        transcript_path: metadata_text(value.as_ref(), &["transcript_path", "transcriptPath"]),
        cwd: metadata_text(value.as_ref(), &["cwd"]),
    }
}

fn metadata_text(value: Option<&Value>, keys: &[&str]) -> String {
    value
        .and_then(|value| first_text(value, keys))
        .filter(|text| !text.is_empty())
        .map(sanitize_metadata)
        .unwrap_or_else(|| "unknown".to_owned())
}

fn first_text<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
}

fn sanitize_metadata(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len().min(1024));
    for character in value.chars().take(1024) {
        if character.is_control() {
            sanitized.push('?');
        } else {
            sanitized.push(character);
        }
    }
    sanitized
}

fn sanitize_tool_name(tool_name: &str) -> String {
    sanitize_metadata(tool_name)
}

fn ledger_path() -> PathBuf {
    std::env::var_os("KERNEL_ONLY_HOOK_SHADOW_LEDGER")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_LEDGER_PATH))
}

fn permission_name(permission: Permission) -> &'static str {
    match permission {
        Permission::Allow => "allow",
        Permission::Deny => "deny",
    }
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        encoded.push(TABLE[(first >> 2) as usize] as char);
        encoded.push(TABLE[(((first & 0x03) << 4) | (second >> 4)) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            TABLE[(((second & 0x0f) << 2) | (third >> 6)) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            TABLE[(third & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

fn sha256_hex(input: &[u8]) -> String {
    const INITIAL: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let bit_len = (input.len() as u64).wrapping_mul(8);
    let padded_len = ((input.len() + 9 + 63) / 64) * 64;
    let mut padded = Vec::with_capacity(padded_len);
    padded.extend_from_slice(input);
    padded.push(0x80);
    padded.resize(padded_len - 8, 0);
    padded.extend_from_slice(&bit_len.to_be_bytes());

    let mut hash = INITIAL;
    for chunk in padded.chunks_exact(64) {
        let mut words = [0u32; 64];
        for (index, bytes) in chunk.chunks_exact(4).enumerate() {
            words[index] = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }

        let mut state = hash;
        for index in 0..64 {
            let ch = (state[4] & state[5]) ^ ((!state[4]) & state[6]);
            let maj = (state[0] & state[1]) ^ (state[0] & state[2]) ^ (state[1] & state[2]);
            let sigma1 =
                state[4].rotate_right(6) ^ state[4].rotate_right(11) ^ state[4].rotate_right(25);
            let sigma0 =
                state[0].rotate_right(2) ^ state[0].rotate_right(13) ^ state[0].rotate_right(22);
            let temp1 = state[7]
                .wrapping_add(sigma1)
                .wrapping_add(ch)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let temp2 = sigma0.wrapping_add(maj);
            state[7] = state[6];
            state[6] = state[5];
            state[5] = state[4];
            state[4] = state[3].wrapping_add(temp1);
            state[3] = state[2];
            state[2] = state[1];
            state[1] = state[0];
            state[0] = temp1.wrapping_add(temp2);
        }
        for index in 0..8 {
            hash[index] = hash[index].wrapping_add(state[index]);
        }
    }

    let mut output = String::with_capacity(64);
    for word in hash {
        write!(&mut output, "{word:08x}").expect("writing to String cannot fail");
    }
    output
}
#[cfg(test)]
mod tests {
    use super::*;

    fn serialized_verdict(input: &[u8]) -> String {
        let metadata = input_metadata(input);
        let predecessor = PredecessorObservation::not_configured();
        serde_json::to_string(&ShadowVerdict {
            schema_version: SCHEMA_VERSION,
            event: EVENT,
            hook_id: HOOK_ID,
            stage: STAGE,
            build_id: "test-build",
            ts_unix: 0,
            pid: 0,
            effective_mode: "shadow",
            tool_name: metadata.tool_name,
            command_sha256: metadata.command_sha256,
            input_sha256: metadata.input_sha256,
            session_id: metadata.session_id,
            turn_id: metadata.turn_id,
            tool_use_id: metadata.tool_use_id,
            transcript_path: metadata.transcript_path,
            cwd: metadata.cwd,
            rust_permission: "allow",
            rust_reason: "unit test",
            predecessor_outcome: predecessor.outcome,
            predecessor_reason_code: predecessor.reason_code,
            predecessor_exit_status: predecessor.exit_status,
            predecessor_stdout_sha256: predecessor.stdout_sha256,
            predecessor_stderr_sha256: predecessor.stderr_sha256,
            parity: predecessor.parity,
            review_status: "unknown",
            live_bash_parity: "not_applicable",
        })
        .expect("shadow verdict serialization cannot fail")
    }

    #[test]
    fn extracts_snake_case_metadata_and_sha256_fields() {
        let input = br#"{"tool_name":"Bash","tool_input":{"command":"echo hello"},"session_id":"session-snake","turn_id":"turn-snake","tool_use_id":"tool-snake","transcript_path":"/tmp/trace.jsonl","cwd":"/tmp"}"#;
        let metadata = input_metadata(input);

        assert_eq!(metadata.tool_name, "Bash");
        assert_eq!(metadata.session_id, "session-snake");
        assert_eq!(metadata.turn_id, "turn-snake");
        assert_eq!(metadata.tool_use_id, "tool-snake");
        assert_eq!(metadata.transcript_path, "/tmp/trace.jsonl");
        assert_eq!(metadata.cwd, "/tmp");
        assert_eq!(
            metadata.command_sha256,
            "584a331fd6b02dcb1ecbe2eba731f609a2e1e3dac0bb73ae998dfad14c309a77"
        );
        assert_eq!(
            metadata.input_sha256,
            "00d862530b4c377854c4bdc1229b63cb4d0e4c0e9068cce922808dcf42958f75"
        );
    }

    #[test]
    fn extracts_camel_case_metadata_fields() {
        let input = br#"{"toolName":"Bash","tool_input":{"command":"echo hello"},"sessionId":"session-camel","turnId":"turn-camel","toolUseId":"tool-camel","transcriptPath":"/tmp/camel-trace.jsonl","cwd":"/tmp/camel"}"#;
        let metadata = input_metadata(input);

        assert_eq!(metadata.tool_name, "Bash");
        assert_eq!(metadata.session_id, "session-camel");
        assert_eq!(metadata.turn_id, "turn-camel");
        assert_eq!(metadata.tool_use_id, "tool-camel");
        assert_eq!(metadata.transcript_path, "/tmp/camel-trace.jsonl");
        assert_eq!(metadata.cwd, "/tmp/camel");
        assert_eq!(metadata.command_sha256.len(), 64);
        assert_eq!(metadata.input_sha256.len(), 64);
    }

    #[test]
    fn serialized_verdict_contains_hashes_but_never_raw_command() {
        let input = br#"{"tool_name":"Bash","tool_input":{"command":"rm -rf /sensitive-command"},"session_id":"session","turn_id":"turn","tool_use_id":"tool","transcript_path":"/tmp/trace","cwd":"/tmp"}"#;
        let serialized = serialized_verdict(input);

        assert!(serialized.contains("command_sha256"));
        assert!(serialized.contains("input_sha256"));
        assert!(!serialized.contains("rm -rf /sensitive-command"));
        assert!(!serialized.contains("\"command\""));
    }

    #[test]
    fn metadata_values_are_bounded() {
        let long_session = "s".repeat(2_000);
        let input =
            format!(r#"{{"session_id":"{long_session}","tool_input":{{"command":"echo hello"}}}}"#);
        let metadata = input_metadata(input.as_bytes());

        assert_eq!(metadata.session_id.len(), 1_024);
    }

    #[test]
    fn predecessor_deadline_is_the_single_bounded_timeout() {
        assert_eq!(PREDECESSOR_DEADLINE, Duration::from_millis(250));
        assert_eq!(PREDECESSOR_DEADLINE.as_millis(), 250);
    }
}

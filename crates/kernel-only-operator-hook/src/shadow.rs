#![forbid(unsafe_code)]

//! Append-only shadow evidence for the Stage 4 operator hook.
//!
//! Shadow evidence is diagnostic only: it is not hook certification and never changes the
//! fail-closed default. The append happens synchronously in the caller's asupersync future; no
//! detached task is created, and a ledger failure cannot turn shadow mode into enforcement.

use kernel_only_operator_hook::{Decision, Permission};
use serde::Serialize;
use serde_json::Value;
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_LEDGER_PATH: &str =
    "/Users/josh/.local/state/flywheel/kernel-only-operator-hook-shadow-verdicts.jsonl";
const SCHEMA_VERSION: &str = "kernel-only-operator-hook-shadow.v1";
const EVENT: &str = "PreToolUse";
const HOOK_ID: &str = "kernel-only-operator-hook";
const STAGE: u8 = 4;

#[derive(Debug, Serialize)]
struct ShadowVerdict<'a> {
    schema_version: &'static str,
    event: &'static str,
    hook_id: &'static str,
    stage: u8,
    build_id: &'a str,
    ts_unix: u64,
    pid: u32,
    tool_name: String,
    command_sha256: String,
    rust_permission: &'static str,
    rust_reason: &'a str,
    review_status: &'static str,
    live_bash_parity: &'static str,
}

/// Append one JSON object with one O_APPEND write. The command is hashed, never stored.
pub fn append_verdict(input: &[u8], decision: &Decision, build_id: &str) -> io::Result<()> {
    let (tool_name, command_sha256) = input_metadata(input);
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
        tool_name,
        command_sha256,
        rust_permission: permission_name(decision.permission),
        rust_reason: &decision.reason,
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

fn input_metadata(input: &[u8]) -> (String, String) {
    let value = serde_json::from_slice::<Value>(input).ok();
    let tool_name = value
        .as_ref()
        .and_then(|value| value.get("tool_name").or_else(|| value.get("toolName")))
        .and_then(Value::as_str)
        .map(sanitize_tool_name)
        .unwrap_or_else(|| "unknown".to_owned());
    let command_sha256 = value
        .as_ref()
        .and_then(|value| value.get("tool_input"))
        .and_then(|value| value.get("command"))
        .and_then(Value::as_str)
        .map(|command| sha256_hex(command.as_bytes()))
        .unwrap_or_else(|| sha256_hex(&[]));
    (tool_name, command_sha256)
}

fn sanitize_tool_name(tool_name: &str) -> String {
    let mut sanitized = String::with_capacity(tool_name.len().min(128));
    for character in tool_name.chars().take(128) {
        if character.is_control() {
            sanitized.push('?');
        } else {
            sanitized.push(character);
        }
    }
    sanitized
}

// Small, dependency-free SHA-256 implementation keeps this leaf's lockfile stable.
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

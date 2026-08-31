#![forbid(unsafe_code)]

//! Fail-closed policy core for the kernel-only PreToolUse hook.
//!
//! The hook changes one product decision: whether an operator may execute a raw command that
//! duplicates an installed kernel. It blocks only the concrete bypass shapes it can classify and
//! reports an unresolved or malformed hook event as DENY rather than silently allowing it.

use asupersync::Cx;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

pub const MAX_INPUT_BYTES: usize = 64 * 1024;
pub const HOOK_EVENT: &str = "PreToolUse";

/// Kernel command shapes that are legitimate alternatives to raw operator handrolls.
pub const KERNEL_ALLOWLIST: &[&str] = &[
    "tick-monitor observe",
    "omp-orchestrator",
    "ntm --robot-send",
    "bv --robot-triage",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Oversized { limit: usize },
    InvalidJson(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oversized { limit } => write!(formatter, "input exceeds {limit}-byte bound"),
            Self::InvalidJson(error) => write!(formatter, "invalid hook JSON: {error}"),
        }
    }
}

/// Claude and Codex share the output envelope but not every input field.
/// Unknown fields are intentionally ignored for client-version tolerance.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct HookInput {
    #[serde(default, alias = "hookEventName")]
    pub hook_event_name: Option<String>,
    #[serde(default, alias = "toolName")]
    pub tool_name: Option<String>,
    #[serde(default, alias = "sessionId")]
    pub session_id: Option<String>,
    #[serde(default, alias = "turnId")]
    pub turn_id: Option<String>,
    #[serde(default, alias = "toolUseId")]
    pub tool_use_id: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub permission_mode: Option<String>,
    #[serde(default)]
    pub transcript_path: Option<String>,
    #[serde(default)]
    pub tool_input: Option<Value>,
}

pub fn parse_input(bytes: &[u8]) -> Result<HookInput, ParseError> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(ParseError::Oversized {
            limit: MAX_INPUT_BYTES,
        });
    }
    serde_json::from_slice(bytes).map_err(|error| ParseError::InvalidJson(error.to_string()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    Allow,
    Deny,
}

impl Permission {
    fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub permission: Permission,
    pub reason: String,
}

impl Decision {
    pub fn allow(reason: impl Into<String>) -> Self {
        Self {
            permission: Permission::Allow,
            reason: reason.into(),
        }
    }

    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            permission: Permission::Deny,
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Serialize)]
struct HookEnvelope<'a> {
    #[serde(rename = "hookSpecificOutput")]
    hook_specific_output: HookSpecificOutput<'a>,
}

#[derive(Debug, Serialize)]
struct HookSpecificOutput<'a> {
    #[serde(rename = "hookEventName")]
    hook_event_name: &'static str,
    #[serde(rename = "permissionDecision")]
    permission_decision: &'static str,
    #[serde(rename = "permissionDecisionReason")]
    permission_decision_reason: &'a str,
}

pub fn render_decision(decision: &Decision) -> String {
    let output = HookEnvelope {
        hook_specific_output: HookSpecificOutput {
            hook_event_name: HOOK_EVENT,
            permission_decision: decision.permission.as_str(),
            permission_decision_reason: &decision.reason,
        },
    };
    match serde_json::to_string(&output) {
        Ok(json) => json,
        Err(_) => "{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"deny\",\"permissionDecisionReason\":\"hook serialization failed\"}}".to_owned(),
    }
}

fn tool_command(input: &HookInput) -> Option<&str> {
    input
        .tool_input
        .as_ref()
        .and_then(|value| value.get("command"))
        .and_then(Value::as_str)
}

fn basename(token: &str) -> &str {
    token
        .trim_matches(|character: char| matches!(character, '"' | '\'' | '`' | ';' | '|' | '&'))
        .rsplit('/')
        .next()
        .unwrap_or(token)
}

fn words(command: &str) -> Vec<&str> {
    command.split_whitespace().map(basename).collect()
}

fn has_adjacent(words: &[&str], first: &str, second: &str) -> bool {
    words
        .windows(2)
        .any(|pair| pair[0] == first && pair[1] == second)
}

fn has_flag(words: &[&str], command: &str) -> bool {
    words
        .iter()
        .any(|word| *word == command || word.starts_with(&format!("{command}=")))
}

fn kernel_candidate(command: &str) -> Option<&'static str> {
    let tokens = words(command);
    if has_adjacent(&tokens, "tick-monitor", "observe") {
        return Some("tick-monitor observe");
    }
    if tokens.iter().any(|word| *word == "omp-orchestrator") {
        return Some("omp-orchestrator");
    }
    if has_adjacent(&tokens, "ntm", "--robot-send") || has_flag(&tokens, "--robot-send") {
        return Some("ntm --robot-send");
    }
    if has_adjacent(&tokens, "bv", "--robot-triage") || has_flag(&tokens, "--robot-triage") {
        return Some("bv --robot-triage");
    }
    None
}

fn classify_bash_with_allowlist(command: &str, allowlist: &[&str]) -> Decision {
    let tokens = words(command);
    // Deny raw effects before recognizing allowlisted text. A compound command
    // containing both a kernel name and raw send-keys is still a bypass.
    if has_adjacent(&tokens, "tmux", "send-keys") {
        return Decision::deny(
            "raw tmux send-keys dispatch is blocked; use the ntm --robot-send dispatch kernel",
        );
    }
    if has_adjacent(&tokens, "br", "create") {
        return Decision::deny(
            "raw br create is blocked; use the finding kernel to create a named obligation",
        );
    }
    if has_adjacent(&tokens, "br", "ready") {
        return Decision::deny("raw br ready is blocked; use the bv --robot-triage queue kernel");
    }
    if let Some(kernel) = kernel_candidate(command) {
        if allowlist.iter().any(|allowed| *allowed == kernel) {
            return Decision::allow(format!("kernel invocation accepted: {kernel}"));
        }
        return Decision::deny(format!(
            "kernel invocation {kernel:?} is missing from the kernel allowlist; use the registered kernel path"
        ));
    }
    if has_adjacent(&tokens, "tmux", "capture-pane") {
        return Decision::allow("diagnostic tmux capture-pane is allowed; it does not dispatch");
    }
    if has_adjacent(&tokens, "tmux", "list-panes") {
        return Decision::allow("diagnostic tmux list-panes is allowed; it does not dispatch");
    }
    Decision::allow("no registered kernel bypass command detected")
}

fn classify_bash(command: &str) -> Decision {
    classify_bash_with_allowlist(command, KERNEL_ALLOWLIST)
}

pub fn classify(input: &HookInput) -> Decision {
    match input.hook_event_name.as_deref() {
        Some(HOOK_EVENT) => {}
        Some(event) => {
            return Decision::deny(format!(
                "unsupported hook event {event:?}; expected PreToolUse"
            ))
        }
        None => return Decision::deny("malformed hook input: missing hook_event_name"),
    }
    let Some(tool_name) = input.tool_name.as_deref() else {
        return Decision::deny("malformed hook input: missing tool_name");
    };
    if tool_name != "Bash" && tool_name != "bash" {
        return Decision::allow(format!(
            "tool {tool_name:?} is outside the Bash kernel-bypass surface"
        ));
    }
    let Some(command) = tool_command(input) else {
        return Decision::deny("malformed Bash hook input: missing tool_input.command");
    };
    if command.trim().is_empty() {
        return Decision::deny("malformed Bash hook input: empty tool_input.command");
    }
    classify_bash(command)
}

/// Cx is the first parameter on the effectful hook path. The policy itself is pure.
pub async fn evaluate(cx: &Cx, bytes: &[u8]) -> Decision {
    if cx.checkpoint().is_err() {
        return Decision::deny("hook context cancelled before evaluation");
    }
    let decision = match parse_input(bytes) {
        Ok(input) => classify(&input),
        Err(error) => Decision::deny(format!("malformed hook input: {error}")),
    };
    if cx.checkpoint().is_err() {
        return Decision::deny("hook context cancelled after evaluation");
    }
    decision
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(tool: &str, command: &str) -> HookInput {
        HookInput {
            hook_event_name: Some(HOOK_EVENT.to_owned()),
            tool_name: Some(tool.to_owned()),
            tool_input: Some(serde_json::json!({"command": command})),
            ..HookInput::default()
        }
    }

    #[test]
    fn raw_dispatch_is_denied_with_kernel_name() {
        let decision = classify(&event("Bash", "tmux send-keys -t %1413 -l packet"));
        assert_eq!(decision.permission, Permission::Deny);
        assert!(decision.reason.contains("ntm --robot-send"));
    }

    #[test]
    fn diagnostic_capture_is_allowed() {
        assert_eq!(
            classify(&event("Bash", "tmux capture-pane -p -t %1413")).permission,
            Permission::Allow
        );
    }

    #[test]
    fn kernel_binary_is_allowed() {
        assert_eq!(
            classify(&event(
                "Bash",
                "/Users/josh/.local/bin/tick-monitor observe --session omp-orchestrator"
            ))
            .permission,
            Permission::Allow
        );
    }

    #[test]
    fn raw_bypass_wins_over_kernel_text_in_compound_command() {
        let decision = classify(&event(
            "Bash",
            "ntm --robot-send && tmux send-keys -t %1413 packet",
        ));
        assert_eq!(decision.permission, Permission::Deny);
        assert!(decision.reason.contains("raw tmux send-keys"));
    }

    #[test]
    fn allowlist_mutation_turns_kernel_good_leg_red() {
        let command = "/Users/josh/.local/bin/tick-monitor observe --session omp-orchestrator";
        assert_eq!(
            classify_bash_with_allowlist(command, &["omp-orchestrator"]).permission,
            Permission::Deny
        );
        assert_eq!(classify_bash(command).permission, Permission::Allow);
    }
    #[test]
    fn malformed_event_is_denied() {
        let decision = classify(&HookInput::default());
        assert_eq!(decision.permission, Permission::Deny);
    }

    #[test]
    fn output_is_current_nested_envelope() {
        let json: Value = serde_json::from_str(&render_decision(&Decision::deny("test"))).unwrap();
        assert_eq!(json["hookSpecificOutput"]["hookEventName"], HOOK_EVENT);
        assert_eq!(json["hookSpecificOutput"]["permissionDecision"], "deny");
    }
}

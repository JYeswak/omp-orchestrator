#![forbid(unsafe_code)]

//! Fail-closed policy core for the kernel-only PreToolUse hook.
//!
//! The hook changes one product decision: whether an operator may execute a raw command that
//! duplicates an installed kernel. It blocks only the concrete bypass shapes it can classify and
//! reports an unresolved or malformed hook event as DENY rather than silently allowing it.
//!
//! NO-CLAIM: this is not a general shell parser. It recognizes only the small set of command
//! shapes and separators needed by this hook policy; shell syntax outside that set is not parsed.
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

// Executable paths are intentionally matched by basename; this hook does not validate installation paths.
fn executable_name(token: &str) -> &str {
    token.rsplit('/').next().unwrap_or(token)
}

/// Split only on the shell separators this policy must notice. This is deliberately not a shell
/// parser: quotes prevent a separator split, but expansion, escaping, and shell grammar are not
/// interpreted.
fn command_segments(command: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut start = 0;
    let mut quote = None;
    let mut escaped = false;

    for (index, character) in command.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == char::from(92) {
            escaped = true;
            continue;
        }
        if let Some(delimiter) = quote {
            if character == delimiter {
                quote = None;
            }
            continue;
        }
        if character == '\'' || character == '"' || character == char::from(96) {
            quote = Some(character);
        } else if character == ';'
            || character == '|'
            || character == '&'
            || character == char::from(10)
        {
            segments.push(&command[start..index]);
            start = index + character.len_utf8();
        }
    }
    segments.push(&command[start..]);
    segments
}

fn tokens(segment: &str) -> Vec<&str> {
    segment.split_whitespace().collect()
}

fn command_name(segment: &str) -> Option<&str> {
    tokens(segment).first().map(|token| executable_name(token))
}

fn option_subcommand<'a>(tokens: &[&'a str], subcommands: &[&str]) -> Option<&'a str> {
    let mut index = 1;
    while index < tokens.len() {
        let token = tokens[index];
        if subcommands.contains(&token) {
            return Some(token);
        }
        if token == "--" {
            return tokens
                .get(index + 1)
                .copied()
                .filter(|candidate| subcommands.contains(candidate));
        }
        if !token.starts_with('-') {
            return None;
        }
        // tmux/br global options may take a separate value. The policy only needs
        // to skip these known option/value pairs before looking for the subcommand.
        if matches!(token, "-L" | "-S" | "-f" | "-c") {
            index += 1;
        }
        index += 1;
    }
    None
}

fn kernel_candidate(segment: &str) -> Option<&'static str> {
    let tokens = tokens(segment);
    let executable = tokens.first().map(|token| executable_name(token))?;
    match executable {
        "tick-monitor" if tokens.get(1) == Some(&"observe") => Some("tick-monitor observe"),
        "omp-orchestrator" => Some("omp-orchestrator"),
        "ntm"
            if tokens.get(1).is_some_and(|token| {
                *token == "--robot-send" || token.starts_with("--robot-send=")
            }) =>
        {
            Some("ntm --robot-send")
        }
        "bv" if tokens.get(1).is_some_and(|token| {
            *token == "--robot-triage" || token.starts_with("--robot-triage=")
        }) =>
        {
            Some("bv --robot-triage")
        }
        _ => None,
    }
}

fn raw_tmux_dispatch(segment: &str) -> bool {
    let tokens = tokens(segment);
    executable_name(tokens.first().copied().unwrap_or_default()) == "tmux"
        && option_subcommand(&tokens, &["send-keys"]).is_some()
}

fn raw_br_mutation(segment: &str) -> Option<&'static str> {
    let tokens = tokens(segment);
    if executable_name(tokens.first().copied().unwrap_or_default()) != "br" {
        return None;
    }
    match option_subcommand(&tokens, &["create", "ready"]) {
        Some("create") => Some("finding"),
        Some("ready") => Some("bv --robot-triage"),
        _ => None,
    }
}

fn diagnostic_tmux_read(segment: &str) -> bool {
    let tokens = tokens(segment);
    executable_name(tokens.first().copied().unwrap_or_default()) == "tmux"
        && option_subcommand(&tokens, &["capture-pane", "list-panes"]).is_some()
}

#[derive(Debug, Clone, Copy)]
struct ShellWord<'a> {
    raw: &'a str,
    closed: bool,
}

/// Tokenize only enough shell syntax to identify a quoted git commit message. This is deliberately
/// not a general shell parser: expansion, escaping, and shell grammar outside this detector remain
/// unparsed.
fn shell_words(segment: &str) -> Vec<ShellWord<'_>> {
    let mut words = Vec::new();
    let mut start = None;
    let mut quote = None;
    let mut escaped = false;

    for (index, character) in segment.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == char::from(92) {
            if start.is_some() {
                escaped = true;
            }
            continue;
        }
        if let Some(delimiter) = quote {
            if character == delimiter {
                quote = None;
            }
            continue;
        }
        if character == char::from(39) || character == '"' {
            if start.is_none() {
                start = Some(index);
            }
            quote = Some(character);
            continue;
        }
        if character.is_whitespace() {
            if let Some(word_start) = start.take() {
                words.push(ShellWord {
                    raw: &segment[word_start..index],
                    closed: quote.is_none(),
                });
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(word_start) = start {
        words.push(ShellWord {
            raw: &segment[word_start..],
            closed: quote.is_none(),
        });
    }
    words
}
/// Return the contents only when a shell word is one complete double-quoted word.
///
/// This validates only the quote boundary needed by the commit detector. It is deliberately not a
/// general shell parser: operators, substitutions, and concatenated shell words remain unparsed.
fn fully_double_quoted(word: &str) -> Option<&str> {
    let bytes = word.as_bytes();
    if bytes.len() < 2 || bytes.first() != Some(&b'"') || bytes.last() != Some(&b'"') {
        return None;
    }
    let mut escaped = false;
    for byte in &bytes[1..bytes.len() - 1] {
        if escaped {
            escaped = false;
            continue;
        }
        if *byte == char::from(92) as u8 {
            escaped = true;
        } else if *byte == b'"' {
            return None;
        }
    }
    (!escaped).then_some(&word[1..word.len() - 1])
}

fn commit_message_has_shell_expansion(message: &str) -> bool {
    let Some(content) = fully_double_quoted(message) else {
        return false;
    };
    let bytes = content.as_bytes();
    let mut escaped = false;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        if byte == char::from(92) as u8 {
            escaped = true;
            continue;
        }
        if byte == char::from(96) as u8 {
            return true;
        }
        if byte == b'$' {
            let Some(next) = bytes.get(index + 1).copied() else {
                continue;
            };
            if next == b'(' || next == b'{' || next.is_ascii_alphabetic() || next == b'_' {
                return true;
            }
        }
    }
    false
}

fn git_global_option_value(token: &str) -> bool {
    matches!(
        token,
        "-C" | "-c"
            | "--config-env"
            | "--git-dir"
            | "--work-tree"
            | "--namespace"
            | "--super-prefix"
            | "--list-cmds"
    )
}

fn git_global_flag(token: &str) -> bool {
    matches!(
        token,
        "--no-pager"
            | "--paginate"
            | "--no-replace-objects"
            | "--no-optional-locks"
            | "--literal-pathspecs"
            | "--glob-pathspecs"
            | "--noglob-pathspecs"
            | "--icase-pathspecs"
            | "--bare"
            | "--exec-path"
    ) || token.starts_with("--exec-path=")
}

/// Find the commit subcommand after the small, documented set of git global options recognized by
/// this hook. Unknown options stop recognition rather than turning this into a general parser.
fn git_commit_subcommand(words: &[ShellWord<'_>]) -> Option<usize> {
    if words.get(0).map(|word| executable_name(word.raw)) != Some("git") {
        return None;
    }
    let mut index = 1;
    while index < words.len() {
        let token = words[index].raw;
        if token == "commit" {
            return Some(index);
        }
        if git_global_option_value(token) {
            if words.get(index + 1).is_none() {
                return None;
            }
            index += 2;
            continue;
        }
        if token.starts_with("-C") && token.len() > 2 {
            index += 1;
            continue;
        }
        if token.starts_with("-c") && token.len() > 2 {
            index += 1;
            continue;
        }
        if token.starts_with("--config-env=")
            || token.starts_with("--git-dir=")
            || token.starts_with("--work-tree=")
            || token.starts_with("--namespace=")
            || token.starts_with("--super-prefix=")
            || token.starts_with("--list-cmds=")
        {
            index += 1;
            continue;
        }
        if token == "--exec-path" {
            if words
                .get(index + 1)
                .is_some_and(|word| word.raw != "commit")
            {
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if git_global_flag(token) {
            index += 1;
            continue;
        }
        return None;
    }
    None
}

fn git_commit_message_expansion(segment: &str) -> bool {
    let words = shell_words(segment);
    let Some(commit_index) = git_commit_subcommand(&words) else {
        return false;
    };
    let mut index = commit_index + 1;
    while index < words.len() {
        let option = words[index].raw;
        // Git's option terminator ends option parsing for the commit command. Everything
        // after it is a pathspec, so a path named `-m` (or containing shell metacharacters)
        // must not be mistaken for a commit-message option.
        if option == "--" {
            break;
        }
        let message = if option == "-m" || option == "--message" {
            let message = words.get(index + 1).filter(|word| word.closed);
            index += 2;
            message.map(|word| word.raw)
        } else if let Some(value) = option.strip_prefix("--message=") {
            index += 1;
            Some(value)
        } else if let Some(value) = option.strip_prefix("-m") {
            index += 1;
            (!value.is_empty()).then_some(value)
        } else {
            index += 1;
            None
        };
        if message.is_some_and(commit_message_has_shell_expansion) {
            return true;
        }
    }
    false
}
fn classify_bash_with_allowlist(command: &str, allowlist: &[&str]) -> Decision {
    let segments = command_segments(command);
    let commands: Vec<&str> = segments
        .iter()
        .copied()
        .filter(|segment| !segment.trim().is_empty())
        .collect();
    // Deny raw effects before recognizing allowlisted text. A compound command
    // containing both a kernel name and raw send-keys is still a bypass.
    if commands.iter().any(|segment| raw_tmux_dispatch(segment))
        || commands.windows(2).any(|pair| {
            command_name(pair[0]) == Some("tmux") && command_name(pair[1]) == Some("send-keys")
        })
    {
        return Decision::deny(
            "raw tmux send-keys dispatch is blocked; use the ntm --robot-send dispatch kernel",
        );
    }
    let br_create_separator = commands
        .windows(2)
        .any(|pair| command_name(pair[0]) == Some("br") && command_name(pair[1]) == Some("create"));
    let br_ready_separator = commands
        .windows(2)
        .any(|pair| command_name(pair[0]) == Some("br") && command_name(pair[1]) == Some("ready"));
    if commands
        .iter()
        .any(|segment| raw_br_mutation(segment).is_some())
        || br_create_separator
        || br_ready_separator
    {
        let kernel = commands
            .iter()
            .find_map(|segment| raw_br_mutation(segment))
            .unwrap_or(if br_ready_separator {
                "bv --robot-triage"
            } else {
                "finding"
            });
        return Decision::deny(if kernel == "finding" {
            "raw br create is blocked; use the finding kernel to create a named obligation"
        } else {
            "raw br ready is blocked; use the bv --robot-triage queue kernel"
        });
    }
    if let Some(kernel) = commands
        .iter()
        .find_map(|segment| kernel_candidate(segment))
    {
        if segments.len() != 1 {
            return Decision::deny(format!(
                "kernel invocation {kernel:?} must be the sole shell command"
            ));
        }
        if allowlist.iter().any(|allowed| *allowed == kernel) {
            return Decision::allow(format!("kernel invocation accepted: {kernel}"));
        }
        return Decision::deny(format!(
            "kernel invocation {kernel:?} is missing from the kernel allowlist; use the registered kernel path"
        ));
    }
    if commands
        .iter()
        .any(|segment| git_commit_message_expansion(segment))
    {
        return Decision::deny(
            "git commit message expansion in a double-quoted -m/--message argument is blocked; use git commit -F <file> instead",
        );
    }
    if commands.iter().any(|segment| diagnostic_tmux_read(segment)) {
        return Decision::allow("diagnostic tmux capture-pane is allowed; it does not dispatch");
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
                "tick-monitor observe --session omp-orchestrator"
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
        let command = "tick-monitor observe --session omp-orchestrator";
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

#[cfg(test)]
mod scratch_home_tests {
    use super::*;

    #[test]
    fn scratch_home_allows_clean_bash() {
        // Under a scratch HOME, the hook classifies from the policy alone —
        // no config files are loaded. A clean Bash command is allowed.
        let input = HookInput {
            hook_event_name: Some("PreToolUse".into()),
            tool_name: Some("Bash".into()),
            tool_input: Some(serde_json::json!({"command": "cargo test"})),
            ..Default::default()
        };
        let decision = classify(&input);
        assert_eq!(decision.permission, Permission::Allow);
    }

    #[test]
    fn scratch_home_denies_raw_send_keys() {
        // The policy is pure: no HOME lookup, no config file, no ambient state.
        // A scratch HOME produces the same verdict as a production HOME.
        let input = HookInput {
            hook_event_name: Some("PreToolUse".into()),
            tool_name: Some("Bash".into()),
            tool_input: Some(serde_json::json!({"command": "tmux send-keys -t %1 packet"})),
            ..Default::default()
        };
        let decision = classify(&input);
        assert_eq!(decision.permission, Permission::Deny);
    }

    #[test]
    fn no_claim_only_enumerated_shapes() {
        // The doc comment at the top of lib.rs states the NO-CLAIM. This test
        // is a structural check: the allowlist has exactly the declared entries.
        assert_eq!(KERNEL_ALLOWLIST.len(), 4);
        assert!(KERNEL_ALLOWLIST.contains(&"tick-monitor observe"));
        assert!(KERNEL_ALLOWLIST.contains(&"omp-orchestrator"));
        assert!(KERNEL_ALLOWLIST.contains(&"ntm --robot-send"));
        assert!(KERNEL_ALLOWLIST.contains(&"bv --robot-triage"));
    }
}

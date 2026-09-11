//! Pre-execution guard for tools the PARENT declares to a child agent.
//!
//! # The seam this is built on, and the one it is not
//!
//! There are two tool surfaces in OMP and only one of them can be intercepted.
//!
//! The interceptable one is inverted from what a guardrail author expects: the
//! parent declares a tool with the `set_host_tools` command
//! (`dist/types/modes/rpc/rpc-types.d.ts:65-67`, `RpcHostToolDefinition` at
//! `:671-679`), the child's invocation arrives at the parent as a
//! `host_tool_call` frame (`:681-687` — `id`, `toolCallId`, `toolName`,
//! `arguments`), and the PARENT is the process that executes it. The child then
//! blocks on a `host_tool_result` frame (`:701-706`) that only the parent can
//! send. Refusal is structural here: the parent holds execution, so declining
//! to run something is the default state of the world, not a race won.
//!
//! The other surface is observation only. `ToolExecutionStartEvent`,
//! `ToolExecutionUpdateEvent` and `ToolExecutionEndEvent`
//! (`dist/types/extensibility/extensions/types.d.ts:562-584`) are subscription
//! events. Measured on that reader: zero `veto`, `allow`, `deny`, `approve` or
//! `block` fields, and no reply channel of any kind. Nothing about emitting
//! them suspends the child.
//!
//! # NO-CLAIM
//!
//! This guard is only reachable for tools the PARENT declared via
//! `set_host_tools`. It does NOT and CANNOT intercept the child's own built-in
//! tools — those execute inside the child and emit observation-only
//! `tool_execution_*` events with no veto. Anyone believing this guard covers
//! all tool calls is wrong, and that misconception is exactly what motivated
//! this crate.
//!
//! Concretely: a child that runs `rm -rf` through its OWN bash tool is not seen
//! here before the fact. Coverage equals the set of names in
//! [`GuardPolicy::guarded`], and nothing widens it except taking a capability
//! away from the child and re-declaring it as a host tool.
//!
//! # Fail-closed
//!
//! [`evaluate`] is pure, total, and I/O-free. Every argument shape it cannot
//! read yields [`GuardDecision::Refuse`], never [`GuardDecision::Execute`]. The
//! reason is asymmetry of cost: a refused benign call is a retry with a clearer
//! payload, while an executed unparsed call is the exact outcome the guard was
//! installed to prevent. A parser that guesses is a guard that fails open.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A tool the parent registers with `set_host_tools`.
///
/// Field-for-field the wire shape of `RpcHostToolDefinition`
/// (`rpc-types.d.ts:671-679`); `parameters` is a JSON Schema object describing
/// the arguments the child is allowed to send.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostToolDecl {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

impl HostToolDecl {
    /// Declare a guarded tool whose single argument is an **argv array**.
    ///
    /// argv rather than a command string on purpose: argv is executable without
    /// a shell, so [`execute_approved`] never has to spawn one. A free-form
    /// command string is still accepted by [`evaluate`] for auditing hosts that
    /// already have one, but it is not runnable from here.
    #[must_use]
    pub fn guarded_argv(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "argv": {
                        "type": "array",
                        "items": { "type": "string" },
                        "minItems": 1,
                        "description": "Program followed by its arguments. No shell is spawned."
                    }
                },
                "required": ["argv"],
                "additionalProperties": false
            }),
        }
    }
}

/// Build the `set_host_tools` command frame for a declaration set.
///
/// Shape is fixed by `rpc-types.d.ts:65-67`: `{ id, type: "set_host_tools",
/// tools }`. Serializing this is the only way the guard becomes reachable at
/// all — an unregistered tool is never called, so the guard never runs.
#[must_use]
pub fn set_host_tools_command(id: &str, tools: &[HostToolDecl]) -> Value {
    serde_json::json!({
        "id": id,
        "type": "set_host_tools",
        "tools": tools,
    })
}

/// An inbound `host_tool_call` frame (`rpc-types.d.ts:681-687`).
///
/// `arguments` is held as a raw [`Value`] deliberately: the child chooses what
/// it sends, so a typed field here would move parse failure to deserialization
/// time where it would surface as a dropped frame rather than a recorded
/// refusal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostToolCall {
    /// Frame id. The matching `host_tool_result` must echo it.
    pub id: String,
    /// Model-side tool call id.
    pub tool_call_id: String,
    /// Name as declared by the parent in `set_host_tools`.
    pub tool_name: String,
    pub arguments: Value,
}

/// Refusal code: `arguments` was not a JSON object.
pub const CODE_ARGUMENTS_NOT_OBJECT: &str = "ARGUMENTS_NOT_OBJECT";
/// Refusal code: the policy's command key was absent from `arguments`.
pub const CODE_COMMAND_KEY_MISSING: &str = "COMMAND_KEY_MISSING";
/// Refusal code: the command key held neither a string nor an array of strings.
pub const CODE_COMMAND_NOT_READABLE: &str = "COMMAND_NOT_READABLE";
/// Refusal code: the command was present but empty after normalization.
pub const CODE_COMMAND_EMPTY: &str = "COMMAND_EMPTY";
/// Refusal code: a shell-free host cannot run a free-form command string.
pub const CODE_NOT_EXECUTABLE_WITHOUT_SHELL: &str = "NOT_EXECUTABLE_WITHOUT_SHELL";

/// The guard's verdict on one `host_tool_call`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum GuardDecision {
    /// The call is within policy. The parent MAY run it.
    Execute,
    /// The call is within the guard's coverage and is denied. `code` is stable
    /// and machine-readable; `detail` explains it to a human or to the child.
    Refuse { code: String, detail: String },
    /// The guard has nothing to say: the tool name is not one this policy
    /// governs.
    ///
    /// # This is NOT permission
    ///
    /// `Unknown` must NEVER be treated as [`GuardDecision::Execute`] by any
    /// caller. It means the guard did not evaluate the call, so no statement
    /// about its safety exists. A caller that folds `Unknown` into the execute
    /// path has built a guard that passes everything it fails to recognize —
    /// which is strictly worse than no guard, because it reads as coverage.
    /// Route `Unknown` to a `host_tool_result` carrying `isError: true`, or to
    /// a human. [`GuardDecision::is_execute`] returns `false` here.
    Unknown { reason: String },
}

impl GuardDecision {
    /// `true` only for [`GuardDecision::Execute`].
    ///
    /// [`GuardDecision::Refuse`] and [`GuardDecision::Unknown`] are both
    /// `false`; see the `Unknown` variant docs for why that is not a
    /// conservatism but the whole point.
    #[must_use]
    pub fn is_execute(&self) -> bool {
        matches!(self, Self::Execute)
    }
}

/// One deny rule, expressed over normalized tokens rather than as a regex.
///
/// A regex over raw command text has to re-solve flag clustering (`-rf`),
/// long-form spelling (`--force`), quoting and case for every rule, and each
/// rule gets it slightly differently wrong. Splitting the problem — normalize
/// once in [`normalize_tokens`], then match token sequences — makes every rule
/// a two-field statement that can be read aloud.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DenyRule {
    /// Stable machine-readable code surfaced as the `code` of
    /// [`GuardDecision::Refuse`].
    pub code: &'static str,
    /// Tokens that must appear IN ORDER (not necessarily adjacent).
    pub head: &'static [&'static str],
    /// Tokens that must ALL appear after the head match, in any order. Used for
    /// flags, which callers order freely.
    pub requires: &'static [&'static str],
    /// Human-readable reason, surfaced as the `detail` of
    /// [`GuardDecision::Refuse`].
    pub why: &'static str,
}

impl DenyRule {
    /// Does this rule fire on `tokens`?
    ///
    /// Leftmost-greedy subsequence matching, which is exact for subsequences:
    /// if a match exists at all, taking the earliest occurrence of each head
    /// token never eliminates it.
    #[must_use]
    pub fn matches(&self, tokens: &[String]) -> bool {
        let mut cursor = 0usize;
        for head in self.head {
            let rest = match tokens.get(cursor..) {
                Some(rest) => rest,
                None => return false,
            };
            match rest.iter().position(|token| token == head) {
                Some(offset) => cursor += offset + 1,
                None => return false,
            }
        }
        let tail = match tokens.get(cursor..) {
            Some(tail) => tail,
            None => return self.requires.is_empty(),
        };
        self.requires
            .iter()
            .all(|needle| tail.iter().any(|token| token == needle))
    }
}

/// The minimum deny set.
///
/// "Minimum" is literal: this list is the floor, not a claim of completeness.
/// It is not a sandbox and does not attempt to enumerate destructive commands.
/// `git worktree add` is here because this repository runs a ZERO-WORKTREE
/// policy — every agent works on `main` — so a child creating one is a policy
/// violation regardless of intent, and it is the one entry that is about repo
/// doctrine rather than data loss.
pub const DEFAULT_DENY: &[DenyRule] = &[
    DenyRule {
        code: "RM_RECURSIVE_FORCE",
        head: &["rm"],
        requires: &["-r", "-f"],
        why: "recursive forced delete: unrecoverable and unbounded by the path argument",
    },
    DenyRule {
        code: "GIT_PUSH_FORCE",
        head: &["git", "push"],
        requires: &["-f"],
        why: "force push: overwrites remote history other agents have already fetched",
    },
    DenyRule {
        code: "GIT_RESET_HARD",
        head: &["git", "reset"],
        requires: &["--hard"],
        why: "hard reset: discards uncommitted work in a shared checkout",
    },
    DenyRule {
        code: "SQL_DROP_TABLE",
        head: &["drop", "table"],
        requires: &[],
        why: "DROP TABLE: schema and row loss with no undo",
    },
    DenyRule {
        code: "GIT_WORKTREE_ADD",
        head: &["git", "worktree", "add"],
        requires: &[],
        why: "worktree creation violates the repository's zero-worktree policy: all work happens on main",
    },
];

/// Long flags normalized to their short form before matching, so a rule states
/// one spelling instead of every spelling.
///
/// `--force-with-lease` collapses to `-f` on purpose: it is safer than a bare
/// force push but still rewrites remote history, which is what the rule is
/// about.
const LONG_FLAG_ALIASES: &[(&str, &str)] = &[
    ("--recursive", "-r"),
    ("--force", "-f"),
    ("--force-with-lease", "-f"),
];

/// Characters treated as token separators in addition to whitespace.
///
/// Shell operators are split so a deny rule fires on ANY leg of a compound
/// command; quotes are split so `psql -c "drop table t"` tokenizes the same as
/// the bare statement. This is normalization for MATCHING only — nothing here
/// is a shell parser and no quoting semantics are honored.
const SEPARATORS: &[char] = &[
    ';', '|', '&', '(', ')', '\n', '\r', '\t', '"', '\'', '`', '=',
];

/// One tool this policy governs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardedTool {
    /// `toolName` exactly as declared to the child via `set_host_tools`.
    pub name: String,
    /// Argument key whose value carries the command: a string, or an array of
    /// strings (argv).
    pub command_key: String,
}

impl GuardedTool {
    #[must_use]
    pub fn new(name: impl Into<String>, command_key: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command_key: command_key.into(),
        }
    }
}

/// Which tools are covered and what is denied inside them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardPolicy {
    /// Coverage. A `host_tool_call` whose `toolName` is absent here yields
    /// [`GuardDecision::Unknown`], never `Execute`.
    pub guarded: Vec<GuardedTool>,
    pub deny: &'static [DenyRule],
}

impl Default for GuardPolicy {
    /// One guarded tool named `guarded_bash` reading its `argv` key, with
    /// [`DEFAULT_DENY`].
    fn default() -> Self {
        Self {
            guarded: vec![GuardedTool::new("guarded_bash", "argv")],
            deny: DEFAULT_DENY,
        }
    }
}

/// Split a command fragment into normalized match tokens.
///
/// Lowercases, splits on whitespace and [`SEPARATORS`], rewrites long flags via
/// [`LONG_FLAG_ALIASES`], and expands clustered short flags (`-rf` → `-r`,
/// `-f`) so a rule can name one flag at a time.
#[must_use]
pub fn normalize_tokens(fragments: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for fragment in fragments {
        let lowered = fragment.to_lowercase();
        for raw in lowered
            .split(|c: char| c.is_whitespace() || SEPARATORS.contains(&c))
            .filter(|piece| !piece.is_empty())
        {
            match LONG_FLAG_ALIASES.iter().find(|(long, _)| *long == raw) {
                Some((_, short)) => out.push((*short).to_string()),
                None => push_expanded_flag(&mut out, raw),
            }
        }
    }
    out
}

/// `-rf` is two flags. Anything else is one token.
fn push_expanded_flag(out: &mut Vec<String>, raw: &str) {
    let is_clustered_short = raw.starts_with('-')
        && !raw.starts_with("--")
        && raw.chars().count() > 2
        && raw.chars().skip(1).all(char::is_alphanumeric);
    if is_clustered_short {
        for flag in raw.chars().skip(1) {
            out.push(format!("-{flag}"));
        }
    } else {
        out.push(raw.to_string());
    }
}

/// Read the command out of `arguments`, as normalization-ready fragments.
///
/// `Err` carries the refusal code: an unreadable shape is a refusal, and the
/// caller cannot accidentally treat "could not read" as "nothing to deny".
fn command_fragments(arguments: &Value, key: &str) -> Result<Vec<String>, (&'static str, String)> {
    let object = arguments.as_object().ok_or_else(|| {
        (
            CODE_ARGUMENTS_NOT_OBJECT,
            format!(
                "`arguments` must be a JSON object; got {}",
                value_kind(arguments)
            ),
        )
    })?;
    let raw = object.get(key).ok_or_else(|| {
        (
            CODE_COMMAND_KEY_MISSING,
            format!("`arguments` has no `{key}` key"),
        )
    })?;
    match raw {
        Value::String(text) => Ok(vec![text.clone()]),
        Value::Array(items) => {
            let mut fragments = Vec::with_capacity(items.len());
            for item in items {
                match item.as_str() {
                    Some(text) => fragments.push(text.to_string()),
                    None => {
                        return Err((
                            CODE_COMMAND_NOT_READABLE,
                            format!(
                                "`{key}` array must hold strings; got a {} element",
                                value_kind(item)
                            ),
                        ))
                    }
                }
            }
            Ok(fragments)
        }
        other => Err((
            CODE_COMMAND_NOT_READABLE,
            format!(
                "`{key}` must be a string or an array of strings; got {}",
                value_kind(other)
            ),
        )),
    }
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// Decide one `host_tool_call`. Pure: no I/O, no clock, no environment.
///
/// Total by construction — every path returns a [`GuardDecision`], there is no
/// panic and no `Result`, because a guard that can fail to produce a decision
/// leaves the caller inventing one.
#[must_use]
pub fn evaluate(call: &HostToolCall, policy: &GuardPolicy) -> GuardDecision {
    let tool = match policy
        .guarded
        .iter()
        .find(|guarded| guarded.name == call.tool_name)
    {
        Some(tool) => tool,
        None => {
            return GuardDecision::Unknown {
                reason: format!(
                    "tool `{}` is not covered by this policy; the guard did not evaluate it and \
                     this is not permission to run it",
                    call.tool_name
                ),
            }
        }
    };

    let fragments = match command_fragments(&call.arguments, &tool.command_key) {
        Ok(fragments) => fragments,
        // Fail-closed. See the crate-level `Fail-closed` section: an argument
        // shape we cannot read is refused, because executing an unparsed call
        // is precisely the failure this guard exists to prevent.
        Err((code, detail)) => {
            return GuardDecision::Refuse {
                code: code.to_string(),
                detail,
            }
        }
    };

    let tokens = normalize_tokens(&fragments);
    if tokens.is_empty() {
        return GuardDecision::Refuse {
            code: CODE_COMMAND_EMPTY.to_string(),
            detail: format!("`{}` normalized to zero tokens", tool.command_key),
        };
    }

    for rule in policy.deny {
        if rule.matches(&tokens) {
            return GuardDecision::Refuse {
                code: rule.code.to_string(),
                detail: rule.why.to_string(),
            };
        }
    }

    GuardDecision::Execute
}

/// What happened when the parent ran an approved call.
#[derive(Debug)]
pub enum ExecOutcome {
    /// The child exited on its own inside the deadline. Both pipes were drained
    /// concurrently by `subprocess-contract`, so these bytes are complete and
    /// the ~64 KiB undrained-pipe deadlock cannot form.
    Ran {
        status_code: Option<i32>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    /// The deadline elapsed and the process GROUP was signalled. Restrictive:
    /// carries no output verdict, because a killed child must never read as a
    /// child that exited non-zero on its own.
    TimedOut,
    /// The child could not be spawned at all.
    Unspawned { detail: String },
    /// Nothing ran. The guard did not say [`GuardDecision::Execute`], or the
    /// approved call was not in argv form.
    NotPermitted { code: String, detail: String },
}

/// Run a call the guard approved, through the shared process-group boundary.
///
/// The decision is a required argument rather than something re-derived here:
/// the only way to reach the spawn is to hold a [`GuardDecision::Execute`],
/// and `Refuse`/`Unknown` both terminate in [`ExecOutcome::NotPermitted`]
/// before any process exists.
///
/// # NO-CLAIM
///
/// No shell is spawned, so a free-form command STRING is auditable by
/// [`evaluate`] but not runnable here — it returns
/// [`ExecOutcome::NotPermitted`] with [`CODE_NOT_EXECUTABLE_WITHOUT_SHELL`].
/// Hosts that want execution declare argv tools ([`HostToolDecl::guarded_argv`]).
/// And a deadline bounds how long the parent WAITS, not how long the child
/// runs: group TERM-then-KILL is best effort.
pub fn execute_approved(
    decision: &GuardDecision,
    call: &HostToolCall,
    policy: &GuardPolicy,
    deadline: std::time::Duration,
) -> ExecOutcome {
    match decision {
        GuardDecision::Execute => {}
        GuardDecision::Refuse { code, detail } => {
            return ExecOutcome::NotPermitted {
                code: code.clone(),
                detail: detail.clone(),
            }
        }
        GuardDecision::Unknown { reason } => {
            return ExecOutcome::NotPermitted {
                code: "UNKNOWN_IS_NOT_PERMISSION".to_string(),
                detail: reason.clone(),
            }
        }
    }

    let tool = match policy
        .guarded
        .iter()
        .find(|guarded| guarded.name == call.tool_name)
    {
        Some(tool) => tool,
        None => {
            return ExecOutcome::NotPermitted {
                code: "UNKNOWN_IS_NOT_PERMISSION".to_string(),
                detail: format!("tool `{}` is not covered by this policy", call.tool_name),
            }
        }
    };

    let argv = match call
        .arguments
        .as_object()
        .and_then(|object| object.get(&tool.command_key))
        .and_then(Value::as_array)
    {
        Some(items) => items,
        None => {
            return ExecOutcome::NotPermitted {
                code: CODE_NOT_EXECUTABLE_WITHOUT_SHELL.to_string(),
                detail: format!(
                    "`{}` must be an argv array to run without a shell",
                    tool.command_key
                ),
            }
        }
    };

    let mut parts = Vec::with_capacity(argv.len());
    for item in argv {
        match item.as_str() {
            Some(text) => parts.push(text),
            None => {
                return ExecOutcome::NotPermitted {
                    code: CODE_COMMAND_NOT_READABLE.to_string(),
                    detail: "argv holds a non-string element".to_string(),
                }
            }
        }
    }
    let (program, args) = match parts.split_first() {
        Some(split) => split,
        None => {
            return ExecOutcome::NotPermitted {
                code: CODE_COMMAND_EMPTY.to_string(),
                detail: "argv is empty".to_string(),
            }
        }
    };

    let mut command = std::process::Command::new(program);
    command.args(args);
    match subprocess_contract::bounded_output(&mut command, deadline) {
        subprocess_contract::BoundedOutcome::Completed(output) => ExecOutcome::Ran {
            status_code: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        },
        subprocess_contract::BoundedOutcome::TimedOut => ExecOutcome::TimedOut,
        subprocess_contract::BoundedOutcome::Unspawned(error) => ExecOutcome::Unspawned {
            detail: error.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(tool_name: &str, arguments: Value) -> HostToolCall {
        HostToolCall {
            id: "frame-1".to_string(),
            tool_call_id: "toolu_1".to_string(),
            tool_name: tool_name.to_string(),
            arguments,
        }
    }

    fn guarded(arguments: Value) -> HostToolCall {
        call("guarded_bash", arguments)
    }

    fn refusal_code(decision: &GuardDecision) -> &str {
        match decision {
            GuardDecision::Refuse { code, .. } => code,
            other => panic!("expected Refuse, got {other:?}"),
        }
    }

    /// Every entry in the minimum deny set fires, under its own code, in the
    /// spelling a child would plausibly send.
    #[test]
    fn each_deny_rule_refuses_with_its_code() {
        let policy = GuardPolicy::default();
        let cases: &[(&str, Value)] = &[
            (
                "RM_RECURSIVE_FORCE",
                serde_json::json!(["rm", "-rf", "/tmp/x"]),
            ),
            (
                "GIT_PUSH_FORCE",
                serde_json::json!(["git", "push", "origin", "main", "--force"]),
            ),
            (
                "GIT_RESET_HARD",
                serde_json::json!(["git", "reset", "--hard", "HEAD~3"]),
            ),
            (
                "SQL_DROP_TABLE",
                serde_json::json!(["psql", "-c", "DROP TABLE beads"]),
            ),
            (
                "GIT_WORKTREE_ADD",
                serde_json::json!(["git", "worktree", "add", "../side", "-b", "feat"]),
            ),
        ];
        for (expected, argv) in cases {
            let decision = evaluate(&guarded(serde_json::json!({ "argv": argv })), &policy);
            assert_eq!(refusal_code(&decision), *expected, "argv {argv:?}");
            assert!(!decision.is_execute());
        }
        assert_eq!(
            cases.len(),
            DEFAULT_DENY.len(),
            "every DEFAULT_DENY rule needs a firing case"
        );
    }

    /// Flag spellings a rule does not literally name: clustered, reordered,
    /// long-form, uppercase, and force-with-lease.
    #[test]
    fn deny_rules_survive_flag_spelling() {
        let policy = GuardPolicy::default();
        let spellings: &[Value] = &[
            serde_json::json!(["rm", "-r", "-f", "build"]),
            serde_json::json!(["rm", "-fr", "build"]),
            serde_json::json!(["rm", "-Rf", "build"]),
            serde_json::json!(["rm", "--recursive", "--force", "build"]),
        ];
        for argv in spellings {
            let decision = evaluate(&guarded(serde_json::json!({ "argv": argv })), &policy);
            assert_eq!(
                refusal_code(&decision),
                "RM_RECURSIVE_FORCE",
                "argv {argv:?}"
            );
        }
        let lease = evaluate(
            &guarded(serde_json::json!({ "argv": ["git", "push", "--force-with-lease"] })),
            &policy,
        );
        assert_eq!(refusal_code(&lease), "GIT_PUSH_FORCE");
    }

    /// A compound command is refused on any leg, not just the first.
    #[test]
    fn deny_rules_see_every_leg_of_a_compound_command() {
        let decision = evaluate(
            &guarded(serde_json::json!({ "argv": ["cargo build && git push --force"] })),
            &GuardPolicy::default(),
        );
        assert_eq!(refusal_code(&decision), "GIT_PUSH_FORCE");
    }

    /// The known-good leg: the guard is not attack-only, and a safe call runs.
    #[test]
    fn benign_calls_execute() {
        let policy = GuardPolicy::default();
        let benign: &[Value] = &[
            serde_json::json!(["git", "status", "--short"]),
            serde_json::json!(["rm", "stale.log"]),
            serde_json::json!(["git", "worktree", "list"]),
            serde_json::json!(["git", "push", "origin", "main"]),
            serde_json::json!(["ls", "-la", "crates"]),
            serde_json::json!(["psql", "-c", "select count(*) from beads"]),
        ];
        for argv in benign {
            assert_eq!(
                evaluate(&guarded(serde_json::json!({ "argv": argv })), &policy),
                GuardDecision::Execute,
                "argv {argv:?} should be Execute"
            );
        }
    }

    /// Fail-closed: every argument shape the guard cannot parse is `Refuse`.
    /// If any of these were `Execute` the guard would fail open on malformed
    /// input, which is the failure mode it exists to prevent.
    #[test]
    fn unparseable_arguments_refuse_and_never_execute() {
        let policy = GuardPolicy::default();
        let cases: &[(&str, Value)] = &[
            (CODE_ARGUMENTS_NOT_OBJECT, serde_json::json!("rm -rf /")),
            (CODE_ARGUMENTS_NOT_OBJECT, serde_json::json!(["rm", "-rf"])),
            (CODE_ARGUMENTS_NOT_OBJECT, Value::Null),
            (
                CODE_COMMAND_KEY_MISSING,
                serde_json::json!({ "cmd": ["ls"] }),
            ),
            (CODE_COMMAND_NOT_READABLE, serde_json::json!({ "argv": 42 })),
            (
                CODE_COMMAND_NOT_READABLE,
                serde_json::json!({ "argv": ["rm", { "nested": true }] }),
            ),
            (CODE_COMMAND_EMPTY, serde_json::json!({ "argv": [] })),
            (CODE_COMMAND_EMPTY, serde_json::json!({ "argv": "   " })),
        ];
        for (expected, arguments) in cases {
            let decision = evaluate(&guarded(arguments.clone()), &policy);
            assert_eq!(refusal_code(&decision), *expected, "arguments {arguments}");
            assert!(
                !decision.is_execute(),
                "malformed arguments must never execute: {arguments}"
            );
        }
    }

    /// An undeclared tool name is `Unknown`, and `Unknown` is not executable.
    #[test]
    fn undeclared_tool_is_unknown_and_not_executable() {
        let decision = evaluate(
            &call("bash", serde_json::json!({ "argv": ["rm", "-rf", "/"] })),
            &GuardPolicy::default(),
        );
        match &decision {
            GuardDecision::Unknown { reason } => {
                assert!(reason.contains("bash"), "reason names the tool: {reason}");
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
        assert!(
            !decision.is_execute(),
            "Unknown must never be treated as Execute"
        );
    }

    /// The structural refusal: without an `Execute` decision, no process is
    /// ever created. No subprocess is spawned by this test.
    #[test]
    fn execution_is_gated_on_the_decision_not_the_call() {
        let policy = GuardPolicy::default();
        // A call whose argv is perfectly runnable, paired with a refusal.
        let runnable = guarded(serde_json::json!({ "argv": ["true"] }));
        let refused = GuardDecision::Refuse {
            code: "GIT_PUSH_FORCE".to_string(),
            detail: "denied".to_string(),
        };
        match execute_approved(
            &refused,
            &runnable,
            &policy,
            std::time::Duration::from_secs(1),
        ) {
            ExecOutcome::NotPermitted { code, .. } => assert_eq!(code, "GIT_PUSH_FORCE"),
            other => panic!("a refused decision must not run: {other:?}"),
        }
        match execute_approved(
            &GuardDecision::Unknown {
                reason: "uncovered".to_string(),
            },
            &runnable,
            &policy,
            std::time::Duration::from_secs(1),
        ) {
            ExecOutcome::NotPermitted { code, .. } => {
                assert_eq!(code, "UNKNOWN_IS_NOT_PERMISSION")
            }
            other => panic!("an unknown decision must not run: {other:?}"),
        }
    }

    /// An approved free-form STRING is auditable but not runnable: this crate
    /// spawns no shell.
    #[test]
    fn approved_command_string_is_not_executable_without_a_shell() {
        let policy = GuardPolicy::default();
        let frame = guarded(serde_json::json!({ "argv": "git status --short" }));
        assert_eq!(evaluate(&frame, &policy), GuardDecision::Execute);
        match execute_approved(
            &GuardDecision::Execute,
            &frame,
            &policy,
            std::time::Duration::from_secs(1),
        ) {
            ExecOutcome::NotPermitted { code, .. } => {
                assert_eq!(code, CODE_NOT_EXECUTABLE_WITHOUT_SHELL)
            }
            other => panic!("a command string must not be shelled out: {other:?}"),
        }
    }

    /// Coverage is exactly the declared name set; renaming the guarded tool
    /// moves the guard with it.
    #[test]
    fn coverage_follows_the_declared_tool_name() {
        let policy = GuardPolicy {
            guarded: vec![GuardedTool::new("run_sql", "statement")],
            deny: DEFAULT_DENY,
        };
        let sql = call(
            "run_sql",
            serde_json::json!({ "statement": "drop table beads" }),
        );
        assert_eq!(refusal_code(&evaluate(&sql, &policy)), "SQL_DROP_TABLE");
        let safe = call(
            "run_sql",
            serde_json::json!({ "statement": "select 1 from beads" }),
        );
        assert_eq!(evaluate(&safe, &policy), GuardDecision::Execute);
    }

    /// The registration frame matches `rpc-types.d.ts:65-67` — an unregistered
    /// tool is never called, so a wrong shape here silently disables the guard.
    #[test]
    fn registration_frame_matches_the_wire_shape() {
        let decl = HostToolDecl::guarded_argv("guarded_bash", "Run a policy-checked command");
        let frame = set_host_tools_command("cmd-7", std::slice::from_ref(&decl));
        assert_eq!(frame["type"], "set_host_tools");
        assert_eq!(frame["id"], "cmd-7");
        assert_eq!(frame["tools"][0]["name"], "guarded_bash");
        assert_eq!(frame["tools"][0]["parameters"]["required"][0], "argv");
        assert_eq!(
            frame["tools"][0]["parameters"]["properties"]["argv"]["type"],
            "array"
        );
    }

    /// The inbound frame deserializes from the camelCase wire form
    /// (`rpc-types.d.ts:681-687`), including the `type` tag it carries.
    #[test]
    fn inbound_frame_deserializes_from_wire_form() {
        let frame = r#"{
            "type": "host_tool_call",
            "id": "h1",
            "toolCallId": "toolu_9",
            "toolName": "guarded_bash",
            "arguments": { "argv": ["git", "reset", "--hard"] }
        }"#;
        let parsed: HostToolCall = serde_json::from_str(frame).expect("frame parses");
        assert_eq!(parsed.tool_call_id, "toolu_9");
        assert_eq!(parsed.tool_name, "guarded_bash");
        assert_eq!(
            refusal_code(&evaluate(&parsed, &GuardPolicy::default())),
            "GIT_RESET_HARD"
        );
    }
}

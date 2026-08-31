#![forbid(unsafe_code)]

use kernel_only_operator_hook::{
    classify, parse_input, render_decision, HookInput, Permission, MAX_INPUT_BYTES,
};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Instant;

fn claude_event(command: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "session_id": "claude-session",
        "cwd": "/tmp/project"
    }))
    .unwrap()
}

#[test]
fn claude_raw_dispatch_is_denied_and_names_kernel() {
    let input: HookInput = parse_input(&claude_event("tmux send-keys -t %1413 -l packet")).unwrap();
    let decision = classify(&input);
    assert_eq!(decision.permission, Permission::Deny);
    assert!(decision.reason.contains("ntm --robot-send"));
}

#[test]
fn codex_apply_patch_dialect_uses_same_policy_surface() {
    let input: HookInput = parse_input(
        br#"{
          "hookEventName":"PreToolUse",
          "toolName":"Bash",
          "turn_id":"turn-7",
          "tool_use_id":"tool-7",
          "tool_input":{"command":"tmux send-keys -t %1413 -l packet"},
          "unknown_future_field":{"x":true}
        }"#,
    )
    .unwrap();
    let decision = classify(&input);
    assert_eq!(decision.permission, Permission::Deny);
    assert!(decision.reason.contains("ntm --robot-send"));
}

#[test]
fn diagnostic_tmux_read_is_allowed() {
    for command in [
        "tmux capture-pane -p -t %1413",
        "tmux list-panes -a -F '#{pane_id}'",
        "tmux capture-pane -p -t %1413 | grep 'tmux send-keys'",
        "tmux list-panes -a -F '#{pane_id}' | grep pane",
        "/Users/josh/.local/bin/tick-monitor observe --session omp-orchestrator",
        "/Users/josh/.local/bin/omp-orchestrator --once",
        "ntm --robot-send=omp-orchestrator --panes=%1413 --msg-file=/tmp/packet",
    ] {
        let input: HookInput = parse_input(&claude_event(command)).unwrap();
        assert_eq!(classify(&input).permission, Permission::Allow, "{command}");
    }
}

#[test]
fn exact_kernel_shapes_and_token_spoofs_have_distinct_verdicts() {
    for command in [
        "/Users/josh/.local/bin/tick-monitor observe --session omp-orchestrator",
        "/Users/josh/.local/bin/omp-orchestrator --once",
        "ntm --robot-send=omp-orchestrator --panes=%1413",
        "bv --robot-triage --json",
    ] {
        let input: HookInput = parse_input(&claude_event(command)).unwrap();
        let decision = classify(&input);
        assert_eq!(
            decision.permission,
            Permission::Allow,
            "{command}: {decision:?}"
        );
        assert!(decision.reason.contains("kernel invocation accepted"));
    }

    for command in [
        "rm -rf --robot-send",
        "echo omp-orchestrator",
        "rm -rf tick-monitor observe",
    ] {
        let input: HookInput = parse_input(&claude_event(command)).unwrap();
        let decision = classify(&input);
        assert_eq!(
            decision.permission,
            Permission::Allow,
            "{command}: {decision:?}"
        );
        assert!(
            !decision.reason.contains("kernel invocation accepted"),
            "{decision:?}"
        );
    }

    for command in [
        "ntm --robot-send; echo done",
        "echo setup; omp-orchestrator --once",
        "tick-monitor observe; echo done",
    ] {
        let input: HookInput = parse_input(&claude_event(command)).unwrap();
        let decision = classify(&input);
        assert_eq!(
            decision.permission,
            Permission::Deny,
            "{command}: {decision:?}"
        );
        assert!(
            decision.reason.contains("sole shell command"),
            "{decision:?}"
        );
    }
}

#[test]
fn raw_dispatch_options_and_separators_are_denied() {
    for (command, kernel) in [
        (
            "tmux -L /tmp/socket send-keys -t %1413 packet",
            "ntm --robot-send",
        ),
        (
            "tmux -S /tmp/socket send-keys -t %1413 packet",
            "ntm --robot-send",
        ),
        (
            "echo setup; tmux send-keys -t %1413 packet",
            "ntm --robot-send",
        ),
        (
            "tmux capture-pane -p -t %1413 | tmux send-keys -t %1413 packet",
            "ntm --robot-send",
        ),
        (
            "tick-monitor observe; tmux send-keys -t %1413 packet",
            "ntm --robot-send",
        ),
        ("tmux;send-keys -t %1413 packet", "ntm --robot-send"),
        ("tmux&&send-keys -t %1413 packet", "ntm --robot-send"),
        ("br --json create --title gap", "finding"),
        ("br -q ready --json", "bv --robot-triage"),
        ("echo setup; br create --title gap", "finding"),
        ("echo setup; br ready --json", "bv --robot-triage"),
        ("br;create --title gap", "finding"),
        ("br&&ready --json", "bv --robot-triage"),
    ] {
        let input: HookInput = parse_input(&claude_event(command)).unwrap();
        let decision = classify(&input);
        assert_eq!(
            decision.permission,
            Permission::Deny,
            "{command}: {decision:?}"
        );
        assert!(decision.reason.contains(kernel), "{decision:?}");
    }
}

#[test]
fn other_raw_kernel_bypasses_are_denied() {
    for (command, kernel) in [
        ("br create --title gap", "finding"),
        ("br ready --json", "bv --robot-triage"),
    ] {
        let input: HookInput = parse_input(&claude_event(command)).unwrap();
        let decision = classify(&input);
        assert_eq!(decision.permission, Permission::Deny, "{command}");
        assert!(decision.reason.contains(kernel), "{decision:?}");
    }
}

#[test]
fn git_commit_message_expansion_requires_file_backed_message() {
    // These are the supported commit spellings, including git global options and
    // attached message forms. Every message value is a complete double-quoted word.
    for command in [
        r#"git commit -m "release `date`""#,
        r#"git commit --message "release $(git status --short)""#,
        r#"git commit -m "release $VERSION""#,
        r#"git commit -m "it's $VAR""#,
        r#"git commit -m"release $VERSION""#,
        r#"git commit --message="release $(git status --short)""#,
        r#"/usr/bin/git --no-pager -C /tmp/repo -c user.name=ci commit --message="release ${VERSION}""#,
        r#"git --git-dir=/tmp/repo/.git --work-tree=/tmp/repo commit -m"release `date`""#,
        r#"git -C /tmp/repo --exec-path /tmp/git commit -m "release $(git status)""#,
    ] {
        let input: HookInput = parse_input(&claude_event(command)).unwrap();
        let decision = classify(&input);
        assert_eq!(
            decision.permission,
            Permission::Deny,
            "{command}: {decision:?}"
        );
        assert!(decision.reason.contains("-F <file>"), "{decision:?}");
    }

    // Quoting and file-backed forms stay outside this narrow detector. In
    // particular, metacharacters in another option or another command are not
    // commit-message expansions.
    for command in [
        "git commit -F /tmp/message",
        r#"git commit --file="/tmp/$(git status)""#,
        r#"git commit -m "plain text""#,
        r#"git commit --message="plain text""#,
        r#"git commit -m 'release `date` $(git status) $VERSION'"#,
        r#"git commit -m "literal \`date\` \$VERSION \$(git status) \${HOME}""#,
        r#"git commit --author="release `date`" -m "safe""#,
        r#"git commit -m 'it'\''s "safe"'"#,
        r#"git status -m "release $VERSION""#,
        r#"printf "git commit -m `date`""#,
        r#"echo "git commit -m $(git status)""#,
    ] {
        let input: HookInput = parse_input(&claude_event(command)).unwrap();
        assert_eq!(classify(&input).permission, Permission::Allow, "{command}");
    }
}
#[test]
fn git_commit_option_terminator_keeps_pathspecs_outside_message_detector() {
    // After `--`, Git treats every token as a pathspec. In particular, a pathspec that
    // resembles `-m` must not activate the narrow commit-message expansion detector.
    for command in [
        r#"git commit -- -m "$VAR""#,
        r#"git commit -- --message="release $(git status)""#,
        r#"git commit -- "release `date`""#,
        r#"git commit -- "path $(git status)""#,
    ] {
        let input: HookInput = parse_input(&claude_event(command)).unwrap();
        let decision = classify(&input);
        assert_eq!(
            decision.permission,
            Permission::Allow,
            "{command}: {decision:?}"
        );
    }
}

#[test]
fn malformed_and_oversized_inputs_fail_closed_without_panicking() {
    for bytes in [
        b"".as_ref(),
        b"not-json".as_ref(),
        br#"{"tool_name":"Bash""#.as_ref(),
    ] {
        let decision = std::panic::catch_unwind(|| parse_input(bytes).err().is_some())
            .expect("parser must not panic");
        assert!(decision);
    }
    let oversized = vec![b'x'; MAX_INPUT_BYTES + 1];
    assert!(parse_input(&oversized).is_err());
}

#[test]
fn deterministic_mutations_remain_typed_and_fail_closed() {
    let mut seed = 0x9e37_79b9_u64;
    for index in 0..128 {
        let mut bytes = claude_event("tmux send-keys -t %1413 -l packet");
        seed ^= seed << 7;
        seed ^= seed >> 9;
        let position = (seed as usize) % bytes.len();
        bytes[position] ^= (index as u8).wrapping_add(1);
        let result = std::panic::catch_unwind(|| {
            parse_input(&bytes)
                .map(|input| classify(&input))
                .unwrap_or_else(|error| {
                    kernel_only_operator_hook::Decision::deny(format!(
                        "malformed hook input: {error}"
                    ))
                })
        })
        .expect("mutation must not panic");
        if result.reason.contains("malformed") {
            assert_eq!(result.permission, Permission::Deny);
        }
    }
}

#[test]
fn hook_policy_p99_is_below_fifty_milliseconds() {
    let input: HookInput = parse_input(&claude_event("tmux send-keys -t %1413 -l packet")).unwrap();
    let mut samples = Vec::with_capacity(100);
    for _ in 0..100 {
        let start = Instant::now();
        let _ = classify(&input);
        samples.push(start.elapsed());
    }
    samples.sort_unstable();
    assert!(samples[99].as_millis() < 50, "p99={:?}", samples[99]);
}

#[test]
fn output_is_json_only_and_nested_in_hook_specific_output() {
    let output = render_decision(&kernel_only_operator_hook::Decision::deny("raw dispatch"));
    let value: Value = serde_json::from_str(&output).unwrap();
    assert!(value.get("hookSpecificOutput").is_some());
    assert_eq!(value["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(value.get("stderr").is_none());
}
#[test]
fn evaluate_uses_the_runtime_context_and_keeps_exit_decision_typed() {
    let runtime = asupersync::runtime::RuntimeBuilder::current_thread()
        .build()
        .expect("runtime");
    let input = claude_event("tmux send-keys -t %1413 -l packet");
    let decision = runtime.block_on(async {
        let cx = asupersync::Cx::current().expect("runtime context");
        kernel_only_operator_hook::evaluate(&cx, &input).await
    });
    assert_eq!(decision.permission, Permission::Deny);
    assert!(decision.reason.contains("ntm --robot-send"));
}
#[test]
fn shadow_compare_predecessor_records_safe_local_observation() {
    let ledger_path = std::env::temp_dir().join(format!(
        "kernel-only-operator-hook-shadow-compare-{}.jsonl",
        std::process::id()
    ));
    let _ = fs::remove_file(&ledger_path);
    let command = "printf safe";
    let input = serde_json::to_vec(&serde_json::json!({
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "session_id": "integration-session",
        "turn_id": "integration-turn",
        "tool_use_id": "integration-tool",
        "transcript_path": "/tmp/integration-transcript.jsonl",
        "cwd": "/tmp/kernel-only-operator-hook-test"
    }))
    .unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_kernel-only-operator-hook"))
        .args(["--shadow", "--compare-predecessor", "/bin/cat"])
        .env("KERNEL_ONLY_HOOK_SHADOW_LEDGER", &ledger_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn kernel-only operator hook");
    child
        .stdin
        .take()
        .expect("hook stdin")
        .write_all(&input)
        .expect("write hook input");
    let output = child
        .wait_with_output()
        .expect("wait for kernel-only operator hook");
    assert!(output.status.success(), "hook failed: {:?}", output);

    let stdout = String::from_utf8(output.stdout).expect("hook stdout is UTF-8");
    assert_eq!(stdout.lines().count(), 1, "hook emits one JSON envelope");
    let envelope: Value = serde_json::from_str(stdout.trim_end()).expect("valid hook envelope");
    assert_eq!(
        envelope["hookSpecificOutput"]["hookEventName"],
        "PreToolUse"
    );
    assert_eq!(
        envelope["hookSpecificOutput"]["permissionDecision"],
        "allow"
    );

    let ledger = fs::read_to_string(&ledger_path).expect("shadow ledger");
    assert_eq!(ledger.lines().count(), 1, "one JSONL shadow row");
    assert!(
        !ledger.contains(command),
        "raw command must not be persisted in shadow evidence"
    );
    let row: Value = serde_json::from_str(ledger.trim()).expect("valid shadow JSONL row");
    assert_eq!(row["effective_mode"], "shadow");
    assert_eq!(row["rust_permission"], "allow");
    for field in ["command_sha256", "input_sha256"] {
        let hash = row[field].as_str().expect("hash field is text");
        assert_eq!(hash.len(), 64, "{field} is SHA-256 hex");
        assert!(
            hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "{field} is hexadecimal"
        );
    }
    for (field, expected) in [
        ("session_id", "integration-session"),
        ("turn_id", "integration-turn"),
        ("tool_use_id", "integration-tool"),
    ] {
        assert_eq!(row[field], expected);
    }
    assert_eq!(row["transcript_path"], "/tmp/integration-transcript.jsonl");
    assert_eq!(row["cwd"], "/tmp/kernel-only-operator-hook-test");
    assert!(
        row["predecessor_outcome"]
            .as_str()
            .is_some_and(|outcome| !outcome.is_empty()),
        "predecessor outcome is recorded"
    );
    assert!(row["predecessor_exit_status"].as_str().is_some());
    assert_eq!(
        envelope["hookSpecificOutput"]["permissionDecision"], "allow",
        "shadow effective behavior remains allow"
    );
    let _ = fs::remove_file(ledger_path);
}

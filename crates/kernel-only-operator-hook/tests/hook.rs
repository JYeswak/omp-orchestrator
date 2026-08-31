#![forbid(unsafe_code)]

use kernel_only_operator_hook::{
    classify, parse_input, render_decision, HookInput, Permission, MAX_INPUT_BYTES,
};
use serde_json::Value;
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
        "/Users/josh/.local/bin/tick-monitor observe --session omp-orchestrator",
        "/Users/josh/.local/bin/omp-orchestrator --once",
        "ntm --robot-send=omp-orchestrator --panes=%1413 --msg-file=/tmp/packet",
    ] {
        let input: HookInput = parse_input(&claude_event(command)).unwrap();
        assert_eq!(classify(&input).permission, Permission::Allow, "{command}");
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

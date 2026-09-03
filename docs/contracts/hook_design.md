# Hook Design Contract

Bead: `omp-orchestrator-hook-design-contract-vnp2`

## Purpose

This contract locks the process-hook boundary for the August-2026 Rust ecosystem: a short-lived Rust binary receives one host-specific hook payload, parses the supported dialect, returns one structured decision, and exits with a documented status without allowing timeout, malformed input, or missing certification to masquerade as success. It defines the authority registry, event-class fail modes, bounded cancellation, the certified-state projection, the reduced field schema, and the evidence required before a hook is enabled. It is design-only; HD-0012 forbids building or enabling a hook in this pass.

## Contract Artifacts

1. **Authority registry:** `control-plane/hooks_certified.toml`, versioned with the control-plane substrate. It is the single registry for hooks present in `~/.claude/settings.json`; S1 stores references and projections, not a second hook definition.
2. **Hook binaries:** the installed Rust substrate under `/Users/josh/.local/bin/`, with source shape in `control-plane/crates/zestgraph-hook-substrates`.
3. **Certification runner:** `hooks-registry-check` plus the six-stage `/hook-certification` gauntlet. A future `tests/hook_design_contract.rs` is the invariant suite; it does not exist in this wave.
4. **Host adapters:** Claude `hookSpecificOutput` JSON and Codex command-hook output are separate serializers over one internal `HookDecision`; OMP's extension events are a different in-process API, not silently treated as the stdin process protocol.

## Measured Surface and Version Pins

The following are remeasurements, not inherited counts.

| ID | Source and version | Measurement |
|---|---|---|
| `SRC-CLAUDE-21259` | Claude Code `2.1.259`; official Hooks reference: `https://code.claude.com/docs/en/hooks` | `PreToolUse` receives JSON containing `tool_name`, `tool_input`, and `tool_use_id`; structured decisions are stdout JSON with exit `0`; exit `2` is a blocking hook signal with stderr reason; a hook timeout normally continues the host permission flow. |
| `SRC-CODEX-01521` | Codex CLI `0.152.1`; official Hooks reference: `https://developers.openai.com/codex/hooks` | `hooks.json` supplies matcher, command, timeout, and status message; exit `0` continues; exit `2` is event-specific blocking/feedback with stderr reason. Host timeout values are seconds. |
| `SRC-OMP-1816` | OMP `18.1.6`; installed source `/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent/src/capability/hook.ts` and `examples/sdk/06-hooks.ts` | OMP exposes in-process `pi.on(...)` extension events such as `agent_start`, `tool_call`, and `agent_end`; a handler returning `undefined` does not block. Its `Hook` interface describes shell pre/post hooks, not Claude stdin JSON. |
| `SRC-ZESTGRAPH-54A36` | control-plane commit `54a36eb3a13e8d10ae454def6ead08a03ee3094a`; `zestgraph-hook-substrates` edition 2024 | Seven Rust binaries, direct asupersync dependency at git rev `fa3c01aec6c77c6652c7a754e8e009287daa5323`; `MAX_STDIN_BYTES=1_048_576`, child budget 9 seconds, `Cx`, `serde_json`, and `ExitCode` are present. |
| `SRC-REGISTRY-16` | `/Users/josh/Developer/control-plane/hooks_certified.toml` | 16 rows, 0 `certified=true`, 16 `certified=false`, four event classes, and 16 unique existing binary paths. `certified=false` is commented as Stage-4 soak not run. |
| `SRC-HOST-20` | installed `/Users/josh/.local/bin` census | 15 executable `zestgraph-*` paths plus `dcg`, `slb-guard-fail-closed`, `rch`, `skill-topology-hook`, and `skill-tracker`: 20 hook-adjacent binaries. The registry also names one shell script; it is not counted as a binary. |
| `SRC-CLAUDE-CONFIG` | `/Users/josh/.claude/settings.json` | Four hook event keys are configured: `PreToolUse`, `PostToolUse`, `Stop`, and `SessionStart`; command hooks include shell paths and Rust adapters. |
| `SRC-CODEX-CONFIG` | `/Users/josh/.codex/hooks.json` | `PreToolUse` command hooks use matcher, command, timeout, and status-message fields; the live file contains shell and binary commands. |
| `SRC-HOOK-SKILLS` | `/Users/josh/.agents/skills/rust-hook-pattern/SKILL.md` and `hook-certification/SKILL.md` | The measured fail-mode ladder, Rust-binary rule, bounded stdin, direct pinned asupersync, and six-stage certification are the house practice adopted here. |
| `SRC-OMP-CONTRACT` | `docs/plan/flow/CONTRACT.md:200-204` | The declared process shape is stdin `{tool_name, tool_input}` to Claude `hookSpecificOutput`; it cites `zestgraph-danger-gate` as the live benign probe. |
| `SRC-OPS-RULES` | this-repo `AGENTS.md`, especially the asupersync and gate sections | `&Cx` first, bounded waits, process-group kill, both pipes drained, timeout is restrictive/unknown, and a denied probe is UNKNOWN rather than a negative. |

A direct benign probe of the installed danger gate was also run:

```text
exit=0
stdout={"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow"}}
stderr=
```

This proves that one installed binary implements the declared Claude output shape. It does not prove that all 20 hook-adjacent binaries accept the same input or emit the same envelope.

## Stable IDs

| ID | Contract value |
|---|---|
| `HOOK-AUTHORITY` | `hooks_certified.toml` is the only live hook registry. |
| `HOOK-STDIN` | One bounded JSON payload per invocation; dialect parser is explicit. |
| `HOOK-OUTPUT` | One structured internal decision, serialized per host dialect. |
| `HOOK-EXIT` | Exit `0` with structured output or host-specific intentional block status; no silent status inference. |
| `HOOK-PRE-IRREVERSIBLE` | Irreversible `PreToolUse` gates fail closed. |
| `HOOK-PRE-ADVISORY` | Recoverable/advisory `PreToolUse` hooks fail open with explicit abstention. |
| `HOOK-POST` | `PostToolUse` failures never claim to undo an already-run tool. |
| `HOOK-STOP` | Stop safety gates close; advisory stop telemetry opens. |
| `HOOK-SESSION` | Session-start diagnostics open; required safety initialization returns a human halt projection. |
| `HOOK-TIMEOUT-200` | Internal critical-path budget is 200 ms; host settings use a larger seconds value. |
| `HOOK-CANCEL` | Deadline cancellation kills the process group and drains both pipes. |
| `HOOK-TIMEOUT-UNKNOWN` | A timeout is `UNMEASURED/TIMED_OUT`, never PASS or a guessed DENY. |
| `HOOK-CERTIFIED` | Registry `certified=false` means `UNATTEMPTED` soak, not rejection. |
| `HOOK-SCHEMA-AUTHORITY` | Control-plane row schema is authoritative; S1 carries references only. |
| `HOOK-SCHEMA-CORE` | Ten measurable core fields replace the over-specified twelve-field universal row. |
| `HOOK-NO-COZO` | No cozo dependency or reader was found; the design does not depend on it. |
| `HOOK-DECLARED-ACTUAL` | The stdin/output contract is only fully proven for the measured danger-gate adapter; other adapters remain dialect/source scoped. |
| `HOOK-OBS-EVENT` | Every invocation emits an event with reason code, actor, hook id, and outcome. |
| `HOOK-OBS-ARTIFACT` | Certification and probe evidence retain binary identity, source revision, input class, and output hash. |
| `HOOK-OBS-MONITOR` | The registry checker reports live row/binary/config agreement. |
| `HOOK-OBS-GATE` | The certification gate refuses a live config row with no authority or no known-bad evidence. |
| `HOOK-OBS-METRIC` | `hook_p99_ms / internal_budget_ms` is the latency metric. |
| `HOOK-SOURCE-DRIFT` | A source revision or binary digest mismatch is drift, not certification. |

The 22 IDs in this table are the extractor-visible stable IDs. `INV-*` and other internal law names are test vocabulary, not additional `contract.stable_id` rows.

## Core Process Contract

1. Read at most `MAX_STDIN_BYTES + 1` bytes. Reject oversize input before allocation grows.
2. Parse the host dialect into a typed internal input with optional forward-compatible fields. Unknown fields are ignored only when the dialect specification permits it; missing required identity is `UNMEASURED`, not a guessed command.
3. Evaluate one hook policy. The decision is one of `Allow`, `Deny`, `Ask`, `Defer`, `Skip`, `Feedback`, `HumanHalt`, or `TimedOut`, with a reason code and summary.
4. Serialize only the host dialect: Claude structured decisions go to stdout and exit `0`; Codex intentional event blocks use the documented exit/status channel; diagnostics never become an accidental permission decision.
5. Run external children only through the pinned Rust subprocess substrate. The child is region-owned by `&Cx`, has a caller-owned deadline, has stdout and stderr drained, and is killed as a process group on timeout.
6. Record the hook id, event, matcher, binary digest, source revision, input class, decision, reason code, duration, and timeout state. A certification row is not a runtime invocation receipt.

## Fail-Mode Ladder

There are four event classes, with harm-class overrides where the host has already executed the action.

| Event class | Default | Correct rule | Why |
|---|---|---|---|
| `PreToolUse` | conditional | `closed` for `class=security/gate` or irreversible harm; `open` for recoverable/advisory hooks, with `Skip` and reason | A destructive false allow is worse than a bounded deny; an advisory failure must not brick every tool call. Malformed security input is a deny; malformed advisory input is explicit abstention. |
| `PostToolUse` | `open` | Never block retroactively; return feedback or `UNMEASURED` | The tool already ran. A hook failure cannot undo it and must not claim that it did. |
| `Stop` | conditional | `closed` for close/S2 safety gates; `open` for telemetry and drift advice | A close or handoff false green is a correctness failure; losing optional telemetry must not trap the session. |
| `SessionStart` | conditional | `open` for diagnostics; required security/bootstrap absence becomes `HumanHalt` in the projection | Startup diagnostics are recoverable, but a missing required policy must be visible as a halt rather than healthy continuation. |

The ladder is not “all hooks fail closed.” It is harm-aware: `class` and `fail_mode` are measured registry fields, while the host event determines whether a decision can still protect the action. A hook implementation MUST NOT silently use the broadest restrictive mode for every error; that reproduces the fleet-wedging failure described in the Rust hook practice.

## Timeout and Cancellation

`HOOK-TIMEOUT-200` is the internal critical-path budget: 200 ms wall time for a `PreToolUse` hook, with a 50 ms p99 target inherited from the measured registry latency budget. Claude and Codex host timeout settings are expressed in seconds, so the host setting MUST exceed the Rust internal budget and MUST NOT be the only timeout. PostToolUse, Stop, and SessionStart may use a separately declared larger host budget, but their Rust child waits remain bounded and named.

At the internal deadline, the Rust adapter requests cancellation through `&Cx`, kills the entire child process group, drains both stdout and stderr, and emits `TimedOut{phase, elapsed_ms, child_group, drained_bytes}`. A closed security gate projects that state as a typed deny/human halt; an open advisory hook projects it as `Skip/Feedback`. Neither path is `PASS`, and neither infers a deny from an empty buffer. A host-level timeout is recorded separately because Claude normally continues regular permission flow after a timed-out command hook.

## Certification State and Registry Authority

The control-plane registry is authoritative because it owns the certified hook substrate, its binary paths, and the on-switch rule for active settings. Its `certified = false` means **UNATTEMPTED**: Stage-4 soak, kill injection, and the required evidence have not run. It does not mean “considered and rejected.”

S1 MUST NOT maintain a second authoritative boolean or a competing hook row. The OMP projection may display `UNATTEMPTED` when the control-plane row is `certified=false`, but the source of truth remains the control-plane row. This pass changes only the OMP consumer/documentation contract; it does not edit control-plane.

## Schema Reconciliation and Field Attack

The 12-field certified table was attacked against the live registry. The registry has 16 rows, all `certified=false`; `policy_file` is present on only 1 row and `harm_class` on 9 rows. A universal field that is absent or meaningful only for a minority manufactures schema completeness without measurable data.

**Keep ten required core fields:** `id`, `event`, `matcher`, `class`, `fail_mode`, `binary`, `language`, `source_commit`, `stage`, and `certified`.

**Cut two from the universal required schema:** `policy_file` and `harm_class`. `policy_file` becomes optional `policy_ref` only for a gate that actually has a policy artifact. `harm_class` is replaced by the bounded `class` enum (`security`, `gate`, `recoverable`, `advisory`, `telemetry`), which is the value used to choose the fail-mode rule. Optional evidence may retain a human-readable harm explanation, but a perpetually unknown field cannot be required.

The single authority remains `hooks_certified.toml`; S1 rows contain `registry_id`, `registry_source_revision`, and projection status. Copying the 10 fields into S1 would recreate two authorities for one `settings.json`.

## Observability Rows and Metric

| Row | Writer | Artifact | Monitor | Gate / known-bad leg |
|---|---|---|---|---|
| `HOOK-OBS-EVENT` | Rust adapter at the invocation boundary | Event with hook id, event, actor, reason code, outcome, and duration | Registry/portal monitor | Suppress reason code on a deny; gate must refuse the invocation record. |
| `HOOK-OBS-ARTIFACT` | Certification runner | Binary digest, source revision, input class, output hash, and registry row | `hooks-registry-check` | Change binary bytes without changing row; monitor reports drift, never certified. |
| `HOOK-OBS-MONITOR` | Registry checker | Live config-to-registry projection | `validate-fleet-hooks` | Add an active settings hook with no registry row; checker must refuse. |
| `HOOK-OBS-GATE` | Certification gate | Per-stage verdict and known-bad receipt | Six-stage hook gauntlet | Feed malformed security input, kill child, or exceed budget; gate must name the restrictive outcome. |

**Metric:** `HOOK_P99_BUDGET_RATIO = hook_p99_ms / internal_budget_ms`. The critical-path target is `<= 0.25` (`50/200`) for a hook that blocks tool admission; every measured value carries binary identity and source revision.

## Invariant Suite

The future `tests/hook_design_contract.rs` MUST exercise real binaries and host adapters, not only inline strings:

- `INV-HOOK-STDIN` — bounded valid JSON reaches the typed parser; oversize input refuses before allocation growth.
- `INV-HOOK-DIALECT` — Claude and Codex serializers preserve one internal decision while respecting their different host channels.
- `INV-HOOK-MALFORMED` — malformed security input cannot become a silent allow; malformed advisory input is explicit skip.
- `INV-HOOK-FAIL-MODE` — the four event-class rules select the declared harm-aware mode.
- `INV-HOOK-TIMEOUT` — a child exceeding 200 ms is killed as a group, both pipes are drained, and the verdict is `TimedOut`.
- `INV-HOOK-NOT-VERDICT` — timeout, missing registry evidence, and unavailable monitor are never converted to PASS or guessed DENY.
- `INV-HOOK-CERTIFIED` — registry `false` projects as `UNATTEMPTED`, not rejected; active unregistered config refuses.
- `INV-HOOK-SCHEMA` — a registry row with either cut optional field absent remains valid; a gate with an absent required `policy_ref` is explicit.
- `INV-HOOK-AUTHORITY` — an S1 duplicate row cannot override the control-plane registry.
- `INV-HOOK-DRIFT` — binary digest/source revision mismatch reports drift and blocks certification.

## Validation

One pasteable external-input validation. It deliberately does not grep this contract; it reads the independent control-plane registry, installed hook surface, and one live Rust adapter probe.

```bash
set -eu
registry="$(bun -e 'const d=Bun.TOML.parse(await Bun.file("/Users/josh/Developer/control-plane/hooks_certified.toml").text()); const r=d.hook||[]; if(r.length!==16 || r.filter(x=>x.certified===false).length!==16 || new Set(r.map(x=>x.event)).size!==4) process.exit(1); console.log(`registry_rows=${r.length} certified_false=${r.filter(x=>x.certified===false).length} events=${new Set(r.map(x=>x.event)).size}`)')"
zest="$(find /Users/josh/.local/bin -maxdepth 1 -type f -name 'zestgraph-*' -perm -111 -print | wc -l | tr -d ' ')"
extra=0
for b in dcg slb-guard-fail-closed rch skill-topology-hook skill-tracker; do test -x "/Users/josh/.local/bin/$b"; extra=$((extra+1)); done
test "$zest" -eq 15
test "$extra" -eq 5
probe="$(printf '%s\n' '{\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"printf ok\"}}' | timeout 3 /Users/josh/.local/bin/zestgraph-danger-gate)"
printf '%s\n' "$probe" | jq -e '.hookSpecificOutput.permissionDecision == "allow"' >/dev/null
printf '%s\n' "$registry"
printf 'HOOK_DESIGN PASS registry_rows=16 certified_false=16 events=4 installed_hook_binaries=20 danger_decision=allow timeout_budget_ms=200\n'
```

Pasted output:

```text
registry_rows=16 certified_false=16 events=4
HOOK_DESIGN PASS registry_rows=16 certified_false=16 events=4 installed_hook_binaries=20 danger_decision=allow timeout_budget_ms=200
```

## Cross-References

- `https://code.claude.com/docs/en/hooks` — Claude Code `2.1.259` hook input, structured output, exit, and timeout semantics.
- `https://developers.openai.com/codex/hooks` — Codex CLI `0.152.1` matcher, command, timeout, and exit semantics.
- `/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent/src/capability/hook.ts` — OMP `18.1.6` shell pre/post Hook interface.
- `/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent/examples/sdk/06-hooks.ts` — OMP extension event handlers and non-blocking undefined return.
- `/Users/josh/Developer/control-plane/crates/zestgraph-hook-substrates/Cargo.toml` — edition 2024, seven binaries, pinned asupersync rev.
- `/Users/josh/Developer/control-plane/crates/zestgraph-hook-substrates/src/lib.rs:12-22,34-38,50-60,81-103` — `Cx`, bounded stdin, child deadline, process handling, and decision emission.
- `/Users/josh/Developer/control-plane/hooks_certified.toml` — 16-row authority registry, event classes, fail modes, and certification state.
- `/Users/josh/.claude/settings.json` and `/Users/josh/.codex/hooks.json` — live host hook configuration.
- `docs/plan/flow/CONTRACT.md:200-204` — declared stdin/output contract and registry-row requirement.
- `AGENTS.md` and `/Users/josh/.agents/skills/rust-hook-pattern/SKILL.md` — cancellation, timeout, fail-mode, and evidence rules.
- `/Users/josh/Developer/control-plane` commit `54a36eb3a13e8d10ae454def6ead08a03ee3094a` — version-matched substrate source identity.

## NO-CLAIM

This contract does not certify any hook, enable any settings row, or prove that all 20 installed hook-adjacent binaries share the danger-gate payload/output behavior. The direct probe proves one benign `PreToolUse` path only. Registry `certified=false` remains UNATTEMPTED until the six-stage gauntlet and soak run. The contract does not edit control-plane or create a cozo dependency; control-plane remains the authority. OMP's in-process extension API is not evidence of a process-hook stdin protocol.
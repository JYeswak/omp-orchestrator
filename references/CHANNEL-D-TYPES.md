# Channel D: OMP Type Surface

**Status:** measured reference, not a runtime dependency.

**Provenance:** `omp --version` returned `omp/18.1.14`. The installed package is
`/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent`; the package metadata and type
files were read from that installation. Counts and field shapes below must be re-derived before
reuse.

## Re-runnable inventory

```bash
omp --version
B=/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent
find "$B/dist/types" -mindepth 1 -maxdepth 1 -type d | wc -l
find "$B/dist/types" -mindepth 1 -maxdepth 1 -name '*.d.ts' | wc -l
jq '{version,types,files:(.files|map(select(. == "dist/types"))),exports:{root:.exports["."],wildcard:.exports["./*"]}}' "$B/package.json"
```

Measured output:

```text
omp/18.1.14
59
14
{
  "version": "18.1.14",
  "types": "./dist/types/index.d.ts",
  "files": ["dist/types"],
  "exports": {
    "root": {
      "types": "./dist/types/index.d.ts",
      "import": "./src/index.ts"
    },
    "wildcard": {
      "types": "./dist/types/*.d.ts",
      "import": "./src/*.ts"
    }
  }
}
```

Say **59 directories + 14 declaration files**, never “73 directories”. The package distributes the
declarations and exposes them through the root `types` condition and the wildcard subpath `types`
condition. They are consumable TypeScript contracts through package subpaths such as
`@oh-my-pi/pi-coding-agent/modes/rpc/rpc-types`; they are not runtime JavaScript. Rust cannot
link to a `.d.ts` file, so this surface is a contract source for Rust projections, not a direct
runtime dependency. Do not hard-code the filesystem path as an integration API.

## OMP surfaces already consumed by Rust

| declaration | contract relevant to this workspace | Rust consumer and gap |
|---|---|---|
| `dist/types/modes/rpc/rpc-types.d.ts` | The complete discriminated `RpcCommand` union, `RpcSessionState`, `RpcResponse`, `RpcSessionEventFrame`, and host-tool/control frames. | `crates/omp-rpc-session/src/lib.rs` consumes four commands only: `negotiate_protocol`, `get_state`, `get_session_stats`, and `get_messages`. Its `ResponseFrame.data` and `SelectedResponses` retain `serde_json::Value`, so the Rust boundary does not enforce the declaration payloads. |
| `dist/types/modes/rpc/rpc-frame.d.ts` | Protocol versions `1 | 2`, frame byte ceilings, `RpcFrameDecoder`, chunk reassembly, and stateful encoding. | Rust validates frame size and parses ready/response/unknown/malformed frames, but does not model `rpc_chunk` reassembly or the declared event-frame union. |
| `dist/types/modes/rpc/rpc-messages.d.ts` | `RpcMessagesPage` has `messages`, optional `nextCursor`, and `totalMessages`; `RpcMessagesPageOptions` has optional `cursor` and `limit`. | Rust uses the unpaged `get_messages` command and stores the response as `Value`; no `get_messages_page` request or typed cursor result exists. |
| `dist/types/session/agent-session-types.d.ts` | `RpcSessionState` imports the session `SessionStats` shape and defines `ContextUsageBreakdown`, `AgentSessionConfig`, prompt options, model-cycle results, and restored queued messages. | `crates/ompo-doctor/src/omp_state.rs` projects only a subset of state. `OmpState.model` is `Option<String>` while the declaration says `model?: Model` and the measured wire value is an object. The projection keeps `model.id` but loses provider, display name, and model capabilities. |
| `dist/types/session/session-stats.d.ts` | `SessionStatsTracker.getSessionStats()`, `getContextBreakdown()`, `getContextUsage()`, revision, and compaction epoch. | `crates/ompo-doctor/src/omp_stats.rs` has a local `OmpStats` projection. It handles numeric `cost` as `f64`, which is correct for integer or fractional cost, but omits declaration fields such as `credits` and `routedModels`; the raw payload is retained. |
| `dist/types/session/messages.d.ts` | `AgentMessage`-based message contracts, custom messages, message sanitizers, and `convertToLlm`. | `crates/ompo-doctor/src/omp_messages.rs` intentionally emits a shape summary plus raw JSON. It does not expose typed `AgentMessage` variants and does not implement the declaration's paged history surface. |
| `dist/types/session/agent-session-events.d.ts` | Session event union extending core events, including model, compaction, retry, todo, notice, and goal events. | Rust `RpcFrame` treats event frames as `Unknown`; it has no typed event projection. |
| `dist/types/extensibility/extensions/types.d.ts` | `ContextUsage` requires `tokens`, `contextWindow`, and fractional `percent`; `MessageStartEvent`, `MessageUpdateEvent`, and `MessageEndEvent` carry an `AgentMessage`. | These declarations explain the payloads behind three outbound event names. Rust currently has no matching event type. |
| `dist/types/extensibility/shared-events.d.ts` | `TurnEndEvent` carries `turnIndex`, `message`, and `toolResults`; `AgentEndEvent` carries `messages` and optional `willContinue`. | Rust currently retains such frames as unknown raw JSON. |
| `dist/types/modes/rpc/rpc-mode.d.ts` | RPC input dispatch, background `bash`, abort control, response correlation, and shutdown draining. | Rust has bounded child/process-group handling for its four requests, but no typed command arms for `bash`, `abort_bash`, or the wider command union. |

### Measured payload consequences

These commands inspect only keys and JSON types, not message content:

```bash
B=$(mktemp)
printf '%s\n%s\n' \
  '{"id":"n","type":"negotiate_protocol","protocolVersion":2}' \
  '{"id":"s","type":"get_state"}' \
  | omp --mode=rpc --no-session --no-tools --max-time=5 >"$B"
jq -c 'select(.type == "response" and .command == "get_state") |
  {command,success,data_keys:(.data|keys),model_type:(.data.model|type),
   model_id:(.data.model.id // null),contextUsage_type:(.data.contextUsage|type),
   isCompacting_type:(.data.isCompacting|type),todoPhases_type:(.data.todoPhases|type)}' "$B"
rm -f "$B"
```

Measured result:

```text
{"command":"get_state","success":true,
 "data_keys":["autoCompactionEnabled","contextUsage","dumpTools","fastModeActive",
 "fastModeEnabled","followUpMode","interruptMode","isCompacting","isStreaming",
 "messageCount","model","queuedMessageCount","sessionId","steeringMode",
 "systemPrompt","thinkingLevel","todoPhases","tokensPerSecond"],
 "model_type":"object","model_id":"gpt-5.6-luna","contextUsage_type":"object",
 "isCompacting_type":"boolean","todoPhases_type":"array"}
```

The statistics probe returned these keys: `assistantMessages`, `contextUsage`, `cost`,
`premiumRequests`, `sessionFile`, `sessionId`, `tokens`, `toolCalls`, `toolResults`,
`totalMessages`, and `userMessages`. `tokens` and `contextUsage` were objects; `cost` was a
number. The Rust `omp_stats` projection gets the numeric tolerance right but is narrower than the
published declaration.

## Relevant declarations not yet consumed

These files describe real OMP surfaces but are not Rust runtime dependencies in this workspace:

- `dist/types/session/session-listing.d.ts` — session status (`complete`, `interrupted`, `aborted`,
  `error`, `pending`, `unknown`) and resumable-session resolution.
- `dist/types/task/omp-command.d.ts`, `task/types.d.ts`, and `task/commands.d.ts` — OMP command
  resolution, task/subagent types, and workflow command discovery.
- `dist/types/goals/state.d.ts` and `goals/runtime.d.ts` — goal lifecycle, budgets, continuation,
  and goal runtime events.
- `dist/types/plan-mode/state.d.ts` — plan-mode state.
- `dist/types/subprocess/worker-client.d.ts` — worker process protocol.
- `dist/types/exec/exec.d.ts` — cancellable command execution with `stdout`, `stderr`, `code`, and
  `killed` fields.
- `dist/types/capability/types.d.ts` — capability and extension contracts.
- `dist/types/registry/*.d.ts`, `collab/*.d.ts`, and `live/*.d.ts` — agent registry, collaboration,
  and live-session surfaces.

“Not consumed” means no Rust type dependency exists; it does not mean these surfaces are absent or
should be adopted. Each needs a separate consumer decision and a live contract check.

## Outbound notification result

The current seam-derived six outbound names are:

```text
tool_stream_update
message_update
message_start
message_end
turn_end
agent_end
```

The fresh alignment runner reports:

```text
ALIGN_SEAM inbound=42 outbound=6 seam_gap_bytes=2206
ALIGN_MANIFEST state=FULL omp_root=omp omp_version=omp/18.1.14
ALIGN_COVERAGE kind=rpc_handler total=42 classified=3 classified_bps=714
ALIGN_COVERAGE kind=rpc_notification total=6 classified=0 classified_bps=0
```

Two bounded no-input runs and a state-only control produced `ready`, `extension_ui_request`, and
`available_commands_update`; none produced the six names. The no-input summary was:

```text
1 available_commands_update
2 extension_ui_request
1 ready
```

Therefore **no spontaneous six-event push was observed on an idle `--mode=rpc` process**. This does
not show that the events never occur during an active prompt. The declarations classify
`message_start`, `message_update`, `message_end`, `turn_end`, and `agent_end` as session events,
not response payloads; `rpc_notification` remains the correct direction kind. The sixth name,
`tool_stream_update`, is present in the bundle extraction but has **zero matches under
`dist/types`**, so the installed declaration surface is incomplete for that event.

## OMP integration skill delta

The skill says the unknown-type negative control draws no response frame. The current installed OMP
responds instead:

```bash
printf '%s\n' '{"id":"control","type":"zzz_cannot_exist"}' \
  | omp --mode=rpc --max-time=5
```

Observed response:

```json
{"type":"response","command":"zzz_cannot_exist","success":false,"error":"Unknown command: zzz_cannot_exist"}
```

This is a skill delta, not a claim that unknown commands are valid. The control distinguishes an
explicit OMP refusal from no frame; future probes must use the observed response behavior until the
skill is corrected. `available_commands_update` remains asynchronous, so fixed-read probes are
still insufficient.

## No-claim boundary

The type files are versioned package artifacts measured at `omp/18.1.14`, not a promise that a future
OMP release preserves these declarations. The Rust projections retain raw payloads where possible,
but the current code does not provide full TypeScript-equivalent type coverage. The no-input probe
did not exercise an active `prompt`, so it does not classify prompt-triggered event delivery.

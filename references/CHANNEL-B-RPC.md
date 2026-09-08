# Channel B: OMP `--mode=rpc` Group 1

**Owner:** pane `%19` (`pane19-omp-luna`)
**Artifact:** `OMP-RPC-SURFACE-MAP.md`
**Measured:** 2026-09-08
**Installed OMP:** `omp/18.1.14` at `/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js`

## Protocol precondition

`--mode=rpc` is not JSON-RPC. Requests are `{"id","type"}` frames. A valid probe must send
`negotiate_protocol` first; sending only the target request produced no target response. The
bounded command shape is:

```bash
SESSION=/path/to/session.jsonl
SESSION_DIR=/path/to/session-directory
printf '%s\n%s\n' \
  '{"id":"n","type":"negotiate_protocol","protocolVersion":2}' \
  '{"id":"1","type":"<method>"}' \
| perl -e '$SIG{ALRM}=sub { exit 124 }; alarm 45; exec @ARGV' -- \
    omp --mode=rpc --session "$SESSION" --session-dir "$SESSION_DIR"
```

The probe must classify these separately:

- response with `success=true`: `ANSWERS`
- response with `success=false` and an error: `REFUSES`
- success with no meaningful payload: `NO_PAYLOAD`
- no target response frame: `NOT_ADDRESSABLE` only when the no-frame control also behaves distinctly
- deadline: `TIMEOUT_UNMEASURED`, never a protocol verdict

## Group 1 results

The resumed-session fixture used below was
`/Users/josh/.local/state/zeststream/scratch/omp-orchestrator/WildStone/rpc-resume-profile/sessions/session.jsonl`.
It is scratch state, not a live pane proof. Each OMP invocation can append to that fixture; total
message counts therefore drift between probes.

| method | request shape | observed response | disposition |
|---|---|---|---|
| `get_messages_page` | `{"type":"get_messages_page","limit":N}`; optional `cursor` | `success=true`; data keys `messages`, `nextCursor`, `totalMessages` | **Answerer:** extend existing `ompo messages` with bounded `--limit`/`--cursor`; this is the one Group 1 capability worth wiring now. `%20` owns `omp_messages`. |
| `get_last_assistant_text` | `{"type":"get_last_assistant_text"}` | `success=true`, data `{}` on both a fresh session and the resumed scratch fixture; no `text` observed | **Do not wire yet.** A successful empty object cannot retire the spinner oracle. `%8` is the candidate answerer once a non-empty assistant-text payload is proven. |
| `abort` | `{"type":"abort"}` on idle session | `success=true`, `data=null` | **Do not wire.** Idle success is a safe no-op; one-shot RPC cannot control an existing pane. |
| `abort_bash` | idle: `{"type":"abort_bash"}`; active probe used `bash sleep 10` then `abort_bash` | idle `success=true`, `data=null`; active abort response `success=true`, but bash later returned `success=true`, `exitCode=0`, `cancelled=false` | **Do not wire.** The bounded active probe did not demonstrate cancellation. |
| `abort_retry` | `{"type":"abort_retry"}` on idle session | `success=true`, `data=null` | **Do not wire.** Same one-shot/no-live-pane boundary; idle response proves only a no-op acknowledgement. |
| `get_available_models` | `{"type":"get_available_models"}` | `success=true`; 9 model records with `provider`, `id`, and `name` | Supporting probe only; not an operator verb by itself. |
| `set_model` | `{"type":"set_model","provider":"openai-codex","modelId":"gpt-5.6-luna"}` | `success=true`; data is a full model record | **Do not wire yet.** A current-model set is not a model transition. An attempted switch to `gpt-5.6-sol` in the same sequence emitted `Error: cannot use null as iterable`; classify alternate switching as `UNKNOWN/UPSTREAM_ERROR`. |
| `cycle_model` | `{"type":"cycle_model"}` followed by `get_state` | `success=true`; response carried `model`, `isScoped`, `thinkingLevel`; state changed `gpt-5.6-luna → gpt-5.6-sol` | **Do not wire.** It changes only the supervisor-spawned one-shot child, not a running tmux pane. |
| `get_subagents` | `{"type":"get_subagents"}` | `success=true`, `{"subagents":[]}` | **Do not wire.** Empty-session observability adds no operator capability; re-probe against an owned session with a real subagent before adopting. |

## Pagination measurements

On the resumed scratch fixture, `get_messages_page` returned a typed page in under the 45-second
bound with a `nextCursor`:

```text
limit=1    messages=1   nextCursor=true
limit=10   messages=10  nextCursor=true
limit=32   messages=32  nextCursor=true
limit=64   messages=64  nextCursor=true
limit=67   messages=67  nextCursor=true
limit=68   messages=67  nextCursor=true
limit=128  messages=67  nextCursor=true
limit=256  messages=67  nextCursor=true
```

The observed effective page ceiling is 67 for this fixture. The exact ceiling is not a universal
contract until re-derived against another session/version. `limit` is the page-size field; the
response carries `nextCursor` and `totalMessages`. A stale cursor after the fixture changed emitted
`Error: cannot use null as iterable`; that is `UNKNOWN` cursor behavior, not a successful continuation
claim.

## Wiring decisions

1. **Wire `get_messages_page` through the existing `ompo messages` answerer**, adding bounded page
   arguments rather than inventing `ompo get_messages_page`. It solves the measured resumed-session
   timeout and preserves one operator-facing message surface.
2. **Do not wire cancellation or model-control verbs yet.** Their successful responses target the
   one-shot child and the active `abort_bash` probe did not cancel the running command.
3. **Do not wire `get_subagents` from an empty-session probe.** The empty array is a truthful result,
   not proof of useful fleet observability.
4. **Do not wire `get_last_assistant_text` until a non-empty `text` payload is measured.** Its
   candidate answerer is the `%8` pane oracle, not a new standalone CLI verb.

The four typed request variants currently in `omp-rpc-session` are only
`NegotiateProtocol`, `GetState`, `GetSessionStats`, and `GetMessages`. Group 1 methods remain
unwired until their owning panes add typed request variants and a real operator caller.

## Skill delta

**CLAIM:** Probe Channel B with a `negotiate_protocol` frame first, use `get_messages_page` for
bounded resumed-session history, and do not advertise idle acknowledgements as cancellation or
live-pane control.

**COMMAND:**

```bash
SESSION=/path/to/session.jsonl
SESSION_DIR=/path/to/session-directory
printf '%s\n%s\n' \
  '{"id":"n","type":"negotiate_protocol","protocolVersion":2}' \
  '{"id":"1","type":"get_messages_page","limit":64}' \
| perl -e '$SIG{ALRM}=sub { exit 124 }; alarm 45; exec @ARGV' -- \
    omp --mode=rpc --session "$SESSION" --session-dir "$SESSION_DIR"
```

**PROVENANCE:** installed `omp/18.1.14`, bundle path above, probe host state on 2026-09-08;
repo reference committed after verification. Counts and page ceilings are fixture/version-bound.

**NO-CLAIM:** These probes run supervisor-spawned RPC sessions and a mutable scratch fixture. They
do not attach to or control an existing tmux pane, prove non-empty last-assistant text, prove a
foreign-session cursor continuation, or establish that `abort` cancels a live pane operation.

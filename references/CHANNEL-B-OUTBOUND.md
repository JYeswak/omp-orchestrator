# Channel B: OMP RPC Outbound Events

**Consumer:** future OMP event consumers and reviewers of the `omp-rpc-session` frame boundary.
**Defect class:** closing stdin before asynchronous frames arrive makes a push channel look absent;
using the wrong seam denominator makes event coverage unverifiable; a bundle event can exist
without a matching declaration. **Deletion condition:** remove this reference when OMP publishes a
versioned outbound-event contract and this repository has a typed event consumer.

**Provenance:** `omp --version` returned `omp/18.1.14`. The installed bundle is
`/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js`.

## The six derived names

The current seam-derived output is:

```text
tool_stream_update
message_update
message_start
message_end
turn_end
agent_end
```

The aligner reports:

```text
ALIGN_SEAM inbound=42 outbound=6 seam_gap_bytes=2206
ALIGN_MANIFEST state=FULL omp_root=omp omp_version=omp/18.1.14
ALIGN_COVERAGE kind=rpc_notification total=6 classified=0 classified_bps=0
```

The `2206`-byte seam remains an inference. A small seam gap is a reason to inspect the boundary,
not permission to treat the six as a protocol specification.

## Active-prompt probe

An idle process is not enough: it emits startup frames but does not enter a turn. The probe sent a
real prompt after protocol negotiation, kept stdin open so asynchronous frames could arrive, and
captured stdout until the OMP deadline:

```bash
p=$(mktemp)
{
  printf '%s\n%s\n' \
    '{"id":"n","type":"negotiate_protocol","protocolVersion":2}' \
    '{"id":"1","type":"prompt","message":"Reply with OK."}'
  sleep 60
} | omp --mode=rpc --no-session --no-tools --max-time=60 >"$p"
rc=$?
printf 'subject_rc=%s\n' "$rc"
printf 'frame_types=\n'
jq -r '.type // "<missing>"' "$p" | sort | uniq -c
rm -f "$p"
```

The read strategy is deliberate: `--max-time=60` is the subject deadline, the producer holds the
input pipe open for the same bounded interval, and frame types are parsed only after the process
finishes. No fixed early read is used.

Measured result:

```text
subject_rc=0
      1 agent_end
      1 agent_start
      1 available_commands_update
      3 extension_ui_request
      2 message_end
      2 message_start
      3 message_update
      1 ready
      2 response
      1 turn_end
      1 turn_start
```

The event-frame shapes were summarized without printing message content:

```text
message_start: 2 frames; message keys included role, content, timestamp, attribution,
               and for the assistant message api, model, provider, stopReason, usage
message_update: 3 frames; assistantMessageEvent keys included type, partial, contentIndex,
                and on later frames delta/content
message_end: 2 frames; message keys included role, content, timestamp and assistant metadata
turn_end: 1 frame; keys message, toolResults, turnIndex, type
agent_end: 1 frame; keys messages, isTerminal, type; messages count=2
tool_stream_update: 0 frames; the probe used --no-tools
```

Five event names therefore **are emitted during an active prompt without a client request for each
frame**: `message_start`, `message_update`, `message_end`, `turn_end`, and `agent_end`. This is the
first measured push-per-turn channel in this fleet. The two response frames are the negotiated and
prompt-correlated replies; they are not the event receipt.

`tool_stream_update` is **UNKNOWN**, not absent, because the probe disabled tools. A tool-enabled
prompt would be required to exercise it. Do not lower the denominator to five based on this probe.

## Seam-versus-declaration test

The seam explanation and the declaration explanation are distinguishable from the installed
bundle. This bounded inspection applies the scanner's actual anchor, cluster-gap, and largest-gap
rules, then reports the location of `tool_stream_update`:

```bash
node -e 'const fs=require("fs"); const p="/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js"; const t=fs.readFileSync(p,"utf8"); const re=/case"([a-z][a-z0-9_]*)"/g; const s=[]; let m; while((m=re.exec(t))) s.push({name:m[1],offset:m.index}); const a=s.findIndex(x=>x.name==="negotiate_protocol"); let lo=a,hi=a; while(lo>0 && s[lo].offset-s[lo-1].offset<4000) lo--; while(hi+1<s.length && s[hi+1].offset-s[hi].offset<4000) hi++; const c=s.slice(lo,hi+1); let k=0; for(let i=1;i<c.length;i++){if(c[i].offset-c[i-1].offset>c[k+1].offset-c[k].offset) k=i-1;} console.log(JSON.stringify({all_case_sites:s.length,cluster_size:c.length,seam_index:k,seam_gap_bytes:c[k+1].offset-c[k].offset,inbound:c.slice(0,k+1).map(x=>x.name),outbound:c.slice(k+1).map(x=>x.name),tool_stream_sites:s.filter(x=>x.name==="tool_stream_update").map(x=>({offset:x.offset,cluster_index:c.findIndex(y=>y.offset==x.offset),side:c.findIndex(y=>y.offset==x.offset)<=k?"inbound":"outbound"}))},null,2));'
```

Measured output:

```text
all_case_sites=2747
cluster_size=48
seam_index=41
seam_gap_bytes=2206
tool_stream_update: cluster_index=42, side=outbound
outbound: tool_stream_update, message_update, message_start, message_end, turn_end, agent_end
```

`tool_stream_update` is a real `case` site immediately after the inferred seam. The denominator
six is not a seam mis-split. Its absence from the installed declaration tree is the separate
finding:

```text
tool.grep tool_stream_update under dist/types -> matchCount=0
message_start/message_update/message_end in extensibility/extensions/types.d.ts -> present
turn_end/agent_end in shared event declarations -> present
```

The right verdict is **declaration gap plus conditional runtime reachability**, not “outbound is
five”. The bundle branch exists, and the no-tools prompt did not exercise the branch that would
stream tool updates.

## Consumer decision

No Rust consumer exists today. `crates/omp-rpc-session` retains non-response event frames as
`RpcFrame::Unknown`, and no package metadata declaration names `rpc_notification`. Do not declare
a speculative consumer. A future consumer must first define typed event payloads and a real
callsite, then declare `kind = "rpc_notification"` with the event name and callsite.

## Gate status

No mechanism or source gate was added in this pass. The deliverable is a probe table and a
confirmed denominator. Mutation, known-good, byte-restore, and wiring gate legs are therefore not
applicable; inventing them would claim an event consumer that does not exist.

## OMP integration skill delta

```text
CLAIM      An active OMP RPC prompt emits message/turn/agent event frames asynchronously; keep
           stdin open through a stated --max-time deadline and classify a no-tools
           tool_stream_update miss as UNKNOWN. The unknown-command no-frame control in the skill
           also remains contradicted by the observed explicit refusal response.
COMMAND    { printf '%s\\n%s\\n' '{"id":"n","type":"negotiate_protocol","protocolVersion":2}' '{"id":"1","type":"prompt","message":"Reply with OK."}'; sleep 60; } | omp --mode=rpc --no-session --no-tools --max-time=60
PROVENANCE omp/18.1.14; bundle dist/cli.js; current repository WORKTREE; probe returned subject_rc=0 and five event kinds.
```

## No-claim boundary

This proves event emission during one fresh no-tools prompt, not delivery to a resumed live pane,
not tool-stream emission, and not long-running prompt cancellation. The event declarations are
versioned package artifacts and may drift independently of the bundle. No OMP installation or
Rust source was changed.

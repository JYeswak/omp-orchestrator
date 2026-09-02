# OMP surface coverage — wave `ipg.4` (DISPATCH): irc, collab, jsonrpc, mcp, launch, exec, subprocess

Bead: `omp-orchestrator-omp-coverage-mission-ipg.4` — Wave DISPATCH: irc, collab, jsonrpc, mcp, launch, exec, subprocess

## Purpose

This document classifies the seven OMP **DISPATCH** surfaces against the eight-clause per-crate contract. It maps what exists in the installed OMP v18.0.11 type surface to the local dispatch boundaries; it does not adopt OMP types and it does not claim that a local alternative is correct merely because it is wired.

The machine-readable rows live in `docs/plan/OMP-COVERAGE-TABLE.jsonl`. The roll-up lives in `docs/inventories/omp_surface_coverage_index.md`. This wave was absent from that index until this document was added.

The sweep was performed against `/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent/dist/types` and its seven named roots. It found **7 surfaces, 52 declaration files, 276 allocated KB, and 363 top-level exported declaration lines** using the source-aware counting rule retained in the command evidence. Zero surfaces is an error; the denominator is not inferred from the table length.

---

> **ipg.4**: *each surface gets a coverage-table row with all 8 columns + classification (a) not ours / (b) reimplemented by scraping / (c) unused capability.*

## Coverage table

| surface | OMP files | OMP KB | OMP symbols | 1 asuper | 2 forbid | 3 cancel | 4 typed | 5 logged | 6 observable | 7 robot | 8 WIRED | classification |
|---|---:|---:|---:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|
| `irc` | 1 | 8 | 4 | — | — | — | — | — | — | — | — | **(c) UNUSED CAPABILITY** — OMP `IrcBus`, `IrcMessage`, and `IrcDeliveryReceipt` provide in-process agent-to-agent mailbox delivery; this orchestrator has no local IRC requirement or consumer. |
| `collab` | 7 | 32 | 33 | — | — | — | — | — | — | — | — | **(c) UNUSED CAPABILITY** — OMP `CollabSessionState`, `AgentSnapshot`, `RelayControlMessage`, and `GuestIdleReconcilerCtx` provide live-session collaboration and replication; this orchestrator does not adopt that session plane. |
| `jsonrpc` | 1 | 4 | 1 | ✓¹ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓¹ | **(b) REIMPLEMENTED BY SCRAPING** — local `omp-rpc-session` drives the OMP `--mode=rpc` process and reconstructs the protocol boundary; OMP's existing alternative is `MessageFramer` in `jsonrpc/message-framing.d.ts`. |
| `mcp` | 26 | 144 | 214 | ✓¹ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓¹ | **(b) REIMPLEMENTED BY SCRAPING** — local `agent-mail-native::MailClient` speaks JSON-RPC over HTTP to an MCP endpoint; OMP's existing alternatives are `callMCP`, `JsonRpcResponse`, and `McpClient`. |
| `launch` | 11 | 48 | 56 | ✓¹ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓¹ | **(b) REIMPLEMENTED BY SCRAPING** — local `omp-orchestrator` and NTM/tmux adapters launch and observe worker processes; OMP's existing alternative is the typed `DaemonBroker` / `DaemonSpec` / `DaemonWireRequest` protocol. |
| `exec` | 4 | 20 | 19 | ✓¹ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓¹ | **(b) REIMPLEMENTED BY SCRAPING** — local subprocess owners execute bounded commands and parse their output; OMP's existing alternative is `execCommand` / `ExecResult` plus `executeBash` / `BashResult`. |
| `subprocess` | 2 | 20 | 36 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | **(b) REIMPLEMENTED BY SCRAPING** — local `subprocess-contract` owns process-group cancellation, both-pipe draining, typed timeout outcomes, and robot-consumable callers; OMP's existing alternatives are `WorkerRuntime` and `WorkerClient`. **FULLY COVERED at the local contract boundary; this is not OMP type adoption.** |

¹ The checkmarks on category-(b) rows describe the local alternative's dispatch boundary, not consumption of the OMP declaration files. A row can therefore be fully covered as a local contract while remaining `MAPPED_NOT_ADOPTED` in the machine ledger.

## Positive control and anti-vacuity

**Positive control: PASSED — `subprocess` reports FULLY COVERED at the local contract boundary.** The positive control is real because `crates/subprocess-contract/src/lib.rs` exists, begins with `#![forbid(unsafe_code)]`, exposes both asupersync `Cx`-owned async functions and bounded synchronous adapters, and is consumed by dispatch crates. This does not claim that OMP's `WorkerRuntime` or `WorkerClient` types are imported.

**Anti-vacuity: PASSED — 7 surfaces enumerated, 52 files walked, 363 exported declaration lines counted.** The table is not empty, each declared root was present, and the positive control is not manufactured from a zero-row result.

## Per-surface detail

### `irc` — (c) UNUSED CAPABILITY

The OMP root contains `IrcBus`, `IrcMessage`, `IrcDeliveryReceipt`, and `IrcAwaitTargetStopped`. Its semantics are process-global in-agent mailbox delivery, waking or reviving recipient sessions and optionally waiting for a reply. The local repository's dispatch path sends packets through NTM/tmux and records receiver observations; no local crate imports these IRC declarations. This is a legitimate unused capability, not an absent measurement.

### `collab` — (c) UNUSED CAPABILITY

The OMP root contains collaboration-session state, relay protocol, host/guest snapshots, participant identity, replication shrink, and the typed idle reconciler context. The local repository has no collab-session consumer. The classification is unused capability, not a claim that the OMP collaboration implementation is incomplete.

### `jsonrpc` — (b) REIMPLEMENTED BY SCRAPING

OMP's `MessageFramer` owns incremental Content-Length framing, remainder persistence, and resynchronization. The local alternative is `crates/omp-rpc-session/src/lib.rs`: `OmpCommand::new` adds `--mode=rpc`, `RpcRequest::sequence` sends the fixed request set, `ResponseFrame` carries typed success/error state, and `RpcSessionConfig` owns bounded startup/request/shutdown deadlines. The local adapter is wired by `crates/omp-orchestrator/src/main.rs` through `run_omp_quick`. It maps the transport boundary but does not consume OMP's TypeScript `MessageFramer` declaration.

### `mcp` — (b) REIMPLEMENTED BY SCRAPING

OMP exposes `callMCP`, `JsonRpcResponse`, `CallMcpOptions`, and MCP transport/client types. The local alternative is `crates/agent-mail-native/src/client.rs`: `MailClient::call_tool` builds a JSON-RPC `tools/call` request, checks the authenticated MCP endpoint, and decodes the double-encoded tool payload. `crates/omp-orchestrator/src/main.rs` consumes the native binding for durable dispatch-result notification. The local path is typed and bounded, but it is a local binding to the wire shape rather than adoption of the OMP MCP declaration files.

### `launch` — (b) REIMPLEMENTED BY SCRAPING

OMP's launch root types the daemon broker: `DaemonState`, `DaemonRestartPolicy`, `DaemonSpec`, `DaemonSnapshot`, `DaemonOperation`, `DaemonRpcResult`, and authenticated wire request/response messages. The local alternative is the orchestrator's process boundary in `crates/omp-orchestrator/src/main.rs:354-384`, where `invoke` runs configured binaries through `subprocess-contract::run_output`, and the NTM/tmux dispatch surfaces provide worker launch and pane targeting. The local path observes command output and lifecycle rows; it does not consume the OMP daemon broker types.

### `exec` — (b) REIMPLEMENTED BY SCRAPING

OMP's `ExecOptions` and `ExecResult` expose a command, arguments, cwd, timeout, abort signal, stdout, stderr, exit code, and killed state. `BashExecutorOptions` and `BashResult` add shell selection, PTY, artifact capture, cancellation, truncation, and output accounting. The local alternative is the Rust subprocess boundary, primarily `crates/subprocess-contract/src/lib.rs` and its callers. That boundary re-expresses the observable process result in Rust typed outcomes and gate-specific rows rather than consuming OMP's TypeScript exec types.

### `subprocess` — (b) REIMPLEMENTED BY SCRAPING

OMP's `WorkerRuntime` and `WorkerClient` type roots describe the worker-process execution boundary. The local alternative is `crates/subprocess-contract/src/lib.rs`, which configures process groups, drains stdout and stderr concurrently, distinguishes `Completed`, `TimedOut`, and `Unspawned`, and exposes the asupersync `Cx`-first `run_output` and `run_status` APIs. This is the positive control because the local eight-clause contract is fully covered and wired. The mapping remains non-adoption: no local Rust crate imports OMP's declaration files.

## Contract interpretation

The eight check columns answer whether the **local alternative** has the required boundary property. A dash means the surface is outside this repository's ownership boundary or is an unused capability; it is not a failed local implementation. For category-(b) rows, the checkmarks are output-plane coverage of the alternative, with `asuper` and `WIRED` footnoted where the local owner is the repository's own Rust boundary. They do not turn scraping into correctness or make the OMP type plane disappear.

The dispatch alternative preserves the independent authorities that existed before this map: pane truth remains authoritative for pane state, NTM/tmux remains the transport observation surface, Agent Mail remains the durable communication surface, and the subprocess contract remains the process lifecycle boundary. A successful command is not a receipt, an empty result is not healthy, and a timeout is not a substantive failure verdict.

## Cross-References

- `docs/inventories/omp_surface_coverage_index.md` — the eleven-wave roll-up
- `docs/plan/12-journey.md` — the nine-stage runbook and historical wave declaration
- `docs/plan/OMP-COVERAGE-TABLE.jsonl` — machine-readable per-surface dispositions
- `docs/plan/02-surface-census.md` — the installed OMP surface census and counting caveats
- `crates/omp-rpc-session/src/lib.rs` — local `--mode=rpc` adapter
- `crates/agent-mail-native/src/client.rs` — local MCP JSON-RPC binding
- `crates/omp-orchestrator/src/main.rs` — local launch/exec/dispatch boundary
- `crates/subprocess-contract/src/lib.rs` — local subprocess contract and positive control
- `.beads/issues.jsonl` — `omp-orchestrator-omp-coverage-mission-ipg.4`

## Validation

Derives the document's surface set from its table and compares it with the seven surfaces in the bead title. It rejects an empty side, so a missing document or malformed table cannot report success. The same command is the one pasteable validation command for this document.

```bash
cd /Users/josh/Developer/omp-orchestrator && W=4 && \
D=$(sed -nE 's/^\| `?([a-z0-9:_-]+)`? \|.*/\1/p' "docs/inventories/omp_surface_coverage_ipg${W}.md" \
    | grep -vE '^(surface|-+)$' | sort -u) && \
B=$(jq -r --arg id "omp-orchestrator-omp-coverage-mission-ipg.${W}" \
      'select(.id==$id)|.title' .beads/issues.jsonl | head -1 \
    | sed 's/^[^:]*: *//' | tr ',' '\n' | tr -d ' ' | sed '/^$/d' | sort -u) && \
[ -n "$D" ] && [ -n "$B" ] && diff <(printf '%s\n' "$D") <(printf '%s\n' "$B") \
  && echo "PASS ipg.${W}" || { echo "FAIL ipg.${W}"; exit 1; }
```

## NO-CLAIM

This is a coverage map, not an adoption or correctness claim. The `(a)`/`(b)`/`(c)` classification records ownership and present alternatives; it does not authorize replacing the local dispatch, receipt, pane-truth, or cancellation contracts. `irc` and `collab` are legitimate unused capabilities. The `subprocess` positive control proves only that the local contract boundary has all eight declared properties; it does not prove OMP's worker types are consumed, nor that every dispatch outcome reaches a receiver. A complete map can still describe a starving fleet — `tick-monitor` had manifest callers and a live process while dispatch starved for 4.5 hours.

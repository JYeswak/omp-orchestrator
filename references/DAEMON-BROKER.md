# OMP daemon broker: project-scoped process supervision

**Owner:** pane `%20` (`FuchsiaDuck`)
**Measured:** 2026-09-08
**Installed OMP:** `omp/18.1.14`
**Repository tree for Step 1:** `3e80d70524fa940f29f77ec42389b05ecc97a3b0`

## Surface decision

`omp ps` is the operator-facing surface identity. The broker reports named processes such as
`omp.browser.headless`, `omp.lsp.mux`, and other supervised lanes, but the OMP surface declaration
names the query interface rather than one row per reported process:

```toml
[package.metadata.omp_surface]
consumes = [
    { kind = "daemon_process", name = "omp ps", call_site = "src/omp_process.rs" },
]
```

The declaration is intentionally not part of Step 1. The deriver must emit the matching
`SurfaceEntry { kind = "daemon_process", name = "omp ps" }` before `DECLARED_AXES` or the Cargo
metadata declaration is added; otherwise alignment becomes partial or reports an orphan.

## Scope identity

The current project scope is:

```text
project:     /Users/josh/Developer/omp-orchestrator
profile:     codex
scope key:   79012643b5202611
runtime:     /Users/josh/.omp/profiles/codex/run/daemons/79012643b5202611
```

The four observed codex profile scope keys and their `scope.json` project paths were:

| scope key | project |
|---|---|
| `645d18f9e7fe9f49` | `/Users/josh/Developer/control-plane` |
| `79012643b5202611` | `/Users/josh/Developer/omp-orchestrator` |
| `cd507419e40040de` | `/Users/josh/Developer/franken-harvest` |
| `f1e8eab282a49088` | `/Users/josh/Developer/clutterfreespaces.ios` |

The installed bundle contains the construction:

```text
Bun.hash.wyhash(resolve(dir)).toString(16).padStart(16, "0")
```

Reproduction command:

```bash
/opt/homebrew/bin/bun -e 'const path=require("node:path"); for (const p of process.argv.slice(1)) console.log(JSON.stringify({input:p,resolved:path.resolve(p),hash:Bun.hash.wyhash(path.resolve(p)).toString(16).padStart(16,"0")}))' \
  /Users/josh/Developer/control-plane \
  /Users/josh/Developer/omp-orchestrator \
  /Users/josh/Developer/franken-harvest \
  /Users/josh/Developer/clutterfreespaces.ios
```

Observed hashes matched the four scope directory names exactly. This makes the project path the
join key; project path and runtime profile are runtime data, not Cargo declaration fields.

## `omp ps --json` contract observed

Probe command:

```bash
omp ps --json --dir /Users/josh/Developer/omp-orchestrator
```

The result is an array. The project scope object has:

```text
kind:       string
projectDir: string
runtimeDir: string
brokerPid:  number
daemons:    array
```

Each daemon row has the following stable fields in the observed version:

```text
name:         string
id:           string
state:        string
pid:          number for running rows; absent/null for exited rows
createdAt:    number
startedAt:    number
readyAt:      number
restartCount: number
outputBytes:  number
readyMatch:   string
persist:      boolean
detached:     boolean
command:      string
cwd:          string
supervised:   boolean
exitCode:     number on exited rows; absent/null while running
exitedAt:     number on exited rows; absent/null while running
```

Observed states were `ready` and `exited`. `--all` adds project scopes and a global browser-relay
scope; its top-level union includes `service`, while exited rows add `exitCode` and `exitedAt` and
omit the live `pid`.

The Step 1 parser is `crates/ompo-doctor/src/omp_process.rs`:

- `parse_ps_json` parses the array without starting OMP.
- `classify_ps_output` keeps child exit code separate from JSON/verdict classification.
- `read_processes` executes only `omp ps --json --dir <repo>` through the bounded subprocess
  contract. It never invokes `stop`, `kill`, or `restart`.
- Invalid JSON, wrong top-level shape, non-object daemon rows, timeout, and spawn failure are
  typed failures; none is promoted to a healthy answer.

## Named endpoint and authentication

The current broker endpoint is:

```text
/Users/josh/.omp/profiles/codex/run/daemons/79012643b5202611/broker.sock
```

The adjacent token file exists at `.../broker.token`. Its contents were not read or copied.
Read-only metadata showed both paths owned by `josh`, mode `0600`; `broker.sock` is a Unix socket
and `broker.token` is a 64-byte regular file.

Installed source evidence:

```text
src/launch/broker.ts
  DaemonBroker.run() creates a Unix server, accepts newline-delimited JSON, and compares the
  request token to broker.token before dispatching.
  Responses are newline-delimited JSON: { id, ok: true, result } or { id, ok: false, error }.

src/launch/paths.ts
  daemonBrokerEndpoint(projectDir, runtimeDir) resolves to runtimeDir/broker.sock on non-Windows.

src/launch/protocol.ts
  Defines and validates daemon wire requests and broker operations including ping, start, list,
  logs, wait, signal, stop, restart, and shutdown.
```

The CLI help documents `auth-broker` and `auth-gateway`, but not the complete daemon socket wire
schema. The socket is therefore documented by installed source, not by a stable public CLI
protocol contract. No socket connection was attempted; no token was used; no lifecycle mutation
was invoked.

## Verification boundaries

Remote verification for the Step 1 crate change:

```bash
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test -j 2 -p ompo-doctor
```

Observed `Remote command finished: exit=0` with all reported test groups passing. The remote test
was run after formatting `omp_process.rs`; no local Cargo build was used.

## Skill delta

**CLAIM:** Integrators should derive the project scope key with
`Bun.hash.wyhash(resolve(projectDir)).toString(16).padStart(16, "0")`, then consume `omp ps` as a
project-scoped daemon-process surface; do not treat each reported process as a separate surface
identity and do not connect to `broker.sock` without its token-authenticated protocol contract.

**COMMAND:**

```bash
omp ps --json --dir /Users/josh/Developer/omp-orchestrator
```

**PROVENANCE:** `omp/18.1.14`; scope mapping and socket/source inspection measured on 2026-09-08;
Step 1 tree `3e80d70524fa940f29f77ec42389b05ecc97a3b0`.

**NO-CLAIM:** This reference does not prove socket responsiveness, token validity, protocol stability
across OMP versions, or the health of every project/profile. It does not authorize stop/kill/restart
or any write under `~/.omp/`.

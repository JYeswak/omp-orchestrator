# Channel C: OMP LSP multiplexer

**Bead:** `omp-orchestrator-hgzx`
**Owner:** pane `%19` (`pane19-omp-luna`)
**Measured:** 2026-09-08

## Scope

The three `omp/*` methods are Channel C of OMP's integration surface:

- `omp/muxConnect`
- `omp/muxPing`
- `omp/muxRestartServer`

They are **LSP multiplexer** methods, not tmux, pane, or agent-session methods. The installed
bundle defines `__omp_worker_lsp_mux`, `OMP_LSP_MUX_SOCKET`, `OMP_LSP_MUX_PROJECT_DIR`,
`omp.lsp.mux`, the ready banner `omp lsp mux listening on \\S+`, and `pong`. Its Unix address is
`path.join(dir, "lsp-mux.sock")`; the Windows alternative is a hashed named pipe. The client uses
`net.connect(path)` with a timeout rejection.

## Reachability result

The observed workers (PIDs `43609` and `75508`) ran
`bun …/dist/cli.js __omp_worker_lsp_mux` and held only anonymous inherited socketpairs. No
`lsp-mux.sock` existed on disk and `OMP_LSP_MUX_SOCKET` was unset. There was therefore no shared
address to dial for the existing tmux server. This is `NOT-AN-ATTACH-API` for the current pane
problem, not evidence that the LSP implementation itself is broken.

The source-side evidence is the installed bundle at
`/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js`, measured against
`omp/18.1.14`. Re-derive after an OMP upgrade:

```bash
B=/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent
omp --version
grep -oE '"omp/(muxConnect|muxPing|muxRestartServer)"' "$B/dist/cli.js" | sort -u
pgrep -fl 'lsp.mux'
find "$HOME/.omp" -name 'lsp-mux.sock' -print
env | grep '^OMP_LSP_MUX_SOCKET='
```

## Route decision

Choose the **validated profile-store reader** for existing panes:

```text
tmux pane %N
  -> pane child argv: omp --profile <profile>
  -> ~/.omp/profiles/<profile>/agent/terminal-sessions/tmux-%N
  -> line 2 candidate session JSONL
  -> validate candidate is under <profile>/agent/sessions/
  -> state-only OMP reader
  -> typed pane state
```

The store is an input, not proof of liveness: it records the **last session opened**. The current
Codex `tmux-%8` entry points outside the profile's `agent/sessions/` root at a scratch conformance
fixture, so that mapping must produce `UNKNOWN`, not live-agent state.

`omp acp` is a valid JSON-RPC-over-stdio protocol for a client-supervised subprocess, but it is
point-to-point and cannot attach to an already-running pane. A new pane-keyed mux would provide a
separately addressable and potentially push-capable endpoint. No current consumer requires that
beyond a validated reader, so building it now would duplicate supervision, socket lifecycle, and
recovery without adding observed capability. Revisit only for a consumer requiring remote
subscriptions or control unavailable through the reader.

## Entry contract observed by the reader owner

Across 64 profile-store entries (Claude 22, Codex 42):

- 3-line entries contain `cwd`, a candidate session JSONL path, and `fresh`.
- 2-line entries contain only `cwd` and a candidate session JSONL path.
- The only observed line-3 token is `fresh`, present in 14 entries.
- 50 entries have no line 3; missing state is `Unknown/MissingStateToken`.
- A line-2 path outside the profile's `agent/sessions/` root is `Unknown/ForeignSessionPath`.
- Unobserved state tokens remain `UNKNOWN`; they are not an absent domain.

## Skill delta

**CLAIM** Future agents must treat `omp/mux*` as LSP plumbing, not as a way to attach to an
existing tmux pane; use the validated profile-store chain first and return `UNKNOWN` for fixture or
foreign-session paths.

**COMMAND**

```bash
B=/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent
omp --version
grep -oE '"omp/(muxConnect|muxPing|muxRestartServer)"' "$B/dist/cli.js" | sort -u
```

**PROVENANCE** `omp --version` measured `omp/18.1.14`; bundle tree is the host-installed path above;
repo documentation change is committed at `f4450e9` (with the initial mux correction at `f2ac970`).

**NO-CLAIM:** This reference does not prove an `omp/mux*` foreign-session request was accepted or
rejected; it proves the observed workers exposed no shared dialable socket and that the methods
name LSP plumbing. The profile-store route is version- and host-state-bound.

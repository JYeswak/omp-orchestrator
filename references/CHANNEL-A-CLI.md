# Channel A: OMP CLI Surface

**Status:** measured reference; no `omp-inventory-map` source was edited by this audit.

**Provenance:** `omp --version` returned `omp/18.1.14`. The current installed CLI help returned 39
subcommands. Counts below are derived from commands, not transcribed from `AGENTS.md` or an older
OMP release.

## Scope and verdict vocabulary

This table asks whether a headless OMP orchestrator can safely call a subcommand and use its result
as runtime state or control. “Consumable” does not mean that a human can read the output.

- **CONSUMABLE** — a bounded machine-readable state, telemetry, or control surface that can feed an
  orchestrator. `ps` is already represented by the `daemon_process` axis and must not be declared a
  second time as `cli`.
- **OPERATOR-ONLY / DELIBERATELY_NOT** — the command is interactive, mutating, credential-bearing,
  user-facing, or unrelated to OMP lifecycle control. Each row carries an owner, reason, and
  `dies_when` condition for the empty allowance list.
- **UNKNOWN** — help did not determine whether a consumer can use the result. No current row needed
  this verdict; an unknown must not be converted into an exclusion by guessing.

## Re-runnable command census

```bash
omp --version
p=$(mktemp)
omp --help >"$p"
rc=$?
printf 'help_rc=%s\n' "$rc"
printf 'subcommands='; awk '/^COMMANDS$/{seen=1;next} seen && NF==0{exit} seen && $1 ~ /^[a-z][a-z0-9-]*$/ {n++} END{print n+0}' "$p"
printf 'flag_lines='; grep -cE '^\s+--[a-z]' "$p"
printf 'unique_flag_names='; grep -oE '^\s+--[a-z][a-z0-9-]*' "$p" | sed 's/^[[:space:]]*--//' | sort -u | wc -l
rm -f "$p"
```

Measured output:

```text
omp/18.1.14
help_rc=0
subcommands=39
flag_lines=46
unique_flag_names=39
```

## Per-subcommand table

The help description is the first non-empty line from `omp <verb> --help`. Every help invocation
returned `rc=0`.

| name | `--help` one-liner | verdict | reason / owner / dies_when |
|---|---|---|---|
| `acp` | Run Oh My Pi as an ACP (Agent Client Protocol) server over stdio | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; point-to-point subprocess server, not an attach-to-live-pane control surface; `dies_when=OMP publishes an attach-existing-pane ACP contract`. |
| `agents` | Manage bundled task agents | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; `unpack` writes agent files into user or project configuration; `dies_when=agent export becomes a read-only runtime registry`. |
| `auth-broker` | Manage the omp auth-broker (credential vault) | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; credential-vault login, migration, token, and service operations cross the secret/operator boundary; `dies_when=OMP exposes a redacted, read-only auth-health contract for orchestration`. |
| `auth-gateway` | Run an auth-gateway forward proxy backed by the configured broker | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; starts or controls a network proxy; `dies_when=the gateway publishes a bounded health/readiness API consumed by this orchestrator`. |
| `bench` | Benchmark models: TTFT/prefill vs decode throughput with p50/p95, across chat, prefill, generation, and prompt-cache workloads | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; provider/model benchmark is an evaluation workload, not live lifecycle state; `dies_when=a scheduled benchmark consumer and budget contract are added`. |
| `browser-relay` | Run the local CDP relay that lets the browser prelude drive your own Chrome tabs | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; starts a local browser-control service; `dies_when=the orchestrator owns a typed relay health/control contract`. |
| `cleanse` | Detect and fix project diagnostics with weighted parallel subagents | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; launches subagents and can fix project state; `dies_when=it exposes a read-only, bounded diagnostic result with no agent or file mutation`. |
| `commit` | Generate a commit message and update changelogs | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; edits repository history-adjacent artifacts and is human-reviewed; `dies_when=the orchestrator explicitly owns a commit-generation protocol`. |
| `completions` | Print a shell completion script (bash, zsh, or fish) | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; emits shell ergonomics rather than runtime state; `dies_when=the completion output becomes an input to a declared runtime consumer`. |
| `compress` | Rewrite a text file into the dense prompt register, reporting what it drops | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; rewrites caller-selected files; `dies_when=the orchestrator owns a reversible prompt-compaction API`. |
| `config` | Manage configuration settings | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; configuration mutation changes future sessions; `dies_when=OMP exposes a read-only effective-config snapshot needed by a consumer`. |
| `dry-balance` | Dry-run OAuth account balancing across random session ids | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; auth-account policy simulation is not OMP session lifecycle state; `dies_when=account admission becomes a declared orchestration input`. |
| `gallery` | Preview tool, composer, and status-line renderers in a deterministic visual gallery | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; visual QA surface; `dies_when=an automated visual regression consumer is declared`. |
| `gc` | Run storage garbage collection | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; performs storage reclamation; `dies_when=OMP publishes a read-only GC readiness/result contract`. |
| `git` | Interactive fullscreen git UI: split diff viewer, staging sidebar, and commit composer | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; interactive terminal UI and repository mutation; `dies_when=OMP publishes a non-interactive typed git operation contract`. |
| `grep` | Test grep tool | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; diagnostic tool invocation, not OMP lifecycle state; `dies_when=the orchestrator declares grep output as a stable control-plane input`. |
| `grievances` | View, clean, or push reported tool issues (auto-QA grievances) | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; QA issue maintenance includes deletion and push actions; `dies_when=a read-only, authenticated grievance feed is an explicit fleet input`. |
| `if-bench` | Benchmark instruction following and working memory: one cached thread of glyph array actions with a moving cat-sound directive | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; evaluation workload, not runtime control; `dies_when=a scheduled benchmark consumer and cost policy are added`. |
| `images` | Inspect, diagnose, probe, and purge image publication backends | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; publication health plus purge mutation is outside this OMP lifecycle; `dies_when=a declared image-backend health consumer exists`. |
| `install` | Install or link an extension package (alias of `plugin install`/`plugin link`) | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; mutates installed extensions and trust surface; `dies_when=extension installation is replaced by a reviewed typed deployment protocol`. |
| `join` | Join a shared collab session (same as /join) | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; human collaboration entry point; `dies_when=collab session membership is an explicit orchestrator-owned resource`. |
| `models` | List, search, and refresh available models | CONSUMABLE | Machine-readable model catalog can inform model selection. `owner=future model-admission consumer`; `dies_when=model catalog is supplied by a typed in-process integration`. |
| `plugin` | Manage plugins (install, uninstall, list, etc.) | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; plugin lifecycle mutates executable extension code; `dies_when=reviewed plugin state is exposed as a read-only contract`. |
| `ps` | List and control daemon-supervised background processes (logs, stop, kill, restart) | CONSUMABLE / EXISTING COLLISION | `%20` already owns this as `daemon_process`; `owner=ompo-doctor`; do not duplicate under `cli`; `dies_when=daemon process state moves to a typed OMP-native lifecycle surface`. |
| `read` | Show what the read tool will return for a path, URL, or internal URI | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; generic operator/tool read surface, not a stable OMP lifecycle input; `dies_when=the orchestrator declares a bounded URI/file source contract`. |
| `render` | Draw a session's entire thread through the production transcript pipeline (with repaint timing) | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; presentation and repaint diagnostics; `dies_when=render timing becomes a declared automated quality signal`. |
| `say` | Synthesize text with the local TTS engine and play it through the speakers | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; local audio side effect; `dies_when=voice output is an explicit orchestrator-owned notification channel`. |
| `search` | Test web search providers | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; external research workload, not OMP session state; `dies_when=search results become a declared bounded research input`. |
| `setup` | Run onboarding setup or install dependencies for optional features | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; installs dependencies and changes machine state; `dies_when=setup becomes a declarative, reversible provisioning API`. |
| `share` | Share a saved session via an encrypted link (same as /share) | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; external publication of session data; `dies_when=the orchestrator owns an approved session-publication contract`. |
| `shell` | Interactive shell console | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; interactive terminal and arbitrary command execution; `dies_when=OMP offers a bounded typed command-execution API accepted by this orchestrator`. |
| `ssh` | Manage SSH host configurations | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; edits operator connection configuration; `dies_when=remote host state is exposed through a declared non-interactive control plane`. |
| `stats` | View usage statistics | CONSUMABLE | Machine-readable usage telemetry can feed admission or cost reporting. `owner=future stats consumer`; `dies_when=stats are exposed by the typed RPC statistics response already consumed by ompo stats.` |
| `tiny-models` | Download tiny local models (session titles + memory) | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; downloads and changes local model state; `dies_when=local-model lifecycle is an explicit provisioned dependency`. |
| `token` | Get the API key or OAuth token for a provider | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; direct credential emission is outside an orchestrator consumer boundary; `dies_when=OMP exposes redacted credential-health metadata without token material`. |
| `ttsr` | Inspect and test Time-Traveling Stream Rules (TTSR) | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; rule diagnostics and source scanning, not OMP lifecycle state; `dies_when=TTSR verdicts become a declared CI/runtime gate input`. |
| `update` | Check for and install updates | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; changes the installed OMP binary; `dies_when=updates are handled by an approved release controller`. |
| `usage` | Show provider usage limits for every authenticated account | CONSUMABLE | Machine-readable quota telemetry can inform admission and provider routing. `owner=future quota consumer`; `dies_when=quota data is supplied by a typed provider-usage service`. |
| `worktree` | Add, list, or clear git worktrees (clone-first when enabled) | OPERATOR-ONLY / DELIBERATELY_NOT | `owner=omp-inventory-map`; repository topology mutation conflicts with this workspace's zero-worktree policy; `dies_when=the workspace policy explicitly permits orchestrator-owned worktrees`. |

No row is `UNKNOWN`: every invocation supplied a usable description and usage shape. This is not a
claim that help proves runtime behavior; commands classified `CONSUMABLE` still require a separate
runtime contract before adoption.

## Slash-command reconciliation

`omp-inventory-map`'s current parser recursively walks every `subcommands` array in the RPC startup
frame. The live stream and the static bundle are different subjects.

Live stream probe:

```bash
p=$(mktemp)
printf '' | omp --mode=rpc --no-session --no-tools --max-time=5 >"$p"
rc=$?
printf 'rc=%s\n' "$rc"
jq -c 'select(.type == "available_commands_update") |
  {top:(.commands|length), top_unique:(.commands|map(.name)|unique|length),
   recursive_paths:([.commands[]|..|objects|select(has("name"))|.name]|unique|length)}' "$p"
rm -f "$p"
```

Measured three times with `rc=0`:

```text
top=591
top_unique=591
recursive_paths=662
```

Static bundle comparison:

```bash
B=/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent
grep -oE '"/[a-z][a-z0-9_-]*"' "$B/dist/cli.js" | sort -u | wc -l
```

Measured output: `48`.

The repository constant is still `EXPECTED_SLASH_COMMANDS = 136`, pinned to `omp/18.0.11`. The
current observations are therefore **591 top-level RPC entries / 662 recursively collected paths /
48 static literal matches**, not a single reconciled count. Neither 136 nor 48 is transcribed as the
truth. The owner must decide whether the expected contract is top-level entries, recursive paths,
or a filtered slash-command subset, then derive that subject with a versioned command.

## CLI flag-axis decision

The current global help has 46 flag lines and 39 unique long-flag names. The relevant names are
`mode`, `profile`, and `model`:

- `--mode` is already represented by the `transport_mode` surface.
- `--profile` selects the profile-rooted pane/session store and must remain invocation provenance,
  not a separate consumable operation.
- `--model` selects the model object and must remain model/session provenance; the installed RPC
  declaration says `model` is an object, not a string.
- The remaining flags are operator configuration, feature switches, credentials, or output policy.

**Recommendation: add no fifth flag axis.** Put `profile` and `model` in the invocation metadata of
existing consumers and retain `mode` under `transport_mode`. Adding a `cli_flag` kind for two
load-bearing flags plus 44 placeholders would recreate the unclassifiable `transport_mode:value`
problem and force every alignment run to become partial. If a future consumer requires a flag-level
contract, add one named flag surface with a typed payload and a real declaration rather than
classifying all 46 by default.

## No-claim boundary

The help table classifies command roles from the installed help surface; it does not prove that a
command's implementation is safe to invoke. The slash counts are different subjects and remain
unreconciled. The idle RPC probe does not test a prompt-triggered event or a live resumed pane. This
reference does not edit `omp-inventory-map`, `ompo-doctor`, or any OMP installation.
